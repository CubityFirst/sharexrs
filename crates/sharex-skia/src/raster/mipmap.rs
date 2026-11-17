//! Mipmaps for `SKSamplingOptions(SKFilterMode.Linear, SKMipmapMode.Linear)`, what
//! ShareX's OCR scales images down with: `SkMipmap::Build` for 8888 pixels (the box
//! and triangle filters), `SkMipmap::ComputeLevel` and `SkMipmapAccessor`'s choice of
//! the two levels to blend.
//!
//! Sources: `src/core/SkMipmap.cpp`, `src/core/SkMipmapAccessor.cpp`,
//! `src/core/SkMatrix.cpp` (`decomposeScale`), Skia m119.

use super::{Bitmap, Matrix};

/// `SkMipmap::ComputeLevelCount`: one level per halving of the longest side.
pub fn level_count(width: u32, height: u32) -> usize {
    if width < 1 || height < 1 {
        return 0;
    }
    let largest = width.max(height);
    if largest < 2 {
        return 0;
    }
    (32 - largest.leading_zeros() as usize).saturating_sub(1)
}

/// `SkMipmap::Build` (`computeContents`): every level below the base, each half the
/// size of the one above (rounded down, at least 1). Odd sides use a 3-tap triangle
/// filter (1-2-1) with a pixel of overlap, even sides a 2-tap box; each tap sums the
/// 8-bit channels in 16 bits and shifts back down.
pub fn build(src: &Bitmap) -> Vec<Bitmap> {
    if src.width() <= 1 && src.height() <= 1 {
        return Vec::new();
    }
    let count = level_count(src.width(), src.height());
    let mut levels: Vec<Bitmap> = Vec::with_capacity(count);
    for _ in 0..count {
        let prev = levels.last().unwrap_or(src);
        let level = downsample(prev);
        levels.push(level);
    }
    levels
}

/// One level from the level above: picks `proc_WxH` from the parity of the source
/// size and runs it over each destination row (two source rows apart).
fn downsample(src: &Bitmap) -> Bitmap {
    let (sw, sh) = (src.width(), src.height());
    let taps_x = if sw & 1 == 1 { if sw == 1 { 1 } else { 3 } } else { 2 };
    let taps_y = if sh & 1 == 1 { if sh == 1 { 1 } else { 3 } } else { 2 };
    let w = (sw >> 1).max(1);
    let h = (sh >> 1).max(1);
    let mut out = Bitmap::new(w, h);
    let expand = |x: u32, y: u32| src.pixel(x, y).map(u16::from);
    let add = |a: [u16; 4], b: [u16; 4]| [0, 1, 2, 3].map(|i| a[i].wrapping_add(b[i]));
    let add_121 = |a: [u16; 4], b: [u16; 4], c: [u16; 4]| add(add(add(a, b), b), c);
    let shr = |c: [u16; 4], bits: u32| c.map(|v| (v >> bits) as u8);
    for y in 0..h {
        let r0 = y * 2;
        for x in 0..w {
            let c0 = x * 2;
            let px = match (taps_x, taps_y) {
                // downsample_1_2
                (1, 2) => shr(add(expand(c0, r0), expand(c0, r0 + 1)), 1),
                // downsample_1_3
                (1, 3) => shr(add_121(expand(c0, r0), expand(c0, r0 + 1), expand(c0, r0 + 2)), 2),
                // downsample_2_1
                (2, 1) => shr(add(expand(c0, r0), expand(c0 + 1, r0)), 1),
                // downsample_2_2: c00 + c10 + c01 + c11
                (2, 2) => {
                    let c = add(add(add(expand(c0, r0), expand(c0, r0 + 1)), expand(c0 + 1, r0)), expand(c0 + 1, r0 + 1));
                    shr(c, 2)
                }
                // downsample_2_3
                (2, 3) => {
                    let a = add_121(expand(c0, r0), expand(c0, r0 + 1), expand(c0, r0 + 2));
                    let b = add_121(expand(c0 + 1, r0), expand(c0 + 1, r0 + 1), expand(c0 + 1, r0 + 2));
                    shr(add(a, b), 3)
                }
                // downsample_3_1
                (3, 1) => shr(add_121(expand(c0, r0), expand(c0 + 1, r0), expand(c0 + 2, r0)), 2),
                // downsample_3_2: a + (b0 + b0 + b1 + b1) + c
                (3, 2) => {
                    let a = add(expand(c0, r0), expand(c0, r0 + 1));
                    let (b0, b1) = (expand(c0 + 1, r0), expand(c0 + 1, r0 + 1));
                    let b = add(add(add(b0, b0), b1), b1);
                    let c = add(expand(c0 + 2, r0), expand(c0 + 2, r0 + 1));
                    shr(add(add(a, b), c), 3)
                }
                // downsample_3_3: a + (add_121(b) << 1) + c
                (3, 3) => {
                    let col = |cx: u32| add_121(expand(cx, r0), expand(cx, r0 + 1), expand(cx, r0 + 2));
                    let b = col(c0 + 1).map(|v| v << 1);
                    shr(add(add(col(c0), b), col(c0 + 2)), 4)
                }
                // A 1x1 source has no levels.
                _ => expand(c0, r0).map(|v| v as u8),
            };
            out.set_pixel(x, y, px);
        }
    }
    out
}

