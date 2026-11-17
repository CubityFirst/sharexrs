//! `ShareX.HelpersLib/Media/FFmpegCLIManager.cs` on
//! `ShareX.HelpersLib/CLI/ExternalCLIManager.cs`: runs FFmpeg with one
//! argument string, no window, stdin/stdout/stderr redirected (UTF-8),
//! collects its output, notices when it starts encoding ("Press [q] to
//! stop") and, when asked, follows the encode's progress. Stopping writes
//! `q` to its stdin, and kills it on the third try.
//!
//! [`FFmpegCliManager`] is a cheap handle: clone it to stop a run from
//! another thread while [`FFmpegCliManager::run`] blocks.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::Duration;

use crate::dotnet::{NEW_LINE, parse_timespan};

/// A C# event: handlers run in the order they were added.
type Callback<T> = Vec<Arc<dyn Fn(T) + Send + Sync>>;

/// Runs instead of FFmpeg ([`FFmpegCliManager::set_run_override`]): gets the
/// argument string, returns the exit code and the lines FFmpeg would write.
pub type RunOverride = Arc<dyn Fn(&str) -> (i32, Vec<String>) + Send + Sync>;

/// What the output parsing found so far (`FFmpegCLIManager`'s properties).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EncodeState {
    /// Every non-empty line FFmpeg wrote, each followed by a new line.
    pub output: String,
    /// FFmpeg said "Press [q] to stop" (reset when the run ends).
    pub is_encoding: bool,
    /// The input's duration from "Duration:" (ticks); never reset, as in ShareX.
    pub video_duration: i64,
    /// The last "time=" (ticks).
    pub encode_time: i64,
    pub encode_percentage: f32,
}

#[derive(Default)]
struct Handlers {
    encode_started: Callback<()>,
    encode_progress: Callback<f32>,
    /// `ShowError`'s output box: ShareX shows the output when a run fails.
    error: Callback<String>,
    /// Every line FFmpeg writes (ShareXrs: for logs and tests).
    line: Callback<String>,
}

struct Running {
    child: Child,
    stdin: Option<ChildStdin>,
}

struct Inner {
    ffmpeg_path: PathBuf,
    /// `lock (this)` in `FFmpeg_DataReceived`.
    state: Mutex<EncodeState>,
    handlers: Mutex<Handlers>,
    process: Mutex<Option<Running>>,
    is_process_running: AtomicBool,
    show_error: AtomicBool,
    stop_requested: AtomicBool,
    track_encode_progress: AtomicBool,
    close_try_count: AtomicU32,
    run_override: Mutex<Option<RunOverride>>,
}

