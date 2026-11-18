//! `MatrixDigitalRainImageEffect` (`Core/ImageEffects/Filters/MatrixDigitalRainImageEffect.cs`).

use crate::effects::prelude::*;

/// `MatrixDigitalRainImageEffect`.
#[derive(Clone, Debug)]
pub struct MatrixDigitalRainImageEffect {
    pub cell_size: i32,
    pub density: f32,
    pub trail_length: i32,
    pub glow_amount: f32,
    pub source_blend: f32,
    pub rain_offset: f32,
    pub luminance_influence: f32,
    pub character_set: String,
    pub seed: i32,
}

impl Default for MatrixDigitalRainImageEffect {
    fn default() -> Self {
        MatrixDigitalRainImageEffect {
            cell_size: 12,
            density: 85.0,
            trail_length: 12,
            glow_amount: 40.0,
            source_blend: 22.0,
            rain_offset: 0.0,
            luminance_influence: 65.0,
            character_set: "01<>[]{}*+-/\\=#$%&".to_string(),
            seed: 1337,
        }
    }
}

impl EffectImpl for MatrixDigitalRainImageEffect {
    fn info() -> Info {
        Info::new("matrix_digital_rain", "Matrix digital rain", EffectCategory::Filters, "app/terminal")
            .desc("Overlays a Matrix-style digital rain effect using source luminance.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("cell_size", "Cell size", 6, 24, 12, |e: &mut Self, v| e.cell_size = v),
            float_slider::<Self>("density", "Density", 0.0, 100.0, 85.0, |e: &mut Self, v| e.density = v),
            int_slider::<Self>("trail_length", "Trail length", 3, 50, 12, |e: &mut Self, v| e.trail_length = v),
            float_slider::<Self>("glow_amount", "Glow amount", 0.0, 100.0, 40.0, |e: &mut Self, v| e.glow_amount = v),
            float_slider::<Self>("source_blend", "Source blend", 0.0, 100.0, 22.0, |e: &mut Self, v| e.source_blend = v),
            float_slider::<Self>("rain_offset", "Rain offset", 0.0, 100.0, 0.0, |e: &mut Self, v| e.rain_offset = v),
            float_slider::<Self>("luminance_influence", "Luminance influence", 0.0, 100.0, 65.0, |e: &mut Self, v| e.luminance_influence = v),
            text_param::<Self>("character_set", "Character set", "01<>[]{}*+-/\\=#$%&", |e: &mut Self, v| e.character_set = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
