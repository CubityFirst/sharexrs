//! `ColorSplashImageEffect` (`Core/ImageEffects/Filters/ColorSplashImageEffect.cs`).

use crate::effects::prelude::*;

/// `ColorSplashImageEffect`.
#[derive(Clone, Debug)]
pub struct ColorSplashImageEffect {
    pub target_hue: i32,
    pub tolerance: i32,
}

impl Default for ColorSplashImageEffect {
    fn default() -> Self {
        ColorSplashImageEffect {
            target_hue: 0,
            tolerance: 30,
        }
    }
}

impl EffectImpl for ColorSplashImageEffect {
    fn info() -> Info {
        Info::new("color_splash", "Color splash", EffectCategory::Filters, "image-effect/droplets")
            .desc("Keeps one color and desaturates the rest of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("hue", "Target hue", 0, 360, 0, |e: &mut Self, v| e.target_hue = v),
            int_slider::<Self>("tolerance", "Tolerance", 1, 180, 30, |e: &mut Self, v| e.tolerance = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
