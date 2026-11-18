//! `PencilSketchImageEffect` (`Core/ImageEffects/Filters/PencilSketchImageEffect.cs`).

use crate::effects::prelude::*;

/// `PencilSketchImageEffect`.
#[derive(Clone, Debug)]
pub struct PencilSketchImageEffect {
    pub blur_radius: i32,
    pub edge_strength: f32,
    pub pencil_darkness: f32,
    pub paper_brightness: f32,
}

impl Default for PencilSketchImageEffect {
    fn default() -> Self {
        PencilSketchImageEffect {
            blur_radius: 8,
            edge_strength: 65.0,
            pencil_darkness: 70.0,
            paper_brightness: 100.0,
        }
    }
}

impl EffectImpl for PencilSketchImageEffect {
    fn info() -> Info {
        Info::new("pencil_sketch", "Pencil sketch", EffectCategory::Filters, "app/edit")
            .desc("Converts the image to a pencil sketch style drawing.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("blur_radius", "Blur radius", 1, 24, 8, |e: &mut Self, v| e.blur_radius = v),
            float_slider::<Self>("edge_strength", "Edge strength", 0.0, 160.0, 65.0, |e: &mut Self, v| e.edge_strength = v),
            float_slider::<Self>("pencil_darkness", "Pencil darkness", 0.0, 100.0, 70.0, |e: &mut Self, v| e.pencil_darkness = v),
            float_slider::<Self>("paper_brightness", "Paper brightness", 40.0, 130.0, 100.0, |e: &mut Self, v| e.paper_brightness = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
