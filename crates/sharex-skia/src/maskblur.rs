//! `SKMaskFilter.CreateBlur(Normal, sigma)` on the raster backend, as Skia
//! m119 runs it: `SkMaskFilterBase::filterPath` (`SkMaskFilter.cpp`) draws
//! a path into an A8 mask and blurs it with `SkMaskBlurFilter`
//! (`SkMaskBlurFilter.cpp`: a 1D Gaussian below sigma 2, three box passes
//! above), or, for one rect or two nested ones, blurs a small copy
//! (`SkBlurMaskFilterImpl::filterRectsToNine`, with `SkBlurMask::BlurRect`'s
//! analytic profile for a single rect) and stretches it as a nine-patch.

use crate::blitter::Blitter;
use crate::geom::{IRect, Rect};
use crate::path::Path;

/// An A8 `SkMask`.
#[derive(Clone, Debug)]
pub struct Mask {
    pub bounds: IRect,
    pub row_bytes: usize,
    pub image: Vec<u8>,
}

impl Mask {
    fn zeroed(bounds: IRect, row_bytes: usize) -> Mask {
        let h = bounds.height().max(0) as usize;
        Mask {
            bounds,
            row_bytes,
            image: vec![0; row_bytes * h],
        }
    }

    /// `getAddr8(x, y)`.
    fn addr(&self, x: i32, y: i32) -> usize {
        (y - self.bounds.top) as usize * self.row_bytes + (x - self.bounds.left) as usize
    }
}

// --- SkGaussFilter ---------------------------------------------------------

/// `SkGaussFilter(sigma)`'s factors (0 <= sigma < 2).
fn gauss_factors(sigma: f64) -> Vec<f64> {
    const GOOD_ENOUGH: f64 = 1.0 / 100.0;
    let var = sigma * sigma;
    let bessel_i0 = |t: f64| {
        let t2_4 = t * t / 4.0;
        let (mut sum, mut factor, mut k) = (1.0f64, 1.0f64, 1.0f64);
        while factor > 1.0 / 1_000_000.0 {
            factor *= t2_4 / (k * k);
            sum += factor;
            k += 1.0;
        }
        sum
    };
    let bessel_i1 = |t: f64| {
        let t2_4 = t * t / 4.0;
        let mut sum = t / 2.0;
        let mut factor = sum;
        let mut k = 1.0f64;
        while factor > 1.0 / 1_000_000.0 {
            factor *= t2_4 / (k * (k + 1.0));
            sum += factor;
            k += 1.0;
        }
        sum
    };
    let d = var.exp();
    let mut b = [0.0f64; 6];
    let mut gauss = [0.0f64; 6];
    b[0] = bessel_i0(var);
    b[1] = bessel_i1(var);
    gauss[0] = b[0] / d;
    gauss[1] = b[1] / d;
    let mut n = 1usize;
    while gauss[n] > GOOD_ENOUGH {
        b[n + 1] = -(2.0 * n as f64 / var) * b[n] + b[n - 1];
        gauss[n + 1] = b[n + 1] / d;
        n += 1;
    }
    // normalize
    let mut sum = 0.0f64;
    for i in (1..n).rev() {
        sum += 2.0 * gauss[i];
    }
    sum += gauss[0];
    for g in gauss.iter_mut().take(n) {
        *g /= sum;
    }
    let mut sum = 0.0f64;
    for i in (1..n).rev() {
        sum += 2.0 * gauss[i];
    }
    gauss[0] = 1.0 - sum;
    gauss[..n].to_vec()
}

// --- SkMaskBlurFilter ------------------------------------------------------

/// `PlanGauss`: three box passes approximating a Gaussian.
struct PlanGauss {
    weight: u64,
    border: i32,
    sliding_window: i32,
    pass0: usize,
    pass1: usize,
    pass2: usize,
}

