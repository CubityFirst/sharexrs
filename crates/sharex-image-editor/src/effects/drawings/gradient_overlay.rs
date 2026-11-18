//! `GradientOverlayImageEffect` (`Core/ImageEffects/Drawings/GradientOverlayImageEffect.cs`).

use crate::effects::prelude::*;

/// `GradientOverlayImageEffect`.
#[derive(Clone, Debug)]
pub struct GradientOverlayImageEffect {
    pub color1: SkColor,
    pub color2: SkColor,
    pub angle: i32,
    pub opacity: f32,
}

impl Default for GradientOverlayImageEffect {
    fn default() -> Self {
        GradientOverlayImageEffect {
            color1: SkColor::argb(0xFFFF0080),
            color2: SkColor::argb(0xFF0080FF),
            angle: 0,
            opacity: 50.0,
        }
    }
}

impl EffectImpl for GradientOverlayImageEffect {
    fn info() -> Info {
        Info::new("gradient_overlay", "Gradient overlay", EffectCategory::Drawings, "image-effect/paint-brush")
            .desc("Blends a two-color gradient overlay on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            color_param::<Self>("color1", "Start color", SkColor::argb(0xFFFF0080), |e: &mut Self, v| e.color1 = v),
            color_param::<Self>("color2", "End color", SkColor::argb(0xFF0080FF), |e: &mut Self, v| e.color2 = v),
            int_slider::<Self>("angle", "Angle", 0, 360, 0, |e: &mut Self, v| e.angle = v),
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 50.0, |e: &mut Self, v| e.opacity = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
