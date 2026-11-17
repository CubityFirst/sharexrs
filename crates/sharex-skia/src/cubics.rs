//! `SkChopMonoCubicAtY`/`SkChopMonoCubicAtX` (`SkGeometry.cpp`) and the double
//! precision solvers they use: `SkCubics`, `SkQuads::RootsReal`,
//! `SkBezierCubic::Subdivide`, `sk_doubles_nearly_equal_ulps`.

use crate::geom::Point;

/// `sk_double_nearly_zero`.
fn nearly_zero(a: f64) -> bool {
    a == 0.0 || a.abs() < f32::EPSILON as f64
}

/// `sk_doubles_nearly_equal_ulps(a, b, 16)`.
fn nearly_equal_ulps(a: f64, b: f64) -> bool {
    let magnitude = |v: f64| f64::from_bits(v.to_bits() & 0x7FF0_0000_0000_0000);
    let max_magnitude = magnitude(a).max(f64::MIN_POSITIVE).max(magnitude(b));
    let tolerance = max_magnitude * (f64::EPSILON * (16.0 + 1.0));
    a == b || (b - a).abs() < tolerance
}

/// `SkQuads::RootsReal`.
fn quad_roots_real(a: f64, b: f64, c: f64, solution: &mut [f64]) -> usize {
    let close_to_linear = if a != 0.0 {
        (b / a).abs() >= 1.0e16
    } else {
        true
    };
    if close_to_linear {
        // solve_linear(B, C)
        if nearly_zero(b) {
            solution[0] = 0.0;
            return if nearly_zero(c) { 1 } else { 0 };
        }
        solution[0] = -c / b;
        return if solution[0].is_finite() { 1 } else { 0 };
    }
    // Roots(A, -0.5 * B, C)
    let bb = -0.5 * b;
    let discriminant = {
        let b2 = bb * bb;
        let ac = a * c;
        let rough = b2 - ac;
        if 3.0 * rough.abs() >= b2 + ac {
            rough
        } else {
            let b2_err = bb.mul_add(bb, -b2);
            let ac_err = a.mul_add(c, -ac);
            (b2 - ac) + (b2_err - ac_err)
        }
    };
    let (root0, root1) = if discriminant == 0.0 {
        (bb / a, bb / a)
    } else if discriminant > 0.0 {
        let d = discriminant.sqrt();
        let r = if bb > 0.0 { bb + d } else { bb - d };
        (r / a, c / r)
    } else {
        (f64::NAN, f64::NAN)
    };
    if !discriminant.is_finite() || discriminant < 0.0 {
        return 0;
    }
    let zero_if_tiny = |x: f64| if nearly_zero(x) { 0.0 } else { x };
    let mut roots = 0;
    for r in [root0, root1] {
        let r = zero_if_tiny(r);
        if r.is_finite() {
            solution[roots] = r;
            roots += 1;
        }
    }
    if roots == 2 && nearly_equal_ulps(solution[0], solution[1]) {
        roots = 1;
    }
    roots
}

/// `SkCubics::EvalAt`.
fn eval(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
    t.mul_add(t.mul_add(t.mul_add(a, b), c), d)
}

