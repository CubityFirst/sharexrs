//! `RustCorrosionImageEffect` (`Core/ImageEffects/Filters/RustCorrosionImageEffect.cs`).

use crate::effects::prelude::*;

/// `RustCorrosionImageEffect`.
#[derive(Clone, Debug)]
pub struct RustCorrosionImageEffect {
    pub corrosion: f32,
    pub pitting: f32,
    pub streaks: f32,
    pub dirt: f32,
    pub edge_wear: f32,
}

impl Default for RustCorrosionImageEffect {
    fn default() -> Self {
        RustCorrosionImageEffect {
            corrosion: 58.0,
            pitting: 44.0,
            streaks: 36.0,
            dirt: 26.0,
            edge_wear: 52.0,
        }
    }
}

impl EffectImpl for RustCorrosionImageEffect {
    fn info() -> Info {
        Info::new("rust_corrosion", "Rust / corrosion", EffectCategory::Filters, "app/clear-all")
            .desc("Simulates rust and corrosion effects with pitting and streaks.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("corrosion", "Corrosion", 0.0, 100.0, 58.0, |e: &mut Self, v| e.corrosion = v),
            float_slider::<Self>("pitting", "Pitting", 0.0, 100.0, 44.0, |e: &mut Self, v| e.pitting = v),
            float_slider::<Self>("streaks", "Streaks", 0.0, 100.0, 36.0, |e: &mut Self, v| e.streaks = v),
            float_slider::<Self>("dirt", "Dirt", 0.0, 100.0, 26.0, |e: &mut Self, v| e.dirt = v),
            float_slider::<Self>("edge_wear", "Edge wear", 0.0, 100.0, 52.0, |e: &mut Self, v| e.edge_wear = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let corrosion = clampf(self.corrosion, 0.0, 100.0) / 100.0;
        let pitting = clampf(self.pitting, 0.0, 100.0) / 100.0;
        let streaks = clampf(self.streaks, 0.0, 100.0) / 100.0;
        let dirt = clampf(self.dirt, 0.0, 100.0) / 100.0;
        let edge_wear = clampf(self.edge_wear, 0.0, 100.0) / 100.0;
        if corrosion <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let lum = |c: SkColor| (0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32);
        let p = |x: i32, y: i32| src[(y * width + x) as usize];
        for y in 0..height {
            let row = y * width;
            let v = y as f32 / 1.max(height - 1) as f32;
            for x in 0..width {
                let (xf, yf) = (x as f32, y as f32);
                let s = src[(row + x) as usize];
                let (mut r, mut g, mut b) = (s.r as f32, s.g as f32, s.b as f32);
                let edge = clamp01(
                    ((lum(p((width - 1).min(x + 1), y)) / 255.0 - lum(p(0.max(x - 1), y)) / 255.0).abs()
                        + (lum(p(x, (height - 1).min(y + 1))) / 255.0 - lum(p(x, 0.max(y - 1))) / 255.0).abs())
                        * 1.1,
                );
                let warp_scale = 3.5 + (corrosion * 12.0);
                let wx = ((fractal_noise(xf * 0.010, yf * 0.010, 3, 2.05, 0.55, 541) * 2.0) - 1.0) * warp_scale;
                let wy = ((fractal_noise((xf + 91.7) * 0.010, (yf - 63.4) * 0.010, 3, 2.00, 0.55, 887) * 2.0) - 1.0)
                    * warp_scale
                    * 0.75;
                let (sx, sy) = (xf + wx, yf + wy);
                let coarse = fractal_noise(sx * 0.016, sy * 0.016, 3, 2.00, 0.56, 701);
                let medium = fractal_noise(sx * 0.040, sy * 0.040, 3, 2.15, 0.52, 1301);
                let fine = fractal_noise(sx * 0.105, sy * 0.105, 2, 2.30, 0.60, 1907);
                let ss = fractal_noise((xf * 0.012) + 17.2, (yf * 0.0035) + 4.1, 3, 2.00, 0.58, 2203);
                let mut streak = smooth_step(0.70 - (streaks * 0.08), 0.98, ss) * streaks * v;
                streak *= 0.42 + (0.58 * fractal_noise((xf - 41.3) * 0.022, (yf + 18.7) * 0.060, 2, 2.20, 0.62, 3251));
                let mut mask = (coarse * 0.52) + (medium * 0.32) + (fine * 0.16);
                mask = smooth_step(0.48 - (corrosion * 0.18), 0.94, mask);
                mask = mask.max(edge * edge_wear * 0.72);
                mask = clamp01(mask * corrosion + streak);
                if mask > 0.0001 {
                    let tone = (coarse * 0.56) + (medium * 0.32) + (fine * 0.12);
                    let warmth = fractal_noise((sx + 37.1) * 0.030, (sy - 22.4) * 0.030, 2, 2.10, 0.60, 4783);
                    let rr = lerp(126.0, 214.0, (tone * 0.72) + (warmth * 0.28));
                    let rg = lerp(44.0, 108.0, tone);
                    let rb = lerp(12.0, 40.0, tone * 0.88);
                    r = lerp(r, rr, mask * 0.86);
                    g = lerp(g, rg, mask * 0.82);
                    b = lerp(b, rb, mask * 0.78);
                    let pn = fractal_noise(sx * 0.150, sy * 0.150, 2, 2.35, 0.58, 6121);
                    let pit = smooth_step(0.72, 0.96, pn) * pitting * mask;
                    r *= 1.0 - (pit * 0.26);
                    g *= 1.0 - (pit * 0.34);
                    b *= 1.0 - (pit * 0.38);
                }
                if dirt > 0.0001 {
                    let grime = ((fractal_noise(xf * 0.028, yf * 0.028, 3, 2.20, 0.55, 4021) * 2.0) - 1.0) * dirt * 14.0;
                    r += grime * 0.80;
                    g += grime * 0.70;
                    b += grime * 0.55;
                }
                dst[(row + x) as usize] = SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
