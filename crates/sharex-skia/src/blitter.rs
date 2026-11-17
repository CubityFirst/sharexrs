//! `SkBlitter` and the legacy N32 blitters Skia picks for a solid colour
//! drawn with srcover on a premultiplied BGRA surface (`SkBlitter::Choose`
//! with `UseLegacyBlitter`): `SkARGB32_Blitter`, `SkARGB32_Opaque_Blitter`
//! and `SkARGB32_Black_Blitter` (`SkBlitter_ARGB32.cpp`), with the row
//! procs they use (`SkBlitRow::Color32`, `blit_mask_d32_a8`).

use crate::geom::{IRect, Rect};
use crate::pixmap::{Pixmap, get_a};

/// The calls a scan converter makes (`SkBlitter`), with Skia's default
/// implementations in terms of each other.
pub trait Blitter {
    fn blit_h(&mut self, x: i32, y: i32, width: i32);

    /// `runs[i]` pixels of alpha `aa[i]` from `x`, until a run of 0.
    fn blit_anti_h(&mut self, x: i32, y: i32, aa: &[u8], runs: &[i16]);

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
        if alpha == 255 {
            self.blit_rect(x, y, 1, height);
        } else {
            for i in 0..height {
                self.blit_anti_h(x, y + i, &[alpha], &[1, 0]);
            }
        }
    }

    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        for i in 0..height {
            self.blit_h(x, y + i, width);
        }
    }

    fn blit_anti_rect(
        &mut self,
        mut x: i32,
        y: i32,
        width: i32,
        height: i32,
        left_alpha: u8,
        right_alpha: u8,
    ) {
        if left_alpha > 0 {
            self.blit_v(x, y, height, left_alpha);
        }
        x += 1;
        if width > 0 {
            self.blit_rect(x, y, width, height);
            x += width;
        }
        if right_alpha > 0 {
            self.blit_v(x, y, height, right_alpha);
        }
    }

    fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u8, a1: u8) {
        self.blit_anti_h(x, y, &[a0, a1], &[1, 1, 0]);
    }

    /// An A8 mask whose rows start at `bounds.left`, blitted inside `clip`.
    fn blit_mask(&mut self, mask: &[u8], bounds: IRect, row_bytes: i32, clip: IRect) {
        for y in clip.top..clip.bottom {
            for x in clip.left..clip.right {
                let a = mask[((y - bounds.top) * row_bytes + (x - bounds.left)) as usize];
                if a != 0 {
                    self.blit_anti_h(x, y, &[a], &[1, 0]);
                }
            }
        }
    }

    /// `SkBlitter::blitFatAntiRect`.
    fn blit_fat_anti_rect(&mut self, rect: Rect) {
        let bounds = rect.round_out();
        if bounds.height() == 0 {
            return;
        }
        let w = bounds.width() as usize;
        let mut runs = vec![0i16; w + 1];
        let mut alphas = vec![0u8; w + 1];
        runs[0] = 1;
        runs[1] = (bounds.width() - 2) as i16;
        runs[w - 1] = 1;
        runs[w] = 0;

        let partial_l = bounds.left as f32 + 1.0 - rect.left;
        let partial_r = rect.right - (bounds.right as f32 - 1.0);
        let mut partial_t = bounds.top as f32 + 1.0 - rect.top;
        let partial_b = rect.bottom - (bounds.bottom as f32 - 1.0);

        if bounds.height() == 1 {
            partial_t = rect.bottom - rect.top;
        }

        alphas[0] = scalar_to_alpha(partial_l * partial_t);
        alphas[1] = scalar_to_alpha(partial_t);
        alphas[w - 1] = scalar_to_alpha(partial_r * partial_t);
        self.blit_anti_h(bounds.left, bounds.top, &alphas, &runs);

        if bounds.height() > 2 {
            self.blit_anti_rect(
                bounds.left,
                bounds.top + 1,
                bounds.width() - 2,
                bounds.height() - 2,
                scalar_to_alpha(partial_l),
                scalar_to_alpha(partial_r),
            );
        }

        if bounds.height() > 1 {
            alphas[0] = scalar_to_alpha(partial_l * partial_b);
            alphas[1] = scalar_to_alpha(partial_b);
            alphas[w - 1] = scalar_to_alpha(partial_r * partial_b);
            self.blit_anti_h(bounds.left, bounds.bottom - 1, &alphas, &runs);
        }
    }
}

