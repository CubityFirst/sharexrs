//! `PixelateImageEffect` (`Core/ImageEffects/Filters/PixelateImageEffect.cs`).

use crate::effects::prelude::*;

/// `PixelateImageEffect`.
#[derive(Clone, Debug)]
pub struct PixelateImageEffect {
    pub size: i32,
}

impl Default for PixelateImageEffect {
    fn default() -> Self {
        PixelateImageEffect { size: 10 }
    }
}

impl EffectImpl for PixelateImageEffect {
    fn info() -> Info {
        Info::new("pixelate", "Pixelate", EffectCategory::Filters, "app/split")
            .desc("Pixelates the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![int_slider::<Self>(
            "size",
            "Size",
            1,
            200,
            10,
            |e: &mut Self, v| e.size = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.size <= 1 {
            return Ok(source.clone());
        }
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        let small_w = 1.max(sw / self.size);
        let small_h = 1.max(sh / self.size);
        let mut small = Bitmap::new(small_w as u32, small_h as u32);
        Canvas::new(&mut small).draw_image_rect(
            source,
            None,
            RRect::ltrb(0.0, 0.0, small_w as f32, small_h as f32),
            Sampling::Nearest,
            None,
        );
        let mut out = Bitmap::new(source.width(), source.height());
        Canvas::new(&mut out).draw_image_rect(
            &small,
            None,
            RRect::ltrb(0.0, 0.0, sw as f32, sh as f32),
            Sampling::Nearest,
            None,
        );
        Ok(out)
    }
}
