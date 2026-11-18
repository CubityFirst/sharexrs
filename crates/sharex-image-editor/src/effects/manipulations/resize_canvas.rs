//! `ResizeCanvasImageEffect` (`Core/ImageEffects/Manipulations/ResizeCanvasImageEffect.cs`).

use crate::effects::prelude::*;

/// `ResizeCanvasImageEffect`.
#[derive(Clone, Debug)]
pub struct ResizeCanvasImageEffect {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
    pub background_color: SkColor,
}

impl Default for ResizeCanvasImageEffect {
    fn default() -> Self {
        ResizeCanvasImageEffect {
            top: 0,
            right: 0,
            bottom: 0,
            left: 0,
            background_color: SkColor::argb(0x00FFFFFF),
        }
    }
}

impl EffectImpl for ResizeCanvasImageEffect {
    fn info() -> Info {
        Info::new("resize_canvas", "Resize canvas", EffectCategory::Manipulations, "app/expand")
            .desc("Resizes the canvas.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("top", "Top", -10000, 10000, 0, |e: &mut Self, v| e.top = v),
            int_numeric::<Self>("right", "Right", -10000, 10000, 0, |e: &mut Self, v| e.right = v),
            int_numeric::<Self>("bottom", "Bottom", -10000, 10000, 0, |e: &mut Self, v| e.bottom = v),
            int_numeric::<Self>("left", "Left", -10000, 10000, 0, |e: &mut Self, v| e.left = v),
            color_param::<Self>("background_color", "Background color", SkColor::argb(0x00FFFFFF), |e: &mut Self, v| e.background_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
