//! `WoodcutPrintImageEffect` (`Core/ImageEffects/Filters/WoodcutPrintImageEffect.cs`).

use crate::effects::prelude::*;

/// `WoodcutPrintImageEffect`.
#[derive(Clone, Debug)]
pub struct WoodcutPrintImageEffect {
    pub threshold: f32,
    pub edge_strength: f32,
    pub grain_amount: f32,
    pub ink_color: SkColor,
    pub paper_color: SkColor,
}

impl Default for WoodcutPrintImageEffect {
    fn default() -> Self {
        WoodcutPrintImageEffect {
            threshold: 50.0,
            edge_strength: 60.0,
            grain_amount: 30.0,
            ink_color: SkColor::argb(0xFF0F0A05),
            paper_color: SkColor::argb(0xFFEBE1C8),
        }
    }
}

impl EffectImpl for WoodcutPrintImageEffect {
    fn info() -> Info {
        Info::new("woodcut_print", "Woodcut Print", EffectCategory::Filters, "image-effect/axe")
            .desc("Simulates a woodcut or linocut print style.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("threshold", "Threshold", 0.0, 100.0, 50.0, |e: &mut Self, v| e.threshold = v),
            float_slider::<Self>("edge_strength", "Edge Strength", 0.0, 100.0, 60.0, |e: &mut Self, v| e.edge_strength = v),
            float_slider::<Self>("grain_amount", "Wood Grain", 0.0, 100.0, 30.0, |e: &mut Self, v| e.grain_amount = v),
            color_param::<Self>("ink_color", "Ink Color", SkColor::argb(0xFF0F0A05), |e: &mut Self, v| e.ink_color = v),
            color_param::<Self>("paper_color", "Paper Color", SkColor::argb(0xFFEBE1C8), |e: &mut Self, v| e.paper_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        let threshold = clampf(self.threshold, 0.0, 100.0) / 100.0;
        let edge_w = clampf(self.edge_strength, 0.0, 100.0) / 100.0;
        let grain_w = clampf(self.grain_amount, 0.0, 100.0) / 100.0;
        let src = pixels(source);
        let lum: Vec<f32> = src
            .iter()
            .map(|c| (0.299 * c.r as f32 + 0.587 * c.g as f32 + 0.114 * c.b as f32) / 255.0)
            .collect();
        let mut edges = vec![0f32; src.len()];
        let l = |x: i32, y: i32| lum[(y * width + x) as usize];
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let gx = -l(x - 1, y - 1) + l(x + 1, y - 1) - 2.0 * l(x - 1, y) + 2.0 * l(x + 1, y) - l(x - 1, y + 1)
                    + l(x + 1, y + 1);
                let gy = -l(x - 1, y - 1) - 2.0 * l(x, y - 1) - l(x + 1, y - 1) + l(x - 1, y + 1) + 2.0 * l(x, y + 1)
                    + l(x + 1, y + 1);
                edges[(y * width + x) as usize] = (gx * gx + gy * gy).sqrt();
            }
        }
        let mut grain = vec![0f32; src.len()];
        if grain_w > 0.001 {
            let mut rng: i32 = 7919;
            let mut phases = vec![0f32; height.max(0) as usize];
            for p in phases.iter_mut() {
                rng ^= rng.wrapping_shl(13);
                rng ^= rng >> 17;
                rng ^= rng.wrapping_shl(5);
                *p = (rng & 0x7FFF_FFFF) as f32 / i32::MAX as f32 * std::f32::consts::PI * 2.0;
            }
            for y in 0..height {
                let phase = phases[y as usize];
                for x in 0..width {
                    let wave = (x as f32 * 0.08 + phase).sin() * 0.5 + 0.5;
                    let fine = (x as f32 * 0.3 + y as f32 * 0.15 + phase * 2.0).sin() * 0.5 + 0.5;
                    grain[(y * width + x) as usize] = wave * 0.6 + fine * 0.4;
                }
            }
        }
        let (ink, paper) = (self.ink_color, self.paper_color);
        let (ir, ig, ib) = (ink.r as f32 / 255.0, ink.g as f32 / 255.0, ink.b as f32 / 255.0);
        let (pr, pg, pb) = (paper.r as f32 / 255.0, paper.g as f32 / 255.0, paper.b as f32 / 255.0);
        let to = |v: f32| (clampf(v, 0.0, 1.0) * 255.0).round_ties_even() as u8;
        let dst: Vec<SkColor> = (0..src.len())
            .map(|i| {
                let local = threshold + (grain[i] - 0.5) * grain_w * 0.4;
                let score = lum[i] - edges[i] * edge_w;
                let is_ink = if score < local - 0.05 {
                    1.0
                } else if score > local + 0.05 {
                    0.0
                } else {
                    1.0 - (score - (local - 0.05)) / 0.1
                };
                SkColor::rgba(
                    to(ir * is_ink + pr * (1.0 - is_ink)),
                    to(ig * is_ink + pg * (1.0 - is_ink)),
                    to(ib * is_ink + pb * (1.0 - is_ink)),
                    src[i].a,
                )
            })
            .collect();
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
