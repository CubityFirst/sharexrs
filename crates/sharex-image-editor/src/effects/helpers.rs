//! `Core/ImageEffects/Helpers` (`ProceduralEffectHelper`,
//! `AnalogEffectHelper`, `DistortionEffectHelper`), the adjustment base's
//! `ApplyColorMatrix`/`ApplyPixelOperation`, and the Skia calls effects
//! share (blur image filter, colour filters, bitmap copies).
//!
//! Arithmetic is kept as C# writes it, in `float` where C# uses `float`:
//! .NET's JIT doesn't fuse multiply-adds and `MathF.Round` rounds half to
//! even, so the expressions are spelled the same way here.

#![allow(dead_code)]

use sharex_skia::raster::{Bitmap, Canvas, Paint, Rect, Sampling, Shader, TileMode};
use sharex_skia::Pixmap;

use super::color::{self, SkColor};

// ---- ProceduralEffectHelper ----

#[inline]
pub fn clamp_int(value: i32, min: i32, max: i32) -> i32 {
    if value < min {
        return min;
    }
    if value > max {
        return max;
    }
    value
}

#[inline]
pub fn clamp01(value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }
    if value >= 1.0 {
        return 1.0;
    }
    value
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + ((b - a) * t)
}

pub fn smooth_step(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() < 0.000001 {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }
    let t = clamp01((value - edge0) / (edge1 - edge0));
    t * t * (3.0 - (2.0 * t))
}

pub fn hash01(x: i32, y: i32, seed: i32) -> f32 {
    let mut h: u32 = 2166136261;
    h = (h ^ x as u32).wrapping_mul(16777619);
    h = (h ^ y as u32).wrapping_mul(16777619);
    h = (h ^ seed as u32).wrapping_mul(16777619);
    h ^= h >> 13;
    h = h.wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 16777215.0
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * ((t * ((t * 6.0) - 15.0)) + 10.0)
}

pub fn value_noise(x: f32, y: f32, seed: i32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0.wrapping_add(1);
    let y1 = y0.wrapping_add(1);
    let tx = fade(x - x0 as f32);
    let ty = fade(y - y0 as f32);
    let v00 = hash01(x0, y0, seed);
    let v10 = hash01(x1, y0, seed);
    let v01 = hash01(x0, y1, seed);
    let v11 = hash01(x1, y1, seed);
    let vx0 = lerp(v00, v10, tx);
    let vx1 = lerp(v01, v11, tx);
    lerp(vx0, vx1, ty)
}

pub fn fractal_noise(x: f32, y: f32, octaves: i32, lacunarity: f32, gain: f32, seed: i32) -> f32 {
    let mut value = 0f32;
    let mut amplitude = 1f32;
    let mut frequency = 1f32;
    let mut normalization = 0f32;
    for i in 0..octaves {
        value += value_noise(x * frequency, y * frequency, seed.wrapping_add(i.wrapping_mul(1619))) * amplitude;
        normalization += amplitude;
        frequency *= lacunarity;
        amplitude *= gain;
    }
    if normalization > 0.0 {
        value / normalization
    } else {
        0.0
    }
}

