//! The FFmpeg frame source feeding the native recorder, with FFmpeg's own
//! `testsrc` paced to real time instead of the screen. Needs
//! `SHAREXRS_TEST_FFMPEG=<ffmpeg.exe>` (skipped otherwise) and, for the
//! encoders, Windows with a D3D11 video device.
#![cfg(windows)]

use std::sync::Arc;

use sharex_native_recorder::ffmpeg_source::FfmpegSources;
use sharex_native_recorder::options::TICKS_PER_SECOND;
use sharex_native_recorder::windows::GpuMedia;
use sharex_native_recorder::{Backends, Rect, RecordingOptions, ScreenRecorder};

#[test]
fn records_ffmpeg_frames() {
    let Ok(ffmpeg) = std::env::var("SHAREXRS_TEST_FFMPEG") else {
        eprintln!("skipped: SHAREXRS_TEST_FFMPEG isn't set");
        return;
    };
    let dir = std::env::temp_dir().join(format!("sharexrs-native-ffmpeg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut o = RecordingOptions::new(dir.join("a.mp4"));
    o.region = Rect::new(100, 50, 320, 240);
    o.capture_system_audio = false;
    o.require_hardware_encoder = false;
    o.duration = TICKS_PER_SECOND;
    let mut probe = o.clone();
    probe.output_path = dir.join("probe.mp4");
    if let Err(e) = GpuMedia::new(&probe, 64, 48, 0) {
        eprintln!("skipped: {e}");
        return;
    }
    let mut sources = FfmpegSources::gdigrab(ffmpeg.into());
    sources.input = Arc::new(|_region, w, h, fps, _cursor| {
        ["-re", "-f", "lavfi", "-i", &format!("testsrc=size={w}x{h}:rate={fps}")].iter().map(|s| s.to_string()).collect()
    });
    let r = ScreenRecorder::new(o, Backends::system(Arc::new(sources))).unwrap();
    r.start().unwrap();
    let result = r.completion().unwrap();
    assert!(result.video_frames >= 25, "{result:?}");
    assert!(dir.join("a.mp4").is_file());
}
