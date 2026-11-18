//! `SoftDiffusionImageEffect` (`Core/ImageEffects/Filters/SoftDiffusionImageEffect.cs`).

use crate::effects::prelude::*;

/// `SoftDiffusionImageEffect`.
#[derive(Clone, Debug)]
pub struct SoftDiffusionImageEffect {
    pub amount: f32,
    pub radius: f32,
    pub highlight_bloom: f32,
    pub contrast_softening: f32,
    pub warmth: f32,
}

impl Default for SoftDiffusionImageEffect {
    fn default() -> Self {
        SoftDiffusionImageEffect {
            amount: 52.0,
            radius: 12.0,
            highlight_bloom: 40.0,
            contrast_softening: 28.0,
            warmth: 22.0,
        }
    }
}

impl EffectImpl for SoftDiffusionImageEffect {
    fn info() -> Info {
        Info::new(
            "soft_diffusion",
            "Soft diffusion",
            EffectCategory::Filters,
            "image-effect/soft-glow",
        )
        .desc("Applies a soft diffusion glow with bloom and warmth.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", 0.0, 100.0, 52.0, |e: &mut Self, v| {
                e.amount = v
            }),
            float_slider::<Self>("radius", "Radius", 0.0, 30.0, 12.0, |e: &mut Self, v| {
                e.radius = v
            }),
            float_slider::<Self>(
                "highlight_bloom",
                "Highlight bloom",
                0.0,
                100.0,
                40.0,
                |e: &mut Self, v| e.highlight_bloom = v,
            ),
            float_slider::<Self>(
                "contrast_softening",
                "Contrast softening",
                0.0,
                100.0,
                28.0,
                |e: &mut Self, v| e.contrast_softening = v,
            ),
            float_slider::<Self>("warmth", "Warmth", 0.0, 100.0, 22.0, |e: &mut Self, v| {
                e.warmth = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let amount = clampf(self.amount, 0.0, 100.0) / 100.0;
        let radius = clampf(self.radius, 0.0, 30.0);
        let bloom = clampf(self.highlight_bloom, 0.0, 100.0) / 100.0;
        let softness = clampf(self.contrast_softening, 0.0, 100.0) / 100.0;
        let warmth = clampf(self.warmth, 0.0, 100.0) / 100.0;
        if amount <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let blur_px = pixels(&create_blurred_clamp(source, 0.1f32.max(radius)));
        let post = 1.0 - (softness * 0.28);
        let dst: Vec<SkColor> = (0..src.len())
            .map(|i| {
                let (s, bl) = (src[i], blur_px[i]);
                let (sr, sg, sb) = (s.r as f32 / 255.0, s.g as f32 / 255.0, s.b as f32 / 255.0);
                let (br, bg, bb) = (
                    bl.r as f32 / 255.0,
                    bl.g as f32 / 255.0,
                    bl.b as f32 / 255.0,
                );
                let bl_lum = analog_luminance01(bl);
                let mask = 0.45 + (bloom * bl_lum.powf(1.7) * 0.85);
                let dr = screen(sr, br * mask * (1.02 + (warmth * 0.08)));
                let dg = screen(sg, bg * mask * 0.96);
                let db = screen(sb, bb * mask * (0.92 - (warmth * 0.06)));
                let mut r = apply_contrast(lerp(sr, dr, amount), post);
                let mut g = apply_contrast(lerp(sg, dg, amount), post);
                let mut b = apply_contrast(lerp(sb, db, amount), post);
                if warmth > 0.0001 {
                    let wm = warmth * bl_lum.powf(1.35) * 0.22;
                    r = screen(r, wm);
                    g = screen(g, wm * 0.55);
                    b = lerp(b, 0f32.max(b - (wm * 0.12)), wm);
                }
                SkColor::rgba(
                    clamp_to_byte(r * 255.0),
                    clamp_to_byte(g * 255.0),
                    clamp_to_byte(b * 255.0),
                    s.a,
                )
            })
            .collect();
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
