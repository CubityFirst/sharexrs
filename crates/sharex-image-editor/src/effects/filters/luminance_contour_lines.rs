//! `LuminanceContourLinesImageEffect` (`Core/ImageEffects/Filters/LuminanceContourLinesImageEffect.cs`).

use crate::effects::prelude::*;

/// `LuminanceContourLinesImageEffect`.
#[derive(Clone, Debug)]
pub struct LuminanceContourLinesImageEffect {
    pub levels: i32,
    pub line_width: f32,
    pub line_strength: f32,
    pub background_strength: f32,
    pub threshold: f32,
    pub invert: bool,
    pub line_color: SkColor,
}

impl Default for LuminanceContourLinesImageEffect {
    fn default() -> Self {
        LuminanceContourLinesImageEffect {
            levels: 12,
            line_width: 6.0,
            line_strength: 65.0,
            background_strength: 20.0,
            threshold: 0.0,
            invert: false,
            line_color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for LuminanceContourLinesImageEffect {
    fn info() -> Info {
        Info::new(
            "luminance_contour_lines",
            "Luminance contour lines",
            EffectCategory::Filters,
            "image-effect/contours",
        )
        .desc("Draws contour lines based on luminance quantization levels.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("levels", "Levels", 2, 64, 12, |e: &mut Self, v| {
                e.levels = v
            }),
            float_slider::<Self>(
                "line_width",
                "Line width",
                0.0,
                200.0,
                6.0,
                |e: &mut Self, v| e.line_width = v,
            ),
            float_slider::<Self>(
                "line_strength",
                "Line strength",
                0.0,
                100.0,
                65.0,
                |e: &mut Self, v| e.line_strength = v,
            ),
            float_slider::<Self>(
                "background_strength",
                "Background strength",
                0.0,
                100.0,
                20.0,
                |e: &mut Self, v| e.background_strength = v,
            ),
            float_slider::<Self>(
                "threshold",
                "Threshold",
                0.0,
                255.0,
                0.0,
                |e: &mut Self, v| e.threshold = v,
            ),
            bool_param::<Self>("invert", "Invert", false, |e: &mut Self, v| e.invert = v),
            color_param::<Self>(
                "line_color",
                "Line color",
                SkColor::argb(0xFF000000),
                |e: &mut Self, v| e.line_color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let levels = self.levels.clamp(2, 64);
        let bg = clampf(self.background_strength, 0.0, 100.0) / 100.0;
        let ls = clampf(self.line_strength, 0.0, 100.0) / 100.0;
        if ls <= 0.0 && bg <= 0.0 {
            return Ok(source.clone());
        }
        let bias = clampf(self.threshold, 0.0, 255.0) / 255.0;
        let lw = clampf(self.line_width, 0.0, 200.0) / 200.0;
        let feather = 0.04 + (0.22 * lw);
        let lc = self.line_color;
        let (la, lr, lg, lb) = (
            lc.a as f32 / 255.0,
            lc.r as f32 / 255.0,
            lc.g as f32 / 255.0,
            lc.b as f32 / 255.0,
        );
        if source.is_empty() {
            return Ok(source.clone());
        }
        let dst: Vec<SkColor> = pixels(source)
            .iter()
            .map(|s| {
                let mut l =
                    ((0.2126 * s.r as f32) + (0.7152 * s.g as f32) + (0.0722 * s.b as f32)) / 255.0;
                l = clamp01(l + bias);
                let scaled = l * levels as f32;
                let q = scaled.floor();
                let frac = scaled - q;
                let ql = clamp01((q + 0.5) / levels as f32);
                let d = frac.min(1.0 - frac);
                let t = smooth_step(0.0, feather, d);
                let mut mask = 1.0 - t;
                mask = mask * mask;
                if self.invert {
                    mask = 1.0 - mask;
                }
                let bl = clamp01(lerp(l, ql, bg));
                let mix = mask * ls * la;
                SkColor::rgba(
                    clamp_to_byte(lerp(bl, lr, mix) * 255.0),
                    clamp_to_byte(lerp(bl, lg, mix) * 255.0),
                    clamp_to_byte(lerp(bl, lb, mix) * 255.0),
                    s.a,
                )
            })
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}
