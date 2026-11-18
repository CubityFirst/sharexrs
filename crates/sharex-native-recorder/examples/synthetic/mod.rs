//! The harness's synthetic input, generated exactly as
//! tools/native-recorder-harness/Synthetic.cs generates it (integer maths,
//! one `float` division per sample), so both sides feed identical data.

use serde_json::Value;

pub const TPS: i64 = 10_000_000;

/// Frame `n` of `pattern` at the region's origin (`x0`, `y0`), BGRA.
pub fn frame(pattern: &str, n: i64, x0: i64, y0: i64, w: i64, h: i64) -> Vec<u8> {
    let mut out = vec![0u8; (w * h * 4) as usize];
    let s = (w.min(h) / 4).max(2);
    let bx = (n * 5) % (w - s).max(1);
    let by = (n * 3) % (h - s).max(1);
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let px: [u8; 4] = match pattern {
                "scroll" => [((x + n * 4) & 255) as u8, ((y * 2 + n) & 255) as u8, ((x ^ y) & 255) as u8, 255],
                "noise" => {
                    let mut v = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663) ^ (n as u32).wrapping_mul(83_492_791);
                    v ^= v << 13;
                    v ^= v >> 17;
                    v ^= v << 5;
                    [v as u8, (v >> 8) as u8, (v >> 16) as u8, 255]
                }
                _ => {
                    if x >= bx && x < bx + s && y >= by && y < by + s {
                        [40, 180, 250, 255]
                    } else if y >= h - 4 && x * 64 / w == n % 64 {
                        [255, 255, 255, 255]
                    } else {
                        [((x + x0) & 255) as u8, ((y + y0) & 255) as u8, (((x / 8 + y / 8) % 2) * 60 + 40) as u8, 255]
                    }
                }
            };
            out[i..i + 4].copy_from_slice(&px);
        }
    }
    out
}

/// One frame of a case: its pattern number, slot, and whether a slot was skipped before it.
pub struct Slot {
    pub n: i64,
    pub slot: i64,
    pub discontinuity: bool,
}

