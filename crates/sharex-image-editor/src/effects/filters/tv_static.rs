//! `TVStaticImageEffect` (`Core/ImageEffects/Filters/TVStaticImageEffect.cs`).

use crate::effects::prelude::*;

/// `TVStaticImageEffect`.
#[derive(Clone, Debug)]
pub struct TVStaticImageEffect {
    pub amount: f32,
    pub seed: i32,
}

impl Default for TVStaticImageEffect {
    fn default() -> Self {
        TVStaticImageEffect {
            amount: 40.0,
            seed: 42,
        }
    }
}

impl EffectImpl for TVStaticImageEffect {
    fn info() -> Info {
        Info::new("tv_static", "TV static", EffectCategory::Filters, "image-effect/tv")
            .desc("Blends black-and-white TV static noise over the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", 0.0, 100.0, 40.0, |e: &mut Self, v| e.amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let blend = clampf(self.amount / 100.0, 0.0, 1.0);
        if blend <= 0.0 {
            return Ok(source.clone());
        }
        let mut rng = random::Compat::new(self.seed);
        let dst: Vec<SkColor> = pixels(source)
            .iter()
            .map(|c| {
                let noise = rng.next_max(256) as u8 as i32;
                let f = |v: u8| (v as f32 + (noise - v as i32) as f32 * blend) as u8;
                SkColor::rgba(f(c.r), f(c.g), f(c.b), c.a)
            })
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}
