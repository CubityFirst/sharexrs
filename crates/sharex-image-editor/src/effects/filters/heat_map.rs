//! `HeatMapImageEffect` (`Core/ImageEffects/Filters/HeatMapImageEffect.cs`).

use crate::effects::prelude::*;

/// `HeatMapImageEffect`.
#[derive(Clone, Debug)]
pub struct HeatMapImageEffect {}

impl Default for HeatMapImageEffect {
    fn default() -> Self {
        HeatMapImageEffect {}
    }
}

impl EffectImpl for HeatMapImageEffect {
    fn info() -> Info {
        Info::new(
            "heat_map",
            "Heat map",
            EffectCategory::Filters,
            "image-effect/flame",
        )
        .desc("Maps image luminance to a fire-like color gradient.")
        .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let dst: Vec<SkColor> = pixels(source)
            .iter()
            .map(|c| {
                let t = (0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32) / 255.0;
                let (r, g, b): (u8, u8, u8) = if t < 0.2 {
                    (0, 0, ((t / 0.2) * 255.0) as u8)
                } else if t < 0.4 {
                    (0, (((t - 0.2) / 0.2) * 255.0) as u8, 255)
                } else if t < 0.6 {
                    (0, 255, ((1.0 - (t - 0.4) / 0.2) * 255.0) as u8)
                } else if t < 0.8 {
                    ((((t - 0.6) / 0.2) * 255.0) as u8, 255, 0)
                } else {
                    let s = (t - 0.8) / 0.2;
                    (255, ((1.0 - s) * 255.0) as u8, (s * 255.0) as u8)
                };
                SkColor::rgba(r, g, b, c.a)
            })
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}
