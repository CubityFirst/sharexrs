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
        Info::new("light_leak", "Light leak", EffectCategory::Filters, "app/sun")
            .desc("Adds a warm film light leak effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("intensity", "Intensity", 0.0, 100.0, 50.0, |e: &mut Self, v| e.intensity = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFFFFA032), |e: &mut Self, v| e.color = v),
            enum_param::<Self, LightLeakPosition>("position", "Position", LightLeakPosition::TopRight, |e: &mut Self, v| e.position = v, &[("Top left", LightLeakPosition::TopLeft), ("Top right", LightLeakPosition::TopRight), ("Bottom left", LightLeakPosition::BottomLeft), ("Bottom right", LightLeakPosition::BottomRight), ("Left", LightLeakPosition::Left), ("Right", LightLeakPosition::Right)]),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
