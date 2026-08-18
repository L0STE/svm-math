use crate::kernel::{
    scale::{normalize_unsigned_q63, normalize_unsigned_q63_bounds, project_signed_q, Q},
    wide::{decimal_exponent, multiply_high, widening_mul},
};
use crate::MathError;

const LOG2E_Q63_LOWER: u64 = 0xB8AA_3B29_5C17_F0BB;
const LOG2E_Q63_UPPER: u64 = 0xB8AA_3B29_5C17_F0BC;

const fn mul_q96(a: u128, b: u128) -> u128 {
    let (a_high, a_low) = (a >> 48, a & ((1_u128 << 48) - 1));
    let (b_high, b_low) = (b >> 48, b & ((1_u128 << 48) - 1));
    a_high * b_high + ((a_high * b_low) >> 48) + ((a_low * b_high) >> 48) + ((a_low * b_low) >> 96)
}

const fn log2_table_bits(node: u64, upper: bool) -> u64 {
    let delta = node as u128 - (1_u128 << 63);
    let denominator = node as u128 + (1_u128 << 63);
    let quotient = (delta << 64) / denominator;
    let remainder = (delta << 64) % denominator;
    let base = (quotient << 32) + (remainder << 32) / denominator;
    let z = base + if upper { 1 } else { 0 };
    let z2 = mul_q96(z, z) + if upper { 3 } else { 0 };
    let mut sum = 0_u128;
    let mut power = z;
    let mut index = 0_u32;
    while index < 20 {
        let term = power / (2 * index as u128 + 1);
        sum += term + if upper { 1 } else { 0 };
        power = mul_q96(power, z2) + if upper { 3 } else { 0 };
        index += 1;
    }
    if upper {
        sum += power + power / 8 + 2;
    }
    let log2e = if upper {
        LOG2E_Q63_UPPER
    } else {
        LOG2E_Q63_LOWER
    } as u128;
    let sum_q64 = (sum >> 32)
        + if upper && sum & ((1_u128 << 32) - 1) != 0 {
            1
        } else {
            0
        };
    let product = sum_q64 * log2e;
    let bits = product >> 62;
    (bits
        + if upper && product & ((1_u128 << 62) - 1) != 0 {
            1
        } else {
            0
        }) as u64
}

const fn log2_table(upper: bool) -> [u64; 2048] {
    let mut table = [0_u64; 2048];
    let mut index = 1;
    while index < 2048 {
        let node = (1_u64 << 63) | ((index as u64) << 52);
        table[index] = log2_table_bits(node, upper);
        index += 1;
    }
    table
}

const fn reciprocal_table(upper: bool) -> [u64; 2048] {
    let mut table = [0_u64; 2048];
    let mut index = 1;
    while index < 2048 {
        let node = ((1_u64 << 63) | ((index as u64) << 52)) as u128;
        let quotient = (1_u128 << 127) / node;
        let remainder = (1_u128 << 127) % node;
        table[index] = if upper && remainder != 0 {
            (quotient + 1) as u64
        } else {
            quotient as u64
        };
        index += 1;
    }
    table
}

static LOG2_LOWER: [u64; 2048] = log2_table(false);
static LOG2_UPPER_DELTA: [u8; 2048] =
    crate::kernel::upper_delta_table(log2_table(false), log2_table(true));
static RECIPROCAL_LOWER: [u64; 2048] = reciprocal_table(false);
static RECIPROCAL_UPPER_DELTA: [u8; 2048] =
    crate::kernel::upper_delta_table(reciprocal_table(false), reciprocal_table(true));

// log2(10) = 3 + log2(1.25), with the fraction bracketed by the same proved
// series generator that builds the main table: the mantissa of 1.25 is
// exactly 0xA000_0000_0000_0000, so no decimal constant is transcribed.
const LOG2_10_Q64_LOWER: u128 =
    (3_u128 << 64) + log2_table_bits(0xA000_0000_0000_0000, false) as u128;
