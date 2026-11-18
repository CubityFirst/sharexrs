//! `SkPathOpsTypes.h`, `SkPathOpsTypes.cpp`: the tolerance tests
//! (`approximately_*`, the ULPs comparisons) and `SkOpGlobalState`.

use super::arena::Arena;
use super::coincidence::Coincidence;
use super::graph::Contour;

pub const WINDING_PATH_OPS_MASK: i32 = -1;
pub const NO_PATH_OPS_MASK: i32 = 0;
pub const EVEN_ODD_PATH_OPS_MASK: i32 = 1;

/// `SkOpPhase`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpPhase {
    NoChange,
    Intersecting,
    Walking,
    FixWinding,
}

/// `SkOpGlobalState` (release build: no debug ids).
pub struct GlobalState {
    pub allocator: Arena,
    pub coincidence: *mut Coincidence,
    pub contour_head: Contour,
    pub nested: i32,
    pub allocated_op_span: bool,
    pub winding_failed: bool,
    pub phase: OpPhase,
}

pub const MAX_WINDING_TRIES: i32 = 10;

impl GlobalState {
    pub fn new(head: Contour) -> Self {
        GlobalState {
            allocator: Arena::default(),
            coincidence: std::ptr::null_mut(),
            contour_head: head,
            nested: 0,
            allocated_op_span: false,
            winding_failed: false,
            phase: OpPhase::Intersecting,
        }
    }

    pub fn set_phase(&mut self, phase: OpPhase) {
        if OpPhase::NoChange == phase {
            return;
        }
        self.phase = phase;
    }
}

// `SkFloatAs2sCompliment`: the float's bits as a two's complement integer.
fn float_as_2s_compliment(x: f32) -> i32 {
    let mut bits = x.to_bits() as i32;
    if bits < 0 {
        bits &= 0x7FFF_FFFF;
        bits = bits.wrapping_neg();
    }
    bits
}

const FLT_EPSILON_F: f32 = f32::EPSILON;

fn arguments_denormalized(a: f32, b: f32, epsilon: i32) -> bool {
    let denormalized_check = FLT_EPSILON_F * epsilon as f32 / 2.0;
    a.abs() <= denormalized_check && b.abs() <= denormalized_check
}

fn equal_ulps(a: f32, b: f32, epsilon: i32, depsilon: i32) -> bool {
    if arguments_denormalized(a, b, depsilon) {
        return true;
    }
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits < b_bits.wrapping_add(epsilon) && b_bits < a_bits.wrapping_add(epsilon)
}

fn equal_ulps_no_normal_check(a: f32, b: f32, epsilon: i32, _depsilon: i32) -> bool {
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits < b_bits.wrapping_add(epsilon) && b_bits < a_bits.wrapping_add(epsilon)
}

fn equal_ulps_pin(a: f32, b: f32, epsilon: i32, depsilon: i32) -> bool {
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    if arguments_denormalized(a, b, depsilon) {
        return true;
    }
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits < b_bits.wrapping_add(epsilon) && b_bits < a_bits.wrapping_add(epsilon)
}

fn d_equal_ulps(a: f32, b: f32, epsilon: i32) -> bool {
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits < b_bits.wrapping_add(epsilon) && b_bits < a_bits.wrapping_add(epsilon)
}

fn not_equal_ulps(a: f32, b: f32, epsilon: i32) -> bool {
    if arguments_denormalized(a, b, epsilon) {
        return false;
    }
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits >= b_bits.wrapping_add(epsilon) || b_bits >= a_bits.wrapping_add(epsilon)
}

fn not_equal_ulps_pin(a: f32, b: f32, epsilon: i32) -> bool {
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    if arguments_denormalized(a, b, epsilon) {
        return false;
    }
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits >= b_bits.wrapping_add(epsilon) || b_bits >= a_bits.wrapping_add(epsilon)
}

fn d_not_equal_ulps(a: f32, b: f32, epsilon: i32) -> bool {
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits >= b_bits.wrapping_add(epsilon) || b_bits >= a_bits.wrapping_add(epsilon)
}

fn less_ulps(a: f32, b: f32, epsilon: i32) -> bool {
    if arguments_denormalized(a, b, epsilon) {
        return a <= b - FLT_EPSILON_F * epsilon as f32;
    }
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits <= b_bits.wrapping_sub(epsilon)
}

