//! `LensBlurImageEffect` (`Core/ImageEffects/Filters/LensBlurImageEffect.cs`).

use crate::effects::prelude::*;

/// `LensBlurImageEffect`.
#[derive(Clone, Debug)]
pub struct LensBlurImageEffect {
    pub radius: i32,
    pub highlight_threshold: f32,
    pub highlight_boost: f32,
}

impl Default for LensBlurImageEffect {
    fn default() -> Self {
        LensBlurImageEffect {
            radius: 8,
            highlight_threshold: 70.0,
            highlight_boost: 85.0,
        }
    }
}

impl EffectImpl for LensBlurImageEffect {
    fn info() -> Info {
        Info::new("lens_blur", "Lens blur (bokeh)", EffectCategory::Filters, "image-effect/aperture")
            .desc("Applies a disc-shaped lens blur with highlight boost.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Radius", 1, 15, 8, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>("highlight_threshold", "Highlight threshold", 0.0, 100.0, 70.0, |e: &mut Self, v| e.highlight_threshold = v),
            float_slider::<Self>("highlight_boost", "Highlight boost", 0.0, 200.0, 85.0, |e: &mut Self, v| e.highlight_boost = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let radius = self.radius.clamp(1, 15);
        let threshold = clampf(self.highlight_threshold, 0.0, 100.0) / 100.0;
        let boost = clampf(self.highlight_boost, 0.0, 200.0) / 100.0;
        let use_boost = boost > 0.001;
        let threshold_inv = 1.0 / 0.0001f32.max(1.0 - threshold);
        if radius <= 1 && boost <= 0.01 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let mut kernel = Vec::new();
        let r2 = radius * radius;
        let inv_r = 1.0 / radius as f32;
        for ky in -radius..=radius {
            for kx in -radius..=radius {
                let d2 = kx * kx + ky * ky;
                if d2 > r2 {
                    continue;
                }
                let distance = (d2 as f32).sqrt() * inv_r;
                kernel.push((kx, ky, 1.0 - (distance * 0.35)));
            }
        }
        let lum: Vec<f32> = src
            .iter()
            .map(|c| ((0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32)) / 255.0)
            .collect();
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let (mut sr, mut sg, mut sb, mut sa, mut sw) = (0f32, 0f32, 0f32, 0f32, 0f32);
                for &(kx, ky, kw) in &kernel {
                    let sx = (x + kx).clamp(0, right);
                    let sy = (y + ky).clamp(0, bottom);
                    let si = (sy * width + sx) as usize;
                    let c = src[si];
                    let mut hw = 1f32;
                    if use_boost {
                        let l = lum[si];
                        if l > threshold {
                            hw += (l - threshold) * threshold_inv * boost;
                        }
                    }
                    let w = kw * hw;
                    sr += c.r as f32 * w;
                    sg += c.g as f32 * w;
                    sb += c.b as f32 * w;
                    sa += c.a as f32 * w;
                    sw += w;
                }
                let i = (row + x) as usize;
                if sw <= 0.0001 {
                    dst[i] = src[i];
                    continue;
                }
                let inv = 1.0 / sw;
                dst[i] = SkColor::rgba(clamp_to_byte(sr * inv), clamp_to_byte(sg * inv), clamp_to_byte(sb * inv), clamp_to_byte(sa * inv));
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
