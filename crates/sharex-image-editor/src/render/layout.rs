//! Avalonia's layout rounding (`LayoutHelper`, at a scale of 1) and the
//! render transform `ImmediateRenderer` builds for a visual.

use sharex_skia::Matrix;

/// Where a `Canvas` child with `Canvas.Left/Top` and `Width/Height` ends
/// up: its position rounded (`Math.Round`, to even), its size rounded up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arranged {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn arrange(left: f64, top: f64, width: f64, height: f64) -> Arranged {
    Arranged {
        x: left.round_ties_even(),
        y: top.round_ties_even(),
        width: width.ceil(),
        height: height.ceil(),
    }
}

/// An Avalonia `Matrix` (row vectors: `x' = x*m11 + y*m21 + m31`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AMatrix {
    pub m11: f64,
    pub m12: f64,
    pub m21: f64,
    pub m22: f64,
    pub m31: f64,
    pub m32: f64,
}

impl AMatrix {
    pub fn translation(x: f64, y: f64) -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            m31: x,
            m32: y,
        }
    }

    /// `Matrix.CreateRotation(radians)`.
    pub fn rotation(radians: f64) -> Self {
        let (s, c) = (radians.sin(), radians.cos());
        Self {
            m11: c,
            m12: s,
            m21: -s,
            m22: c,
            m31: 0.0,
            m32: 0.0,
        }
    }

    /// `self * o` (Avalonia's operator*).
    pub fn mul(&self, o: &AMatrix) -> AMatrix {
        AMatrix {
            m11: self.m11 * o.m11 + self.m12 * o.m21,
            m12: self.m11 * o.m12 + self.m12 * o.m22,
            m21: self.m21 * o.m11 + self.m22 * o.m21,
            m22: self.m21 * o.m12 + self.m22 * o.m22,
            m31: self.m31 * o.m11 + self.m32 * o.m21 + o.m31,
            m32: self.m31 * o.m12 + self.m32 * o.m22 + o.m32,
        }
    }

    /// `ToSKMatrix`.
    pub fn to_sk(self) -> Matrix {
        Matrix {
            sx: self.m11 as f32,
            kx: self.m21 as f32,
            tx: self.m31 as f32,
            ky: self.m12 as f32,
            sy: self.m22 as f32,
            ty: self.m32 as f32,
        }
    }
}

/// Avalonia's `Matrix.ToRadians`: `angle * 0.0174532925` (not pi / 180).
pub fn to_radians(degrees: f64) -> f64 {
    degrees * 0.0174532925
}

/// The transform `ImmediateRenderer` pushes for a visual arranged at
/// `a`, with a `RotateTransform(angle)` about its centre when the
/// annotation is rotated (`ApplyRotationTransform`).
pub fn visual_transform(a: &Arranged, rotation_degrees: f32) -> AMatrix {
    let pos = AMatrix::translation(a.x, a.y);
    if rotation_degrees == 0.0 {
        return pos;
    }
    let origin = AMatrix::translation(a.width * 0.5, a.height * 0.5);
    let neg_origin = AMatrix::translation(-(a.width * 0.5), -(a.height * 0.5));
    // RotateTransform.Value: Matrix.CreateRotation(Matrix.ToRadians(Angle))
    let rot = AMatrix::rotation(to_radians(rotation_degrees as f64));
    neg_origin.mul(&rot).mul(&origin).mul(&pos)
}

/// [`visual_transform`] with a `RenderTransformOrigin` relative to the
/// arranged size (`RelativePoint(x, y, Relative)`), as the speech balloon
/// rotates about its body's centre when it has a tail.
pub fn visual_transform_origin(a: &Arranged, rotation_degrees: f32, origin: (f64, f64)) -> AMatrix {
    let pos = AMatrix::translation(a.x, a.y);
    if rotation_degrees == 0.0 {
        return pos;
    }
    let (ox, oy) = (origin.0 * a.width, origin.1 * a.height);
    let origin = AMatrix::translation(ox, oy);
    let neg_origin = AMatrix::translation(-ox, -oy);
    let rot = AMatrix::rotation(to_radians(rotation_degrees as f64));
    neg_origin.mul(&rot).mul(&origin).mul(&pos)
}
