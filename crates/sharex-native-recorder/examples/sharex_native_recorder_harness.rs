//! ShareXrs's side of tools/native-recorder-harness: drives the native
//! recorder's encoders with the cases' synthetic frames, camera frames and
//! sound, in the order ShareX's `ScreenRecorder.Run` does, and writes what
//! they produce (GIF, MP4, the mixer's PCM, the NV12 handed to the encoder,
//! the composited camera frames). `--compare` checks it against the .NET
//! harness's output (README there).
//!
//!   sharex_native_recorder_harness <cases.json> <out dir>
//!   sharex_native_recorder_harness --compare <sharex dir> <sharexrs dir> [report.md]

#[path = "synthetic/mod.rs"]
#[cfg_attr(not(windows), allow(dead_code))]
mod synthetic;

#[cfg(windows)]
#[path = "harness_compare/mod.rs"]
mod compare;

#[path = "harness_hardware/mod.rs"]
#[cfg_attr(not(windows), allow(dead_code))]
mod hardware;

use std::path::Path;
use std::sync::Arc;

use serde_json::Value;
#[cfg(windows)]
use serde_json::json;
use sharex_native_recorder::clock::{RecordingClock, VirtualTime};
use sharex_native_recorder::mixer::{AudioMixer, write_audio_until};
use synthetic::*;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--compare") {
        #[cfg(windows)]
        std::process::exit(compare::run(Path::new(&args[1]), Path::new(&args[2]), args.get(3).map(Path::new)));
        #[cfg(not(windows))]
        {
            eprintln!("--compare decodes MP4 with Media Foundation: Windows only");
            std::process::exit(2);
        }
    }
    let [cases, out] = &args[..] else {
        eprintln!("usage: sharex_native_recorder_harness <cases.json> <out dir> | --compare <sharex dir> <sharexrs dir> [report.md]");
        std::process::exit(2);
    };
    let cases: Value = serde_json::from_str(&std::fs::read_to_string(cases).expect("cases")).expect("json");
    let out = Path::new(out);
    std::fs::create_dir_all(out).expect("out dir");
    let only = std::env::var("CASE").ok();
    for case in cases["mixer"].as_array().unwrap() {
        if only.as_ref().is_some_and(|o| case["name"] != o.as_str()) {
            continue;
        }
        let name = case["name"].as_str().unwrap();
        std::fs::write(out.join(format!("{name}.pcm")), mixer_case(case)).expect("write");
        println!("{name}: ok");
    }
    for case in cases["drain"].as_array().into_iter().flatten() {
        if only.as_ref().is_some_and(|o| case["name"] != o.as_str()) {
            continue;
        }
        let name = case["name"].as_str().unwrap();
        let (pcm, info) = hardware::drain(case);
        std::fs::write(out.join(format!("{name}.pcm")), pcm).expect("write");
        std::fs::write(out.join(format!("{name}.json")), serde_json::to_string_pretty(&info).unwrap()).expect("write");
        println!("{name}: ok");
    }
    #[cfg(windows)]
    for case in cases["hdr"].as_array().into_iter().flatten() {
        if only.as_ref().is_some_and(|o| case["name"] != o.as_str()) {
            continue;
        }
        let name = case["name"].as_str().unwrap();
        match hardware::hdr(case) {
            Ok(bgra) => {
                std::fs::write(out.join(format!("{name}.bgra")), bgra).expect("write");
                println!("{name}: ok");
            }
            Err(e) => println!("{name}: ERROR {e}"),
        }
    }
    if only.as_deref().is_none_or(|o| o == "devices") {
        std::fs::write(out.join("devices.json"), serde_json::to_string_pretty(&hardware::devices()).unwrap()).expect("write");
        println!("devices: ok");
    }
    #[cfg(windows)]
    for kind in ["gif", "mp4"] {
        for case in cases[kind].as_array().unwrap() {
            if only.as_ref().is_some_and(|o| case["name"] != o.as_str()) {
                continue;
            }
            let name = case["name"].as_str().unwrap();
            match encode::run(kind, case, out) {
                Ok(info) => {
                    std::fs::write(out.join(format!("{name}.json")), serde_json::to_string_pretty(&info).unwrap()).expect("write");
                    println!("{name}: {}", info["encoder"]);
                }
                Err(e) => {
                    std::fs::write(out.join(format!("{name}.json")), serde_json::to_string_pretty(&json!({ "error": e })).unwrap()).expect("write");
                    println!("{name}: ERROR {e}");
                }
            }
        }
    }
}

