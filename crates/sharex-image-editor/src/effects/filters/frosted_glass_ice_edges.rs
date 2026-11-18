//! `FrostedGlassIceEdgesImageEffect` (`Core/ImageEffects/Filters/FrostedGlassIceEdgesImageEffect.cs`).

use crate::effects::prelude::*;

/// `FrostedGlassIceEdgesImageEffect`.
#[derive(Clone, Debug)]
pub struct FrostedGlassIceEdgesImageEffect {
    pub distortion: f32,
    pub blur: f32,
    pub edge_threshold: i32,
    pub ice_strength: f32,
    pub tint: f32,
    pub seed: i32,
}

impl Default for FrostedGlassIceEdgesImageEffect {
    fn default() -> Self {
        FrostedGlassIceEdgesImageEffect {
            distortion: 10.0,
            blur: 35.0,
            edge_threshold: 28,
            ice_strength: 75.0,
            tint: 35.0,
            seed: 1337,
        }
    }
}

impl EffectImpl for FrostedGlassIceEdgesImageEffect {
    fn info() -> Info {
        Info::new(
            "frosted_glass_ice_edges",
            "Frosted glass + ice edges",
            EffectCategory::Filters,
            "image-effect/snowflake",
        )
        .desc("Combines frosted glass distortion with icy edge highlights and cool tinting.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "distortion",
                "Distortion",
                0.0,
                30.0,
                10.0,
                |e: &mut Self, v| e.distortion = v,
            ),
            float_slider::<Self>("blur", "Blur", 0.0, 100.0, 35.0, |e: &mut Self, v| {
                e.blur = v
            }),
            int_slider::<Self>(
                "edge_threshold",
                "Edge threshold",
                1,
                120,
                28,
                |e: &mut Self, v| e.edge_threshold = v,
            ),
            float_slider::<Self>(
                "ice_strength",
                "Ice strength",
                0.0,
                100.0,
                75.0,
                |e: &mut Self, v| e.ice_strength = v,
            ),
            float_slider::<Self>("tint", "Tint", 0.0, 100.0, 35.0, |e: &mut Self, v| {
                e.tint = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let distortion = clampf(self.distortion, 0.0, 30.0);
        let blur_mix = clampf(self.blur, 0.0, 100.0) / 100.0;
        let edge_threshold = self.edge_threshold.clamp(1, 120);
        let ice = clampf(self.ice_strength, 0.0, 100.0) / 100.0;
        let tint = clampf(self.tint, 0.0, 100.0) / 100.0;
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let lum: Vec<f32> = src
            .iter()
            .map(|c| {
                ((0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32)) / 255.0
            })
            .collect();
        let seed = self.seed;
        for y in 0..height {
            let row = y * width;
            let y_up = if y > 0 { y - 1 } else { 0 };
            let y_down = if y < bottom { y + 1 } else { bottom };
            for x in 0..width {
                let cx = (hash01(x / 9, y / 9, seed) * 2.0) - 1.0;
                let cy = (hash01(x / 9, y / 9, seed ^ 977) * 2.0) - 1.0;
                let fine = (hash01(x, y, seed ^ 4099) * 2.0) - 1.0;
                let sx = x as f32 + (((cx * 0.75) + (fine * 0.25)) * distortion);
                let sy = y as f32 + (((cy * 0.75) - (fine * 0.25)) * distortion);
                let s = bilinear_sample(&src, width, height, sx, sy);
                let (mut r, mut g, mut b, mut a) = (s.r as f32, s.g as f32, s.b as f32, s.a as f32);
                if blur_mix > 0.001 {
                    let s1 = bilinear_sample(&src, width, height, sx - 1.0, sy);
                    let s2 = bilinear_sample(&src, width, height, sx + 1.0, sy);
                    let s3 = bilinear_sample(&src, width, height, sx, sy - 1.0);
                    let s4 = bilinear_sample(&src, width, height, sx, sy + 1.0);
                    let sum = |f: fn(SkColor) -> u8| {
                        (f(s1) as i32 + f(s2) as i32 + f(s3) as i32 + f(s4) as i32) as f32 * 0.25
                    };
                    r = lerp(r, sum(|c| c.r), blur_mix * 0.85);
                    g = lerp(g, sum(|c| c.g), blur_mix * 0.85);
                    b = lerp(b, sum(|c| c.b), blur_mix * 0.85);
                    a = lerp(a, sum(|c| c.a), blur_mix * 0.6);
                }
                let x_left = if x > 0 { x - 1 } else { 0 };
                let x_right = if x < right { x + 1 } else { right };
                let gx = (lum[(row + x_right) as usize] - lum[(row + x_left) as usize]).abs();
                let gy =
                    (lum[(y_down * width + x) as usize] - lum[(y_up * width + x) as usize]).abs();
                let metric = (gx + gy) * 255.0;
                let mut edge =
                    (metric - edge_threshold as f32) / 1f32.max(255.0 - edge_threshold as f32);
                edge = clamp01(edge) * ice;
                let cool = (tint * 0.45) + (edge * 0.55);
                r = lerp(r, (r * 0.82) + 16.0, cool);
                g = lerp(g, (g * 0.95) + 24.0, cool);
                b = lerp(b, (b * 1.08) + 56.0, cool);
                if edge > 0.001 {
                    let spec = hash01(x, y, seed ^ 12345).powf(18.0) * edge;
                    let glow = edge * (0.55 + (spec * 0.8));
                    r += 120.0 * glow;
                    g += 142.0 * glow;
                    b += 175.0 * glow;
                }
                dst[(row + x) as usize] = SkColor::rgba(
                    clamp_to_byte(r),
                    clamp_to_byte(g),
                    clamp_to_byte(b),
                    clamp_to_byte(a),
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
