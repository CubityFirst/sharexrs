//! `ChainLinkBorderImageEffect` (`Core/ImageEffects/Filters/ChainLinkBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `ChainLinkBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct ChainLinkBorderImageEffect {
    pub border_size: i32,
    pub wire_color: SkColor,
    pub bg_color: SkColor,
    pub cell_size: f32,
    pub wire_width: f32,
    pub highlight: bool,
}

impl Default for ChainLinkBorderImageEffect {
    fn default() -> Self {
        ChainLinkBorderImageEffect {
            border_size: 52,
            wire_color: SkColor::argb(0xFFAAAFB4),
            bg_color: SkColor::argb(0xFF232830),
            cell_size: 20.0,
            wire_width: 2.5,
            highlight: true,
        }
    }
}

impl EffectImpl for ChainLinkBorderImageEffect {
    fn info() -> Info {
        Info::new("chain_link_border", "Chain link border", EffectCategory::Filters, "image-effect/fence")
            .desc("Adds a metallic chain-link fence border with interlocking wire loops and highlights.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 16, 250, 52, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("wire_color", "Wire color", SkColor::argb(0xFFAAAFB4), |e: &mut Self, v| e.wire_color = v),
            color_param::<Self>("bg_color", "Background color", SkColor::argb(0xFF232830), |e: &mut Self, v| e.bg_color = v),
            float_slider::<Self>("cell_size", "Cell size", 8.0, 60.0, 20.0, |e: &mut Self, v| e.cell_size = v),
            float_slider::<Self>("wire_width", "Wire width", 1.0, 8.0, 2.5, |e: &mut Self, v| e.wire_width = v),
            bool_param::<Self>("highlight", "Wire highlight", true, |e: &mut Self, v| e.highlight = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
