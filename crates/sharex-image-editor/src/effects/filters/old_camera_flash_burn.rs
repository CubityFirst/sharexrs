//! `OldCameraFlashBurnImageEffect` (`Core/ImageEffects/Filters/OldCameraFlashBurnImageEffect.cs`).

use crate::effects::prelude::*;

/// `OldCameraFlashBurnImageEffect`.
#[derive(Clone, Debug)]
pub struct OldCameraFlashBurnImageEffect {
    pub flash_strength: f32,
    pub flash_radius: f32,
    pub edge_burn: f32,
    pub warmth: f32,
    pub grain: f32,
    pub center_x: f32,
    pub center_y: f32,
}

impl Default for OldCameraFlashBurnImageEffect {
    fn default() -> Self {
        OldCameraFlashBurnImageEffect {
            flash_strength: 70.0,
            flash_radius: 68.0,
            edge_burn: 45.0,
            warmth: 35.0,
            grain: 20.0,
            center_x: 50.0,
            center_y: 50.0,
        }
    }
}

impl EffectImpl for OldCameraFlashBurnImageEffect {
    fn info() -> Info {
        Info::new("old_camera_flash_burn", "Old camera flash burn", EffectCategory::Filters, "image-effect/flashlight")
            .desc("Simulates an old camera flash burn with warm glow and edge burn effects.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("flash_strength", "Flash strength", 0.0, 100.0, 70.0, |e: &mut Self, v| e.flash_strength = v),
            float_slider::<Self>("flash_radius", "Flash radius", 20.0, 100.0, 68.0, |e: &mut Self, v| e.flash_radius = v),
            float_slider::<Self>("edge_burn", "Edge burn", 0.0, 100.0, 45.0, |e: &mut Self, v| e.edge_burn = v),
            float_slider::<Self>("warmth", "Warmth", 0.0, 100.0, 35.0, |e: &mut Self, v| e.warmth = v),
            float_slider::<Self>("grain", "Grain", 0.0, 100.0, 20.0, |e: &mut Self, v| e.grain = v),
            float_slider::<Self>("center_x", "Center X", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x = v),
            float_slider::<Self>("center_y", "Center Y", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
