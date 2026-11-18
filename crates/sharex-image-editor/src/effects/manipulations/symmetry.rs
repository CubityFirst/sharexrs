//! `SymmetryImageEffect` (`Core/ImageEffects/Manipulations/SymmetryImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymmetryDirection {
    LeftToRight,
    RightToLeft,
    TopToBottom,
    BottomToTop,
}

/// `SymmetryImageEffect`.
#[derive(Clone, Debug)]
pub struct SymmetryImageEffect {
    pub direction: SymmetryDirection,
}

impl Default for SymmetryImageEffect {
    fn default() -> Self {
        SymmetryImageEffect {
            direction: SymmetryDirection::LeftToRight,
        }
    }
}

impl EffectImpl for SymmetryImageEffect {
    fn info() -> Info {
        Info::new("symmetry", "Symmetry", EffectCategory::Manipulations, "image-effect/flip-horizontal")
            .desc("Creates bilateral symmetry by mirroring one half of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, SymmetryDirection>("direction", "Direction", SymmetryDirection::LeftToRight, |e: &mut Self, v| e.direction = v, &[("Left → Right", SymmetryDirection::LeftToRight), ("Right → Left", SymmetryDirection::RightToLeft), ("Top → Bottom", SymmetryDirection::TopToBottom), ("Bottom → Top", SymmetryDirection::BottomToTop)]),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
