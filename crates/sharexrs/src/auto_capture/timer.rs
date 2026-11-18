//! `AutoCaptureWindow`'s timing and status text without the window: its two
//! `DispatcherTimer`s (the screenshot timer, 1 s to the first capture then
//! the repeat time, 1 s retries while tasks are busy; the 250 ms status
//! timer), the stopwatch, the capture count, and `UpdateStatus`'s formulas.
//! Checked against .NET by `tools/scrolling-harness --auto`.

use std::time::{Duration, Instant};

use sharex_tools::text::{Arg, format_dotnet};

/// `_statusTimer.Interval`.
pub const STATUS_INTERVAL: Duration = Duration::from_millis(250);

/// `(int)(ApplicationState.Settings.AutoCaptureRepeatTime * 1000)`: a
/// `decimal` in ShareX, so the value is taken as the decimal it was written
/// as (its shortest form) and truncated to whole milliseconds.
pub fn repeat_ms(repeat_time: f64) -> i32 {
    if !repeat_time.is_finite() {
        return 0;
    }
    let s = format!("{}", repeat_time.abs());
    let (int, frac) = s.split_once('.').unwrap_or((&s, ""));
    let mut ms: i128 = int.parse::<i128>().unwrap_or(0) * 1000;
    let frac: String = frac.chars().chain(std::iter::repeat('0')).take(3).collect();
    ms += frac.parse::<i128>().unwrap_or(0);
    if repeat_time < 0.0 {
        ms = -ms;
    }
    ms.clamp(i32::MIN as i128, i32::MAX as i128) as i32
}