/// `Math.Clamp(float, min, max)`.
#[inline]
pub fn clampf(v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

/// `Math.Clamp(double, min, max)`.
#[inline]
pub fn clampd(v: f64, min: f64, max: f64) -> f64 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

pub fn bilinear_sample(pixels: &[SkColor], width: i32, height: i32, x: f32, y: f32) -> SkColor {
    let right = width - 1;
    let bottom = height - 1;
    let x = clampf(x, 0.0, right as f32);
    let y = clampf(y, 0.0, bottom as f32);
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = if x0 < right { x0 + 1 } else { right };
    let y1 = if y0 < bottom { y0 + 1 } else { bottom };
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let c00 = pixels[(y0 * width + x0) as usize];
    let c10 = pixels[(y0 * width + x1) as usize];
    let c01 = pixels[(y1 * width + x0) as usize];
    let c11 = pixels[(y1 * width + x1) as usize];
    let r0 = lerp(c00.r as f32, c10.r as f32, tx);
    let g0 = lerp(c00.g as f32, c10.g as f32, tx);
    let b0 = lerp(c00.b as f32, c10.b as f32, tx);
    let a0 = lerp(c00.a as f32, c10.a as f32, tx);
    let r1 = lerp(c01.r as f32, c11.r as f32, tx);
    let g1 = lerp(c01.g as f32, c11.g as f32, tx);
    let b1 = lerp(c01.b as f32, c11.b as f32, tx);
    let a1 = lerp(c01.a as f32, c11.a as f32, tx);
    SkColor::rgba(
        clamp_to_byte(lerp(r0, r1, ty)),
        clamp_to_byte(lerp(g0, g1, ty)),
        clamp_to_byte(lerp(b0, b1, ty)),
        clamp_to_byte(lerp(a0, a1, ty)),
    )
}

/// `ProceduralEffectHelper.ClampToByte`: `(byte)MathF.Round(value)`.
#[inline]
pub fn clamp_to_byte(value: f32) -> u8 {
    if value <= 0.0 {
        return 0;
    }
    if value >= 255.0 {
        return 255;
    }
    value.round_ties_even() as u8
}

// ---- AnalogEffectHelper ----

pub fn analog_luminance(c: SkColor) -> f32 {
    (0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32)
}

pub fn analog_luminance01(c: SkColor) -> f32 {
    analog_luminance(c) / 255.0
}

pub fn lerp_color(from: SkColor, to: SkColor, t: f32) -> SkColor {
    let a = clamp01(t);
    SkColor::rgba(
        clamp_to_byte(lerp(from.r as f32, to.r as f32, a)),
        clamp_to_byte(lerp(from.g as f32, to.g as f32, a)),
        clamp_to_byte(lerp(from.b as f32, to.b as f32, a)),
        clamp_to_byte(lerp(from.a as f32, to.a as f32, a)),
    )
}

pub fn apply_contrast(value: f32, contrast: f32) -> f32 {
    clamp01(((value - 0.5) * contrast) + 0.5)
}

pub fn screen(base: f32, blend: f32) -> f32 {
    let b = clamp01(base);
    let s = clamp01(blend);
    1.0 - ((1.0 - b) * (1.0 - s))
}

pub fn overlay(base: f32, blend: f32) -> f32 {
    let b = clamp01(base);
    let s = clamp01(blend);
    if b < 0.5 {
        2.0 * b * s
    } else {
        1.0 - (2.0 * (1.0 - b) * (1.0 - s))
    }
}

/// `AnalogEffectHelper.FromHsl`.
pub fn analog_from_hsl(mut h: f32, s: f32, l: f32, alpha: u8) -> SkColor {
    while h < 0.0 {
        h += 360.0;
    }
    while h >= 360.0 {
        h -= 360.0;
    }
    SkColor::from_hsl(h, clampf(s, 0.0, 100.0), clampf(l, 0.0, 100.0), alpha)
}

/// `AnalogEffectHelper.CreateBlurredClamp`: the image blurred with its edge
/// pixels repeated outwards (an expanded copy through a clamped shader,
/// `SKImageFilter.CreateBlur(radius / 3)`, the middle cut back out).
pub fn create_blurred_clamp(source: &Bitmap, radius: f32) -> Bitmap {
    if radius <= 0.01 {
        return source.clone();
    }
    let padding = 2.max((radius * 2.0).ceil() as i32);
    let expanded = expand_clamped(source, padding);
    let sigma = 0.001f32.max(radius / 3.0);
    let blurred = blur(&expanded, sigma, sigma);
    crop_copy(&blurred, padding, padding, source.width() as i32, source.height() as i32)
}

// ---- DistortionEffectHelper ----

pub fn percentage_to_x(width: i32, percentage: f32) -> f32 {
    (clampf(percentage, 0.0, 100.0) / 100.0) * 0.max(width - 1) as f32
}

pub fn percentage_to_y(height: i32, percentage: f32) -> f32 {
    (clampf(percentage, 0.0, 100.0) / 100.0) * 0.max(height - 1) as f32
}

pub fn sample_transparent(pixels: &[SkColor], width: i32, height: i32, x: f32, y: f32) -> SkColor {
    if x < 0.0 || x > (width - 1) as f32 || y < 0.0 || y > (height - 1) as f32 {
        return color::TRANSPARENT;
    }
    bilinear_sample(pixels, width, height, x, y)
}

pub fn blend(from: SkColor, to: SkColor, amount: f32) -> SkColor {
    lerp_color(from, to, amount)
}

pub fn multiply_rgb(c: SkColor, factor: f32) -> SkColor {
    let v = 0f32.max(factor);
    SkColor::rgba(
        clamp_to_byte(c.r as f32 * v),
        clamp_to_byte(c.g as f32 * v),
        clamp_to_byte(c.b as f32 * v),
        c.a,
    )
}

pub fn wrap_angle(radians: f32) -> f32 {
    let two_pi = std::f32::consts::PI * 2.0;
    let mut r = radians % two_pi;
    if r < 0.0 {
        r += two_pi;
    }
    r
}

// ---- AdjustmentImageEffectBase ----

/// `ApplyColorMatrix`: drawn through `SKColorFilter.CreateColorMatrix` onto
/// a cleared bitmap.
pub fn apply_color_matrix(source: &Bitmap, matrix: &[f32; 20]) -> Bitmap {
    let mut out = Bitmap::new(source.width(), source.height());
    {
        let mut canvas = Canvas::new(&mut out);
        let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
        paint.color_filter = Some(*matrix);
        canvas.draw_bitmap(source, 0.0, 0.0, Some(&paint));
    }
    out
}

/// `ApplyPixelOperation`: each pixel's premultiplied bytes read and written
/// as an `SKColor` (a `Bgra8888` bitmap is never unpremultiplied here).
pub fn apply_pixel_operation(source: &Bitmap, op: impl Fn(SkColor) -> SkColor) -> Bitmap {
    let mut out = Bitmap::new(source.width(), source.height());
    for (d, s) in out.data_mut().chunks_exact_mut(4).zip(source.data().chunks_exact(4)) {
        let c = op(SkColor::from_raw([s[0], s[1], s[2], s[3]]));
        d.copy_from_slice(&[c.r, c.g, c.b, c.a]);
    }
    out
}

// ---- Skia glue ----

pub fn bitmap_to_pixmap(b: &Bitmap) -> Pixmap {
    Pixmap::from_premul_rgba(b.width(), b.height(), b.data())
}

pub fn pixmap_to_bitmap(p: &Pixmap) -> Bitmap {
    Bitmap::from_premultiplied(p.width(), p.height(), p.to_premul_rgba())
}

/// `DrawBitmap(bmp, 0, 0, paint { ImageFilter = SKImageFilter.CreateBlur(sx, sy) })`
/// onto a transparent bitmap of the same size: decal edges.
pub fn blur(b: &Bitmap, sigma_x: f32, sigma_y: f32) -> Bitmap {
    if b.is_empty() {
        return b.clone();
    }
    pixmap_to_bitmap(&sharex_skia::blur::blur(&bitmap_to_pixmap(b), sigma_x, sigma_y))
}

/// The image grown by `padding` on every side, edge pixels repeated
/// (`SKShader.CreateBitmap(Clamp, Clamp, translate(padding))` filling the
/// bigger bitmap).
pub fn expand_clamped(source: &Bitmap, padding: i32) -> Bitmap {
    let (w, h) = (source.width() as i32, source.height() as i32);
    let (ew, eh) = (w + padding * 2, h + padding * 2);
    let mut out = Bitmap::new(ew as u32, eh as u32);
    for y in 0..eh {
        let sy = (y - padding).clamp(0, h - 1) as u32;
        for x in 0..ew {
            let sx = (x - padding).clamp(0, w - 1) as u32;
            out.set_pixel(x as u32, y as u32, source.pixel(sx, sy));
        }
    }
    out
}

/// `DrawBitmap(src, new SKRect(x, y, x + w, y + h), new SKRect(0, 0, w, h))`
/// onto a new transparent bitmap: a plain copy of that area.
pub fn crop_copy(src: &Bitmap, x: i32, y: i32, w: i32, h: i32) -> Bitmap {
    let mut out = Bitmap::new(w.max(0) as u32, h.max(0) as u32);
    for row in 0..h {
        let sy = row + y;
        if sy < 0 || sy >= src.height() as i32 {
            continue;
        }
        for col in 0..w {
            let sx = col + x;
            if sx < 0 || sx >= src.width() as i32 {
                continue;
            }
            out.set_pixel(col as u32, row as u32, src.pixel(sx as u32, sy as u32));
        }
    }
    out
}

/// A draw of `image` with default sampling at an integer offset onto
/// `dst`, src-over (a sprite blit).
pub fn draw_bitmap(dst: &mut Bitmap, image: &Bitmap, x: f32, y: f32) {
    let mut canvas = Canvas::new(dst);
    canvas.draw_bitmap(image, x, y, None);
}

/// `canvas.DrawRect(rect, paint { Shader = SKShader.CreateBitmap(image, tx, ty, matrix) })`.
pub fn fill_rect_with_image(dst: &mut Bitmap, rect: Rect, image: &Bitmap, tile: TileMode, matrix: sharex_skia::raster::Matrix) {
    let mut canvas = Canvas::new(dst);
    let paint = Paint::shader(Shader::Image {
        image,
        matrix,
        sampling: Sampling::Nearest,
        tile_x: tile,
        tile_y: tile,
    });
    canvas.draw_rect(rect, &paint);
}
