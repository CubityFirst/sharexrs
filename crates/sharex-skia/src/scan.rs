//! `SkScan::AntiFillPath`: picks analytic AA or supersampling for a path
//! (`ShouldUseAAA`), as Skia m119 does.

use crate::aaa;
use crate::blitter::Blitter;
use crate::geom::IRect;
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
        aaa::aaa_fill_path_entry(path, blitter, ir, clip);
    } else {
        crate::saa::saa_fill_path(path, blitter, ir, clip);
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
