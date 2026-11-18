//! `MacOSWindowImageEffect` (`Core/ImageEffects/Filters/MacOSWindowImageEffect.cs`).

use crate::effects::prelude::*;

/// `MacOSWindowImageEffect`.
#[derive(Clone, Debug)]
pub struct MacOSWindowImageEffect {
    pub title: String,
    pub corner_radius: i32,
    pub dark_mode: bool,
    pub shadow_size: i32,
    pub padding: i32,
}

impl Default for MacOSWindowImageEffect {
    fn default() -> Self {
        MacOSWindowImageEffect {
            title: "".to_string(),
            corner_radius: 10,
            dark_mode: false,
            shadow_size: 20,
            padding: 20,
        }
    }
}

impl EffectImpl for MacOSWindowImageEffect {
    fn info() -> Info {
        Info::new("macos_window", "macOS", EffectCategory::Filters, "app/monitor")
            .desc("Wraps the image in a macOS-style window frame with a title bar and traffic light buttons.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            text_param::<Self>("title", "Title", "", |e: &mut Self, v| e.title = v),
            int_slider::<Self>("corner_radius", "Corner radius", 0, 30, 10, |e: &mut Self, v| e.corner_radius = v),
            bool_param::<Self>("dark_mode", "Dark mode", false, |e: &mut Self, v| e.dark_mode = v),
            int_slider::<Self>("shadow_size", "Shadow size", 0, 60, 20, |e: &mut Self, v| e.shadow_size = v),
            int_slider::<Self>("padding", "Padding", 0, 100, 20, |e: &mut Self, v| e.padding = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
