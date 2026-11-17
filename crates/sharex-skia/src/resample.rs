//! `SkPixmap::scalePixels` (what `SKBitmap.Resize` does): the image
//! shader drawn with `kSrc` over the destination, clamped, through the
//! raster pipeline's lowp stages (`bilerp_clamp_8888` for linear sampling,
//! `gather_8888` with round-down-at-integer for nearest).

use crate::pixmap::Pixmap;

/// Lowp `mad` (`f*m+a`): fused, as clang contracts it in the HSW build of
/// the raster pipeline SkiaSharp runs on x64.
#[inline]
fn mad(f: f32, m: f32, a: f32) -> f32 {
    f.mul_add(m, a)
}

/// The inverse of `SkMatrix::RectToRect(src, dst)`'s scale, as
/// `SkMatrix::invert` computes it.
fn inv_scale(src: u32, dst: u32) -> f32 {
    let s = dst as f32 / src as f32;
    1.0 / s
}

fn channels(p: u32) -> [u16; 4] {
    [
        (p & 0xFF) as u16,
        ((p >> 8) & 0xFF) as u16,
        ((p >> 16) & 0xFF) as u16,
        (p >> 24) as u16,
    ]
}

/// `scaled_mult` (`_mm_mulhrs_epi16`).
#[inline]
fn scaled_mult(a: i16, b: i16) -> i16 {
    ((a as i32 * b as i32 + (1 << 14)) >> 15) as i16
}

fn pixel_clamped(src: &Pixmap, x: i32, y: i32) -> u32 {
    let ax = x.clamp(0, src.width() as i32 - 1);
    let ay = y.clamp(0, src.height() as i32 - 1);
    src.pixel(ax, ay)
}

/// Linear (bilinear, no mipmaps) resize.
pub fn resize_linear(src: &Pixmap, w: u32, h: u32) -> Pixmap {
    if w == src.width() && h == src.height() {
        return src.clone();
    }
    let isx = inv_scale(src.width(), w);
    let isy = inv_scale(src.height(), h);
    let mut out = Pixmap::new(w, h);
    for dy in 0..h as i32 {
        for dx in 0..w as i32 {
            let x = mad(dx as f32 + 0.5, isx, 0.0);
            let y = mad(dy as f32 + 0.5, isy, 0.0);
            let qx = (65536.0f32 * x + 0.5).floor() as i32 - 32768;
            let qy = (65536.0f32 * y + 0.5).floor() as i32 - 32768;
            let sx = qx >> 16;
            let sy = qy >> 16;
            let tx = (qx ^ 0x8000) as i16;
            let ty = (qy ^ 0x8000) as i16;
            let lerp_x = |l: u16, r: u16| -> u16 {
                let width = ((r.wrapping_sub(l)) as i16).wrapping_shl(7);
                let middle = (r.wrapping_add(l)).wrapping_shl(7);
                let v2 = middle
                    .wrapping_add(scaled_mult(tx, width) as u16)
                    .wrapping_add(1);
                v2 >> 1
            };
            let lerp_y = |t: u16, b: u16| -> u16 {
                let width = (b as i16).wrapping_sub(t as i16);
                let middle = b.wrapping_add(t);
                let blend = middle
                    .wrapping_add(scaled_mult(ty, width) as u16)
                    .wrapping_add(0x80);
                blend >> 8
            };
            let l = channels(pixel_clamped(src, sx, sy));
            let r = channels(pixel_clamped(src, sx + 1, sy));
            let top: Vec<u16> = (0..4).map(|i| lerp_x(l[i], r[i])).collect();
            let l = channels(pixel_clamped(src, sx, sy + 1));
            let r = channels(pixel_clamped(src, sx + 1, sy + 1));
            let bottom: Vec<u16> = (0..4).map(|i| lerp_x(l[i], r[i])).collect();
            let mut p = 0u32;
            for i in 0..4 {
                p |= ((lerp_y(top[i], bottom[i]) & 0xFF) as u32) << (8 * i);
            }
            *out.pixel_mut(dx, dy) = p;
        }
    }
    out
}

/// `ix_and_ptr` for float coordinates with `roundDownAtInteger`: clamped
/// to the image, then nudged down one ulp and truncated.
fn nearest_index(v: f32, limit: u32) -> i32 {
    let max = f32::from_bits((limit as f32).to_bits() - 1);
    let v = v.max(f32::MIN_POSITIVE).min(max);
    let v = f32::from_bits(v.to_bits() - 1);
    v.trunc() as i32
}

/// Nearest-neighbour resize.
pub fn resize_nearest(src: &Pixmap, w: u32, h: u32) -> Pixmap {
    if w == src.width() && h == src.height() {
        return src.clone();
    }
    let isx = inv_scale(src.width(), w);
    let isy = inv_scale(src.height(), h);
    let mut out = Pixmap::new(w, h);
    for dy in 0..h as i32 {
        let y = mad(dy as f32 + 0.5, isy, 0.0);
        let iy = nearest_index(y, src.height());
        for dx in 0..w as i32 {
            let x = mad(dx as f32 + 0.5, isx, 0.0);
            let ix = nearest_index(x, src.width());
            *out.pixel_mut(dx, dy) = src.pixel(ix, iy);
        }
    }
    out
}

