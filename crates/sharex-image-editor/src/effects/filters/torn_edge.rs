//! `TornEdgeImageEffect` (`Core/ImageEffects/Filters/TornEdgeImageEffect.cs`).

use crate::effects::prelude::*;

/// `TornEdgeImageEffect`.
#[derive(Clone, Debug)]
pub struct TornEdgeImageEffect {
    pub depth: i32,
    pub range: i32,
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
    pub curved: bool,
}

impl Default for TornEdgeImageEffect {
    fn default() -> Self {
        TornEdgeImageEffect {
            depth: 20,
            range: 20,
            top: true,
            right: true,
            bottom: true,
            left: true,
            curved: false,
        }
    }
}

impl EffectImpl for TornEdgeImageEffect {
    fn info() -> Info {
        Info::new("torn_edge", "Torn edge", EffectCategory::Filters, "image-effect/torn-edge")
            .desc("Adds a torn edge border effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("depth", "Depth", 1, 100, 20, |e: &mut Self, v| e.depth = v).snap(false),
            int_slider::<Self>("range", "Range", 1, 100, 20, |e: &mut Self, v| e.range = v).snap(false),
            bool_param::<Self>("top", "Top", true, |e: &mut Self, v| e.top = v),
            bool_param::<Self>("right", "Right", true, |e: &mut Self, v| e.right = v),
            bool_param::<Self>("bottom", "Bottom", true, |e: &mut Self, v| e.bottom = v),
            bool_param::<Self>("left", "Left", true, |e: &mut Self, v| e.left = v),
            bool_param::<Self>("curved", "Curved edges", false, |e: &mut Self, v| e.curved = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
