//! `AutoCropImageEffect` (`Core/ImageEffects/Manipulations/AutoCropImageEffect.cs`).

use crate::effects::prelude::*;

/// `AutoCropImageEffect`.
#[derive(Clone, Debug)]
pub struct AutoCropImageEffect {
    pub tolerance: i32,
}

impl Default for AutoCropImageEffect {
    fn default() -> Self {
        AutoCropImageEffect {
            tolerance: 0,
        }
    }
}

impl EffectImpl for AutoCropImageEffect {
    fn info() -> Info {
        Info::new("auto_crop_image", "Auto crop image", EffectCategory::Manipulations, "app/region")
            .desc("Automatically crops the image using tolerance on edge pixels.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("tolerance", "Tolerance", 0, 255, 0, |e: &mut Self, v| e.tolerance = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
