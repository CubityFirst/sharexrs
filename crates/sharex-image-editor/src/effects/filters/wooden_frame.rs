//! `WoodenFrameImageEffect` (`Core/ImageEffects/Filters/WoodenFrameImageEffect.cs`).

use crate::effects::prelude::*;

/// `WoodenFrameImageEffect`.
#[derive(Clone, Debug)]
pub struct WoodenFrameImageEffect {
    pub frame_width: i32,
    pub grain_strength: f32,
    pub bevel_strength: f32,
    pub wood_color: SkColor,
    pub seed: i32,
}

impl Default for WoodenFrameImageEffect {
    fn default() -> Self {
        WoodenFrameImageEffect {
            frame_width: 48,
            grain_strength: 60.0,
            bevel_strength: 65.0,
            wood_color: SkColor::argb(0xFF8B5E3C),
            seed: 1649,
        }
    }
}

impl EffectImpl for WoodenFrameImageEffect {
    fn info() -> Info {
        Info::new("wooden_frame", "Wooden frame", EffectCategory::Filters, "image-effect/frame")
            .desc("Adds a procedural wooden frame around the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("frame_width", "Frame width", 2, 300, 48, |e: &mut Self, v| e.frame_width = v),
            float_slider::<Self>("grain_strength", "Grain strength", 0.0, 100.0, 60.0, |e: &mut Self, v| e.grain_strength = v),
            float_slider::<Self>("bevel_strength", "Bevel strength", 0.0, 100.0, 65.0, |e: &mut Self, v| e.bevel_strength = v),
            color_param::<Self>("wood_color", "Wood color", SkColor::argb(0xFF8B5E3C), |e: &mut Self, v| e.wood_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
