//! The recorder against a real FFmpeg, on synthetic input only (`-f lavfi
//! -i testsrc`/`sine`, paced to real time), never the screen: start, stop
//! with `q`, fixed duration, pause/resume (segments joined), restart, abort,
//! the third stop killing FFmpeg, the error output, GIF through palettegen
//! and the two-pass encodes, and the output parsing on real output.
//!
//! Set `SHAREXRS_TEST_FFMPEG` to an FFmpeg (ShareX ships one beside
//! ShareX.exe); without it these tests are skipped.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use sharex_screen_recording::platform::{AudioInput, Input};
use sharex_screen_recording::session::{FFmpegSession, PreparedRecording, RecordingControl, ScreenRecordState, ScreenRecordingStatus};
use sharex_screen_recording::{
    FFmpegCliManager, RecorderInput, ScreenRecordOutput, ScreenRecorder, ScreenRecordingOptions, SessionHooks,
    prepare_ffmpeg_recording, run_ffmpeg_session,
};
use sharex_settings::TaskSettingsCapture;
use sharex_settings::ffmpeg_options::{FFmpegAudioCodec, FFmpegVideoCodec};
use sharex_settings::types::Rectangle;

fn ffmpeg() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("SHAREXRS_TEST_FFMPEG")?);
    p.is_file().then_some(p)
}

macro_rules! need_ffmpeg {
    () => {
        match ffmpeg() {
            Some(p) => p,
            None => {
                eprintln!("skipped: set SHAREXRS_TEST_FFMPEG to an ffmpeg");
                return;
            }
        }
    };
}

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sxr-rec-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const VIDEO: &str = "testsrc=size=320x240:rate=30,realtime";
const SOUND: &str = "sine=frequency=440:sample_rate=48000,arealtime";

fn input(sound: bool) -> RecorderInput {
    RecorderInput::Platform(Input::Lavfi { graph: VIDEO.into() }, sound.then(|| AudioInput::Lavfi(SOUND.into())))
}

fn capture(sound: bool) -> TaskSettingsCapture {
    let mut c = TaskSettingsCapture::default();
    // Any non-empty source; the lavfi input replaces the screen.
    c.FFmpegOptions.VideoSource = "gdigrab".into();
    c.FFmpegOptions.AudioSource = if sound { "sine".into() } else { String::new() };
    c
}

fn prepare(ffmpeg: &Path, output: ScreenRecordOutput, capture: &TaskSettingsCapture) -> PreparedRecording {
    prepare_ffmpeg_recording(output, capture, Rectangle::new(0, 0, 320, 240), Rectangle::new(0, 0, 1920, 1080), Ok(ffmpeg.into()))
        .unwrap()
}

#[derive(Default)]
struct Log {
    states: Vec<ScreenRecordState>,
    progress: Vec<i32>,
    errors: Vec<String>,
    completed: u32,
}

struct Hooks {
    dir: PathBuf,
    log: Arc<Mutex<Log>>,
}

impl SessionHooks for Hooks {
    fn output_path(&mut self, extension: &str) -> Option<String> {
        Some(self.dir.join(format!("rec.{extension}")).to_string_lossy().into_owned())
    }
    fn two_pass_output_path(&mut self, extension: &str) -> String {
        self.dir.join(format!("final.{extension}")).to_string_lossy().into_owned()
    }
    fn action_completed(&mut self) {
        self.log.lock().unwrap().completed += 1;
    }
    fn ffmpeg_error(&mut self, output: &str) {
        self.log.lock().unwrap().errors.push(output.to_string());
    }
}

/// Starts a session on a thread; the control, the log and its result.
fn start(
    ffmpeg: &Path,
    dir: &Path,
    output: ScreenRecordOutput,
    capture: TaskSettingsCapture,
    sound: bool,
) -> (Arc<RecordingControl>, Arc<Mutex<Log>>, thread::JoinHandle<Option<String>>) {
    let prepared = prepare(ffmpeg, output, &capture);
    let control = RecordingControl::new(prepared.capture_rectangle);
    let log = Arc::new(Mutex::new(Log::default()));
    let l = log.clone();
    control.on_state_changed(move |s, _| l.lock().unwrap().states.push(s));
    let l = log.clone();
    control.on_progress(move |p| l.lock().unwrap().progress.push(p));
    let mut hooks = Hooks { dir: dir.into(), log: log.clone() };
    let c = control.clone();
    let session = FFmpegSession { prepared, screens: Vec::new(), input: input(sound) };
    let handle = thread::spawn(move || run_ffmpeg_session(session, c, &mut hooks));
    (control, log, handle)
}

