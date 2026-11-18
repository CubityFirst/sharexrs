//! `PopArtGridImageEffect` (`Core/ImageEffects/Filters/PopArtGridImageEffect.cs`).

use crate::effects::prelude::*;

/// `PopArtGridImageEffect`.
#[derive(Clone, Debug)]
pub struct PopArtGridImageEffect {
    pub color1: SkColor,
    pub color2: SkColor,
    pub color3: SkColor,
    pub color4: SkColor,
    pub strength: f32,
}

impl Default for PopArtGridImageEffect {
    fn default() -> Self {
        PopArtGridImageEffect {
            color1: SkColor::argb(0xFFFF3232),
            color2: SkColor::argb(0xFF32C832),
            color3: SkColor::argb(0xFF3232FF),
            color4: SkColor::argb(0xFFFFC832),
            strength: 60.0,
        }
    }
}

impl EffectImpl for PopArtGridImageEffect {
    fn info() -> Info {
        Info::new("pop_art_grid", "Pop art grid", EffectCategory::Filters, "app/split")
            .desc("Creates an Andy Warhol-style 2×2 pop art grid with color variations.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            color_param::<Self>("color1", "Color 1", SkColor::argb(0xFFFF3232), |e: &mut Self, v| e.color1 = v),
            color_param::<Self>("color2", "Color 2", SkColor::argb(0xFF32C832), |e: &mut Self, v| e.color2 = v),
            color_param::<Self>("color3", "Color 3", SkColor::argb(0xFF3232FF), |e: &mut Self, v| e.color3 = v),
            color_param::<Self>("color4", "Color 4", SkColor::argb(0xFFFFC832), |e: &mut Self, v| e.color4 = v),
            float_slider::<Self>("strength", "Color strength", 0.0, 100.0, 60.0, |e: &mut Self, v| e.strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
