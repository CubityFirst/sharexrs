//! `ScanlineImageEffect` (`Core/ImageEffects/Filters/ScanlineImageEffect.cs`).

use crate::effects::prelude::*;

/// `ScanlineImageEffect`.
#[derive(Clone, Debug)]
pub struct ScanlineImageEffect {
    pub spacing: i32,
    pub opacity: f32,
}

impl Default for ScanlineImageEffect {
    fn default() -> Self {
        ScanlineImageEffect {
            spacing: 4,
            opacity: 50.0,
        }
    }
}

impl EffectImpl for ScanlineImageEffect {
    fn info() -> Info {
        Info::new("scanline", "Scanlines", EffectCategory::Filters, "app/scan-line")
            .desc("Adds retro CRT-style horizontal scanlines.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("spacing", "Line spacing", 2, 20, 4, |e: &mut Self, v| e.spacing = v),
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 50.0, |e: &mut Self, v| e.opacity = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