fn scalar_to_alpha(a: f32) -> u8 {
    let alpha = (a * 255.0) as i32 as u8;
    if alpha > 247 {
        0xFF
    } else if alpha < 8 {
        0
    } else {
        alpha
    }
}

// --- SkColorPriv / SkColorData ---------------------------------------------

#[inline]
pub fn alpha_mul_q(c: u32, scale: u32) -> u32 {
    let mask = 0x00FF_00FFu32;
    let rb = ((c & mask).wrapping_mul(scale)) >> 8;
    let ag = ((c >> 8) & mask).wrapping_mul(scale);
    (rb & mask) | (ag & !mask)
}

#[inline]
fn alpha_mul_inv256(value: u32, alpha256: u32) -> u32 {
    let prod = 0xFFFFu32.wrapping_sub(value * alpha256);
    (prod + (prod >> 8)) >> 8
}

/// `SkBlendARGB32`.
#[inline]
fn blend_argb32(src: u32, dst: u32, aa: u32) -> u32 {
    let src_scale = aa + 1;
    let dst_scale = alpha_mul_inv256(get_a(src), src_scale);
    let mask = 0x00FF_00FFu32;
    let src_rb = (src & mask).wrapping_mul(src_scale);
    let src_ag = ((src >> 8) & mask).wrapping_mul(src_scale);
    let dst_rb = (dst & mask).wrapping_mul(dst_scale);
    let dst_ag = ((dst >> 8) & mask).wrapping_mul(dst_scale);
    ((src_rb.wrapping_add(dst_rb) >> 8) & mask) | (src_ag.wrapping_add(dst_ag) & !mask)
}

/// `SkFastFourByteInterp` (the 64-bit version).
#[inline]
fn fast_four_byte_interp(src: u32, dst: u32, src_weight: u32) -> u32 {
    let scale = src_weight + (src_weight >> 7);
    let splay = |c: u32| -> u64 {
        let mask = 0x00FF_00FFu32;
        let agrb = ((c >> 8) & mask) as u64;
        (agrb << 32) | (c & mask) as u64
    };
    let v = splay(src) * scale as u64 + (256 - scale) as u64 * splay(dst);
    let mask = 0xFF00_FF00u64;
    (((v & mask) >> 8) | ((v >> 32) & mask)) as u32
}

/// `SkBlitRow::Color32`.
pub fn color32(dst: &mut [u32], color: u32) {
    match get_a(color) {
        0 => {}
        255 => dst.fill(color),
        a => {
            // blit_row_color32
            let mut inv_a = 255 - a;
            inv_a += inv_a >> 7;
            for d in dst.iter_mut() {
                let mut out = 0u32;
                for shift in [0, 8, 16, 24] {
                    let s = (*d >> shift) & 0xFF;
                    let c = (color >> shift) & 0xFF;
                    let v = ((s * inv_a + (c << 8) + 128) >> 8) & 0xFF;
                    out |= v << shift;
                }
                *d = out;
            }
        }
    }
}

/// `approx_scale(x, y)`: `(x*y + x) / 256`.
#[inline]
fn approx_mul_div255(x: u32, y: u32) -> u32 {
    (x * y + x) / 256
}

fn per_channel(c: u32, f: impl Fn(u32, u32) -> u32) -> u32 {
    let mut out = 0;
    for shift in [0, 8, 16, 24] {
        out |= (f((c >> shift) & 0xFF, shift) & 0xFF) << shift;
    }
    out
}

/// `SkPreMultiplyColor` (rounding).
pub fn premultiply_color(a: u32, r: u32, g: u32, b: u32) -> u32 {
    let (r, g, b) = if a != 255 {
        let m = |c: u32| {
            let prod = c * a + 128;
            (prod + (prod >> 8)) >> 8
        };
        (m(r), m(g), m(b))
    } else {
        (r, g, b)
    };
    (a << 24) | (r << 16) | (g << 8) | b
}

