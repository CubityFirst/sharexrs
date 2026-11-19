//! ShareXrs's side of the harness's hardware cases (Hardware.cs there):
//! the device lists, `Drain` on synthetic packets, and the HDR tone mapper
//! on a synthetic FP16 texture.

use std::sync::Arc;

use serde_json::{Value, json};
use sharex_native_recorder::audio::{CapturePacket, DrainState, drain_packet};
use sharex_native_recorder::clock::{RecordingClock, VirtualTime};
use sharex_native_recorder::mixer::{AudioMixer, write_audio_until};

/// The device lists, as Hardware.Devices writes them.
pub fn devices() -> Value {
    use sharex_native_recorder::devices as d;
    let audio = |l: Result<Vec<d::AudioCaptureDevice>, String>| l.unwrap_or_default().into_iter().map(|x| json!({ "id": x.id, "name": x.name })).collect::<Vec<_>>();
    let one = |x: Option<d::AudioCaptureDevice>| x.map_or(Value::Null, |x| json!({ "id": x.id, "name": x.name }));
    json!({
        "system_audio": audio(d::system_audio_devices()),
        "microphones": audio(d::microphones()),
        "default_system_audio": one(d::default_system_audio_device()),
        "default_microphone": one(d::default_microphone()),
        "cameras": d::cameras().unwrap_or_default().into_iter().map(|x| json!({ "id": x.id, "name": x.name })).collect::<Vec<_>>(),
    })
}

struct Packet {
    frames: u32,
    flags: u32,
    qpc: u64,
    data: Vec<f32>,
}

/// `DrainPackets`: endpoint `e`'s packets of batch `batch`, samples numbered across the endpoint.
fn packets(endpoint: &Value, batch: usize) -> Vec<Packet> {
    let mut out = Vec::new();
    let mut index = 0i64;
    for (current, b) in endpoint["batches"].as_array().unwrap().iter().enumerate() {
        for p in b.as_array().unwrap() {
            if current == batch {
                let frames = p[0].as_u64().unwrap() as u32;
                let mut data = vec![0f32; frames as usize * 2];
                for i in 0..frames as i64 {
                    for ch in 0..2i64 {
                        data[(i * 2 + ch) as usize] = ((index * 7 + i * 3 + ch * 5) % 200 - 100) as f32 / 128.0;
                    }
                }
                out.push(Packet { frames, flags: p[1].as_u64().unwrap() as u32, qpc: p[2].as_u64().unwrap(), data });
            }
            index += 1;
        }
    }
    out
}

/// A drain case: each batch's packets drained per endpoint, the PCM read
/// 50 ms behind each batch's end, then the rest; and the discontinuities.
pub fn drain(case: &Value) -> (Vec<u8>, Value) {
    let clock = Arc::new(RecordingClock::new(VirtualTime::new(0)));
    clock.start(0);
    let mixer = AudioMixer::new(clock);
    let endpoints = case["endpoints"].as_array().unwrap();
    let mut states = vec![DrainState::default(); endpoints.len()];
    let batches = endpoints.iter().map(|e| e["batches"].as_array().unwrap().len()).max().unwrap_or(0);
    let (mut pcm, mut block, mut first, mut last_end) = (Vec::new(), vec![0i16; 960], 0i64, 0i64);
    let mut read = |mixer: &AudioMixer, until: i64, partial: bool, first: &mut i64, pcm: &mut Vec<u8>| {
        write_audio_until(mixer, &mut block, first, until, partial, &mut |s, _| {
            pcm.extend(s.iter().flat_map(|v| v.to_le_bytes()));
            Ok(())
        })
        .unwrap();
    };
    for b in 0..batches {
        let mut batch_end = 0i64;
        for (e, state) in endpoints.iter().zip(states.iter_mut()) {
            if b >= e["batches"].as_array().unwrap().len() {
                continue;
            }
            let (source, gain) = (e["source"].as_u64().unwrap() as usize, e["gain"].as_f64().unwrap() as f32);
            for p in packets(e, b) {
                batch_end = batch_end.max(p.qpc as i64 + p.frames as i64 * 10_000_000 / 48_000);
                let data: &[f32] = if p.flags & 2 == 0 { &p.data } else { &[] };
                drain_packet(state, &mixer, source, gain, CapturePacket { frames: p.frames, flags: p.flags, qpc_position: p.qpc, data }, 0);
            }
        }
        last_end = last_end.max(batch_end);
        read(&mixer, (batch_end - 50 * 10_000).max(0), false, &mut first, &mut pcm);
    }
    read(&mixer, last_end, true, &mut first, &mut pcm);
    (pcm, json!({ "discontinuities": mixer.discontinuities() }))
}

