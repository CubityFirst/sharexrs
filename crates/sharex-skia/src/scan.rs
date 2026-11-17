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
    if should_use_aaa(path) {
        aaa::aaa_fill_path_entry(path, blitter, ir, clip);
    } else {
        supersample_fill_path(path, ir, clip, blitter);
    }
}

/// `SkScan::SAAFillPath`, through tiny-skia's port of Skia's supersampling
/// scan converter: its coverage, delivered as Skia's `MaskSuperBlitter`
/// (small paths: one `blitMask`) or `SuperBlitter` (`blitAntiH` per row)
/// would.
fn supersample_fill_path(path: &Path, ir: IRect, clip: IRect, blitter: &mut dyn Blitter) {
    let Some(ts_path) = path.to_tiny_skia() else {
        return;
    };
    let mut sect = ir;
    if !sect.intersect(&clip) {
        return;
    }
    let (w, h) = (sect.width() as u32, sect.height() as u32);
    let Some(mut mask) = tiny_skia::Mask::new(w, h) else {
        return;
    };
    let rule = match path.fill_type {
        crate::path::FillType::Winding => tiny_skia::FillRule::Winding,
        crate::path::FillType::EvenOdd => tiny_skia::FillRule::EvenOdd,
    };
    mask.fill_path(
        &ts_path,
        rule,
        true,
        tiny_skia::Transform::from_translate(-sect.left as f32, -sect.top as f32),
    );
    let data = mask.data();
    // MaskSuperBlitter::CanHandleRect
    let width = ir.width();
    let rb = (width + 3) & !3; // SkAlign4
    let small = width <= 32 && (rb as i64 * ir.height() as i64) <= 32 * 32;
    if small {
        blitter.blit_mask(data, sect, w as i32, sect);
    } else {
        for y in 0..h as i32 {
            let row = &data[(y as u32 * w) as usize..((y as u32 + 1) * w) as usize];
            if row.iter().all(|a| *a == 0) {
                continue;
            }
            let runs = vec![1i16; row.len() + 1];
            let mut runs = runs;
            runs[row.len()] = 0;
            blitter.blit_anti_h(sect.left, sect.top + y, row, &runs);
        }
    }
}