impl PlanGauss {
    fn new(sigma: f64) -> Self {
        let k_pi = std::f64::consts::PI;
        let possible = (sigma * 3.0 * (2.0 * k_pi).sqrt() / 4.0 + 0.5).floor() as i32;
        let window = 1.max(possible);
        let pass0 = (window - 1) as usize;
        let pass1 = (window - 1) as usize;
        let pass2 = if window & 1 == 1 { window - 1 } else { window } as usize;
        let border = if window & 1 == 1 { 3 * ((window - 1) / 2) } else { 3 * (window / 2) - 1 };
        let window2 = window * window;
        let window3 = window2 * window;
        let divisor = if window & 1 == 1 { window3 } else { window3 + window2 };
        let weight = (1.0 / divisor as f64 * (1u64 << 32) as f64).round() as u64;
        PlanGauss {
            weight,
            border,
            sliding_window: 2 * border + 1,
            pass0,
            pass1,
            pass2,
        }
    }

    fn final_scale(&self, sum: u32) -> u8 {
        ((self.weight.wrapping_mul(sum as u64).wrapping_add(1u64 << 31)) >> 32) as u8
    }

    /// `Scan::blur`: `src` read in order, written to `dst[dst_start + k * stride]`
    /// for `k` in `0..dst_count`.
    fn blur(&self, src: &[u8], dst: &mut [u8], dst_start: usize, stride: usize, dst_count: usize) {
        let src_w = src.len();
        let no_change = if self.sliding_window as usize > src_w { self.sliding_window as usize - src_w } else { 0 };
        let (b0, b1, b2) = (0usize, self.pass0, self.pass0 + self.pass1);
        let (e0, e1, e2) = (self.pass0, self.pass0 + self.pass1, self.pass0 + self.pass1 + self.pass2);
        let mut buf = vec![0u32; e2];
        let (mut c0, mut c1, mut c2) = (b0, b1, b2);
        let (mut sum0, mut sum1, mut sum2) = (0u32, 0u32, 0u32);
        let step = |edge: u32, buf: &mut Vec<u32>, c0: &mut usize, c1: &mut usize, c2: &mut usize, s0: &mut u32, s1: &mut u32, s2: &mut u32| -> u8 {
            *s0 = s0.wrapping_add(edge);
            *s1 = s1.wrapping_add(*s0);
            *s2 = s2.wrapping_add(*s1);
            let out = self.final_scale(*s2);
            *s2 = s2.wrapping_sub(buf[*c2]);
            buf[*c2] = *s1;
            *c2 = if *c2 + 1 < e2 { *c2 + 1 } else { b2 };
            *s1 = s1.wrapping_sub(buf[*c1]);
            buf[*c1] = *s0;
            *c1 = if *c1 + 1 < e1 { *c1 + 1 } else { b1 };
            *s0 = s0.wrapping_sub(buf[*c0]);
            buf[*c0] = edge;
            *c0 = if *c0 + 1 < e0 { *c0 + 1 } else { b0 };
            out
        };
        let mut k = 0usize;
        for &s in src {
            let v = step(s as u32, &mut buf, &mut c0, &mut c1, &mut c2, &mut sum0, &mut sum1, &mut sum2);
            if k < dst_count {
                dst[dst_start + k * stride] = v;
            }
            k += 1;
        }
        for _ in 0..no_change {
            let v = step(0, &mut buf, &mut c0, &mut c1, &mut c2, &mut sum0, &mut sum1, &mut sum2);
            if k < dst_count {
                dst[dst_start + k * stride] = v;
            }
            k += 1;
        }
        // From the right, fill in the rest (the cursors carry on where they were).
        buf.iter_mut().for_each(|v| *v = 0);
        sum0 = 0;
        sum1 = 0;
        sum2 = 0;
        let mut cursor = dst_count;
        let mut si = src_w;
        while cursor > k {
            cursor -= 1;
            si -= 1;
            let v = step(src[si] as u32, &mut buf, &mut c0, &mut c1, &mut c2, &mut sum0, &mut sum1, &mut sum2);
            dst[dst_start + cursor * stride] = v;
        }
    }
}

/// `mulhi` on 16-bit lanes.
fn mulhi(a: u16, b: u16) -> u16 {
    ((a as u32 * b as u32) >> 16) as u16
}

