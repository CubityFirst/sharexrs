//! `LightLeakImageEffect` (`Core/ImageEffects/Filters/LightLeakImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightLeakPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Left,
    Right,
}

/// `LightLeakImageEffect`.
#[derive(Clone, Debug)]
pub struct LightLeakImageEffect {
    pub intensity: f32,
    pub color: SkColor,
    pub position: LightLeakPosition,
}

impl Default for LightLeakImageEffect {
    fn default() -> Self {
        LightLeakImageEffect {
            intensity: 50.0,
            color: SkColor::argb(0xFFFFA032),
            position: LightLeakPosition::TopRight,
        }
    }
}

impl EffectImpl for LightLeakImageEffect {
    fn info() -> Info {
        Info::new(
            "light_leak",
            "Light leak",
            EffectCategory::Filters,
            "app/sun",
        )
        .desc("Adds a warm film light leak effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "intensity",
                "Intensity",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.intensity = v,
            ),
            color_param::<Self>(
                "color",
                "Color",
                SkColor::argb(0xFFFFA032),
                |e: &mut Self, v| e.color = v,
            ),
            enum_param::<Self, LightLeakPosition>(
                "position",
                "Position",
                LightLeakPosition::TopRight,
                |e: &mut Self, v| e.position = v,
                &[
                    ("Top left", LightLeakPosition::TopLeft),
                    ("Top right", LightLeakPosition::TopRight),
                    ("Bottom left", LightLeakPosition::BottomLeft),
                    ("Bottom right", LightLeakPosition::BottomRight),
                    ("Left", LightLeakPosition::Left),
                    ("Right", LightLeakPosition::Right),
                ],
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let alpha = clampf(self.intensity / 100.0, 0.0, 1.0);
        if alpha <= 0.0 {
            return Ok(source.clone());
        }
        let (w, h) = (source.width() as i32, source.height() as i32);
        let (wf, hf) = (w as f32, h as f32);
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        let center = match self.position {
            LightLeakPosition::TopLeft => (0.0, 0.0),
            LightLeakPosition::TopRight => (wf, 0.0),
            LightLeakPosition::BottomLeft => (0.0, hf),
            LightLeakPosition::BottomRight => (wf, hf),
            LightLeakPosition::Left => (0.0, hf / 2.0),
            LightLeakPosition::Right => (wf, hf / 2.0),
        };
        let radius = ((w * w + h * h) as f32).sqrt() * 0.8;
        let leak = self.color.with_alpha((255.0 * alpha) as u8);
        let mut paint = sk::radial_gradient(
            center,
            radius,
            &[leak, color::TRANSPARENT],
            Some(&[0.0, 1.0]),
            false,
        );
        paint.blend = Blend::Screen;
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, wf, hf, &paint, None);
        drop(canvas);
        Ok(out)
    }
}
