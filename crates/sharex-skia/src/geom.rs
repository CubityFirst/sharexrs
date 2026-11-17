//! Points, rectangles and the curve maths of `src/core/SkGeometry.cpp`
//! that the scan converter uses: chopping quads and cubics at their Y
//! extrema, and approximating conics with quads.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    pub fn is_zero(self) -> bool {
        self.x == 0.0 && self.y == 0.0
    }

    pub fn dot(self, o: Point) -> f32 {
        self.x * o.x + self.y * o.y
    }

    pub fn cross(self, o: Point) -> f32 {
        self.x * o.y - self.y * o.x
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

impl std::ops::Add for Point {
    type Output = Point;
    fn add(self, o: Point) -> Point {
        Point::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Sub for Point {
    type Output = Point;
    fn sub(self, o: Point) -> Point {
        Point::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Mul<f32> for Point {
    type Output = Point;
    fn mul(self, s: f32) -> Point {
        Point::new(self.x * s, self.y * s)
    }
}

impl std::ops::Neg for Point {
    type Output = Point;
    fn neg(self) -> Point {
        Point::new(-self.x, -self.y)
    }
}

/// `SkRect`: left, top, right, bottom.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub const fn from_ltrb(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn from_xywh(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::from_ltrb(x, y, x + w, y + h)
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn is_empty(&self) -> bool {
        // We write it as the NOT of a non-empty rect, so we will return true if any values
        // are NaN.
        !(self.left < self.right && self.top < self.bottom)
    }

    pub fn is_finite(&self) -> bool {
        self.left.is_finite()
            && self.top.is_finite()
            && self.right.is_finite()
            && self.bottom.is_finite()
    }

    /// `SkRect::roundOut`.
    pub fn round_out(&self) -> IRect {
        IRect {
            left: sat_floor(self.left),
            top: sat_floor(self.top),
            right: sat_ceil(self.right),
            bottom: sat_ceil(self.bottom),
        }
    }

    /// `SkRect::round`.
    pub fn round(&self) -> IRect {
        IRect {
            left: sat_round(self.left),
            top: sat_round(self.top),
            right: sat_round(self.right),
            bottom: sat_round(self.bottom),
        }
    }

    /// `SkRect::intersect`: false (and unchanged) when they don't overlap.
    pub fn intersect(&mut self, r: &Rect) -> bool {
        let l = self.left.max(r.left);
        let t = self.top.max(r.top);
        let rr = self.right.min(r.right);
        let b = self.bottom.min(r.bottom);
        if !(l < rr && t < b) {
            return false;
        }
        *self = Rect::from_ltrb(l, t, rr, b);
        true
    }

    pub fn contains(&self, r: &Rect) -> bool {
        !r.is_empty()
            && !self.is_empty()
            && self.left <= r.left
            && self.top <= r.top
            && self.right >= r.right
            && self.bottom >= r.bottom
    }

    /// `SkRect::setBoundsCheck`: the bounds of `pts`; false when one isn't
    /// finite (and then the rect is empty).
    pub fn bounds_of(pts: &[Point]) -> (Rect, bool) {
        if pts.is_empty() {
            return (Rect::default(), true);
        }
        let (mut l, mut t, mut r, mut b) = (pts[0].x, pts[0].y, pts[0].x, pts[0].y);
        let mut finite = pts[0].is_finite();
        for p in &pts[1..] {
            l = l.min(p.x);
            t = t.min(p.y);
            r = r.max(p.x);
            b = b.max(p.y);
            finite &= p.is_finite();
        }
        if finite {
            (Rect::from_ltrb(l, t, r, b), true)
        } else {
            (Rect::default(), false)
        }
    }
}

/// `SkIRect`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl IRect {
    pub const fn from_ltrb(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn from_xywh(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self::from_ltrb(x, y, x + w, y + h)
    }

    pub fn width(&self) -> i32 {
        self.right.wrapping_sub(self.left)
    }

    pub fn height(&self) -> i32 {
        self.bottom.wrapping_sub(self.top)
    }

    pub fn width64(&self) -> i64 {
        self.right as i64 - self.left as i64
    }

    pub fn height64(&self) -> i64 {
        self.bottom as i64 - self.top as i64
    }

    pub fn is_empty(&self) -> bool {
        let w = self.width64();
        let h = self.height64();
        if w <= 0 || h <= 0 {
            return true;
        }
        // Return true if either exceeds int32_t
        !(i32::try_from(w).is_ok() && i32::try_from(h).is_ok())
    }

    pub fn intersect(&mut self, r: &IRect) -> bool {
        let l = self.left.max(r.left);
        let t = self.top.max(r.top);
        let rr = self.right.min(r.right);
        let b = self.bottom.min(r.bottom);
        let tmp = IRect::from_ltrb(l, t, rr, b);
        if tmp.is_empty() {
            return false;
        }
        *self = tmp;
        true
    }

    pub fn contains(&self, r: &IRect) -> bool {
        !r.is_empty()
            && !self.is_empty()
            && self.left <= r.left
            && self.top <= r.top
            && self.right >= r.right
            && self.bottom >= r.bottom
    }

    pub fn to_rect(&self) -> Rect {
        Rect::from_ltrb(
            self.left as f32,
            self.top as f32,
            self.right as f32,
            self.bottom as f32,
        )
    }
}

fn sat(x: f32) -> i32 {
    // sk_float_saturate2int
    const MAX: f32 = 2147483520.0; // SK_MaxS32FitsInFloat
    const MIN: f32 = -2147483520.0;
    let x = if x.is_nan() { 0.0 } else { x.clamp(MIN, MAX) };
    x as i32
}

pub fn sat_floor(x: f32) -> i32 {
    sat(x.floor())
}

pub fn sat_ceil(x: f32) -> i32 {
    sat(x.ceil())
}

/// `sk_float_round2int`: `sk_float_saturate2int(sk_float_floor(x + 0.5f))`.
pub fn sat_round(x: f32) -> i32 {
    sat((x + 0.5).floor())
}

// ---------------------------------------------------------------------------
// SkGeometry.cpp

fn is_not_monotonic(a: f32, b: f32, c: f32) -> bool {
    let ab = a - b;
    let mut bc = b - c;
    if ab < 0.0 {
        bc = -bc;
    }
    ab == 0.0 || bc < 0.0
}

/// `valid_unit_divide`.
pub fn valid_unit_divide(mut numer: f32, mut denom: f32) -> Option<f32> {
    if numer < 0.0 {
        numer = -numer;
        denom = -denom;
    }
    if denom == 0.0 || numer == 0.0 || numer >= denom {
        return None;
    }
    let r = numer / denom;
    if r.is_nan() || r == 0.0 {
        return None;
    }
    Some(r)
}

/// `SkFindUnitQuadRoots`.
pub fn find_unit_quad_roots(a: f32, b: f32, c: f32, roots: &mut [f32; 2]) -> usize {
    if a == 0.0 {
        return match valid_unit_divide(-c, b) {
            Some(r) => {
                roots[0] = r;
                1
            }
            None => 0,
        };
    }
    let mut n = 0;
    let dr = b as f64 * b as f64 - 4.0 * a as f64 * c as f64;
    if dr < 0.0 {
        return 0;
    }
    let r = dr.sqrt() as f32;
    if !r.is_finite() {
        return 0;
    }
    let q = if b < 0.0 {
        -(b - r) / 2.0
    } else {
        -(b + r) / 2.0
    };
    if let Some(v) = valid_unit_divide(q, a) {
        roots[n] = v;
        n += 1;
    }
    if let Some(v) = valid_unit_divide(c, q) {
        roots[n] = v;
        n += 1;
    }
    if n == 2 {
        if roots[0] > roots[1] {
            roots.swap(0, 1);
        } else if roots[0] == roots[1] {
            n -= 1;
        }
    }
    n
}

#[inline]
fn interp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// `SkChopQuadAt`.
pub fn chop_quad_at(src: &[Point; 3], t: f32) -> [Point; 5] {
    let p01 = Point::new(interp(src[0].x, src[1].x, t), interp(src[0].y, src[1].y, t));
    let p12 = Point::new(interp(src[1].x, src[2].x, t), interp(src[1].y, src[2].y, t));
    [
        src[0],
        p01,
        Point::new(interp(p01.x, p12.x, t), interp(p01.y, p12.y, t)),
        p12,
        src[2],
    ]
}

/// `SkChopQuadAtYExtrema`: one or two monotonic quads (dst[0..3], dst[2..5]).
pub fn chop_quad_at_y_extrema(src: &[Point; 3], dst: &mut [Point; 5]) -> usize {
    let a = src[0].y;
    let mut b = src[1].y;
    let c = src[2].y;
    if is_not_monotonic(a, b, c) {
        if let Some(t) = valid_unit_divide(a - b, a - b - b + c) {
            *dst = chop_quad_at(src, t);
            // flatten_double_quad_extrema
            dst[1].y = dst[2].y;
            dst[3].y = dst[2].y;
            return 1;
        }
        b = if (a - b).abs() < (b - c).abs() { a } else { c };
    }
    dst[0] = Point::new(src[0].x, a);
    dst[1] = Point::new(src[1].x, b);
    dst[2] = Point::new(src[2].x, c);
    0
}

/// `SkChopQuadAtXExtrema`.
pub fn chop_quad_at_x_extrema(src: &[Point; 3], dst: &mut [Point; 5]) -> usize {
    let a = src[0].x;
    let mut b = src[1].x;
    let c = src[2].x;
    if is_not_monotonic(a, b, c) {
        if let Some(t) = valid_unit_divide(a - b, a - b - b + c) {
            *dst = chop_quad_at(src, t);
            dst[1].x = dst[2].x;
            dst[3].x = dst[2].x;
            return 1;
        }
        b = if (a - b).abs() < (b - c).abs() { a } else { c };
    }
    dst[0] = Point::new(a, src[0].y);
    dst[1] = Point::new(b, src[1].y);
    dst[2] = Point::new(c, src[2].y);
    0
}

/// `SkFindCubicExtrema`.
pub fn find_cubic_extrema(a: f32, b: f32, c: f32, d: f32, t: &mut [f32; 2]) -> usize {
    let aa = d - a + 3.0 * (b - c);
    let bb = 2.0 * (a - b - b + c);
    let cc = b - a;
    find_unit_quad_roots(aa, bb, cc, t)
}

#[inline]
fn mix(a: f32, b: f32, t: f32) -> f32 {
    (b - a) * t + a
}

fn mixp(a: Point, b: Point, t: f32) -> Point {
    Point::new(mix(a.x, b.x, t), mix(a.y, b.y, t))
}

/// `SkChopCubicAt(src, dst[7], t)`.
pub fn chop_cubic_at(src: &[Point], dst: &mut [Point], t: f32) {
    if t == 1.0 {
        dst[..4].copy_from_slice(&src[..4]);
        dst[4] = src[3];
        dst[5] = src[3];
        dst[6] = src[3];
        return;
    }
    let (p0, p1, p2, p3) = (src[0], src[1], src[2], src[3]);
    let ab = mixp(p0, p1, t);
    let bc = mixp(p1, p2, t);
    let cd = mixp(p2, p3, t);
    let abc = mixp(ab, bc, t);
    let bcd = mixp(bc, cd, t);
    let abcd = mixp(abc, bcd, t);
    dst[0] = p0;
    dst[1] = ab;
    dst[2] = abc;
    dst[3] = abcd;
    dst[4] = bcd;
    dst[5] = cd;
    dst[6] = p3;
}

/// `SkChopCubicAt(src, dst[10], t0, t1)`.
fn chop_cubic_at2(src: &[Point], dst: &mut [Point], t0: f32, t1: f32) {
    if t1 == 1.0 {
        chop_cubic_at(src, dst, t0);
        dst[7] = src[3];
        dst[8] = src[3];
        dst[9] = src[3];
        return;
    }
    let (p0, p1, p2, p3) = (src[0], src[1], src[2], src[3]);
    let ab0 = mixp(p0, p1, t0);
    let bc0 = mixp(p1, p2, t0);
    let cd0 = mixp(p2, p3, t0);
    let abc0 = mixp(ab0, bc0, t0);
    let bcd0 = mixp(bc0, cd0, t0);
    let abcd0 = mixp(abc0, bcd0, t0);
    let ab1 = mixp(p0, p1, t1);
    let bc1 = mixp(p1, p2, t1);
    let cd1 = mixp(p2, p3, t1);
    let abc1 = mixp(ab1, bc1, t1);
    let bcd1 = mixp(bc1, cd1, t1);
    let abcd1 = mixp(abc1, bcd1, t1);
    let middle_lo = mixp(abc0, bcd0, t1);
    let middle_hi = mixp(abc1, bcd1, t0);
    dst[0] = p0;
    dst[1] = ab0;
    dst[2] = abc0;
    dst[3] = abcd0;
    dst[4] = middle_lo;
    dst[5] = middle_hi;
    dst[6] = abcd1;
    dst[7] = bcd1;
    dst[8] = cd1;
    dst[9] = p3;
}

/// `SkChopCubicAt(src, dst, tValues, tCount)`.
pub fn chop_cubic_at_ts(src: &[Point; 4], dst: &mut [Point], t_values: &[f32]) {
    if t_values.is_empty() {
        dst[..4].copy_from_slice(src);
        return;
    }
    let mut buf_src: [Point; 4] = *src;
    let mut offset = 0;
    let mut i = 0;
    while i + 1 < t_values.len() {
        let (mut ta, mut tb) = (t_values[i], t_values[i + 1]);
        if i != 0 {
            let last = t_values[i - 1];
            ta = ((ta - last) / (1.0 - last)).clamp(0.0, 1.0);
            tb = ((tb - last) / (1.0 - last)).clamp(0.0, 1.0);
        }
        chop_cubic_at2(&buf_src, &mut dst[offset..], ta, tb);
        offset += 6;
        buf_src.copy_from_slice(&dst[offset..offset + 4]);
        i += 2;
    }
    if i < t_values.len() {
        let mut t = t_values[i];
        if i != 0 {
            let last = t_values[i - 1];
            t = ((t - last) / (1.0 - last)).clamp(0.0, 1.0);
        }
        chop_cubic_at(&buf_src, &mut dst[offset..], t);
    }
}

/// `SkChopCubicAtYExtrema`.
pub fn chop_cubic_at_y_extrema(src: &[Point; 4], dst: &mut [Point; 10]) -> usize {
    let mut t = [0.0f32; 2];
    let roots = find_cubic_extrema(src[0].y, src[1].y, src[2].y, src[3].y, &mut t);
    chop_cubic_at_ts(src, dst, &t[..roots]);
    if roots > 0 {
        // flatten_double_cubic_extrema
        dst[2].y = dst[3].y;
        dst[4].y = dst[3].y;
        if roots == 2 {
            dst[5].y = dst[6].y;
            dst[7].y = dst[6].y;
        }
    }
    roots
}

/// `SkChopCubicAtXExtrema`.
pub fn chop_cubic_at_x_extrema(src: &[Point; 4], dst: &mut [Point; 10]) -> usize {
    let mut t = [0.0f32; 2];
    let roots = find_cubic_extrema(src[0].x, src[1].x, src[2].x, src[3].x, &mut t);
    chop_cubic_at_ts(src, dst, &t[..roots]);
    if roots > 0 {
        // flatten_double_cubic_extrema
        dst[2].x = dst[3].x;
        dst[4].x = dst[3].x;
        if roots == 2 {
            dst[5].x = dst[6].x;
            dst[7].x = dst[6].x;
        }
    }
    roots
}

// --- conics -----------------------------------------------------------------

/// `SkConic`.
#[derive(Clone, Copy, Debug)]
pub struct Conic {
    pub pts: [Point; 3],
    pub w: f32,
}

const MAX_CONIC_TO_QUAD_POW2: i32 = 5;

impl Conic {
    pub fn new(pts: [Point; 3], w: f32) -> Self {
        // setW: guard against bad weights by forcing them to 1.
        let w = if w > 0.0 && w.is_finite() { w } else { 1.0 };
        Self { pts, w }
    }

    /// `SkConic::chop` (the non-legacy version).
    pub fn chop(&self) -> [Conic; 2] {
        let scale = 1.0 / (1.0 + self.w);
        let t0 = self.pts[0] * scale;
        let t1 = self.pts[1] * (self.w * scale);
        let t2 = self.pts[2] * scale;
        let p1 = t0 + t1;
        let p3 = t1 + t2;
        let p2 = Point::new(
            0.5 * t0.x + t1.x + 0.5 * t2.x,
            0.5 * t0.y + t1.y + 0.5 * t2.y,
        );
        let w = (0.5 + self.w * 0.5).sqrt();
        [
            Conic {
                pts: [self.pts[0], p1, p2],
                w,
            },
            Conic {
                pts: [p2, p3, self.pts[2]],
                w,
            },
        ]
    }

    /// `SkConic::computeQuadPOW2`.
    pub fn compute_quad_pow2(&self, tol: f32) -> i32 {
        if tol < 0.0 || !tol.is_finite() || !self.pts.iter().all(|p| p.is_finite()) {
            return 0;
        }
        let a = self.w - 1.0;
        let k = a / (4.0 * (2.0 + a));
        let x = k * (self.pts[0].x - 2.0 * self.pts[1].x + self.pts[2].x);
        let y = k * (self.pts[0].y - 2.0 * self.pts[1].y + self.pts[2].y);
        let mut error = (x * x + y * y).sqrt();
        let mut pow2 = 0;
        while pow2 < MAX_CONIC_TO_QUAD_POW2 {
            if error <= tol {
                break;
            }
            error *= 0.25;
            pow2 += 1;
        }
        pow2
    }

    /// `SkConic::chopIntoQuadsPOW2`: `1 + 2 * count` points.
    pub fn chop_into_quads_pow2(&self, pow2: i32) -> Vec<Point> {
        let mut pts = vec![self.pts[0]];
        let mut pow2 = pow2;
        let mut done = false;
        if pow2 == MAX_CONIC_TO_QUAD_POW2 {
            let dst = self.chop();
            if equals_within_tolerance(dst[0].pts[1], dst[0].pts[2])
                && equals_within_tolerance(dst[1].pts[0], dst[1].pts[1])
            {
                pts.extend([dst[0].pts[1], dst[0].pts[1], dst[0].pts[1], dst[1].pts[2]]);
                pow2 = 1;
                done = true;
            }
        }
        if !done {
            subdivide(self, &mut pts, pow2);
        }
        let quad_count = 1usize << pow2;
        let pt_count = 2 * quad_count + 1;
        debug_assert_eq!(pts.len(), pt_count);
        if !pts.iter().all(|p| p.is_finite()) {
            for p in pts.iter_mut().take(pt_count - 1).skip(1) {
                *p = self.pts[1];
            }
        }
        pts
    }

    /// `SkAutoConicToQuads::computeQuads`.
    pub fn to_quads(&self, tol: f32) -> Vec<Point> {
        let pow2 = self.compute_quad_pow2(tol);
        self.chop_into_quads_pow2(pow2)
    }
}

/// `SkPointPriv::EqualsWithinTolerance`: `!CanNormalize(dx, dy)`.
fn equals_within_tolerance(a: Point, b: Point) -> bool {
    let d = a - b;
    !(d.is_finite() && (d.x != 0.0 || d.y != 0.0))
}

fn between(a: f32, b: f32, c: f32) -> bool {
    (a - b) * (c - b) <= 0.0
}

fn subdivide(src: &Conic, pts: &mut Vec<Point>, level: i32) {
    if level == 0 {
        pts.push(src.pts[1]);
        pts.push(src.pts[2]);
        return;
    }
    let mut dst = src.chop();
    let start_y = src.pts[0].y;
    let end_y = src.pts[2].y;
    if between(start_y, src.pts[1].y, end_y) {
        let mid_y = dst[0].pts[2].y;
        if !between(start_y, mid_y, end_y) {
            let closer_y = if (mid_y - start_y).abs() < (mid_y - end_y).abs() {
                start_y
            } else {
                end_y
            };
            dst[0].pts[2].y = closer_y;
            dst[1].pts[0].y = closer_y;
        }
        if !between(start_y, dst[0].pts[1].y, dst[0].pts[2].y) {
            dst[0].pts[1].y = start_y;
        }
        if !between(dst[1].pts[0].y, dst[1].pts[1].y, end_y) {
            dst[1].pts[1].y = end_y;
        }
    }
    subdivide(&dst[0], pts, level - 1);
    subdivide(&dst[1], pts, level - 1);
}
