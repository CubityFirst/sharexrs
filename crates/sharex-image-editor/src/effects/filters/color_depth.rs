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
        Info::new(
            "color_depth",
            "Color depth",
            EffectCategory::Filters,
            "app/layers",
        )
        .desc("Reduces the number of bits per color channel.")
    }

    fn params() -> Vec<ParamDef> {
        vec![int_slider::<Self>(
            "bitsPerChannel",
            "Bits per channel",
            1,
            8,
            4,
            |e: &mut Self, v| e.bits_per_channel = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let bits = self.bits_per_channel.clamp(1, 8);
        if bits == 8 {
            return Ok(source.clone());
        }
        let colors = 2f64.powf(bits as f64);
        let interval = 255.0 / (colors - 1.0);
        let remap =
            |c: u8| ((c as f64 / interval).round_ties_even() * interval).round_ties_even() as u8;
        let dst: Vec<SkColor> = pixels(source)
            .iter()
            .map(|c| SkColor::rgba(remap(c.r), remap(c.g), remap(c.b), c.a))
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}
