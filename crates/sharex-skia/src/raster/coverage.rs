//! Which pixels a draw touches, how much, and through which blitter call: Skia's scan
//! conversion. Rects: `SkScan::FillRect` (rounded edges) and `SkScan::AntiFillRect`
//! (fractional edges); anti-aliased paths: `SkScan::AntiFillPath` (analytic AA or the
//! supersampler, [`crate::scan`]); aliased paths: tiny-skia, whose non-AA scan
//! converter is Skia's.
//!
//! The call matters: Skia's legacy blitters blend the same coverage differently in
//! `blitAntiH`, `blitV`, `blitAntiH2` and `blitMask`.

use super::Rect;
use crate::blitter::{Blitter, Call};
use crate::geom::IRect;

/// Coverage over the destination: full rows of spans, or a per-pixel record.
pub(crate) enum Coverage {
    /// Every pixel of `[left, right) x [top, bottom)` fully covered (clipped).
    Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    },
    /// Per pixel of the destination: the call that covered it (`None`: untouched)
    /// and its coverage; then per pixel the x where its span starts (the
    /// `shadeSpan` call Skia's shader blitters make for it: a `blitH` span,
    /// a `blitAntiH` run, the mask row's clip, or the pixel alone for
    /// `blitV`/`blitAntiH2`). Legacy image shaders step their coordinates
    /// from a span's start, so it changes the pixels.
    Mask(Vec<Option<(Call, u8)>>, Vec<i32>),
    /// Each blit in order, for scan converters that can cover a pixel more
    /// than once (hairlines): x, y, call, coverage.
    Hits(Vec<(i32, i32, Call, u8)>),
}

/// An anti-aliased fill of `path` (device space) through an anti-aliased
/// clip: `SkScan::AntiFillPath` with an `SkAAClipBlitter` (forceRLE) over
/// the clip's bounds.
pub(crate) fn fill_path_aa_clip(
    path: &crate::Path,
    clip: &crate::aaclip::AAClip,
    width: u32,
    height: u32,
) -> Coverage {
    let mut rec = Recorder::new(width, height);
    if clip.is_empty() {
        return rec.into_mask();
    }
    let mut bounds = clip.bounds();
    if !bounds.intersect(&rec.clip()) {
        return rec.into_mask();
    }
    {
        let mut blitter = crate::aaclip::AAClipBlitter {
            inner: &mut rec,
            clip,
        };
        crate::scan::anti_fill_path_rle(path, bounds, &mut blitter, true);
    }
    rec.into_mask()
}

/// A rect through an anti-aliased clip (`SkScan::FillRect`/`AntiFillRect`
/// with an `SkAAClipBlitterWrapper`): the clip's bounds clip the scan, the
/// `SkAAClipBlitter` scales by the clip's coverage.
pub(crate) fn rect_aa_clip(r: Rect, anti_alias: bool, clip: &crate::aaclip::AAClip, width: u32, height: u32) -> Coverage {
    let mut rec = Recorder::new(width, height);
    if clip.is_empty() {
        return rec.into_mask();
    }
    let mut bounds = clip.bounds();
    if !bounds.intersect(&rec.clip()) {
        return rec.into_mask();
    }
    {
        let mut blitter = crate::aaclip::AAClipBlitter {
            inner: &mut rec,
            clip,
        };
        let whole = |v: f32| v == v.floor();
        if anti_alias && !(whole(r.left) && whole(r.top) && whole(r.right) && whole(r.bottom)) {
            crate::scan::anti_fill_rect(
                crate::geom::Rect::from_ltrb(r.left, r.top, r.right, r.bottom),
                bounds,
                &mut blitter,
            );
        } else {
            let round = |v: f32| (v + 0.5).floor() as i32;
            let mut ir = IRect::from_ltrb(round(r.left), round(r.top), round(r.right), round(r.bottom));
            if ir.intersect(&bounds) {
                blitter.blit_rect(ir.left, ir.top, ir.right - ir.left, ir.bottom - ir.top);
            }
        }
    }
    rec.into_mask()
}

