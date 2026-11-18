//! `CircuitBoardBorderImageEffect` (`Core/ImageEffects/Filters/CircuitBoardBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `CircuitBoardBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct CircuitBoardBorderImageEffect {
    pub border_size: i32,
    pub board_color: SkColor,
    pub trace_color: SkColor,
    pub trace_density: f32,
    pub glow: bool,
    pub inner_line: bool,
}

impl Default for CircuitBoardBorderImageEffect {
    fn default() -> Self {
        CircuitBoardBorderImageEffect {
            border_size: 64,
            board_color: SkColor::argb(0xFF0C3A20),
            trace_color: SkColor::argb(0xFFB87333),
            trace_density: 55.0,
            glow: true,
            inner_line: true,
        }
    }
}

impl EffectImpl for CircuitBoardBorderImageEffect {
    fn info() -> Info {
        Info::new("circuit_board_border", "Circuit board border", EffectCategory::Filters, "image-effect/circuit-board")
            .desc("Adds a PCB circuit board border with copper traces, vias, and pads on a dark substrate.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 20, 300, 64, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("board_color", "Board color", SkColor::argb(0xFF0C3A20), |e: &mut Self, v| e.board_color = v),
            color_param::<Self>("trace_color", "Trace color", SkColor::argb(0xFFB87333), |e: &mut Self, v| e.trace_color = v),
            float_slider::<Self>("trace_density", "Trace density", 10.0, 100.0, 55.0, |e: &mut Self, v| e.trace_density = v),
            bool_param::<Self>("glow", "Trace glow", true, |e: &mut Self, v| e.glow = v),
            bool_param::<Self>("inner_line", "Inner line", true, |e: &mut Self, v| e.inner_line = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