fn less_or_equal_ulps(a: f32, b: f32, epsilon: i32) -> bool {
    if arguments_denormalized(a, b, epsilon) {
        return a < b + FLT_EPSILON_F * epsilon as f32;
    }
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits < b_bits.wrapping_add(epsilon)
}

// The float versions; the double overloads (`inline ... (double a, double b)`)
// convert to float first (`SkDoubleToScalar`), which is what the `_d` names do.

/// `AlmostBequalUlps`: equality with the same error term as between.
pub fn almost_bequal_ulps(a: f32, b: f32) -> bool {
    equal_ulps(a, b, 2, 2)
}
pub fn almost_bequal_ulps_d(a: f64, b: f64) -> bool {
    almost_bequal_ulps(a as f32, b as f32)
}

pub fn almost_pequal_ulps(a: f32, b: f32) -> bool {
    equal_ulps(a, b, 8, 8)
}
pub fn almost_pequal_ulps_d(a: f64, b: f64) -> bool {
    almost_pequal_ulps(a as f32, b as f32)
}

pub fn almost_dequal_ulps(a: f32, b: f32) -> bool {
    d_equal_ulps(a, b, 16)
}

/// `AlmostDequalUlps(double, double)`.
pub fn almost_dequal_ulps_d(a: f64, b: f64) -> bool {
    if a.abs() < f32::MAX as f64 && b.abs() < f32::MAX as f64 {
        return almost_dequal_ulps(a as f32, b as f32);
    }
    ((a - b).abs() / smax(a.abs(), b.abs())) < (FLT_EPSILON_F * 16.0) as f64
}

pub fn almost_equal_ulps(a: f32, b: f32) -> bool {
    equal_ulps(a, b, 16, 16)
}
pub fn almost_equal_ulps_d(a: f64, b: f64) -> bool {
    almost_equal_ulps(a as f32, b as f32)
}

pub fn almost_equal_ulps_no_normal_check(a: f32, b: f32) -> bool {
    equal_ulps_no_normal_check(a, b, 16, 16)
}
pub fn almost_equal_ulps_no_normal_check_d(a: f64, b: f64) -> bool {
    almost_equal_ulps_no_normal_check(a as f32, b as f32)
}

pub fn almost_equal_ulps_pin(a: f32, b: f32) -> bool {
    equal_ulps_pin(a, b, 16, 16)
}
pub fn almost_equal_ulps_pin_d(a: f64, b: f64) -> bool {
    almost_equal_ulps_pin(a as f32, b as f32)
}

pub fn not_almost_equal_ulps(a: f32, b: f32) -> bool {
    not_equal_ulps(a, b, 16)
}
pub fn not_almost_equal_ulps_d(a: f64, b: f64) -> bool {
    not_almost_equal_ulps(a as f32, b as f32)
}

pub fn not_almost_equal_ulps_pin(a: f32, b: f32) -> bool {
    not_equal_ulps_pin(a, b, 16)
}
pub fn not_almost_equal_ulps_pin_d(a: f64, b: f64) -> bool {
    not_almost_equal_ulps_pin(a as f32, b as f32)
}

pub fn not_almost_dequal_ulps(a: f32, b: f32) -> bool {
    d_not_equal_ulps(a, b, 16)
}
pub fn not_almost_dequal_ulps_d(a: f64, b: f64) -> bool {
    not_almost_dequal_ulps(a as f32, b as f32)
}

pub fn roughly_equal_ulps(a: f32, b: f32) -> bool {
    equal_ulps(a, b, 256, 1024)
}
pub fn roughly_equal_ulps_d(a: f64, b: f64) -> bool {
    roughly_equal_ulps(a as f32, b as f32)
}

pub fn almost_between_ulps(a: f32, b: f32, c: f32) -> bool {
    const ULPS_EPSILON: i32 = 2;
    if a <= c {
        less_or_equal_ulps(a, b, ULPS_EPSILON) && less_or_equal_ulps(b, c, ULPS_EPSILON)
    } else {
        less_or_equal_ulps(b, a, ULPS_EPSILON) && less_or_equal_ulps(c, b, ULPS_EPSILON)
    }
}
pub fn almost_between_ulps_d(a: f64, b: f64, c: f64) -> bool {
    almost_between_ulps(a as f32, b as f32, c as f32)
}

