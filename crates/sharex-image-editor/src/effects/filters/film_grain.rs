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
        Info::new("film_grain", "Film grain", EffectCategory::Filters, "app/film")
            .desc("Adds organic film-like grain texture.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("amount", "Amount", 1, 100, 30, |e: &mut Self, v| e.amount = v),
            bool_param::<Self>("monochrome", "Monochrome", true, |e: &mut Self, v| e.monochrome = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
