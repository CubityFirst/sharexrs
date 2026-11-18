//! `SpinBlurImageEffect` (`Core/ImageEffects/Filters/SpinBlurImageEffect.cs`).

use crate::effects::prelude::*;

/// `SpinBlurImageEffect`.
#[derive(Clone, Debug)]
pub struct SpinBlurImageEffect {
    pub angle: f32,
    pub samples: i32,
    pub center_x: f32,
    pub center_y: f32,
}

impl Default for SpinBlurImageEffect {
    fn default() -> Self {
        SpinBlurImageEffect {
            angle: 20.0,
            samples: 24,
            center_x: 50.0,
            center_y: 50.0,
        }
    }
}

impl EffectImpl for SpinBlurImageEffect {
    fn info() -> Info {
        Info::new("spin_blur", "Spin blur", EffectCategory::Filters, "image-effect/rotate-clockwise")
            .desc("Applies a rotational motion blur around a center point.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("angle", "Angle", 0.0, 180.0, 20.0, |e: &mut Self, v| e.angle = v),
            int_slider::<Self>("samples", "Samples", 4, 64, 24, |e: &mut Self, v| e.samples = v),
            float_slider::<Self>("center_x", "Center X", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x = v),
            float_slider::<Self>("center_y", "Center Y", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let angle = clampf(self.angle, 0.0, 180.0);
        let n = self.samples.clamp(4, 64) as usize;
        if angle <= 0.001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let cx = (clampf(self.center_x, 0.0, 100.0) / 100.0) * (width - 1) as f32;
        let cy = (clampf(self.center_y, 0.0, 100.0) / 100.0) * (height - 1) as f32;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let max_rad = angle * (std::f32::consts::PI / 180.0);
        let half = (n as i32 - 1) as f32 * 0.5;
        let inv_half = if half <= 0.0001 { 1.0 } else { 1.0 / half };
        let mut sins = vec![0f32; n];
        let mut coss = vec![0f32; n];
        let mut ws = vec![0f32; n];
        for i in 0..n {
            let t = (i as f32 - half) * inv_half;
            let theta = t * max_rad;
            sins[i] = theta.sin();
            coss[i] = theta.cos();
            ws[i] = 1.0 - (t.abs() * 0.7);
        }
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                if (dx * dx) + (dy * dy) < 0.25 {
                    dst[idx] = src[idx];
                    continue;
                }
                let (mut sr, mut sg, mut sb, mut sa, mut sw) = (0f32, 0f32, 0f32, 0f32, 0f32);
                for i in 0..n {
                    let (s, c, w) = (sins[i], coss[i], ws[i]);
                    let px = cx + ((dx * c) - (dy * s));
                    let py = cy + ((dx * s) + (dy * c));
                    let p = bilinear_sample(&src, width, height, px, py);
                    sr += p.r as f32 * w;
                    sg += p.g as f32 * w;
                    sb += p.b as f32 * w;
                    sa += p.a as f32 * w;
                    sw += w;
                }
                let inv = 1.0 / 0.0001f32.max(sw);
                dst[idx] = SkColor::rgba(clamp_to_byte(sr * inv), clamp_to_byte(sg * inv), clamp_to_byte(sb * inv), clamp_to_byte(sa * inv));
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
