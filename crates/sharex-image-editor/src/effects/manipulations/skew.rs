//! `SkewImageEffect` (`Core/ImageEffects/Manipulations/SkewImageEffect.cs`).

use crate::effects::prelude::*;

/// `SkewImageEffect`.
#[derive(Clone, Debug)]
pub struct SkewImageEffect {
    pub horizontally: f32,
    pub vertically: f32,
    pub auto_resize: bool,
}

impl Default for SkewImageEffect {
    fn default() -> Self {
        SkewImageEffect {
            horizontally: 0.0,
            vertically: 0.0,
            auto_resize: true,
        }
    }
}

impl EffectImpl for SkewImageEffect {
    fn info() -> Info {
        Info::new("skew", "Skew", EffectCategory::Manipulations, "image-effect/move-diagonal")
            .desc("Skews the image horizontally and vertically.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("horizontally", "Horizontally", -89.0, 89.0, 0.0, |e: &mut Self, v| e.horizontally = v),
            float_slider::<Self>("vertically", "Vertically", -89.0, 89.0, 0.0, |e: &mut Self, v| e.vertically = v),
            bool_param::<Self>("auto_resize", "Auto resize", true, |e: &mut Self, v| e.auto_resize = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
