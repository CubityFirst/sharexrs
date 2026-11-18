//! `ColorHalftoneImageEffect` (`Core/ImageEffects/Filters/ColorHalftoneImageEffect.cs`).

use crate::effects::prelude::*;

/// `ColorHalftoneImageEffect`.
#[derive(Clone, Debug)]
pub struct ColorHalftoneImageEffect {
    pub dot_size: i32,
}

impl Default for ColorHalftoneImageEffect {
    fn default() -> Self {
        ColorHalftoneImageEffect {
            dot_size: 8,
        }
    }
}

impl EffectImpl for ColorHalftoneImageEffect {
    fn info() -> Info {
        Info::new("color_halftone", "Color halftone", EffectCategory::Filters, "image-effect/circle-dot")
            .desc("Simulates CMYK color halftone printing with rotated dot grids.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Dot size", 4, 30, 8, |e: &mut Self, v| e.dot_size = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let size = 4.max(self.dot_size);
        let (w, h) = (source.width() as i32, source.height() as i32);
        let mut out = Bitmap::new(source.width(), source.height());
        let src = pixels(source);
        let mut canvas = Canvas::new(&mut out);
        canvas.clear([255, 255, 255, 255]);
        let channels = [
            (SkColor::rgb(0, 255, 255), 15f32),
            (SkColor::rgb(255, 0, 255), 75.0),
            (SkColor::rgb(255, 255, 0), 0.0),
            (SkColor::rgb(0, 0, 0), 45.0),
        ];
        for (color, _angle) in channels {
            let mut paint = solid(color, true);
            paint.blend = Blend::Multiply;
            let mut gy = -size;
            while gy < h + size * 2 {
                let mut gx = -size;
                while gx < w + size * 2 {
                    let sx = gx.clamp(0, w - 1);
                    let sy = gy.clamp(0, h - 1);
                    let sc = src[(sy * w + sx) as usize];
                    let v = if color.r == 0 && color.g == 0 && color.b == 0 {
                        1.0 - (0.2126 * sc.r as f32 + 0.7152 * sc.g as f32 + 0.0722 * sc.b as f32) / 255.0
                    } else if color.r == 0 {
                        1.0 - sc.r as f32 / 255.0
                    } else if color.g == 0 {
                        1.0 - sc.g as f32 / 255.0
                    } else {
                        1.0 - sc.b as f32 / 255.0
                    };
                    let radius = size as f32 * 0.5 * v;
                    if radius > 0.5 {
                        sk::draw_circle(&mut canvas, gx as f32, gy as f32, radius, &paint, None);
                    }
                    gx += size;
                }
                gy += size;
            }
        }
        drop(canvas);
        Ok(out)
    }
}
