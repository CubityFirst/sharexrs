//! `LiquidMercuryImageEffect` (`Core/ImageEffects/Filters/LiquidMercuryImageEffect.cs`).

use crate::effects::prelude::*;

/// `LiquidMercuryImageEffect`.
#[derive(Clone, Debug)]
pub struct LiquidMercuryImageEffect {
    pub reflection: f32,
    pub ripple: f32,
    pub shine: f32,
    pub fluidity: f32,
    pub depth: f32,
}

impl Default for LiquidMercuryImageEffect {
    fn default() -> Self {
        LiquidMercuryImageEffect {
            reflection: 78.0,
            ripple: 42.0,
            shine: 82.0,
            fluidity: 55.0,
            depth: 65.0,
        }
    }
}

impl EffectImpl for LiquidMercuryImageEffect {
    fn info() -> Info {
        Info::new("liquid_mercury", "Liquid mercury", EffectCategory::Filters, "image-effect/droplet")
            .desc("Transforms the image into a reflective liquid mercury surface.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("reflection", "Reflection", 0.0, 100.0, 78.0, |e: &mut Self, v| e.reflection = v),
            float_slider::<Self>("ripple", "Ripple", 0.0, 100.0, 42.0, |e: &mut Self, v| e.ripple = v),
            float_slider::<Self>("shine", "Shine", 0.0, 100.0, 82.0, |e: &mut Self, v| e.shine = v),
            float_slider::<Self>("fluidity", "Fluidity", 0.0, 100.0, 55.0, |e: &mut Self, v| e.fluidity = v),
            float_slider::<Self>("depth", "Depth", 0.0, 100.0, 65.0, |e: &mut Self, v| e.depth = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let reflection = clampf(self.reflection, 0.0, 100.0) / 100.0;
        let ripple = clampf(self.ripple, 0.0, 100.0) / 100.0;
        let shine = clampf(self.shine, 0.0, 100.0) / 100.0;
        let fluidity = clampf(self.fluidity, 0.0, 100.0) / 100.0;
        let depth = clampf(self.depth, 0.0, 100.0) / 100.0;
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let inv_w = 1.0 / 1.max(width - 1) as f32;
        let inv_h = 1.0 / 1.max(height - 1) as f32;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let lum = |c: SkColor| (0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32);
        let p = |x: i32, y: i32| src[(y * width + x) as usize];
        for y in 0..height {
            let row = y * width;
            let v = y as f32 * inv_h;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let l = lum(s) / 255.0;
                let ll = lum(p(0.max(x - 1), y)) / 255.0;
                let lr = lum(p((width - 1).min(x + 1), y)) / 255.0;
                let lt = lum(p(x, 0.max(y - 1))) / 255.0;
                let lb = lum(p(x, (height - 1).min(y + 1))) / 255.0;
                let gx = lr - ll;
                let gy = lb - lt;
                let u = x as f32 * inv_w;
                let wa = ((u * 12.0) + (v * 6.5) + 0.8).sin();
                let wb = ((u * 7.5) - (v * 14.0) - 1.4).cos();
                let field = ((wa * 0.6) + (wb * 0.4)) * ripple;
                let mut nx = (-gx * (1.35 + (depth * 3.8))) + (field * 0.45);
                let mut ny = (-gy * (1.35 + (depth * 3.8))) + (field * 0.22);
                let mut nz = 1f32;
                let len = ((nx * nx) + (ny * ny) + (nz * nz)).sqrt();
                if len <= 0.0001 {
                    nx = 0.0;
                    ny = 0.0;
                    nz = 1.0;
                } else {
                    let inv = 1.0 / len;
                    nx *= inv;
                    ny *= inv;
                    nz *= inv;
                }
                let ru = u + (nx * (0.55 + (fluidity * 0.55))) + (wb * fluidity * 0.05);
                let rv = v - (ny * (0.85 + (fluidity * 0.45))) + (wa * fluidity * 0.03);
                let env = {
                    let a = ((ru * 11.5) + (rv * 2.1) + 0.7).sin();
                    let b = ((ru * 18.5) - (rv * 8.2) - 1.1).cos();
                    let c = (((ru * 0.82) + (rv * 1.28)) * 12.0 + 2.6).sin();
                    (((a * 0.42) + (b * 0.36) + (c * 0.22)) * 0.5) + 0.5
                };
                let diffuse = clamp01((nz * 0.65) + 0.15);
                let fresnel = (1.0 - clamp01(nz)).powf(2.7);
                let specular = clamp01((nx * -0.22) + (ny * -0.32) + (nz * 0.92)).powf(12.0 + (shine * 42.0));
                let mut base = clamp01((l * 0.20) + (diffuse * 0.26) + (env * (0.32 + (reflection * 0.42))));
                base = clamp01(((base - 0.5) * (1.15 + (depth * 1.55))) + 0.5);
                let mut silver = lerp(0.06, 0.94, base);
                silver = lerp(silver, 1.0, specular * (0.34 + (shine * 0.46)));
                silver = lerp(silver, 1.0, fresnel * (0.20 + (reflection * 0.22)));
                let cool = 0.01 + (env * 0.03);
                let r = clamp01(silver - cool);
                let g = clamp01(silver);
                let b = clamp01(silver + (cool * 1.25));
                dst[(row + x) as usize] =
                    SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
