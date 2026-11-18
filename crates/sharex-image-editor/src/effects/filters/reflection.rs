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
        Info::new(
            "reflection",
            "Reflection",
            EffectCategory::Filters,
            "image-effect/flip-vertical",
        )
        .desc("Adds a reflection to the bottom of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("percentage", "Percentage", 1, 100, 20, |e: &mut Self, v| {
                e.percentage = v
            })
            .snap(false)
            .format("{}{0:0}%"),
            int_slider::<Self>("max_alpha", "Max Alpha", 0, 255, 255, |e: &mut Self, v| {
                e.max_alpha = v
            })
            .snap(false),
            int_slider::<Self>("min_alpha", "Min Alpha", 0, 255, 0, |e: &mut Self, v| {
                e.min_alpha = v
            })
            .snap(false),
            int_slider::<Self>("offset", "Offset", 0, 100, 0, |e: &mut Self, v| {
                e.offset = v
            })
            .snap(false),
            bool_param::<Self>("skew", "Skew", false, |e: &mut Self, v| e.skew = v),
            int_slider::<Self>("skew_size", "Skew Size", 1, 100, 25, |e: &mut Self, v| {
                e.skew_size = v
            })
            .snap(false),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.percentage <= 0 {
            return Ok(source.clone());
        }
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        let rh = ((sh * self.percentage) as f32 / 100.0) as i32;
        let nh = sh + self.offset + rh;
        let mut nw = sw;
        if self.skew && self.skew_size > 0 {
            nw += (rh as f32 * (self.skew_size as f32 / 100.0)) as i32;
        }
        if rh <= 0 {
            // DrawBitmap(flipped): SKImage.FromBitmap of an empty bitmap is null.
            return throw("Value cannot be null. (Parameter 'image')");
        }
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        canvas.draw_bitmap(source, 0.0, 0.0, None);
        let mut flipped = Bitmap::new(sw as u32, rh.max(0) as u32);
        {
            let mut rc = Canvas::new(&mut flipped);
            rc.translate(0.0, sh as f32);
            rc.scale(1.0, -1.0);
            rc.draw_bitmap(source, 0.0, 0.0, None);
        }
        let mut gp = sk::linear_gradient(
            (0.0, 0.0),
            (0.0, rh as f32),
            &[
                SkColor::rgba(255, 255, 255, self.max_alpha as u8),
                SkColor::rgba(255, 255, 255, self.min_alpha as u8),
            ],
            None,
            false,
        );
        gp.blend = Blend::DstIn;
        let mut reflection = Bitmap::new(sw as u32, rh.max(0) as u32);
        {
            let mut mc = Canvas::new(&mut reflection);
            mc.draw_bitmap(&flipped, 0.0, 0.0, None);
            sk::draw_rect(
                &mut mc,
                RRect::ltrb(0.0, 0.0, sw as f32, rh as f32),
                &gp,
                None,
            );
        }
        if self.skew && self.skew_size > 0 {
            let saved = canvas.matrix;
            canvas.translate(0.0, (sh + self.offset) as f32);
            sk::skew(&mut canvas, self.skew_size as f32 / 100.0, 0.0);
            canvas.draw_bitmap(&reflection, 0.0, 0.0, None);
            canvas.matrix = saved;
        } else {
            canvas.draw_bitmap(&reflection, 0.0, (sh + self.offset) as f32, None);
        }
        drop(canvas);
        Ok(out)
    }
}
