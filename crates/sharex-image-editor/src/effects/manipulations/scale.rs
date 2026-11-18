//! `ScaleImageEffect` (`Core/ImageEffects/Manipulations/ScaleImageEffect.cs`).

use crate::effects::prelude::*;

/// `ScaleImageEffect`.
#[derive(Clone, Debug)]
pub struct ScaleImageEffect {
    pub width_percentage: f32,
    pub height_percentage: f32,
}

impl Default for ScaleImageEffect {
    fn default() -> Self {
        ScaleImageEffect {
            width_percentage: 100.0,
            height_percentage: 0.0,
        }
    }
}

impl EffectImpl for ScaleImageEffect {
    fn info() -> Info {
        Info::new("scale", "Scale", EffectCategory::Manipulations, "app/resize")
            .desc("Scales the image by width and height percentages.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("width_percentage", "Width %", 0.0, 500.0, 100.0, |e: &mut Self, v| e.width_percentage = v),
            float_slider::<Self>("height_percentage", "Height %", 0.0, 500.0, 0.0, |e: &mut Self, v| e.height_percentage = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.width_percentage <= 0.0 && self.height_percentage <= 0.0 {
            return Ok(source.clone());
        }
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        let mut width = ((self.width_percentage / 100.0 * sw as f32) as f64).round_ties_even() as i32;
        let mut height = ((self.height_percentage / 100.0 * sh as f32) as f64).round_ties_even() as i32;
        if width == 0 {
            width = ((height as f32 / sh as f32 * sw as f32) as f64).round_ties_even() as i32;
        } else if height == 0 {
            height = ((width as f32 / sw as f32 * sh as f32) as f64).round_ties_even() as i32;
        }
        if width <= 0 || height <= 0 {
            return Ok(source.clone());
        }
        Ok(super::resize_image::resize_catmull_rom(source, width as u32, height as u32))
    }
}