/// A path (device space) through a normal blur mask filter of device
/// `sigma`: a fill, or its hairline with `cap`. None when Skia would draw
/// it unfiltered (`filterPath` returns false).
pub(crate) fn mask_blur_path(
    path: &crate::Path,
    sigma: f32,
    hairline: Option<crate::Cap>,
    width: u32,
    height: u32,
) -> Option<Coverage> {
    let mut rec = Recorder::new(width, height);
    let clip = crate::blitter::device_clip(rec.clip());
    if clip.is_empty() {
        return Some(rec.into_mask());
    }
    if !crate::maskblur::filter_path(path, sigma, hairline, clip, &mut rec) {
        return None;
    }
    Some(rec.into_mask())
}

/// An anti-aliased hairline of `path` (device space) with `cap`
/// (`SkScan::AntiHairPath` and its cap variants).
pub(crate) fn anti_hair_path(path: &crate::Path, cap: crate::Cap, width: u32, height: u32) -> Coverage {
    anti_hair_path_clipped(path, cap, None, width, height)
}

/// The same through an anti-aliased clip (`SkAAClipBlitterWrapper`).
pub(crate) fn anti_hair_path_clipped(
    path: &crate::Path,
    cap: crate::Cap,
    aa_clip: Option<&crate::aaclip::AAClip>,
    width: u32,
    height: u32,
) -> Coverage {
    let mut rec = HitRecorder {
        width: width as i32,
        height: height as i32,
        hits: Vec::new(),
    };
    let mut clip = IRect::from_ltrb(0, 0, width as i32, height as i32);
    match aa_clip {
        Some(c) => {
            // hair_path: the path's bounds, rounded out and outset for the cap;
            // an AA clip that contains them needs no SkAAClipBlitter.
            let out = if cap == crate::Cap::Butt { 1 } else { 2 };
            let b = path.bounds().round_out();
            let ib = IRect::from_ltrb(b.left - out, b.top - out, b.right + out, b.bottom + out);
            let mut test = ib;
            if c.is_empty() || !test.intersect(&c.bounds()) {
                return Coverage::Hits(rec.hits);
            }
            if c.quick_contains(ib.left, ib.top, ib.right, ib.bottom) {
                crate::hair::anti_hair_path(path, cap, clip, &mut rec);
            } else {
                if !clip.intersect(&c.bounds()) {
                    return Coverage::Hits(rec.hits);
                }
                let mut blitter = crate::aaclip::AAClipBlitter { inner: &mut rec, clip: c };
                crate::hair::anti_hair_path(path, cap, clip, &mut blitter);
            }
        }
        None => crate::hair::anti_hair_path(path, cap, clip, &mut rec),
    }
    Coverage::Hits(rec.hits)
}

/// A simple round rect (device space) through a normal blur mask filter:
/// `SkMaskFilterBase::filterRRect`. None when Skia draws its path instead.
pub(crate) fn mask_blur_rrect(r: Rect, rx: f32, ry: f32, sigma: f32, width: u32, height: u32) -> Option<Coverage> {
    let mut rec = Recorder::new(width, height);
    let clip = crate::blitter::device_clip(rec.clip());
    if clip.is_empty() {
        return Some(rec.into_mask());
    }
    let rect = crate::geom::Rect::from_ltrb(r.left, r.top, r.right, r.bottom);
    if !crate::maskblur::filter_rrect(rect, rx, ry, sigma, clip, &mut rec) {
        return None;
    }
    Some(rec.into_mask())
}

/// A blitter that records every blit in order, clipped to the destination.
struct HitRecorder {
    width: i32,
    height: i32,
    hits: Vec<(i32, i32, Call, u8)>,
}

impl HitRecorder {
    fn put(&mut self, x: i32, y: i32, call: Call, alpha: u8) {
        if x >= 0 && y >= 0 && x < self.width && y < self.height {
            self.hits.push((x, y, call, alpha));
        }
    }
}

