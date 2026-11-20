//! The video player's and trimmer's playback: what ShareX's
//! `WindowsMediaPlayer` (`ShareX.Tools/Controls/Media`) does with Media
//! Foundation's `IMFMediaEngine` (Windows only), done on every platform
//! with the FFmpeg ShareX's other video tools run:
//!
//! - `LoadAsync` probes the file (`ffmpeg -i`): its duration, the first
//!   video stream's size, frame rate and aspect, and whether it has sound.
//!   No video stream, or no finite duration, is `VideoTrimmer_InvalidVideo`.
//!   A file FFmpeg can't read at all (`AVERROR_INVALIDDATA`, "Invalid data
//!   found when processing input") is what Media Foundation reports for
//!   the same file: `MF_E_END_OF_STREAM`, as .NET's `COMException` words it
//!   (`<system message> (0xC00D3E84)`; checked live with a text file named
//!   `invalid.mp4`, VID-04). See [`ffmpeg_failure_as_media_foundation`].
//! - Playing runs two FFmpeg processes from the position: one writes the
//!   video as RGBA frames to a pipe, the other the first audio stream as
//!   32-bit float PCM, which cpal plays on the default output device at
//!   the player's `Volume`/`IsMuted`. The audio's playhead is the clock
//!   the frames follow (the wall clock without sound).
//! - `Seek` restarts them at the new time (FFmpeg's input seeking decodes
//!   to the exact frame, as `MediaEngineSeekMode.Normal`); paused, it
//!   decodes the one frame there. `StepFrame` seeks a frame forward or back.
//! - Seeking to the end keeps the last frame (`duration - 1µs`), and play
//!   at the end starts again from 0, as ShareX's.
//!
//! `PositionChanged`/`IsPlayingChanged` become [`MediaPlayer::position`]
//! and [`MediaPlayer::is_playing`], read each frame.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// `VideoPlaybackInfo` and what the pipes need.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    /// Seconds.
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    /// Display aspect ratio (`GetVideoAspectRatio`).
    pub aspect: f64,
    pub has_audio: bool,
}

/// `MF_E_END_OF_STREAM`.
const MF_E_END_OF_STREAM: u32 = 0xC00D_3E84;

/// English Windows' system message for `MF_E_END_OF_STREAM` (ShareX's
/// VID-04 shot), for where the system has none (macOS, Linux).
const END_OF_STREAM_MESSAGE: &str = "The media stream cannot process any more samples because there are no more samples in the stream.";

/// The `ex.Message` ShareX's player shows for the Media Foundation error an
/// FFmpeg failure corresponds to, or `None` when FFmpeg's output has none
/// of them (then it's `VideoTrimmer_InvalidVideo`, as ShareX's checks of
/// the loaded media).
///
/// | FFmpeg | Media Foundation (`IMFMediaEngine` error) |
/// |---|---|
/// | `Invalid data found when processing input` (`AVERROR_INVALIDDATA`) | `MF_E_END_OF_STREAM` `0xC00D3E84` |
/// | no `Duration`, or no video stream | `VideoTrimmer_InvalidVideo` (`HasVideo()`, a finite duration) |
pub fn ffmpeg_failure_as_media_foundation(output: &str) -> Option<String> {
    output.contains("Invalid data found when processing input").then(|| com_exception_message(MF_E_END_OF_STREAM, END_OF_STREAM_MESSAGE))
}

/// `Marshal.GetExceptionForHR(hr).Message`: the system's message for the
/// code (`FormatMessage`, trimmed), then the code as `(0xXXXXXXXX)`.
fn com_exception_message(hr: u32, fallback: &str) -> String {
    format!("{} (0x{hr:08X})", system_message(hr).unwrap_or_else(|| fallback.to_string()))
}

#[cfg(windows)]
fn system_message(code: u32) -> Option<String> {
    use windows_sys::Win32::System::Diagnostics::Debug::{FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW};
    let mut buf = [0u16; 1024];
    // SAFETY: the buffer and its length are valid; no inserts are read.
    let n = unsafe {
        FormatMessageW(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS, std::ptr::null(), code, 0, buf.as_mut_ptr(), buf.len() as u32, std::ptr::null())
    };
    let text = String::from_utf16_lossy(&buf[..n as usize]);
    let text = text.trim_end();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(not(windows))]
