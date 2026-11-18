//! `HexagonalPixelateImageEffect` (`Core/ImageEffects/Filters/HexagonalPixelateImageEffect.cs`).

use crate::effects::prelude::*;

/// `HexagonalPixelateImageEffect`.
#[derive(Clone, Debug)]
pub struct HexagonalPixelateImageEffect {
    pub size: i32,
}

impl Default for HexagonalPixelateImageEffect {
    fn default() -> Self {
        HexagonalPixelateImageEffect { size: 16 }
    }
}

impl EffectImpl for HexagonalPixelateImageEffect {
    fn info() -> Info {
        Info::new(
            "hexagonal_pixelate",
            "Hexagonal pixelate",
            EffectCategory::Filters,
            "image-effect/hexagon",
        )
        .desc("Pixelates the image using hexagonal tiles.")
    }

    fn params() -> Vec<ParamDef> {
        vec![int_slider::<Self>(
            "size",
            "Size",
            4,
            100,
            16,
            |e: &mut Self, v| e.size = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let size = 4.max(self.size);
        let (w, h) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        let sizef = size as f32;
        let hex_w = sizef * 2.0;
        let hex_h = sizef * 3f32.sqrt();
        let mut row = -1f32;
        while row * hex_h / 2.0 < h as f32 + hex_h {
            let yc = row * hex_h / 2.0;
            let x_off = if row % 2.0 == 0.0 { 0.0 } else { sizef * 1.5 };
            let mut col = -1f32;
            while col * hex_w * 0.75 < w as f32 + hex_w {
                let xc = col * sizef * 3.0 + x_off;
                let sx = (xc as i32).clamp(0, w - 1);
                let sy = (yc as i32).clamp(0, h - 1);
                let color = src[(sy * w + sx) as usize];
                let mut path = SkPath::new();
                for i in 0..6 {
                    let a = std::f32::consts::PI / 3.0 * i as f32 - std::f32::consts::PI / 6.0;
                    let (x, y) = (xc + sizef * a.cos(), yc + sizef * a.sin());
                    if i == 0 {
                        path.move_to(x, y);
                    } else {
                        path.line_to(x, y);
                    }
                }
                path.close();
                canvas.fill_sk_path(&path, &solid(color, false));
                col += 1.0;
            }
            row += 1.0;
        }
        drop(canvas);
        Ok(out)
    }
}
