use crate::kernel::{
    scale::{project_binary_unsigned, scaled_signed_to_q, scaled_signed_to_q_bounds, Q},
    wide::multiply_high,
};
use crate::MathError;

pub(super) const LN2_Q64_LOWER: u64 = 0xB172_17F7_D1CF_79AB;
pub(super) const LN2_Q64_UPPER: u64 = 0xB172_17F7_D1CF_79AC;

const fn isqrt_u128(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut estimate = 1_u128 << (128 - value.leading_zeros()).div_ceil(2);
    loop {
        let next = (estimate + value / estimate) / 2;
        if next >= estimate {
            return estimate;
        }
        estimate = next;
    }
}

const fn exp2_fraction_bits(upper: bool) -> [u64; 64] {
    let mut table = [0_u64; 64];
    let mut previous = 0_u64;
    let mut index = 0;
    while index < 64 {
        let square = if index == 0 {
            1_u128 << 127
        } else {
            (previous as u128) << 63
        };
        let root = isqrt_u128(square);
        table[index] = if upper && root * root != square {
            (root + 1) as u64
        } else {
            root as u64
        };
        previous = table[index];
        index += 1;
    }
    table
}

const fn const_mul_q63(a: u64, b: u64, upper: bool) -> u64 {
    let product = a as u128 * b as u128;
    let mut result = (product >> 63) as u64;
    if upper && product & ((1_u128 << 63) - 1) != 0 {
        result += 1;
    }
    result
}

const fn exp2_table(upper: bool) -> [u64; 2048] {
    let factors = exp2_fraction_bits(upper);
    let mut table = [0_u64; 2048];
    let mut index = 0;
    while index < 2048 {
        let mut q63 = 1_u64 << 63;
        let mut bit = 0;
        while bit < 11 {
            if index & (1 << bit) != 0 {
                q63 = const_mul_q63(q63, factors[10 - bit], upper);
            }
            bit += 1;
        }
        table[index] = q63;
        index += 1;
    }
    table
}

static EXP2_LOWER: [u64; 2048] = exp2_table(false);
static EXP2_UPPER_DELTA: [u8; 2048] =
    crate::kernel::upper_delta_table(exp2_table(false), exp2_table(true));

#[inline(always)]
pub(crate) fn exp2_lower(exponent: i64, scale: u64) -> Result<u64, MathError> {
    let exponent_q = scaled_signed_to_q::<false>(exponent, scale)?;
    exp2_from_q::<false>(exponent_q, scale)
}

#[inline(always)]
pub(crate) fn exp2_upper(exponent: i64, scale: u64) -> Result<u64, MathError> {
    let exponent_q = scaled_signed_to_q::<true>(exponent, scale)?;
    exp2_from_q::<true>(exponent_q, scale)
}

/// Both directed powers in one pass, sharing the scale-entry division;
/// exactly `(exp2_lower, exp2_upper)`.
#[inline(always)]
pub(crate) fn exp2_bounds(exponent: i64, scale: u64) -> Result<(u64, u64), MathError> {
    let (lower_q, upper_q) = scaled_signed_to_q_bounds(exponent, scale)?;
    Ok((
        exp2_from_q::<false>(lower_q, scale)?,
        exp2_from_q::<true>(upper_q, scale)?,
    ))
}

#[inline(always)]
pub(super) fn exp2_from_q<const UPPER: bool>(
    exponent_q: i128,
    scale: u64,
) -> Result<u64, MathError> {
    let integer = exponent_q.div_euclid(i128::from(Q));
    let fraction = exponent_q.rem_euclid(i128::from(Q)) as u64;
    if fraction == 0 {
        return project_binary_unsigned::<UPPER>(Q, integer, scale);
    }

    let fraction_q64 = fraction << 3;
    let index = (fraction_q64 >> 53) as usize;
    let residual = fraction_q64 << 11;
    let ln2 = if UPPER { LN2_Q64_UPPER } else { LN2_Q64_LOWER };
    let y = multiply_high(residual, ln2);
    let y2 = multiply_high(y, y) >> 11;
    let y3 = multiply_high(y2, y) >> 11;
    // The quartic term needs only its top bits: truncating y2 to 26 bits
    // keeps the square in one native multiply with an error below 16 units
    // of y4 — an underestimate, so the lower series stays a lower bound,
    // and 16/24 < 1 unit is covered by one extra upper slack unit.
    let y4 = ((y2 >> 26) * (y2 >> 26)) >> 23;
    let series = if UPPER {
        y + (y2 / 2 + 1) + (y3 / 6 + 1) + (y4 / 24 + 2) + 1_408
    } else {
        y + y2 / 2 + y3 / 6 + y4 / 24
    };
    let base = if UPPER {
        EXP2_LOWER[index & 2047] + u64::from(EXP2_UPPER_DELTA[index & 2047])
    } else {
        EXP2_LOWER[index & 2047]
    };
    let correction = multiply_high(base, series) >> 11;
    let (mantissa, carry0) = base.overflowing_add(correction);
    let (mantissa, carry1) = mantissa.overflowing_add(if UPPER { 2 } else { 0 });
    let (mantissa, integer) = if carry0 || carry1 {
        (
            (1_u64 << 63) + (mantissa >> 1) + u64::from(UPPER && mantissa & 1 != 0),
            integer + 1,
        )
    } else {
        (mantissa, integer)
    };
    project_binary_unsigned::<UPPER>(mantissa, integer - 2, scale)
}
