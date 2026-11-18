//! `AnimeSpeedLinesImageEffect` (`Core/ImageEffects/Filters/AnimeSpeedLinesImageEffect.cs`).

use crate::effects::prelude::*;

/// `AnimeSpeedLinesImageEffect`.
#[derive(Clone, Debug)]
pub struct AnimeSpeedLinesImageEffect {
    pub density: f32,
    pub strength: f32,
    pub focus_radius: f32,
    pub center_x: f32,
    pub center_y: f32,
    pub contrast: f32,
    pub seed: i32,
}

impl Default for AnimeSpeedLinesImageEffect {
    fn default() -> Self {
        AnimeSpeedLinesImageEffect {
            density: 70.0,
            strength: 65.0,
            focus_radius: 18.0,
            center_x: 50.0,
            center_y: 50.0,
            contrast: 35.0,
            seed: 9911,
        }
    }
}

impl EffectImpl for AnimeSpeedLinesImageEffect {
    fn info() -> Info {
        Info::new("anime_speed_lines", "Anime speed lines", EffectCategory::Filters, "app/scan-line")
            .desc("Overlays radial speed lines in a manga/anime style.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("density", "Density", 10.0, 100.0, 70.0, |e: &mut Self, v| e.density = v),
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 65.0, |e: &mut Self, v| e.strength = v),
            float_slider::<Self>("focus_radius", "Focus radius", 0.0, 80.0, 18.0, |e: &mut Self, v| e.focus_radius = v),
            float_slider::<Self>("center_x", "Center X", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x = v),
            float_slider::<Self>("center_y", "Center Y", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y = v),
            float_slider::<Self>("contrast", "Contrast", 0.0, 100.0, 35.0, |e: &mut Self, v| e.contrast = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