fn system_message(_code: u32) -> Option<String> {
    None
}

/// Reads `ffmpeg -i`'s stream summary.
pub fn parse_probe(output: &str) -> Result<MediaInfo, String> {
    if let Some(message) = ffmpeg_failure_as_media_foundation(output) {
        return Err(message);
    }
    let invalid = || sharex_strings::tools::VideoTrimmer_InvalidVideo().to_string();
    let duration_re = regex::Regex::new(r"Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)").expect("pattern");
    let video_re = regex::Regex::new(r"Stream #\d+:\d+[^:]*: Video: [^\n]*?, (\d+)x(\d+)([^\n]*)").expect("pattern");
    let audio_re = regex::Regex::new(r"Stream #\d+:\d+[^:]*: Audio: ").expect("pattern");
    let d = duration_re.captures(output).ok_or_else(invalid)?;
    let duration = d[1].parse::<f64>().unwrap_or(0.0) * 3600.0 + d[2].parse::<f64>().unwrap_or(0.0) * 60.0 + d[3].parse::<f64>().unwrap_or(0.0);
    let v = video_re.captures(output).ok_or_else(invalid)?;
    let (width, height): (u32, u32) = (v[1].parse().map_err(|_| invalid())?, v[2].parse().map_err(|_| invalid())?);
    if !duration.is_finite() || duration <= 0.0 || width == 0 || height == 0 {
        return Err(invalid());
    }
    let rest = &v[3];
    let number_before = |unit: &str| {
        regex::Regex::new(&format!(r"(\d+(?:\.\d+)?)(k?) {unit}"))
            .expect("pattern")
            .captures(rest)
            .and_then(|c| c[1].parse::<f64>().ok().map(|n| if &c[2] == "k" { n * 1000.0 } else { n }))
    };
    let fps = number_before("fps").or_else(|| number_before("tbr")).filter(|f| *f > 0.0 && f.is_finite()).unwrap_or(30.0);
    let dar = regex::Regex::new(r"DAR (\d+):(\d+)").expect("pattern").captures(rest).and_then(|c| {
        let (n, d) = (c[1].parse::<f64>().ok()?, c[2].parse::<f64>().ok()?);
        (n > 0.0 && d > 0.0).then_some(n / d)
    });
    Ok(MediaInfo { duration, width, height, fps, aspect: dar.unwrap_or(width as f64 / height as f64), has_audio: audio_re.is_match(output) })
}

/// `ffmpeg -hide_banner -i <file>`: the summary FFmpeg prints before it
/// says there's no output.
pub fn probe(ffmpeg: &Path, path: &str) -> Result<MediaInfo, String> {
    let out = command(ffmpeg).args(["-hide_banner", "-nostdin", "-i", path]).stdout(Stdio::null()).stderr(Stdio::piped()).output().map_err(|e| e.to_string())?;
    parse_probe(&String::from_utf8_lossy(&out.stderr))
}

fn command(ffmpeg: &Path) -> std::process::Command {
    let mut c = std::process::Command::new(ffmpeg);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c.stdin(Stdio::null());
    c
}

/// The size the frames are decoded at: the video's display size, no
/// larger than 1920x1080 (the picture is scaled to the viewport anyway).
fn frame_size(info: &MediaInfo) -> (u32, u32) {
    let w = (info.height as f64 * info.aspect).round().max(1.0);
    let h = info.height as f64;
    let scale = (1920.0 / w).min(1080.0 / h).min(1.0);
    (((w * scale).round() as u32).max(2) & !1, ((h * scale).round() as u32).max(2) & !1)
}

/// A decoded frame.
pub struct Frame {
    /// Seconds.
    pub time: f64,
    pub image: Arc<egui::ColorImage>,
}

