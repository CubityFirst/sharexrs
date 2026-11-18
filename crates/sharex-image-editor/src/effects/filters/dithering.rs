//! `DitheringImageEffect` (`Core/ImageEffects/Filters/DitheringImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DitheringMethod {
    FloydSteinberg,
    Bayer4x4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DitheringPalette {
    OneBitBW,
    WebSafe216,
    RGB332,
    Grayscale4,
}

/// `DitheringImageEffect`.
#[derive(Clone, Debug)]
pub struct DitheringImageEffect {
    pub method: DitheringMethod,
    pub palette: DitheringPalette,
    pub serpentine: bool,
    pub strength: f32,
}

impl Default for DitheringImageEffect {
    fn default() -> Self {
        DitheringImageEffect {
            method: DitheringMethod::FloydSteinberg,
            palette: DitheringPalette::OneBitBW,
            serpentine: true,
            strength: 100.0,
        }
    }
}

impl EffectImpl for DitheringImageEffect {
    fn info() -> Info {
        Info::new("dithering", "Dithering", EffectCategory::Filters, "image-effect/checkerboard")
            .desc("Applies dithering to reduce color depth with various methods and palettes.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, DitheringMethod>("method", "Method", DitheringMethod::FloydSteinberg, |e: &mut Self, v| e.method = v, &[("Floyd-Steinberg", DitheringMethod::FloydSteinberg), ("Bayer 4x4", DitheringMethod::Bayer4x4)]),
            enum_param::<Self, DitheringPalette>("palette", "Palette", DitheringPalette::OneBitBW, |e: &mut Self, v| e.palette = v, &[("1-bit B&W", DitheringPalette::OneBitBW), ("Web safe 216", DitheringPalette::WebSafe216), ("RGB332", DitheringPalette::RGB332), ("Grayscale 4", DitheringPalette::Grayscale4)]),
            bool_param::<Self>("serpentine", "Serpentine", true, |e: &mut Self, v| e.serpentine = v),
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 100.0, |e: &mut Self, v| e.strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