/// `SkImageShader::CubicResamplerMatrix(B, C)`, column-major.
fn cubic_weights(b: f32, c: f32) -> [f32; 16] {
    let rows = [
        [
            (1.0 / 6.0) * b,
            -(3.0 / 6.0) * b - c,
            (3.0 / 6.0) * b + 2.0 * c,
            -(1.0 / 6.0) * b - c,
        ],
        [
            1.0 - (2.0 / 6.0) * b,
            0.0,
            -3.0 + (12.0 / 6.0) * b + c,
            2.0 - (9.0 / 6.0) * b - c,
        ],
        [
            (1.0 / 6.0) * b,
            (3.0 / 6.0) * b + c,
            3.0 - (15.0 / 6.0) * b - 2.0 * c,
            -2.0 + (9.0 / 6.0) * b + c,
        ],
        [0.0, 0.0, -c, (1.0 / 6.0) * b + c],
    ];
    let mut w = [0.0f32; 16];
    for (r, row) in rows.iter().enumerate() {
        for (col, v) in row.iter().enumerate() {
            w[col * 4 + r] = *v;
        }
    }
    w
}

/// Highp `mad` on HSW: fused.
#[inline]
fn hmad(f: f32, m: f32, a: f32) -> f32 {
    f.mul_add(m, a)
}

fn bicubic_wts(t: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    hmad(t, hmad(t, hmad(t, d, c), b), a)
}

/// Highp `ix_and_ptr` (`clamp_ex`, no rounding down).
fn clamp_index(v: f32, limit: u32) -> i32 {
    let max = f32::from_bits((limit as f32).to_bits() - 1);
    v.max(f32::MIN_POSITIVE).min(max).trunc() as i32
}

/// `SkCanvas::drawImageRect(image, src, dst, SkCubicResampler::Mitchell(),
/// paint)` onto a transparent `dst_pm` (`SkBitmapDevice::drawImageRect`: a
/// non-AA rect filled with a clamped image shader mapping `src` to `dst`),
/// for a whole-pixel `dst`.
pub fn draw_image_rect_mitchell(
    dst_pm: &mut Pixmap,
    image: &Pixmap,
    src: crate::geom::Rect,
    dst: crate::geom::IRect,
) {
    let third = 1.0f32 / 3.0;
    let w = cubic_weights(third, third);
    let dr = dst.to_rect();
    // SkMatrix::setRectToRect(src, dst, kFill) and its inverse.
    let sx = dr.width() / src.width();
    let sy = dr.height() / src.height();
    let tx = dr.left - src.left * sx;
    let ty = dr.top - src.top * sy;
    let inv_x = 1.0 / sx;
    let inv_y = 1.0 / sy;
    let itx = -tx * inv_x;
    let ity = -ty * inv_y;
    let (iw, ih) = (image.width(), image.height());
    for py in dst.top.max(0)..dst.bottom.min(dst_pm.height() as i32) {
        for px in dst.left.max(0)..dst.right.min(dst_pm.width() as i32) {
            let cx = hmad(px as f32 + 0.5, inv_x, itx);
            let cy = hmad(py as f32 + 0.5, inv_y, ity);
            let fract = |v: f32| v - v.floor();
            let fx = fract(cx + 0.5);
            let fy = fract(cy + 0.5);
            let wts = |t: f32| {
                [
                    bicubic_wts(t, w[0], w[4], w[8], w[12]),
                    bicubic_wts(t, w[1], w[5], w[9], w[13]),
                    bicubic_wts(t, w[2], w[6], w[10], w[14]),
                    bicubic_wts(t, w[3], w[7], w[11], w[15]),
                ]
            };
            let scaley = wts(fy);
            let scalex = wts(fx);
            let mut acc = [0.0f32; 4];
            let mut sample_y = cy - 1.5;
            for sy_w in scaley {
                let mut sample_x = cx - 1.5;
                for sx_w in scalex {
                    let scale = sx_w * sy_w;
                    let p = image.pixel(clamp_index(sample_x, iw), clamp_index(sample_y, ih));
                    let ch = [p & 0xFF, (p >> 8) & 0xFF, (p >> 16) & 0xFF, p >> 24];
                    for i in 0..4 {
                        acc[i] = hmad(scale, ch[i] as f32 * (1.0 / 255.0), acc[i]);
                    }
                    sample_x += 1.0;
                }
                sample_y += 1.0;
            }
            // clamp_gamut (premul): alpha (channel 3) to [0,1], colours to [0,a].
            let a = acc[3].clamp(0.0, 1.0);
            let mut out = 0u32;
            for (i, v) in acc.iter().enumerate() {
                let v = if i == 3 { a } else { v.max(0.0).min(a) };
                // srcover onto transparent, then store_8888 (cvtps rounds to even).
                let u = (v.clamp(0.0, 1.0) * 255.0).round_ties_even() as u32;
                out |= u << (8 * i);
            }
            *dst_pm.pixel_mut(px, py) = out;
        }
    }
}
