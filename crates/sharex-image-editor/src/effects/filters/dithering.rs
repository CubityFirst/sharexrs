//! `DitheringImageEffect` (`Core/ImageEffects/Filters/DitheringImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DitheringMethod {
    FloydSteinberg,
    Bayer4x4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DitheringPalette {
    OneBitBW,
    WebSafe216,
    RGB332,
    Grayscale4,
}

/// `DitheringImageEffect`.
#[derive(Clone, Debug)]
pub struct DitheringImageEffect {
    pub method: DitheringMethod,
    pub palette: DitheringPalette,
    pub serpentine: bool,
    pub strength: f32,
}

impl Default for DitheringImageEffect {
    fn default() -> Self {
        DitheringImageEffect {
            method: DitheringMethod::FloydSteinberg,
            palette: DitheringPalette::OneBitBW,
            serpentine: true,
            strength: 100.0,
        }
    }
}

impl EffectImpl for DitheringImageEffect {
    fn info() -> Info {
        Info::new("dithering", "Dithering", EffectCategory::Filters, "image-effect/checkerboard")
            .desc("Applies dithering to reduce color depth with various methods and palettes.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, DitheringMethod>("method", "Method", DitheringMethod::FloydSteinberg, |e: &mut Self, v| e.method = v, &[("Floyd-Steinberg", DitheringMethod::FloydSteinberg), ("Bayer 4x4", DitheringMethod::Bayer4x4)]),
            enum_param::<Self, DitheringPalette>("palette", "Palette", DitheringPalette::OneBitBW, |e: &mut Self, v| e.palette = v, &[("1-bit B&W", DitheringPalette::OneBitBW), ("Web safe 216", DitheringPalette::WebSafe216), ("RGB332", DitheringPalette::RGB332), ("Grayscale 4", DitheringPalette::Grayscale4)]),
            bool_param::<Self>("serpentine", "Serpentine", true, |e: &mut Self, v| e.serpentine = v),
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 100.0, |e: &mut Self, v| e.strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        if width <= 0 || height <= 0 {
            return Ok(source.clone());
        }
        let strength = clampf(self.strength, 0.0, 100.0) / 100.0;
        if strength <= 0.0 {
            return Ok(source.clone());
        }
        let src = pixels(source);
        let dithered = if self.method == DitheringMethod::Bayer4x4 {
            bayer(&src, width, height, self.palette)
        } else {
            floyd_steinberg(&src, width, height, self.palette, self.serpentine)
        };
        let out: Vec<SkColor> = src
            .iter()
            .zip(&dithered)
            .map(|(s, d)| {
                SkColor::rgba(
                    clamp_to_byte(lerp(s.r as f32, d.r as f32, strength)),
                    clamp_to_byte(lerp(s.g as f32, d.g as f32, strength)),
                    clamp_to_byte(lerp(s.b as f32, d.b as f32, strength)),
                    s.a,
                )
            })
            .collect();
        Ok(from_pixels(width as u32, height as u32, &out))
    }
}

// ---- port ----

fn floyd_steinberg(src: &[SkColor], width: i32, height: i32, palette: DitheringPalette, serpentine: bool) -> Vec<SkColor> {
    let mut r: Vec<f32> = src.iter().map(|c| c.r as f32).collect();
    let mut g: Vec<f32> = src.iter().map(|c| c.g as f32).collect();
    let mut b: Vec<f32> = src.iter().map(|c| c.b as f32).collect();
    let mut out = vec![SkColor::default(); src.len()];
    for y in 0..height {
        let reverse = serpentine && (y & 1) == 1;
        let (start, end, step) = if reverse { (width - 1, -1, -1) } else { (0, width, 1) };
        let mut x = start;
        while x != end {
            let i = (y * width + x) as usize;
            let (cr, cg, cb) = (r[i], g[i], b[i]);
            let (qr, qg, qb) = quantize(cr, cg, cb, palette);
            out[i] = SkColor::rgba(qr, qg, qb, src[i].a);
            let (er, eg, eb) = (cr - qr as f32, cg - qg as f32, cb - qb as f32);
            let dir = if reverse { -1 } else { 1 };
            let mut dist = |xx: i32, yy: i32, f: f32| {
                if xx < 0 || xx >= width || yy < 0 || yy >= height {
                    return;
                }
                let j = (yy * width + xx) as usize;
                r[j] = clampf(r[j] + (er * f), 0.0, 255.0);
                g[j] = clampf(g[j] + (eg * f), 0.0, 255.0);
                b[j] = clampf(b[j] + (eb * f), 0.0, 255.0);
            };
            dist(x + dir, y, 7.0 / 16.0);
            dist(x - dir, y + 1, 3.0 / 16.0);
            dist(x, y + 1, 5.0 / 16.0);
            dist(x + dir, y + 1, 1.0 / 16.0);
            x += step;
        }
    }
    out
}

fn bayer(src: &[SkColor], width: i32, height: i32, palette: DitheringPalette) -> Vec<SkColor> {
    const B4: [[i32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    let mut out = vec![SkColor::default(); src.len()];
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) as usize;
            let s = src[i];
            let threshold = (B4[(y & 3) as usize][(x & 3) as usize] as f32 / 16.0) - 0.5;
            let bias = threshold * 36.0;
            let (qr, qg, qb) = quantize(s.r as f32 + bias, s.g as f32 + bias, s.b as f32 + bias, palette);
            out[i] = SkColor::rgba(qr, qg, qb, s.a);
        }
    }
    out
}

fn quantize(r: f32, g: f32, b: f32, palette: DitheringPalette) -> (u8, u8, u8) {
    match palette {
        DitheringPalette::OneBitBW => {
            let lum = (0.2126 * r) + (0.7152 * g) + (0.0722 * b);
            let bw = if lum >= 128.0 { 255 } else { 0 };
            (bw, bw, bw)
        }
        DitheringPalette::WebSafe216 => (channel(r, 6), channel(g, 6), channel(b, 6)),
        DitheringPalette::RGB332 => (channel(r, 8), channel(g, 8), channel(b, 4)),
        DitheringPalette::Grayscale4 => {
            let lum = (0.2126 * r) + (0.7152 * g) + (0.0722 * b);
            let v = channel(lum, 4);
            (v, v, v)
        }
    }
}

fn channel(value: f32, levels: i32) -> u8 {
    let c = clampf(value, 0.0, 255.0) / 255.0;
    let steps = levels - 1;
    let q = (c * steps as f32).round_ties_even() as i32;
    clamp_to_byte((q as f32 / steps as f32) * 255.0)
}
