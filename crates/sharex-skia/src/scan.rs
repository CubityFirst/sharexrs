//! `SkScan::AntiFillPath`: picks analytic AA or supersampling for a path
//! (`ShouldUseAAA`), as Skia m119 does.

use crate::aaa;
use crate::blitter::Blitter;
use crate::geom::{IRect, Rect};
use crate::path::Path;

/// `ShouldUseAAA` (with `gSkUseAnalyticAA` on, as Skia ships).
pub fn should_use_aaa(path: &Path) -> bool {
    if path.is_rect().is_some() {
        return true;
    }
    let b = path.bounds();
    (path.count_points() as f32) < b.width().max(b.height()) / 2.0 - 10.0
}

/// `SkScan::AntiFillPath` into `blitter`, clipped to `clip`.
pub fn anti_fill_path(path: &Path, clip: IRect, blitter: &mut dyn Blitter) {
    anti_fill_path_rle(path, clip, blitter, false);
}

/// `SkScan::AntiFillPath(path, region, blitter, forceRLE)`: `force_rle`
/// keeps the scan converters off their mask blitters (an `SkAAClip` being
/// built, or a draw through one).
pub fn anti_fill_path_rle(path: &Path, clip: IRect, blitter: &mut dyn Blitter, force_rle: bool) {
    let clip = crate::blitter::device_clip(clip);
    if clip.is_empty() {
        return;
    }
    let ir = path.bounds().round_out();
    if ir.is_empty() {
        return;
    }
    let mut clipped = ir;
    if !clipped.intersect(&clip) {
        return;
    }
    // SkScanClipper: a path that isn't inside the clip draws through an
    // SkRectClipBlitter, which (not overriding blitAntiH2) blends pairs of
    // pixels as blitAntiH does.
    let mut rect_clip;
    let blitter: &mut dyn Blitter =
        if clip.contains(&ir) || !(clip.left > ir.left || clip.right < ir.right) {
            blitter
        } else {
            rect_clip = RectClipBlitter { inner: blitter };
            &mut rect_clip
        };
    if should_use_aaa(path) {
        aaa::aaa_fill_path_entry(path, blitter, ir, clip, force_rle);
    } else {
        crate::saa::saa_fill_path(path, blitter, ir, clip, force_rle);
    }
}

/// `SkRectClipBlitter` (the inner blitter clips to the device itself).
struct RectClipBlitter<'a> {
    inner: &'a mut dyn Blitter,
}

impl Blitter for RectClipBlitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        self.inner.blit_h(x, y, width);
    }
    fn blit_anti_h(&mut self, x: i32, y: i32, aa: &[u8], runs: &[i16]) {
        self.inner.blit_anti_h(x, y, aa, runs);
    }
    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
        self.inner.blit_v(x, y, height, alpha);
    }
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.inner.blit_rect(x, y, width, height);
    }
    fn blit_anti_rect(&mut self, x: i32, y: i32, width: i32, height: i32, la: u8, ra: u8) {
        self.inner.blit_anti_rect(x, y, width, height, la, ra);
    }
    fn blit_mask(&mut self, mask: &[u8], bounds: IRect, row_bytes: i32, clip: IRect) {
        self.inner.blit_mask(mask, bounds, row_bytes, clip);
    }
}

// --- SkScan::AntiFillRect (SkScan_Antihair.cpp) ------------------------------

/// `SkAlphaMul(value, alpha256)`.
fn alpha_mul(value: i32, alpha256: i32) -> i32 {
    (value * alpha256) >> 8
}

/// `call_hline_blitter`.
fn hline(blitter: &mut dyn Blitter, mut x: i32, y: i32, mut count: i32, alpha: i32) {
    const HLINE_STACK_BUFFER: i32 = 100;
    while count > 0 {
        let n = count.min(HLINE_STACK_BUFFER);
        crate::hair::blit_run(blitter, x, y, n, alpha as u8);
        x += n;
        count -= n;
    }
}

