//! `GhostTrailImageEffect` (`Core/ImageEffects/Filters/GhostTrailImageEffect.cs`).

use crate::effects::prelude::*;

/// `GhostTrailImageEffect`.
#[derive(Clone, Debug)]
pub struct GhostTrailImageEffect {
    pub copies: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub fade: f32,
}

impl Default for GhostTrailImageEffect {
    fn default() -> Self {
        GhostTrailImageEffect {
            copies: 3,
            offset_x: 15,
            offset_y: 5,
            fade: 50.0,
        }
    }
}

impl EffectImpl for GhostTrailImageEffect {
    fn info() -> Info {
        Info::new(
            "ghost_trail",
            "Ghost trail",
            EffectCategory::Filters,
            "image-effect/ghost",
        )
        .desc("Creates fading ghost copies offset from the original image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("copies", "Copies", 1, 8, 3, |e: &mut Self, v| e.copies = v),
            int_slider::<Self>("offset_x", "Offset X", -50, 50, 15, |e: &mut Self, v| {
                e.offset_x = v
            }),
            int_slider::<Self>("offset_y", "Offset Y", -50, 50, 5, |e: &mut Self, v| {
                e.offset_y = v
            }),
            float_slider::<Self>("fade", "Fade", 10.0, 90.0, 50.0, |e: &mut Self, v| {
                e.fade = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let copies = 1.max(self.copies);
        let (w, h) = (source.width(), source.height());
        let mut out = Bitmap::new(w, h);
        let mut canvas = Canvas::new(&mut out);
        let fade = clampf(self.fade / 100.0, 0.1, 0.9);
        for i in (1..=copies).rev() {
            let alpha = (1.0 - fade).powf(i as f32);
            let a = (255.0 * alpha) as u8;
            if a == 0 {
                // SkPaint::nothingToDraw: src-over with a zero alpha draws nothing.
                continue;
            }
            let mut p = Paint::shader(Shader::Color([255, 255, 255, 255]));
            p.alpha = a;
            canvas.draw_bitmap(
                source,
                self.offset_x.wrapping_mul(i) as f32,
                self.offset_y.wrapping_mul(i) as f32,
                Some(&p),
            );
        }
        canvas.draw_bitmap(source, 0.0, 0.0, None);
        drop(canvas);
        Ok(out)
    }
}
