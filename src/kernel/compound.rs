use crate::kernel::exp2::exp2_from_q;
use crate::kernel::scale::{mul_q_unsigned, mul_signed_q, project_binary_unsigned, Q, Q_BITS};
use crate::kernel::wide::{div_rem_wide_quotient, widening_mul};
use crate::MathError;

const INV_LN2_LOWER: u64 = 3_326_628_274_461_080_622;
const INV_LN2_UPPER: u64 = 3_326_628_274_461_080_623;

#[inline]
pub(crate) fn compound_lower(
    annual_rate: u64,
    periods_per_year: u64,
    elapsed_periods: u64,
    scale: u64,
) -> Result<u64, MathError> {
    compound::<false>(annual_rate, periods_per_year, elapsed_periods, scale)
}

#[inline]
pub(crate) fn compound_upper(
    annual_rate: u64,
    periods_per_year: u64,
    elapsed_periods: u64,
    scale: u64,
) -> Result<u64, MathError> {
    compound::<true>(annual_rate, periods_per_year, elapsed_periods, scale)
}

/// Both directed compoundings in one pass, sharing the per-period rate
/// division and the ln(1+x) series; exactly `(compound_lower, compound_upper)`.
pub(crate) fn compound_bounds(
    annual_rate: u64,
    periods_per_year: u64,
    elapsed_periods: u64,
    scale: u64,
) -> Result<(u64, u64), MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }
    if periods_per_year == 0 {
        return Err(MathError::OutOfDomain);
    }
    if elapsed_periods == 0 || annual_rate == 0 {
        return Ok((scale, scale));
    }
    if beyond_series_domain(annual_rate, periods_per_year, scale) {
        return compound_binary_bounds(annual_rate, periods_per_year, elapsed_periods, scale);
    }

    // In the series domain the staged quotient is below 2^53, so its high
    // word is zero and the ceiling endpoint cannot wrap.
    let (x_high, x_lower, inexact) = rate_q_stages(annual_rate, periods_per_year, scale);
    debug_assert!(x_high == 0);
    let x_upper = x_lower + u64::from(inexact);
    let (ln_lower, ln_upper) = ln1p_bounds(x_lower, x_upper)?;
    let log2_lower = mul_q_unsigned::<false>(ln_lower, INV_LN2_LOWER) as u64;
    let log2_upper = mul_q_unsigned::<true>(ln_upper, INV_LN2_UPPER) as u64;
    let elapsed_q = i128::from(elapsed_periods) * i128::from(Q);
    Ok((
        exp2_from_q::<false>(
            mul_signed_q::<false>(i128::from(log2_lower), elapsed_q),
            scale,
        )?,
        exp2_from_q::<true>(
            mul_signed_q::<true>(i128::from(log2_upper), elapsed_q),
            scale,
        )?,
    ))
}

fn compound<const UPPER: bool>(
    annual_rate: u64,
    periods_per_year: u64,
    elapsed_periods: u64,
    scale: u64,
) -> Result<u64, MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }
    if periods_per_year == 0 {
        return Err(MathError::OutOfDomain);
    }
    if elapsed_periods == 0 || annual_rate == 0 {
        return Ok(scale);
    }
    if beyond_series_domain(annual_rate, periods_per_year, scale) {
        // Beyond the series domain the per-period rate is at least 2^-8, so
        // the period count that still fits the result in `u64 / scale` is
        // small and direct binary squaring is both exact enough and cheap.
        return compound_binary::<UPPER>(annual_rate, periods_per_year, elapsed_periods, scale);
    }

    // In the series domain the staged quotient is below 2^53, so its high
    // word is zero and the ceiling endpoint cannot wrap.
    let (x_high, x_lower, inexact) = rate_q_stages(annual_rate, periods_per_year, scale);
    debug_assert!(x_high == 0);
    let x_upper = x_lower + u64::from(inexact);
    let (ln_lower, ln_upper) = ln1p_bounds(x_lower, x_upper)?;
    let log2_lower = mul_q_unsigned::<false>(ln_lower, INV_LN2_LOWER) as u64;
    let log2_upper = mul_q_unsigned::<true>(ln_upper, INV_LN2_UPPER) as u64;
    let elapsed_q = i128::from(elapsed_periods) * i128::from(Q);
    let exponent_q = if UPPER {
        mul_signed_q::<true>(i128::from(log2_upper), elapsed_q)
    } else {
        mul_signed_q::<false>(i128::from(log2_lower), elapsed_q)
    };
    exp2_from_q::<UPPER>(exponent_q, scale)
}

/// A normalized binary value `mantissa / 2^63 * 2^exponent` with
/// `mantissa` in `[2^63, 2^64)`. The exponent saturates instead of
/// wrapping; a saturated value projects to `Overflow` or an outward 0/1
/// bound, so saturation never weakens a certificate.
#[derive(Clone, Copy)]
struct Normalized {
    mantissa: u64,
    exponent: i64,
}