const LOG2_10_Q64_UPPER: u128 =
    (3_u128 << 64) + log2_table_bits(0xA000_0000_0000_0000, true) as u128;

/// `k * log2(10)` in Q61 for every `u64` power of ten, bracketing: the
/// pow10 log2 fast path subtracts these instead of dividing by the scale.
static LOG2_POW10_LOWER: [i128; 20] = log2_pow10_table(false);
static LOG2_POW10_UPPER: [i128; 20] = log2_pow10_table(true);

const fn log2_pow10_table(upper: bool) -> [i128; 20] {
    let mut table = [0_i128; 20];
    let mut power = 0;
    while power < 20 {
        let q64 = power as u128
            * if upper {
                LOG2_10_Q64_UPPER
            } else {
                LOG2_10_Q64_LOWER
            };
        table[power] = (q64 >> 3) as i128 + if upper && q64 & 7 != 0 { 1 } else { 0 };
        power += 1;
    }
    table
}

#[inline(always)]
pub(crate) fn log2_lower(value: u64, scale: u64) -> Result<i64, MathError> {
    let result = log2_q::<false>(value, scale)?;
    project_signed_q::<false>(result, scale)
}

#[inline(always)]
pub(crate) fn log2_upper(value: u64, scale: u64) -> Result<i64, MathError> {
    let result = log2_q::<true>(value, scale)?;
    project_signed_q::<true>(result, scale)
}

/// Both directed logarithms in one pass; exactly `(log2_lower, log2_upper)`.
#[inline(always)]
pub(crate) fn log2_bounds(value: u64, scale: u64) -> Result<(i64, i64), MathError> {
    let (lower_q, upper_q) = log2_q_bounds(value, scale)?;
    Ok((
        project_signed_q::<false>(lower_q, scale)?,
        project_signed_q::<true>(upper_q, scale)?,
    ))
}

#[inline(always)]
pub(super) fn log2_q<const UPPER: bool>(value: u64, scale: u64) -> Result<i128, MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }
    if value == 0 {
        return Err(MathError::OutOfDomain);
    }
    if let Some(power) = decimal_exponent(scale) {
        return Ok(log2_q_pow10::<UPPER>(value, scale, power));
    }

    let integer = integer_estimate(value, scale);
    let (mantissa, normalization_carry) = normalize_unsigned_q63::<UPPER>(value, scale, integer);
    let fraction = log2_fraction_q64::<UPPER>(mantissa);
    Ok(pack_q61::<UPPER>(
        integer + normalization_carry as i32,
        fraction,
    ))
}

/// Packs an integer part and a Q64 fraction (with its carry) into the Q61
/// result, rounding the dropped three bits in the promised direction.
#[inline(always)]
fn pack_q61<const UPPER: bool>(integer: i32, (fraction, fraction_carry): (u64, bool)) -> i128 {
    let mut integer = integer + fraction_carry as i32;
    let mut fraction_q61 = (fraction >> 3) + u64::from(UPPER && fraction & 7 != 0);
    if fraction_q61 == Q {
        fraction_q61 = 0;
        integer += 1;
    }
    i128::from(integer) * i128::from(Q) + i128::from(fraction_q61)
}

/// `log2(value / 10^power)` as `log2(value) - power*log2(10)`: the value is
/// its own mantissa after one shift, so the scale division disappears and
/// only the constant subtraction pays for the decimal scale.
#[inline(always)]
fn log2_q_pow10<const UPPER: bool>(value: u64, scale: u64, power: usize) -> i128 {
    if let Some(exact) = pow10_exact(value, scale, power) {
        return exact;
    }

    let leading = value.leading_zeros();
    let mantissa = value << leading;
    let integer = 63 - leading as i32;
    let fraction = log2_fraction_q64::<UPPER>(mantissa);
    let log_value = pack_q61::<UPPER>(integer, fraction);
    // lower(log2 v) - upper(log2 10^k) keeps a lower bound and vice versa.
    let constant = if UPPER {
        LOG2_POW10_LOWER[power]
    } else {
        LOG2_POW10_UPPER[power]
    };
    log_value - constant
}

