//! `DrawCheckerboardEffect` (`Core/ImageEffects/Drawings/DrawCheckerboardEffect.cs`).

use crate::effects::prelude::*;

/// `DrawCheckerboardEffect`.
#[derive(Clone, Debug)]
pub struct DrawCheckerboardEffect {
    /// `Math.Max(1, value)` when set.
    pub size: i32,
    pub color: SkColor,
    pub color2: SkColor,
}

impl Default for DrawCheckerboardEffect {
    fn default() -> Self {
        DrawCheckerboardEffect {
            size: 10,
            color: SkColor::argb(0xFFD3D3D3),
            color2: SkColor::argb(0xFFFFFFFF),
        }
    }
}

impl EffectImpl for DrawCheckerboardEffect {
    fn info() -> Info {
        Info::new("draw_checkerboard", "Checkerboard", EffectCategory::Drawings, "image-effect/checkerboard")
            .desc("Draws a checkerboard pattern behind the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("size", "Size", 1, 100, 10, |e: &mut Self, v| e.size = v.max(1)),
            color_param::<Self>("color", "Color", SkColor::argb(0xFFD3D3D3), |e: &mut Self, v| e.color = v),
            color_param::<Self>("color2", "Color 2", SkColor::argb(0xFFFFFFFF), |e: &mut Self, v| e.color2 = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
