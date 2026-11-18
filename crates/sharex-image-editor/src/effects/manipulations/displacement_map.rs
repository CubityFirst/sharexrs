//! `DisplacementMapImageEffect` (`Core/ImageEffects/Manipulations/DisplacementMapImageEffect.cs`).

use crate::effects::prelude::*;

/// `DisplacementMapImageEffect`.
#[derive(Clone, Debug)]
pub struct DisplacementMapImageEffect {
    pub map_file_path: String,
    pub amount_x: f32,
    pub amount_y: f32,
}

impl Default for DisplacementMapImageEffect {
    fn default() -> Self {
        DisplacementMapImageEffect {
            map_file_path: "".to_string(),
            amount_x: 20.0,
            amount_y: 20.0,
        }
    }
}

impl EffectImpl for DisplacementMapImageEffect {
    fn info() -> Info {
        Info::new("displacement_map", "Displacement map", EffectCategory::Manipulations, "image-effect/map")
            .desc("Displaces pixels using a map image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            file_param::<Self>("map_file_path", "Map file path", "", |e: &mut Self, v| e.map_file_path = v),
            float_slider::<Self>("amount_x", "Amount X", -200.0, 200.0, 20.0, |e: &mut Self, v| e.amount_x = v),
            float_slider::<Self>("amount_y", "Amount Y", -200.0, 200.0, 20.0, |e: &mut Self, v| e.amount_y = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