/// The 8.8 fixed-point Gaussian of `small_blur` along one line: `src` (A8)
/// into `dst` (`src.len() + 2r` values).
fn gauss_line(src: &[u8], g: &[u16], radius: usize, dst: &mut [u8]) {
    let n = src.len() + 2 * radius;
    for (j, d) in dst.iter_mut().enumerate().take(n) {
        let mut acc: u16 = 0x80;
        for t in 0..=2 * radius {
            if j >= t && j - t < src.len() {
                let s = (src[j - t] as u16) << 8;
                let gi = (t as i32 - radius as i32).unsigned_abs() as usize;
                acc = acc.wrapping_add(mulhi(s, g[gi]));
            }
        }
        *d = (acc >> 8) as u8;
    }
}

/// `SkMaskBlurFilter` for equal sigmas.
pub struct MaskBlurFilter {
    sigma: f64,
}

impl MaskBlurFilter {
    pub fn new(sigma: f64) -> Self {
        MaskBlurFilter { sigma: sigma.clamp(0.0, 135.0) }
    }

    pub fn has_no_blur(&self) -> bool {
        3.0 * self.sigma <= 1.0
    }

    /// The border `blur` adds on each side.
    pub fn margin(&self) -> i32 {
        if self.sigma < 2.0 {
            gauss_factors(self.sigma).len() as i32 - 1
        } else {
            PlanGauss::new(self.sigma).border
        }
    }

    /// `SkMaskBuilder::PrepareDestination`.
    fn prepare(radius: i32, src: &Mask) -> Mask {
        let w = src.bounds.width() + 2 * radius;
        let h = src.bounds.height() + 2 * radius;
        let bounds = IRect::from_ltrb(
            src.bounds.left - radius,
            src.bounds.top - radius,
            src.bounds.left - radius + w,
            src.bounds.top - radius + h,
        );
        Mask::zeroed(bounds, w.max(0) as usize)
    }

    /// `SkMaskBlurFilter::blur` of an A8 mask.
    pub fn blur(&self, src: &Mask) -> (Mask, i32) {
        if self.sigma < 2.0 {
            let factors = gauss_factors(self.sigma);
            let r = factors.len() - 1;
            let g: Vec<u16> = factors.iter().map(|d| (d * 65536.0).round() as u16).collect();
            let mut dst = Self::prepare(r as i32, src);
            let (sw, sh) = (src.bounds.width() as usize, src.bounds.height() as usize);
            let (dw, dh) = (dst.bounds.width() as usize, dst.bounds.height() as usize);
            // Vertically into columns r..r+sw, then horizontally in place.
            let mut col = vec![0u8; sh];
            let mut out = vec![0u8; sh + 2 * r];
            for x in 0..sw {
                for (y, c) in col.iter_mut().enumerate() {
                    *c = src.image[y * src.row_bytes + x];
                }
                gauss_line(&col, &g, r, &mut out);
                for (y, v) in out.iter().enumerate() {
                    dst.image[y * dst.row_bytes + r + x] = *v;
                }
            }
            let mut row = vec![0u8; sw];
            let mut out = vec![0u8; dw];
            for y in 0..dh {
                row.copy_from_slice(&dst.image[y * dst.row_bytes + r..y * dst.row_bytes + r + sw]);
                gauss_line(&row, &g, r, &mut out);
                dst.image[y * dst.row_bytes..y * dst.row_bytes + dw].copy_from_slice(&out);
            }
            return (dst, r as i32);
        }
        let plan = PlanGauss::new(self.sigma);
        let border = plan.border;
        let mut dst = Self::prepare(border, src);
        let (sw, sh) = (src.bounds.width() as usize, src.bounds.height() as usize);
        let (dw, dh) = (dst.bounds.width() as usize, dst.bounds.height() as usize);
        let (tmp_w, tmp_h) = (sh, dw);
        let mut tmp = vec![0u8; tmp_w * tmp_h];
        // Blur horizontally, and transpose.
        for y in 0..sh {
            let row = &src.image[y * src.row_bytes..y * src.row_bytes + sw];
            plan.blur(row, &mut tmp, y, tmp_w, tmp_h);
        }
        // Blur vertically, and transpose back.
        let rb = dst.row_bytes;
        for y in 0..tmp_h {
            let line = tmp[y * tmp_w..(y + 1) * tmp_w].to_vec();
            plan.blur(&line, &mut dst.image, y, rb, dh);
        }
        (dst, border)
    }
}