/// Decodes RGBA frames from `start` (all of them, or `count`).
fn spawn_video(ffmpeg: &Path, path: &str, info: &MediaInfo, start: f64, count: Option<u32>) -> Result<(Child, mpsc::Receiver<Frame>), String> {
    let (w, h) = frame_size(info);
    let mut c = command(ffmpeg);
    c.args(["-hide_banner", "-loglevel", "error", "-ss", &format!("{start:.6}"), "-i", path, "-map", "0:v:0", "-an", "-sn", "-dn"]);
    c.args(["-vf", &format!("scale={w}:{h}:flags=bilinear"), "-pix_fmt", "rgba", "-fps_mode", "passthrough"]);
    if let Some(n) = count {
        c.args(["-frames:v", &n.to_string()]);
    }
    c.args(["-f", "rawvideo", "-"]).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = c.spawn().map_err(|e| e.to_string())?;
    let mut out = child.stdout.take().ok_or("no pipe")?;
    // A few frames ahead; the decoder waits on the pipe beyond that.
    let (tx, rx) = mpsc::sync_channel(4);
    let fps = info.fps;
    std::thread::spawn(move || {
        let size = (w * h * 4) as usize;
        let mut n = 0u64;
        loop {
            let mut buf = vec![0u8; size];
            if out.read_exact(&mut buf).is_err() {
                return;
            }
            let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &buf);
            let frame = Frame { time: start + n as f64 / fps, image: Arc::new(image) };
            if tx.send(frame).is_err() {
                return;
            }
            n += 1;
        }
    });
    Ok((child, rx))
}

/// What the audio callback and the PCM reader share.
struct AudioShared {
    samples: Mutex<std::collections::VecDeque<f32>>,
    /// Frames (per channel) played since the stream's start.
    played: AtomicU64,
    paused: AtomicBool,
    /// `f32` bits.
    volume: AtomicU32,
    muted: AtomicBool,
    /// The reader got to the end.
    done: AtomicBool,
}

struct Audio {
    child: Child,
    shared: Arc<AudioShared>,
    rate: u32,
    _stream: cpal::Stream,
}

impl Drop for Audio {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The first audio stream from `start`, played on the default device.
fn spawn_audio(ffmpeg: &Path, path: &str, start: f64, volume: f64, muted: bool, paused: bool) -> Option<Audio> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let device = cpal::default_host().default_output_device()?;
    let config = device.default_output_config().ok()?;
    let rate = config.sample_rate();
    let channels = config.channels() as usize;
    let shared = Arc::new(AudioShared {
        samples: Mutex::new(std::collections::VecDeque::new()),
        played: AtomicU64::new(0),
        paused: AtomicBool::new(paused),
        volume: AtomicU32::new((volume as f32).to_bits()),
        muted: AtomicBool::new(muted),
        done: AtomicBool::new(false),
    });
    let mut c = command(ffmpeg);
    c.args(["-hide_banner", "-loglevel", "error", "-ss", &format!("{start:.6}"), "-i", path, "-map", "0:a:0", "-vn", "-sn", "-dn"]);
    c.args(["-ac", "2", "-ar", &rate.to_string(), "-f", "f32le", "-"]).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = c.spawn().ok()?;
    let mut out = child.stdout.take()?;
    let reader = shared.clone();
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 8192];
        loop {
            // Half a second ahead at most.
            while reader.samples.lock().map(|s| s.len()).unwrap_or(0) > rate as usize {
                std::thread::sleep(Duration::from_millis(10));
            }
            match out.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let n = n - n % 4;
                    if let Ok(mut s) = reader.samples.lock() {
                        s.extend(buf[..n].chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])));
                    }
                }
            }
        }
        reader.done.store(true, Ordering::Relaxed);
    });
    let cb = shared.clone();
    let stream = device
        .build_output_stream(
            config.config(),
            move |data: &mut [f32], _| {
                if cb.paused.load(Ordering::Relaxed) {
                    data.fill(0.0);
                    return;
                }
                let gain = if cb.muted.load(Ordering::Relaxed) { 0.0 } else { f32::from_bits(cb.volume.load(Ordering::Relaxed)) };
                let mut s = match cb.samples.lock() {
                    Ok(s) => s,
                    Err(_) => return,
                };
                let mut frames = 0u64;
                for frame in data.chunks_mut(channels) {
                    if s.len() < 2 {
                        frame.fill(0.0);
                        continue;
                    }
                    let (l, r) = (s.pop_front().unwrap_or(0.0), s.pop_front().unwrap_or(0.0));
                    for (i, out) in frame.iter_mut().enumerate() {
                        *out = gain * match (channels, i) {
                            (1, _) => (l + r) / 2.0,
                            (_, 0) => l,
                            (_, 1) => r,
                            _ => 0.0,
                        };
                    }
                    frames += 1;
                }
                cb.played.fetch_add(frames, Ordering::Relaxed);
            },
            |e| eprintln!("Video player: audio output error: {e}"),
            None,
        )
        .ok()?;
    stream.play().ok()?;
    Some(Audio { child, shared, rate, _stream: stream })
}

