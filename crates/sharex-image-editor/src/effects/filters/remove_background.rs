//! `RemoveBackgroundImageEffect` (`Core/ImageEffects/Filters/RemoveBackgroundImageEffect.cs`).

use crate::effects::prelude::*;

/// `RemoveBackgroundImageEffect`.
#[derive(Clone, Debug)]
pub struct RemoveBackgroundImageEffect {
    pub sensitivity: f32,
    pub center_protection: f32,
    pub edge_feather: f32,
}

impl Default for RemoveBackgroundImageEffect {
    fn default() -> Self {
        RemoveBackgroundImageEffect {
            sensitivity: 60.0,
            center_protection: 65.0,
            edge_feather: 4.0,
        }
    }
}

impl EffectImpl for RemoveBackgroundImageEffect {
    fn info() -> Info {
        Info::new("remove_background", "Remove background", EffectCategory::Manipulations, "app/scissors")
            .desc("Removes border-connected background colors and turns them transparent.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("sensitivity", "Sensitivity (%)", 0.0, 100.0, 60.0, |e: &mut Self, v| e.sensitivity = v),
            float_slider::<Self>("center_protection", "Center protection (%)", 0.0, 100.0, 65.0, |e: &mut Self, v| e.center_protection = v),
            float_slider::<Self>("edge_feather", "Edge feather (px)", 0.0, 24.0, 4.0, |e: &mut Self, v| e.edge_feather = v).tick(0.5).snap(false).format("{}{0:0.#}"),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
