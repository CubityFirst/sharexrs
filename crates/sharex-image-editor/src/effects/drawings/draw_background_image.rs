//! `DrawBackgroundImageEffect` (`Core/ImageEffects/Drawings/DrawBackgroundImageEffect.cs`).

use crate::effects::prelude::*;

/// `DrawBackgroundImageEffect`.
#[derive(Clone, Debug)]
pub struct DrawBackgroundImageEffect {
    pub image_file_path: String,
    pub center: bool,
    pub tile: bool,
}

impl Default for DrawBackgroundImageEffect {
    fn default() -> Self {
        DrawBackgroundImageEffect {
            image_file_path: "".to_string(),
            center: true,
            tile: false,
        }
    }
}

impl EffectImpl for DrawBackgroundImageEffect {
    fn info() -> Info {
        Info::new("draw_background_image", "Background image", EffectCategory::Drawings, "app/image")
            .desc("Draws a background image behind the source image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            file_param::<Self>("image_file_path", "Image file path", "", |e: &mut Self, v| e.image_file_path = v),
            bool_param::<Self>("center", "Center", true, |e: &mut Self, v| e.center = v),
            bool_param::<Self>("tile", "Tile", false, |e: &mut Self, v| e.tile = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
