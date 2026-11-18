//! `Windows11ImageEffect` (`Core/ImageEffects/Filters/Windows11ImageEffect.cs`).

use crate::effects::prelude::*;

/// `Windows11ImageEffect`.
#[derive(Clone, Debug)]
pub struct Windows11ImageEffect {
    pub title: String,
    pub corner_radius: i32,
    pub dark_mode: bool,
    pub shadow_size: i32,
    pub padding: i32,
}

impl Default for Windows11ImageEffect {
    fn default() -> Self {
        Windows11ImageEffect {
            title: "".to_string(),
            corner_radius: 8,
            dark_mode: false,
            shadow_size: 16,
            padding: 20,
        }
    }
}

impl EffectImpl for Windows11ImageEffect {
    fn info() -> Info {
        Info::new("windows11_window", "Windows 11", EffectCategory::Filters, "app/monitor")
            .desc("Wraps the image in a Windows 11-style window frame with a title bar and window controls.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            text_param::<Self>("title", "Title", "", |e: &mut Self, v| e.title = v),
            int_slider::<Self>("corner_radius", "Corner radius", 0, 16, 8, |e: &mut Self, v| e.corner_radius = v),
            bool_param::<Self>("dark_mode", "Dark mode", false, |e: &mut Self, v| e.dark_mode = v),
            int_slider::<Self>("shadow_size", "Shadow size", 0, 60, 16, |e: &mut Self, v| e.shadow_size = v),
            int_slider::<Self>("padding", "Padding", 0, 100, 20, |e: &mut Self, v| e.padding = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
