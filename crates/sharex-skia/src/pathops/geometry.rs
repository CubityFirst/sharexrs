//! The parts of `src/core/SkGeometry.cpp` the path ops call:
//! `SkFindQuadMaxCurvature`, `SkChopQuadAtMaxCurvature`, `SkConic::chopAt`
//! and `SkClassifyCubic`.

use crate::geom::{Conic, Point};

/// `SkFindQuadMaxCurvature`.
pub fn find_quad_max_curvature(src: &[Point]) -> f32 {
    let ax = src[1].x - src[0].x;
    let ay = src[1].y - src[0].y;
    let bx = src[0].x - src[1].x - src[1].x + src[2].x;
    let by = src[0].y - src[1].y - src[1].y + src[2].y;

    let mut numer = -(ax * bx + ay * by);
    let mut denom = bx * bx + by * by;
    if denom < 0.0 {
        numer = -numer;
        denom = -denom;
    }
    if numer <= 0.0 {
        return 0.0;
    }
    if numer >= denom {
        // Also catches denom=0.
        return 1.0;
    }
    numer / denom
}

/// `SkChopQuadAt`.
pub fn chop_quad_at(src: &[Point], t: f32) -> [Point; 5] {
    let interp = |a: Point, b: Point| Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
    let p01 = interp(src[0], src[1]);
    let p12 = interp(src[1], src[2]);
    [src[0], p01, interp(p01, p12), p12, src[2]]
}

/// `SkChopQuadAtMaxCurvature`: 2 quads in `dst`, or 1 (the source).
pub fn chop_quad_at_max_curvature(src: &[Point], dst: &mut [Point; 5]) -> i32 {
    let t = find_quad_max_curvature(src);
    if t > 0.0 && t < 1.0 {
        *dst = chop_quad_at(src, t);
        2
    } else {
        dst[..3].copy_from_slice(&src[..3]);
        1
    }
}

/// `SkConic::chopAt(SkScalar t, SkConic dst[2])`: false if infinity or NaN
/// is generated.
pub fn conic_chop_at(conic: &Conic, t: f32, dst: &mut [Conic; 2]) -> bool {
    let w = conic.w;
    let p = &conic.pts;
    // ratquad_mapTo3D
    let tmp = [
        [p[0].x * 1.0, p[0].y * 1.0, 1.0],
        [p[1].x * w, p[1].y * w, w],
        [p[2].x * 1.0, p[2].y * 1.0, 1.0],
    ];
    let interp = |a: f32, b: f32| a + (b - a) * t;
    let mut tmp2 = [[0.0f32; 3]; 3];
    for c in 0..3 {
        let ab = interp(tmp[0][c], tmp[1][c]);
        let bc = interp(tmp[1][c], tmp[2][c]);
        tmp2[0][c] = ab;
        tmp2[1][c] = interp(ab, bc);
        tmp2[2][c] = bc;
    }
    let project_down = |s: [f32; 3]| Point::new(s[0] / s[2], s[1] / s[2]);
    dst[0].pts[0] = p[0];
    dst[0].pts[1] = project_down(tmp2[0]);
    dst[0].pts[2] = project_down(tmp2[1]);
    dst[1].pts[0] = dst[0].pts[2];
    dst[1].pts[1] = project_down(tmp2[2]);
    dst[1].pts[2] = p[2];
    let root = tmp2[1][2].sqrt();
    dst[0].w = tmp2[0][2] / root;
    dst[1].w = tmp2[2][2] / root;
    dst.iter().all(|c| c.pts.iter().all(|q| q.x.is_finite() && q.y.is_finite()) && c.w.is_finite())
}

/// `SkCubicType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CubicType {
    Serpentine,
    Loop,
    LocalCusp,
    CuspAtInfinity,
    Quadratic,
    LineOrPoint,
}

// Calcs p0 . (p1 x p2), the third component of the points being 1.
fn calc_dot_cross_cubic(p0: Point, p1: Point, p2: Point) -> f64 {
    let x_comp = p0.x as f64 * (p1.y as f64 - p2.y as f64);
    let y_comp = p0.y as f64 * (p2.x as f64 - p1.x as f64);
    let w_comp = p1.x as f64 * p2.y as f64 - p1.y as f64 * p2.x as f64;
    x_comp + y_comp + w_comp
}

fn previous_inverse_pow2(n: f64) -> f64 {
    let mut bits = n.to_bits();
    bits = (((1023u64 * 2) << 52) + ((1u64 << 52) - 1)).wrapping_sub(bits); // exp=-exp
    bits &= 0x7ffu64 << 52; // mantissa=1.0, sign=0
    f64::from_bits(bits)
}

fn write_cubic_inflection_roots(t0: f64, s0: f64, t1: f64, s1: f64, t: &mut [f64; 2], s: &mut [f64; 2]) {
    t[0] = t0;
    s[0] = s0;
    // This copysign/abs business orients the implicit function so positive values are always on
    // the "left" side of the curve.
    t[1] = -t1.copysign(t1 * s1);
    s[1] = -s1.abs();
    // Ensure t[0]/s[0] <= t[1]/s[1] (s[1] is negative from above).
    if s[1].copysign(s[0]) * t[0] > -s[0].abs() * t[1] {
        t.swap(0, 1);
        s.swap(0, 1);
    }
}

/// `SkClassifyCubic(P, t, s, nullptr)`.
pub fn classify_cubic(p: &[Point], t: &mut [f64; 2], s: &mut [f64; 2]) -> CubicType {
    let a1 = calc_dot_cross_cubic(p[0], p[3], p[2]);
    let a2 = calc_dot_cross_cubic(p[1], p[0], p[3]);
    let a3 = calc_dot_cross_cubic(p[2], p[1], p[0]);

    let mut d3 = 3.0 * a3;
    let mut d2 = d3 - a2;
    let mut d1 = d2 - a2 + a1;

    // Shift the exponents in D so the largest magnitude falls somewhere in 1..2.
    let fmax = |a: f64, b: f64| if a < b { b } else { a };
    let dmax = fmax(fmax(d1.abs(), d2.abs()), d3.abs());
    let norm = previous_inverse_pow2(dmax);
    d1 *= norm;
    d2 *= norm;
    d3 *= norm;

    if 0.0 != d1 {
        let discr = 3.0 * d2 * d2 - 4.0 * d1 * d3;
        if discr > 0.0 {
            // Serpentine.
            let q = 3.0 * d2 + (3.0 * discr).sqrt().copysign(d2);
            write_cubic_inflection_roots(q, 6.0 * d1, 2.0 * d3, q, t, s);
            CubicType::Serpentine
        } else if discr < 0.0 {
            // Loop.
            let q = d2 + (-discr).sqrt().copysign(d2);
            write_cubic_inflection_roots(q, 2.0 * d1, 2.0 * (d2 * d2 - d3 * d1), d1 * q, t, s);
            CubicType::Loop
        } else {
            // Cusp.
            write_cubic_inflection_roots(d2, 2.0 * d1, d2, 2.0 * d1, t, s);
            CubicType::LocalCusp
        }
    } else if 0.0 != d2 {
        // Cusp at T=infinity.
        write_cubic_inflection_roots(d3, 3.0 * d2, 1.0, 0.0, t, s); // T1=infinity.
        CubicType::CuspAtInfinity
    } else {
        // Degenerate.
        write_cubic_inflection_roots(1.0, 0.0, 1.0, 0.0, t, s); // T0=T1=infinity.
        if 0.0 != d3 { CubicType::Quadratic } else { CubicType::LineOrPoint }
    }
}
