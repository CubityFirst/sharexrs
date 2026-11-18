//! `DrawBackgroundEffect` (`Core/ImageEffects/Drawings/DrawBackgroundEffect.cs`).

use crate::effects::prelude::*;

/// `DrawBackgroundEffect`.
#[derive(Clone, Debug)]
pub struct DrawBackgroundEffect {
    pub color: SkColor,
}

impl Default for DrawBackgroundEffect {
    fn default() -> Self {
        DrawBackgroundEffect {
            color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for DrawBackgroundEffect {
    fn info() -> Info {
        Info::new("draw_background", "Background", EffectCategory::Drawings, "image-effect/paint-bucket")
            .desc("Draws a solid color background behind the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            color_param::<Self>("color", "Color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, source.width() as f32, source.height() as f32, &solid(self.color, true), None);
        canvas.draw_bitmap(source, 0.0, 0.0, None);
        drop(canvas);
        Ok(out)
    }
}
