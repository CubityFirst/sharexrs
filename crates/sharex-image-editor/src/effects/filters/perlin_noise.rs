//! `PerlinNoiseImageEffect` (`Core/ImageEffects/Filters/PerlinNoiseImageEffect.cs`).

use crate::effects::prelude::*;

/// `PerlinNoiseImageEffect`.
#[derive(Clone, Debug)]
pub struct PerlinNoiseImageEffect {
    pub scale: f32,
    pub octaves: i32,
    pub persistence: f32,
    pub strength: f32,
    pub monochrome: bool,
}

impl Default for PerlinNoiseImageEffect {
    fn default() -> Self {
        PerlinNoiseImageEffect {
            scale: 64.0,
            octaves: 4,
            persistence: 0.5,
            strength: 50.0,
            monochrome: true,
        }
    }
}

impl EffectImpl for PerlinNoiseImageEffect {
    fn info() -> Info {
        Info::new("perlin_noise", "Perlin noise", EffectCategory::Filters, "image-effect/waves")
            .desc("Generates a Perlin noise pattern and blends it with the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("scale", "Scale", 1.0, 500.0, 64.0, |e: &mut Self, v| e.scale = v).snap(false),
            int_slider::<Self>("octaves", "Octaves", 1, 8, 4, |e: &mut Self, v| e.octaves = v),
            float_slider::<Self>("persistence", "Persistence", 0.0, 1.0, 0.5, |e: &mut Self, v| e.persistence = v).tick(0.01).snap(false).format("{}{0:0.00}"),
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 50.0, |e: &mut Self, v| e.strength = v).snap(false),
            bool_param::<Self>("monochrome", "Monochrome", true, |e: &mut Self, v| e.monochrome = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let strength = clampf(self.strength, 0.0, 100.0) / 100.0;
        if strength <= 0.0 {
            return Ok(source.clone());
        }
        let scale = 1f32.max(self.scale);
        let octaves = self.octaves.clamp(1, 8);
        let persistence = clampf(self.persistence, 0.0, 1.0);
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let inv = 1.0 / scale;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let pl = |a: f32, b: f32, t: f32| a + (b - a) * t;
        for y in 0..height {
            for x in 0..width {
                let i = (y * width + x) as usize;
                let s = src[i];
                let (fx, fy) = (x as f32 * inv, y as f32 * inv);
                let (nr, ng, nb) = if self.monochrome {
                    let n = (fractal_perlin(fx, fy, octaves, persistence, 0) + 1.0) * 0.5;
                    (n, n, n)
                } else {
                    (
                        (fractal_perlin(fx, fy, octaves, persistence, 0) + 1.0) * 0.5,
                        (fractal_perlin(fx, fy, octaves, persistence, 1) + 1.0) * 0.5,
                        (fractal_perlin(fx, fy, octaves, persistence, 2) + 1.0) * 0.5,
                    )
                };
                let r = pl(s.r as f32 / 255.0, nr, strength);
                let g = pl(s.g as f32 / 255.0, ng, strength);
                let b = pl(s.b as f32 / 255.0, nb, strength);
                dst[i] = SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

#[rustfmt::skip]
const PERM: [u8; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225,
    140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148,
    247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32,
    57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122,
    60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54,
    65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169,
    200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64,
    52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212,
    207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213,
    119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
    129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104,
    218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241,
    81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157,
    184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
    222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
];

fn fractal_perlin(x: f32, y: f32, octaves: i32, persistence: f32, seed: i32) -> f32 {
    let (mut value, mut amp, mut freq, mut max) = (0f32, 1f32, 1f32, 0f32);
    for i in 0..octaves {
        let ox = (seed as f32 * 137.3) + (i as f32 * 47.7);
        let oy = (seed as f32 * 251.1) + (i as f32 * 89.3);
        value += perlin(x * freq + ox, y * freq + oy) * amp;
        max += amp;
        freq *= 2.0;
        amp *= persistence;
    }
    if max > 0.0 { value / max } else { 0.0 }
}

fn perlin(x: f32, y: f32) -> f32 {
    let xi = (x.floor() as i32 & 255) as usize;
    let yi = (y.floor() as i32 & 255) as usize;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (u, v) = (fade(xf), fade(yf));
    let p = |i: usize| PERM[i & 255] as usize;
    let aa = p(p(xi) + yi);
    let ab = p(p(xi) + yi + 1);
    let ba = p(p((xi + 1) & 255) + yi);
    let bb = p(p((xi + 1) & 255) + yi + 1);
    let grad = |h: usize, x: f32, y: f32| match h & 3 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        _ => -x - y,
    };
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x0 = l(grad(aa, xf, yf), grad(ba, xf - 1.0, yf), u);
    let x1 = l(grad(ab, xf, yf - 1.0), grad(bb, xf - 1.0, yf - 1.0), u);
    l(x0, x1, v)
}
