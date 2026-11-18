//! `WaveEdgeImageEffect` (`Core/ImageEffects/Filters/WaveEdgeImageEffect.cs`).

use crate::effects::prelude::*;

/// `WaveEdgeImageEffect`.
#[derive(Clone, Debug)]
pub struct WaveEdgeImageEffect {
    pub depth: i32,
    pub range: i32,
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl Default for WaveEdgeImageEffect {
    fn default() -> Self {
        WaveEdgeImageEffect {
            depth: 15,
            range: 20,
            top: true,
            right: true,
            bottom: true,
            left: true,
        }
    }
}

impl EffectImpl for WaveEdgeImageEffect {
    fn info() -> Info {
        Info::new("wave_edge", "Wave edge", EffectCategory::Filters, "image-effect/waves")
            .desc("Adds a wavy edge to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("depth", "Depth", 1, 100, 15, |e: &mut Self, v| e.depth = v).snap(false),
            int_slider::<Self>("range", "Range", 1, 100, 20, |e: &mut Self, v| e.range = v).snap(false),
            bool_param::<Self>("top", "Top", true, |e: &mut Self, v| e.top = v),
            bool_param::<Self>("right", "Right", true, |e: &mut Self, v| e.right = v),
            bool_param::<Self>("bottom", "Bottom", true, |e: &mut Self, v| e.bottom = v),
            bool_param::<Self>("left", "Left", true, |e: &mut Self, v| e.left = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