/// `annual_rate * 256 > periods_per_year * scale`, in words: the product
/// stays as a two-word pair, so no 128-bit multiply is synthesized.
#[inline(always)]
fn beyond_series_domain(annual_rate: u64, periods_per_year: u64, scale: u64) -> bool {
    let (product_high, product_low) = widening_mul(periods_per_year, scale);
    (annual_rate >> 56, annual_rate << 8) > (product_high, product_low)
}

/// Directed `annual_rate * 2^61 / (periods_per_year * scale)` through two
/// word divisions instead of a synthesized 128-bit one: nested floors
/// divide by a product exactly, and the division is inexact when either
/// stage leaves a remainder.
#[inline(always)]
fn rate_q_stages(annual_rate: u64, periods_per_year: u64, scale: u64) -> (u64, u64, bool) {
    let (stage_high, stage_low, first_remainder) =
        div_rem_wide_quotient(annual_rate >> 3, annual_rate << 61, periods_per_year);
    let (quotient_high, quotient_low, second_remainder) =
        div_rem_wide_quotient(stage_high, stage_low, scale);
    (
        quotient_high,
        quotient_low,
        first_remainder != 0 || second_remainder != 0,
    )
}

/// `(1 + annual_rate / denominator)^elapsed_periods` by directed binary
/// squaring, for the large-per-period-rate regime the series rejects.
///
/// The base is seeded once at Q61 precision — never quantized to `scale`
/// units, whose one-unit rounding would be amplified `elapsed_periods`
/// times — then squared with one directed rounding of `2^-63` per step.
fn compound_binary<const UPPER: bool>(
    annual_rate: u64,
    periods_per_year: u64,
    elapsed_periods: u64,
    scale: u64,
) -> Result<u64, MathError> {
    let (quotient, inexact) = binary_rate_q(annual_rate, periods_per_year, scale);
    let rate_q = quotient + u128::from(UPPER && inexact);
    let base = normalize_q61::<UPPER>((1_u128 << 61) + rate_q);
    binary_power::<UPPER>(base, elapsed_periods, scale)
}

/// Both directed binary-path results sharing the seed division; exactly the
/// pair of single-direction calls.
fn compound_binary_bounds(
    annual_rate: u64,
    periods_per_year: u64,
    elapsed_periods: u64,
    scale: u64,
) -> Result<(u64, u64), MathError> {
    let (quotient, inexact) = binary_rate_q(annual_rate, periods_per_year, scale);
    let base_lower = normalize_q61::<false>((1_u128 << 61) + quotient);
    let base_upper = normalize_q61::<true>((1_u128 << 61) + quotient + u128::from(inexact));
    Ok((
        binary_power::<false>(base_lower, elapsed_periods, scale)?,
        binary_power::<true>(base_upper, elapsed_periods, scale)?,
    ))
}

/// rate_q = directed(annual_rate * 2^61 / (periods_per_year * scale)),
/// entirely in word divisions through the nested-floor stages.
#[inline(always)]
fn binary_rate_q(annual_rate: u64, periods_per_year: u64, scale: u64) -> (u128, bool) {
    let (high, low, inexact) = rate_q_stages(annual_rate, periods_per_year, scale);
    ((u128::from(high) << 64) | u128::from(low), inexact)
}

/// `(base/scale)^exponent` through the same directed binary squaring as
/// the beyond-domain compound path: one Q61 seed division, one directed
/// `2^-63` rounding per step, one projection division — instead of a
/// full 128-by-64 scale division on every squaring step.
pub(crate) fn powi_binary<const UPPER: bool>(
    base: u64,
    exponent: u64,
    scale: u64,
) -> Result<u64, MathError> {
    debug_assert!(scale != 0 && exponent != 0);
    let (high, low, remainder) = div_rem_wide_quotient(base >> 3, base << 61, scale);
    let quotient = (u128::from(high) << 64) | u128::from(low);
    let base_q = quotient + u128::from(UPPER && remainder != 0);
    if base_q == 0 {
        // base/scale < 2^-61 rounded down (or base is zero): every
        // positive power of the seed floors to zero at any scale.
        return Ok(0);
    }
    binary_power::<UPPER>(normalize_q61::<UPPER>(base_q), exponent, scale)
}

#[inline(always)]
fn binary_power<const UPPER: bool>(
    mut base: Normalized,
    mut exponent: u64,
    scale: u64,
) -> Result<u64, MathError> {
    let mut result = Normalized {
        mantissa: 1 << 63,
        exponent: 0,
    };
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = mul_normalized::<UPPER>(result, base);
        }
        exponent >>= 1;
        if exponent != 0 {
            base = mul_normalized::<UPPER>(base, base);
        }
    }
    project_normalized::<UPPER>(result, scale)
}