/// `SkBlurMask::BoxBlur` (normal style): None when there's no blur.
pub fn box_blur(src: &Mask, sigma: f32) -> Option<(Mask, i32)> {
    let f = MaskBlurFilter::new(sigma as f64);
    if f.has_no_blur() {
        return None;
    }
    Some(f.blur(src))
}

/// `filterMask` with a null image: the blurred bounds and margin.
fn box_blur_bounds(src_bounds: IRect, sigma: f32) -> Option<(IRect, i32)> {
    let f = MaskBlurFilter::new(sigma as f64);
    if f.has_no_blur() {
        return None;
    }
    let m = f.margin();
    Some((
        IRect::from_ltrb(src_bounds.left - m, src_bounds.top - m, src_bounds.right + m, src_bounds.bottom + m),
        m,
    ))
}

// --- SkBlurMask::BlurRect ----------------------------------------------------

fn gaussian_integral(x: f32) -> f32 {
    if x > 1.5 {
        return 0.0;
    }
    if x < -1.5 {
        return 1.0;
    }
    let x2 = x * x;
    let x3 = x2 * x;
    if x > 0.5 {
        return 0.5625 - (x3 / 6.0 - 3.0 * x2 * 0.25 + 1.125 * x);
    }
    if x > -0.5 {
        return 0.5 - (0.75 * x - x3 / 3.0);
    }
    0.4375 + (-x3 / 6.0 - 3.0 * x2 * 0.25 - 1.125 * x)
}

fn blur_profile(size: i32, sigma: f32) -> Vec<u8> {
    let mut profile = vec![0u8; size as usize];
    let center = size >> 1;
    let invr = 1.0 / (2.0 * sigma);
    profile[0] = 255;
    for x in 1..size {
        let scaled = (center - x) as f32 - 0.5;
        let gi = gaussian_integral(scaled * invr);
        profile[x as usize] = 255 - (255.0 * gi) as u8;
    }
    profile
}

fn blurred_scanline(profile: &[u8], width: u32, sigma: f32) -> Vec<u8> {
    let profile_size = (6.0 * sigma).ceil() as i32 as u32;
    let sw = width.wrapping_sub(profile_size);
    let center = (profile_size & !1) as i32 - 1;
    let w = (sw as i32).wrapping_sub(center);
    (0..width)
        .map(|x| {
            if profile_size <= sw {
                // ProfileLookup
                let dx = (((x as i32) << 1) + 1 - width as i32).abs() - w;
                let ox = (dx >> 1).max(0);
                profile[ox as usize]
            } else {
                let span = sw as f32 / (2.0 * sigma);
                let gi_x = 1.5 - (x as f32 + 0.5) / (2.0 * sigma);
                (255.0 * (gaussian_integral(gi_x) - gaussian_integral(gi_x + span))) as u8
            }
        })
        .collect()
}

/// `SkBlurMask::BlurRect` (normal style): the mask bounds, and the image
/// when `render`.
fn blur_rect(sigma: f32, src: Rect, render: bool) -> Option<(IRect, Option<Mask>, i32)> {
    let profile_size = (6.0 * sigma).ceil() as i32;
    if profile_size <= 0 {
        return None;
    }
    let pad = profile_size / 2;
    let round = |v: f32| (v + 0.5).floor() as i32;
    let p = pad as f32;
    let bounds = IRect::from_ltrb(round(src.left - p), round(src.top - p), round(src.right + p), round(src.bottom + p));
    if !render {
        return Some((bounds, None, pad));
    }
    let profile = blur_profile(profile_size, sigma);
    let (dw, dh) = (bounds.width(), bounds.height());
    let hs = blurred_scanline(&profile, dw as u32, sigma);
    let vs = blurred_scanline(&profile, dh as u32, sigma);
    let mut mask = Mask::zeroed(bounds, dw as usize);
    for y in 0..dh as usize {
        for x in 0..dw as usize {
            mask.image[y * dw as usize + x] = mul_div_255_round(hs[x] as u32, vs[y] as u32) as u8;
        }
    }
    Some((bounds, Some(mask), pad))
}

