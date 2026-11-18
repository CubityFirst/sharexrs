//! `BloomImageEffect` (`Core/ImageEffects/Filters/BloomImageEffect.cs`).

use crate::effects::prelude::*;

/// `BloomImageEffect`.
#[derive(Clone, Debug)]
pub struct BloomImageEffect {
    pub threshold: f32,
    pub soft_knee: f32,
    pub radius: f32,
    pub intensity: f32,
}

impl Default for BloomImageEffect {
    fn default() -> Self {
        BloomImageEffect {
            threshold: 65.0,
            soft_knee: 35.0,
            radius: 24.0,
            intensity: 85.0,
        }
    }
}

impl EffectImpl for BloomImageEffect {
    fn info() -> Info {
        Info::new("bloom", "Bloom", EffectCategory::Filters, "image-effect/sparkle")
            .desc("Adds a soft glow bloom effect to bright areas.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("threshold", "Threshold", 0.0, 100.0, 65.0, |e: &mut Self, v| e.threshold = v),
            float_slider::<Self>("soft_knee", "Soft knee", 0.0, 100.0, 35.0, |e: &mut Self, v| e.soft_knee = v),
            float_slider::<Self>("radius", "Radius", 1.0, 100.0, 24.0, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>("intensity", "Intensity", 0.0, 200.0, 85.0, |e: &mut Self, v| e.intensity = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let threshold = clampf(self.threshold, 0.0, 100.0) / 100.0;
        let knee = clampf(self.soft_knee, 0.0, 100.0) / 100.0;
        let radius = clampf(self.radius, 1.0, 100.0);
        let intensity = clampf(self.intensity, 0.0, 200.0) / 100.0;
        if intensity <= 0.0 {
            return Ok(source.clone());
        }
        let src = pixels(source);
        let half = knee * 0.5;
        let low = clampf(threshold - half, 0.0, 1.0);
        let high = clampf(threshold + half, 0.0, 1.0);
        let lum = |c: &SkColor| ((0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32)) / 255.0;
        let bright: Vec<SkColor> = src
            .iter()
            .map(|c| {
                let l = lum(c);
                let mask = if knee > 0.0 { smooth_step(low, high, l) } else if l >= threshold { 1.0 } else { 0.0 };
                if mask <= 0.0 {
                    return color::EMPTY;
                }
                let gain = mask * (0.35 + (0.65 * l));
                SkColor::rgba(
                    clamp_to_byte(c.r as f32 * gain),
                    clamp_to_byte(c.g as f32 * gain),
                    clamp_to_byte(c.b as f32 * gain),
                    clamp_to_byte(c.a as f32 * gain),
                )
            })
            .collect();
        let bright_bmp = from_pixels(source.width(), source.height(), &bright);
        let blurred = create_blurred_clamp(&bright_bmp, radius);
        let bloom = pixels(&blurred);
        let ch = |s: u8, b: u8| {
            let s = s as f32 / 255.0;
            let b = (b as f32 / 255.0) * intensity;
            let mut v = 1.0 - ((1.0 - s) * (1.0 - clampf(b, 0.0, 1.0)));
            if b > 1.0 {
                v += (b - 1.0) * 0.40;
            }
            clamp_to_byte(v * 255.0)
        };
        let dst: Vec<SkColor> = src
            .iter()
            .zip(&bloom)
            .map(|(s, b)| SkColor::rgba(ch(s.r, b.r), ch(s.g, b.g), ch(s.b, b.b), s.a))
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}
