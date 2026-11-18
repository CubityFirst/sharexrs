//! `FoldCreaseWarpImageEffect` (`Core/ImageEffects/Manipulations/FoldCreaseWarpImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoldCreaseOrientation {
    Vertical,
    Horizontal,
}

/// `FoldCreaseWarpImageEffect`.
#[derive(Clone, Debug)]
pub struct FoldCreaseWarpImageEffect {
    pub orientation: FoldCreaseOrientation,
    pub fold_count: i32,
    pub fold_depth: f32,
    pub shadow_strength: f32,
}

impl Default for FoldCreaseWarpImageEffect {
    fn default() -> Self {
        FoldCreaseWarpImageEffect {
            orientation: FoldCreaseOrientation::Vertical,
            fold_count: 3,
            fold_depth: 30.0,
            shadow_strength: 40.0,
        }
    }
}

impl EffectImpl for FoldCreaseWarpImageEffect {
    fn info() -> Info {
        Info::new("fold_crease_warp", "Fold crease warp", EffectCategory::Manipulations, "image-effect/newspaper")
            .desc("Simulates a folded paper crease distortion.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, FoldCreaseOrientation>("orientation", "Orientation", FoldCreaseOrientation::Vertical, |e: &mut Self, v| e.orientation = v, &[("Vertical", FoldCreaseOrientation::Vertical), ("Horizontal", FoldCreaseOrientation::Horizontal)]),
            int_slider::<Self>("fold_count", "Fold count", 1, 10, 3, |e: &mut Self, v| e.fold_count = v),
            float_slider::<Self>("fold_depth", "Fold depth", 0.0, 100.0, 30.0, |e: &mut Self, v| e.fold_depth = v),
            float_slider::<Self>("shadow_strength", "Shadow strength", 0.0, 100.0, 40.0, |e: &mut Self, v| e.shadow_strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
