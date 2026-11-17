//! Rectangles as ShareX uses them: `System.Drawing.Rectangle` (integer
//! screen pixels, [`Rect`]) and Avalonia's `Rect` (doubles, [`RectF`]) for
//! the region capture overlay, which works in screenshot pixels.

/// `System.Drawing.Rectangle`: integer pixels, `x..x+w` by `y..y+h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    /// `Rectangle.IsEmpty`: all zero.
    pub fn is_empty(&self) -> bool {
        *self == Rect::default()
    }

    /// ShareX's `Rectangle.IsValid()` extension: a positive size.
    pub fn is_valid(&self) -> bool {
        self.w > 0 && self.h > 0
    }

    /// `Rectangle.Contains(Point)`.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// `Rectangle.Contains(Rectangle)`.
    pub fn contains_rect(&self, r: &Rect) -> bool {
        self.x <= r.x && r.right() <= self.right() && self.y <= r.y && r.bottom() <= self.bottom()
    }

    /// `Rectangle.Intersect`: empty when they don't overlap.
    pub fn intersect(&self, other: &Rect) -> Rect {
        let (x0, y0) = (self.x.max(other.x), self.y.max(other.y));
        let (x1, y1) = (self.right().min(other.right()), self.bottom().min(other.bottom()));
        if x1 >= x0 && y1 >= y0 { Rect::new(x0, y0, x1 - x0, y1 - y0) } else { Rect::default() }
    }

    /// `Rectangle.Union`.
    pub fn union(&self, other: &Rect) -> Rect {
        let (x0, y0) = (self.x.min(other.x), self.y.min(other.y));
        let (x1, y1) = (self.right().max(other.right()), self.bottom().max(other.bottom()));
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    pub fn offset(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    pub fn to_f(self) -> RectF {
        RectF::new(self.x as f64, self.y as f64, self.w as f64, self.h as f64)
    }
}

/// ShareX's `IEnumerable<Rectangle>.Combine()`: the union of them all.
pub fn combine(rects: impl IntoIterator<Item = Rect>) -> Rect {
    rects.into_iter().reduce(|a, b| a.union(&b)).unwrap_or_default()
}

/// Avalonia's `Rect`: doubles, `x..x+w` by `y..y+h`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectF {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Avalonia's `Point`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointF {
    pub x: f64,
    pub y: f64,
}

impl PointF {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance(&self, other: PointF) -> f64 {
        ((other.x - self.x).powi(2) + (other.y - self.y).powi(2)).sqrt()
    }
}

impl RectF {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    pub fn center(&self) -> PointF {
        PointF::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// `RegionSelectionOverlay.IsValid`: a positive size.
    pub fn is_valid(&self) -> bool {
        self.w > 0.0 && self.h > 0.0
    }

    /// Avalonia's `Rect.Contains`: edges included.
    pub fn contains(&self, p: PointF) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    /// `RegionSelectionOverlay.Intersect`: empty when they don't overlap.
    pub fn intersect(&self, bounds: &RectF) -> RectF {
        let (l, t) = (self.x.max(bounds.x), self.y.max(bounds.y));
        let (r, b) = (self.right().min(bounds.right()), self.bottom().min(bounds.bottom()));
        if r > l && b > t { RectF::new(l, t, r - l, b - t) } else { RectF::default() }
    }

    pub fn translate(&self, dx: f64, dy: f64) -> RectF {
        RectF::new(self.x + dx, self.y + dy, self.w, self.h)
    }
}

/// `RegionSelectionOverlay.CreatePixelRectangle`: the rectangle from the
/// pixel under `first` to the pixel under `second`, both included, so the
/// first to the last pixel of a 1920x1080 surface is 1920x1080.
pub fn create_pixel_rectangle(first: PointF, second: PointF, bounds: (f64, f64)) -> RectF {
    // Positions are whole pixels with noise from the DPI transform: rounded.
    let pixel = |v: f64, size: f64| v.round().clamp(0.0, (size - 1.0).max(0.0));
    let (fx, fy) = (pixel(first.x, bounds.0), pixel(first.y, bounds.1));
    let (sx, sy) = (pixel(second.x, bounds.0), pixel(second.y, bounds.1));
    RectF::new(fx.min(sx), fy.min(sy), (sx - fx).abs() + 1.0, (sy - fy).abs() + 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_rectangles_include_both_ends() {
        let r = create_pixel_rectangle(PointF::new(200.0, 150.0), PointF::new(600.0, 450.0), (1024.0, 768.0));
        assert_eq!(r, RectF::new(200.0, 150.0, 401.0, 301.0));
        let r = create_pixel_rectangle(PointF::new(0.0, 0.0), PointF::new(1024.0, 768.0), (1024.0, 768.0));
        assert_eq!(r, RectF::new(0.0, 0.0, 1024.0, 768.0));
        let r = create_pixel_rectangle(PointF::new(10.0, 10.0), PointF::new(4.0, 2.0), (100.0, 100.0));
        assert_eq!(r, RectF::new(4.0, 2.0, 7.0, 9.0));
    }

    #[test]
    fn rectangles_intersect_and_combine() {
        let a = Rect::new(0, 0, 10, 10);
        assert_eq!(a.intersect(&Rect::new(5, 5, 10, 10)), Rect::new(5, 5, 5, 5));
        assert!(a.intersect(&Rect::new(20, 0, 5, 5)).is_empty());
        assert_eq!(combine([a, Rect::new(-5, 2, 3, 3)]), Rect::new(-5, 0, 15, 10));
    }
}
