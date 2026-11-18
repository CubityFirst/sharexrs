//! The whole native recorder on the real clock (high-resolution timer),
//! the GPU and Media Foundation, from synthetic frames and sound: no screen,
//! no audio device. Skipped where there is no D3D11 video device (CI).
#![cfg(windows)]

use std::sync::Arc;

use sharex_native_recorder::audio::SyntheticAudioDevices;
use sharex_native_recorder::clock::SystemTime;
use sharex_native_recorder::options::TICKS_PER_SECOND;
use sharex_native_recorder::source::SyntheticSources;
use sharex_native_recorder::windows::GpuMedia;
use sharex_native_recorder::{Backends, Rect, RecordingOptions, ScreenRecorder};

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sharexrs-native-gpu-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn gpu_available(o: &RecordingOptions) -> bool {
    match GpuMedia::new(o, 64, 48, 0) {
        Ok(_) => true,
        Err(e) => {
            eprintln!("skipped: no D3D11 video device or encoder ({e})");
            false
        }
    }
}

fn record(o: RecordingOptions) -> Option<sharex_native_recorder::RecordingResult> {
    let mut probe = o.clone();
    probe.output_path = o.output_path.with_extension(format!("probe.{}", if o.is_gif() { "gif" } else { "mp4" }));
    if !gpu_available(&probe) {
        return None;
    }
    let time = Arc::new(SystemTime);
    let mut b = Backends::system(Arc::new(SyntheticSources { capture_fps: 60 }));
    b.audio = Some(Arc::new(SyntheticAudioDevices { time: time.clone(), drift_ppm: 0 }));
    let r = ScreenRecorder::new(o, b).unwrap();
    r.start().unwrap();
    Some(r.completion().unwrap())
}

#[test]
fn mp4_with_sound_in_real_time() {
    let path = temp("a.mp4");
    let mut o = RecordingOptions::new(&path);
    o.region = Rect::new(0, 0, 320, 240);
    o.require_hardware_encoder = false;
    o.duration = TICKS_PER_SECOND;
    let Some(result) = record(o) else { return };
    assert!(result.video_frames >= 25, "{result:?}");
    assert_eq!(result.duration, TICKS_PER_SECOND);
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[4..8], b"ftyp");
}

#[test]
fn gif_in_real_time() {
    let path = temp("a.gif");
    let mut o = RecordingOptions::new(&path);
    o.region = Rect::new(0, 0, 97, 61);
    o.frames_per_second = 15;
    o.duration = TICKS_PER_SECOND / 2;
    let Some(result) = record(o) else { return };
    assert!(result.video_frames >= 5, "{result:?}");
    assert_eq!(&std::fs::read(&path).unwrap()[..6], b"GIF89a");
}
