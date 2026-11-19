//! `AnimatedImagePreview.PlayAsync`: an animated GIF or WebP played until
//! whoever asked stops wanting it (the hovered thumbnail, the selected
//! preview, the viewer's current image).
//!
//! Frames are decoded (and fitted, by the caller's `prepare`) on a thread of
//! their own, one ahead of what's shown, and shown when ShareX's clock says:
//! `nextFrameTime = Math.Max(nextFrameTime + duration, elapsed)`. The UI
//! polls [`Player::poll`] while it paints and asks to be repainted when the
//! next frame is due ([`Poll::Wait`]). Dropping the player stops playback
//! (`isCurrent()` false).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::time::{Duration, Instant};

use sharex_tools::animated_preview::{AnimatedImagePreview, next_frame_time};
use sharex_tools::imaging::Bitmap;

/// Turns a decoded frame (premultiplied) into what's shown, on the
/// decoder's thread.
pub type Prepare = Box<dyn Fn(Bitmap) -> image::RgbaImage + Send>;

/// What [`Player::poll`] has for the UI.
#[derive(Debug)]
pub enum Poll {
    /// A frame to show now.
    Frame(image::RgbaImage),
    /// Nothing new: look again after this long (or once the decoder answers).
    Wait(Duration),
    /// The file isn't animated (or decoding failed before any frame): show
    /// the still image (`PlayAsync` returned false).
    NotAnimated,
    /// Playback stopped after an error (`break` in `PlayAsync`).
    Ended,
}

pub struct Player {
    rx: Receiver<(image::RgbaImage, u32)>,
    stop: Arc<AtomicBool>,
    started: Option<Instant>,
    /// When the next frame is due, in ms since the first frame.
    next_ms: u64,
    shown_any: bool,
    done: Option<bool>,
}

impl Player {
    /// Starts decoding `path` at most `max_size` large (`PlayAsync(filePath,
    /// maxSize, ...)`).
    pub fn start(path: PathBuf, max_size: u32, prepare: Prepare) -> Self {
        Self::start_with(move || AnimatedImagePreview::try_create(&path, max_size), prepare)
    }

    fn start_with(open: impl FnOnce() -> Option<AnimatedImagePreview> + Send + 'static, prepare: Prepare) -> Self {
        // One frame ahead of what's shown.
        let (tx, rx) = sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        std::thread::spawn(move || {
            let Some(mut animation) = open() else { return };
            while !flag.load(Ordering::Relaxed) {
                let (frame, duration) = match animation.next_frame() {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("{e}");
                        break;
                    }
                };
                if flag.load(Ordering::Relaxed) || tx.send((prepare(frame), duration)).is_err() {
                    break;
                }
            }
        });
        Self { rx, stop, started: None, next_ms: 0, shown_any: false, done: None }
    }

    /// The frame to show now, or how long until there is one.
    pub fn poll(&mut self, now: Instant) -> Poll {
        if let Some(animated) = self.done {
            return if animated { Poll::Ended } else { Poll::NotAnimated };
        }
        let elapsed = self.started.map_or(0, |s| now.duration_since(s).as_millis() as u64);
        if self.started.is_some() && elapsed < self.next_ms {
            return Poll::Wait(Duration::from_millis(self.next_ms - elapsed));
        }
        match self.rx.try_recv() {
            Ok((frame, duration)) => {
                let start = *self.started.get_or_insert(now);
                let elapsed = now.duration_since(start).as_millis() as u64;
                self.next_ms = next_frame_time(self.next_ms, duration, elapsed);
                self.shown_any = true;
                Poll::Frame(frame)
            }
            // The decoder is behind: look again soon.
            Err(TryRecvError::Empty) => Poll::Wait(Duration::from_millis(5)),
            Err(TryRecvError::Disconnected) => {
                self.done = Some(self.shown_any);
                self.poll(now)
            }
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Premultiplied to straight RGBA, as Avalonia shows a premultiplied frame.
pub fn straight(bmp: Bitmap) -> image::RgbaImage {
    let (w, h) = (bmp.width(), bmp.height());
    image::RgbaImage::from_raw(w, h, bmp.to_straight()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gif(w: u16, h: u16, colours: &[[u8; 3]], delay: u16) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let palette: Vec<u8> = colours.iter().flatten().copied().collect();
            let mut enc = gif::Encoder::new(&mut out, w, h, &palette).unwrap();
            for i in 0..colours.len() {
                let f = gif::Frame {
                    width: w,
                    height: h,
                    delay,
                    buffer: std::borrow::Cow::Owned(vec![i as u8; w as usize * h as usize]),
                    ..Default::default()
                };
                enc.write_frame(&f).unwrap();
            }
        }
        out
    }

    /// Waits for the decoder thread (frames come one ahead).
    fn next(p: &mut Player, now: Instant) -> Poll {
        for _ in 0..500 {
            match p.poll(now) {
                Poll::Wait(d) if d <= Duration::from_millis(5) => std::thread::sleep(Duration::from_millis(2)),
                other => return other,
            }
        }
        panic!("no frame");
    }

    #[test]
    fn frames_come_when_they_are_due() {
        let bytes = gif(2, 2, &[[255, 0, 0], [0, 255, 0]], 10);
        let mut p = Player::start_with(move || AnimatedImagePreview::from_bytes(bytes, 64), Box::new(straight));
        let t0 = Instant::now();
        let Poll::Frame(f) = next(&mut p, t0) else { panic!() };
        assert_eq!(f.get_pixel(0, 0).0, [255, 0, 0, 255]);
        // 100 ms per frame: nothing new until then.
        let Poll::Wait(d) = p.poll(t0 + Duration::from_millis(40)) else { panic!() };
        assert_eq!(d, Duration::from_millis(60));
        let Poll::Frame(f) = next(&mut p, t0 + Duration::from_millis(100)) else { panic!() };
        assert_eq!(f.get_pixel(0, 0).0, [0, 255, 0, 255]);
        // Late: shown at once, and the clock restarts from now, so the
        // next one is due at once too (`Math.Max(next + duration, elapsed)`).
        let Poll::Frame(f) = next(&mut p, t0 + Duration::from_millis(350)) else { panic!() };
        assert_eq!(f.get_pixel(0, 0).0, [255, 0, 0, 255]);
        let Poll::Frame(f) = next(&mut p, t0 + Duration::from_millis(350)) else { panic!() };
        assert_eq!(f.get_pixel(0, 0).0, [0, 255, 0, 255]);
        let Poll::Wait(d) = p.poll(t0 + Duration::from_millis(400)) else { panic!() };
        assert_eq!(d, Duration::from_millis(50));
    }

    #[test]
    fn a_still_image_reports_not_animated() {
        let bytes = gif(2, 2, &[[1, 2, 3]], 10);
        let mut p = Player::start_with(move || AnimatedImagePreview::from_bytes(bytes, 64), Box::new(straight));
        assert!(matches!(next(&mut p, Instant::now()), Poll::NotAnimated));
    }
}