/// `SkCubics::RootsReal`.
fn roots_real(a_: f64, b_: f64, c_: f64, d_: f64, solution: &mut [f64; 3]) -> usize {
    let nearly_equal = |x: f64, y: f64| {
        if nearly_zero(x) {
            nearly_zero(y)
        } else {
            nearly_equal_ulps(x, y)
        }
    };
    let close_to_a_quadratic = if nearly_zero(b_) {
        nearly_zero(a_)
    } else {
        (a_ / b_).abs() < 1.0e-7
    };
    if close_to_a_quadratic {
        return quad_roots_real(b_, c_, d_, solution);
    }
    if nearly_zero(d_) {
        let num = quad_roots_real(a_, b_, c_, solution);
        if solution[..num].iter().any(|&s| nearly_zero(s)) {
            return num;
        }
        solution[num] = 0.0;
        return num + 1;
    }
    if nearly_zero(a_ + b_ + c_ + d_) {
        let num = quad_roots_real(a_, a_ + b_, -d_, solution);
        if solution[..num].iter().any(|&s| nearly_equal_ulps(s, 1.0)) {
            return num;
        }
        solution[num] = 1.0;
        return num + 1;
    }
    let (a, b, c) = {
        let inv_a = 1.0 / a_;
        (b_ * inv_a, c_ * inv_a, d_ * inv_a)
    };
    let a2 = a * a;
    let q = (a2 - b * 3.0) / 9.0;
    let r = (2.0 * a2 * a - 9.0 * a * b + 27.0 * c) / 54.0;
    let r2 = r * r;
    let q3 = q * q * q;
    let r2_minus_q3 = r2 - q3;
    if !r2_minus_q3.is_finite() {
        return 0;
    }
    let adiv3 = a / 3.0;
    let mut n = 0;
    use std::f64::consts::PI;
    if r2_minus_q3 < 0.0 {
        let theta = (r / q3.sqrt()).clamp(-1.0, 1.0).acos();
        let neg2_root_q = -2.0 * q.sqrt();
        solution[n] = neg2_root_q * (theta / 3.0).cos() - adiv3;
        n += 1;
        let rr = neg2_root_q * ((theta + 2.0 * PI) / 3.0).cos() - adiv3;
        if !nearly_equal(solution[0], rr) {
            solution[n] = rr;
            n += 1;
        }
        let rr = neg2_root_q * ((theta - 2.0 * PI) / 3.0).cos() - adiv3;
        if !nearly_equal(solution[0], rr) && (n == 1 || !nearly_equal(solution[1], rr)) {
            solution[n] = rr;
            n += 1;
        }
    } else {
        let sqrt_r2_minus_q3 = r2_minus_q3.sqrt();
        let mut aa = (r.abs() + sqrt_r2_minus_q3).cbrt();
        if r > 0.0 {
            aa = -aa;
        }
        if !nearly_zero(aa) {
            aa += q / aa;
        }
        solution[n] = aa - adiv3;
        n += 1;
        if !nearly_zero(r2) && nearly_equal_ulps(r2, q3) {
            let rr = -aa / 2.0 - adiv3;
            if !nearly_equal(solution[0], rr) {
                solution[n] = rr;
                n += 1;
            }
        }
    }
    n
}

/// `SkCubics::RootsValidT`.
fn roots_valid_t(a: f64, b: f64, c: f64, d: f64, solution: &mut [f64; 3]) -> usize {
    let mut all = [0.0f64; 3];
    let real = roots_real(a, b, c, d, &mut all);
    let mut found = 0;
    for &t in &all[..real] {
        if (1.0..=1.00005).contains(&t) {
            if (found < 1 || !nearly_equal_ulps(solution[0], 1.0))
                && (found < 2 || !nearly_equal_ulps(solution[1], 1.0))
            {
                solution[found] = 1.0;
                found += 1;
            }
        } else if t >= -0.00005 && (t <= 0.0 || nearly_zero(t)) {
            if (found < 1 || !nearly_zero(solution[0])) && (found < 2 || !nearly_zero(solution[1]))
            {
                solution[found] = 0.0;
                found += 1;
            }
        } else if t > 0.0 && t < 1.0 {
            solution[found] = t;
            found += 1;
        }
    }
    found
}

fn approximately_zero(x: f64) -> bool {
    x.abs() < 0.00000001
}

