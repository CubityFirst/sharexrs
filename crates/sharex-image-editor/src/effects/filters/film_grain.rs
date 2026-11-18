//! `FilmGrainImageEffect` (`Core/ImageEffects/Filters/FilmGrainImageEffect.cs`).

use crate::effects::prelude::*;

/// `FilmGrainImageEffect`.
#[derive(Clone, Debug)]
pub struct FilmGrainImageEffect {
    pub amount: i32,
    pub monochrome: bool,
}

impl Default for FilmGrainImageEffect {
    fn default() -> Self {
        FilmGrainImageEffect {
            amount: 30,
            monochrome: true,
        }
    }
}

impl EffectImpl for FilmGrainImageEffect {
    fn info() -> Info {
        Info::new(
            "film_grain",
            "Film grain",
            EffectCategory::Filters,
            "app/film",
        )
        .desc("Adds organic film-like grain texture.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("amount", "Amount", 1, 100, 30, |e: &mut Self, v| {
                e.amount = v
            }),
            bool_param::<Self>("monochrome", "Monochrome", true, |e: &mut Self, v| {
                e.monochrome = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let mut rng = random::Compat::new(42);
        let range = (self.amount as f32 * 2.55) as i32;
        let cb = |v: i32| v.clamp(0, 255) as u8;
        let dst: Vec<SkColor> = pixels(source)
            .iter()
            .map(|c| {
                if self.monochrome {
                    let n = rng.next_range(-range, range + 1);
                    SkColor::rgba(
                        cb(c.r as i32 + n),
                        cb(c.g as i32 + n),
                        cb(c.b as i32 + n),
                        c.a,
                    )
                } else {
                    let r = cb(c.r as i32 + rng.next_range(-range, range + 1));
                    let g = cb(c.g as i32 + rng.next_range(-range, range + 1));
                    let b = cb(c.b as i32 + rng.next_range(-range, range + 1));
                    SkColor::rgba(r, g, b, c.a)
                }
            })
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}