/// `FFmpegCLIManager`.
#[derive(Clone)]
pub struct FFmpegCliManager {
    inner: Arc<Inner>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl FFmpegCliManager {
    pub fn new(ffmpeg_path: impl Into<PathBuf>) -> Self {
        Self {
            inner: Arc::new(Inner {
                ffmpeg_path: ffmpeg_path.into(),
                state: Mutex::default(),
                handlers: Mutex::default(),
                process: Mutex::default(),
                is_process_running: AtomicBool::new(false),
                show_error: AtomicBool::new(false),
                stop_requested: AtomicBool::new(false),
                track_encode_progress: AtomicBool::new(false),
                close_try_count: AtomicU32::new(0),
                run_override: Mutex::default(),
            }),
        }
    }

    pub fn ffmpeg_path(&self) -> &Path {
        &self.inner.ffmpeg_path
    }

    /// `ShowError`: report a failed run's output through
    /// [`Self::on_error`] (ShareX's output box).
    pub fn set_show_error(&self, value: bool) {
        self.inner.show_error.store(value, Ordering::SeqCst);
    }

    pub fn set_track_encode_progress(&self, value: bool) {
        self.inner.track_encode_progress.store(value, Ordering::SeqCst);
    }

    pub fn track_encode_progress(&self) -> bool {
        self.inner.track_encode_progress.load(Ordering::SeqCst)
    }

    pub fn stop_requested(&self) -> bool {
        self.inner.stop_requested.load(Ordering::SeqCst)
    }

    pub fn is_process_running(&self) -> bool {
        self.inner.is_process_running.load(Ordering::SeqCst)
    }

    /// `EncodeStarted += f`: FFmpeg printed "Press [q] to stop".
    pub fn on_encode_started(&self, f: impl Fn() + Send + Sync + 'static) {
        lock(&self.inner.handlers).encode_started.push(Arc::new(move |()| f()));
    }

    /// `EncodeProgressChanged += f`: percent of the input's duration encoded.
    pub fn on_encode_progress(&self, f: impl Fn(f32) + Send + Sync + 'static) {
        lock(&self.inner.handlers).encode_progress.push(Arc::new(f));
    }

    /// The output of a failed run, with [`Self::set_show_error`].
    pub fn on_error(&self, f: impl Fn(String) + Send + Sync + 'static) {
        lock(&self.inner.handlers).error.push(Arc::new(f));
    }

    /// Every line FFmpeg writes.
    pub fn on_line(&self, f: impl Fn(String) + Send + Sync + 'static) {
        lock(&self.inner.handlers).line.push(Arc::new(f));
    }

    /// Instead of starting FFmpeg, `f` gets the arguments and says what
    /// FFmpeg would have done (tests, the harness, previews).
    pub fn set_run_override(&self, f: impl Fn(&str) -> (i32, Vec<String>) + Send + Sync + 'static) {
        *lock(&self.inner.run_override) = Some(Arc::new(f));
    }

    /// A line as if FFmpeg had written it (`FFmpeg_DataReceived`).
    pub fn receive_line(&self, line: &str) {
        self.data_received(line.to_string());
    }

    pub fn state(&self) -> EncodeState {
        lock(&self.inner.state).clone()
    }

    pub fn output(&self) -> String {
        lock(&self.inner.state).output.clone()
    }

    /// `Run(args)`: runs FFmpeg to the end; whether it exited with 0.
    /// ShareX only runs an `.exe`; ShareXrs requires that on Windows only.
    pub fn run(&self, args: &str) -> bool {
        let path = &self.inner.ffmpeg_path;
        if cfg!(windows) && !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
            return false;
        }
        self.inner.stop_requested.store(false, Ordering::SeqCst);
        let code = self.open(args);
        lock(&self.inner.state).is_encoding = false;
        let result = code == 0;
        if !result && self.inner.show_error.load(Ordering::SeqCst) {
            let handlers = lock(&self.inner.handlers).error.clone();
            for h in handlers {
                h(self.output());
            }
        }
        result
    }

