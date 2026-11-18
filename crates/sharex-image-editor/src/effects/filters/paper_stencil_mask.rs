//! `PaperStencilMaskImageEffect` (`Core/ImageEffects/Filters/PaperStencilMaskImageEffect.cs`).

use crate::effects::prelude::*;

/// `PaperStencilMaskImageEffect`.
#[derive(Clone, Debug)]
pub struct PaperStencilMaskImageEffect {
    pub threshold: f32,
    pub feather_radius: f32,
    pub edge_strength: f32,
    pub background_dim: f32,
    pub invert_mask: bool,
    pub seed: i32,
    pub stencil_color: SkColor,
}

impl Default for PaperStencilMaskImageEffect {
    fn default() -> Self {
        PaperStencilMaskImageEffect {
            threshold: 140.0,
            feather_radius: 8.0,
            edge_strength: 70.0,
            background_dim: 35.0,
            invert_mask: false,
            seed: 1337,
            stencil_color: SkColor::argb(0xDC000000),
        }
    }
}

impl EffectImpl for PaperStencilMaskImageEffect {
    fn info() -> Info {
        Info::new("paper_stencil_mask", "Paper stencil mask", EffectCategory::Filters, "app/stamp")
            .desc("Applies a paper stencil mask effect with adjustable threshold and edge strength.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("threshold", "Threshold", 0.0, 255.0, 140.0, |e: &mut Self, v| e.threshold = v),
            float_slider::<Self>("feather_radius", "Feather radius", 0.0, 200.0, 8.0, |e: &mut Self, v| e.feather_radius = v),
            float_slider::<Self>("edge_strength", "Edge strength", 0.0, 100.0, 70.0, |e: &mut Self, v| e.edge_strength = v),
            float_slider::<Self>("background_dim", "Background dim", 0.0, 100.0, 35.0, |e: &mut Self, v| e.background_dim = v),
            bool_param::<Self>("invert_mask", "Invert mask", false, |e: &mut Self, v| e.invert_mask = v),
            color_param::<Self>("stencil_color", "Stencil color", SkColor::argb(0xDC000000), |e: &mut Self, v| e.stencil_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
