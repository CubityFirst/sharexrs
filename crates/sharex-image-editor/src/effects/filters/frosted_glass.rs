//! `FrostedGlassImageEffect` (`Core/ImageEffects/Filters/FrostedGlassImageEffect.cs`).

use crate::effects::prelude::*;

/// `FrostedGlassImageEffect`.
#[derive(Clone, Debug)]
pub struct FrostedGlassImageEffect {
    pub radius: i32,
}

impl Default for FrostedGlassImageEffect {
    fn default() -> Self {
        FrostedGlassImageEffect {
            radius: 5,
        }
    }
}

impl EffectImpl for FrostedGlassImageEffect {
    fn info() -> Info {
        Info::new("frosted_glass", "Frosted glass", EffectCategory::Filters, "image-effect/snowflake")
            .desc("Simulates looking through frosted glass by randomizing nearby pixels.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Radius", 1, 20, 5, |e: &mut Self, v| e.radius = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let radius = 1.max(self.radius);
        let (w, h) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let mut rng = random::Compat::new(42);
        for y in 0..h {
            for x in 0..w {
                let ox = (x + rng.next_range(-radius, radius + 1)).clamp(0, w - 1);
                let oy = (y + rng.next_range(-radius, radius + 1)).clamp(0, h - 1);
                dst[(y * w + x) as usize] = src[(oy * w + ox) as usize];
            }
        }
        Ok(from_pixels(w as u32, h as u32, &dst))
    }
}