pub fn almost_less_ulps(a: f32, b: f32) -> bool {
    less_ulps(a, b, 16)
}
pub fn almost_less_ulps_d(a: f64, b: f64) -> bool {
    almost_less_ulps(a as f32, b as f32)
}

pub fn almost_less_or_equal_ulps(a: f32, b: f32) -> bool {
    less_or_equal_ulps(a, b, 16)
}
pub fn almost_less_or_equal_ulps_d(a: f64, b: f64) -> bool {
    almost_less_or_equal_ulps(a as f32, b as f32)
}

/// `UlpsDistance`.
pub fn ulps_distance(a: f32, b: f32) -> i32 {
    let ia = a.to_bits() as i32;
    let ib = b.to_bits() as i32;
    if (ia < 0) != (ib < 0) {
        return if a == b { 0 } else { i32::MAX };
    }
    ia.wrapping_sub(ib).wrapping_abs()
}
pub fn ulps_distance_d(a: f64, b: f64) -> i32 {
    ulps_distance(a as f32, b as f32)
}

// FLT_EPSILON == 1.19209290E-07 == 1 / (2 ^ 23); the products are exact
// powers of two, so computing them in double gives the same constants.
pub const FLT_EPSILON: f64 = f32::EPSILON as f64;
pub const DBL_EPSILON: f64 = f64::EPSILON;
pub const FLT_EPSILON_CUBED: f64 = FLT_EPSILON * FLT_EPSILON * FLT_EPSILON;
pub const FLT_EPSILON_HALF: f64 = FLT_EPSILON / 2.0;
pub const FLT_EPSILON_DOUBLE: f64 = FLT_EPSILON * 2.0;
pub const FLT_EPSILON_ORDERABLE_ERR: f64 = FLT_EPSILON * 16.0;
pub const FLT_EPSILON_SQUARED: f64 = FLT_EPSILON * FLT_EPSILON;
pub const FLT_EPSILON_SQRT: f64 = 0.00034526697709225118;
pub const FLT_EPSILON_INVERSE: f64 = 1.0 / FLT_EPSILON;
pub const DBL_EPSILON_ERR: f64 = DBL_EPSILON * 4.0;
pub const DBL_EPSILON_SUBDIVIDE_ERR: f64 = DBL_EPSILON * 16.0;
pub const ROUGH_EPSILON: f64 = FLT_EPSILON * 64.0;
pub const MORE_ROUGH_EPSILON: f64 = FLT_EPSILON * 256.0;
pub const WAY_ROUGH_EPSILON: f64 = FLT_EPSILON * 2048.0;
pub const BUMP_EPSILON: f64 = FLT_EPSILON * 4096.0;

pub const INVERSE_NUMBER_RANGE: f32 = FLT_EPSILON_ORDERABLE_ERR as f32;