/// `do_scanline` (FDot8 edges).
fn do_scanline(l: i32, top: i32, r: i32, alpha: i32, blitter: &mut dyn Blitter) {
    if (l >> 8) == ((r - 1) >> 8) {
        blitter.blit_v(l >> 8, top, 1, alpha_mul(alpha, r - l) as u8);
        return;
    }
    let mut left = l >> 8;
    if l & 0xFF != 0 {
        blitter.blit_v(left, top, 1, alpha_mul(alpha, 256 - (l & 0xFF)) as u8);
        left += 1;
    }
    let rite = r >> 8;
    let width = rite - left;
    if width > 0 {
        hline(blitter, left, top, width, alpha);
    }
    if r & 0xFF != 0 {
        blitter.blit_v(rite, top, 1, alpha_mul(alpha, r & 0xFF) as u8);
    }
}

/// `antifilldot8`.
fn antifilldot8(l: i32, t: i32, r: i32, b: i32, blitter: &mut dyn Blitter, fill_inner: bool) {
    if l >= r || t >= b {
        return;
    }
    let mut top = t >> 8;
    if top == ((b - 1) >> 8) {
        do_scanline(l, top, r, b - t - 1, blitter);
        return;
    }
    if t & 0xFF != 0 {
        do_scanline(l, top, r, 256 - (t & 0xFF), blitter);
        top += 1;
    }
    let bot = b >> 8;
    let height = bot - top;
    if height > 0 {
        let mut left = l >> 8;
        if left == ((r - 1) >> 8) {
            blitter.blit_v(left, top, height, (r - l - 1) as u8);
        } else {
            if l & 0xFF != 0 {
                blitter.blit_v(left, top, height, (256 - (l & 0xFF)) as u8);
                left += 1;
            }
            let rite = r >> 8;
            let width = rite - left;
            if width > 0 && fill_inner {
                blitter.blit_rect(left, top, width, height);
            }
            if r & 0xFF != 0 {
                blitter.blit_v(rite, top, height, (r & 0xFF) as u8);
            }
        }
    }
    if b & 0xFF != 0 {
        do_scanline(l, bot, r, b & 0xFF, blitter);
    }
}

/// `SkScan::AntiFillRect` with a rectangular clip.
pub fn anti_fill_rect(r: Rect, clip: IRect, blitter: &mut dyn Blitter) {
    let clip = crate::blitter::device_clip(clip);
    // antifillrect(const SkRect&): XRect_set (SkScalarToFixed, truncating),
    // then SkFixedToFDot8, which rounds: (x + 0x80) >> 8.
    let fdot8 = |v: f32| {
        let fixed = (v * 65536.0) as i32;
        (fixed + 0x80) >> 8
    };
    let fill = |r: Rect, blitter: &mut dyn Blitter| {
        antifilldot8(
            fdot8(r.left),
            fdot8(r.top),
            fdot8(r.right),
            fdot8(r.bottom),
            blitter,
            true,
        )
    };
    let outer = r.round_out();
    if clip.contains(&outer) {
        fill(r, blitter);
    } else {
        // Keeps the original edges fractional.
        let c = Rect::from_ltrb(
            clip.left as f32,
            clip.top as f32,
            clip.right as f32,
            clip.bottom as f32,
        );
        let i = Rect::from_ltrb(
            c.left.max(r.left),
            c.top.max(r.top),
            c.right.min(r.right),
            c.bottom.min(r.bottom),
        );
        if i.left < i.right && i.top < i.bottom {
            fill(i, blitter);
        }
    }
}

/// `SkAlphaMulRound` (`SkMulDiv255Round`).
fn alpha_mul_round(a: i32, b: i32) -> i32 {
    let prod = a * b + 128;
    (prod + (prod >> 8)) >> 8
}

/// `InvAlphaMul`: `1 - (1 - a) * (1 - b)`.
fn inv_alpha_mul(a: i32, b: i32) -> u8 {
    (a + b - alpha_mul_round(a, b)) as u8
}

