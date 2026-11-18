//! `RetroSunsetImageEffect` (`Core/ImageEffects/Filters/RetroSunsetImageEffect.cs`).

use crate::effects::prelude::*;

/// `RetroSunsetImageEffect`.
#[derive(Clone, Debug)]
pub struct RetroSunsetImageEffect {
    pub strength: f32,
    pub horizontal_lines: bool,
}

impl Default for RetroSunsetImageEffect {
    fn default() -> Self {
        RetroSunsetImageEffect {
            strength: 50.0,
            horizontal_lines: true,
        }
    }
}

impl EffectImpl for RetroSunsetImageEffect {
    fn info() -> Info {
        Info::new(
            "retro_sunset",
            "Retro sunset",
            EffectCategory::Filters,
            "image-effect/sunset",
        )
        .desc("Overlays retro 80s-style sunset gradient bands.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "strength",
                "Strength",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.strength = v,
            ),
            bool_param::<Self>(
                "horizontal_lines",
                "Horizontal lines",
                true,
                |e: &mut Self, v| e.horizontal_lines = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let alpha = clampf(self.strength / 100.0, 0.0, 1.0);
        if alpha <= 0.0 {
            return Ok(source.clone());
        }
        let (w, h) = (source.width() as i32, source.height() as i32);
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        let ga = (200.0 * alpha) as u8;
        let colors = [
            SkColor::rgba(120, 0, 180, ga),
            SkColor::rgba(220, 20, 80, ga),
            SkColor::rgba(255, 100, 0, ga),
            SkColor::rgba(255, 200, 0, ga),
        ];
        let mut paint = sk::linear_gradient(
            (0.0, 0.0),
            (0.0, h as f32),
            &colors,
            Some(&[0.0, 0.33, 0.66, 1.0]),
            false,
        );
        paint.blend = Blend::Overlay;
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, w as f32, h as f32, &paint, None);
        if self.horizontal_lines {
            let lp = solid(SkColor::rgba(0, 0, 0, (60.0 * alpha) as u8), false);
            let pen = Pen::new(1.0);
            let spacing = 3.max(h / 80);
            let mut y = 0;
            while y < h {
                sk::draw_line(&mut canvas, 0.0, y as f32, w as f32, y as f32, &pen, &lp);
                y += spacing;
            }
        }
        drop(canvas);
        Ok(out)
    }
}
