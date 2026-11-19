//! `Presentation/RegionCapture/CaptureRegion.cs`: a selected region in
//! screenshot pixels. [`CaptureRegion::bounds`] is the rectangle that is
//! captured; an ellipse is inscribed in it and a freehand region is the
//! polygon through its points. `RegionCaptureTasks.ApplyRegionMask` keeps
//! only the pixels inside the regions.

use image::RgbaImage;
use sharex_skia::{Direction, FillType, Path, Rect as SkRect};

use crate::geometry::{PointF, Rect, RectF};

/// `RegionShape`: what a region capture tool selects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RegionShape {
    #[default]
    Rectangle,
    Ellipse,
    Freehand,
}

/// `CaptureRegion`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CaptureRegion {
    pub shape: RegionShape,
    pub bounds: RectF,
    /// A freehand region's points.
    pub points: Option<Vec<PointF>>,
}

impl CaptureRegion {
    pub fn new(shape: RegionShape, bounds: RectF) -> Self {
        Self { shape, bounds, points: None }
    }

    /// `FromRectangle`.
    pub fn rectangle(bounds: RectF) -> Self {
        Self::new(RegionShape::Rectangle, bounds)
    }

    pub fn is_rectangle(&self) -> bool {
        self.shape == RegionShape::Rectangle
    }

    /// The region with other bounds (`region with { Bounds = ... }`).
    pub fn with_bounds(&self, bounds: RectF) -> Self {
        Self { bounds, ..self.clone() }
    }

    /// `Contains`: inside the bounds, and the ellipse or polygon (even-odd).
    pub fn contains(&self, p: PointF) -> bool {
        if !self.bounds.contains(p) {
            return false;
        }
        match self.shape {
            RegionShape::Ellipse => {
                let (rx, ry) = (self.bounds.w / 2.0, self.bounds.h / 2.0);
                let c = self.bounds.center();
                let (dx, dy) = ((p.x - c.x) / rx, (p.y - c.y) / ry);
                dx * dx + dy * dy <= 1.0
            }
            RegionShape::Freehand => {
                self.points.as_ref().is_some_and(|pts| pts.len() >= 3 && polygon_contains(pts, p))
            }
            RegionShape::Rectangle => true,
        }
    }

    /// `Offset`: moved, a freehand region's points with its bounds.
    pub fn offset(&self, dx: f64, dy: f64) -> Self {
        if dx == 0.0 && dy == 0.0 {
            return self.clone();
        }
        Self {
            shape: self.shape,
            bounds: self.bounds.translate(dx, dy),
            points: self.points.as_ref().map(|p| p.iter().map(|q| PointF::new(q.x + dx, q.y + dy)).collect()),
        }
    }

    /// `CreateGeometry`: the outline (Avalonia's `EllipseGeometry` is
    /// `SKPath.AddOval`; a freehand region is an even-odd closed polygon),
    /// or `None` for a freehand region with fewer than two points.
    pub fn geometry(&self) -> Option<Path> {
        let b = self.bounds;
        let mut path = Path::new();
        match self.shape {
            RegionShape::Ellipse => {
                path.add_oval(sk_rect(b.x, b.y, b.w, b.h), Direction::Cw);
            }
            RegionShape::Freehand => {
                let pts = self.points.as_ref().filter(|p| p.len() >= 2)?;
                path.fill_type = FillType::EvenOdd;
                path.move_to(pts[0].x as f32, pts[0].y as f32);
                for q in &pts[1..] {
                    path.line_to(q.x as f32, q.y as f32);
                }
                path.close();
            }
            RegionShape::Rectangle => {
                path.add_rect(sk_rect(b.x, b.y, b.w, b.h), Direction::Cw, 0);
            }
        }
        Some(path)
    }

    /// `CreatePath(offsetX, offsetY)`: the region moved by the offset, for
    /// clipping a captured image.
    pub fn create_path(&self, ox: f64, oy: f64) -> Path {
        let mut path = Path::new();
        path.fill_type = FillType::EvenOdd;
        let r = SkRect::from_xywh(
            (self.bounds.x + ox) as f32,
            (self.bounds.y + oy) as f32,
            self.bounds.w as f32,
            self.bounds.h as f32,
        );
        match (self.shape, self.points.as_ref()) {
            (RegionShape::Ellipse, _) => {
                path.add_oval(r, Direction::Cw);
            }
            (RegionShape::Freehand, Some(pts)) if pts.len() >= 3 => {
                path.move_to((pts[0].x + ox) as f32, (pts[0].y + oy) as f32);
                for q in &pts[1..] {
                    path.line_to((q.x + ox) as f32, (q.y + oy) as f32);
                }
                path.close();
            }
            _ => {
                path.add_rect(r, Direction::Cw, 0);
            }
        }
        path
    }

