//! Skia's fixed-point types: `SkFixed` (16.16) and `SkFDot6` (26.6), from
//! `include/private/base/SkFixed.h`, `src/core/SkFDot6.h` and `SkMath.h`.
//! Wrapping arithmetic, as C++'s signed overflow does on every platform
//! Skia ships on.

pub type Fixed = i32;
pub type FDot6 = i32;

pub const FIXED1: Fixed = 1 << 16;
pub const FIXED_HALF: Fixed = 1 << 15;
pub const MAX_S32: i32 = i32::MAX;
pub const MIN_S32: i32 = -i32::MAX;

#[inline]
pub fn left_shift(value: i32, shift: i32) -> i32 {
    ((value as u32) << shift) as i32
}

#[inline]
pub fn fixed_mul(a: Fixed, b: Fixed) -> Fixed {
    ((a as i64 * b as i64) >> 16) as i32
}

#[inline]
pub fn fixed_div(numer: i32, denom: i32) -> Fixed {
    let v = ((numer as i64) << 16) / denom as i64;
    v.clamp(MIN_S32 as i64, MAX_S32 as i64) as i32
}

#[inline]
pub fn fixed_round_to_int(x: Fixed) -> i32 {
    x.wrapping_add(FIXED_HALF) >> 16
}

#[inline]
pub fn fixed_ceil_to_int(x: Fixed) -> i32 {
    x.wrapping_add(FIXED1 - 1) >> 16
}

#[inline]
pub fn fixed_floor_to_int(x: Fixed) -> i32 {
    x >> 16
}

#[inline]
pub fn fixed_round_to_fixed(x: Fixed) -> Fixed {
    (x.wrapping_add(FIXED_HALF) as u32 & 0xFFFF_0000) as i32
}

#[inline]
pub fn fixed_ceil_to_fixed(x: Fixed) -> Fixed {
    (x.wrapping_add(FIXED1 - 1) as u32 & 0xFFFF_0000) as i32
}

#[inline]
pub fn fixed_floor_to_fixed(x: Fixed) -> Fixed {
    (x as u32 & 0xFFFF_0000) as i32
}

#[inline]
pub fn int_to_fixed(x: i32) -> Fixed {
    left_shift(x, 16)
}

#[inline]
pub fn fixed_to_fdot6(x: Fixed) -> FDot6 {
    x >> 10
}

#[inline]
pub fn fdot6_to_fixed(x: FDot6) -> Fixed {
    left_shift(x, 10)
}

#[inline]
pub fn fdot6_round(x: FDot6) -> i32 {
    x.wrapping_add(32) >> 6
}

/// `SkScalarToFDot6`: `(SkFDot6)(x * 64)`, truncating.
#[inline]
pub fn scalar_to_fdot6(x: f32) -> FDot6 {
    (x * 64.0) as i32
}

#[inline]
pub fn fdot6_div(a: FDot6, b: FDot6) -> Fixed {
    if a as i16 as i32 == a {
        left_shift(a, 16) / b
    } else {
        fixed_div(a, b)
    }
}

/// `SkCLZ`.
#[inline]
pub fn clz(x: u32) -> i32 {
    x.leading_zeros() as i32
}
