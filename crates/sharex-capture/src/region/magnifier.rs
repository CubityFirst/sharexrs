//! The magnifier (`RegionCaptureWindow.axaml` `MagnifierView` and
//! `MagnifierPixelGrid.cs`), drawn on the CPU into one picture so its
//! pixel grid lands on physical pixels exactly as ShareX's does: a white
//! ring, the pixels around the pointer enlarged (nearest neighbour), the
//! accent-tinted centre row and column, grid lines, the black and white
//! centre outline, all clipped to a circle with a black ring.

use image::RgbaImage;

use crate::options::RegionCaptureOptions;

/// `GridBrush`: black at 75/255.
const GRID: [u8; 4] = [0, 0, 0, 75];

/// `NormalizeMagnifierPixelCount`: an odd count between 3 and 35 that
/// leaves each magnified pixel at least 3 physical pixels wide.
pub fn normalize_pixel_count(requested: i32, magnifier_size: i32, scale: f64) -> i32 {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let size = magnifier_size
        .clamp(RegionCaptureOptions::MAGNIFIER_SIZE_MINIMUM, RegionCaptureOptions::MAGNIFIER_SIZE_MAXIMUM);
    let limited = (size as f64 * scale / RegionCaptureOptions::MAGNIFIER_PIXEL_SIZE_MINIMUM as f64).floor() as i32;
    let mut maximum = limited.clamp(
        RegionCaptureOptions::MAGNIFIER_PIXEL_COUNT_MINIMUM,
        RegionCaptureOptions::MAGNIFIER_PIXEL_COUNT_MAXIMUM,
    );
    if maximum & 1 == 0 {
        maximum -= 1;
    }
    let mut count = requested.clamp(RegionCaptureOptions::MAGNIFIER_PIXEL_COUNT_MINIMUM, maximum);
    if count & 1 == 0 {
        count = if count < maximum { count + 1 } else { count - 1 };
    }
    count
}

/// `MagnifierPixelGrid.SnapPhysical`: rounded half away from zero.
fn snap(v: f64) -> i32 {
    v.round() as i32
}

fn over(dst: [f32; 4], src: [u8; 4], coverage: f32) -> [f32; 4] {
    let a = src[3] as f32 / 255.0 * coverage;
    if a <= 0.0 {
        return dst;
    }
    let out_a = a + dst[3] * (1.0 - a);
    let mut out = [0.0; 4];
    for c in 0..3 {
        // Premultiplied while compositing.
        out[c] = src[c] as f32 / 255.0 * a + dst[c] * (1.0 - a);
    }
    out[3] = out_a;
    out
}

/// What fraction of the pixel at (`px`, `py`) is inside the circle at
/// `c` of radius `r` (8x8 samples near the edge).
fn circle_coverage(px: i32, py: i32, c: f64, r: f64) -> f32 {
    let (x, y) = (px as f64 + 0.5 - c, py as f64 + 0.5 - c);
    let d = (x * x + y * y).sqrt();
    if d <= r - 0.75 {
        return 1.0;
    }
    if d >= r + 0.75 {
        return 0.0;
    }
    let mut inside = 0;
    for j in 0..8 {
        for i in 0..8 {
            let sx = px as f64 + (i as f64 + 0.5) / 8.0 - c;
            let sy = py as f64 + (j as f64 + 0.5) / 8.0 - c;
            if sx * sx + sy * sy <= r * r {
                inside += 1;
            }
        }
    }
    inside as f32 / 64.0
}