/// Both directed Q61 logarithms sharing the integer estimate and the
/// normalization division; every rounding matches the single-direction path.
#[inline(always)]
pub(super) fn log2_q_bounds(value: u64, scale: u64) -> Result<(i128, i128), MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }
    if value == 0 {
        return Err(MathError::OutOfDomain);
    }
    if let Some(power) = decimal_exponent(scale) {
        return Ok(log2_q_pow10_bounds(value, scale, power));
    }

    let integer = integer_estimate(value, scale);

    let ((mantissa_lower, carry_lower), (mantissa_upper, carry_upper)) =
        normalize_unsigned_q63_bounds(value, scale, integer);
    let lower = pack_q61::<false>(
        integer + carry_lower as i32,
        log2_fraction_q64::<false>(mantissa_lower),
    );
    let upper = pack_q61::<true>(
        integer + carry_upper as i32,
        log2_fraction_q64::<true>(mantissa_upper),
    );
    Ok((lower, upper))
}

/// Both directed pow10 logarithms with the residual reduction shared: the
/// mantissa is the same for both directions, so only the series legs differ.
#[inline(always)]
fn log2_q_pow10_bounds(value: u64, scale: u64, power: usize) -> (i128, i128) {
    if let Some(exact) = pow10_exact(value, scale, power) {
        return (exact, exact);
    }

    let leading = value.leading_zeros();
    let mantissa = value << leading;
    let integer = 63 - leading as i32;
    let (fraction_lower, fraction_upper) = log2_fraction_q64_bounds(mantissa);
    (
        pack_q61::<false>(integer, fraction_lower) - LOG2_POW10_UPPER[power],
        pack_q61::<true>(integer, fraction_upper) - LOG2_POW10_LOWER[power],
    )
}

#[inline(always)]
fn log2_fraction_q64<const UPPER: bool>(mantissa: u64) -> (u64, bool) {
    let (index, residual) = fraction_split(mantissa);
    if residual == 0 {
        return (fraction_node::<UPPER>(index), false);
    }
    let (lower, upper) = residual_legs(index, residual);
    fraction_assemble::<UPPER>(index, lower, upper)
}

/// Both directed fractions from one shared residual reduction: the mantissa
/// is direction-independent here, so the reciprocal legs are paid once.
#[inline(always)]
fn log2_fraction_q64_bounds(mantissa: u64) -> ((u64, bool), (u64, bool)) {
    let (index, residual) = fraction_split(mantissa);
    if residual == 0 {
        return (
            (fraction_node::<false>(index), false),
            (fraction_node::<true>(index), false),
        );
    }
    let (lower, upper) = residual_legs(index, residual);
    (
        fraction_assemble::<false>(index, lower, upper),
        fraction_assemble::<true>(index, lower, upper),
    )
}

/// Fraction-free cases stay exact: `value = 10^power * 2^j` in either
/// direction has the integer logarithm `+/- j`. The scale-side shift in the
/// negative branch drops nothing because `10^power` has `power` binary
/// trailing zeros and `j <= power < 20`.
#[inline(always)]
fn pow10_exact(value: u64, scale: u64, power: usize) -> Option<i128> {
    let power_bits = power as u32;
    let trailing = value.trailing_zeros();
    if trailing >= power_bits {
        let j = trailing - power_bits;
        if value >> j == scale {
            return Some(i128::from(j) * i128::from(Q));
        }
    } else {
        let j = power_bits - trailing;
        if scale >> j == value {
            return Some(-i128::from(j) * i128::from(Q));
        }
    }
    None
}

/// `floor(log2(value / scale))` from bit lengths and one comparison.
#[inline(always)]
fn integer_estimate(value: u64, scale: u64) -> i32 {
    let value_bits = 64_i32 - value.leading_zeros() as i32;
    let scale_bits = 64_i32 - scale.leading_zeros() as i32;
    let integer = value_bits - scale_bits;
    let below = if integer >= 0 {
        u128::from(value) < (u128::from(scale) << integer as u32)
    } else {
        (u128::from(value) << integer.unsigned_abs()) < u128::from(scale)
    };
    integer - i32::from(below)
}

