//! `CropImageEffect` (`Core/ImageEffects/Manipulations/CropImageEffect.cs`).

use crate::effects::prelude::*;

/// `CropImageEffect`.
#[derive(Clone, Debug)]
pub struct CropImageEffect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Default for CropImageEffect {
    fn default() -> Self {
        CropImageEffect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }
}

impl EffectImpl for CropImageEffect {
    fn info() -> Info {
        Info::new("crop_image", "Crop image", EffectCategory::Manipulations, "app/crop")
            .desc("Crops the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("x", "X", 0, 100000, 0, |e: &mut Self, v| e.x = v),
            int_numeric::<Self>("y", "Y", 0, 100000, 0, |e: &mut Self, v| e.y = v),
            int_numeric::<Self>("width", "Width", 1, 100000, 100, |e: &mut Self, v| e.width = v),
            int_numeric::<Self>("height", "Height", 1, 100000, 100, |e: &mut Self, v| e.height = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        let x = self.x.clamp(0, sw);
        let y = self.y.clamp(0, sh);
        let w = self.width.clamp(0, sw - x);
        let h = self.height.clamp(0, sh - y);
        if w <= 0 || h <= 0 {
            return Ok(source.clone());
        }
        Ok(crop_copy(source, x, y, w, h))
    }
}
