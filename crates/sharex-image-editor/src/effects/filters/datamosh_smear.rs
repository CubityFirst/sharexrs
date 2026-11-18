//! `DatamoshSmearImageEffect` (`Core/ImageEffects/Filters/DatamoshSmearImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatamoshDirection {
    Horizontal,
    Vertical,
}

/// `DatamoshSmearImageEffect`.
#[derive(Clone, Debug)]
pub struct DatamoshSmearImageEffect {
    pub direction: DatamoshDirection,
    pub smear_amount: f32,
    pub corruption: f32,
    pub block_size: i32,
    pub drift: f32,
    pub channel_split: f32,
    pub seed: i32,
}

impl Default for DatamoshSmearImageEffect {
    fn default() -> Self {
        DatamoshSmearImageEffect {
            direction: DatamoshDirection::Horizontal,
            smear_amount: 58.0,
            corruption: 36.0,
            block_size: 12,
            drift: 24.0,
            channel_split: 25.0,
            seed: 9011,
        }
    }
}

impl EffectImpl for DatamoshSmearImageEffect {
    fn info() -> Info {
        Info::new("datamosh_smear", "Datamosh smear", EffectCategory::Filters, "image-effect/cpu")
            .desc("Simulates digital datamoshing with smeared pixel blocks and channel splitting.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, DatamoshDirection>("direction", "Direction", DatamoshDirection::Horizontal, |e: &mut Self, v| e.direction = v, &[("Horizontal", DatamoshDirection::Horizontal), ("Vertical", DatamoshDirection::Vertical)]),
            float_slider::<Self>("smear_amount", "Smear amount", 0.0, 100.0, 58.0, |e: &mut Self, v| e.smear_amount = v),
            float_slider::<Self>("corruption", "Corruption", 0.0, 100.0, 36.0, |e: &mut Self, v| e.corruption = v),
            int_slider::<Self>("block_size", "Block size", 4, 64, 12, |e: &mut Self, v| e.block_size = v),
            float_slider::<Self>("drift", "Drift", -100.0, 100.0, 24.0, |e: &mut Self, v| e.drift = v),
            float_slider::<Self>("channel_split", "Channel split", 0.0, 100.0, 25.0, |e: &mut Self, v| e.channel_split = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
