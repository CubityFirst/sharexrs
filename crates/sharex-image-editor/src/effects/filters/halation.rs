//! `HalationImageEffect` (`Core/ImageEffects/Filters/HalationImageEffect.cs`).

use crate::effects::prelude::*;

/// `HalationImageEffect`.
#[derive(Clone, Debug)]
pub struct HalationImageEffect {
    pub threshold: f32,
    pub radius: f32,
    pub strength: f32,
    pub warmth: f32,
}

impl Default for HalationImageEffect {
    fn default() -> Self {
        HalationImageEffect {
            threshold: 72.0,
            radius: 18.0,
            strength: 60.0,
            warmth: 70.0,
        }
    }
}

impl EffectImpl for HalationImageEffect {
    fn info() -> Info {
        Info::new("halation", "Halation", EffectCategory::Filters, "image-effect/sunrise")
            .desc("Simulates film halation where bright areas bleed warm light into surrounding regions.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("threshold", "Threshold", 0.0, 100.0, 72.0, |e: &mut Self, v| e.threshold = v),
            float_slider::<Self>("radius", "Radius", 1.0, 100.0, 18.0, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>("strength", "Strength", 0.0, 150.0, 60.0, |e: &mut Self, v| e.strength = v),
            float_slider::<Self>("warmth", "Warmth", 0.0, 100.0, 70.0, |e: &mut Self, v| e.warmth = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let threshold = clampf(self.threshold, 0.0, 100.0) / 100.0;
        let radius = clampf(self.radius, 1.0, 100.0);
        let strength = clampf(self.strength, 0.0, 150.0) / 100.0;
        let warmth = clampf(self.warmth, 0.0, 100.0) / 100.0;
        if strength <= 0.0 {
            return Ok(source.clone());
        }
        let src = pixels(source);
        let low = clampf(threshold - 0.18, 0.0, 1.0);
        let high = clampf(threshold + 0.12, 0.0, 1.0);
        let rb = lerp(1.20, 1.85, warmth);
        let gb = lerp(0.55, 0.85, warmth);
        let bb = lerp(0.35, 0.15, warmth);
        let halo_src: Vec<SkColor> = src
            .iter()
            .map(|c| {
                let l = ((0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32)) / 255.0;
                let mask = smooth_step(low, high, l);
                if mask <= 0.0 {
                    return color::EMPTY;
                }
                let gain = mask * (0.40 + (0.60 * l.powf(1.4)));
                SkColor::rgba(
                    clamp_to_byte(c.r as f32 * gain * rb),
                    clamp_to_byte(c.g as f32 * gain * gb),
                    clamp_to_byte(c.b as f32 * gain * bb),
                    clamp_to_byte(c.a as f32 * mask),
                )
            })
            .collect();
        let halo = pixels(&create_blurred_clamp(&from_pixels(source.width(), source.height(), &halo_src), radius));
        let ch = |s: u8, h: f32| {
            let s = s as f32 / 255.0;
            let mut v = 1.0 - ((1.0 - s) * (1.0 - clampf(h, 0.0, 1.0)));
            if h > 1.0 {
                v += (h - 1.0) * 0.35;
            }
            clamp_to_byte(v * 255.0)
        };
        let dst: Vec<SkColor> = src
            .iter()
            .zip(&halo)
            .map(|(s, h)| {
                let hr = (h.r as f32 / 255.0) * strength * (1.0 + (warmth * 0.5));
                let hg = (h.g as f32 / 255.0) * strength * (0.85 + (warmth * 0.15));
                let hb = (h.b as f32 / 255.0) * strength * (0.65 - (warmth * 0.25));
                SkColor::rgba(ch(s.r, hr), ch(s.g, hg), ch(s.b, hb), s.a)
            })
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}
