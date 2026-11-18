//! `FlipImageEffect` (`Core/ImageEffects/Manipulations/FlipImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlipDirection {
    Horizontal,
    Vertical,
}

/// `FlipImageEffect`.
#[derive(Clone, Debug)]
pub struct FlipImageEffect {
    pub horizontally: bool,
    pub vertically: bool,
}

impl Default for FlipImageEffect {
    fn default() -> Self {
        FlipImageEffect {
            horizontally: false,
            vertically: false,
        }
    }
}

impl EffectImpl for FlipImageEffect {
    fn info() -> Info {
        Info::new("flip", "Flip", EffectCategory::Manipulations, "image-effect/flip-horizontal")
            .desc("Flips the image horizontally or vertically.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            bool_param::<Self>("horizontally", "Horizontally", false, |e: &mut Self, v| e.horizontally = v),
            bool_param::<Self>("vertically", "Vertically", false, |e: &mut Self, v| e.vertically = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
