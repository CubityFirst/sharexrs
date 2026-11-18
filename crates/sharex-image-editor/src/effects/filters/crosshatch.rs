//! `CrosshatchImageEffect` (`Core/ImageEffects/Filters/CrosshatchImageEffect.cs`).

use crate::effects::prelude::*;

/// `CrosshatchImageEffect`.
#[derive(Clone, Debug)]
pub struct CrosshatchImageEffect {
    pub line_spacing: i32,
    pub line_thickness: f32,
    pub contrast: f32,
    pub layer_count: i32,
}

impl Default for CrosshatchImageEffect {
    fn default() -> Self {
        CrosshatchImageEffect {
            line_spacing: 8,
            line_thickness: 1.2,
            contrast: 120.0,
            layer_count: 4,
        }
    }
}

impl EffectImpl for CrosshatchImageEffect {
    fn info() -> Info {
        Info::new(
            "crosshatch",
            "Crosshatch",
            EffectCategory::Filters,
            "app/hash",
        )
        .desc("Renders the image as a crosshatch pen drawing.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>(
                "lineSpacing",
                "Line spacing",
                4,
                24,
                8,
                |e: &mut Self, v| e.line_spacing = v,
            ),
            float_slider::<Self>(
                "lineThickness",
                "Line thickness",
                0.5,
                4.0,
                1.2000000476837158,
                |e: &mut Self, v| e.line_thickness = v,
            ),
            float_slider::<Self>(
                "contrast",
                "Contrast",
                50.0,
                200.0,
                120.0,
                |e: &mut Self, v| e.contrast = v,
            ),
            int_slider::<Self>("layerCount", "Layer count", 1, 6, 4, |e: &mut Self, v| {
                e.layer_count = v
            }),
        ]
    }

    // ShareX's 0.7071, not 1/sqrt(2).
    #[allow(clippy::approx_constant)]
    fn apply(&self, source: &Bitmap) -> EffectResult {
        let spacing = self.line_spacing.clamp(4, 24) as f32;
        let thickness = clampf(self.line_thickness, 0.5, 4.0);
        let contrast = clampf(self.contrast, 50.0, 200.0) / 100.0;
        let layers = self.layer_count.clamp(1, 6);
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let th = [0.14f32, 0.28, 0.42, 0.58, 0.74, 0.88];
        let is_line = |value: f32| {
            let mut m = value % spacing;
            if m < 0.0 {
                m += spacing;
            }
            m.min(spacing - m) <= (thickness * 0.5)
        };
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let mut lum =
                    ((0.2126 * s.r as f32) + (0.7152 * s.g as f32) + (0.0722 * s.b as f32)) / 255.0;
                lum = clampf(((lum - 0.5) * contrast) + 0.5, 0.0, 1.0);
                let dark = 1.0 - lum;
                let (xf, yf) = (x as f32, y as f32);
                let mut active = 0;
                if layers >= 1 && dark > th[0] && is_line(((x + y) as f32 * 0.7071).abs()) {
                    active += 1;
                }
                if layers >= 2 && dark > th[1] && is_line(((x - y) as f32 * 0.7071).abs()) {
                    active += 1;
                }
                if layers >= 3 && dark > th[2] && is_line(yf) {
                    active += 1;
                }
                if layers >= 4 && dark > th[3] && is_line(xf) {
                    active += 1;
                }
                if layers >= 5 && dark > th[4] && is_line((xf * 0.5) + yf) {
                    active += 1;
                }
                if layers >= 6 && dark > th[5] && is_line((xf * 0.5) - yf) {
                    active += 1;
                }
                let ink = active as f32 / layers as f32;
                let mut value = lum * (1.0 - (ink * 0.94));
                value = clampf(value.min(lum + 0.06), 0.0, 1.0);
                let v = clamp_to_byte(value * 255.0);
                dst[(row + x) as usize] = SkColor::rgba(v, v, v, s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
