//! `TrianglePixelateImageEffect` (`Core/ImageEffects/Filters/TrianglePixelateImageEffect.cs`).

use crate::effects::prelude::*;

/// `TrianglePixelateImageEffect`.
#[derive(Clone, Debug)]
pub struct TrianglePixelateImageEffect {
    pub size: i32,
}

impl Default for TrianglePixelateImageEffect {
    fn default() -> Self {
        TrianglePixelateImageEffect { size: 24 }
    }
}

impl EffectImpl for TrianglePixelateImageEffect {
    fn info() -> Info {
        Info::new(
            "triangle_pixelate",
            "Triangle pixelate",
            EffectCategory::Filters,
            "image-effect/triangle",
        )
        .desc("Pixelates the image using triangular tiles.")
    }

    fn params() -> Vec<ParamDef> {
        vec![int_slider::<Self>(
            "size",
            "Size",
            8,
            100,
            24,
            |e: &mut Self, v| e.size = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let size = 8.max(self.size);
        let (w, h) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        let sizef = size as f32;
        let half = sizef / 2.0;
        let mut row = 0;
        while (row as f32) * half < (h + size) as f32 {
            let mut col = 0;
            while col * size < w + size {
                let x = (col * size) as f32;
                let y = row as f32 * half;
                let (p1, p2, p3) = if (row + col) % 2 == 0 {
                    ((x, y), (x + sizef, y), (x + sizef / 2.0, y + half))
                } else {
                    ((x, y + half), (x + sizef, y + half), (x + sizef / 2.0, y))
                };
                let cx = (p1.0 + p2.0 + p3.0) / 3.0;
                let cy = (p1.1 + p2.1 + p3.1) / 3.0;
                let sx = (cx as i32).clamp(0, w - 1);
                let sy = (cy as i32).clamp(0, h - 1);
                let color = src[(sy * w + sx) as usize];
                let mut path = SkPath::new();
                path.move_to(p1.0, p1.1);
                path.line_to(p2.0, p2.1);
                path.line_to(p3.0, p3.1);
                path.close();
                canvas.fill_sk_path(&path, &solid(color, false));
                col += 1;
            }
            row += 1;
        }
        drop(canvas);
        Ok(out)
    }
}