impl Blitter for HitRecorder {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        for i in 0..width {
            self.put(x + i, y, Call::H, 255);
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
            for k in 0..count {
                self.put(px + k, y, Call::AntiH, aa[i]);
            }
            px += count;
            i += count as usize;
        }
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
        for i in 0..height {
            self.put(x, y + i, Call::V, alpha);
        }
    }

    fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u8, a1: u8) {
        self.put(x, y, Call::AntiH2, a0);
        self.put(x + 1, y, Call::AntiH2, a1);
    }

    fn blit_anti_v2(&mut self, x: i32, y: i32, a0: u8, a1: u8) {
        self.put(x, y, Call::AntiH2, a0);
        self.put(x, y + 1, Call::AntiH2, a1);
    }
}

/// `SkScan::FillRect` (non-AA): edges rounded to the nearest pixel.
pub(crate) fn fill_rect(r: Rect, width: u32, height: u32) -> Coverage {
    let round = |v: f32| (v + 0.5).floor() as i32;
    Coverage::Rect {
        left: round(r.left).max(0),
        top: round(r.top).max(0),
        right: round(r.right).min(width as i32),
        bottom: round(r.bottom).min(height as i32),
    }
}

/// An anti-aliased rect: `SkScan::AntiFillRect`.
pub(crate) fn anti_fill_rect(r: Rect, width: u32, height: u32) -> Coverage {
    let whole = |v: f32| v == v.floor();
    if whole(r.left) && whole(r.top) && whole(r.right) && whole(r.bottom) {
        return fill_rect(r, width, height);
    }
    let mut rec = Recorder::new(width, height);
    crate::scan::anti_fill_rect(
        crate::geom::Rect::from_ltrb(r.left, r.top, r.right, r.bottom),
        rec.clip(),
        &mut rec,
    );
    rec.into_mask()
}

/// A filled path (already in device space), anti-aliased or not.
pub(crate) fn fill_path(path: &crate::Path, anti_alias: bool, width: u32, height: u32) -> Coverage {
    if anti_alias {
        let mut rec = Recorder::new(width, height);
        let clip = rec.clip();
        crate::scan::anti_fill_path(path, clip, &mut rec);
        return rec.into_mask();
    }
    let Some(path) = path.to_tiny_skia() else {
        return Recorder::new(width, height).into_mask();
    };
    let Some(mut pm) = tiny_skia::Pixmap::new(width.max(1), height.max(1)) else {
        return Coverage::Mask(Vec::new(), Vec::new());
    };
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    paint.blend_mode = tiny_skia::BlendMode::Source;
    paint.anti_alias = false;
    pm.fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        tiny_skia::Transform::identity(),
        None,
    );
    // Aliased scan conversion only blits whole spans (`blitH`): each row's
    // runs of covered pixels.
    let cells: Vec<Option<(Call, u8)>> =
        pm.data().chunks_exact(4).map(|p| (p[3] != 0).then_some((Call::H, 255))).collect();
    let w = width.max(1) as usize;
    let mut spans = vec![0i32; cells.len()];
    for (row, chunk) in cells.chunks(w).enumerate() {
        let mut start = 0;
        for (x, c) in chunk.iter().enumerate() {
            if c.is_none() || x == 0 || chunk[x - 1].is_none() {
                start = x as i32;
            }
            spans[row * w + x] = start;
        }
    }
    Coverage::Mask(cells, spans)
}

/// A blitter that records each covered pixel's call and coverage, clipped to the
/// destination.
struct Recorder {
    width: i32,
    height: i32,
    cells: Vec<Option<(Call, u8)>>,
    spans: Vec<i32>,
}

impl Recorder {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width: width as i32,
            height: height as i32,
            cells: vec![None; width as usize * height as usize],
            spans: vec![0; width as usize * height as usize],
        }
    }

    fn into_mask(self) -> Coverage {
        Coverage::Mask(self.cells, self.spans)
    }

    fn clip(&self) -> IRect {
        IRect::from_ltrb(0, 0, self.width, self.height)
    }

    /// Records `(x, y)` as covered by `call` at `alpha`, in a span from `span`.
    fn put(&mut self, x: i32, y: i32, call: Call, alpha: u8, span: i32) {
        if x >= 0 && y >= 0 && x < self.width && y < self.height {
            let i = (y * self.width + x) as usize;
            self.cells[i] = Some((call, alpha));
            self.spans[i] = span;
        }
    }
}