/// `(timeLeft / 1000f).ToString("0.0")`: .NET formats a `float` with a
/// custom format from its 7 significant digits, then rounds those half up.
pub fn seconds_left_text(time_left_ms: i32) -> String {
    let x = time_left_ms as f32 / 1000.0;
    let neg = x < 0.0;
    let s = format!("{:.6e}", (x as f64).abs());
    let (mantissa, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let digits: u128 = mantissa.replace('.', "").parse().unwrap_or(0);
    let exp: i32 = exp.parse().unwrap_or(0);
    // `x * 10` = digits * 10^(exp + 1 - 6).
    let k = exp + 1 - 6;
    let tenths = if k >= 0 {
        digits * 10u128.pow(k as u32)
    } else if -k > 30 {
        0
    } else {
        let d = 10u128.pow((-k) as u32);
        let (q, r) = (digits / d, digits % d);
        if r * 2 >= d { q + 1 } else { q }
    };
    let text = format!("{}.{}", tenths / 10, tenths % 10);
    if neg && tenths != 0 { format!("-{text}") } else { text }
}

/// `(int)(100 - (double)timeLeft / _delay * 100)`, 100 with no delay.
pub fn percentage(time_left: i32, delay: i32) -> i32 {
    if delay > 0 { (100.0 - time_left as f64 / delay as f64 * 100.0) as i32 } else { 100 }
}

/// `Strings.AutoCaptureWindow_Status`.
pub fn status_text(seconds_left: &str, percentage: i32, count: i32) -> String {
    format_dotnet(sharex_strings::sharex::AutoCaptureWindow_Status(), &[seconds_left.into(), percentage.into(), count.into()])
}

/// `UpdateRegion`'s `RegionText`.
pub fn region_text(r: sharex_settings::types::Rectangle) -> String {
    if is_empty(r) {
        return sharex_strings::sharex::AutoCaptureWindow_NoRegion().to_string();
    }
    format_dotnet(sharex_strings::sharex::AutoCaptureWindow_Region(), &[Arg::I64(r.x as i64), Arg::I64(r.y as i64), Arg::I64(r.width as i64), Arg::I64(r.height as i64)])
}

/// `Rectangle.IsEmpty`: every field 0.
pub fn is_empty(r: sharex_settings::types::Rectangle) -> bool {
    r.x == 0 && r.y == 0 && r.width == 0 && r.height == 0
}

/// The screenshot timer, stopwatch and count (`_screenshotTimer`,
/// `_stopwatch`, `_count`, `_delay`, `_waitUploads`).
#[derive(Clone, Debug, Default)]
pub struct AutoCaptureTimer {
    /// `IsRunning`.
    pub running: bool,
    /// When the screenshot timer ticks next.
    pub next_tick: Option<Instant>,
    /// When the status timer ticks next.
    pub next_status: Option<Instant>,
    /// `_delay`, ms.
    pub delay: i32,
    pub wait_uploads: bool,
    /// `_count`: never reset (the window keeps it while it's open).
    pub count: i32,
    /// `_stopwatch`, restarted at every capture, reset by Stop.
    stopwatch: Option<Instant>,
}

impl AutoCaptureTimer {
    /// `Start`: the first capture one second after.
    pub fn start(&mut self, now: Instant, repeat_time: f64, wait_uploads: bool) {
        self.running = true;
        self.delay = repeat_ms(repeat_time);
        self.wait_uploads = wait_uploads;
        self.next_tick = Some(now + Duration::from_secs(1));
        self.next_status = Some(now + STATUS_INTERVAL);
    }

    /// `Stop`.
    pub fn stop(&mut self) {
        self.running = false;
        self.next_tick = None;
        self.next_status = None;
        self.stopwatch = None;
    }

    /// `OnScreenshotTimerTick` at `now` if it's due: true when a
    /// screenshot is taken now (`TakeScreenshot`). While tasks are busy and
    /// `AutoCaptureWaitUpload`, it tries again in a second.
    pub fn tick(&mut self, now: Instant, tasks_busy: bool) -> bool {
        if !self.running || self.next_tick.is_none_or(|t| t > now) {
            return false;
        }
        if self.wait_uploads && tasks_busy {
            self.next_tick = Some(now + Duration::from_secs(1));
            return false;
        }
        self.stopwatch = Some(now);
        // The interval set in the tick restarts the timer from now.
        self.next_tick = Some(now + Duration::from_millis(self.delay.max(1) as u64));
        self.count += 1;
        true
    }

    /// The status timer: true when `UpdateStatus` runs now.
    pub fn status_due(&mut self, now: Instant) -> bool {
        match self.next_status {
            Some(t) if self.running && t <= now => {
                self.next_status = Some(now + STATUS_INTERVAL);
                true
            }
            _ => false,
        }
    }

    /// `UpdateStatus`: the progress (0..=100) and the text at `now`.
    pub fn status(&self, now: Instant) -> (i32, String) {
        let elapsed = self.stopwatch.map_or(0, |s| now.saturating_duration_since(s).as_millis().min(i32::MAX as u128) as i32);
        let time_left = (self.delay - elapsed).max(0);
        let p = percentage(time_left, self.delay);
        (p.clamp(0, 100), status_text(&seconds_left_text(time_left), p, self.count))
    }

    /// When the event loop should wake for the timers.
    pub fn wake_at(&self) -> Option<Instant> {
        match (self.next_tick, self.next_status) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn sharex() -> Value {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/auto_capture/results-sharex.json");
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    }

    #[test]
    fn auto_capture_seconds_left_match_dotnet() {
        let v = sharex();
        let mut n = 0;
        for pair in v["seconds_left"].as_str().unwrap().split(' ') {
            let (t, want) = pair.split_once('=').unwrap();
            let t: i32 = t.parse().unwrap();
            assert_eq!(seconds_left_text(t), want, "timeLeft {t}");
            n += 1;
        }
        assert_eq!(n, 22002);
    }

    #[test]
    fn auto_capture_percentages_and_repeat_times_match_dotnet() {
        let v = sharex();
        for p in v["percentages"].as_array().unwrap() {
            let (d, t, want) = (p[0].as_i64().unwrap() as i32, p[1].as_i64().unwrap() as i32, p[2].as_i64().unwrap() as i32);
            assert_eq!(percentage(t, d), want, "delay {d} timeLeft {t}");
        }
        for r in v["repeat_ms"].as_array().unwrap() {
            let s = r[0].as_str().unwrap();
            assert_eq!(repeat_ms(s.parse().unwrap()), r[1].as_i64().unwrap() as i32, "{s}");
        }
    }

    #[test]
    fn auto_capture_texts_match_sharex() {
        let v = sharex();
        for s in v["status_text"].as_array().unwrap() {
            let (t, p, n) = (s[0].as_i64().unwrap() as i32, s[1].as_i64().unwrap() as i32, s[2].as_i64().unwrap() as i32);
            assert_eq!(status_text(&seconds_left_text(t), p, n), s[3].as_str().unwrap());
        }
        for r in v["region_text"].as_array().unwrap() {
            let rect = sharex_settings::types::Rectangle::new(r[0].as_i64().unwrap() as i32, r[1].as_i64().unwrap() as i32, r[2].as_i64().unwrap() as i32, r[3].as_i64().unwrap() as i32);
            assert_eq!(region_text(rect), r[4].as_str().unwrap());
        }
    }

    #[test]
    fn auto_capture_timer_ticks_as_sharex() {
        let t0 = Instant::now();
        let mut t = AutoCaptureTimer::default();
        t.start(t0, 2.5, true);
        assert!(!t.tick(t0 + Duration::from_millis(999), false));
        // Busy: a second more.
        assert!(!t.tick(t0 + Duration::from_secs(1), true));
        assert_eq!(t.next_tick, Some(t0 + Duration::from_secs(2)));
        assert!(t.tick(t0 + Duration::from_secs(2), false));
        assert_eq!((t.count, t.next_tick), (1, Some(t0 + Duration::from_millis(4500))));
        let (p, text) = t.status(t0 + Duration::from_millis(3250));
        assert_eq!(p, 50);
        assert_eq!(text, status_text("1.3", 50, 1));
        t.stop();
        assert!(!t.running && t.wake_at().is_none());
        // The count carries on after a restart.
        t.start(t0, 1.0, false);
        assert!(t.tick(t0 + Duration::from_secs(1), true));
        assert_eq!(t.count, 2);
    }
}