/// `inner_scanline` (SkScan_Antihair.cpp).
fn inner_scanline(l: i32, top: i32, r: i32, alpha: i32, blitter: &mut dyn Blitter) {
    if (l >> 8) == ((r - 1) >> 8) {
        let mut wid = r - l;
        wid -= wid >> 8;
        blitter.blit_v(l >> 8, top, 1, inv_alpha_mul(alpha, wid));
        return;
    }
    let mut left = l >> 8;
    if l & 0xFF != 0 {
        blitter.blit_v(left, top, 1, inv_alpha_mul(alpha, l & 0xFF));
        left += 1;
    }
    let rite = r >> 8;
    let width = rite - left;
    if width > 0 {
        hline(blitter, left, top, width, alpha);
    }
    if r & 0xFF != 0 {
        blitter.blit_v(rite, top, 1, inv_alpha_mul(alpha, !r & 0xFF));
    }
}

/// `innerstrokedot8`.
fn innerstrokedot8(l: i32, t: i32, r: i32, b: i32, blitter: &mut dyn Blitter) {
    let mut top = t >> 8;
    if top == ((b - 1) >> 8) {
        let alpha = 256 - (b - t);
        if alpha != 0 {
            inner_scanline(l, top, r, alpha, blitter);
        }
        return;
    }
    if t & 0xFF != 0 {
        inner_scanline(l, top, r, t & 0xFF, blitter);
        top += 1;
    }
    let bot = b >> 8;
    let height = bot - top;
    if height > 0 {
        if l & 0xFF != 0 {
            blitter.blit_v(l >> 8, top, height, (l & 0xFF) as u8);
        }
        if r & 0xFF != 0 {
            blitter.blit_v(r >> 8, top, height, (!r & 0xFF) as u8);
        }
    }
    if b & 0xFF != 0 {
        inner_scanline(l, bot, r, !b & 0xFF, blitter);
    }
}

/// `SkScan::AntiFrameRect`: an anti-aliased stroked rectangle (`r` the
/// rectangle's centre line, `stroke` the device stroke size). The clip is
/// applied by the blitter (rectangular here).
pub fn anti_frame_rect(r: Rect, stroke_x: f32, stroke_y: f32, blitter: &mut dyn Blitter) {
    let to = |x: f32| (x * 256.0) as i32;
    let floor8 = |x: i32| x >> 8;
    let ceil8 = |x: i32| (x + 0xFF) >> 8;
    let mut rx = stroke_x * 0.5;
    let mut ry = stroke_y * 0.5;
    let mut outer_l = to(r.left - rx);
    let mut outer_t = to(r.top - ry);
    let mut outer_r = to(r.right + rx);
    let mut outer_b = to(r.bottom + ry);
    rx = stroke_x - rx;
    ry = stroke_y - ry;
    let mut inner_l = to(r.left + rx);
    let mut inner_t = to(r.top + ry);
    let mut inner_r = to(r.right - rx);
    let mut inner_b = to(r.bottom - ry);
    let align = |e1: &mut i32, e2: &mut i32| {
        if floor8(*e1) == floor8(*e2) {
            *e2 -= *e1 & 0xFF;
            *e1 &= !0xFF;
        }
    };
    if stroke_x < 1.0 || stroke_y < 1.0 {
        align(&mut outer_l, &mut inner_l);
        align(&mut outer_t, &mut inner_t);
        align(&mut inner_r, &mut outer_r);
        align(&mut inner_b, &mut outer_b);
    }
    antifilldot8(outer_l, outer_t, outer_r, outer_b, blitter, false);
    let (ol, ot, or, ob) = (ceil8(outer_l), ceil8(outer_t), floor8(outer_r), floor8(outer_b));
    let fillcheck = |l: i32, t: i32, r: i32, b: i32, blitter: &mut dyn Blitter| {
        if l < r && t < b {
            blitter.blit_rect(l, t, r - l, b - t);
        }
    };
    if inner_l >= inner_r || inner_t >= inner_b {
        fillcheck(ol, ot, or, ob, blitter);
    } else {
        let (il, it, ir, ib) = (floor8(inner_l), floor8(inner_t), ceil8(inner_r), ceil8(inner_b));
        fillcheck(ol, ot, or, it, blitter);
        fillcheck(ol, it, il, ib, blitter);
        fillcheck(ir, it, or, ib, blitter);
        fillcheck(ol, ib, or, ob, blitter);
        innerstrokedot8(inner_l, inner_t, inner_r, inner_b, blitter);
    }
}
