//! A [`FrameSource`] that has FFmpeg grab the screen (ShareXrs's stand-in
//! for Windows Graphics Capture until a desktop session ports it): FFmpeg
//! writes raw BGRA frames to a pipe, a reader thread keeps the newest one
//! with the QPC time it arrived, and the recorder takes it on its own frame
//! grid, as it takes WGC's newest frame. FFmpeg draws the pointer
//! (`-draw_mouse`), as WGC does with `IsCursorCaptureEnabled`.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::clock::TimeSource;
use crate::options::{Rect, RecordingOptions};
use crate::source::{Canvas, CaptureSources, FrameSource, region_size};

/// The input arguments for a region: `(region, width, height, fps, cursor)`.
pub type InputArgs = Arc<dyn Fn(Rect, i32, i32, i32, bool) -> Vec<String> + Send + Sync>;

/// gdigrab, as ShareX's FFmpeg recorder grabs the desktop.
pub fn gdigrab_input(region: Rect, width: i32, height: i32, fps: i32, cursor: bool) -> Vec<String> {
    [
        "-f",
        "gdigrab",
        "-framerate",
        &fps.to_string(),
        "-offset_x",
        &region.x.to_string(),
        "-offset_y",
        &region.y.to_string(),
        "-video_size",
        &format!("{width}x{height}"),
        "-draw_mouse",
        if cursor { "1" } else { "0" },
        "-i",
        "desktop",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[derive(Default)]
struct Latest {
    frame: Vec<u8>,
    timestamp: i64,
    serial: u64,
}

/// Opens [`FfmpegSource`]s.
#[derive(Clone)]
pub struct FfmpegSources {
    pub ffmpeg: PathBuf,
    pub input: InputArgs,
    /// The monitor whose adapter records (`GetTargets`' first target).
    pub monitor: Arc<dyn Fn(Rect) -> isize + Send + Sync>,
}

impl FfmpegSources {
    pub fn gdigrab(ffmpeg: PathBuf) -> Self {
        Self { ffmpeg, input: Arc::new(gdigrab_input), monitor: Arc::new(first_monitor) }
    }
}

impl CaptureSources for FfmpegSources {
    fn open(&self, options: &RecordingOptions, time: Arc<dyn TimeSource>) -> Result<Box<dyn FrameSource>, String> {
        if options.window_handle != 0 {
            return Err("Window capture needs Windows Graphics Capture, which isn't ported yet.".into());
        }
        let (width, height) = region_size(options)?;
        if !self.ffmpeg.is_file() {
            return Err(format!("FFmpeg does not exist at the following path:\n{}", self.ffmpeg.display()));
        }
        Ok(Box::new(FfmpegSource {
            sources: self.clone(),
            region: options.region,
            width,
            height,
            fps: options.frames_per_second,
            cursor: options.include_cursor,
            time,
            child: None,
            reader: None,
            latest: Arc::new(Mutex::new(Latest::default())),
            ended: Arc::new(AtomicBool::new(false)),
            stopping: Arc::new(AtomicBool::new(false)),
            seen: 0,
            has_frame: false,
            latest_timestamp: 0,
            error: Arc::new(Mutex::new(String::new())),
        }))
    }
}

/// FFmpeg grabbing one region.
pub struct FfmpegSource {
    sources: FfmpegSources,
    region: Rect,
    width: i32,
    height: i32,
    fps: i32,
    cursor: bool,
    time: Arc<dyn TimeSource>,
    child: Option<Child>,
    reader: Option<JoinHandle<()>>,
    latest: Arc<Mutex<Latest>>,
    ended: Arc<AtomicBool>,
    stopping: Arc<AtomicBool>,
    seen: u64,
    has_frame: bool,
    latest_timestamp: i64,
    error: Arc<Mutex<String>>,
}

impl FfmpegSource {
    fn spawn(&mut self) -> Result<(), String> {
        let mut args: Vec<String> = vec!["-hide_banner".into(), "-loglevel".into(), "error".into(), "-nostdin".into()];
        args.extend((self.sources.input)(self.region, self.width, self.height, self.fps, self.cursor));
        args.extend(["-f", "rawvideo", "-pix_fmt", "bgra", "-"].iter().map(|s| s.to_string()));
        let mut cmd = Command::new(&self.sources.ffmpeg);
        cmd.args(&args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().map_err(|e| format!("Couldn't start FFmpeg: {e}"))?;
        let mut stdout = child.stdout.take().ok_or("no FFmpeg output")?;
        let mut stderr = child.stderr.take().ok_or("no FFmpeg errors")?;
        let error = self.error.clone();
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = stderr.read_to_string(&mut s);
            error.lock().unwrap_or_else(|e| e.into_inner()).push_str(&s);
        });
        let (latest, ended, stopping, time) = (self.latest.clone(), self.ended.clone(), self.stopping.clone(), self.time.clone());
        let bytes = self.width as usize * self.height as usize * 4;
        self.reader = Some(
            std::thread::Builder::new()
                .name("FFmpeg frames".into())
                .spawn(move || {
                    let mut buf = vec![0u8; bytes];
                    loop {
                        if stdout.read_exact(&mut buf).is_err() {
                            if !stopping.load(Ordering::SeqCst) {
                                ended.store(true, Ordering::SeqCst);
                            }
                            return;
                        }
                        let now = time.now();
                        let mut l = latest.lock().unwrap_or_else(|e| e.into_inner());
                        std::mem::swap(&mut l.frame, &mut buf);
                        if buf.len() != bytes {
                            buf = vec![0u8; bytes];
                        }
                        l.timestamp = now;
                        l.serial += 1;
                    }
                })
                .map_err(|e| e.to_string())?,
        );
        self.child = Some(child);
        Ok(())
    }

    fn kill(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if let Some(r) = self.reader.take() {
            let _ = r.join();
        }
        self.stopping.store(false, Ordering::SeqCst);
    }

    /// What FFmpeg printed (errors only).
    pub fn errors(&self) -> String {
        self.error.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl FrameSource for FfmpegSource {
    fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }
    fn monitor(&self) -> isize {
        (self.sources.monitor)(self.region)
    }
    fn start(&mut self) -> Result<(), String> {
        self.spawn()
    }
    fn update(&mut self, canvas: &mut dyn Canvas) -> Result<(), String> {
        let l = self.latest.lock().unwrap_or_else(|e| e.into_inner());
        if l.serial == self.seen {
            return Ok(());
        }
        self.seen = l.serial;
        canvas.write_bgra(0, 0, self.width, self.height, &l.frame, self.width as usize * 4);
        self.latest_timestamp = self.latest_timestamp.max(l.timestamp);
        self.has_frame = true;
        Ok(())
    }
    fn has_frame(&self) -> bool {
        self.has_frame
    }
    fn latest_timestamp(&self) -> i64 {
        self.latest_timestamp
    }
    fn has_ended(&self) -> bool {
        self.ended.load(Ordering::SeqCst)
    }
    fn retarget(&mut self, region: Rect, canvas: &mut dyn Canvas) -> Result<(), String> {
        self.kill();
        canvas.clear();
        self.region = region;
        self.latest_timestamp = 0;
        self.has_frame = false;
        self.spawn()
    }
}

impl Drop for FfmpegSource {
    fn drop(&mut self) {
        self.kill();
    }
}

/// `GetTargets`' first monitor: the first (in `EnumDisplayMonitors` order)
/// the region touches; 0 when none or off Windows.
pub fn first_monitor(region: Rect) -> isize {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{LPARAM, RECT};
        use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};
        use windows::core::BOOL;
        unsafe extern "system" fn collect(monitor: HMONITOR, _: HDC, bounds: *mut RECT, state: LPARAM) -> BOOL {
            // SAFETY: `state` is the Vec below, `bounds` the monitor's rectangle.
            unsafe {
                let list = &mut *(state.0 as *mut Vec<(isize, Rect)>);
                let b = &*bounds;
                list.push((monitor.0 as isize, Rect::from_ltrb(b.left, b.top, b.right, b.bottom)));
            }
            true.into()
        }
        let mut monitors: Vec<(isize, Rect)> = Vec::new();
        // SAFETY: the callback only runs during the call.
        unsafe {
            let _ = EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut monitors as *mut _ as isize));
        }
        for (m, bounds) in monitors {
            let i = Rect::intersect(region, bounds);
            if i.width > 0 && i.height > 0 {
                return m;
            }
        }
        0
    }
    #[cfg(not(windows))]
    {
        let _ = region;
        0
    }
}