struct Video {
    child: Child,
    rx: mpsc::Receiver<Frame>,
}

impl Drop for Video {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A load in progress (`LoadAsync`).
type Loading = mpsc::Receiver<Result<(MediaInfo, Option<Frame>), String>>;

/// `WindowsMediaPlayer`.
pub struct MediaPlayer {
    ffmpeg: Option<PathBuf>,
    path: String,
    pub info: Option<MediaInfo>,
    loading: Option<Loading>,
    /// `IsPlaybackRequested`.
    playing: bool,
    /// Where the running pipes started, and when (the wall clock).
    base: f64,
    started: Option<Instant>,
    video: Option<Video>,
    audio: Option<Audio>,
    /// The frame shown and the next one waiting for its time.
    current: Option<Frame>,
    next: Option<Frame>,
    /// A paused seek's one-frame decode.
    still: Option<mpsc::Receiver<Frame>>,
    position: f64,
    volume: f64,
    muted: bool,
    texture: Option<sharex_ui::tiled::TiledTexture>,
    texture_frame: f64,
    /// Decodes on the calling thread (`--render`).
    pub sync: bool,
}

impl MediaPlayer {
    pub fn new(ffmpeg: Option<PathBuf>) -> Self {
        Self {
            ffmpeg,
            path: String::new(),
            info: None,
            loading: None,
            playing: false,
            base: 0.0,
            started: None,
            video: None,
            audio: None,
            current: None,
            next: None,
            still: None,
            position: 0.0,
            volume: 1.0,
            muted: false,
            texture: None,
            texture_frame: f64::NAN,
            sync: false,
        }
    }

    pub fn duration(&self) -> f64 {
        self.info.as_ref().map_or(0.0, |i| i.duration)
    }

    /// `IsPlaybackRequested`.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// The current time (`CurrentTime`).
    pub fn position(&self) -> f64 {
        self.position
    }

    pub fn set_volume(&mut self, v: f64) {
        if !v.is_finite() {
            return;
        }
        self.volume = v.clamp(0.0, 1.0);
        if let Some(a) = &self.audio {
            a.shared.volume.store((self.volume as f32).to_bits(), Ordering::Relaxed);
        }
    }

    pub fn set_muted(&mut self, m: bool) {
        self.muted = m;
        if let Some(a) = &self.audio {
            a.shared.muted.store(m, Ordering::Relaxed);
        }
    }

    /// `Unload` (`CloseMedia`).
    pub fn unload(&mut self) {
        self.stop_pipes();
        self.loading = None;
        self.info = None;
        self.playing = false;
        self.current = None;
        self.next = None;
        self.still = None;
        self.position = 0.0;
        self.texture = None;
        self.texture_frame = f64::NAN;
    }

    fn stop_pipes(&mut self) {
        self.video = None;
        self.audio = None;
        self.started = None;
        self.next = None;
    }

    /// `LoadAsync`: probes the file and decodes its first frame off the UI
    /// thread; [`Self::poll`] reports how it went.
    pub fn load(&mut self, path: &str) {
        self.unload();
        self.path = path.to_string();
        let (tx, rx) = mpsc::channel();
        let ffmpeg = self.ffmpeg.clone();
        let path = path.to_string();
        let work = move || {
            let r = (|| {
                let ffmpeg = ffmpeg.ok_or_else(|| sharex_strings::tools::VideoConverterWindow_FFmpeg_unavailable().to_string())?;
                let info = probe(&ffmpeg, &path)?;
                let first = spawn_video(&ffmpeg, &path, &info, 0.0, Some(1)).ok().and_then(|(child, rx)| {
                    let f = rx.recv_timeout(Duration::from_secs(20)).ok();
                    drop(Video { child, rx });
                    f
                });
                Ok((info, first))
            })();
            let _ = tx.send(r);
        };
        if self.sync {
            work();
        } else {
            std::thread::spawn(work);
        }
        self.loading = Some(rx);
    }

