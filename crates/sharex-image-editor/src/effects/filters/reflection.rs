//! `ReflectionImageEffect` (`Core/ImageEffects/Filters/ReflectionImageEffect.cs`).

use crate::effects::prelude::*;

/// `ReflectionImageEffect`.
#[derive(Clone, Debug)]
pub struct ReflectionImageEffect {
    pub percentage: i32,
    pub max_alpha: i32,
    pub min_alpha: i32,
    pub offset: i32,
    pub skew: bool,
    pub skew_size: i32,
}

impl Default for ReflectionImageEffect {
    fn default() -> Self {
        ReflectionImageEffect {
            percentage: 20,
            max_alpha: 255,
            min_alpha: 0,
            offset: 0,
            skew: false,
            skew_size: 25,
        }
    }
}

impl EffectImpl for ReflectionImageEffect {
    fn info() -> Info {
        Info::new("reflection", "Reflection", EffectCategory::Filters, "image-effect/flip-vertical")
            .desc("Adds a reflection to the bottom of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("percentage", "Percentage", 1, 100, 20, |e: &mut Self, v| e.percentage = v).snap(false).format("{}{0:0}%"),
            int_slider::<Self>("max_alpha", "Max Alpha", 0, 255, 255, |e: &mut Self, v| e.max_alpha = v).snap(false),
            int_slider::<Self>("min_alpha", "Min Alpha", 0, 255, 0, |e: &mut Self, v| e.min_alpha = v).snap(false),
            int_slider::<Self>("offset", "Offset", 0, 100, 0, |e: &mut Self, v| e.offset = v).snap(false),
            bool_param::<Self>("skew", "Skew", false, |e: &mut Self, v| e.skew = v),
            int_slider::<Self>("skew_size", "Skew Size", 1, 100, 25, |e: &mut Self, v| e.skew_size = v).snap(false),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
