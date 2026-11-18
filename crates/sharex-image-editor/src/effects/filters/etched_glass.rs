//! `EtchedGlassImageEffect` (`Core/ImageEffects/Filters/EtchedGlassImageEffect.cs`).

use crate::effects::prelude::*;

/// `EtchedGlassImageEffect`.
#[derive(Clone, Debug)]
pub struct EtchedGlassImageEffect {
    pub frost: f32,
    pub engrave: f32,
    pub refraction: f32,
    pub highlight: f32,
    pub background_fade: f32,
}

impl Default for EtchedGlassImageEffect {
    fn default() -> Self {
        EtchedGlassImageEffect {
            frost: 48.0,
            engrave: 68.0,
            refraction: 18.0,
            highlight: 42.0,
            background_fade: 38.0,
        }
    }
}

impl EffectImpl for EtchedGlassImageEffect {
    fn info() -> Info {
        Info::new("etched_glass", "Etched glass", EffectCategory::Filters, "image-effect/glass")
            .desc("Simulates an etched glass surface with frost, engraving, and refraction effects.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("frost", "Frost", 0.0, 100.0, 48.0, |e: &mut Self, v| e.frost = v),
            float_slider::<Self>("engrave", "Engrave", 0.0, 100.0, 68.0, |e: &mut Self, v| e.engrave = v),
            float_slider::<Self>("refraction", "Refraction", 0.0, 100.0, 18.0, |e: &mut Self, v| e.refraction = v),
            float_slider::<Self>("highlight", "Highlight", 0.0, 100.0, 42.0, |e: &mut Self, v| e.highlight = v),
            float_slider::<Self>("background_fade", "Background fade", 0.0, 100.0, 38.0, |e: &mut Self, v| e.background_fade = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