impl Blitter for Recorder {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        for i in 0..width {
            self.put(x + i, y, Call::H, 255, x);
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
            if aa[i] != 0 {
                for k in 0..count {
                    self.put(px + k, y, Call::AntiH, aa[i], px);
                }
            }
            px += count;
            i += count as usize;
        }
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
        if alpha != 0 {
            for i in 0..height {
                self.put(x, y + i, Call::V, alpha, x);
            }
        }
    }

    fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u8, a1: u8) {
        // Kept even at 0: the translucent legacy blitter still changes the pixel.
        // Two runs of one pixel (SkBlitter::blitAntiH2).
        self.put(x, y, Call::AntiH2, a0, x);
        self.put(x + 1, y, Call::AntiH2, a1, x + 1);
    }

    fn blit_anti_v2(&mut self, x: i32, y: i32, a0: u8, a1: u8) {
        self.put(x, y, Call::AntiH2, a0, x);
        self.put(x, y + 1, Call::AntiH2, a1, x);
    }

    fn blit_mask(&mut self, mask: &[u8], bounds: IRect, row_bytes: i32, clip: IRect) {
        // SkARGB32_Shader_Blitter::blitMask: shadeSpan(clip.fLeft, y, width).
        for y in clip.top..clip.bottom {
            for x in clip.left..clip.right {
                let a = mask[((y - bounds.top) * row_bytes + (x - bounds.left)) as usize];
                if a != 0 {
                    self.put(x, y, Call::Mask, a, clip.left);
                }
            }
        }
    }
}

/// `SkScan::AntiFrameRect`.
pub(crate) fn anti_frame_rect(
    r: Rect,
    sx: f32,
    sy: f32,
    aa_clip: Option<&crate::aaclip::AAClip>,
    width: u32,
    height: u32,
) -> Coverage {
    let mut rec = Recorder::new(width, height);
    let gr = crate::geom::Rect::from_ltrb(r.left, r.top, r.right, r.bottom);
    match aa_clip {
        Some(c) => {
            if !c.is_empty() {
                let mut blitter = crate::aaclip::AAClipBlitter { inner: &mut rec, clip: c };
                crate::scan::anti_frame_rect(gr, sx, sy, &mut blitter);
            }
        }
        None => crate::scan::anti_frame_rect(gr, sx, sy, &mut rec),
    }
    rec.into_mask()
}

/// `SkScan::FrameRect`: the stroke's four sides as aliased rects (or one
/// rect when the stroke covers the middle).
pub(crate) fn frame_rect(r: Rect, dx: f32, dy: f32, width: u32, height: u32) -> Coverage {
    let mut rec = Recorder::new(width, height);
    let (rx, ry) = (dx * 0.5, dy * 0.5);
    let outer = Rect::ltrb(r.left - rx, r.top - ry, r.right + rx, r.bottom + ry);
    let mut fill = |r: Rect| {
        if let Coverage::Rect { left, top, right, bottom } = fill_rect(r, width, height) {
            for y in top..bottom {
                for x in left..right {
                    rec.put(x, y, Call::H, 255, left);
                }
            }
        }
    };
    if r.width() <= dx || r.height() <= dy {
        fill(outer);
    } else {
        fill(Rect::ltrb(outer.left, outer.top, outer.right, outer.top + dy));
        fill(Rect::ltrb(outer.left, outer.bottom - dy, outer.right, outer.bottom));
        fill(Rect::ltrb(outer.left, outer.top + dy, outer.left + dx, outer.bottom - dy));
        fill(Rect::ltrb(outer.right - dx, outer.top + dy, outer.right, outer.bottom - dy));
    }
    rec.into_mask()
}
