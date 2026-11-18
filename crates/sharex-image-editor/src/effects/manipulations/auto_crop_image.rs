//! `AutoCropImageEffect` (`Core/ImageEffects/Manipulations/AutoCropImageEffect.cs`).

use crate::effects::prelude::*;

/// `AutoCropImageEffect`.
#[derive(Clone, Debug)]
pub struct AutoCropImageEffect {
    pub tolerance: i32,
    /// The colour to crop away; `SKColors.Transparent` means the top-left pixel's.
    pub color: SkColor,
}

impl Default for AutoCropImageEffect {
    fn default() -> Self {
        AutoCropImageEffect {
            tolerance: 0,
            color: color::TRANSPARENT,
        }
    }
}

impl EffectImpl for AutoCropImageEffect {
    fn info() -> Info {
        Info::new(
            "auto_crop_image",
            "Auto crop image",
            EffectCategory::Manipulations,
            "app/region",
        )
        .desc("Automatically crops the image using tolerance on edge pixels.")
    }

    fn params() -> Vec<ParamDef> {
        vec![int_slider::<Self>(
            "tolerance",
            "Tolerance",
            0,
            255,
            0,
            |e: &mut Self, v| e.tolerance = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width(), source.height());
        let match_color = if self.color == color::TRANSPARENT && width > 0 && height > 0 {
            get_pixel(source, 0, 0)
        } else {
            self.color
        };
        let tol = self.tolerance;
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (width as i32, height as i32, -1, -1);
        for y in 0..height {
            for x in 0..width {
                let c = get_pixel(source, x, y);
                let m = |a: u8, b: u8| (a as i32 - b as i32).abs() <= tol;
                if !(m(c.r, match_color.r)
                    && m(c.g, match_color.g)
                    && m(c.b, match_color.b)
                    && m(c.a, match_color.a))
                {
                    min_x = min_x.min(x as i32);
                    min_y = min_y.min(y as i32);
                    max_x = max_x.max(x as i32);
                    max_y = max_y.max(y as i32);
                }
            }
        }
        if max_x < 0 {
            // CropToContentBounds(null): a new 1x1 bitmap.
            return Ok(Bitmap::new(1, 1));
        }
        Ok(crop_copy(
            source,
            min_x,
            min_y,
            max_x + 1 - min_x,
            max_y + 1 - min_y,
        ))
    }
}