fn wait_for(what: &str, timeout: Duration, mut f: impl FnMut() -> bool) {
    let start = Instant::now();
    while !f() {
        assert!(start.elapsed() < timeout, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_recording(c: &RecordingControl) {
    wait_for("recording", Duration::from_secs(20), || c.status() == ScreenRecordingStatus::Recording);
}

/// What `ffmpeg -i` says about a file, and its duration in seconds.
fn probe(ffmpeg: &Path, file: &str) -> (String, f64) {
    let m = FFmpegCliManager::new(ffmpeg);
    let info = m.get_video_info(file).unwrap_or_else(|| panic!("no info for {file}:\n{}", m.output()));
    (m.output(), info.duration as f64 / 1e7)
}

fn files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn records_and_stops_with_q() {
    let ff = need_ffmpeg!();
    let dir = temp_dir("stop");
    let (c, log, h) = start(&ff, &dir, ScreenRecordOutput::FFmpeg, capture(true), true);
    wait_recording(&c);
    thread::sleep(Duration::from_millis(1500));
    c.start_stop_recording();
    let path = h.join().unwrap().expect("a file");
    let (info, secs) = probe(&ff, &path);
    assert!(info.contains("Video: h264") && info.contains("Audio: aac"), "{info}");
    assert!((1.0..3.5).contains(&secs), "{secs} s");
    let log = log.lock().unwrap();
    assert_eq!(
        log.states,
        [ScreenRecordState::BeforeStart, ScreenRecordState::AfterStart, ScreenRecordState::AfterRecordingStart, ScreenRecordState::RecordingEnd]
    );
    assert_eq!(log.completed, 1);
    assert!(log.errors.is_empty());
    assert_eq!(files(&dir), ["rec.mp4"]);
}

#[test]
fn fixed_duration_ends_by_itself() {
    let ff = need_ffmpeg!();
    let dir = temp_dir("duration");
    let mut cap = capture(false);
    cap.ScreenRecordFixedDuration = true;
    cap.ScreenRecordDuration = 1.0;
    let (_c, _log, h) = start(&ff, &dir, ScreenRecordOutput::FFmpeg, cap, false);
    let path = h.join().unwrap().expect("a file");
    let (_, secs) = probe(&ff, &path);
    assert!((0.9..1.2).contains(&secs), "{secs} s");
}

#[test]
fn pause_and_resume_join_the_segments() {
    let ff = need_ffmpeg!();
    let dir = temp_dir("pause");
    let (c, log, h) = start(&ff, &dir, ScreenRecordOutput::FFmpeg, capture(true), true);
    wait_recording(&c);
    thread::sleep(Duration::from_millis(1200));
    c.pause_resume_recording();
    assert_eq!(c.status(), ScreenRecordingStatus::Paused);
    // FFmpeg stops on pause: the segment is finished and the worker waits.
    wait_for("the first segment", Duration::from_secs(10), || log.lock().unwrap().states.contains(&ScreenRecordState::RecordingEnd));
    thread::sleep(Duration::from_millis(800));
    assert!(dir.join("rec.mp4").exists());
    c.pause_resume_recording();
    wait_recording(&c);
    thread::sleep(Duration::from_millis(1200));
    c.start_stop_recording();
    let path = h.join().unwrap().expect("a file");
    let (info, secs) = probe(&ff, &path);
    // Two ~1.2 s segments; the pause isn't in it.
    assert!((2.0..4.0).contains(&secs), "{secs} s\n{info}");
    assert_eq!(files(&dir), ["rec.mp4"], "the -concat and -temp files are gone");
    assert_eq!(log.lock().unwrap().completed, 2);
}

#[test]
fn abort_deletes_the_recording() {
    let ff = need_ffmpeg!();
    let dir = temp_dir("abort");
    let (c, _log, h) = start(&ff, &dir, ScreenRecordOutput::FFmpeg, capture(false), false);
    wait_recording(&c);
    thread::sleep(Duration::from_millis(800));
    c.abort_recording();
    assert_eq!(h.join().unwrap(), None);
    assert!(files(&dir).is_empty(), "{:?}", files(&dir));
}

#[test]
fn restart_keeps_only_the_new_take() {
    let ff = need_ffmpeg!();
    let dir = temp_dir("restart");
    let (c, log, h) = start(&ff, &dir, ScreenRecordOutput::FFmpeg, capture(false), false);
    wait_recording(&c);
    thread::sleep(Duration::from_millis(2500));
    c.restart_recording();
    assert_eq!(c.status(), ScreenRecordingStatus::Waiting);
    wait_recording(&c);
    thread::sleep(Duration::from_millis(800));
    c.start_stop_recording();
    let path = h.join().unwrap().expect("a file");
    let (_, secs) = probe(&ff, &path);
    assert!(secs < 2.0, "{secs} s: the first take was kept");
    // The restart skipped the first segment's end (no sound, no join).
    assert_eq!(log.lock().unwrap().completed, 1);
}

#[test]
fn gif_records_lossless_then_palettegen() {
    let ff = need_ffmpeg!();
    let dir = temp_dir("gif");
    let (c, log, h) = start(&ff, &dir, ScreenRecordOutput::GIF, capture(false), false);
    wait_recording(&c);
    thread::sleep(Duration::from_millis(1500));
    c.start_stop_recording();
    let path = h.join().unwrap().expect("a file");
    assert!(path.ends_with("final.gif"), "{path}");
    assert_eq!(files(&dir), ["final.gif"], "the lossless .mp4 is deleted");
    let (info, _) = probe(&ff, &path);
    assert!(info.contains("Video: gif"), "{info}");
    let log = log.lock().unwrap();
    assert!(log.states.contains(&ScreenRecordState::Encoding));
    assert!(log.progress.last().is_some_and(|&p| p >= 90), "{:?}", log.progress);
}

#[test]
fn two_pass_webm_with_opus() {
    let ff = need_ffmpeg!();
    let dir = temp_dir("webm");
    let mut cap = capture(true);
    cap.ScreenRecordTwoPassEncoding = true;
    cap.FFmpegOptions.VideoCodec = FFmpegVideoCodec::libvpx_vp9;
    cap.FFmpegOptions.AudioCodec = FFmpegAudioCodec::libopus;
    let (c, _log, h) = start(&ff, &dir, ScreenRecordOutput::FFmpeg, cap, true);
    wait_recording(&c);
    thread::sleep(Duration::from_millis(1200));
    c.start_stop_recording();
    let path = h.join().unwrap().expect("a file");
    assert!(path.ends_with("final.webm"), "{path}");
    let (info, _) = probe(&ff, &path);
    assert!(info.contains("Video: vp9") && info.contains("Audio: opus"), "{info}");
    assert_eq!(files(&dir), ["final.webm"]);
}

#[test]
fn failed_run_reports_its_output() {
    let ff = need_ffmpeg!();
    let dir = temp_dir("error");
    let mut cap = capture(false);
    cap.FFmpegOptions.UserArgs = "-no-such-option".into();
    let (_c, log, h) = start(&ff, &dir, ScreenRecordOutput::FFmpeg, cap, false);
    assert_eq!(h.join().unwrap(), None);
    let log = log.lock().unwrap();
    assert_eq!(log.errors.len(), 1);
    assert!(log.errors[0].contains("no-such-option"), "{}", log.errors[0]);
}

#[test]
fn third_stop_kills_ffmpeg() {
    let ff = need_ffmpeg!();
    let dir = temp_dir("kill");
    let mut options = ScreenRecordingOptions {
        is_recording: true,
        fps: 30,
        capture_area: Rectangle::new(0, 0, 320, 240),
        output_path: Some(dir.join("rec.mp4").to_string_lossy().into_owned()),
        ..Default::default()
    };
    // FFmpeg ignores `q` without stdin.
    options.ffmpeg.UserArgs = "-nostdin".into();
    let rec = Arc::new(ScreenRecorder::new(options, Vec::new(), &ff).unwrap());
    rec.set_input(input(false));
    let started = Arc::new(Mutex::new(false));
    let s = started.clone();
    rec.ffmpeg().on_line(move |l| {
        if l.contains("frame=") {
            *s.lock().unwrap() = true;
        }
    });
    let r = rec.clone();
    let h = thread::spawn(move || r.start_recording());
    wait_for("frames", Duration::from_secs(20), || *started.lock().unwrap());
    rec.stop_recording();
    rec.stop_recording();
    thread::sleep(Duration::from_millis(1000));
    assert!(rec.ffmpeg().is_process_running(), "q was ignored, so it still runs");
    let t = Instant::now();
    rec.stop_recording();
    assert!(!h.join().unwrap(), "killed: not a clean exit");
    assert!(t.elapsed() < Duration::from_secs(5));
}

#[test]
fn encode_progress_from_real_output() {
    let ff = need_ffmpeg!();
    let dir = temp_dir("progress");
    let src = dir.join("src.mp4").to_string_lossy().into_owned();
    let m = FFmpegCliManager::new(&ff);
    assert!(m.run(&format!("-f lavfi -i testsrc=size=160x120:rate=30:duration=4 -c:v libx264 -y \"{src}\"")), "{}", m.output());
    assert!(!m.state().is_encoding, "reset after the run");
    let options = ScreenRecordingOptions { output_path: Some(src.clone()), ..Default::default() };
    let rec = ScreenRecorder::new(options, Vec::new(), &ff).unwrap();
    let progress = Arc::new(Mutex::new(Vec::new()));
    let p = progress.clone();
    rec.on_encoding_progress(move |v| p.lock().unwrap().push(v));
    let out = dir.join("out.gif").to_string_lossy().into_owned();
    assert!(rec.ffmpeg_encode_as_gif(&src, &out));
    assert_eq!(rec.ffmpeg().state().video_duration, 40_000_000);
    let progress = progress.lock().unwrap();
    assert!(progress.windows(2).all(|w| w[0] != w[1]), "only changes are reported: {progress:?}");
    assert!(progress.last().is_some_and(|&v| v >= 95), "{progress:?}");
}

#[cfg(windows)]
#[test]
fn lists_directshow_devices() {
    let ff = need_ffmpeg!();
    let m = FFmpegCliManager::new(&ff);
    let devices = m.get_direct_show_devices();
    // Whatever this machine has; the listing ran and was parsed.
    assert!(m.output().contains("dummy") || !devices.video_devices.is_empty() || !devices.audio_devices.is_empty(), "{}", m.output());
}
