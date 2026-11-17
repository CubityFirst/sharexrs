//! `ShareX.OCR/Geometry.cs`: convex hull, minimum-area rectangle, PaddleOCR's corner
//! order and unclip, in `float` with C#'s operation order, so boxes come out bit for bit.

/// `SKPoint`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// `SKPoint.Length`: `(float)Math.Sqrt(x * x + y * y)` (the sum in float; a
    /// correctly rounded double square root rounds to the float one).
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// `SKPoint.Distance(a, b)`.
    pub fn distance(a: Point, b: Point) -> f32 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        (dx * dx + dy * dy).sqrt()
    }
}

impl std::ops::Sub for Point {
    type Output = Point;
    fn sub(self, o: Point) -> Point {
        Point::new(self.x - o.x, self.y - o.y)
    }
}

/// `SKRect` (left, top, right, bottom).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// `SKRect.MidY`: `top + Height / 2`.
    pub fn mid_y(&self) -> f32 {
        self.top + self.height() / 2.0
    }

    /// `SKRect.Union(rect)`: the plain minimum and maximum of the sides (SkiaSharp's,
    /// not Skia's, so empty rectangles count too).
    pub fn union(&mut self, r: &Rect) {
        self.left = cs_min(self.left, r.left);
        self.top = cs_min(self.top, r.top);
        self.right = cs_max(self.right, r.right);
        self.bottom = cs_max(self.bottom, r.bottom);
    }
}

/// .NET's `Math.Min(float, float)`: NaN wins, and -0 is smaller than +0.
pub fn cs_min(x: f32, y: f32) -> f32 {
    if x != y {
        if !x.is_nan() {
            return if x < y { x } else { y };
        }
        return x;
    }
    if x.is_sign_negative() { x } else { y }
}

/// .NET's `Math.Max(float, float)`: NaN wins, and +0 is larger than -0.
pub fn cs_max(x: f32, y: f32) -> f32 {
    if x != y {
        if !x.is_nan() {
            return if x < y { y } else { x };
        }
        return x;
    }
    if y.is_sign_negative() { x } else { y }
}

