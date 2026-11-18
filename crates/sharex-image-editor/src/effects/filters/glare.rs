//! `GlareImageEffect` (`Core/ImageEffects/Filters/GlareImageEffect.cs`).

use crate::effects::prelude::*;

/// `GlareImageEffect`.
#[derive(Clone, Debug)]
pub struct GlareImageEffect {
    pub position_x: f32,
    pub position_y: f32,
    pub radius: f32,
    pub intensity: f32,
    pub ray_count: i32,
    pub ray_length: f32,
    pub color: SkColor,
}

impl Default for GlareImageEffect {
    fn default() -> Self {
        GlareImageEffect {
            position_x: 50.0,
            position_y: 30.0,
            radius: 30.0,
            intensity: 80.0,
            ray_count: 6,
            ray_length: 50.0,
            color: SkColor::argb(0xFFFFFFFF),
        }
    }
}

impl EffectImpl for GlareImageEffect {
    fn info() -> Info {
        Info::new("glare", "Glare", EffectCategory::Filters, "image-effect/sparkle")
            .desc("Adds a bright glare/light flare at a specified position.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("position_x", "Position X (%)", 0.0, 100.0, 50.0, |e: &mut Self, v| e.position_x = v),
            float_slider::<Self>("position_y", "Position Y (%)", 0.0, 100.0, 30.0, |e: &mut Self, v| e.position_y = v),
            float_slider::<Self>("radius", "Radius", 5.0, 100.0, 30.0, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>("intensity", "Intensity", 0.0, 100.0, 80.0, |e: &mut Self, v| e.intensity = v),
            int_slider::<Self>("ray_count", "Ray count", 0, 12, 6, |e: &mut Self, v| e.ray_count = v),
            float_slider::<Self>("ray_length", "Ray length", 0.0, 100.0, 50.0, |e: &mut Self, v| e.ray_length = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFFFFFFFF), |e: &mut Self, v| e.color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