/// `blit_mask_d32_a8` (the SSE/Sk4px versions).
fn blit_mask_d32_a8(dst: &mut u32, aa: u32, color_argb: (u32, u32, u32, u32)) {
    let (a, r, g, b) = color_argb;
    let d = *dst;
    if a == 255 && r == 0 && g == 0 && b == 0 {
        // black: (aa & alpha lane) + d * (1 - aa)
        *dst = per_channel(d, |dc, shift| {
            let left = if shift == 24 { aa } else { 0 };
            left + approx_mul_div255(dc, 255 - aa)
        });
    } else if a == 255 {
        let s = premultiply_color(a, r, g, b);
        *dst = per_channel(d, |dc, shift| {
            let sc = (s >> shift) & 0xFF;
            approx_mul_div255(sc, aa) + approx_mul_div255(dc, 255 - aa)
        });
    } else {
        let s = premultiply_color(a, r, g, b);
        let left_a = approx_mul_div255(get_a(s), aa);
        *dst = per_channel(d, |dc, shift| {
            let sc = (s >> shift) & 0xFF;
            approx_mul_div255(sc, aa) + approx_mul_div255(dc, 255 - left_a)
        });
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `SkARGB32_Blitter`: a translucent colour.
    Translucent,
    /// `SkARGB32_Opaque_Blitter`.
    Opaque,
    /// `SkARGB32_Black_Blitter`.
    Black,
}

/// The legacy N32 blitter for a solid colour (an unpremultiplied
/// `SkColor`, as `SkPaint::getColor`), clipped to the pixmap and `clip`.
pub struct ColorBlitter<'a> {
    pm: &'a mut Pixmap,
    clip: IRect,
    kind: Kind,
    /// `fColor`: a, r, g, b unpremultiplied.
    color: (u32, u32, u32, u32),
    pm_color: u32,
    src_a: u32,
}

impl<'a> ColorBlitter<'a> {
    pub fn new(pm: &'a mut Pixmap, a: u8, r: u8, g: u8, b: u8, clip: Option<IRect>) -> Self {
        let (a, r, g, b) = (a as u32, r as u32, g as u32, b as u32);
        let scale = a + 1;
        let pm_color =
            (a << 24) | (((r * scale) >> 8) << 16) | (((g * scale) >> 8) << 8) | ((b * scale) >> 8);
        let kind = if a == 255 && r == 0 && g == 0 && b == 0 {
            Kind::Black
        } else if a == 255 {
            Kind::Opaque
        } else {
            Kind::Translucent
        };
        let full = IRect::from_ltrb(0, 0, pm.width() as i32, pm.height() as i32);
        let mut c = clip.unwrap_or(full);
        if !c.intersect(&full) {
            c = IRect::default();
        }
        Self {
            pm,
            clip: c,
            kind,
            color: (a, r, g, b),
            pm_color,
            src_a: a,
        }
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        x >= self.clip.left && x < self.clip.right && y >= self.clip.top && y < self.clip.bottom
    }

    /// Clips a horizontal span to the clip: (x0, x1) or None.
    fn span(&self, x: i32, y: i32, w: i32) -> Option<(i32, i32)> {
        if y < self.clip.top || y >= self.clip.bottom {
            return None;
        }
        let x0 = x.max(self.clip.left);
        let x1 = (x + w).min(self.clip.right);
        if x0 < x1 { Some((x0, x1)) } else { None }
    }
}

