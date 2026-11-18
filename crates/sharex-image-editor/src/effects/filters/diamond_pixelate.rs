//! `DiamondPixelateImageEffect` (`Core/ImageEffects/Filters/DiamondPixelateImageEffect.cs`).

use crate::effects::prelude::*;

/// `DiamondPixelateImageEffect`.
#[derive(Clone, Debug)]
pub struct DiamondPixelateImageEffect {
    pub size: i32,
}

impl Default for DiamondPixelateImageEffect {
    fn default() -> Self {
        DiamondPixelateImageEffect { size: 16 }
    }
}

impl EffectImpl for DiamondPixelateImageEffect {
    fn info() -> Info {
        Info::new(
            "diamond_pixelate",
            "Diamond pixelate",
            EffectCategory::Filters,
            "image-effect/diamond",
        )
        .desc("Pixelates the image using diamond-shaped blocks.")
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
        let half = size / 2;
        let mut cy = 0;
        while cy < h + size {
            let mut cx = 0;
            while cx < w + size {
                let off = if (cy / half) % 2 == 0 { 0 } else { half };
                let dx = cx + off;
                let color = src[(cy.clamp(0, h - 1) * w + dx.clamp(0, w - 1)) as usize];
                let mut path = SkPath::new();
                path.move_to(dx as f32, (cy - half) as f32);
                path.line_to((dx + half) as f32, cy as f32);
                path.line_to(dx as f32, (cy + half) as f32);
                path.line_to((dx - half) as f32, cy as f32);
                path.close();
                canvas.fill_sk_path(&path, &solid(color, false));
                cx += size;
            }
            cy += half;
        }
        drop(canvas);
        Ok(out)
    }
}
