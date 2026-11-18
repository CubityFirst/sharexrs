//! `VignetteImageEffect` (`Core/ImageEffects/Filters/VignetteImageEffect.cs`).

use crate::effects::prelude::*;

/// `VignetteImageEffect`.
#[derive(Clone, Debug)]
pub struct VignetteImageEffect {
    pub strength: f32,
    pub radius: f32,
}

impl Default for VignetteImageEffect {
    fn default() -> Self {
        VignetteImageEffect {
            strength: 0.5,
            radius: 0.75,
        }
    }
}

impl EffectImpl for VignetteImageEffect {
    fn info() -> Info {
        Info::new(
            "vignette",
            "Vignette",
            EffectCategory::Filters,
            "app/circle-dashed",
        )
        .desc("Applies a vignette effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.0, 1.0, 0.5, |e: &mut Self, v| {
                e.strength = v
            })
            .tick(0.05)
            .snap(false)
            .format("{}{0:0.00}"),
            float_slider::<Self>("radius", "Radius", 0.05, 0.999, 0.75, |e: &mut Self, v| {
                e.radius = v
            })
            .tick(0.01)
            .snap(false)
            .format("{}{0:0.00}"),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let strength = clampf(self.strength, 0.0, 1.0);
        let radius = clampf(self.radius, 0.05, 0.999);
        if strength <= 0.0 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let cx = (width - 1) as f32 * 0.5;
        let cy = (height - 1) as f32 * 0.5;
        let inv_cx = if cx > 0.0 { 1.0 / cx } else { 1.0 };
        let inv_cy = if cy > 0.0 { 1.0 / cy } else { 1.0 };
        const INV_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let dy = (y as f32 - cy) * inv_cy;
            for x in 0..width {
                let dx = (x as f32 - cx) * inv_cx;
                let d = ((dx * dx) + (dy * dy)).sqrt() * INV_SQRT2;
                let t = clampf((d - radius) / (1.0 - radius), 0.0, 1.0);
                let falloff = t * t * (3.0 - (2.0 * t));
                let f = 1.0 - (strength * falloff);
                let c = src[(y * width + x) as usize];
                dst[(y * width + x) as usize] = SkColor::rgba(
                    clamp_to_byte(c.r as f32 * f),
                    clamp_to_byte(c.g as f32 * f),
                    clamp_to_byte(c.b as f32 * f),
                    c.a,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
