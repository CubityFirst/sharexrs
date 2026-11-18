//! `ScanlineImageEffect` (`Core/ImageEffects/Filters/ScanlineImageEffect.cs`).

use crate::effects::prelude::*;

/// `ScanlineImageEffect`.
#[derive(Clone, Debug)]
pub struct ScanlineImageEffect {
    pub spacing: i32,
    pub opacity: f32,
}

impl Default for ScanlineImageEffect {
    fn default() -> Self {
        ScanlineImageEffect {
            spacing: 4,
            opacity: 50.0,
        }
    }
}

impl EffectImpl for ScanlineImageEffect {
    fn info() -> Info {
        Info::new(
            "scanline",
            "Scanlines",
            EffectCategory::Filters,
            "app/scan-line",
        )
        .desc("Adds retro CRT-style horizontal scanlines.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("spacing", "Line spacing", 2, 20, 4, |e: &mut Self, v| {
                e.spacing = v
            }),
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 50.0, |e: &mut Self, v| {
                e.opacity = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let alpha = clampf(self.opacity / 100.0, 0.0, 1.0);
        if alpha <= 0.0 {
            return Ok(source.clone());
        }
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        let paint = solid(SkColor::rgba(0, 0, 0, (255.0 * alpha) as u8), false);
        let pen = Pen::new(1.0);
        let step = 2.max(self.spacing);
        let mut y = 0;
        while y < source.height() as i32 {
            sk::draw_line(
                &mut canvas,
                0.0,
                y as f32,
                source.width() as f32,
                y as f32,
                &pen,
                &paint,
            );
            y += step;
        }
        drop(canvas);
        Ok(out)
    }
}