/// A mixer case: the packets written in order, the PCM read on the
/// recorder's schedule (50 ms behind each packet, then the rest).
fn mixer_case(case: &Value) -> Vec<u8> {
    let time = VirtualTime::new(0);
    let clock = Arc::new(RecordingClock::new(time));
    clock.start(0);
    let mixer = AudioMixer::new(clock);
    let packets = mixer_packets(case);
    let mut pcm = Vec::new();
    let mut block = vec![0i16; 960];
    let mut first = 0i64;
    let mut last_end = 0;
    for p in &packets {
        mixer.write(p.source, p.timestamp, p.frames, &p.data, p.gain, p.discontinuity);
        last_end = p.end;
        write_audio_until(&mixer, &mut block, &mut first, (p.end - 50 * 10_000).max(0), false, &mut |s, _| {
            pcm.extend(s.iter().flat_map(|v| v.to_le_bytes()));
            Ok(())
        })
        .unwrap();
    }
    write_audio_until(&mixer, &mut block, &mut first, last_end, true, &mut |s, _| {
        pcm.extend(s.iter().flat_map(|v| v.to_le_bytes()));
        Ok(())
    })
    .unwrap();
    pcm
}

#[cfg(windows)]
mod encode {
    use super::*;
    use sharex_native_recorder::camera::{CameraFrame, CameraMode, CameraPixelFormat};
    use sharex_native_recorder::media::Media;
    use sharex_native_recorder::options::{CameraCaptureInfo, CameraCaptureResolution, CameraOverlayPosition, CameraOverlayShape, Rect, RecordingOptions};
    use sharex_native_recorder::windows::GpuMedia;

    fn camera_mode(c: &Value) -> CameraMode {
        let format = match c["format"].as_str().unwrap() {
            "NV12" => CameraPixelFormat::Nv12,
            "YUY2" => CameraPixelFormat::Yuy2,
            _ => CameraPixelFormat::Rgb32,
        };
        let (w, h) = (c["width"].as_i64().unwrap() as i32, c["height"].as_i64().unwrap() as i32);
        CameraMode {
            info: CameraCaptureInfo { name: "Synthetic camera".into(), width: w, height: h, frames_per_second: c["fps"].as_f64().unwrap(), pixel_format: format.name().into() },
            format,
            // A bottom-up RGB32 camera: MF_MT_DEFAULT_STRIDE is negative.
            stride: if c["bottom_up"].as_bool().unwrap_or(false) { -format.row_bytes(w) } else { format.row_bytes(w) },
            // `ReadSelectedFormat`'s defaults for a type without these attributes.
            yuv_matrix: if h > 576 { 1 } else { 2 },
            nominal_range: 2,
        }
    }

    fn camera_frame(c: &Value, mode: &CameraMode, m: i64) -> CameraFrame {
        let (w, h) = (mode.info.width as i64, mode.info.height as i64);
        let data = synthetic::camera_frame(c["format"].as_str().unwrap(), w, h, m);
        if c["bottom_up"].as_bool().unwrap_or(false) {
            let row = mode.format.row_bytes(mode.info.width) as usize;
            return CameraFrame::memory(data.chunks_exact(row).rev().flatten().copied().collect(), mode.stride);
        }
        CameraFrame::memory(data, mode.stride)
    }

    pub fn options(kind: &str, case: &Value, path: &Path) -> RecordingOptions {
        let mut o = RecordingOptions::new(path);
        let (w, h) = (case["width"].as_i64().unwrap() as i32, case["height"].as_i64().unwrap() as i32);
        o.region = Rect::new(case["x"].as_i64().unwrap_or(0) as i32, case["y"].as_i64().unwrap_or(0) as i32, w, h);
        o.frames_per_second = case["fps"].as_i64().unwrap() as i32;
        if let Some(b) = case["bitrate"].as_i64() {
            o.video_bitrate = b as i32;
        }
        let audio = &case["audio"];
        o.capture_system_audio = kind == "mp4" && audio["system"].as_bool().unwrap_or(false);
        o.capture_microphone = kind == "mp4" && audio["microphone"].as_bool().unwrap_or(false);
        o.system_audio_gain = audio["system_gain"].as_f64().unwrap_or(1.0) as f32;
        o.microphone_gain = audio["microphone_gain"].as_f64().unwrap_or(1.0) as f32;
        if let Some(b) = audio["bitrate"].as_i64() {
            o.audio_bitrate = b as i32;
        }
        let c = &case["camera"];
        if c.is_object() {
            o.capture_camera = true;
            o.camera_shape = if c["shape"] == "Circle" { CameraOverlayShape::Circle } else { CameraOverlayShape::Rectangle };
            o.camera_position = match c["position"].as_str().unwrap() {
                "TopLeft" => CameraOverlayPosition::TopLeft,
                "TopRight" => CameraOverlayPosition::TopRight,
                "BottomLeft" => CameraOverlayPosition::BottomLeft,
                _ => CameraOverlayPosition::BottomRight,
            };
            o.camera_width_percent = c["percent"].as_i64().unwrap() as i32;
            o.camera_margin = c["margin"].as_i64().unwrap() as i32;
            o.camera_frames_per_second = c["fps"].as_i64().unwrap() as i32;
            o.camera_resolution = CameraCaptureResolution::Size1280x720;
        }
        o
    }