impl Blitter for ColorBlitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        if let Some((x0, x1)) = self.span(x, y, width) {
            let color = self.pm_color;
            color32(self.pm.row_span(y, x0, x1), color);
        }
    }

    fn blit_anti_h(&mut self, x: i32, y: i32, aa: &[u8], runs: &[i16]) {
        if self.kind == Kind::Translucent && self.src_a == 0 {
            return;
        }
        let mut px = x;
        let mut i = 0usize;
        loop {
            let count = runs[i] as i32;
            if count <= 0 {
                return;
            }
            let a = aa[i] as u32;
            if a != 0
                && let Some((x0, x1)) = self.span(px, y, count)
            {
                let color = self.pm_color;
                let row = self.pm.row_span(y, x0, x1);
                match self.kind {
                    Kind::Black => {
                        if a == 255 {
                            row.fill(color);
                        } else {
                            let src = a << 24;
                            let dst_scale = 256 - a;
                            for d in row.iter_mut() {
                                *d = src.wrapping_add(alpha_mul_q(*d, dst_scale));
                            }
                        }
                    }
                    _ => {
                        if (self.src_a & a) == 255 {
                            row.fill(color);
                        } else {
                            let sc = alpha_mul_q(color, a + 1);
                            color32(row, sc);
                        }
                    }
                }
            }
            px += count;
            i += count as usize;
        }
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
        if alpha == 0 || self.src_a == 0 {
            return;
        }
        let mut color = self.pm_color;
        if alpha != 255 {
            color = alpha_mul_q(color, alpha as u32 + 1);
        }
        let dst_scale = 256 - get_a(color);
        for i in 0..height {
            if self.inside(x, y + i) {
                let d = self.pm.pixel_mut(x, y + i);
                *d = color.wrapping_add(alpha_mul_q(*d, dst_scale));
            }
        }
    }

    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        if self.src_a == 0 {
            return;
        }
        for j in 0..height {
            self.blit_h(x, y + j, width);
        }
    }

    fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u8, a1: u8) {
        for (i, a) in [a0, a1].into_iter().enumerate() {
            let px = x + i as i32;
            if !self.inside(px, y) {
                continue;
            }
            let color = self.pm_color;
            let d = self.pm.pixel_mut(px, y);
            *d = match self.kind {
                Kind::Black => ((a as u32) << 24).wrapping_add(alpha_mul_q(*d, 256 - a as u32)),
                Kind::Opaque => fast_four_byte_interp(color, *d, a as u32),
                Kind::Translucent => blend_argb32(color, *d, a as u32),
            };
        }
    }

    fn blit_mask(&mut self, mask: &[u8], bounds: IRect, row_bytes: i32, clip: IRect) {
        if self.kind == Kind::Translucent && self.src_a == 0 {
            return;
        }
        let mut c = clip;
        if !c.intersect(&self.clip) {
            return;
        }
        for y in c.top..c.bottom {
            for x in c.left..c.right {
                let aa = mask[((y - bounds.top) * row_bytes + (x - bounds.left)) as usize] as u32;
                let color = self.color;
                blit_mask_d32_a8(self.pm.pixel_mut(x, y), aa, color);
            }
        }
    }
}

/// The raster pipeline blitter for a `kClear` paint (`SkBlitter::Choose`
/// turns it into `kSrc` with a transparent colour): coverage `c` scales
/// the destination by `1 - c` (lowp `lerp` with `div255` = `(v+255)/256`).
pub struct ClearBlitter<'a> {
    pm: &'a mut Pixmap,
}

impl<'a> ClearBlitter<'a> {
    pub fn new(pm: &'a mut Pixmap) -> Self {
        Self { pm }
    }

    fn lerp(&mut self, x: i32, y: i32, c: u32) {
        if x < 0 || y < 0 || x >= self.pm.width() as i32 || y >= self.pm.height() as i32 {
            return;
        }
        let d = self.pm.pixel_mut(x, y);
        if c == 255 {
            *d = 0;
            return;
        }
        *d = per_channel(*d, |dc, _| (dc * (255 - c) + 255) / 256);
    }
}

impl Blitter for ClearBlitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        for i in 0..width {
            self.lerp(x + i, y, 255);
        }
    }

    fn blit_anti_h(&mut self, x: i32, y: i32, aa: &[u8], runs: &[i16]) {
        let mut px = x;
        let mut i = 0usize;
        loop {
            let count = runs[i] as i32;
            if count <= 0 {
                return;
            }
            let a = aa[i] as u32;
            if a != 0 {
                for k in 0..count {
                    self.lerp(px + k, y, a);
                }
            }
            px += count;
            i += count as usize;
        }
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
        for i in 0..height {
            self.lerp(x, y + i, alpha as u32);
        }
    }

    fn blit_mask(&mut self, mask: &[u8], bounds: IRect, row_bytes: i32, clip: IRect) {
        for y in clip.top..clip.bottom {
            for x in clip.left..clip.right {
                let a = mask[((y - bounds.top) * row_bytes + (x - bounds.left)) as usize] as u32;
                if a != 0 {
                    self.lerp(x, y, a);
                }
            }
        }
    }
}
