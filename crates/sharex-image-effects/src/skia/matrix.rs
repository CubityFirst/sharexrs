//! `SkMatrix` (affine part) with Skia's own float arithmetic, so the matrices the
//! raster pipeline samples through are bit for bit the ones Skia computes
//! (`src/core/SkMatrix.cpp`, and `SkM44` for the canvas's own transforms).

/// Row-major 3x3, Skia's `fMat` layout: `[sx, kx, tx, ky, sy, ty, p0, p1, p2]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix(pub [f32; 9]);

const SX: usize = 0;
const KX: usize = 1;
const TX: usize = 2;
const KY: usize = 3;
const SY: usize = 4;
const TY: usize = 5;

impl Default for Matrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);

    pub fn translate(dx: f32, dy: f32) -> Self {
        Matrix([1.0, 0.0, dx, 0.0, 1.0, dy, 0.0, 0.0, 1.0])
    }

    pub fn scale_translate(sx: f32, sy: f32, tx: f32, ty: f32) -> Self {
        Matrix([sx, 0.0, tx, 0.0, sy, ty, 0.0, 0.0, 1.0])
    }

    /// `SkMatrix::setRotate(degrees)`: `SkDegreesToRadians`, sine and cosine snapped to
    /// zero below `SK_ScalarNearlyZero`.
    pub fn rotate(degrees: f32) -> Self {
        let rad = degrees * (std::f32::consts::PI / 180.0);
        let snap = |v: f32| if v.abs() <= 1.0 / 4096.0 { 0.0 } else { v };
        let (s, c) = (snap(rad.sin()), snap(rad.cos()));
        Matrix([c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0])
    }

    pub fn sx(&self) -> f32 {
        self.0[SX]
    }
    pub fn sy(&self) -> f32 {
        self.0[SY]
    }
    pub fn kx(&self) -> f32 {
        self.0[KX]
    }
    pub fn ky(&self) -> f32 {
        self.0[KY]
    }
    pub fn tx(&self) -> f32 {
        self.0[TX]
    }
    pub fn ty(&self) -> f32 {
        self.0[TY]
    }

    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    pub fn is_translate(&self) -> bool {
        self.0[SX] == 1.0 && self.0[SY] == 1.0 && self.is_scale_translate()
    }

    pub fn is_scale_translate(&self) -> bool {
        self.0[KX] == 0.0 && self.0[KY] == 0.0
    }

    /// `rectStaysRect`: axis-aligned rectangles map to axis-aligned rectangles.
    pub fn rect_stays_rect(&self) -> bool {
        (self.0[KX] == 0.0 && self.0[KY] == 0.0 && self.0[SX] != 0.0 && self.0[SY] != 0.0)
            || (self.0[SX] == 0.0 && self.0[SY] == 0.0 && self.0[KX] != 0.0 && self.0[KY] != 0.0)
    }

    /// `SkMatrix::setConcat(a, b)` (`a * b`): scale/translate in float, affine with
    /// `muladdmul` in double.
    pub fn concat(a: &Matrix, b: &Matrix) -> Matrix {
        if a.is_identity() {
            return *b;
        }
        if b.is_identity() {
            return *a;
        }
        let (a, b) = (&a.0, &b.0);
        if a[KX] == 0.0 && a[KY] == 0.0 && b[KX] == 0.0 && b[KY] == 0.0 {
            return Matrix::scale_translate(
                a[SX] * b[SX],
                a[SY] * b[SY],
                a[SX] * b[TX] + a[TX],
                a[SY] * b[TY] + a[TY],
            );
        }
        let mam =
            |a: f32, b: f32, c: f32, d: f32| (a as f64 * b as f64 + c as f64 * d as f64) as f32;
        Matrix([
            mam(a[SX], b[SX], a[KX], b[KY]),
            mam(a[SX], b[KX], a[KX], b[SY]),
            mam(a[SX], b[TX], a[KX], b[TY]) + a[TX],
            mam(a[KY], b[SX], a[SY], b[KY]),
            mam(a[KY], b[KX], a[SY], b[SY]),
            mam(a[KY], b[TX], a[SY], b[TY]) + a[TY],
            0.0,
            0.0,
            1.0,
        ])
    }

    /// `SkM44::setConcat(this, b)` restricted to 2D, as `SkCanvas::concat` keeps its
    /// matrix: each column `c0*r0 + (c1*r1 + (c2*r2 + c3*r3))` in float.
    pub fn m44_pre_concat(&self, b: &Matrix) -> Matrix {
        let a = &self.0;
        let b = &b.0;
        let col = |r0: f32, r1: f32, r3: f32| -> (f32, f32) {
            (
                a[SX] * r0 + (a[KX] * r1 + (0.0 + a[TX] * r3)),
                a[KY] * r0 + (a[SY] * r1 + (0.0 + a[TY] * r3)),
            )
        };
        let (sx, ky) = col(b[SX], b[KY], 0.0);
        let (kx, sy) = col(b[KX], b[SY], 0.0);
        let (tx, ty) = col(b[TX], b[TY], 1.0);
        Matrix([sx, kx, tx, ky, sy, ty, 0.0, 0.0, 1.0])
    }

    /// `SkM44::preTranslate`: `col3 = c0*x + (c1*y + (c2*z + c3))`.
    pub fn m44_pre_translate(&self, dx: f32, dy: f32) -> Matrix {
        let mut m = *self;
        m.0[TX] = self.0[SX] * dx + (self.0[KX] * dy + self.0[TX]);
        m.0[TY] = self.0[KY] * dx + (self.0[SY] * dy + self.0[TY]);
        m
    }

    /// `SkM44::preScale`.
    pub fn m44_pre_scale(&self, sx: f32, sy: f32) -> Matrix {
        let mut m = *self;
        m.0[SX] *= sx;
        m.0[KY] *= sx;
        m.0[KX] *= sy;
        m.0[SY] *= sy;
        m
    }

    /// `SkMatrix::invert`.
    pub fn invert(&self) -> Option<Matrix> {
        let m = &self.0;
        if self.is_identity() {
            return Some(*self);
        }
        if self.is_scale_translate() {
            if m[SX] == 1.0 && m[SY] == 1.0 {
                return Some(Matrix::translate(-m[TX], -m[TY]));
            }
            let ix = 1.0 / m[SX];
            let iy = 1.0 / m[SY];
            if !ix.is_finite() || !iy.is_finite() {
                return None;
            }
            return Some(Matrix::scale_translate(ix, iy, -m[TX] * ix, -m[TY] * iy));
        }
        let det = m[SX] as f64 * m[SY] as f64 - m[KX] as f64 * m[KY] as f64;
        let nearly_zero = 1.0f32 / 4096.0;
        if ((det as f32).abs()) <= nearly_zero * nearly_zero * nearly_zero {
            return None;
        }
        let inv = 1.0 / det;
        let dcross = |a: f32, b: f32, c: f32, d: f32| {
            ((a as f64 * b as f64 - c as f64 * d as f64) * inv) as f32
        };
        let out = Matrix([
            (m[SY] as f64 * inv) as f32,
            (-m[KX] as f64 * inv) as f32,
            dcross(m[KX], m[TY], m[SY], m[TX]),
            (-m[KY] as f64 * inv) as f32,
            (m[SX] as f64 * inv) as f32,
            dcross(m[KY], m[TX], m[SX], m[TY]),
            0.0,
            0.0,
            1.0,
        ]);
        out.0.iter().all(|v| v.is_finite()).then_some(out)
    }

    /// `SkMatrix::mapXY` for an affine matrix.
    pub fn map(&self, x: f32, y: f32) -> (f32, f32) {
        let m = &self.0;
        if self.is_scale_translate() {
            (x * m[SX] + m[TX], y * m[SY] + m[TY])
        } else {
            (
                m[SX] * x + (m[KX] * y + m[TX]),
                m[KY] * x + (m[SY] * y + m[TY]),
            )
        }
    }

    /// `SkMatrix::RectToRect(src, dst)` (`kFill_ScaleToFit`).
    pub fn rect_to_rect(src: Rect, dst: Rect) -> Matrix {
        let sx = dst.width() / src.width();
        let sy = dst.height() / src.height();
        let tx = dst.left - src.left * sx;
        let ty = dst.top - src.top * sy;
        if sx == 1.0 && sy == 1.0 {
            Matrix::translate(tx, ty)
        } else {
            Matrix::scale_translate(sx, sy, tx, ty)
        }
    }

    /// `SkMatrix::mapRect`: the bounds of the mapped corners.
    pub fn map_rect(&self, r: Rect) -> Rect {
        let pts = [
            self.map(r.left, r.top),
            self.map(r.right, r.top),
            self.map(r.right, r.bottom),
            self.map(r.left, r.bottom),
        ];
        let mut out = Rect {
            left: f32::MAX,
            top: f32::MAX,
            right: f32::MIN,
            bottom: f32::MIN,
        };
        for (x, y) in pts {
            out.left = out.left.min(x);
            out.top = out.top.min(y);
            out.right = out.right.max(x);
            out.bottom = out.bottom.max(y);
        }
        out
    }
}

/// `SkRect`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub fn ltrb(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn xywh(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        }
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn is_empty(&self) -> bool {
        !(self.left < self.right && self.top < self.bottom)
    }
}