    /// `ScreenRecorder.Run`'s encoding, frame by frame, without the capture.
    pub fn run(kind: &str, case: &Value, out: &Path) -> Result<Value, String> {
        let name = case["name"].as_str().unwrap();
        let path = out.join(format!("{name}.{kind}"));
        let _ = std::fs::remove_file(&path);
        let o = options(kind, case, &path);
        o.validate()?;
        let (w, h) = o.region_size();
        let fps = o.frames_per_second as i64;
        let mut media = GpuMedia::new(&o, w, h, 0)?;
        media.nv12_dump = Some((3, Vec::new()));
        let camera = &case["camera"];
        let mode = camera.is_object().then(|| camera_mode(camera));
        if let Some(m) = &mode {
            media.create_camera_overlay(m, &o)?;
        }
        let every = camera["every"].as_i64().unwrap_or(1);
        let time = VirtualTime::new(0);
        let clock = Arc::new(RecordingClock::new(time));
        clock.start(0);
        let mixer = AudioMixer::new(clock);
        let slots = slots(case);
        let duration = (slots.last().unwrap().slot + 1) * TPS / fps + case["extra_ticks"].as_i64().unwrap_or(0);
        let packets = if o.has_audio() { mp4_packets(&case["audio"], duration) } else { Vec::new() };
        let mut next_packet = 0;
        let mut block = vec![0i16; 960];
        let mut audio_frame = 0i64;
        let mut composited = Vec::new();
        let mut timestamps = Vec::new();
        for (k, s) in slots.iter().enumerate() {
            let timestamp = s.slot * TPS / fps;
            let end = (s.slot + 1) * TPS / fps;
            while next_packet < packets.len() && packets[next_packet].timestamp < end {
                let p = &packets[next_packet];
                mixer.write(p.source, p.timestamp, p.frames, &p.data, p.gain, p.discontinuity);
                next_packet += 1;
            }
            let frame = frame(case["pattern"].as_str().unwrap(), s.n, o.region.x as i64, o.region.y as i64, w as i64, h as i64);
            media.canvas().write_bgra(0, 0, w, h, &frame, w as usize * 4);
            while !media.try_rent(s.slot) {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            if let Some(m) = &mode
                && (k as i64) % every == 0
            {
                media.camera_update(&camera_frame(camera, m, k as i64 / every + 1))?;
            }
            if kind == "gif" && mode.is_some() {
                composited.extend(media.read_bgra_frame()?);
            }
            while !media.write_video(s.slot, timestamp, end, s.discontinuity)? {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            timestamps.push(json!([timestamp, end - timestamp, s.discontinuity]));
            if o.has_audio() {
                write_audio_until(&mixer, &mut block, &mut audio_frame, (timestamp - 50 * 10_000).max(0), false, &mut |pcm, offset| media.write_audio(pcm, offset))?;
            }
        }
        while next_packet < packets.len() {
            let p = &packets[next_packet];
            mixer.write(p.source, p.timestamp, p.frames, &p.data, p.gain, p.discontinuity);
            next_packet += 1;
        }
        if o.has_audio() {
            write_audio_until(&mixer, &mut block, &mut audio_frame, duration, true, &mut |pcm, offset| media.write_audio(pcm, offset))?;
        }
        media.finish_video()?;
        let mut progress = Vec::new();
        media.finish_gif(duration, &mut |p| progress.push(p), &|| false)?;
        let encoder = media.encoder();
        let (ow, oh) = media.output_size();
        if let Some((_, frames)) = media.nv12_dump.take().filter(|(_, f)| !f.is_empty()) {
            std::fs::write(out.join(format!("{name}.nv12")), frames.concat()).map_err(|e| e.to_string())?;
        }
        if !composited.is_empty() {
            std::fs::write(out.join(format!("{name}.frames")), composited).map_err(|e| e.to_string())?;
        }
        drop(media);
        Ok(json!({
            "encoder": encoder.name,
            "hardware": encoder.is_hardware_accelerated,
            "d3d11": encoder.is_d3d11_aware,
            "output": [ow, oh],
            "duration": duration,
            "samples": timestamps,
            "progress": progress,
        }))
    }
}
