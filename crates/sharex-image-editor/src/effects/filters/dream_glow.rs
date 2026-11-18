//! `DreamGlowImageEffect` (`Core/ImageEffects/Filters/DreamGlowImageEffect.cs`).

use crate::effects::prelude::*;

/// `DreamGlowImageEffect`.
#[derive(Clone, Debug)]
pub struct DreamGlowImageEffect {
    pub radius: i32,
    pub opacity: f32,
}

impl Default for DreamGlowImageEffect {
    fn default() -> Self {
        DreamGlowImageEffect {
            radius: 15,
            opacity: 50.0,
        }
    }
}

impl EffectImpl for DreamGlowImageEffect {
    fn info() -> Info {
        Info::new("dream_glow", "Dream glow", EffectCategory::Filters, "app/cloud")
            .desc("Creates a dreamy soft glow by blending a blurred copy over the original.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Blur radius", 1, 50, 15, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>("opacity", "Blend opacity", 0.0, 100.0, 50.0, |e: &mut Self, v| e.opacity = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let alpha = clampf(self.opacity / 100.0, 0.0, 1.0);
        if alpha <= 0.0 || self.radius <= 0 {
            return Ok(source.clone());
        }
        let blurred = blur(source, self.radius as f32, self.radius as f32);
        let mut out = source.clone();
        composite_screen(&mut out, &blurred, (255.0 * alpha) as u8);
        Ok(out)
    }
}
