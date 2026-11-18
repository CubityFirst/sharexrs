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
    /// and its coverage.
    Mask(Vec<Option<(Call, u8)>>),
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
        return Coverage::Mask(rec.cells);
    }
    let mut bounds = clip.bounds();
    if !bounds.intersect(&rec.clip()) {
        return Coverage::Mask(rec.cells);
    }
    {
        let mut blitter = crate::aaclip::AAClipBlitter {
            inner: &mut rec,
            clip,
        };
        crate::scan::anti_fill_path_rle(path, bounds, &mut blitter, true);
    }
    Coverage::Mask(rec.cells)
}

/// A rect through an anti-aliased clip (`SkScan::FillRect`/`AntiFillRect`
/// with an `SkAAClipBlitterWrapper`): the clip's bounds clip the scan, the
/// `SkAAClipBlitter` scales by the clip's coverage.
pub(crate) fn rect_aa_clip(r: Rect, anti_alias: bool, clip: &crate::aaclip::AAClip, width: u32, height: u32) -> Coverage {
    let mut rec = Recorder::new(width, height);
    if clip.is_empty() {
        return Coverage::Mask(rec.cells);
    }
    let mut bounds = clip.bounds();
    if !bounds.intersect(&rec.clip()) {
        return Coverage::Mask(rec.cells);
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
    Coverage::Mask(rec.cells)
}

/// An anti-aliased hairline of `path` (device space) with `cap`
/// (`SkScan::AntiHairPath` and its cap variants).
pub(crate) fn anti_hair_path(path: &crate::Path, cap: crate::Cap, width: u32, height: u32) -> Coverage {
    let mut rec = HitRecorder {
        width: width as i32,
        height: height as i32,
        hits: Vec::new(),
    };
    let clip = IRect::from_ltrb(0, 0, width as i32, height as i32);
    crate::hair::anti_hair_path(path, cap, clip, &mut rec);
    Coverage::Hits(rec.hits)
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
    Coverage::Mask(rec.cells)
}

/// A filled path (already in device space), anti-aliased or not.
pub(crate) fn fill_path(path: &crate::Path, anti_alias: bool, width: u32, height: u32) -> Coverage {
    if anti_alias {
        let mut rec = Recorder::new(width, height);
        let clip = rec.clip();
        crate::scan::anti_fill_path(path, clip, &mut rec);
        return Coverage::Mask(rec.cells);
    }
    let Some(path) = path.to_tiny_skia() else {
        return Coverage::Mask(vec![None; width as usize * height as usize]);
    };
    let Some(mut pm) = tiny_skia::Pixmap::new(width.max(1), height.max(1)) else {
        return Coverage::Mask(Vec::new());
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
    // Aliased scan conversion only blits whole spans (`blitH`).
    Coverage::Mask(
        pm.data()
            .chunks_exact(4)
            .map(|p| (p[3] != 0).then_some((Call::H, 255)))
            .collect(),
    )
}

/// A blitter that records each covered pixel's call and coverage, clipped to the
/// destination.
struct Recorder {
    width: i32,
    height: i32,
    cells: Vec<Option<(Call, u8)>>,
}

impl Recorder {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width: width as i32,
            height: height as i32,
            cells: vec![None; width as usize * height as usize],
        }
    }

    fn clip(&self) -> IRect {
        IRect::from_ltrb(0, 0, self.width, self.height)
    }

    fn put(&mut self, x: i32, y: i32, call: Call, alpha: u8) {
        if x >= 0 && y >= 0 && x < self.width && y < self.height {
            self.cells[(y * self.width + x) as usize] = Some((call, alpha));
        }
    }
}

impl Blitter for Recorder {
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
            if aa[i] != 0 {
                for k in 0..count {
                    self.put(px + k, y, Call::AntiH, aa[i]);
                }
            }
            px += count;
            i += count as usize;
        }
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
        if alpha != 0 {
            for i in 0..height {
                self.put(x, y + i, Call::V, alpha);
            }
        }
    }

    fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u8, a1: u8) {
        // Kept even at 0: the translucent legacy blitter still changes the pixel.
        self.put(x, y, Call::AntiH2, a0);
        self.put(x + 1, y, Call::AntiH2, a1);
    }

    fn blit_anti_v2(&mut self, x: i32, y: i32, a0: u8, a1: u8) {
        self.put(x, y, Call::AntiH2, a0);
        self.put(x, y + 1, Call::AntiH2, a1);
    }

    fn blit_mask(&mut self, mask: &[u8], bounds: IRect, row_bytes: i32, clip: IRect) {
        for y in clip.top..clip.bottom {
            for x in clip.left..clip.right {
                let a = mask[((y - bounds.top) * row_bytes + (x - bounds.left)) as usize];
                if a != 0 {
                    self.put(x, y, Call::Mask, a);
                }
            }
        }
    }
}

/// `SkScan::AntiFrameRect`.
pub(crate) fn anti_frame_rect(r: Rect, sx: f32, sy: f32, width: u32, height: u32) -> Coverage {
    let mut rec = Recorder::new(width, height);
    crate::scan::anti_frame_rect(
        crate::geom::Rect::from_ltrb(r.left, r.top, r.right, r.bottom),
        sx,
        sy,
        &mut rec,
    );
    Coverage::Mask(rec.cells)
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
                    rec.put(x, y, Call::H, 255);
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
    Coverage::Mask(rec.cells)
}
