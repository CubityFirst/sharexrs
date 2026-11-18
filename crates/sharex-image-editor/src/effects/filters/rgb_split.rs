//! `RGBSplitImageEffect` (`Core/ImageEffects/Filters/RGBSplitImageEffect.cs`).

use crate::effects::prelude::*;

/// `RGBSplitImageEffect`.
#[derive(Clone, Debug)]
pub struct RGBSplitImageEffect {
    pub offset_red_x: i32,
    pub offset_red_y: i32,
    pub offset_green_x: i32,
    pub offset_green_y: i32,
    pub offset_blue_x: i32,
    pub offset_blue_y: i32,
}

impl Default for RGBSplitImageEffect {
    fn default() -> Self {
        RGBSplitImageEffect {
            offset_red_x: -5,
            offset_red_y: 0,
            offset_green_x: 0,
            offset_green_y: 0,
            offset_blue_x: 5,
            offset_blue_y: 0,
        }
    }
}

impl EffectImpl for RGBSplitImageEffect {
    fn info() -> Info {
        Info::new("rgb_split", "RGB split", EffectCategory::Filters, "image-effect/rgb-split")
            .desc("Splits and offsets the red, green, and blue color channels.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("offset_red_x", "Offset red X", -100, 100, -5, |e: &mut Self, v| e.offset_red_x = v),
            int_slider::<Self>("offset_red_y", "Offset red Y", -100, 100, 0, |e: &mut Self, v| e.offset_red_y = v),
            int_slider::<Self>("offset_green_x", "Offset green X", -100, 100, 0, |e: &mut Self, v| e.offset_green_x = v),
            int_slider::<Self>("offset_green_y", "Offset green Y", -100, 100, 0, |e: &mut Self, v| e.offset_green_y = v),
            int_slider::<Self>("offset_blue_x", "Offset blue X", -100, 100, 5, |e: &mut Self, v| e.offset_blue_x = v),
            int_slider::<Self>("offset_blue_y", "Offset blue Y", -100, 100, 0, |e: &mut Self, v| e.offset_blue_y = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let rr = clamp_int(y - self.offset_red_y, 0, bottom) * width;
            let gr = clamp_int(y - self.offset_green_y, 0, bottom) * width;
            let br = clamp_int(y - self.offset_blue_y, 0, bottom) * width;
            for x in 0..width {
                let cr = src[(rr + clamp_int(x - self.offset_red_x, 0, right)) as usize];
                let cg = src[(gr + clamp_int(x - self.offset_green_x, 0, right)) as usize];
                let cb = src[(br + clamp_int(x - self.offset_blue_x, 0, right)) as usize];
                dst[(y * width + x) as usize] = SkColor::rgba(
                    (cr.r as i32 * cr.a as i32 / 255) as u8,
                    (cg.g as i32 * cg.a as i32 / 255) as u8,
                    (cb.b as i32 * cb.a as i32 / 255) as u8,
                    ((cr.a as i32 + cg.a as i32 + cb.a as i32) / 3) as u8,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