#[inline(always)]
fn fraction_split(mantissa: u64) -> (usize, u64) {
    let index = ((mantissa << 1) >> 53) as usize;
    let node = (1_u64 << 63) | ((index as u64) << 52);
    (index, mantissa - node)
}

#[inline(always)]
fn fraction_node<const UPPER: bool>(index: usize) -> u64 {
    if UPPER {
        LOG2_LOWER[index] + u64::from(LOG2_UPPER_DELTA[index])
    } else {
        LOG2_LOWER[index]
    }
}

#[inline(always)]
fn residual_legs(index: usize, residual: u64) -> (u64, u64) {
    let (fraction_lower, fraction_upper) = if index == 0 {
        let exact = residual << 1;
        (exact, exact)
    } else {
        let (lower_high, lower_low) = widening_mul(residual, RECIPROCAL_LOWER[index]);
        let lower = (lower_high << 1) | (lower_low >> 63);
        let reciprocal_upper = RECIPROCAL_LOWER[index] + u64::from(RECIPROCAL_UPPER_DELTA[index]);
        let (upper_high, upper_low) = widening_mul(residual, reciprocal_upper);
        let upper = ((upper_high << 1) | (upper_low >> 63)) + 1;
        (lower, upper)
    };
    (fraction_lower << 11, fraction_upper << 11)
}

#[inline(always)]
fn fraction_assemble<const UPPER: bool>(index: usize, lower: u64, upper: u64) -> (u64, bool) {
    let series = if UPPER {
        let square_lower = multiply_high(lower, lower) >> 11;
        let square_upper = (multiply_high(upper, upper) >> 11) + 1;
        let cube_upper = (multiply_high(square_upper, upper) >> 11) + 1;
        let fourth_lower = multiply_high(square_lower, square_lower) >> 11;
        let fourth_upper = (multiply_high(square_upper, square_upper) >> 11) + 1;
        let fifth_upper = (multiply_high(fourth_upper, upper) >> 11) + 1;
        upper - square_lower / 2 + (cube_upper / 3 + 1) - fourth_lower / 4 + (fifth_upper / 5 + 1)
    } else {
        let square_lower = multiply_high(lower, lower) >> 11;
        let square_upper = (multiply_high(upper, upper) >> 11) + 1;
        let cube_lower = multiply_high(square_lower, lower) >> 11;
        let fourth_upper = (multiply_high(square_upper, square_upper) >> 11) + 1;
        lower - (square_upper / 2 + 1) - (fourth_upper / 4 + 1) + cube_lower / 3
    };
    let residual_log = if UPPER {
        (multiply_high(series, LOG2E_Q63_UPPER) >> 10) + 1
    } else {
        multiply_high(series, LOG2E_Q63_LOWER) >> 10
    };
    let base = if index == 0 {
        0
    } else {
        fraction_node::<UPPER>(index)
    };
    base.overflowing_add(residual_log)
}

#[cfg(test)]
mod tests {
    use super::{LOG2E_Q63_LOWER, LOG2E_Q63_UPPER};
    use crate::kernel::exp2::{LN2_Q64_LOWER, LN2_Q64_UPPER};

    /// The defining property linking the transcribed constant pairs:
    /// `ln(2) * log2(e) = 1`, so the Q64 x Q63 cross products must straddle
    /// `2^127` strictly (both factors are irrational, so equality is
    /// impossible and a transcription error on either side fails here).
    #[test]
    fn ln2_and_log2e_brackets_are_mutual_reciprocals() {
        let lower = u128::from(LN2_Q64_LOWER) * u128::from(LOG2E_Q63_LOWER);
        let upper = u128::from(LN2_Q64_UPPER) * u128::from(LOG2E_Q63_UPPER);
        assert!(lower < 1_u128 << 127);
        assert!(upper > 1_u128 << 127);
    }
}