    /// `ExternalCLIManager.Open`: the exit code, or -1 when the program
    /// doesn't exist or can't be started.
    fn open(&self, args: &str) -> i32 {
        let run_override = lock(&self.inner.run_override).clone();
        if let Some(f) = run_override {
            let (code, lines) = f(args);
            for line in lines {
                self.data_received(line);
            }
            return code;
        }
        let path = &self.inner.ffmpeg_path;
        if !path.is_file() {
            return -1;
        }
        let mut cmd = command(path, args);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        log(&format!("CLI: \"{}\" {args}", path.display()));
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                log(&format!("CLI start failed: {e}"));
                return -1;
            }
        };
        let readers: Vec<_> = [child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>)]
            .into_iter()
            .flatten()
            .map(|stream| {
                let this = self.clone();
                thread::spawn(move || read_lines(stream, |line| this.data_received(line)))
            })
            .collect();
        let stdin = child.stdin.take();
        *lock(&self.inner.process) = Some(Running { child, stdin });
        self.inner.is_process_running.store(true, Ordering::SeqCst);

        // `WaitForExit()`: the exit, then the end of both streams.
        let code = loop {
            let mut guard = lock(&self.inner.process);
            let running = guard.as_mut().expect("running");
            match running.child.try_wait() {
                Ok(Some(status)) => break status.code().unwrap_or(-1),
                Ok(None) => {}
                Err(_) => break -1,
            }
            drop(guard);
            thread::sleep(Duration::from_millis(5));
        };
        for r in readers {
            let _ = r.join();
        }
        self.inner.is_process_running.store(false, Ordering::SeqCst);
        *lock(&self.inner.process) = None;
        code
    }

    /// `WriteInput`: a line to FFmpeg's stdin while it runs.
    pub fn write_input(&self, input: &str) {
        if !self.is_process_running() {
            return;
        }
        if let Some(running) = lock(&self.inner.process).as_mut()
            && let Some(stdin) = running.stdin.as_mut()
        {
            let _ = stdin.write_all(format!("{input}{NEW_LINE}").as_bytes());
            let _ = stdin.flush();
        }
    }

    /// `Close`: asks FFmpeg to stop (`q`), which finishes the file; the third
    /// call kills it.
    pub fn close(&self) {
        self.inner.stop_requested.store(true, Ordering::SeqCst);
        if !self.is_process_running() {
            return;
        }
        if self.inner.close_try_count.load(Ordering::SeqCst) >= 2 {
            if let Some(running) = lock(&self.inner.process).as_mut() {
                let _ = running.child.kill();
            }
        } else {
            self.write_input("q");
            self.inner.close_try_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// `FFmpeg_DataReceived`, for each line of stdout or stderr.
    fn data_received(&self, data: String) {
        if data.is_empty() {
            return;
        }
        let handlers = {
            let h = lock(&self.inner.handlers);
            (h.encode_started.clone(), h.encode_progress.clone(), h.line.clone())
        };
        let mut state = lock(&self.inner.state);
        state.output.push_str(&data);
        state.output.push_str(NEW_LINE);
        for line in &handlers.2 {
            line(data.clone());
        }
        if !state.is_encoding && contains_ignore_ascii_case(&data, "press [q] to stop") {
            state.is_encoding = true;
            for started in &handlers.0 {
                started(());
            }
        }
        if self.track_encode_progress()
            && let Some(progress) = update_encode_progress(&mut state, &data)
        {
            for cb in &handlers.1 {
                cb(progress);
            }
        }
    }

    /// `GetDirectShowDevices`: FFmpeg's DirectShow devices (Windows).
    pub fn get_direct_show_devices(&self) -> crate::devices::DirectShowDevices {
        self.run("-list_devices true -f dshow -i dummy");
        crate::devices::parse_direct_show_devices(&self.output())
    }

    /// `GetVideoInfo`: runs `-i "<path>"` and reads the input's details.
    pub fn get_video_info(&self, video_path: &str) -> Option<crate::devices::VideoInfo> {
        self.run(&format!("-i \"{video_path}\""));
        crate::devices::parse_video_info(video_path, &self.output())
    }

    /// `ConcatenateVideos`: joins `input_files` without re-encoding into
    /// `output_file` through a list file (`<output>.txt`, removed after),
    /// deleting the inputs when it worked and `auto_delete_input_files`.
    pub fn concatenate_videos(&self, input_files: &[&str], output_file: &str, auto_delete_input_files: bool) -> bool {
        let list_file = format!("{output_file}.txt");
        if std::fs::write(&list_file, crate::args::concat_list(input_files)).is_err() {
            return false;
        }
        let result = self.run(&crate::args::concat_args(&list_file, output_file));
        if result && auto_delete_input_files {
            for f in input_files {
                let _ = std::fs::remove_file(f);
            }
        }
        let _ = std::fs::remove_file(&list_file);
        result
    }
}

/// `UpdateEncodeProgress`: the duration from the input's header, then each
/// "time=" against it; the new percentage, if this line had one.
pub fn update_encode_progress(state: &mut EncodeState, data: &str) -> Option<f32> {
    if state.video_duration == 0 {
        // Duration: 00:00:15.32, start: 0.000000, bitrate: 1095 kb/s
        if let Some(d) = match_duration(data).and_then(parse_timespan) {
            state.video_duration = d;
        }
        None
    } else {
        // frame=  942 fps=187 q=35.0 size=    3072kB time=00:00:38.10 bitrate= 660.5kbits/s speed=7.55x
        let time = match_time(data).and_then(parse_timespan)?;
        state.encode_time = time;
        state.encode_percentage = (state.encode_time as f32 / state.video_duration as f32) * 100.0;
        Some(state.encode_percentage)
    }
}

/// `Duration:\s*(\d+:\d+:\d+\.\d+),\s*start:` (case-insensitive).
fn match_duration(data: &str) -> Option<&str> {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?i)Duration:\s*(\d+:\d+:\d+\.\d+),\s*start:").expect("pattern"));
    re.captures(data).and_then(|c| c.get(1)).map(|m| m.as_str())
}