    /// Advances the clock and takes in finished work: `Some(Ok(info))`
    /// when a load finished (`CanPlay`), `Some(Err)` when it failed.
    pub fn poll(&mut self, ctx: &egui::Context) -> Option<Result<MediaInfo, String>> {
        let mut loaded = None;
        if let Some(rx) = &self.loading {
            match rx.try_recv() {
                Ok(Ok((info, first))) => {
                    self.loading = None;
                    self.current = first;
                    self.info = Some(info.clone());
                    loaded = Some(Ok(info));
                }
                Ok(Err(e)) => {
                    self.loading = None;
                    loaded = Some(Err(e));
                }
                Err(mpsc::TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(30)),
                Err(mpsc::TryRecvError::Disconnected) => self.loading = None,
            }
        }
        if let Some(rx) = &self.still {
            match rx.try_recv() {
                Ok(f) => {
                    self.current = Some(f);
                    self.still = None;
                }
                Err(mpsc::TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(15)),
                Err(mpsc::TryRecvError::Disconnected) => self.still = None,
            }
        }
        if self.playing {
            self.advance();
            ctx.request_repaint_after(Duration::from_millis(8));
        }
        loaded
    }

    /// The playback clock: the audio's playhead, else the wall clock.
    fn clock(&self) -> f64 {
        if let Some(a) = &self.audio {
            let played = a.shared.played.load(Ordering::Relaxed);
            if played > 0 || !a.shared.done.load(Ordering::Relaxed) {
                return self.base + played as f64 / a.rate as f64;
            }
        }
        self.base + self.started.map_or(0.0, |s| s.elapsed().as_secs_f64())
    }

    fn advance(&mut self) {
        let duration = self.duration();
        let now = self.clock().min(duration);
        let mut ended = false;
        if let Some(v) = &self.video {
            loop {
                if self.next.is_none() {
                    match v.rx.try_recv() {
                        Ok(f) => self.next = Some(f),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            ended = true;
                            break;
                        }
                    }
                }
                if self.next.as_ref().is_some_and(|f| f.time <= now) {
                    self.current = self.next.take();
                } else {
                    break;
                }
            }
        }
        self.position = now;
        if ended && now >= self.current.as_ref().map_or(0.0, |f| f.time) {
            // `Ended`: paused at the end.
            self.position = duration;
            self.pause();
        } else if now >= duration {
            self.position = duration;
            self.pause();
        }
    }

    /// `Play`: from the position (from 0 when at the end).
    pub fn play(&mut self) {
        let Some(info) = self.info.clone() else { return };
        let Some(ffmpeg) = self.ffmpeg.clone() else { return };
        if self.playing {
            return;
        }
        if self.position >= info.duration - 0.000_002 {
            self.position = 0.0;
        }
        self.still = None;
        self.base = self.position;
        match spawn_video(&ffmpeg, &self.path, &info, self.base, None) {
            Ok((child, rx)) => self.video = Some(Video { child, rx }),
            Err(e) => eprintln!("Video player: {e}"),
        }
        self.audio = if info.has_audio { spawn_audio(&ffmpeg, &self.path, self.base, self.volume, self.muted, false) } else { None };
        self.started = Some(Instant::now());
        self.playing = true;
    }

    /// `Pause`: the frame shown stays.
    pub fn pause(&mut self) {
        if self.playing {
            self.position = self.clock().min(self.duration());
        }
        self.playing = false;
        self.stop_pipes();
    }

    /// `Seek(seconds)`: paused at the frame there (exactly at the end is the
    /// last frame).
    pub fn seek(&mut self, seconds: f64) {
        let duration = self.duration();
        if duration <= 0.0 || !seconds.is_finite() {
            return;
        }
        self.pause();
        let target = seconds.clamp(0.0, (duration - 0.000_001).max(0.0));
        self.position = target;
        self.decode_still(target);
    }

    /// `StepFrame(forward)`.
    pub fn step_frame(&mut self, forward: bool) {
        let Some(info) = self.info.clone() else { return };
        self.pause();
        let shown = self.current.as_ref().map_or(self.position, |f| f.time);
        let target = if forward { shown + 1.0 / info.fps } else { shown - 1.0 / info.fps };
        let target = target.clamp(0.0, (info.duration - 0.000_001).max(0.0));
        self.position = target;
        self.decode_still(target);
    }