/// `HdrValue`.
pub fn hdr_value(x: i32, y: i32, ch: i32) -> f32 {
    if ch == 1 && (x * y) % 29 == 7 {
        return f32::NAN;
    }
    let v = ((x * 5 + y * 3 + ch * 7) % 401) as f32 / 16.0;
    if (x + y) % 13 == 0 { -v } else { v }
}

/// `(Half)value` for the values `hdr_value` makes (exact in FP16), NaN as .NET's quiet NaN.
pub fn half_bits(v: f32) -> u16 {
    if v.is_nan() {
        // .NET: (Half)float.NaN keeps the sign and top mantissa bits of float.NaN (0xFFC00000).
        return 0xFE00;
    }
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let a = v.abs();
    if a == 0.0 {
        return sign;
    }
    let exp = ((bits >> 23) & 0xFF) as i32 - 127 + 15;
    assert!((1..31).contains(&exp), "not a normal FP16 value");
    let mantissa = ((bits & 0x7F_FFFF) >> 13) as u16;
    assert_eq!(bits & 0x1FFF, 0, "not exact in FP16");
    sign | ((exp as u16) << 10) | mantissa
}

#[cfg(windows)]
pub fn hdr(case: &Value) -> Result<Vec<u8>, String> {
    use sharex_native_recorder::options::Rect;
    use sharex_native_recorder::windows::device::GraphicsDevice;
    use sharex_native_recorder::windows::hdr::{HdrToneMapShader, MonitorColorInfo};
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT};
    let n = |k: &str| case[k].as_i64().unwrap() as i32;
    let (sw, sh, cw, ch, dx, dy) = (n("width"), n("height"), n("canvas_width"), n("canvas_height"), n("x"), n("y"));
    let graphics = GraphicsDevice::new(0)?;
    let mut half = vec![0u16; (sw * sh * 4) as usize];
    for y in 0..sh {
        for x in 0..sw {
            for k in 0..4 {
                half[((y * sw + x) * 4 + k) as usize] = if k == 3 { half_bits(1.0) } else { half_bits(hdr_value(x, y, k)) };
            }
        }
    }
    let source = graphics.create_texture(sw, sh, DXGI_FORMAT_R16G16B16A16_FLOAT, D3D11_BIND_SHADER_RESOURCE.0 as u32)?;
    let canvas = graphics.create_texture(cw, ch, DXGI_FORMAT_B8G8R8A8_UNORM, D3D11_BIND_RENDER_TARGET.0 as u32)?;
    let (mut view, mut target) = (None, None);
    // SAFETY: D3D11 calls on textures made here, with buffers of their sizes.
    unsafe {
        graphics.context.UpdateSubresource(&source, 0, None, half.as_ptr().cast(), (sw * 8) as u32, 0);
        graphics.device.CreateShaderResourceView(&source, None, Some(&mut view)).map_err(|e| e.to_string())?;
        graphics.device.CreateRenderTargetView(&canvas, None, Some(&mut target)).map_err(|e| e.to_string())?;
    }
    let (view, target) = (view.unwrap(), target.unwrap());
    // SAFETY: the canvas's own view.
    unsafe { graphics.context.ClearRenderTargetView(&target, &[0.0, 0.0, 0.0, 1.0]) };
    let color = MonitorColorInfo { is_hdr: true, sdr_white_scale: case["white"].as_f64().unwrap() as f32, peak_nits: case["peak"].as_f64().unwrap() as f32 };
    HdrToneMapShader::new(&graphics)?.draw(&graphics, &target, &view, Rect::new(dx, dy, sw, sh), color);
    let staging = graphics.create_staging(cw, ch, DXGI_FORMAT_B8G8R8A8_UNORM)?;
    let mut out = vec![0u8; (cw * ch * 4) as usize];
    graphics.read_bgra(&staging, &canvas, cw, ch, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn halves() {
        assert_eq!(super::half_bits(1.0), 0x3C00);
        assert_eq!(super::half_bits(-0.5), 0xB800);
        assert_eq!(super::half_bits(25.0), 0x4E40);
        assert_eq!(super::half_bits(1.0 / 16.0), 0x2C00);
    }
}