/// `time=\s*(\d+:\d+:\d+\.\d+)\s*bitrate=` (case-insensitive).
fn match_time(data: &str) -> Option<&str> {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?i)time=\s*(\d+:\d+:\d+\.\d+)\s*bitrate=").expect("pattern"));
    re.captures(data).and_then(|c| c.get(1)).map(|m| m.as_str())
}

/// `text.Contains(value, StringComparison.OrdinalIgnoreCase)` for an ASCII
/// `value`.
pub(crate) fn contains_ignore_ascii_case(text: &str, value: &str) -> bool {
    text.as_bytes().windows(value.len()).any(|w| w.eq_ignore_ascii_case(value.as_bytes()))
}

/// .NET's `BeginOutputReadLine`: lines end at `\n`, `\r` or `\r\n` (so
/// FFmpeg's `\r`-terminated progress lines arrive one by one), decoded as
/// UTF-8.
fn read_lines(stream: Box<dyn Read + Send>, mut on_line: impl FnMut(String)) {
    let mut reader = BufReader::new(stream);
    let mut line: Vec<u8> = Vec::new();
    let mut pending_cr = false;
    loop {
        let buf = match reader.fill_buf() {
            Ok([]) | Err(_) => break,
            Ok(b) => b,
        };
        let n = buf.len();
        for &b in buf {
            if pending_cr {
                pending_cr = false;
                if b == b'\n' {
                    continue;
                }
            }
            match b {
                b'\n' | b'\r' => {
                    on_line(String::from_utf8_lossy(&line).into_owned());
                    line.clear();
                    pending_cr = b == b'\r';
                }
                _ => line.push(b),
            }
        }
        reader.consume(n);
    }
    if !line.is_empty() {
        on_line(String::from_utf8_lossy(&line).into_owned());
    }
}

/// `ProcessStartInfo` as ShareX fills it: the program's folder as the
/// working directory, no window; the argument string goes to
/// `CreateProcess` as it is on Windows, and is split as .NET splits it
/// elsewhere.
pub fn command(program: &Path, args: &str) -> Command {
    let mut cmd = Command::new(program);
    if let Some(dir) = program.parent().filter(|d| !d.as_os_str().is_empty()) {
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
        if !args.is_empty() {
            cmd.raw_arg(args);
        }
    }
    #[cfg(not(windows))]
    cmd.args(crate::dotnet::split_arguments(args));
    cmd
}

pub(crate) fn log(message: &str) {
    sharex_settings::settings_base::log::write(message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_progress_like_sharex() {
        let mut s = EncodeState::default();
        assert_eq!(update_encode_progress(&mut s, "  Duration: 00:00:10.00, start: 0.000000, bitrate: 1095 kb/s"), None);
        assert_eq!(s.video_duration, 100_000_000);
        let p = update_encode_progress(&mut s, "frame=  942 fps=187 q=35.0 size=    3072kB time=00:00:02.50 bitrate= 660.5kbits/s speed=7.55x");
        assert_eq!(p, Some(25.0));
        assert_eq!(update_encode_progress(&mut s, "nothing"), None);
    }

    #[test]
    fn splits_lines_like_dotnet() {
        let mut lines = Vec::new();
        read_lines(Box::new(&b"a\r\nb\rc\n\nd"[..]), |l| lines.push(l));
        assert_eq!(lines, ["a", "b", "c", "", "d"]);
    }
}