/// Packs a positive Q61 fixed-point value into [`Normalized`], rounding in
/// the promised direction when more than 64 significant bits survive.
#[inline(always)]
fn normalize_q61<const UPPER: bool>(value_q: u128) -> Normalized {
    debug_assert!(value_q > 0);
    let bits = 128 - value_q.leading_zeros() as i64;
    if bits <= 64 {
        return Normalized {
            mantissa: (value_q as u64) << (64 - bits) as u32,
            exponent: bits - 62,
        };
    }
    let shift = (bits - 64) as u32;
    let mantissa = (value_q >> shift) as u64;
    let inexact = value_q & ((1_u128 << shift) - 1) != 0;
    if UPPER && inexact {
        match mantissa.checked_add(1) {
            Some(rounded) => Normalized {
                mantissa: rounded,
                exponent: bits - 62,
            },
            None => Normalized {
                mantissa: 1 << 63,
                exponent: bits - 61,
            },
        }
    } else {
        Normalized {
            mantissa,
            exponent: bits - 62,
        }
    }
}

/// One directed multiply: full 128-bit product, renormalize by one
/// compare, round the dropped word in the promised direction.
#[inline(always)]
fn mul_normalized<const UPPER: bool>(lhs: Normalized, rhs: Normalized) -> Normalized {
    let (high, low) = widening_mul(lhs.mantissa, rhs.mantissa);
    let exponent = lhs.exponent.saturating_add(rhs.exponent);
    let (mantissa, inexact, carry) = if high >= 1 << 63 {
        (high, low != 0, 1)
    } else {
        ((high << 1) | (low >> 63), low & ((1_u64 << 63) - 1) != 0, 0)
    };
    let (mantissa, carry) = if UPPER && inexact {
        match mantissa.checked_add(1) {
            Some(rounded) => (rounded, carry),
            None => (1 << 63, carry + 1),
        }
    } else {
        (mantissa, carry)
    };
    Normalized {
        mantissa,
        exponent: exponent.saturating_add(carry),
    }
}

/// Projects a [`Normalized`] value outward through the shared binary
/// projection: `mantissa * 2^(exponent - 63)` is `mantissa * 2^(E - Q_BITS)`
/// with `E = exponent + Q_BITS - 63`.
#[inline(always)]
fn project_normalized<const UPPER: bool>(value: Normalized, scale: u64) -> Result<u64, MathError> {
    let exponent = i128::from(value.exponent) + i128::from(Q_BITS) - 63;
    project_binary_unsigned::<UPPER>(value.mantissa, exponent, scale)
}

fn ln1p_bounds(x_lower: u64, x_upper: u64) -> Result<(u64, u64), MathError> {
    // For 0 <= x <= 1/256, the alternating series remainder has the sign of
    // its first omitted term. S_6 is a lower bound and S_5 is an upper bound.
    // Added and subtracted terms accumulate in separate u64 sums (every term
    // is below x <= Q/256, so six of them cannot overflow), and one
    // saturating subtraction at the end replaces per-term wide arithmetic.
    // Saturation is the same sharpening as before: dependency between the
    // two rounded copies of x can push the mechanical lower polynomial a few
    // Q-units negative, and the mathematical range ln(1+x) >= 0 clamps it.
    let mut power_lower = x_lower;
    let mut power_upper = x_upper;
    let mut lower_added = 0_u64;
    let mut lower_subtracted = 0_u64;
    let mut upper_added = 0_u64;
    let mut upper_subtracted = 0_u64;
    for degree in 1..=6_u64 {
        if degree & 1 == 1 {
            lower_added += div_small::<false>(power_lower, degree);
            if degree <= 5 {
                upper_added += div_small::<true>(power_upper, degree);
            }
        } else {
            lower_subtracted += div_small::<true>(power_upper, degree);
            if degree <= 5 {
                upper_subtracted += div_small::<false>(power_lower, degree);
            }
        }
        power_lower = mul_q_unsigned::<false>(power_lower, x_lower) as u64;
        power_upper = mul_q_unsigned::<true>(power_upper, x_upper) as u64;
    }
    Ok((
        lower_added.saturating_sub(lower_subtracted),
        upper_added.saturating_sub(upper_subtracted),
    ))
}

#[inline]
fn div_small<const UPPER: bool>(value: u64, denominator: u64) -> u64 {
    value / denominator + u64::from(UPPER && value % denominator != 0)
}

#[cfg(test)]
mod tests {
    use super::{INV_LN2_LOWER, INV_LN2_UPPER};
    use crate::kernel::exp2::{LN2_Q64_LOWER, LN2_Q64_UPPER};

    /// The defining property of the transcribed reciprocal pair:
    /// `ln(2) * (1 / ln(2)) = 1`, so the Q64 x Q61 cross products must
    /// straddle `2^125` strictly.
    #[test]
    fn inv_ln2_brackets_are_reciprocal_to_ln2() {
        let lower = u128::from(LN2_Q64_LOWER) * u128::from(INV_LN2_LOWER);
        let upper = u128::from(LN2_Q64_UPPER) * u128::from(INV_LN2_UPPER);
        assert!(lower < 1_u128 << 125);
        assert!(upper > 1_u128 << 125);
    }
}
