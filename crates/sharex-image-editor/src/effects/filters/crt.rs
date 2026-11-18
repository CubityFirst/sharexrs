//! `CRTImageEffect` (`Core/ImageEffects/Filters/CRTImageEffect.cs`).

use crate::effects::prelude::*;

/// `CRTImageEffect`.
#[derive(Clone, Debug)]
pub struct CRTImageEffect {
    pub scanline_strength: f32,
    pub rgb_shift: i32,
    pub noise_amount: f32,
    pub vignette_strength: f32,
    pub seed: i32,
}

impl Default for CRTImageEffect {
    fn default() -> Self {
        CRTImageEffect {
            scanline_strength: 35.0,
            rgb_shift: 1,
            noise_amount: 8.0,
            vignette_strength: 18.0,
            seed: 1337,
        }
    }
}

impl EffectImpl for CRTImageEffect {
    fn info() -> Info {
        Info::new("crt", "CRT", EffectCategory::Filters, "app/monitor")
            .desc("Simulates a CRT monitor with scanlines, RGB shift, noise, and vignette.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "scanlineStrength",
                "Scanline strength",
                0.0,
                100.0,
                35.0,
                |e: &mut Self, v| e.scanline_strength = v,
            ),
            int_slider::<Self>("rgbShift", "RGB shift", 0, 8, 1, |e: &mut Self, v| {
                e.rgb_shift = v
            }),
            float_slider::<Self>(
                "noiseAmount",
                "Noise amount",
                0.0,
                100.0,
                8.0,
                |e: &mut Self, v| e.noise_amount = v,
            ),
            float_slider::<Self>(
                "vignetteStrength",
                "Vignette strength",
                0.0,
                100.0,
                18.0,
                |e: &mut Self, v| e.vignette_strength = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let scanline = clampf(self.scanline_strength, 0.0, 100.0) / 100.0;
        let shift = self.rgb_shift.clamp(0, 8);
        let noise = clampf(self.noise_amount, 0.0, 100.0);
        let vignette = clampf(self.vignette_strength, 0.0, 100.0) / 100.0;
        if scanline <= 0.0 && shift == 0 && noise <= 0.0 && vignette <= 0.0 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let right = width - 1;
        let amp = (noise * 0.6).round_ties_even() as i32;
        let cx = (width - 1) as f32 * 0.5;
        let cy = (height - 1) as f32 * 0.5;
        let inv_cx = if cx > 0.0 { 1.0 / cx } else { 1.0 };
        let inv_cy = if cy > 0.0 { 1.0 / cy } else { 1.0 };
        const INV_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let seed = (self.seed as u32).wrapping_mul(747796405);
        let cl = |v: i32| v.clamp(0, 255) as u8;
        for y in 0..height {
            let row = y * width;
            let mut line_factor = 1f32;
            if scanline > 0.0 {
                let dark = if (y & 1) == 0 { 0.62 } else { 0.18 };
                line_factor = 1.0 - (scanline * dark);
            }
            let dy = (y as f32 - cy) * inv_cy;
            for x in 0..width {
                let idx = row + x;
                let xr = clamp_int(x + shift, 0, right);
                let xb = clamp_int(x - shift, 0, right);
                let sr = src[(row + xr) as usize];
                let sg = src[idx as usize];
                let sb = src[(row + xb) as usize];
                let (mr, mg, mb) = match x % 3 {
                    0 => (1.06f32, 0.97f32, 0.93f32),
                    1 => (0.93, 1.06, 0.97),
                    _ => (0.97, 0.93, 1.06),
                };
                let mut vf = 1f32;
                if vignette > 0.0 {
                    let dx = (x as f32 - cx) * inv_cx;
                    let d = ((dx * dx) + (dy * dy)).sqrt() * INV_SQRT2;
                    let t = clampf((d - 0.55) / 0.45, 0.0, 1.0);
                    let smooth = t * t * (3.0 - (2.0 * t));
                    vf = 1.0 - (vignette * smooth);
                }
                let brightness = line_factor * vf;
                let (mut nr, mut ng, mut nb) = (0, 0, 0);
                if amp > 0 {
                    let mut h = crt_hash(idx as u32 ^ seed);
                    nr = ((h & 0xFF) as i32 - 128) * amp / 128;
                    h = crt_hash(h ^ 0x9E37_79B9);
                    ng = ((h & 0xFF) as i32 - 128) * amp / 128;
                    h = crt_hash(h ^ 0x85EB_CA6B);
                    nb = ((h & 0xFF) as i32 - 128) * amp / 128;
                }
                let r = (sr.r as f32 * mr * brightness).round_ties_even() as i32 + nr;
                let g = (sg.g as f32 * mg * brightness).round_ties_even() as i32 + ng;
                let b = (sb.b as f32 * mb * brightness).round_ties_even() as i32 + nb;
                dst[idx as usize] = SkColor::rgba(cl(r), cl(g), cl(b), sg.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

fn crt_hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    x
}
