//! `GlassSphereImageEffect` (`Core/ImageEffects/Manipulations/GlassSphereImageEffect.cs`).

use crate::effects::prelude::*;

/// `GlassSphereImageEffect`.
#[derive(Clone, Debug)]
pub struct GlassSphereImageEffect {
    pub radius_percent: f32,
    pub refraction: f32,
}

impl Default for GlassSphereImageEffect {
    fn default() -> Self {
        GlassSphereImageEffect {
            radius_percent: 50.0,
            refraction: 1.5,
        }
    }
}

impl EffectImpl for GlassSphereImageEffect {
    fn info() -> Info {
        Info::new(
            "glass_sphere",
            "Glass sphere",
            EffectCategory::Manipulations,
            "image-effect/sphere",
        )
        .desc("Applies a spherical glass lens distortion at the center.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "radius",
                "Radius %",
                10.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.radius_percent = v,
            ),
            float_slider::<Self>(
                "refraction",
                "Refraction",
                0.1,
                2.0,
                1.5,
                |e: &mut Self, v| e.refraction = v,
            )
            .tick(0.05)
            .snap(false)
            .format("{}{0:0.00}"),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (w, h) = (source.width() as i32, source.height() as i32);
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        let radius = w.min(h) as f32 * self.radius_percent / 200.0;
        let radius2 = radius * radius;
        let src = pixels(source);
        let mut dst = src.clone();
        for y in 0..h {
            for x in 0..w {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist2 = dx * dx + dy * dy;
                if dist2 >= radius2 {
                    continue;
                }
                let dist = dist2.sqrt();
                let normalized = dist / radius;
                let theta = normalized.asin();
                let new_dist = radius * (theta * self.refraction).sin() / (theta + 0.0001).sin();
                let ratio = if dist > 0.0001 { new_dist / dist } else { 1.0 };
                let sx = (cx + dx * ratio) as i32;
                let sy = (cy + dy * ratio) as i32;
                if sx >= 0 && sx < w && sy >= 0 && sy < h {
                    dst[(y * w + x) as usize] = src[(sy * w + sx) as usize];
                }
            }
        }
        Ok(from_pixels(w as u32, h as u32, &dst))
    }
}
