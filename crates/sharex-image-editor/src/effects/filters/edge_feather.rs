//! `EdgeFeatherImageEffect` (`Core/ImageEffects/Filters/EdgeFeatherImageEffect.cs`).

use crate::effects::prelude::*;

/// `EdgeFeatherImageEffect`.
#[derive(Clone, Debug)]
pub struct EdgeFeatherImageEffect {
    pub radius: f32,
}

impl Default for EdgeFeatherImageEffect {
    fn default() -> Self {
        EdgeFeatherImageEffect {
            radius: 12.0,
        }
    }
}

impl EffectImpl for EdgeFeatherImageEffect {
    fn info() -> Info {
        Info::new("edge_feather", "Edge feather", EffectCategory::Filters, "app/circle-dashed")
            .desc("Softens the edges of non-transparent regions with a feathered alpha falloff.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("radius", "Radius", 0.0, 100.0, 12.0, |e: &mut Self, v| e.radius = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
