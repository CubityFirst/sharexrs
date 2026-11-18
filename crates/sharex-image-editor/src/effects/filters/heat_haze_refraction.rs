//! `HeatHazeRefractionImageEffect` (`Core/ImageEffects/Filters/HeatHazeRefractionImageEffect.cs`).

use crate::effects::prelude::*;

/// `HeatHazeRefractionImageEffect`.
#[derive(Clone, Debug)]
pub struct HeatHazeRefractionImageEffect {
    pub strength: f32,
    pub frequency: f32,
    pub blur_radius: f32,
    pub offset: f32,
    pub luminance_influence: f32,
    pub seed: i32,
}

impl Default for HeatHazeRefractionImageEffect {
    fn default() -> Self {
        HeatHazeRefractionImageEffect {
            strength: 45.0,
            frequency: 40.0,
            blur_radius: 10.0,
            offset: 6.0,
            luminance_influence: 55.0,
            seed: 2026,
        }
    }
}

impl EffectImpl for HeatHazeRefractionImageEffect {
    fn info() -> Info {
        Info::new("heat_haze_refraction", "Heat haze refraction", EffectCategory::Filters, "image-effect/thermometer-sun")
            .desc("Simulates heat haze refraction with wavy distortion.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 45.0, |e: &mut Self, v| e.strength = v),
            float_slider::<Self>("frequency", "Frequency", 0.0, 100.0, 40.0, |e: &mut Self, v| e.frequency = v),
            float_slider::<Self>("blur_radius", "Blur radius", 0.0, 200.0, 10.0, |e: &mut Self, v| e.blur_radius = v),
            float_slider::<Self>("offset", "Offset", -200.0, 200.0, 6.0, |e: &mut Self, v| e.offset = v),
            float_slider::<Self>("luminance_influence", "Luminance influence", 0.0, 100.0, 55.0, |e: &mut Self, v| e.luminance_influence = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let strength = clampf(self.strength, 0.0, 100.0) / 100.0;
        if strength <= 0.0 {
            return Ok(source.clone());
        }
        let freq = clampf(self.frequency, 0.0, 100.0) / 100.0;
        let blur_px = clampf(self.blur_radius, 0.0, 30.0);
        let max_off = clampf(self.offset, -200.0, 200.0) * (0.5 + 1.2 * freq) * strength;
        let lum_inf = clampf(self.luminance_influence, 0.0, 100.0) / 100.0;
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let blurred = (blur_px > 0.01 && strength > 0.01).then(|| pixels(&blur(source, blur_px, blur_px)));
        let inv_w = 1.0 / 1f32.max((width - 1) as f32);
        let inv_h = 1.0 / 1f32.max((height - 1) as f32);
        let phase = (self.seed % 10_000) as f32 * 0.017;
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let v = y as f32 * inv_h;
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let s = src[idx];
                let l = ((0.2126 * s.r as f32) + (0.7152 * s.g as f32) + (0.0722 * s.b as f32)) / (255.0 * 1.0);
                let lw = lerp(1.0, l, lum_inf);
                let u = x as f32 * inv_w;
                let w1 = ((u * 10.0 + v * 6.0) * (0.6 + freq * 1.8) + phase).sin();
                let w2 = ((u * 7.0 - v * 12.0) * (0.4 + freq * 1.7) - phase * 1.23).cos();
                let dx = (w1 * 0.60 + w2 * 0.40) * max_off * lw;
                let dy = (w2 * 0.55 - w1 * 0.45) * max_off * lw;
                let warped = bilinear_sample(&src, width, height, x as f32 + dx, y as f32 + dy);
                let (mut wr, mut wg, mut wb) = (warped.r as f32 / 255.0, warped.g as f32 / 255.0, warped.b as f32 / 255.0);
                if let Some(bp) = &blurred {
                    let bl = bp[idx];
                    let mix = strength * (0.25 + 0.75 * (blur_px / 30.0));
                    wr = lerp(wr, bl.r as f32 / 255.0, mix);
                    wg = lerp(wg, bl.g as f32 / 255.0, mix);
                    wb = lerp(wb, bl.b as f32 / 255.0, mix);
                }
                let r = lerp(s.r as f32 / 255.0, wr, strength);
                let g = lerp(s.g as f32 / 255.0, wg, strength);
                let b = lerp(s.b as f32 / 255.0, wb, strength);
                dst[idx] = SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
