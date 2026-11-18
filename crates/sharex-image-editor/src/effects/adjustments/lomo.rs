//! `LomoImageEffect` (`Core/ImageEffects/Adjustments/LomoImageEffect.cs`).

use crate::effects::prelude::*;

/// `LomoImageEffect`.
#[derive(Clone, Debug)]
pub struct LomoImageEffect {
    pub strength: f32,
}

impl Default for LomoImageEffect {
    fn default() -> Self {
        LomoImageEffect {
            strength: 100.0,
        }
    }
}

impl EffectImpl for LomoImageEffect {
    fn info() -> Info {
        Info::new("lomo", "Lomo", EffectCategory::Adjustments, "image-effect/aperture")
            .desc("Applies a lomography look with high contrast, warm tones, and vignette.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 100.0, |e: &mut Self, v| e.strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let s = clampf(self.strength / 100.0, 0.0, 1.0);
        if s <= 0.0 {
            return Ok(source.clone());
        }
        let (w, h) = (source.width() as i32, source.height() as i32);
        const LOMO: [f32; 20] = [
            1.3, 0.05, -0.05, 0.0, 0.02, //
            -0.02, 1.1, -0.02, 0.0, 0.01, //
            -0.05, 0.0, 0.9, 0.0, -0.02, //
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        let matrix = blend_with_identity(&LOMO, s);
        let mut colored = apply_color_matrix(source, &matrix);
        let mut canvas = Canvas::new(&mut colored);
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        let radius = ((w * w + h * h) as f32).sqrt() * 0.5;
        let paint = sk::radial_gradient(
            (cx, cy),
            radius,
            &[color::TRANSPARENT, SkColor::rgba(0, 0, 0, (180.0 * s) as u8)],
            Some(&[0.5, 1.0]),
            false,
        );
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, w as f32, h as f32, &paint, None);
        drop(canvas);
        Ok(colored)
    }
}