fn mul_div_255_round(a: u32, b: u32) -> u32 {
    let prod = a * b + 128;
    (prod + (prod >> 8)) >> 8
}

// --- drawing into a mask -------------------------------------------------------

/// `SkA8_Blitter` (src-over, opaque paint) over a mask.
struct A8Blitter<'a> {
    mask: &'a mut Mask,
}

fn a8_div255(prod: u32) -> u32 {
    ((prod + 128) * 257) >> 16
}

impl A8Blitter<'_> {
    fn px(&mut self, x: i32, y: i32) -> &mut u8 {
        let i = self.mask.addr(x, y);
        &mut self.mask.image[i]
    }

    fn aa(&mut self, x: i32, y: i32, aa: u8) {
        let src = a8_div255(255 * aa as u32);
        let d = self.px(x, y);
        *d = (src + a8_div255((255 - src) * *d as u32)) as u8;
    }
}

impl Blitter for A8Blitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        for i in 0..width {
            *self.px(x + i, y) = 255;
        }
    }

    fn blit_anti_h(&mut self, x: i32, y: i32, aa: &[u8], runs: &[i16]) {
        let mut i = 0usize;
        let mut px = x;
        while runs[i] > 0 {
            let n = runs[i] as i32;
            let a = aa[i];
            for k in 0..n {
                if a == 255 {
                    *self.px(px + k, y) = 255;
                } else if a != 0 {
                    self.aa(px + k, y, a);
                }
            }
            px += n;
            i += n as usize;
        }
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
        for k in 0..height {
            if alpha == 255 {
                *self.px(x, y + k) = 255;
            } else if alpha != 0 {
                self.aa(x, y + k, alpha);
            }
        }
    }

    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        for j in 0..height {
            self.blit_h(x, y + j, width);
        }
    }

    fn blit_mask(&mut self, mask: &[u8], bounds: IRect, row_bytes: i32, clip: IRect) {
        for y in clip.top..clip.bottom {
            for x in clip.left..clip.right {
                let m = mask[((y - bounds.top) * row_bytes + (x - bounds.left)) as usize] as u32;
                let d = self.px(x, y);
                *d = a8_div255((255 - m) * *d as u32 + m * 255) as u8;
            }
        }
    }
}

/// Draws `path` (device space) anti-aliased into a zeroed mask of `bounds`
/// (`draw_into_mask`): a fill, or a hairline with `cap`.
fn draw_path_into_mask(path: &Path, bounds: IRect, hairline: Option<crate::Cap>) -> Mask {
    let mut mask = Mask::zeroed(bounds, bounds.width().max(0) as usize);
    let local = path.translated(-bounds.left as f32, -bounds.top as f32);
    let clip = IRect::from_ltrb(0, 0, bounds.width(), bounds.height());
    {
        let mut shifted = Mask {
            bounds: IRect::from_ltrb(0, 0, bounds.width(), bounds.height()),
            row_bytes: mask.row_bytes,
            image: std::mem::take(&mut mask.image),
        };
        {
            let mut b = A8Blitter { mask: &mut shifted };
            crate::blitter::with_no_device_clip(|| match hairline {
                Some(cap) => crate::hair::anti_hair_path(&local, cap, clip, &mut b),
                None => crate::scan::anti_fill_path(&local, clip, &mut b),
            });
        }
        mask.image = shifted.image;
    }
    mask
}

// --- the nine-patch ------------------------------------------------------------

struct NinePatch {
    mask: Mask,
    outer: IRect,
    center: (i32, i32),
}

