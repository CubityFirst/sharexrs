//! `RisoPrintImageEffect` (`Core/ImageEffects/Filters/RisoPrintImageEffect.cs`).

use crate::effects::prelude::*;

/// `RisoPrintImageEffect`.
#[derive(Clone, Debug)]
pub struct RisoPrintImageEffect {
    pub ink_strength: f32,
    pub paper_fade: f32,
    pub offset: f32,
    pub dot_scale: f32,
    pub ink_noise: f32,
    pub seed: i32,
    pub ink_color_a: SkColor,
    pub ink_color_b: SkColor,
}

impl Default for RisoPrintImageEffect {
    fn default() -> Self {
        RisoPrintImageEffect {
            ink_strength: 70.0,
            paper_fade: 25.0,
            offset: 3.0,
            dot_scale: 18.0,
            ink_noise: 35.0,
            seed: 2026,
            ink_color_a: SkColor::argb(0xFFDC4646),
            ink_color_b: SkColor::argb(0xFF46C8D2),
        }
    }
}

impl EffectImpl for RisoPrintImageEffect {
    fn info() -> Info {
        Info::new("riso_print", "Riso print", EffectCategory::Filters, "app/printer")
            .desc("Simulates a risograph print with halftone dots and ink colors.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("ink_strength", "Ink strength", 0.0, 100.0, 70.0, |e: &mut Self, v| e.ink_strength = v),
            float_slider::<Self>("paper_fade", "Paper fade", 0.0, 100.0, 25.0, |e: &mut Self, v| e.paper_fade = v),
            float_slider::<Self>("offset", "Offset", -200.0, 200.0, 3.0, |e: &mut Self, v| e.offset = v),
            float_slider::<Self>("dot_scale", "Dot scale", 0.0, 100.0, 18.0, |e: &mut Self, v| e.dot_scale = v),
            float_slider::<Self>("ink_noise", "Ink noise", 0.0, 100.0, 35.0, |e: &mut Self, v| e.ink_noise = v),
            color_param::<Self>("ink_color_a", "Ink color A", SkColor::argb(0xFFDC4646), |e: &mut Self, v| e.ink_color_a = v),
            color_param::<Self>("ink_color_b", "Ink color B", SkColor::argb(0xFF46C8D2), |e: &mut Self, v| e.ink_color_b = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let ink = clampf(self.ink_strength, 0.0, 100.0) / 100.0;
        if ink <= 0.0 {
            return Ok(source.clone());
        }
        let fade = clampf(self.paper_fade, 0.0, 100.0) / 100.0;
        let dot_scale = clampf(self.dot_scale, 0.0, 100.0) / 100.0;
        let noise = clampf(self.ink_noise, 0.0, 100.0) / 100.0;
        let misreg = clampf(self.offset, -200.0, 200.0) * 0.30;
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let cell = ((4.0 + (dot_scale * 20.0)).round_ties_even() as i32).clamp(3, 28);
        let (ca, cb) = (self.ink_color_a, self.ink_color_b);
        let aw = (ca.a as f32 / 255.0) * ink;
        let bw = (cb.a as f32 / 255.0) * ink;
        let keep = clamp01(1.0 - (fade * 0.90));
        let seed = self.seed;
        let dot = |x: i32, y: i32, coverage: f32, ink_seed: i32| -> f32 {
            let cx = (x / cell) * cell + (cell / 2);
            let cy = (y / cell) * cell + (cell / 2);
            let dx = (x - cx) as f32;
            let dy = (y - cy) as f32;
            let dist = ((dx * dx) + (dy * dy)).sqrt();
            let max_r = cell as f32 * 0.45;
            let mut r = max_r * 0f32.max(coverage).sqrt();
            let h = hash01(x, y, seed ^ ink_seed);
            r *= 1.0 + (0.25 * ((h * 2.0 - 1.0) * noise));
            let edge = 1.0 + (0.8 * noise);
            1.0 - smooth_step(r, r + edge, dist)
        };
        for y in 0..height {
            for x in 0..width {
                let i = (y * width + x) as usize;
                let s = src[i];
                let l = ((0.2126 * s.r as f32) + (0.7152 * s.g as f32) + (0.0722 * s.b as f32)) / (255.0 * 1.0);
                let coverage = clamp01(1.0 - l);
                let da = dot(x, y, coverage, 0xA11);
                let xb = (x as f32 + misreg).round_ties_even() as i32;
                let db = dot(xb, y, coverage, 0xB22);
                let a = clamp01(da * aw * keep);
                let b = clamp01(db * bw * keep);
                let (mut or, mut og, mut ob) = (1f32, 1f32, 1f32);
                or = (or * (1.0 - a)) + ((ca.r as f32 / 255.0) * a);
                og = (og * (1.0 - a)) + ((ca.g as f32 / 255.0) * a);
                ob = (ob * (1.0 - a)) + ((ca.b as f32 / 255.0) * a);
                or = (or * (1.0 - b)) + ((cb.r as f32 / 255.0) * b);
                og = (og * (1.0 - b)) + ((cb.g as f32 / 255.0) * b);
                ob = (ob * (1.0 - b)) + ((cb.b as f32 / 255.0) * b);
                if noise > 0.001 {
                    let n = (hash01(x, y, seed ^ 0x99) * 2.0) - 1.0;
                    let grain = n * noise * 0.06;
                    or = 0f32.max(or + grain);
                    og = 0f32.max(og + grain * 0.92);
                    ob = 0f32.max(ob + grain * 1.06);
                }
                dst[i] = SkColor::rgba(clamp_to_byte(or * 255.0), clamp_to_byte(og * 255.0), clamp_to_byte(ob * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
