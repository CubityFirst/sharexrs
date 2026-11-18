//! `ChannelSwapImageEffect` (`Core/ImageEffects/Adjustments/ChannelSwapImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelSwapMode {
    RedGreen,
    RedBlue,
    GreenBlue,
    RotateRGB,
    RotateBGR,
}

/// `ChannelSwapImageEffect`.
#[derive(Clone, Debug)]
pub struct ChannelSwapImageEffect {
    pub mode: ChannelSwapMode,
}

impl Default for ChannelSwapImageEffect {
    fn default() -> Self {
        ChannelSwapImageEffect {
            mode: ChannelSwapMode::RedGreen,
        }
    }
}

impl EffectImpl for ChannelSwapImageEffect {
    fn info() -> Info {
        Info::new("channel_swap", "Channel swap", EffectCategory::Adjustments, "image-effect/shuffle")
            .desc("Swaps color channels for creative color effects.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, ChannelSwapMode>("mode", "Swap mode", ChannelSwapMode::RedGreen, |e: &mut Self, v| e.mode = v, &[("Red ↔ Green", ChannelSwapMode::RedGreen), ("Red ↔ Blue", ChannelSwapMode::RedBlue), ("Green ↔ Blue", ChannelSwapMode::GreenBlue), ("Rotate R→G→B", ChannelSwapMode::RotateRGB), ("Rotate B→G→R", ChannelSwapMode::RotateBGR)]),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let mode = self.mode;
        Ok(apply_pixel_operation(source, |c| match mode {
            ChannelSwapMode::RedGreen => SkColor::rgba(c.g, c.r, c.b, c.a),
            ChannelSwapMode::RedBlue => SkColor::rgba(c.b, c.g, c.r, c.a),
            ChannelSwapMode::GreenBlue => SkColor::rgba(c.r, c.b, c.g, c.a),
            ChannelSwapMode::RotateRGB => SkColor::rgba(c.b, c.r, c.g, c.a),
            ChannelSwapMode::RotateBGR => SkColor::rgba(c.g, c.b, c.r, c.a),
        }))
    }
}