/// The frames of a case (`frames`, `still`, `drop`).
pub fn slots(case: &Value) -> Vec<Slot> {
    let frames = case["frames"].as_i64().unwrap();
    let drops: Vec<i64> = case["drop"].as_array().map(|a| a.iter().map(|v| v.as_i64().unwrap()).collect()).unwrap_or_default();
    let still: Vec<(i64, i64)> = case["still"]
        .as_array()
        .map(|a| a.iter().map(|r| (r[0].as_i64().unwrap(), r[1].as_i64().unwrap())).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    let mut slot = 0;
    let mut previous = -1;
    for k in 0..frames {
        while drops.contains(&slot) {
            slot += 1;
        }
        let n = still.iter().find(|(a, b)| k >= *a && k < *b).map_or(k, |(a, _)| a - 1);
        out.push(Slot { n, slot, discontinuity: previous >= 0 && slot > previous + 1 });
        previous = slot;
        slot += 1;
    }
    out
}

/// Camera frame `m` (`SyntheticCamera::pattern`), tightly packed top-down.
pub fn camera_frame(format: &str, w: i64, h: i64, m: i64) -> Vec<u8> {
    let (wu, hu) = (w as usize, h as usize);
    let shift = (m * 3) as usize;
    match format {
        "RGB32" => {
            let mut out = vec![0u8; wu * hu * 4];
            for y in 0..hu {
                for x in 0..wu {
                    let i = (y * wu + x) * 4;
                    out[i] = ((x + shift) * 255 / wu) as u8;
                    out[i + 1] = (y * 255 / hu) as u8;
                    out[i + 2] = (((x ^ y) + shift) & 0xFF) as u8;
                    out[i + 3] = 255;
                }
            }
            out
        }
        "YUY2" => {
            let mut out = vec![0u8; wu * hu * 2];
            for y in 0..hu {
                for x in (0..wu).step_by(2) {
                    let i = (y * wu + x) * 2;
                    out[i] = (16 + (x + shift) * 219 / wu) as u8;
                    out[i + 1] = (64 + y * 128 / hu) as u8;
                    out[i + 2] = (16 + (x + 1 + shift) * 219 / wu).min(235) as u8;
                    out[i + 3] = (192 - ((x + shift) * 128 / wu) % 128) as u8;
                }
            }
            out
        }
        _ => {
            let mut out = vec![0u8; wu * hu * 3 / 2];
            for y in 0..hu {
                for x in 0..wu {
                    out[y * wu + x] = (16 + ((x + y + shift) % 220)) as u8;
                }
            }
            for y in 0..hu / 2 {
                for x in (0..wu).step_by(2) {
                    let i = wu * hu + y * wu + x;
                    out[i] = (64 + x * 128 / wu) as u8;
                    out[i + 1] = (192 - y * 2 * 128 / hu) as u8;
                }
            }
            out
        }
    }
}

/// Sample frame `f` of a source (0 system audio, 1 microphone), (left, right).
pub fn sample(source: usize, f: i64) -> (f32, f32) {
    if source == 0 {
        let a = ((f * 37) % 2000 - 1000) as f32 / 2500.0;
        let b = ((f * 53) % 1500 - 750) as f32 / 2000.0;
        (a, b)
    } else {
        let v = ((f * 11) % 960 - 480) as f32 / 1200.0;
        (v, v)
    }
}

/// A packet as the mixer gets it.
pub struct Packet {
    pub source: usize,
    pub timestamp: i64,
    pub frames: i32,
    pub data: Vec<f32>,
    pub gain: f32,
    pub discontinuity: bool,
    /// The capture time of the packet's end (for the read schedule).
    pub end: i64,
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array().map(|a| a.iter().map(|x| x.as_i64().unwrap()).collect()).unwrap_or_default()
}

/// The packets of a mixer case, in write order.
pub fn mixer_packets(case: &Value) -> Vec<Packet> {
    let packets = case["packets"].as_i64().unwrap();
    let base = case["packet_frames"].as_i64().unwrap();
    let vary = case["vary"].as_bool().unwrap_or(false);
    let jitter = ints(&case["jitter"]);
    let drift = case["drift_ppm"].as_i64().unwrap_or(0);
    let gaps = ints(&case["gap_after"]);
    let gap_frames = case["gap_frames"].as_i64().unwrap_or(0);
    let discontinuity = ints(&case["discontinuity"]);
    let silent = ints(&case["silent"]);
    let nan = ints(&case["nan"]);
    let sources: Vec<usize> = case["sources"].as_array().unwrap().iter().map(|s| if s == "system" { 0 } else { 1 }).collect();
    let gains: Vec<f32> = case["gains"].as_array().map(|a| a.iter().map(|g| g.as_f64().unwrap() as f32).collect()).unwrap_or_else(|| vec![1.0; sources.len()]);
    let mut out = Vec::new();
    let mut f = 0i64;
    for i in 0..packets {
        let frames = if vary { base + (i % 3) * 7 - 7 } else { base };
        let mut ts = f * TPS / 48_000;
        if drift != 0 {
            ts = ts * 1_000_000 / (1_000_000 + drift);
        }
        if !jitter.is_empty() {
            ts += jitter[(i as usize) % jitter.len()] * TPS / 48_000;
        }
        let end = ts + frames * TPS / 48_000;
        for (si, &source) in sources.iter().enumerate() {
            let data = if silent.contains(&i) {
                Vec::new()
            } else if nan.contains(&i) {
                vec![f32::NAN; frames as usize * 2]
            } else {
                (0..frames).flat_map(|j| {
                    let (l, r) = sample(source, f + j);
                    [l, r]
                })
                .collect()
            };
            out.push(Packet { source, timestamp: ts.max(0), frames: frames as i32, data, gain: gains[si], discontinuity: discontinuity.contains(&i), end });
        }
        f += frames;
        if gaps.contains(&i) {
            f += gap_frames;
        }
    }
    out
}

/// The 480-frame packets of an MP4 case's sources, by start time.
pub fn mp4_packets(audio: &Value, until: i64) -> Vec<Packet> {
    let mut sources = Vec::new();
    if audio["system"].as_bool().unwrap_or(false) {
        sources.push((0usize, audio["system_gain"].as_f64().unwrap_or(1.0) as f32));
    }
    if audio["microphone"].as_bool().unwrap_or(false) {
        sources.push((1usize, audio["microphone_gain"].as_f64().unwrap_or(1.0) as f32));
    }
    let mut out = Vec::new();
    let mut f = 0i64;
    loop {
        let ts = f * TPS / 48_000;
        if ts >= until {
            break;
        }
        for &(source, gain) in &sources {
            let data = (0..480).flat_map(|j| {
                let (l, r) = sample(source, f + j);
                [l, r]
            })
            .collect();
            out.push(Packet { source, timestamp: ts, frames: 480, data, gain, discontinuity: false, end: ts + 480 * TPS / 48_000 });
        }
        f += 480;
    }
    out
}