    /// `GetBounds`: the rectangle bounding the points.
    pub fn points_bounds(points: &[PointF]) -> RectF {
        if points.is_empty() {
            return RectF::default();
        }
        let (mut l, mut t, mut r, mut b) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in points {
            l = l.min(p.x);
            t = t.min(p.y);
            r = r.max(p.x);
            b = b.max(p.y);
        }
        RectF::new(l, t, r - l, b - t)
    }
}

fn sk_rect(x: f64, y: f64, w: f64, h: f64) -> SkRect {
    // Avalonia's `Rect` to `SKRect` (`ToSKRect`: floats of the edges).
    SkRect::from_ltrb(x as f32, y as f32, (x + w) as f32, (y + h) as f32)
}

/// `PolygonContains`: the even-odd rule, as the outline and the mask fill.
fn polygon_contains(points: &[PointF], p: PointF) -> bool {
    let mut inside = false;
    let n = points.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (points[i], points[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// `RegionCaptureTasks.ApplyRegionMask(source, rectangles)`: only the
/// pixels inside `regions` (image pixels) are kept, the rest transparent
/// (a non-anti-aliased clip of whole rectangles).
pub fn apply_rect_mask(image: &RgbaImage, regions: &[Rect]) -> RgbaImage {
    let mut out = RgbaImage::new(image.width(), image.height());
    for (x, y, p) in image.enumerate_pixels() {
        if regions.iter().any(|r| r.contains(x as i32, y as i32)) {
            out.put_pixel(x, y, *p);
        }
    }
    out
}

/// `RegionCaptureTasks.ApplyRegionMask(source, paths)` on a premultiplied
/// RGBA source: for each path, `ClipPath(path, Intersect, antialias)` and
/// `DrawBitmap` with `SKBlendMode.Src` onto a transparent bitmap, so where
/// regions overlap their edges aren't drawn twice. Returns premultiplied
/// RGBA.
pub fn apply_path_mask(width: u32, height: u32, premul: &[u8], paths: &[Path]) -> Vec<u8> {
    use sharex_skia::raster::bitmap::Bitmap;
    use sharex_skia::raster::canvas::{Blend, Canvas, Paint, Shader};
    let source = Bitmap::from_premultiplied(width, height, premul.to_vec());
    let mut result = Bitmap::new(width, height);
    for path in paths {
        let mut canvas = Canvas::new(&mut result);
        canvas.clip_path_aa(path);
        let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
        paint.blend = Blend::Src;
        canvas.draw_bitmap(&source, 0.0, 0.0, Some(&paint));
    }
    result.into_data()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipses_and_polygons_contain_as_sharexs() {
        let e = CaptureRegion::new(RegionShape::Ellipse, RectF::new(0.0, 0.0, 10.0, 20.0));
        assert!(e.contains(PointF::new(5.0, 10.0)));
        assert!(!e.contains(PointF::new(0.5, 0.5)));
        let tri = vec![PointF::new(0.0, 0.0), PointF::new(10.0, 0.0), PointF::new(0.0, 10.0)];
        let f = CaptureRegion {
            shape: RegionShape::Freehand,
            bounds: CaptureRegion::points_bounds(&tri),
            points: Some(tri),
        };
        assert!(f.contains(PointF::new(2.0, 2.0)));
        assert!(!f.contains(PointF::new(8.0, 8.0)));
        let moved = f.offset(5.0, 1.0);
        assert_eq!(moved.bounds, RectF::new(5.0, 1.0, 10.0, 10.0));
        assert_eq!(moved.points.unwrap()[1], PointF::new(15.0, 1.0));
    }

    #[test]
    fn the_path_mask_keeps_the_inside_and_clears_the_outside() {
        let (w, h) = (20u32, 20u32);
        let src = vec![255u8; (w * h * 4) as usize];
        let e = CaptureRegion::new(RegionShape::Ellipse, RectF::new(0.0, 0.0, 20.0, 20.0));
        let out = apply_path_mask(w, h, &src, &[e.create_path(0.0, 0.0)]);
        let a = |x: u32, y: u32| out[((y * w + x) * 4 + 3) as usize];
        assert_eq!(a(10, 10), 255);
        assert_eq!(a(0, 0), 0);
        assert!(a(10, 0) > 0 && a(10, 0) < 255 || a(10, 0) == 255);
    }
}
