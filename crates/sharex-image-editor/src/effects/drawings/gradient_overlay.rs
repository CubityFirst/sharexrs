//! `GradientOverlayImageEffect` (`Core/ImageEffects/Drawings/GradientOverlayImageEffect.cs`).

use crate::effects::prelude::*;

/// `GradientOverlayImageEffect`.
#[derive(Clone, Debug)]
pub struct GradientOverlayImageEffect {
    pub color1: SkColor,
    pub color2: SkColor,
    pub angle: i32,
    pub opacity: f32,
}

impl Default for GradientOverlayImageEffect {
    fn default() -> Self {
        GradientOverlayImageEffect {
            color1: SkColor::argb(0xFFFF0080),
            color2: SkColor::argb(0xFF0080FF),
            angle: 0,
            opacity: 50.0,
        }
    }
}

impl EffectImpl for GradientOverlayImageEffect {
    fn info() -> Info {
        Info::new(
            "gradient_overlay",
            "Gradient overlay",
            EffectCategory::Drawings,
            "image-effect/paint-brush",
        )
        .desc("Blends a two-color gradient overlay on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            color_param::<Self>(
                "color1",
                "Start color",
                SkColor::argb(0xFFFF0080),
                |e: &mut Self, v| e.color1 = v,
            ),
            color_param::<Self>(
                "color2",
                "End color",
                SkColor::argb(0xFF0080FF),
                |e: &mut Self, v| e.color2 = v,
            ),
            int_slider::<Self>("angle", "Angle", 0, 360, 0, |e: &mut Self, v| e.angle = v),
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
        let (w, h) = (source.width() as i32, source.height() as i32);
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        let rad = self.angle as f32 * std::f32::consts::PI / 180.0;
        let diagonal = ((w * w + h * h) as f32).sqrt() / 2.0;
        let start = (cx - rad.cos() * diagonal, cy - rad.sin() * diagonal);
        let end = (cx + rad.cos() * diagonal, cy + rad.sin() * diagonal);
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        let (c1, c2) = (self.color1, self.color2);
        let gradient = sharex_skia::raster::gradient::LinearGradient {
            start,
            end,
            colors: vec![
                c1.with_alpha((c1.a as f32 * alpha) as u8).rgba_array(),
                c2.with_alpha((c2.a as f32 * alpha) as u8).rgba_array(),
            ],
            positions: Some(vec![0.0, 1.0]),
            local: Matrix::IDENTITY,
        };
        let mut paint = Paint::shader(Shader::Gradient(gradient));
        paint.blend = Blend::Overlay;
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, w as f32, h as f32, &paint, None);
        drop(canvas);
        Ok(out)
    }
}
