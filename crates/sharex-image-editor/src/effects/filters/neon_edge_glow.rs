//! `NeonEdgeGlowImageEffect` (`Core/ImageEffects/Filters/NeonEdgeGlowImageEffect.cs`).

use crate::effects::prelude::*;

/// `NeonEdgeGlowImageEffect`.
#[derive(Clone, Debug)]
pub struct NeonEdgeGlowImageEffect {
    pub edge_strength: f32,
    pub threshold: i32,
    pub glow_radius: f32,
    pub glow_intensity: f32,
    pub base_dim: f32,
    pub neon_color: SkColor,
}

impl Default for NeonEdgeGlowImageEffect {
    fn default() -> Self {
        NeonEdgeGlowImageEffect {
            edge_strength: 2.2,
            threshold: 36,
            glow_radius: 8.0,
            glow_intensity: 120.0,
            base_dim: 30.0,
            neon_color: SkColor::argb(0xFF00F0FF),
        }
    }
}

impl EffectImpl for NeonEdgeGlowImageEffect {
    fn info() -> Info {
        Info::new(
            "neon_edge_glow",
            "Neon edge glow",
            EffectCategory::Filters,
            "app/zap",
        )
        .desc("Detects edges and applies a neon glow effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "edge_strength",
                "Edge strength",
                0.1,
                6.0,
                2.2,
                |e: &mut Self, v| e.edge_strength = v,
            ),
            int_slider::<Self>("threshold", "Threshold", 0, 255, 36, |e: &mut Self, v| {
                e.threshold = v
            }),
            float_slider::<Self>(
                "glow_radius",
                "Glow radius",
                0.0,
                40.0,
                8.0,
                |e: &mut Self, v| e.glow_radius = v,
            ),
            float_slider::<Self>(
                "glow_intensity",
                "Glow intensity",
                0.0,
                250.0,
                120.0,
                |e: &mut Self, v| e.glow_intensity = v,
            ),
            float_slider::<Self>(
                "base_dim",
                "Base dim",
                0.0,
                100.0,
                30.0,
                |e: &mut Self, v| e.base_dim = v,
            ),
            color_param::<Self>(
                "neon_color",
                "Neon color",
                SkColor::argb(0xFF00F0FF),
                |e: &mut Self, v| e.neon_color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let edge_strength = clampf(self.edge_strength, 0.1, 6.0);
        let threshold = self.threshold.clamp(0, 255);
        let glow_radius = clampf(self.glow_radius, 0.0, 40.0);
        let glow_intensity = clampf(self.glow_intensity, 0.0, 250.0) / 100.0;
        let base_dim = clampf(self.base_dim, 0.0, 100.0) / 100.0;
        if edge_strength <= 0.0 && glow_intensity <= 0.0 && base_dim <= 0.0 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let neon = self.neon_color;
        let neon_a = neon.a as f32 / 255.0;
        let src = pixels(source);
        let luma: Vec<i32> = src
            .iter()
            .map(|c| ((c.r as i32 * 77) + (c.g as i32 * 150) + (c.b as i32 * 29)) >> 8)
            .collect();
        let l = |x: i32, y: i32| luma[(y * width + x) as usize];
        let mut core = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let (y0, y2) = (clamp_int(y - 1, 0, bottom), clamp_int(y + 1, 0, bottom));
            for x in 0..width {
                let (x0, x2) = (clamp_int(x - 1, 0, right), clamp_int(x + 1, 0, right));
                let gx = (-l(x0, y0) + l(x2, y0))
                    + (-2 * l(x0, y) + 2 * l(x2, y))
                    + (-l(x0, y2) + l(x2, y2));
                let gy = (-l(x0, y0) - 2 * l(x, y0) - l(x2, y0))
                    + (l(x0, y2) + 2 * l(x, y2) + l(x2, y2));
                let m = (((gx * gx) + (gy * gy)) as f32).sqrt() * edge_strength;
                let mask = if m <= threshold as f32 {
                    0.0
                } else {
                    let t = clamp01((m - threshold as f32) / 1f32.max(255.0 - threshold as f32));
                    t * t * (3.0 - (2.0 * t))
                } * neon_a;
                let s = src[(y * width + x) as usize];
                core[(y * width + x) as usize] = SkColor::rgba(
                    clamp_to_byte(neon.r as f32 * mask),
                    clamp_to_byte(neon.g as f32 * mask),
                    clamp_to_byte(neon.b as f32 * mask),
                    clamp_to_byte(s.a as f32 * mask),
                );
            }
        }
        let core_bmp = from_pixels(width as u32, height as u32, &core);
        let glow = pixels(&create_blurred_clamp(&core_bmp, glow_radius));
        let base = 1.0 - base_dim;
        let core_gain = 0.85 + (0.35 * glow_intensity.min(1.6));
        let dst: Vec<SkColor> = (0..src.len())
            .map(|i| {
                let (s, c, g) = (src[i], core[i], glow[i]);
                let r =
                    (s.r as f32 * base) + (c.r as f32 * core_gain) + (g.r as f32 * glow_intensity);
                let gg =
                    (s.g as f32 * base) + (c.g as f32 * core_gain) + (g.g as f32 * glow_intensity);
                let b =
                    (s.b as f32 * base) + (c.b as f32 * core_gain) + (g.b as f32 * glow_intensity);
                SkColor::rgba(clamp_to_byte(r), clamp_to_byte(gg), clamp_to_byte(b), s.a)
            })
            .collect();
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
