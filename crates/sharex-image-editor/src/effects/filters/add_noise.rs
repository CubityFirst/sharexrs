//! `AddNoiseImageEffect` (`Core/ImageEffects/Filters/AddNoiseImageEffect.cs`).

use crate::effects::prelude::*;

/// `AddNoiseImageEffect`.
#[derive(Clone, Debug)]
pub struct AddNoiseImageEffect {
    pub amount: f32,
    pub seed: i32,
}

impl Default for AddNoiseImageEffect {
    fn default() -> Self {
        AddNoiseImageEffect {
            amount: 8.0,
            seed: 1337,
        }
    }
}

impl EffectImpl for AddNoiseImageEffect {
    fn info() -> Info {
        Info::new("add_noise", "Add noise", EffectCategory::Filters, "image-effect/spray-can")
            .desc("Adds random noise to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", 0.0, 100.0, 8.0, |e: &mut Self, v| e.amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let amount = clampf(self.amount, 0.0, 100.0);
        let amplitude = (amount * 1.27).round_ties_even() as i32;
        if amplitude <= 0 {
            return Ok(source.clone());
        }
        let src = pixels(source);
        let seed = (self.seed as u32).wrapping_mul(747796405);
        let clamp = |v: i32| v.clamp(0, 255) as u8;
        let dst: Vec<SkColor> = src
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut h = noise_hash(i as u32 ^ seed);
                let nr = ((h & 0xFF) as i32 - 128) * amplitude / 128;
                h = noise_hash(h ^ 0x9E37_79B9);
                let ng = ((h & 0xFF) as i32 - 128) * amplitude / 128;
                h = noise_hash(h ^ 0x85EB_CA6B);
                let nb = ((h & 0xFF) as i32 - 128) * amplitude / 128;
                SkColor::rgba(clamp(c.r as i32 + nr), clamp(c.g as i32 + ng), clamp(c.b as i32 + nb), c.a)
            })
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}

// ---- port ----

fn noise_hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    x
}
