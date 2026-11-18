//! `SnowflakesImageEffect` (`Core/ImageEffects/Drawings/SnowflakesImageEffect.cs`).

use crate::effects::prelude::*;

/// `SnowflakesImageEffect`.
#[derive(Clone, Debug)]
pub struct SnowflakesImageEffect {
    pub count: i32,
    pub min_size: i32,
    pub max_size: i32,
    pub opacity: f32,
    pub random_angle: bool,
    pub background: bool,
}

impl Default for SnowflakesImageEffect {
    fn default() -> Self {
        SnowflakesImageEffect {
            count: 80,
            min_size: 3,
            max_size: 14,
            opacity: 85.0,
            random_angle: true,
            background: false,
        }
    }
}

impl EffectImpl for SnowflakesImageEffect {
    fn info() -> Info {
        Info::new("snowflakes", "Snowflakes", EffectCategory::Drawings, "image-effect/snowflake")
            .desc("Scatters snowflakes over the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("count", "Count", 10, 500, 80, |e: &mut Self, v| e.count = v),
            int_slider::<Self>("min_size", "Min size", 2, 20, 3, |e: &mut Self, v| e.min_size = v),
            int_slider::<Self>("max_size", "Max size", 4, 60, 14, |e: &mut Self, v| e.max_size = v),
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 85.0, |e: &mut Self, v| e.opacity = v),
            bool_param::<Self>("random_angle", "Random angle", true, |e: &mut Self, v| e.random_angle = v),
            bool_param::<Self>("background", "Background", false, |e: &mut Self, v| e.background = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