/// `std::min`: `(b < a) ? b : a`.
#[inline]
pub fn smin(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// `std::max`: `(a < b) ? b : a`.
#[inline]
pub fn smax(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

#[inline]
pub fn sminf(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

#[inline]
pub fn smaxf(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

#[inline]
pub fn zero_or_one(x: f64) -> bool {
    x == 0.0 || x == 1.0
}

#[inline]
pub fn approximately_zero(x: f64) -> bool {
    x.abs() < FLT_EPSILON
}

#[inline]
pub fn precisely_zero(x: f64) -> bool {
    x.abs() < DBL_EPSILON_ERR
}

#[inline]
pub fn precisely_subdivide_zero(x: f64) -> bool {
    x.abs() < DBL_EPSILON_SUBDIVIDE_ERR
}

/// `approximately_zero(float)`.
#[inline]
pub fn approximately_zero_f(x: f32) -> bool {
    x.abs() < FLT_EPSILON_F
}

#[inline]
pub fn approximately_zero_half(x: f64) -> bool {
    x.abs() < FLT_EPSILON_HALF
}

#[inline]
pub fn approximately_zero_double(x: f64) -> bool {
    x.abs() < FLT_EPSILON_DOUBLE
}

#[inline]
pub fn approximately_zero_orderable(x: f64) -> bool {
    x.abs() < FLT_EPSILON_ORDERABLE_ERR
}

#[inline]
pub fn approximately_zero_squared(x: f64) -> bool {
    x.abs() < FLT_EPSILON_SQUARED
}

#[inline]
pub fn approximately_zero_sqrt(x: f64) -> bool {
    x.abs() < FLT_EPSILON_SQRT
}

#[inline]
pub fn roughly_zero(x: f64) -> bool {
    x.abs() < ROUGH_EPSILON
}

#[inline]
pub fn approximately_zero_inverse(x: f64) -> bool {
    x.abs() > FLT_EPSILON_INVERSE
}

#[inline]
pub fn approximately_zero_when_compared_to(x: f64, y: f64) -> bool {
    x == 0.0 || x.abs() < (y * FLT_EPSILON).abs()
}

#[inline]
pub fn precisely_zero_when_compared_to(x: f64, y: f64) -> bool {
    x == 0.0 || x.abs() < (y * DBL_EPSILON).abs()
}

#[inline]
pub fn roughly_zero_when_compared_to(x: f64, y: f64) -> bool {
    x == 0.0 || x.abs() < (y * ROUGH_EPSILON).abs()
}

#[inline]
pub fn approximately_equal(x: f64, y: f64) -> bool {
    approximately_zero(x - y)
}

#[inline]
pub fn precisely_equal(x: f64, y: f64) -> bool {
    precisely_zero(x - y)
}

#[inline]
pub fn precisely_subdivide_equal(x: f64, y: f64) -> bool {
    precisely_subdivide_zero(x - y)
}

#[inline]
pub fn approximately_equal_half(x: f64, y: f64) -> bool {
    approximately_zero_half(x - y)
}

#[inline]
pub fn approximately_equal_double(x: f64, y: f64) -> bool {
    approximately_zero_double(x - y)
}

#[inline]
pub fn approximately_equal_orderable(x: f64, y: f64) -> bool {
    approximately_zero_orderable(x - y)
}

#[inline]
pub fn approximately_equal_squared(x: f64, y: f64) -> bool {
    approximately_equal(x, y)
}

#[inline]
pub fn approximately_greater(x: f64, y: f64) -> bool {
    x - FLT_EPSILON >= y
}

#[inline]
pub fn approximately_greater_double(x: f64, y: f64) -> bool {
    x - FLT_EPSILON_DOUBLE >= y
}

#[inline]
pub fn approximately_greater_orderable(x: f64, y: f64) -> bool {
    x - FLT_EPSILON_ORDERABLE_ERR >= y
}

#[inline]
pub fn approximately_greater_or_equal(x: f64, y: f64) -> bool {
    x + FLT_EPSILON > y
}

#[inline]
pub fn approximately_greater_or_equal_double(x: f64, y: f64) -> bool {
    x + FLT_EPSILON_DOUBLE > y
}

#[inline]
pub fn approximately_greater_or_equal_orderable(x: f64, y: f64) -> bool {
    x + FLT_EPSILON_ORDERABLE_ERR > y
}

#[inline]
pub fn approximately_lesser(x: f64, y: f64) -> bool {
    x + FLT_EPSILON <= y
}

#[inline]
pub fn approximately_lesser_double(x: f64, y: f64) -> bool {
    x + FLT_EPSILON_DOUBLE <= y
}

#[inline]
pub fn approximately_lesser_orderable(x: f64, y: f64) -> bool {
    x + FLT_EPSILON_ORDERABLE_ERR <= y
}

#[inline]
pub fn approximately_lesser_or_equal(x: f64, y: f64) -> bool {
    x - FLT_EPSILON < y
}

#[inline]
pub fn approximately_lesser_or_equal_double(x: f64, y: f64) -> bool {
    x - FLT_EPSILON_DOUBLE < y
}

#[inline]
pub fn approximately_lesser_or_equal_orderable(x: f64, y: f64) -> bool {
    x - FLT_EPSILON_ORDERABLE_ERR < y
}

#[inline]
pub fn approximately_greater_than_one(x: f64) -> bool {
    x > 1.0 - FLT_EPSILON
}

#[inline]
pub fn precisely_greater_than_one(x: f64) -> bool {
    x > 1.0 - DBL_EPSILON_ERR
}

#[inline]
pub fn approximately_less_than_zero(x: f64) -> bool {
    x < FLT_EPSILON
}

#[inline]
pub fn precisely_less_than_zero(x: f64) -> bool {
    x < DBL_EPSILON_ERR
}

#[inline]
pub fn approximately_negative(x: f64) -> bool {
    x < FLT_EPSILON
}

#[inline]
pub fn approximately_negative_orderable(x: f64) -> bool {
    x < FLT_EPSILON_ORDERABLE_ERR
}

#[inline]
pub fn precisely_negative(x: f64) -> bool {
    x < DBL_EPSILON_ERR
}

#[inline]
pub fn approximately_one_or_less(x: f64) -> bool {
    x < 1.0 + FLT_EPSILON
}

#[inline]
pub fn approximately_one_or_less_double(x: f64) -> bool {
    x < 1.0 + FLT_EPSILON_DOUBLE
}

#[inline]
pub fn approximately_positive(x: f64) -> bool {
    x > -FLT_EPSILON
}

#[inline]
pub fn approximately_positive_squared(x: f64) -> bool {
    x > -(FLT_EPSILON_SQUARED)
}

#[inline]
pub fn approximately_zero_or_more(x: f64) -> bool {
    x > -FLT_EPSILON
}

#[inline]
pub fn approximately_zero_or_more_double(x: f64) -> bool {
    x > -FLT_EPSILON_DOUBLE
}

#[inline]
pub fn approximately_between_orderable(a: f64, b: f64, c: f64) -> bool {
    if a <= c {
        approximately_negative_orderable(a - b) && approximately_negative_orderable(b - c)
    } else {
        approximately_negative_orderable(b - a) && approximately_negative_orderable(c - b)
    }
}

#[inline]
pub fn approximately_between(a: f64, b: f64, c: f64) -> bool {
    if a <= c {
        approximately_negative(a - b) && approximately_negative(b - c)
    } else {
        approximately_negative(b - a) && approximately_negative(c - b)
    }
}

#[inline]
pub fn precisely_between(a: f64, b: f64, c: f64) -> bool {
    if a <= c {
        precisely_negative(a - b) && precisely_negative(b - c)
    } else {
        precisely_negative(b - a) && precisely_negative(c - b)
    }
}

/// True if (a <= b <= c) || (a >= b >= c).
#[inline]
pub fn between(a: f64, b: f64, c: f64) -> bool {
    (a - b) * (c - b) <= 0.0
}

#[inline]
pub fn roughly_equal(x: f64, y: f64) -> bool {
    (x - y).abs() < ROUGH_EPSILON
}

#[inline]
pub fn roughly_negative(x: f64) -> bool {
    x < ROUGH_EPSILON
}

#[inline]
pub fn roughly_between(a: f64, b: f64, c: f64) -> bool {
    if a <= c {
        roughly_negative(a - b) && roughly_negative(b - c)
    } else {
        roughly_negative(b - a) && roughly_negative(c - b)
    }
}

#[inline]
pub fn more_roughly_equal(x: f64, y: f64) -> bool {
    (x - y).abs() < MORE_ROUGH_EPSILON
}

#[inline]
pub fn sk_d_interp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// `SkDSign`: -1 if negative, 0 if zero, 1 if positive.
#[inline]
pub fn sk_d_sign(x: f64) -> i32 {
    (x > 0.0) as i32 - (x < 0.0) as i32
}

/// `SKDSide`: 0 if negative, 1 if zero, 2 if positive.
#[inline]
pub fn sk_d_side(x: f64) -> i32 {
    (x > 0.0) as i32 + (x >= 0.0) as i32
}

/// `SkDSideBit`: 1 if negative, 2 if zero, 4 if positive.
#[inline]
pub fn sk_d_side_bit(x: f64) -> i32 {
    1 << sk_d_side(x)
}

#[inline]
pub fn sk_pin_t(t: f64) -> f64 {
    if precisely_less_than_zero(t) {
        0.0
    } else if precisely_greater_than_one(t) {
        1.0
    } else {
        t
    }
}

/// `SK_MinS32`: -INT32_MAX (not INT32_MIN), the "not computed" winding.
pub const SK_MIN_S32: i32 = -i32::MAX;
/// `SK_NaN32`: INT32_MIN.
pub const SK_NAN32: i32 = i32::MIN;
/// `SK_MaxS32`.
pub const SK_MAX_S32: i32 = i32::MAX;
