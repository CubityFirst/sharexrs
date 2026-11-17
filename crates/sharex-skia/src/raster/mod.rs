//! The parts of Skia m119 (SkiaSharp 3.119, what ShareX draws with) that ShareX's
//! image effects go through, ported with Skia's own arithmetic so results match bit for
//! bit: the raster pipeline's float ("highp") and 16-bit ("lowp") stages as libSkiaSharp
//! runs them (the SSE build: no fused multiply-adds, round-to-nearest-even stores), the
//! sprite blitter, pixel conversions, matrices, image sampling and gradients.
//!
//! Sources: `src/opts/SkRasterPipeline_opts.h`, `src/core/SkRasterPipelineBlitter.cpp`,
//! `src/opts/SkBlitRow_opts.h`, `src/opts/SkSwizzler_opts.h`, `src/core/SkConvertPixels.cpp`,
//! `src/shaders/SkImageShader.cpp`, `src/core/SkMatrix.cpp`, `src/core/SkM44.cpp`.

pub mod bitmap;
pub mod canvas;
mod coverage;
pub mod gradient;
pub mod matrix;
pub mod mipmap;

pub use bitmap::Bitmap;
pub use canvas::{Blend, Canvas, Paint, Sampling, Shader, TileMode};
pub use matrix::{Matrix, Rect};

/// `SkOpts::RGBA_to_rgbA`: `(c * a + 127) / 255`, exact.
#[inline]
pub fn premul_byte(c: u8, a: u8) -> u8 {
    ((c as u32 * a as u32 + 127) / 255) as u8
}

/// `from_byte` / `from_8888`: `b * (1/255.0f)`.
#[inline]
pub fn from_byte(b: u8) -> f32 {
    b as f32 * (1.0 / 255.0)
}

/// `mad`: `f * m + a`, not fused. ShareX's libSkiaSharp runs Skia's SSE pipeline
/// (`SkOpts` without the HSW/FMA variant): fused multiply-adds give ±1 differences.
#[inline]
pub fn mad(f: f32, m: f32, a: f32) -> f32 {
    f * m + a
}

/// `to_unorm(v, scale, bias)`: clamp to `[0, bias]`, scale, round to nearest even
/// (`_mm256_cvtps_epi32`).
#[inline]
pub fn to_unorm(v: f32, scale: f32, bias: f32) -> u8 {
    let v = if v > 0.0 { v } else { 0.0 };
    let v = if v < bias { v } else { bias };
    (v * scale).round_ties_even() as u8
}

/// A premultiplied pixel read as floats (`from_8888`).
#[inline]
pub fn load(p: [u8; 4]) -> [f32; 4] {
    [
        from_byte(p[0]),
        from_byte(p[1]),
        from_byte(p[2]),
        from_byte(p[3]),
    ]
}

/// `store_8888`.
#[inline]
pub fn store(c: [f32; 4]) -> [u8; 4] {
    [
        to_unorm(c[0], 255.0, 1.0),
        to_unorm(c[1], 255.0, 1.0),
        to_unorm(c[2], 255.0, 1.0),
        to_unorm(c[3], 255.0, 1.0),
    ]
}

/// The `unpremul` stage.
#[inline]
pub fn unpremul_f(c: [f32; 4]) -> [f32; 4] {
    let inv = 1.0 / c[3];
    let scale = if inv < f32::INFINITY { inv } else { 0.0 };
    [c[0] * scale, c[1] * scale, c[2] * scale, c[3]]
}

/// Premultiplied to straight, as `SkPixmap::readPixels` converts (a highp pipeline:
/// `load_8888`, `unpremul`, `store_8888`).
#[inline]
pub fn unpremul(p: [u8; 4]) -> [u8; 4] {
    match p[3] {
        255 => p,
        0 => [0, 0, 0, 0],
        _ => store(unpremul_f(load(p))),
    }
}

/// Straight to premultiplied, as `readPixels` converts back (`SkOpts::RGBA_to_rgbA`).
#[inline]
pub fn premul(p: [u8; 4]) -> [u8; 4] {
    [
        premul_byte(p[0], p[3]),
        premul_byte(p[1], p[3]),
        premul_byte(p[2], p[3]),
        p[3],
    ]
}

/// The `matrix_4x5` stage over Skia's row-major 4x5 (`SkColorFilters::Matrix`).
#[inline]
pub fn matrix_4x5(m: &[f32; 20], c: [f32; 4]) -> [f32; 4] {
    let [r, g, b, a] = c;
    let row = |i: usize| {
        mad(
            r,
            m[i],
            mad(g, m[i + 1], mad(b, m[i + 2], mad(a, m[i + 3], m[i + 4]))),
        )
    };
    [row(0), row(5), row(10), row(15)]
}

#[inline]
pub fn clamp_01(c: [f32; 4]) -> [f32; 4] {
    c.map(|v| v.clamp(0.0, 1.0))
}

#[inline]
pub fn premul_f(c: [f32; 4]) -> [f32; 4] {
    [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]
}

/// `SkMatrixColorFilter::appendStages` for a source that isn't opaque: `unpremul`,
/// `matrix_4x5`, `clamp_01`, `premul`.
#[inline]
pub fn color_filter(m: &[f32; 20], c: [f32; 4]) -> [f32; 4] {
    premul_f(clamp_01(matrix_4x5(m, unpremul_f(c))))
}

/// `lowp::div255`: `(v + 255) / 256`, never more than 1 off `(v + 127) / 255`.
#[inline]
pub fn div255(v: u32) -> u32 {
    (v + 255) >> 8
}

/// `lowp::div255_accurate`.
#[inline]
pub fn div255_accurate(v: u32) -> u32 {
    let v = v + 128;
    (v + (v >> 8)) >> 8
}

/// `lowp::from_float`: `f * 255 + 0.5`, truncated.
#[inline]
pub fn lowp_from_float(f: f32) -> u32 {
    (f * 255.0 + 0.5) as u32
}
