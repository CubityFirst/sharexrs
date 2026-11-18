//! `ColorDepthImageEffect` (`Core/ImageEffects/Filters/ColorDepthImageEffect.cs`).

use crate::effects::prelude::*;

/// `ColorDepthImageEffect`.
#[derive(Clone, Debug)]
pub struct ColorDepthImageEffect {
    pub bits_per_channel: i32,
}

impl Default for ColorDepthImageEffect {
    fn default() -> Self {
        ColorDepthImageEffect {
            bits_per_channel: 4,
        }
    }
}

impl EffectImpl for ColorDepthImageEffect {
    fn info() -> Info {
        Info::new("color_depth", "Color depth", EffectCategory::Filters, "app/layers")
            .desc("Reduces the number of bits per color channel.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("bitsPerChannel", "Bits per channel", 1, 8, 4, |e: &mut Self, v| e.bits_per_channel = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
