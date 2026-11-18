//! `ThresholdImageEffect` (`Core/ImageEffects/Adjustments/ThresholdImageEffect.cs`).

use crate::effects::prelude::*;

/// `ThresholdImageEffect`.
#[derive(Clone, Debug)]
pub struct ThresholdImageEffect {
    pub value: i32,
}

impl Default for ThresholdImageEffect {
    fn default() -> Self {
        ThresholdImageEffect {
            value: 128,
        }
    }
}

impl EffectImpl for ThresholdImageEffect {
    fn info() -> Info {
        Info::new("threshold", "Threshold", EffectCategory::Adjustments, "image-effect/binary")
            .desc("Applies a contrast threshold.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("value", "Threshold", 0, 255, 128, |e: &mut Self, v| e.value = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let t = self.value.clamp(0, 255);
        Ok(apply_pixel_operation(source, |c| {
            let luma = ((c.r as i32 * 77) + (c.g as i32 * 150) + (c.b as i32 * 29)) >> 8;
            let bw = if luma >= t { 255 } else { 0 };
            SkColor::rgba(bw, bw, bw, c.a)
        }))
    }
}
