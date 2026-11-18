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
        let _ = source;
        not_ported()
    }
}
