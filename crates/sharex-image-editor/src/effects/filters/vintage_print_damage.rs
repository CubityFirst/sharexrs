//! `VintagePrintDamageImageEffect` (`Core/ImageEffects/Filters/VintagePrintDamageImageEffect.cs`).

use crate::effects::prelude::*;

/// `VintagePrintDamageImageEffect`.
#[derive(Clone, Debug)]
pub struct VintagePrintDamageImageEffect {
    pub sepia_amount: f32,
    pub fade_amount: f32,
    pub grain_amount: f32,
    pub scratch_amount: f32,
    pub dust_amount: f32,
    pub seed: i32,
}

impl Default for VintagePrintDamageImageEffect {
    fn default() -> Self {
        VintagePrintDamageImageEffect {
            sepia_amount: 65.0,
            fade_amount: 30.0,
            grain_amount: 25.0,
            scratch_amount: 25.0,
            dust_amount: 20.0,
            seed: 1947,
        }
    }
}

impl EffectImpl for VintagePrintDamageImageEffect {
    fn info() -> Info {
        Info::new("vintage_print_damage", "Vintage print damage", EffectCategory::Filters, "image-effect/newspaper")
            .desc("Simulates aged print damage with sepia toning, grain, scratches, and dust.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("sepia_amount", "Sepia amount", 0.0, 100.0, 65.0, |e: &mut Self, v| e.sepia_amount = v),
            float_slider::<Self>("fade_amount", "Fade amount", 0.0, 100.0, 30.0, |e: &mut Self, v| e.fade_amount = v),
            float_slider::<Self>("grain_amount", "Grain amount", 0.0, 100.0, 25.0, |e: &mut Self, v| e.grain_amount = v),
            float_slider::<Self>("scratch_amount", "Scratch amount", 0.0, 100.0, 25.0, |e: &mut Self, v| e.scratch_amount = v),
            float_slider::<Self>("dust_amount", "Dust amount", 0.0, 100.0, 20.0, |e: &mut Self, v| e.dust_amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
