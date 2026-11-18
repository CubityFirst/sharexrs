//! `BrickWallBorderImageEffect` (`Core/ImageEffects/Filters/BrickWallBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `BrickWallBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct BrickWallBorderImageEffect {
    pub border_size: i32,
    pub brick_color: SkColor,
    pub mortar_color: SkColor,
    pub mortar_thickness: f32,
    pub color_variation: f32,
    pub bevel_strength: f32,
}

impl Default for BrickWallBorderImageEffect {
    fn default() -> Self {
        BrickWallBorderImageEffect {
            border_size: 56,
            brick_color: SkColor::argb(0xFFB25A3E),
            mortar_color: SkColor::argb(0xFFC3BCAF),
            mortar_thickness: 4.0,
            color_variation: 40.0,
            bevel_strength: 50.0,
        }
    }
}

impl EffectImpl for BrickWallBorderImageEffect {
    fn info() -> Info {
        Info::new("brick_wall_border", "Brick wall border", EffectCategory::Filters, "image-effect/brick-wall")
            .desc("Adds a procedural brick wall texture border with mortar lines, shading, and color variation.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 10, 300, 56, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("brick_color", "Brick color", SkColor::argb(0xFFB25A3E), |e: &mut Self, v| e.brick_color = v),
            color_param::<Self>("mortar_color", "Mortar color", SkColor::argb(0xFFC3BCAF), |e: &mut Self, v| e.mortar_color = v),
            float_slider::<Self>("mortar_thickness", "Mortar thickness", 1.0, 20.0, 4.0, |e: &mut Self, v| e.mortar_thickness = v),
            float_slider::<Self>("color_variation", "Color variation", 0.0, 100.0, 40.0, |e: &mut Self, v| e.color_variation = v),
            float_slider::<Self>("bevel_strength", "Bevel strength", 0.0, 100.0, 50.0, |e: &mut Self, v| e.bevel_strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
