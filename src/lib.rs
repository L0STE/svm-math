//! Primitive, dependency-free mathematics for SVM programs.
//!
//! # Exact arithmetic laws
//!
//! For `d > 0`:
//!
//! - `mul_div_floor(a, b, d) = floor(a * b / d)`.
//! - `mul_div_ceil(a, b, d) = ceil(a * b / d)`.
//!
//! For every `n`, `isqrt(n)^2 <= n < (isqrt(n) + 1)^2`, where the
//! right-hand comparison is understood over the mathematical integers.
//!
//! For `scale > 0`:
//!
//! - `sqrt_floor(value, scale) = floor(sqrt(value * scale))`.
//! - `sqrt_ceil(value, scale) = ceil(sqrt(value * scale))`.
//!
//! Certified transcendental functions return one-sided fixed-point bounds:
//! `lower / scale <= truth <= upper / scale`.

#![no_std]
#![deny(unsafe_code)]
#![deny(missing_docs)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_doctests {}

mod error;
mod kernel;
#[cfg(kani)]
mod proofs;

pub use error::MathError;

/// Stateless recipes built from the primitive math kernels.
pub mod defi;

/// Returns `floor(a * b / denominator)` exactly.
///
/// The multiplication is evaluated at full `u128` width. A zero denominator
/// returns [`MathError::DivByZero`], and a quotient wider than `u64` returns
/// [`MathError::Overflow`].
#[inline]
pub fn mul_div_floor(a: u64, b: u64, denominator: u64) -> Result<u64, MathError> {
    kernel::wide::mul_div_floor(a, b, denominator)
}

/// Returns `ceil(a * b / denominator)` exactly.
///
/// The multiplication is evaluated at full `u128` width. A zero denominator
/// returns [`MathError::DivByZero`], and a result wider than `u64` returns
/// [`MathError::Overflow`].
#[inline]
pub fn mul_div_ceil(a: u64, b: u64, denominator: u64) -> Result<u64, MathError> {
    kernel::wide::mul_div_ceil(a, b, denominator)
}

/// Returns the exact integer floor of `sqrt(value)`.
///
/// The result `r` satisfies `r^2 <= value < (r + 1)^2` over the mathematical
/// integers.
#[inline]
pub fn isqrt(value: u128) -> u128 {
    kernel::sqrt::isqrt(value)
}

/// Returns `floor(sqrt(value * scale))` exactly.
///
/// `scale` is the number of raw units per mathematical unit. A zero scale
/// returns [`MathError::DivByZero`].
#[inline]
pub fn sqrt_floor(value: u64, scale: u64) -> Result<u64, MathError> {
    kernel::sqrt::sqrt_floor(value, scale)
}

/// Returns `ceil(sqrt(value * scale))` exactly.
///
/// `scale` is the number of raw units per mathematical unit. A zero scale
/// returns [`MathError::DivByZero`].
#[inline]
pub fn sqrt_ceil(value: u64, scale: u64) -> Result<u64, MathError> {
    kernel::sqrt::sqrt_ceil(value, scale)
}

/// Returns a lower fixed-point bound for `2^(exponent / scale)`.
#[inline]
pub fn exp2_lower(exponent: i64, scale: u64) -> Result<u64, MathError> {
    kernel::exp2::exp2_lower(exponent, scale)
}

/// Returns an upper fixed-point bound for `2^(exponent / scale)`.
#[inline]
pub fn exp2_upper(exponent: i64, scale: u64) -> Result<u64, MathError> {
    kernel::exp2::exp2_upper(exponent, scale)
}

/// Returns both fixed-point bounds of `2^(exponent / scale)` in one pass.
///
/// The pair is exactly (`exp2_lower`, `exp2_upper`); the shared entry work
/// makes it cheaper than the two calls.
#[inline]
pub fn exp2_bounds(exponent: i64, scale: u64) -> Result<(u64, u64), MathError> {
    kernel::exp2::exp2_bounds(exponent, scale)
}

/// Returns a lower fixed-point bound for `log2(value / scale)`.
#[inline]
pub fn log2_lower(value: u64, scale: u64) -> Result<i64, MathError> {
    kernel::log2::log2_lower(value, scale)
}

/// Returns an upper fixed-point bound for `log2(value / scale)`.
#[inline]
pub fn log2_upper(value: u64, scale: u64) -> Result<i64, MathError> {
    kernel::log2::log2_upper(value, scale)
}

/// Returns both fixed-point bounds of `log2(value / scale)` in one pass.
///
/// The pair is exactly (`log2_lower`, `log2_upper`); the shared
/// normalization makes it cheaper than the two calls.
#[inline]
pub fn log2_bounds(value: u64, scale: u64) -> Result<(i64, i64), MathError> {
    kernel::log2::log2_bounds(value, scale)
}

/// Returns a lower fixed-point bound for `(base / scale)^(exponent / scale)`.
#[inline]
pub fn pow_lower(base: u64, exponent: u64, scale: u64) -> Result<u64, MathError> {
    kernel::pow::pow_lower(base, exponent, scale)
}

/// Returns an upper fixed-point bound for `(base / scale)^(exponent / scale)`.
#[inline]
pub fn pow_upper(base: u64, exponent: u64, scale: u64) -> Result<u64, MathError> {
    kernel::pow::pow_upper(base, exponent, scale)
}

/// Returns both fixed-point bounds of `(base / scale)^(exponent / scale)`
/// in one pass.
///
/// The pair is exactly (`pow_lower`, `pow_upper`); the shared logarithm
/// normalization makes it cheaper than the two calls.
#[inline]
pub fn pow_bounds(base: u64, exponent: u64, scale: u64) -> Result<(u64, u64), MathError> {
    kernel::pow::pow_bounds(base, exponent, scale)
}

/// Returns a lower fixed-point bound for `(base / scale)^exponent`.
#[inline]
pub fn powi_lower(base: u64, exponent: u64, scale: u64) -> Result<u64, MathError> {
    kernel::pow::powi_lower(base, exponent, scale)
}

/// Returns an upper fixed-point bound for `(base / scale)^exponent`.
#[inline]
pub fn powi_upper(base: u64, exponent: u64, scale: u64) -> Result<u64, MathError> {
    kernel::pow::powi_upper(base, exponent, scale)
}

/// Returns a lower bound for periodic compounding.
#[inline]
pub fn compound_lower(
    annual_rate: u64,
    periods_per_year: u64,
    elapsed_periods: u64,
    scale: u64,
) -> Result<u64, MathError> {
    kernel::compound::compound_lower(annual_rate, periods_per_year, elapsed_periods, scale)
}

/// Returns an upper bound for periodic compounding.
#[inline]
pub fn compound_upper(
    annual_rate: u64,
    periods_per_year: u64,
    elapsed_periods: u64,
    scale: u64,
) -> Result<u64, MathError> {
    kernel::compound::compound_upper(annual_rate, periods_per_year, elapsed_periods, scale)
}

/// Returns both bounds for periodic compounding in one pass.
///
/// The pair is exactly (`compound_lower`, `compound_upper`); the shared
/// rate division and series make it cheaper than the two calls.
#[inline]
pub fn compound_bounds(
    annual_rate: u64,
    periods_per_year: u64,
    elapsed_periods: u64,
    scale: u64,
) -> Result<(u64, u64), MathError> {
    kernel::compound::compound_bounds(annual_rate, periods_per_year, elapsed_periods, scale)
}