/// `SkBlurMaskFilterImpl::filterRectsToNine` (normal style): Err(()) for
/// `kUnimplemented` (draw the path's mask instead), Ok(None) for `kFalse`.
fn filter_rects_to_nine(rects: &[Rect], sigma: f32) -> Result<Option<NinePatch>, ()> {
    let count = rects.len();
    let exceeds = |r: &Rect, v: f32| {
        r.left < -v || r.top < -v || r.right > v || r.bottom > v || r.width() > v || r.height() > v
    };
    if exceeds(&rects[0], 32767.0) {
        return Err(());
    }
    let src_bounds = rects[0].round_out();
    let (dst_bounds, margin) = if count == 1 {
        match blur_rect(sigma, rects[0], false) {
            Some((b, _, m)) => (b, m),
            None => return Ok(None),
        }
    } else {
        match box_blur_bounds(src_bounds, sigma) {
            Some(v) => v,
            None => return Ok(None),
        }
    };
    let _ = margin;
    let mut small_w = dst_bounds.width() - src_bounds.width() + 2;
    let mut small_h = dst_bounds.height() - src_bounds.height() + 2;
    let (inner, center) = if count == 1 {
        (src_bounds, (small_w, small_h))
    } else {
        // roundIn
        let r = &rects[1];
        let inner = IRect::from_ltrb(r.left.ceil() as i32, r.top.ceil() as i32, r.right.floor() as i32, r.bottom.floor() as i32);
        let c = (small_w + (inner.left - src_bounds.left), small_h + (inner.top - src_bounds.top));
        (inner, c)
    };
    small_w += 1;
    small_h += 1;
    let dx = (inner.width() - small_w) as f32;
    let dy = (inner.height() - small_h) as f32;
    if dx < 0.0 || dy < 0.0 {
        return Err(());
    }
    let r0 = Rect::from_ltrb(rects[0].left, rects[0].top, rects[0].right - dx, rects[0].bottom - dy);
    if r0.width() < 2.0 || r0.height() < 2.0 {
        return Err(());
    }
    let filtered = if count == 1 {
        match blur_rect(sigma, r0, true) {
            Some((_, Some(m), _)) => m,
            _ => return Ok(None),
        }
    } else {
        let r1 = Rect::from_ltrb(rects[1].left, rects[1].top, rects[1].right - dx, rects[1].bottom - dy);
        // draw_rects_into_mask: both rects, even-odd, into a mask of r0's bounds.
        let mut path = Path::new();
        path.add_rect(r0, crate::Direction::Cw, 0);
        path.add_rect(r1, crate::Direction::Cw, 0);
        path.fill_type = crate::FillType::EvenOdd;
        let src = draw_path_into_mask(&path, r0.round_out(), None);
        match box_blur(&src, sigma) {
            Some((m, _)) => m,
            None => return Ok(None),
        }
    };
    let mut mask = filtered;
    let w = mask.bounds.width();
    let h = mask.bounds.height();
    mask.bounds = IRect::from_ltrb(0, 0, w, h);
    Ok(Some(NinePatch {
        mask,
        outer: dst_bounds,
        center,
    }))
}

