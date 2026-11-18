//! `ClaymationTextureImageEffect` (`Core/ImageEffects/Filters/ClaymationTextureImageEffect.cs`).

use crate::effects::prelude::*;

/// `ClaymationTextureImageEffect`.
#[derive(Clone, Debug)]
pub struct ClaymationTextureImageEffect {
    pub chunk_size: i32,
    pub smoothness: f32,
    pub relief: f32,
    pub saturation: f32,
    pub texture_grain: f32,
}

impl Default for ClaymationTextureImageEffect {
    fn default() -> Self {
        ClaymationTextureImageEffect {
            chunk_size: 8,
            smoothness: 55.0,
            relief: 35.0,
            saturation: 20.0,
            texture_grain: 30.0,
        }
    }
}

impl EffectImpl for ClaymationTextureImageEffect {
    fn info() -> Info {
        Info::new("claymation_texture", "Claymation texture", EffectCategory::Filters, "image-effect/paint-roller")
            .desc("Gives the image a sculpted claymation look with chunky quantization and relief texture.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("chunkSize", "Chunk size", 3, 18, 8, |e: &mut Self, v| e.chunk_size = v),
            float_slider::<Self>("smoothness", "Smoothness", 0.0, 100.0, 55.0, |e: &mut Self, v| e.smoothness = v),
            float_slider::<Self>("relief", "Relief", 0.0, 100.0, 35.0, |e: &mut Self, v| e.relief = v),
            float_slider::<Self>("saturation", "Saturation", -100.0, 100.0, 20.0, |e: &mut Self, v| e.saturation = v),
            float_slider::<Self>("textureGrain", "Texture grain", 0.0, 100.0, 30.0, |e: &mut Self, v| e.texture_grain = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let chunk = self.chunk_size.clamp(3, 18);
        let smoothness = clampf(self.smoothness, 0.0, 100.0) / 100.0;
        let relief = clampf(self.relief, 0.0, 100.0) / 100.0;
        let saturation = clampf(self.saturation, -100.0, 100.0) / 100.0;
        let grain = clampf(self.texture_grain, 0.0, 100.0) / 100.0;
        let seed = random::shared(|r| r.next_range(1, i32::MAX));
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let blur_radius = (chunk / 4).clamp(1, 4);
        let smooth_mix = smoothness * 0.9;
        let sat_factor = 1.0 + saturation;
        let cell = chunk as f32 * 1.85;
        let levels = (20 - chunk).clamp(5, 18);
        let qstep = 255.0 / (levels - 1) as f32;
        let quant = |v: f32| (v / qstep).round_ties_even() * qstep;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let pi = std::f32::consts::PI;
        for y in 0..height {
            let row = y * width;
            let y_up = (y - blur_radius).max(0);
            let y_down = (y + blur_radius).min(bottom);
            for x in 0..width {
                let x_left = (x - blur_radius).max(0);
                let x_right = (x + blur_radius).min(right);
                let p = |xx: i32, yy: i32| src[(yy * width + xx) as usize];
                let (c, n, s, w, e, nw, se) = (
                    p(x, y),
                    p(x, y_up),
                    p(x, y_down),
                    p(x_left, y),
                    p(x_right, y),
                    p(x_left, y_up),
                    p(x_right, y_down),
                );
                let avg = |f: fn(SkColor) -> u8| {
                    ((f(c) as f32 * 3.0)
                        + (f(n) as f32 * 1.5)
                        + (f(s) as f32 * 1.5)
                        + (f(w) as f32 * 1.5)
                        + (f(e) as f32 * 1.5)
                        + f(nw) as f32
                        + f(se) as f32)
                        / 10.0
                };
                let (ar, ag, ab) = (avg(|c| c.r), avg(|c| c.g), avg(|c| c.b));
                let mut r = quant(lerp(c.r as f32, ar, smooth_mix));
                let mut g = quant(lerp(c.g as f32, ag, smooth_mix));
                let mut b = quant(lerp(c.b as f32, ab, smooth_mix));
                let fx = x as f32 / cell;
                let fy = y as f32 / cell;
                let cx = fx.floor() as i32;
                let cy = fy.floor() as i32;
                let tx = fx - cx as f32;
                let ty = fy - cy as f32;
                let n00 = hash01(cx, cy, seed);
                let n10 = hash01(cx + 1, cy, seed);
                let n01 = hash01(cx, cy + 1, seed);
                let n11 = hash01(cx + 1, cy + 1, seed);
                let coarse = lerp(lerp(n00, n10, tx), lerp(n01, n11, tx), ty);
                let ridge_a = (((x as f32 + (coarse * 13.0)) / cell) * pi * 2.0).sin();
                let ridge_b = (((y as f32 - (coarse * 11.0)) / cell) * pi * 2.0).cos();
                let ridge = ridge_a * ridge_b;
                let fine = ((hash01(x, y, seed ^ 919) * 2.0) - 1.0) * grain;
                let relief_offset = (((coarse - 0.5) * 2.0) + (ridge * 0.65)) * relief * 23.0;
                let grain_offset = fine * (4.0 + (relief * 7.0));
                let offset = relief_offset + grain_offset;
                r += offset + (relief * 2.0);
                g += offset;
                b += offset - (relief * 2.5);
                let gray = (0.2126 * r) + (0.7152 * g) + (0.0722 * b);
                r = gray + ((r - gray) * sat_factor);
                g = gray + ((g - gray) * sat_factor);
                b = gray + ((b - gray) * sat_factor);
                dst[(row + x) as usize] = SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), c.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