/// `SkCubics::BinarySearchRootsValidT`.
fn binary_search_roots_valid_t(a: f64, b: f64, c: f64, d: f64, solution: &mut [f64; 3]) -> usize {
    if !a.is_finite() || !b.is_finite() || !c.is_finite() || !d.is_finite() {
        return 0;
    }
    let binary_search = |mut start: f64, mut stop: f64| -> f64 {
        let left = eval(a, b, c, d, start);
        if approximately_zero(left) {
            return start;
        }
        let right = eval(a, b, c, d, stop);
        if !left.is_finite() || !right.is_finite() {
            return -1.0;
        }
        if (left > 0.0 && right > 0.0) || (left < 0.0 && right < 0.0) {
            return -1.0;
        }
        for _ in 0..1000 {
            let step = (start + stop) / 2.0;
            let curr = eval(a, b, c, d, step);
            if approximately_zero(curr) {
                return step;
            }
            if (curr < 0.0 && left < 0.0) || (curr > 0.0 && left > 0.0) {
                start = step;
            } else {
                stop = step;
            }
        }
        -1.0
    };
    let mut regions = [0.0, 0.0, 0.0, 1.0];
    // find_extrema_valid_t
    let mut min_max = [0.0f64; 2];
    let extrema_count = {
        let mut roots = [0.0f64; 2];
        let n = quad_roots_real(3.0 * a, 2.0 * b, c, &mut roots);
        let mut valid = 0;
        for &t in &roots[..n] {
            if (0.0..=1.0).contains(&t) {
                min_max[valid] = t;
                valid += 1;
            }
        }
        valid
    };
    let mut start_index = 2 - extrema_count;
    if extrema_count == 1 {
        regions[start_index + 1] = min_max[0];
    }
    if extrema_count == 2 {
        regions[start_index + 1] = min_max[0].min(min_max[1]);
        regions[start_index + 2] = min_max[0].max(min_max[1]);
    }
    let mut found = 0;
    while start_index < 3 {
        let root = binary_search(regions[start_index], regions[start_index + 1]);
        if root >= 0.0
            && (found < 1 || !approximately_zero(solution[0] - root))
            && (found < 2 || !approximately_zero(solution[1] - root))
        {
            solution[found] = root;
            found += 1;
        }
        start_index += 1;
    }
    found
}

/// `first_axis_intersection`.
fn first_axis_intersection(coef: &[f64; 8], y_direction: bool, intercept: f64) -> Option<f64> {
    let off = if y_direction { 1 } else { 0 };
    let p = |n: usize| coef[2 * n + off];
    // SkBezierCubic::ConvertToPolynomial
    let a = -p(0) + 3.0 * p(1) - 3.0 * p(2) + p(3);
    let b = 3.0 * p(0) - 6.0 * p(1) + 3.0 * p(2);
    let c = -3.0 * p(0) + 3.0 * p(1);
    let d = p(0) - intercept;
    let close_enough_to_zero = |x: f64| x.abs() < 0.00001;
    let mut roots = [0.0f64; 3];
    let count = roots_valid_t(a, b, c, d, &mut roots);
    if count == 0 {
        return None;
    }
    if let Some(&r) = roots[..count]
        .iter()
        .find(|&&r| close_enough_to_zero(eval(a, b, c, d, r)))
    {
        return Some(r);
    }
    let count = binary_search_roots_valid_t(a, b, c, d, &mut roots);
    roots[..count]
        .iter()
        .copied()
        .find(|&r| close_enough_to_zero(eval(a, b, c, d, r)))
}

/// `SkChopMonoCubicAtY` (`y_direction`) / `SkChopMonoCubicAtX`: false when no root
/// is accurate enough.
pub fn chop_mono_cubic_at(
    src: &[Point; 4],
    y_direction: bool,
    intercept: f32,
    dst: &mut [Point; 7],
) -> bool {
    let coef: [f64; 8] = [
        src[0].x as f64,
        src[0].y as f64,
        src[1].x as f64,
        src[1].y as f64,
        src[2].x as f64,
        src[2].y as f64,
        src[3].x as f64,
        src[3].y as f64,
    ];
    let Some(t) = first_axis_intersection(&coef, y_direction, intercept as f64) else {
        return false;
    };
    // SkBezierCubic::Subdivide
    let lerp = |a: f64, b: f64| a + (b - a) * t;
    let mut out = [0.0f64; 14];
    out[0] = coef[0];
    out[1] = coef[1];
    out[12] = coef[6];
    out[13] = coef[7];
    let (x01, y01) = (lerp(coef[0], coef[2]), lerp(coef[1], coef[3]));
    let (x12, y12) = (lerp(coef[2], coef[4]), lerp(coef[3], coef[5]));
    let (x23, y23) = (lerp(coef[4], coef[6]), lerp(coef[5], coef[7]));
    out[2] = x01;
    out[3] = y01;
    out[10] = x23;
    out[11] = y23;
    out[4] = lerp(x01, x12);
    out[5] = lerp(y01, y12);
    out[8] = lerp(x12, x23);
    out[9] = lerp(y12, y23);
    out[6] = lerp(out[4], out[8]);
    out[7] = lerp(out[5], out[9]);
    for i in 0..7 {
        dst[i] = Point::new(out[2 * i] as f32, out[2 * i + 1] as f32);
    }
    true
}
