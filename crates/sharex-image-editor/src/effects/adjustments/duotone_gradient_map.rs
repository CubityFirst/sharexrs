//! `DuotoneGradientMapImageEffect` (`Core/ImageEffects/Adjustments/DuotoneGradientMapImageEffect.cs`).

use crate::effects::prelude::*;

/// `DuotoneGradientMapImageEffect`.
#[derive(Clone, Debug)]
pub struct DuotoneGradientMapImageEffect {
    pub shadow_color: SkColor,
    pub midtone_color: SkColor,
    pub highlight_color: SkColor,
    pub contrast: f32,
    pub gamma: f32,
    pub blend: f32,
}

impl Default for DuotoneGradientMapImageEffect {
    fn default() -> Self {
        DuotoneGradientMapImageEffect {
            shadow_color: SkColor::argb(0xFF181C4E),
            midtone_color: SkColor::argb(0xFFB63C84),
            highlight_color: SkColor::argb(0xFFFFE084),
            contrast: 110.0,
            gamma: 1.0,
            blend: 100.0,
        }
    }
}

impl EffectImpl for DuotoneGradientMapImageEffect {
    fn info() -> Info {
        Info::new(
            "duotone_gradient_map",
            "Duotone / Gradient map",
            EffectCategory::Adjustments,
            "app/blend",
        )
        .desc("Maps grayscale tones to a custom multi-color gradient.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            color_param::<Self>(
                "shadow_color",
                "Shadow color",
                SkColor::argb(0xFF181C4E),
                |e: &mut Self, v| e.shadow_color = v,
            ),
            color_param::<Self>(
                "midtone_color",
                "Midtone color",
                SkColor::argb(0xFFB63C84),
                |e: &mut Self, v| e.midtone_color = v,
            ),
            color_param::<Self>(
                "highlight_color",
                "Highlight color",
                SkColor::argb(0xFFFFE084),
                |e: &mut Self, v| e.highlight_color = v,
            ),
            float_slider::<Self>(
                "contrast",
                "Contrast",
                50.0,
                200.0,
                110.0,
                |e: &mut Self, v| e.contrast = v,
            ),
            float_slider::<Self>("gamma", "Gamma", 0.1, 5.0, 1.0, |e: &mut Self, v| {
                e.gamma = v
            })
            .tick(0.1)
            .snap(false)
            .format("{}{0:0.0}"),
            float_slider::<Self>("blend", "Blend", 0.0, 100.0, 100.0, |e: &mut Self, v| {
                e.blend = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let contrast = clampf(self.contrast, 50.0, 200.0) / 100.0;
        let gamma = clampf(self.gamma, 0.3, 3.0);
        let blend = clampf(self.blend, 0.0, 100.0) / 100.0;
        if source.is_empty() {
            return Ok(source.clone());
        }
        let src = pixels(source);
        let dst: Vec<SkColor> = src
            .iter()
            .map(|s| {
                let mut lum =
                    ((0.2126 * s.r as f32) + (0.7152 * s.g as f32) + (0.0722 * s.b as f32)) / 255.0;
                lum = clampf(((lum - 0.5) * contrast) + 0.5, 0.0, 1.0);
                lum = lum.powf(1.0 / gamma);
                let mapped = sample_three(
                    lum,
                    self.shadow_color,
                    self.midtone_color,
                    self.highlight_color,
                );
                let r = lerp(s.r as f32, mapped.r as f32, blend);
                let g = lerp(s.g as f32, mapped.g as f32, blend);
                let b = lerp(s.b as f32, mapped.b as f32, blend);
                SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), s.a)
            })
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}

// ---- port ----

fn sample_three(t: f32, shadows: SkColor, mids: SkColor, highs: SkColor) -> SkColor {
    let t = clampf(t, 0.0, 1.0);
    if t <= 0.5 {
        return lerp_opaque(shadows, mids, t * 2.0);
    }
    lerp_opaque(mids, highs, (t - 0.5) * 2.0)
}

fn lerp_opaque(a: SkColor, b: SkColor, t: f32) -> SkColor {
    SkColor::rgba(
        clamp_to_byte(lerp(a.r as f32, b.r as f32, t)),
        clamp_to_byte(lerp(a.g as f32, b.g as f32, t)),
        clamp_to_byte(lerp(a.b as f32, b.b as f32, t)),
        255,
    )
}
