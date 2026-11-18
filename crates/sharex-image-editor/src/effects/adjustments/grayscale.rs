//! `GrayscaleImageEffect` (`Core/ImageEffects/Adjustments/GrayscaleImageEffect.cs`).

use crate::effects::prelude::*;

/// `GrayscaleImageEffect`.
#[derive(Clone, Debug)]
pub struct GrayscaleImageEffect {
    pub strength: f32,
}

impl Default for GrayscaleImageEffect {
    fn default() -> Self {
        GrayscaleImageEffect { strength: 100.0 }
    }
}

impl EffectImpl for GrayscaleImageEffect {
    fn info() -> Info {
        Info::new(
            "grayscale",
            "Grayscale",
            EffectCategory::Adjustments,
            "image-effect/eclipse",
        )
        .desc("Converts the image to grayscale.")
    }

    fn params() -> Vec<ParamDef> {
        vec![float_slider::<Self>(
            "strength",
            "Strength",
            0.0,
            100.0,
            100.0,
            |e: &mut Self, v| e.strength = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let strength = self.strength;
        if strength >= 100.0 {
            #[rustfmt::skip]
            let m = [
                0.2126, 0.7152, 0.0722, 0.0, 0.0,
                0.2126, 0.7152, 0.0722, 0.0, 0.0,
                0.2126, 0.7152, 0.0722, 0.0, 0.0,
                0.0, 0.0, 0.0, 1.0, 0.0,
            ];
            Ok(apply_color_matrix(source, &m))
        } else if strength <= 0.0 {
            Ok(source.clone())
        } else {
            let s = strength / 100.0;
            let inv = 1.0 - s;
            #[rustfmt::skip]
            let m = [
                0.2126 * s + inv, 0.7152 * s, 0.0722 * s, 0.0, 0.0,
                0.2126 * s, 0.7152 * s + inv, 0.0722 * s, 0.0, 0.0,
                0.2126 * s, 0.7152 * s, 0.0722 * s + inv, 0.0, 0.0,
                0.0, 0.0, 0.0, 1.0, 0.0,
            ];
            Ok(apply_color_matrix(source, &m))
        }
    }
}
