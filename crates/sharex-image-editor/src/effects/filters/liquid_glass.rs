//! `LiquidGlassImageEffect` (`Core/ImageEffects/Filters/LiquidGlassImageEffect.cs`).

use crate::effects::prelude::*;

/// `LiquidGlassImageEffect`.
#[derive(Clone, Debug)]
pub struct LiquidGlassImageEffect {
    pub distortion: f32,
    pub refraction: f32,
    pub chroma_shift: i32,
    pub gloss: f32,
    pub flow_scale: f32,
    pub seed: i32,
}

impl Default for LiquidGlassImageEffect {
    fn default() -> Self {
        LiquidGlassImageEffect {
            distortion: 9.0,
            refraction: 45.0,
            chroma_shift: 1,
            gloss: 40.0,
            flow_scale: 100.0,
            seed: 9421,
        }
    }
}

impl EffectImpl for LiquidGlassImageEffect {
    fn info() -> Info {
        Info::new(
            "liquid_glass",
            "Liquid glass",
            EffectCategory::Filters,
            "image-effect/glass",
        )
        .desc("Distorts the image as if viewed through liquid glass.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "distortion",
                "Distortion",
                0.0,
                35.0,
                9.0,
                |e: &mut Self, v| e.distortion = v,
            ),
            float_slider::<Self>(
                "refraction",
                "Refraction",
                0.0,
                100.0,
                45.0,
                |e: &mut Self, v| e.refraction = v,
            ),
            int_slider::<Self>(
                "chroma_shift",
                "Chroma shift",
                0,
                8,
                1,
                |e: &mut Self, v| e.chroma_shift = v,
            ),
            float_slider::<Self>("gloss", "Gloss", 0.0, 100.0, 40.0, |e: &mut Self, v| {
                e.gloss = v
            }),
            float_slider::<Self>(
                "flow_scale",
                "Flow scale",
                40.0,
                220.0,
                100.0,
                |e: &mut Self, v| e.flow_scale = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let dist_px = clampf(self.distortion, 0.0, 35.0);
        let refraction = clampf(self.refraction, 0.0, 100.0) / 100.0;
        let chroma = self.chroma_shift.clamp(0, 8);
        let gloss = clampf(self.gloss, 0.0, 100.0) / 100.0;
        let flow = clampf(self.flow_scale, 40.0, 220.0) / 100.0;
        if dist_px <= 0.0 && refraction <= 0.0 && chroma <= 0 && gloss <= 0.0 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let freq_x = 0.072 / flow;
        let freq_y = 0.061 / flow;
        let lum = |c: SkColor| {
            ((0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32)) / 255.0
        };
        let seed = self.seed;
        for y in 0..height {
            let row = y * width;
            let y_top = if y > 0 { y - 1 } else { 0 };
            let y_bottom = if y < height - 1 { y + 1 } else { height - 1 };
            for x in 0..width {
                let x_left = if x > 0 { x - 1 } else { 0 };
                let x_right = if x < width - 1 { x + 1 } else { width - 1 };
                let (xf, yf) = (x as f32, y as f32);
                let cn = (hash01(x / 18, y / 18, seed) * 2.0) - 1.0;
                let w1 =
                    ((xf * freq_x) + (yf * (freq_y * 0.45)) + (cn * 2.4) + (seed as f32 * 0.001))
                        .sin();
                let w2 = ((xf * (freq_x * 0.58)) - (yf * (freq_y * 1.12)) + (cn * 1.7)).cos();
                let w3 = (((x + y) as f32 * (freq_x * 0.62)) + (cn * 3.1)).sin();
                let mut ox = dist_px * ((0.52 * w1) + (0.26 * w3));
                let mut oy = dist_px * ((0.46 * w2) - (0.22 * w3));
                if refraction > 0.0 {
                    let gx = lum(src[(row + x_right) as usize]) - lum(src[(row + x_left) as usize]);
                    let gy = lum(src[(y_bottom * width + x) as usize])
                        - lum(src[(y_top * width + x) as usize]);
                    let edge = ((gx * gx) + (gy * gy)).sqrt();
                    let boost = 1.0 + (edge * 3.2 * refraction);
                    ox = (ox * boost) + (gx * refraction * dist_px * 3.0);
                    oy = (oy * boost) + (gy * refraction * dist_px * 3.0);
                }
                let sx = xf + ox;
                let sy = yf + oy;
                let (mut r, mut g, mut b, a);
                if chroma > 0 {
                    let shift = chroma as f32 * 0.75;
                    let cr = bilinear_sample(&src, width, height, sx + shift, sy - (shift * 0.15));
                    let cg = bilinear_sample(&src, width, height, sx, sy);
                    let cb = bilinear_sample(&src, width, height, sx - shift, sy + (shift * 0.15));
                    r = cr.r as f32;
                    g = cg.g as f32;
                    b = cb.b as f32;
                    a = cg.a;
                } else {
                    let s = bilinear_sample(&src, width, height, sx, sy);
                    r = s.r as f32;
                    g = s.g as f32;
                    b = s.b as f32;
                    a = s.a;
                }
                if refraction > 0.0 {
                    let n1 = bilinear_sample(&src, width, height, sx + 1.0, sy);
                    let n2 = bilinear_sample(&src, width, height, sx - 1.0, sy);
                    let n3 = bilinear_sample(&src, width, height, sx, sy + 1.0);
                    let n4 = bilinear_sample(&src, width, height, sx, sy - 1.0);
                    let sum = |f: fn(SkColor) -> u8| {
                        (f(n1) as i32 + f(n2) as i32 + f(n3) as i32 + f(n4) as i32) as f32 * 0.25
                    };
                    let mix = 0.06 + (0.18 * refraction);
                    r = lerp(r, sum(|c| c.r), mix);
                    g = lerp(g, sum(|c| c.g), mix);
                    b = lerp(b, sum(|c| c.b), mix);
                }
                let ridge = (w1 - w2).abs();
                let micro = w3.abs();
                let spec = 0f32.max(1.0 - (ridge * 1.35)).powf(6.0);
                let streak = 0f32.max(1.0 - (micro * 1.55)).powf(4.0);
                let l = ((0.2126 * r) + (0.7152 * g) + (0.0722 * b)) / 255.0;
                let hl = gloss * ((spec * 0.62) + (streak * 0.24) + (l.powf(2.8) * 0.14));
                r += hl * 88.0;
                g += hl * 102.0;
                b += hl * 126.0;
                let tint = (0.04 + (0.10 * refraction)) * (0.4 + (0.6 * gloss));
                r = lerp(r, r + 6.0, tint);
                g = lerp(g, g + 12.0, tint);
                b = lerp(b, b + 22.0, tint);
                dst[(row + x) as usize] =
                    SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
