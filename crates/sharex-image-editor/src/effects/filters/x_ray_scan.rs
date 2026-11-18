//! `XRayScanImageEffect` (`Core/ImageEffects/Filters/XRayScanImageEffect.cs`).

use crate::effects::prelude::*;

/// `XRayScanImageEffect`.
#[derive(Clone, Debug)]
pub struct XRayScanImageEffect {
    pub contrast: f32,
    pub glow: f32,
    pub edge_boost: f32,
    pub scanlines: f32,
    pub noise: f32,
}

impl Default for XRayScanImageEffect {
    fn default() -> Self {
        XRayScanImageEffect {
            contrast: 70.0,
            glow: 60.0,
            edge_boost: 68.0,
            scanlines: 40.0,
            noise: 14.0,
        }
    }
}

impl EffectImpl for XRayScanImageEffect {
    fn info() -> Info {
        Info::new("x_ray_scan", "X-ray scan", EffectCategory::Filters, "app/scan-eye")
            .desc("Simulates an X-ray scan with edge detection and glow effects.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("contrast", "Contrast", 0.0, 100.0, 70.0, |e: &mut Self, v| e.contrast = v),
            float_slider::<Self>("glow", "Glow", 0.0, 100.0, 60.0, |e: &mut Self, v| e.glow = v),
            float_slider::<Self>("edge_boost", "Edge boost", 0.0, 100.0, 68.0, |e: &mut Self, v| e.edge_boost = v),
            float_slider::<Self>("scanlines", "Scanlines", 0.0, 100.0, 40.0, |e: &mut Self, v| e.scanlines = v),
            float_slider::<Self>("noise", "Noise", 0.0, 100.0, 14.0, |e: &mut Self, v| e.noise = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
