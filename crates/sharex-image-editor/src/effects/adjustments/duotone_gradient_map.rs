//! `DuotoneGradientMapImageEffect` (`Core/ImageEffects/Adjustments/DuotoneGradientMapImageEffect.cs`).

use crate::effects::prelude::*;

/// `DuotoneGradientMapImageEffect`.
#[derive(Clone, Debug)]
pub struct DuotoneGradientMapImageEffect {
    pub shadow_color: SkColor,
    pub midtone_color: SkColor,
    pub highlight_color: SkColor,
    pub contrast: f32,
    pub gamma: f32,
    pub blend: f32,
}

impl Default for DuotoneGradientMapImageEffect {
    fn default() -> Self {
        DuotoneGradientMapImageEffect {
            shadow_color: SkColor::argb(0xFF181C4E),
            midtone_color: SkColor::argb(0xFFB63C84),
            highlight_color: SkColor::argb(0xFFFFE084),
            contrast: 110.0,
            gamma: 1.0,
            blend: 100.0,
        }
    }
}

impl EffectImpl for DuotoneGradientMapImageEffect {
    fn info() -> Info {
        Info::new("duotone_gradient_map", "Duotone / Gradient map", EffectCategory::Adjustments, "app/blend")
            .desc("Maps grayscale tones to a custom multi-color gradient.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            color_param::<Self>("shadow_color", "Shadow color", SkColor::argb(0xFF181C4E), |e: &mut Self, v| e.shadow_color = v),
            color_param::<Self>("midtone_color", "Midtone color", SkColor::argb(0xFFB63C84), |e: &mut Self, v| e.midtone_color = v),
            color_param::<Self>("highlight_color", "Highlight color", SkColor::argb(0xFFFFE084), |e: &mut Self, v| e.highlight_color = v),
            float_slider::<Self>("contrast", "Contrast", 50.0, 200.0, 110.0, |e: &mut Self, v| e.contrast = v),
            float_slider::<Self>("gamma", "Gamma", 0.1, 5.0, 1.0, |e: &mut Self, v| e.gamma = v).tick(0.1).snap(false).format("{}{0:0.0}"),
            float_slider::<Self>("blend", "Blend", 0.0, 100.0, 100.0, |e: &mut Self, v| e.blend = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
