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
        Info::new(
            "paper_stencil_mask",
            "Paper stencil mask",
            EffectCategory::Filters,
            "app/stamp",
        )
        .desc("Applies a paper stencil mask effect with adjustable threshold and edge strength.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "threshold",
                "Threshold",
                0.0,
                255.0,
                140.0,
                |e: &mut Self, v| e.threshold = v,
            ),
            float_slider::<Self>(
                "feather_radius",
                "Feather radius",
                0.0,
                200.0,
                8.0,
                |e: &mut Self, v| e.feather_radius = v,
            ),
            float_slider::<Self>(
                "edge_strength",
                "Edge strength",
                0.0,
                100.0,
                70.0,
                |e: &mut Self, v| e.edge_strength = v,
            ),
            float_slider::<Self>(
                "background_dim",
                "Background dim",
                0.0,
                100.0,
                35.0,
                |e: &mut Self, v| e.background_dim = v,
            ),
            bool_param::<Self>("invert_mask", "Invert mask", false, |e: &mut Self, v| {
                e.invert_mask = v
            }),
            color_param::<Self>(
                "stencil_color",
                "Stencil color",
                SkColor::argb(0xDC000000),
                |e: &mut Self, v| e.stencil_color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let threshold = clampf(self.threshold, 0.0, 255.0) / 255.0;
        let feather_px = clampf(self.feather_radius, 0.0, 200.0);
        let feather = clampf(feather_px, 0.0, 30.0) / 30.0;
        let range = 0.02 + 0.13 * feather;
        let edge = clampf(self.edge_strength, 0.0, 100.0) / 100.0;
        if edge <= 0.0 && self.background_dim <= 0.0 {
            return Ok(source.clone());
        }
        let dim = clamp01(1.0 - clampf(self.background_dim, 0.0, 100.0) / 100.0);
        let sc = self.stencil_color;
        let cap = (sc.a as f32 / 255.0) * edge;
        let (sr, sg, sb) = (
            sc.r as f32 / 255.0,
            sc.g as f32 / 255.0,
            sc.b as f32 / 255.0,
        );
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            for x in 0..width {
                let i = (y * width + x) as usize;
                let s = src[i];
                let l =
                    ((0.2126 * s.r as f32) + (0.7152 * s.g as f32) + (0.0722 * s.b as f32)) / 255.0;
                let jitter = (hash01(x, y, self.seed ^ 0xCAFE) * 2.0 - 1.0) * 0.003;
                let mut mask =
                    smooth_step(threshold - range + jitter, threshold + range + jitter, l);
                if self.invert_mask {
                    mask = 1.0 - mask;
                }
                let mix = clamp01(mask * cap);
                let r = lerp((s.r as f32 / 255.0) * dim, sr, mix);
                let g = lerp((s.g as f32 / 255.0) * dim, sg, mix);
                let b = lerp((s.b as f32 / 255.0) * dim, sb, mix);
                dst[i] = SkColor::rgba(
                    clamp_to_byte(r * 255.0),
                    clamp_to_byte(g * 255.0),
                    clamp_to_byte(b * 255.0),
                    s.a,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