/// .NET's `Math.Clamp(float, min, max)`.
pub fn cs_clamp(v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

fn cross(o: Point, a: Point, b: Point) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// `ConvexHull`: Andrew's monotone chain, counter-clockwise (in y-up terms), the
/// first point not repeated. Sorts `points` by x, then y, as ShareX does.
pub fn convex_hull(points: &mut [Point]) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }
    points.sort_by(|a, b| {
        if a.x != b.x {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    let mut hull = vec![Point::default(); points.len() * 2];
    let mut count = 0usize;
    for &p in points.iter() {
        while count >= 2 && cross(hull[count - 2], hull[count - 1], p) <= 0.0 {
            count -= 1;
        }
        hull[count] = p;
        count += 1;
    }
    let lower_count = count + 1;
    for i in (0..points.len() - 1).rev() {
        let p = points[i];
        while count >= lower_count && cross(hull[count - 2], hull[count - 1], p) <= 0.0 {
            count -= 1;
        }
        hull[count] = p;
        count += 1;
    }
    hull.truncate(count - 1);
    hull
}

/// `MinAreaRect`: the smallest rotated rectangle around a convex hull, trying each
/// hull edge as a side. Returns its corners (in [`order_box`] order) and its shorter side.
pub fn min_area_rect(hull: &[Point]) -> ([Point; 4], f32) {
    if hull.is_empty() {
        return ([Point::default(); 4], 0.0);
    }
    if hull.len() == 1 {
        return ([hull[0]; 4], 0.0);
    }
    let mut best_area = f32::MAX;
    let mut best_axis = Point::new(1.0, 0.0);
    let (mut best_min_u, mut best_max_u, mut best_min_v, mut best_max_v) =
        (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for i in 0..hull.len() {
        let edge = hull[(i + 1) % hull.len()] - hull[i];
        let length = edge.length();
        if length < 1e-6 {
            continue;
        }
        let axis = Point::new(edge.x / length, edge.y / length);
        let (mut min_u, mut max_u, mut min_v, mut max_v) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for p in hull {
            let u = p.x * axis.x + p.y * axis.y;
            let v = p.y * axis.x - p.x * axis.y;
            min_u = cs_min(min_u, u);
            max_u = cs_max(max_u, u);
            min_v = cs_min(min_v, v);
            max_v = cs_max(max_v, v);
        }
        let area = (max_u - min_u) * (max_v - min_v);
        if area < best_area {
            best_area = area;
            best_axis = axis;
            best_min_u = min_u;
            best_max_u = max_u;
            best_min_v = min_v;
            best_max_v = max_v;
        }
    }
    let corner = |u: f32, v: f32| {
        Point::new(
            u * best_axis.x - v * best_axis.y,
            u * best_axis.y + v * best_axis.x,
        )
    };
    let b = [
        corner(best_min_u, best_min_v),
        corner(best_max_u, best_min_v),
        corner(best_max_u, best_max_v),
        corner(best_min_u, best_max_v),
    ];
    (
        order_box(&b),
        cs_min(best_max_u - best_min_u, best_max_v - best_min_v),
    )
}

/// `OrderBox`: top-left, top-right, bottom-right, bottom-left, as PaddleOCR orders
/// them: the two leftmost points are the left side, the higher of each pair the top.
/// (`OrderBy` is stable, like `sort_by`.)
pub fn order_box(b: &[Point; 4]) -> [Point; 4] {
    let mut s = *b;
    s.sort_by(|p, q| p.x.partial_cmp(&q.x).unwrap_or(std::cmp::Ordering::Equal));
    let (tl, bl) = if s[0].y <= s[1].y {
        (s[0], s[1])
    } else {
        (s[1], s[0])
    };
    let (tr, br) = if s[2].y <= s[3].y {
        (s[2], s[3])
    } else {
        (s[3], s[2])
    };
    [tl, tr, br, bl]
}

/// `Unclip`: grows a rectangle by PaddleOCR's unclip distance (area * ratio /
/// perimeter) on every side. Returns the corners and the shorter side.
pub fn unclip(b: &[Point; 4], unclip_ratio: f32) -> ([Point; 4], f32) {
    let center = Point::new(
        (b[0].x + b[1].x + b[2].x + b[3].x) / 4.0,
        (b[0].y + b[1].y + b[2].y + b[3].y) / 4.0,
    );
    let wv = b[1] - b[0];
    let hv = b[3] - b[0];
    let width = wv.length();
    let height = hv.length();
    let perimeter = 2.0 * (width + height);
    if perimeter < 1e-6 || width < 1e-6 || height < 1e-6 {
        return (*b, 0.0);
    }
    let distance = width * height * unclip_ratio / perimeter;
    let wa = Point::new(wv.x / width, wv.y / width);
    let ha = Point::new(hv.x / height, hv.y / height);
    let half_w = width / 2.0 + distance;
    let half_h = height / 2.0 + distance;
    let corner = |u: f32, v: f32| {
        Point::new(
            center.x + wa.x * u + ha.x * v,
            center.y + wa.y * u + ha.y * v,
        )
    };
    let r = [
        corner(-half_w, -half_h),
        corner(half_w, -half_h),
        corner(half_w, half_h),
        corner(-half_w, half_h),
    ];
    (order_box(&r), cs_min(half_w, half_h) * 2.0)
}

/// `GetBounds`.
pub fn bounds(points: &[Point]) -> Rect {
    let (mut l, mut t, mut r, mut b) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in points {
        l = cs_min(l, p.x);
        t = cs_min(t, p.y);
        r = cs_max(r, p.x);
        b = cs_max(b, p.y);
    }
    Rect {
        left: l,
        top: t,
        right: r,
        bottom: b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_and_rectangle_of_an_axis_aligned_block() {
        let mut pts = vec![];
        for y in 0..5 {
            for x in 0..10 {
                pts.push(Point::new(x as f32, y as f32));
            }
        }
        let hull = convex_hull(&mut pts);
        assert_eq!(hull.len(), 4);
        let (b, short) = min_area_rect(&hull);
        assert_eq!(short, 4.0);
        assert_eq!(b[0], Point::new(0.0, 0.0));
        assert_eq!(b[2], Point::new(9.0, 4.0));
        let (u, s) = unclip(&b, 1.5);
        // distance = 9 * 4 * 1.5 / 26
        let d = 9.0f32 * 4.0 * 1.5 / 26.0;
        assert_eq!(s, (4.0 / 2.0 + d) * 2.0);
        assert!((u[0].x - (-d)).abs() < 1e-5);
    }

    #[test]
    fn dotnet_min_max_keep_signed_zero() {
        assert!(cs_min(0.0, -0.0).is_sign_negative());
        assert!(cs_max(-0.0, 0.0).is_sign_positive());
        assert!(cs_min(f32::NAN, 1.0).is_nan());
    }
}
