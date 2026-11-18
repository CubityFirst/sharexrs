//! `FilmEmulationImageEffect` (`Core/ImageEffects/Adjustments/FilmEmulationImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilmEmulationPreset {
    Classic,
    Warm,
    Cool,
    Faded,
    CrossProcessed,
}

/// `FilmEmulationImageEffect`.
#[derive(Clone, Debug)]
pub struct FilmEmulationImageEffect {
    pub preset: FilmEmulationPreset,
    pub tone_strength: f32,
    pub grain_amount: f32,
    pub fade_amount: f32,
    pub contrast_amount: f32,
    pub seed: i32,
}

impl Default for FilmEmulationImageEffect {
    fn default() -> Self {
        FilmEmulationImageEffect {
            preset: FilmEmulationPreset::Classic,
            tone_strength: 65.0,
            grain_amount: 12.0,
            fade_amount: 10.0,
            contrast_amount: 110.0,
            seed: 1977,
        }
    }
}

impl EffectImpl for FilmEmulationImageEffect {
    fn info() -> Info {
        Info::new("film_emulation", "Film emulation", EffectCategory::Adjustments, "app/film")
            .desc("Applies cinematic analog film looks with grain and fade.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, FilmEmulationPreset>("preset", "Preset", FilmEmulationPreset::Classic, |e: &mut Self, v| e.preset = v, &[("Classic", FilmEmulationPreset::Classic), ("Warm", FilmEmulationPreset::Warm), ("Cool", FilmEmulationPreset::Cool), ("Faded", FilmEmulationPreset::Faded), ("Cross processed", FilmEmulationPreset::CrossProcessed)]),
            float_slider::<Self>("tone_strength", "Tone strength", 0.0, 100.0, 65.0, |e: &mut Self, v| e.tone_strength = v),
            float_slider::<Self>("grain_amount", "Grain amount", 0.0, 100.0, 12.0, |e: &mut Self, v| e.grain_amount = v),
            float_slider::<Self>("fade_amount", "Fade amount", 0.0, 100.0, 10.0, |e: &mut Self, v| e.fade_amount = v),
            float_slider::<Self>("contrast_amount", "Contrast amount", 50.0, 150.0, 110.0, |e: &mut Self, v| e.contrast_amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
