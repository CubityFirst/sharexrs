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
        Info::new(
            "slice",
            "Slice",
            EffectCategory::Filters,
            "image-effect/slice",
        )
        .desc("Slices the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("min_height", "Min Height", 1, 100, 10, |e: &mut Self, v| {
                e.min_height = v
            })
            .snap(false),
            int_slider::<Self>(
                "max_height",
                "Max Height",
                1,
                200,
                100,
                |e: &mut Self, v| e.max_height = v,
            )
            .snap(false),
            int_slider::<Self>("min_shift", "Min Shift", 0, 100, 0, |e: &mut Self, v| {
                e.min_shift = v
            })
            .snap(false),
            int_slider::<Self>("max_shift", "Max Shift", 0, 100, 10, |e: &mut Self, v| {
                e.max_shift = v
            })
            .snap(false),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.min_height <= 0 && self.max_height <= 0 {
            return Ok(source.clone());
        }
        let min_h = 1.max(self.min_height.min(self.max_height));
        let max_h = min_h.max(self.min_height.max(self.max_height));
        let min_s = self
            .min_shift
            .wrapping_abs()
            .min(self.max_shift.wrapping_abs());
        let max_s = self
            .min_shift
            .wrapping_abs()
            .max(self.max_shift.wrapping_abs());
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        let nw = sw + max_s * 2;
        let mut out = Bitmap::new(nw.max(0) as u32, sh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        let mut y = 0;
        while y < sh {
            let (slice_h, shift) = random::shared(|r| {
                let s = r.next_range(min_h, max_h + 1);
                let shift = if r.next_max(2) == 0 {
                    r.next_range(-max_s, -min_s + 1)
                } else {
                    r.next_range(min_s, max_s + 1)
                };
                (s, shift)
            });
            let slice_h = slice_h.min(sh - y);
            let src = RRect::ltrb(0.0, y as f32, sw as f32, (y + slice_h) as f32);
            let dst = RRect::ltrb(
                (max_s + shift) as f32,
                y as f32,
                (max_s + shift + sw) as f32,
                (y + slice_h) as f32,
            );
            canvas.draw_image_rect(source, Some(src), dst, Sampling::Nearest, None);
            y += slice_h;
        }
        drop(canvas);
        Ok(out)
    }
}
