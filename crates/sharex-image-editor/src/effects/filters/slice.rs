//! `SliceImageEffect` (`Core/ImageEffects/Filters/SliceImageEffect.cs`).

use crate::effects::prelude::*;

/// `SliceImageEffect`.
#[derive(Clone, Debug)]
pub struct SliceImageEffect {
    pub min_height: i32,
    pub max_height: i32,
    pub min_shift: i32,
    pub max_shift: i32,
}

impl Default for SliceImageEffect {
    fn default() -> Self {
        SliceImageEffect {
            min_height: 10,
            max_height: 100,
            min_shift: 0,
            max_shift: 10,
        }
    }
}

impl EffectImpl for SliceImageEffect {
    fn info() -> Info {
        Info::new("slice", "Slice", EffectCategory::Filters, "image-effect/slice")
            .desc("Slices the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("min_height", "Min Height", 1, 100, 10, |e: &mut Self, v| e.min_height = v).snap(false),
            int_slider::<Self>("max_height", "Max Height", 1, 200, 100, |e: &mut Self, v| e.max_height = v).snap(false),
            int_slider::<Self>("min_shift", "Min Shift", 0, 100, 0, |e: &mut Self, v| e.min_shift = v).snap(false),
            int_slider::<Self>("max_shift", "Max Shift", 0, 100, 10, |e: &mut Self, v| e.max_shift = v).snap(false),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
