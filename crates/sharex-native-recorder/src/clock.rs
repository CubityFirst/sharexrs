//! `RecordingClock.cs`: the media clock. WGC's `SystemRelativeTime` and
//! WASAPI's `QPCPosition` both count from the system QPC epoch in 100 ns,
//! so every timestamp the recorder sees is on [`TimeSource::now`]'s scale.

use std::sync::{Arc, Mutex};

use crate::options::TICKS_PER_SECOND;

/// Where "now" comes from: QPC for real recordings, a virtual clock in tests.
pub trait TimeSource: Send + Sync {
    /// `RecordingClock.Now`, in 100 ns.
    fn now(&self) -> i64;
}

/// `Stopwatch.GetTimestamp() * TicksPerSecond / Stopwatch.Frequency`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemTime;

impl TimeSource for SystemTime {
    fn now(&self) -> i64 {
        qpc_now()
    }
}

/// QPC in 100 ns (the epoch WGC and WASAPI timestamps use).
#[cfg(windows)]
pub fn qpc_now() -> i64 {
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    let (mut counter, mut frequency) = (0i64, 0i64);
    // SAFETY: both write an i64.
    unsafe {
        let _ = QueryPerformanceCounter(&mut counter);
        let _ = QueryPerformanceFrequency(&mut frequency);
    }
    (counter as i128 * TICKS_PER_SECOND as i128 / frequency.max(1) as i128) as i64
}

/// A monotonic clock in 100 ns (no QPC outside Windows).
#[cfg(not(windows))]
pub fn qpc_now() -> i64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    // Starts at a second so that "0 = never" checks stay false.
    let epoch = *EPOCH.get_or_init(Instant::now);
    TICKS_PER_SECOND + (epoch.elapsed().as_nanos() / 100) as i64
}

#[derive(Debug, Default)]
struct State {
    origin: i64,
    pause_started: i64,
    paused_time: i64,
    segment_start: i64,
    stopped_elapsed: i64,
    started: bool,
    paused: bool,
    stopped: bool,
}

/// `RecordingClock`.
pub struct RecordingClock {
    time: Arc<dyn TimeSource>,
    state: Mutex<State>,
}

impl RecordingClock {
    pub fn new(time: Arc<dyn TimeSource>) -> Self {
        Self { time, state: Mutex::new(State::default()) }
    }

    pub fn now(&self) -> i64 {
        self.time.now()
    }

    pub fn time_source(&self) -> Arc<dyn TimeSource> {
        self.time.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_paused(&self) -> bool {
        self.lock().paused
    }

    /// `Elapsed`: media time, paused time excluded.
    pub fn elapsed(&self) -> i64 {
        let s = self.lock();
        if s.stopped {
            s.stopped_elapsed
        } else if s.started {
            (if s.paused { s.pause_started } else { self.now() }) - s.origin - s.paused_time
        } else {
            0
        }
    }

    pub fn start(&self, timestamp: i64) {
        let mut s = self.lock();
        s.origin = timestamp;
        s.segment_start = timestamp;
        s.started = true;
    }

    pub fn pause(&self) {
        let now = self.now();
        let mut s = self.lock();
        if s.started && !s.paused && !s.stopped {
            s.pause_started = now;
            s.paused = true;
        }
    }

    pub fn resume(&self) {
        let now = self.now();
        let mut s = self.lock();
        if s.paused && !s.stopped {
            s.segment_start = now;
            s.paused_time += s.segment_start - s.pause_started;
            s.paused = false;
        }
    }

    pub fn stop(&self) {
        let now = self.now();
        let mut s = self.lock();
        if s.stopped {
            return;
        }
        s.stopped_elapsed = if s.started { (if s.paused { s.pause_started } else { now }) - s.origin - s.paused_time } else { 0 };
        s.stopped = true;
        s.paused = false;
    }

    /// `TryMap`: a QPC timestamp on the media timeline, and the segment it
    /// belongs to; `ok` only while recording and from the segment's start.
    pub fn try_map(&self, timestamp: i64) -> (i64, i64, bool) {
        let s = self.lock();
        let relative = timestamp - s.origin - s.paused_time;
        let segment = s.segment_start;
        (relative, segment, s.started && !s.paused && !s.stopped && timestamp >= s.segment_start)
    }
}

/// A clock that only moves when told to, for tests and the harness.
#[derive(Debug, Default)]
pub struct VirtualTime {
    now: std::sync::atomic::AtomicI64,
}

impl VirtualTime {
    pub fn new(start: i64) -> Arc<Self> {
        Arc::new(Self { now: std::sync::atomic::AtomicI64::new(start) })
    }
    pub fn advance(&self, ticks: i64) {
        self.now.fetch_add(ticks, std::sync::atomic::Ordering::SeqCst);
    }
    pub fn set(&self, ticks: i64) {
        self.now.store(ticks, std::sync::atomic::Ordering::SeqCst);
    }
}

impl TimeSource for VirtualTime {
    fn now(&self) -> i64 {
        self.now.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_excludes_time() {
        let t = VirtualTime::new(1_000);
        let c = RecordingClock::new(t.clone());
        assert_eq!(c.elapsed(), 0);
        c.start(1_000);
        t.advance(500);
        assert_eq!(c.elapsed(), 500);
        c.pause();
        t.advance(300);
        assert_eq!(c.elapsed(), 500);
        assert!(!c.try_map(1_600).2);
        c.resume();
        assert_eq!(c.try_map(1_800), (500, 1_800, true));
        assert!(!c.try_map(1_799).2);
        t.advance(100);
        assert_eq!(c.elapsed(), 600);
        c.stop();
        t.advance(100);
        assert_eq!(c.elapsed(), 600);
        assert!(!c.is_paused());
    }

    #[test]
    fn stop_while_paused() {
        let t = VirtualTime::new(0);
        let c = RecordingClock::new(t.clone());
        c.start(0);
        t.advance(10);
        c.pause();
        t.advance(10);
        c.stop();
        assert_eq!(c.elapsed(), 10);
        assert!(!c.is_paused());
    }
}