/// `draw_nine_clipped`.
fn draw_nine(patch: &NinePatch, fill_center: bool, clip: IRect, blitter: &mut dyn Blitter) {
    let mask = &patch.mask;
    let outer = patch.outer;
    let (cx, cy) = patch.center;
    let mb = mask.bounds;
    let rb = mask.row_bytes as i32;
    let blit_sub = |sub: IRect, at_x: i32, at_y: i32, blitter: &mut dyn Blitter| {
        if sub.width() > 0 && sub.height() > 0 {
            let off = mask.addr(sub.left, sub.top);
            let bounds = IRect::from_ltrb(at_x, at_y, at_x + sub.width(), at_y + sub.height());
            let mut r = bounds;
            if r.intersect(&clip) {
                blitter.blit_mask(&mask.image[off..], bounds, rb, r);
            }
        }
    };
    // top-left, top-right, bottom-left, bottom-right
    let tl = IRect::from_ltrb(mb.left, mb.top, cx, cy);
    blit_sub(tl, outer.left, outer.top, blitter);
    let tr = IRect::from_ltrb(cx + 1, mb.top, mb.right, cy);
    blit_sub(tr, outer.right - tr.width(), outer.top, blitter);
    let bl = IRect::from_ltrb(mb.left, cy + 1, cx, mb.bottom);
    blit_sub(bl, outer.left, outer.bottom - bl.height(), blitter);
    let br = IRect::from_ltrb(cx + 1, cy + 1, mb.right, mb.bottom);
    blit_sub(br, outer.right - br.width(), outer.bottom - br.height(), blitter);

    let inner = IRect::from_ltrb(
        outer.left + cx - mb.left,
        outer.top + cy - mb.top,
        outer.right + (cx + 1 - mb.right),
        outer.bottom + (cy + 1 - mb.bottom),
    );
    if fill_center {
        let mut r = inner;
        if r.intersect(&clip) {
            blitter.blit_rect(r.left, r.top, r.width(), r.height());
        }
    }
    // top
    let mut r = IRect::from_ltrb(inner.left, outer.top, inner.right, inner.top);
    if r.intersect(&clip) {
        let start = 0.max(r.top - outer.top);
        let width = r.width();
        for y in start..start + r.height() {
            let mut runs = vec![0i16; width as usize + 1];
            let mut aa = vec![0u8; width as usize + 1];
            runs[0] = width as i16;
            aa[0] = mask.image[mask.addr(cx, mb.top + y)];
            blitter.blit_anti_h(r.left, outer.top + y, &aa, &runs);
        }
    }
    // bottom
    let mut r = IRect::from_ltrb(inner.left, inner.bottom, inner.right, outer.bottom);
    if r.intersect(&clip) {
        let start = outer.bottom - r.bottom;
        let width = r.width();
        for y in start..start + r.height() {
            let mut runs = vec![0i16; width as usize + 1];
            let mut aa = vec![0u8; width as usize + 1];
            runs[0] = width as i16;
            aa[0] = mask.image[mask.addr(cx, mb.bottom - y - 1)];
            blitter.blit_anti_h(r.left, outer.bottom - y - 1, &aa, &runs);
        }
    }
    // left: one scanline repeated (row bytes 0)
    let mut r = IRect::from_ltrb(outer.left, inner.top, inner.left, inner.bottom);
    if r.intersect(&clip) {
        let off = mask.addr(mb.left + r.left - outer.left, mb.top + cy);
        blitter.blit_mask(&mask.image[off..], r, 0, r);
    }
    // right
    let mut r = IRect::from_ltrb(inner.right, inner.top, outer.right, inner.bottom);
    if r.intersect(&clip) {
        let off = mask.addr(mb.right - outer.right + r.left, mb.top + cy);
        blitter.blit_mask(&mask.image[off..], r, 0, r);
    }
}

/// `SkMaskFilterBase::filterPath` for a normal blur of (device-space)
/// `sigma`: a fill of `path`, or (`hairline`) its hairline. False when
/// Skia would draw it without the filter (no effective blur).
pub fn filter_path(path: &Path, sigma: f32, hairline: Option<crate::Cap>, clip: IRect, blitter: &mut dyn Blitter) -> bool {
    if hairline.is_none() {
        let rects: Option<Vec<Rect>> = path
            .nested_fill_rects()
            .map(|r| r.to_vec())
            .or_else(|| path.is_rect().map(|r| vec![r]));
        if let Some(rects) = rects {
            match filter_rects_to_nine(&rects, sigma) {
                Ok(None) => return false,
                Ok(Some(patch)) => {
                    let mut c = patch.outer;
                    if c.intersect(&clip) {
                        draw_nine(&patch, rects.len() == 1, clip, blitter);
                    }
                    return true;
                }
                Err(()) => {}
            }
        }
    }
    if path.is_empty() {
        return false;
    }
    // DrawToMask: ComputeMaskBounds (path bounds outset by 1/2, rounded out,
    // trimmed to the clip plus the filter's margin, at most 128).
    let pb = path.bounds();
    let mut bounds = Rect::from_ltrb(pb.left - 0.5, pb.top - 0.5, pb.right + 0.5, pb.bottom + 0.5).round_out();
    let Some((_, margin)) = box_blur_bounds(bounds, sigma) else {
        return false;
    };
    let m = margin.min(128);
    if !bounds.intersect(&IRect::from_ltrb(clip.left - m, clip.top - m, clip.right + m, clip.bottom + m)) {
        return false;
    }
    let src = draw_path_into_mask(path, bounds, hairline);
    let Some((dst, _)) = box_blur(&src, sigma) else {
        return false;
    };
    let mut r = dst.bounds;
    if r.intersect(&clip) {
        blitter.blit_mask(&dst.image, dst.bounds, dst.row_bytes as i32, r);
    }
    true
}