/// `SkMatrix::decomposeScale`: the lengths of the matrix's columns, or `None` when
/// it has perspective or a nearly zero (or non-finite) scale.
pub fn decompose_scale(m: &Matrix) -> Option<(f32, f32)> {
    let length = |x: f32, y: f32| {
        let mag2 = x * x + y * y;
        if mag2.is_finite() {
            mag2.sqrt()
        } else {
            ((x as f64 * x as f64 + y as f64 * y as f64).sqrt()) as f32
        }
    };
    let sx = length(m.sx(), m.ky());
    let sy = length(m.kx(), m.sy());
    let nearly_zero = 1.0f32 / 4096.0;
    if !sx.is_finite() || !sy.is_finite() || sx.abs() <= nearly_zero || sy.abs() <= nearly_zero {
        return None;
    }
    Some((sx, sy))
}

/// `SkMipmap::ComputeLevel` for a downscale of (`sx`, `sy`): the smaller scale, with
/// the GPU's half-level sharpening bias; negative for no mipmapping.
pub fn compute_level(sx: f32, sy: f32) -> f32 {
    let scale = sx.min(sy);
    if scale >= 1.0 || scale <= 0.0 || !scale.is_finite() {
        return -1.0;
    }
    let level = (-scale.log2() - 0.5).max(0.0);
    if !level.is_finite() {
        return -1.0;
    }
    level
}

/// What `SkMipmapAccessor` picks for a linear mipmap: the level to sample (`upper`,
/// `None` for the base image) and, when between two levels, the next smaller level
/// and its blend weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelChoice {
    /// Index into [`build`]'s levels, `None` for the base.
    pub upper: Option<usize>,
    /// The smaller level and how much of it to blend in.
    pub lower: Option<(usize, f32)>,
}

/// `SkMipmapAccessor(image, inv, SkMipmapMode::kLinear)`: `inv` maps device to image
/// space, so its scale is the reciprocal of the downscale.
pub fn choose_linear(inv: &Matrix, width: u32, height: u32) -> LevelChoice {
    let mut linear = true;
    let mut level = 0.0f32;
    match decompose_scale(inv) {
        None => linear = false,
        Some((sx, sy)) => {
            level = compute_level(1.0 / sx, 1.0 / sy);
            if level <= 0.0 {
                linear = false;
                level = 0.0;
            }
        }
    }
    let level_num = level.floor() as i32;
    let lower_weight = level - level_num as f32;
    let count = if width <= 1 && height <= 1 { 0 } else { level_count(width, height) };
    let mut choice = LevelChoice { upper: None, lower: None };
    if level_num > 0 || (linear && lower_weight > 0.0) {
        if count == 0 {
            return choice;
        }
        if level_num > 0 {
            if ((level_num - 1) as usize) < count {
                choice.upper = Some((level_num - 1) as usize);
            } else {
                linear = false;
            }
        }
        if linear && (level_num as usize) < count {
            choice.lower = Some((level_num as usize, lower_weight));
        }
    }
    choice
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_halve_down_to_one_pixel() {
        assert_eq!(level_count(1, 1), 0);
        assert_eq!(level_count(2, 1), 1);
        assert_eq!(level_count(100, 7), 6);
        let levels = build(&Bitmap::new(5, 3));
        let sizes: Vec<_> = levels.iter().map(|l| (l.width(), l.height())).collect();
        assert_eq!(sizes, vec![(2, 1), (1, 1)]);
    }

    #[test]
    fn odd_sizes_use_the_triangle_filter() {
        // 3x1: (a + 2b + c) / 4, truncated.
        let src = Bitmap::from_premultiplied(3, 1, vec![10, 0, 0, 255, 21, 0, 0, 255, 40, 0, 0, 255]);
        assert_eq!(build(&src)[0].pixel(0, 0), [23, 0, 0, 255]);
    }

    #[test]
    fn half_size_blends_between_levels() {
        // A 0.4x downscale: level = log2(2.5) - 0.5 = 0.82, so the base and level 1.
        let inv = Matrix::scale_translate(2.5, 2.5, 0.0, 0.0);
        let c = choose_linear(&inv, 100, 100);
        assert_eq!(c.upper, None);
        let (lower, w) = c.lower.unwrap();
        assert_eq!(lower, 0);
        assert!((w - (2.5f32.log2() - 0.5)).abs() < 1e-6);
        // Upscales and 1:1 never mipmap.
        assert_eq!(choose_linear(&Matrix::scale_translate(0.5, 0.5, 0.0, 0.0), 100, 100), LevelChoice { upper: None, lower: None });
    }
}
