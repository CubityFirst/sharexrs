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

/// A real camera's Media Foundation samples (a plain memory buffer at the
/// default stride, and a 2D buffer with its own pitch, bottom-up RGB32 too)
/// give the same tile as the same pixels as a memory frame. Synthetic
/// pixels only: no camera is opened.
#[test]
fn camera_samples_match_memory_frames() {
    use sharex_native_recorder::camera::{CameraFrame, CameraPixelFormat, CameraSource, SyntheticCamera};
    use sharex_native_recorder::media::Media;
    use sharex_native_recorder::windows::camera_capture::CameraSample;
    use windows::Win32::Media::MediaFoundation::*;

    let path = temp("cam.gif");
    let mut o = RecordingOptions::new(&path);
    o.region = Rect::new(0, 0, 320, 240);
    o.capture_camera = true;
    o.camera_width_percent = 50;
    if !gpu_available(&o) {
        return;
    }
    let time = Arc::new(SystemTime);
    // The camera overlay needs the D3D11 video processor, which a software
    // device (CI's runners) can create media on without: skip there too.
    {
        let probe = SyntheticCamera::new(time.clone(), 160, 120, 30.0, CameraPixelFormat::Nv12);
        let overlay = GpuMedia::new(&o, 320, 240, 0).and_then(|mut m| m.create_camera_overlay(probe.mode(), &o));
        if let Err(e) = overlay {
            eprintln!("skipped: no D3D11 video processor for the camera overlay ({e})");
            return;
        }
    }
    for (format, bottom_up) in [(CameraPixelFormat::Nv12, false), (CameraPixelFormat::Yuy2, false), (CameraPixelFormat::Rgb32, false), (CameraPixelFormat::Rgb32, true)] {
        let (w, h) = (160, 120);
        let mut cam = SyntheticCamera::new(time.clone(), w, h, 30.0, format);
        cam.bottom_up = bottom_up;
        let mode = cam.mode().clone();
        let pixels = SyntheticCamera::pattern(format, w, h, 7);
        let row = format.row_bytes(w) as usize;
        // As a memory frame at the mode's stride (bottom-up rows when negative).
        let (data, stride) = if bottom_up { (pixels.chunks_exact(row).rev().flatten().copied().collect::<Vec<u8>>(), -(row as i32)) } else { (pixels.clone(), row as i32) };
        let mut mode = mode;
        mode.stride = stride;
        let tile = |frame: CameraFrame| {
            let mut m = GpuMedia::new(&o, 320, 240, 0).unwrap();
            m.create_camera_overlay(&mode, &o).unwrap();
            m.camera_update(&frame).unwrap();
            m.read_bgra_frame().unwrap()
        };
        let expected = tile(CameraFrame::memory(data.clone(), stride));
        // SAFETY: Media Foundation buffers written within their lengths and pitches.
        let (plain, two_d) = unsafe {
            let plain = MFCreateMemoryBuffer(data.len() as u32).unwrap();
            let mut p = std::ptr::null_mut();
            plain.Lock(&mut p, None, None).unwrap();
            std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
            plain.Unlock().unwrap();
            plain.SetCurrentLength(data.len() as u32).unwrap();
            let fourcc = match format {
                CameraPixelFormat::Nv12 => MFVideoFormat_NV12.data1,
                CameraPixelFormat::Yuy2 => MFVideoFormat_YUY2.data1,
                CameraPixelFormat::Rgb32 => MFVideoFormat_RGB32.data1,
            };
            let two_d = MFCreate2DMediaBuffer(w as u32, h as u32, fourcc, bottom_up).unwrap();
            let buffer2d: IMF2DBuffer = windows::core::Interface::cast(&two_d).unwrap();
            let (mut scan, mut pitch) = (std::ptr::null_mut(), 0i32);
            buffer2d.Lock2D(&mut scan, &mut pitch).unwrap();
            let rows = if format == CameraPixelFormat::Nv12 { h as usize * 3 / 2 } else { h as usize };
            for r in 0..rows {
                std::ptr::copy_nonoverlapping(pixels[r * row..].as_ptr(), scan.offset(r as isize * pitch as isize), row);
            }
            buffer2d.Unlock2D().unwrap();
            let sample = |b: &IMFMediaBuffer| {
                let s = MFCreateSample().unwrap();
                s.AddBuffer(b).unwrap();
                s
            };
            (sample(&plain), sample(&two_d))
        };
        let frame = |s: IMFSample| CameraFrame { data: Vec::new(), stride, sample: Some(CameraSample(s)) };
        assert!(tile(frame(plain)) == expected, "{format:?} memory buffer");
        assert!(tile(frame(two_d)) == expected, "{format:?} 2D buffer (bottom-up {bottom_up})");
    }
}
