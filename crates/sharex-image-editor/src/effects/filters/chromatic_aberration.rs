//! `ChromaticAberrationImageEffect` (`Core/ImageEffects/Filters/ChromaticAberrationImageEffect.cs`).

use crate::effects::prelude::*;

/// `ChromaticAberrationImageEffect`.
#[derive(Clone, Debug)]
pub struct ChromaticAberrationImageEffect {
    pub amount: f32,
    pub edge_start: f32,
    pub strength: f32,
}

impl Default for ChromaticAberrationImageEffect {
    fn default() -> Self {
        ChromaticAberrationImageEffect {
            amount: 8.0,
            edge_start: 0.2,
            strength: 75.0,
        }
    }
}

impl EffectImpl for ChromaticAberrationImageEffect {
    fn info() -> Info {
        Info::new(
            "chromatic_aberration",
            "Chromatic aberration",
            EffectCategory::Filters,
            "image-effect/aperture",
        )
        .desc("Simulates lens chromatic aberration with RGB channel splitting.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", 0.0, 40.0, 8.0, |e: &mut Self, v| {
                e.amount = v
            }),
            float_slider::<Self>(
                "edgeStart",
                "Edge start",
                0.0,
                0.949999988079071,
                0.20000000298023224,
                |e: &mut Self, v| e.edge_start = v,
            ),
            float_slider::<Self>(
                "strength",
                "Strength",
                0.0,
                100.0,
                75.0,
                |e: &mut Self, v| e.strength = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let amount = clampf(self.amount, 0.0, 40.0);
        let edge_start = clampf(self.edge_start, 0.0, 0.95);
        let strength = clampf(self.strength, 0.0, 100.0) / 100.0;
        if amount <= 0.0 || strength <= 0.0 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let cx = right as f32 * 0.5;
        let cy = bottom as f32 * 0.5;
        let inv_cx = if cx > 0.0 { 1.0 / cx } else { 0.0 };
        let inv_cy = if cy > 0.0 { 1.0 / cy } else { 0.0 };
        const INV_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;
        let lum = |c: SkColor| {
            ((0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32)) / 255.0
        };
        for y in 0..height {
            let row = y * width;
            let dyn_ = (y as f32 - cy) * inv_cy;
            for x in 0..width {
                let i = (row + x) as usize;
                let o = src[i];
                let dxn = (x as f32 - cx) * inv_cx;
                let d01 = ((dxn * dxn) + (dyn_ * dyn_)).sqrt() * INV_SQRT2;
                let radial = smooth_step(edge_start, 1.0, d01);
                if radial <= 0.0 {
                    dst[i] = o;
                    continue;
                }
                let len = ((dxn * dxn) + (dyn_ * dyn_)).sqrt();
                let dir_x = if len > 0.0001 { dxn / len } else { 0.0 };
                let dir_y = if len > 0.0001 { dyn_ / len } else { 0.0 };
                let mix = strength * radial;
                if mix <= 0.0001 {
                    dst[i] = o;
                    continue;
                }
                let shift = amount * radial;
                let sr = bilinear_sample(
                    &src,
                    width,
                    height,
                    x as f32 + (dir_x * shift),
                    y as f32 + (dir_y * shift),
                );
                let sb = bilinear_sample(
                    &src,
                    width,
                    height,
                    x as f32 - (dir_x * shift),
                    y as f32 - (dir_y * shift),
                );
                let split = smooth_step(0.01, 0.30, (lum(sr) - lum(sb)).abs());
                let boost = mix * (0.85 + (1.35 * split));
                let red = lerp(o.r as f32, sr.r as f32, mix)
                    + ((sr.r as i32 - o.r as i32) as f32 * boost);
                let green = lerp(
                    o.g as f32,
                    (sr.g as i32 + o.g as i32 + sb.g as i32) as f32 / 3.0,
                    mix * 0.06,
                );
                let blue = lerp(o.b as f32, sb.b as f32, mix)
                    + ((sb.b as i32 - o.b as i32) as f32 * boost);
                let alpha = lerp(
                    o.a as f32,
                    (sr.a as i32 + o.a as i32 + sb.a as i32) as f32 / 3.0,
                    mix,
                );
                dst[i] = SkColor::rgba(
                    clamp_to_byte(red),
                    clamp_to_byte(green),
                    clamp_to_byte(blue),
                    clamp_to_byte(alpha),
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