    fn decode_still(&mut self, at: f64) {
        let (Some(info), Some(ffmpeg)) = (self.info.clone(), self.ffmpeg.clone()) else { return };
        // A seek near the end may decode nothing: back off a frame or two.
        let at = at.min((info.duration - 1.5 / info.fps).max(0.0));
        let Ok((child, rx)) = spawn_video(&ffmpeg, &self.path, &info, at, Some(1)) else { return };
        let video = Video { child, rx };
        if self.sync {
            if let Ok(f) = video.rx.recv_timeout(Duration::from_secs(20)) {
                self.current = Some(f);
            }
            return;
        }
        // The one frame handed over without blocking the UI; a newer seek
        // drops this receiver, and its frame with it.
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            if let Ok(f) = video.rx.recv_timeout(Duration::from_secs(20)) {
                let _ = tx.send(f);
            }
            drop(video);
        });
        self.still = Some(rx);
    }

    /// The frame shown, as a texture.
    /// Tiled past the GPU's largest texture side (a 4K video on a weak GPU).
    pub fn texture(&mut self, ctx: &egui::Context) -> Option<(&sharex_ui::tiled::TiledTexture, [usize; 2])> {
        let frame = self.current.as_ref()?;
        if self.texture.is_none() || self.texture_frame.to_bits() != frame.time.to_bits() {
            let image = (*frame.image).clone();
            match &mut self.texture {
                Some(t) => t.set(ctx, "media-player-frame", image, egui::TextureOptions::LINEAR),
                None => self.texture = Some(sharex_ui::tiled::TiledTexture::new(ctx, "media-player-frame", image, egui::TextureOptions::LINEAR)),
            }
            self.texture_frame = frame.time;
        }
        Some((self.texture.as_ref()?, frame.image.size))
    }

    /// The display aspect (`GetVideoAspectRatio`), 0 without a video.
    pub fn aspect(&self) -> f64 {
        self.info.as_ref().map_or(0.0, |i| i.aspect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reads_ffmpegs_summary() {
        let text = "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'v.mp4':\n  Duration: 00:00:08.02, start: 0.000000, bitrate: 327 kb/s\n  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 320x180 [SAR 1:1 DAR 16:9], 190 kb/s, 30 fps, 30 tbr, 15360 tbn (default)\n  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 44100 Hz, mono, fltp, 69 kb/s (default)\n";
        let i = parse_probe(text).unwrap();
        assert_eq!((i.width, i.height, i.fps, i.has_audio), (320, 180, 30.0, true));
        assert!((i.duration - 8.02).abs() < 1e-9);
        assert!((i.aspect - 16.0 / 9.0).abs() < 1e-9);
        assert_eq!(frame_size(&i), (320, 180));
    }

    #[test]
    fn unreadable_input_is_media_foundations_end_of_stream() {
        // VID-04: `ffmpeg -i invalid.mp4` on the bytes `not a video`.
        let text = "[mov,mp4,m4a,3gp,3g2,mj2 @ 0000020c1f4f8c40] moov atom not found
[in#0 @ 0000020c1f4f8a40] Error opening input: Invalid data found when processing input
Error opening input file invalid.mp4.
Error opening input files: Invalid data found when processing input
";
        let message = parse_probe(text).unwrap_err();
        assert!(message.ends_with(" (0xC00D3E84)"), "{message}");
        // English Windows' message is ShareX's shot's; elsewhere it's that text.
        if cfg!(not(windows)) || message.starts_with("The media stream") {
            assert_eq!(message, format!("{END_OF_STREAM_MESSAGE} (0xC00D3E84)"));
        }
    }

    #[test]
    fn probe_without_video_is_invalid() {
        let text = "  Duration: 00:00:03.00, start: 0.000000, bitrate: 128 kb/s\n  Stream #0:0: Audio: mp3, 44100 Hz, stereo, fltp, 128 kb/s\n";
        assert_eq!(parse_probe(text).unwrap_err(), sharex_strings::tools::VideoTrimmer_InvalidVideo());
        assert!(parse_probe("garbage").is_err());
    }

    #[test]
    fn anamorphic_frames_use_the_display_aspect() {
        let text = "  Duration: 00:00:01.00, start: 0.0, bitrate: 1 kb/s\n  Stream #0:0: Video: mpeg2video (Main), yuv420p(tv), 720x576 [SAR 64:45 DAR 16:9], 25 fps, 25 tbr, 90k tbn\n";
        let i = parse_probe(text).unwrap();
        assert_eq!(frame_size(&i), (1024, 576));
        assert_eq!(i.fps, 25.0);
    }
}