/// The magnifier's picture, `(size + 2) * scale` physical pixels square,
/// for the screenshot pixel under (`cx`, `cy`). `accent` is
/// `ShareX.Brush.Accent.Start`.
pub fn render(
    screenshot: &RgbaImage,
    (cx, cy): (i32, i32),
    size: i32,
    count: i32,
    square: bool,
    scale: f64,
    accent: [u8; 3],
) -> RgbaImage {
    let size = size as f64;
    let view = ((size + 2.0) * scale).round().max(1.0) as i32;
    let o = scale; // MagnifierContent sits 1 DIP inside the view.
    let content = size * scale;
    let n = count.max(1);
    let ci = n / 2;
    let (left, top) = (snap(o), snap(o));
    let (right, bottom) = (snap(o + content), snap(o + content));
    let edge = |i: i32| snap(o + i as f64 * content / n as f64);
    let (cl, ct, cr, cb) = (edge(ci), edge(ci), edge(ci + 1), edge(ci + 1));
    let (iw, ih) = (screenshot.width() as i32, screenshot.height() as i32);
    let radius = n / 2;
    let source = |px: i32, py: i32| -> [u8; 4] {
        // `Stretch="Fill"` with nearest-neighbour sampling, at pixel centres.
        let i = (((px as f64 + 0.5 - o) * n as f64 / content).floor() as i32).clamp(0, n - 1);
        let j = (((py as f64 + 0.5 - o) * n as f64 / content).floor() as i32).clamp(0, n - 1);
        let sx = (cx + i - radius).clamp(0, iw - 1);
        let sy = (cy + j - radius).clamp(0, ih - 1);
        let p = screenshot.get_pixel(sx as u32, sy as u32);
        [p[0], p[1], p[2], p[3]]
    };
    let inside = |x: i32, y: i32, l: i32, t: i32, r: i32, b: i32| x >= l && x < r && y >= t && y < b;
    let accent = [accent[0], accent[1], accent[2], 75];
    let content_pixel = |x: i32, y: i32| -> [f32; 4] {
        let s = source(x, y);
        let mut c = [s[0] as f32 / 255.0, s[1] as f32 / 255.0, s[2] as f32 / 255.0, s[3] as f32 / 255.0];
        // The centre row and column, at 75/255 opacity.
        if inside(x, y, left, ct, cl, cb - 1)
            || inside(x, y, cr, ct, right, cb - 1)
            || inside(x, y, cl, top, cr - 1, ct)
            || inside(x, y, cl, cb, cr - 1, bottom)
        {
            c = over(c, accent, 1.0);
        }
        // Grid lines: the physical pixel just before each cell boundary.
        for k in 1..n {
            let g = edge(k);
            if inside(x, y, g - 1, top, g, bottom) {
                c = over(c, GRID, 1.0);
            }
            if inside(x, y, left, g - 1, right, g) {
                c = over(c, GRID, 1.0);
            }
        }
        // The centre outline: black, then white inside it.
        let outline = |x: i32, y: i32, l: i32, t: i32, r: i32, b: i32| {
            r > l
                && b > t
                && (inside(x, y, l, t, r, t + 1)
                    || inside(x, y, l, b - 1, r, b)
                    || inside(x, y, l, t + 1, l + 1, b - 1)
                    || inside(x, y, r - 1, t + 1, r, b - 1))
        };
        if outline(x, y, cl - 1, ct - 1, cr, cb) {
            c = over(c, [0, 0, 0, 255], 1.0);
        }
        if outline(x, y, cl, ct, cr - 1, cb - 1) {
            c = over(c, [255, 255, 255, 255], 1.0);
        }
        c
    };
    let centre = view as f64 / 2.0;
    let mut out = RgbaImage::new(view as u32, view as u32);
    for y in 0..view {
        for x in 0..view {
            let mut c = [0.0f32; 4];
            if square {
                // White outer square, the content, a 1 DIP black border
                // (aliased) around the content.
                c = over(c, [255, 255, 255, 255], 1.0);
                if inside(x, y, left, top, right, bottom) {
                    let p = content_pixel(x, y);
                    c = [p[0], p[1], p[2], 1.0];
                }
                let b = scale.round().max(1.0) as i32;
                let in_content = inside(x, y, left, top, right, bottom);
                let in_inner = inside(x, y, left + b, top + b, right - b, bottom - b);
                if in_content && !in_inner {
                    c = over(c, [0, 0, 0, 255], 1.0);
                }
            } else {
                c = over(c, [255, 255, 255, 255], circle_coverage(x, y, centre, centre));
                let k = circle_coverage(x, y, centre, content / 2.0);
                if k > 0.0 {
                    let p = content_pixel(x, y);
                    let a = k * p[3];
                    for i in 0..3 {
                        c[i] = p[i] * a + c[i] * (1.0 - a);
                    }
                    c[3] = a + c[3] * (1.0 - a);
                }
                // The inner ring: 1 DIP wide, just inside the content's edge.
                let ring = circle_coverage(x, y, centre, content / 2.0)
                    - circle_coverage(x, y, centre, content / 2.0 - scale);
                c = over(c, [0, 0, 0, 255], ring.max(0.0));
            }
            let a = c[3];
            let un = |v: f32| if a > 0.0 { ((v / a) * 255.0).round().clamp(0.0, 255.0) as u8 } else { 0 };
            out.put_pixel(x as u32, y as u32, image::Rgba([un(c[0]), un(c[1]), un(c[2]), (a * 255.0).round() as u8]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_counts_stay_odd_and_in_range() {
        assert_eq!(normalize_pixel_count(15, 150, 1.0), 15);
        assert_eq!(normalize_pixel_count(16, 150, 1.0), 17);
        assert_eq!(normalize_pixel_count(1, 150, 1.0), 3);
        assert_eq!(normalize_pixel_count(99, 150, 1.0), 35);
        // A 50 px magnifier fits 16 pixels of 3: 15 at most.
        assert_eq!(normalize_pixel_count(35, 50, 1.0), 15);
    }

    #[test]
    fn magnifier_cells_are_ten_pixels_with_a_grid() {
        let mut shot = RgbaImage::from_pixel(64, 64, image::Rgba([200, 100, 50, 255]));
        shot.put_pixel(32, 32, image::Rgba([0, 255, 0, 255]));
        let m = render(&shot, (32, 32), 150, 15, false, 1.0, [9, 170, 255]);
        assert_eq!(m.dimensions(), (152, 152));
        // The centre pixel's cell (cells start at 1 + 10 i): white outline
        // at its edge, the pixel's colour inside.
        assert_eq!(m.get_pixel(76, 76), &image::Rgba([0, 255, 0, 255]));
        assert_eq!(m.get_pixel(71, 76), &image::Rgba([255, 255, 255, 255]));
        assert_eq!(m.get_pixel(70, 76), &image::Rgba([0, 0, 0, 255]));
        // A grid line one pixel before a cell boundary, in a plain cell.
        let line = m.get_pixel(30, 40);
        assert!(line[0] < 200 && line[3] == 255, "{line:?}");
        // Outside the ring: transparent corners.
        assert_eq!(m.get_pixel(0, 0)[3], 0);
    }
}
