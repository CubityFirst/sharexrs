//! `ASCIIArtImageEffect` (`Core/ImageEffects/Filters/ASCIIArtImageEffect.cs`).

use crate::effects::prelude::*;

/// `ASCIIArtImageEffect`.
#[derive(Clone, Debug)]
pub struct ASCIIArtImageEffect {
    pub cell_size: i32,
    pub contrast: f32,
    pub character_set: String,
    pub invert: bool,
    pub dark_background: bool,
    pub use_source_color: bool,
}

impl Default for ASCIIArtImageEffect {
    fn default() -> Self {
        ASCIIArtImageEffect {
            cell_size: 8,
            contrast: 110.0,
            character_set: "@%#*+=-:. ".to_string(),
            invert: false,
            dark_background: true,
            use_source_color: true,
        }
    }
}

impl EffectImpl for ASCIIArtImageEffect {
    fn info() -> Info {
        Info::new("ascii_art", "ASCII art", EffectCategory::Filters, "app/terminal")
            .desc("Converts the image into ASCII character art.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("cell_size", "Cell size", 4, 24, 8, |e: &mut Self, v| e.cell_size = v),
            float_slider::<Self>("contrast", "Contrast", 50.0, 200.0, 110.0, |e: &mut Self, v| e.contrast = v),
            text_param::<Self>("character_set", "Character set", "@%#*+=-:. ", |e: &mut Self, v| e.character_set = v),
            bool_param::<Self>("invert", "Invert", false, |e: &mut Self, v| e.invert = v),
            bool_param::<Self>("dark_background", "Dark background", true, |e: &mut Self, v| e.dark_background = v),
            bool_param::<Self>("use_source_color", "Use source color", true, |e: &mut Self, v| e.use_source_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
