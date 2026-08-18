use crate::{
    kernel::wide::{div_rem_128_by_64, mul_div, widening_mul},
    MathError,
};

pub(super) const Q_BITS: u32 = 61;
pub(super) const Q: u64 = 1_u64 << Q_BITS;
const EXPONENT_SATURATION: i128 = 128 * Q as i128;

#[inline(always)]
pub(super) fn mul_q_unsigned<const UPPER: bool>(a: u64, b: u64) -> u128 {
    let (high, low) = widening_mul(a, b);
    let quotient = (u128::from(high) << (64 - Q_BITS)) | u128::from(low >> Q_BITS);
    quotient + u128::from(UPPER && low & (Q - 1) != 0)
}

#[inline(always)]
pub(super) fn mul_signed_q<const UPPER: bool>(a: i128, b: i128) -> i128 {
    let negative = (a < 0) ^ (b < 0);
    let a = a.unsigned_abs();
    let b = b.unsigned_abs();
    let magnitude = match (u64::try_from(a), u64::try_from(b)) {
        (Ok(a), Ok(b)) => {
            if UPPER != negative {
                mul_q_unsigned::<true>(a, b)
            } else {
                mul_q_unsigned::<false>(a, b)
            }
        }
        _ => match a.checked_mul(b) {
            Some(product) => {
                let quotient = product >> Q_BITS;
                let discarded = product & (u128::from(Q) - 1) != 0;
                quotient + u128::from(discarded && UPPER != negative)
            }
            None => {
                return if negative {
                    -EXPONENT_SATURATION
                } else {
                    EXPONENT_SATURATION
                };
            }
        },
    };
    if magnitude >= EXPONENT_SATURATION as u128 {
        return if negative {
            -EXPONENT_SATURATION
        } else {
            EXPONENT_SATURATION
        };
    }
    if negative {
        -(magnitude as i128)
    } else {
        magnitude as i128
    }
}

#[inline(always)]
pub(super) fn scaled_signed_to_q<const UPPER: bool>(
    value: i64,
    scale: u64,
) -> Result<i128, MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }

    let negative = value < 0;
    let (magnitude, discarded) = scaled_magnitude_to_q(value.unsigned_abs(), scale)?;
    let magnitude = magnitude + u128::from(UPPER != negative && discarded);

    let signed = i128::try_from(magnitude).map_err(|_| MathError::Overflow)?;
    Ok(if negative { -signed } else { signed })
}

#[inline(always)]
pub(super) fn scaled_unsigned_to_q<const UPPER: bool>(
    value: u64,
    scale: u64,
) -> Result<i128, MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }
    let (magnitude, discarded) = scaled_magnitude_to_q(value, scale)?;
    let magnitude = magnitude + u128::from(UPPER && discarded);
    i128::try_from(magnitude).map_err(|_| MathError::Overflow)
}

/// Both directed values of `value / scale` in Q61 from one division, exactly
/// as the single-direction constructors assemble them.
#[inline(always)]
pub(super) fn scaled_signed_to_q_bounds(value: i64, scale: u64) -> Result<(i128, i128), MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }
    let negative = value < 0;
    let (magnitude, discarded) = scaled_magnitude_to_q(value.unsigned_abs(), scale)?;
    let assemble = |magnitude: u128| -> Result<i128, MathError> {
        let signed = i128::try_from(magnitude).map_err(|_| MathError::Overflow)?;
        Ok(if negative { -signed } else { signed })
    };
    Ok((
        assemble(magnitude + u128::from(negative && discarded))?,
        assemble(magnitude + u128::from(!negative && discarded))?,
    ))
}

/// The unsigned twin of [`scaled_signed_to_q_bounds`].
#[inline(always)]
pub(super) fn scaled_unsigned_to_q_bounds(
    value: u64,
    scale: u64,
) -> Result<(i128, i128), MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }
    let (magnitude, discarded) = scaled_magnitude_to_q(value, scale)?;
    let lower = i128::try_from(magnitude).map_err(|_| MathError::Overflow)?;
    let upper =
        i128::try_from(magnitude + u128::from(discarded)).map_err(|_| MathError::Overflow)?;
    Ok((lower, upper))
}

#[inline(always)]
fn scaled_magnitude_to_q(value: u64, scale: u64) -> Result<(u128, bool), MathError> {
    let (whole, fraction, discarded) = if scale.is_power_of_two() {
        let scale_bits = scale.trailing_zeros();
        let whole = value >> scale_bits;
        let remainder = value & (scale - 1);
        if scale_bits <= Q_BITS {
            (whole, remainder << (Q_BITS - scale_bits), false)
        } else {
            let shift = scale_bits - Q_BITS;
            (
                whole,
                remainder >> shift,
                remainder & ((1_u64 << shift) - 1) != 0,
            )
        }
    } else {
        let whole = value / scale;
        let remainder = value % scale;
        let (fraction, discarded) = mul_div(remainder, Q, scale)?;
        (whole, fraction, discarded != 0)
    };
    Ok((
        u128::from(whole) * u128::from(Q) + u128::from(fraction),
        discarded,
    ))
}

#[inline(always)]
pub(super) fn project_binary_unsigned<const UPPER: bool>(
    mantissa: u64,
    exponent: i128,
    scale: u64,
) -> Result<u64, MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }
    if mantissa == 0 {
        return Ok(0);
    }

    let (high, low) = widening_mul(mantissa, scale);
    let shift = i128::from(Q_BITS) - exponent;
    let (quotient, discarded) = if shift <= 0 {
        let left = u32::try_from(-shift).map_err(|_| MathError::Overflow)?;
        if high != 0 || left >= 64 || low > (u64::MAX >> left) {
            return Err(MathError::Overflow);
        }
        (low << left, false)
    } else if shift < 64 {
        let right = shift as u32;
        if high >> right != 0 {
            return Err(MathError::Overflow);
        }
        (
            (high << (64 - right)) | (low >> right),
            low & ((1_u64 << right) - 1) != 0,
        )
    } else if shift == 64 {
        (high, low != 0)
    } else if shift < 128 {
        let right = (shift - 64) as u32;
        (
            high >> right,
            low != 0 || high & ((1_u64 << right) - 1) != 0,
        )
    } else {
        (0, true)
    };

    let rounded = quotient
        .checked_add(u64::from(UPPER && discarded))
        .ok_or(MathError::Overflow)?;
    Ok(rounded)
}

#[inline(always)]
pub(super) fn project_signed_q<const UPPER: bool>(
    value: i128,
    scale: u64,
) -> Result<i64, MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }

    let negative = value < 0;
    let magnitude = value.unsigned_abs();
    let whole = u64::try_from(magnitude >> Q_BITS).map_err(|_| MathError::Overflow)?;
    let fraction = magnitude as u64 & (Q - 1);
    let integer_raw = whole.checked_mul(scale).ok_or(MathError::Overflow)?;
    let (high, low) = widening_mul(fraction, scale);
    if high >> Q_BITS != 0 {
        return Err(MathError::Overflow);
    }
    let fraction_raw = (high << (64 - Q_BITS)) | (low >> Q_BITS);
    let discarded = low & (Q - 1) != 0;
    let magnitude = integer_raw
        .checked_add(fraction_raw)
        .and_then(|raw| raw.checked_add(u64::from(UPPER != negative && discarded)))
        .ok_or(MathError::Overflow)?;

    if negative {
        if magnitude > (1_u64 << 63) {
            return Err(MathError::Overflow);
        }
        if magnitude == 1_u64 << 63 {
            Ok(i64::MIN)
        } else {
            Ok(-(magnitude as i64))
        }
    } else {
        i64::try_from(magnitude).map_err(|_| MathError::Overflow)
    }
}

#[inline(always)]
pub(super) fn normalize_unsigned_q63<const UPPER: bool>(
    value: u64,
    scale: u64,
    integer: i32,
) -> (u64, bool) {
    let (quotient, remainder) = normalize_division(value, scale, integer);
    if UPPER && remainder != 0 {
        round_up_q63(quotient)
    } else {
        (quotient, false)
    }
}

/// Both directed normalizations from one division; identical assembly to
/// the single-direction calls.
#[inline(always)]
pub(super) fn normalize_unsigned_q63_bounds(
    value: u64,
    scale: u64,
    integer: i32,
) -> ((u64, bool), (u64, bool)) {
    let (quotient, remainder) = normalize_division(value, scale, integer);
    let upper = if remainder != 0 {
        round_up_q63(quotient)
    } else {
        (quotient, false)
    };
    ((quotient, false), upper)
}

#[inline(always)]
fn round_up_q63(quotient: u64) -> (u64, bool) {
    match quotient.checked_add(1) {
        Some(rounded) => (rounded, false),
        None => (1_u64 << 63, true),
    }
}

#[inline(always)]
fn normalize_division(value: u64, scale: u64, integer: i32) -> (u64, u64) {
    let shift = (63_i32 - integer) as u32;
    let (high, low) = if shift >= 64 {
        (value << (shift - 64), 0)
    } else if shift == 0 {
        (0, value)
    } else {
        (value >> (64 - shift), value << shift)
    };
    if high == 0 {
        (low / scale, low % scale)
    } else {
        div_rem_128_by_64(high, low, scale)
    }
}

#[cfg(test)]
mod tests {
    use super::{scaled_magnitude_to_q, Q};

    fn next_u64(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    #[test]
    fn decimal_scale_entry_matches_u128_definition() {
        let mut scale = 1_u64;
        let mut state = 0x5ca1_edec_1a15_f00d;
        for index in 0..20 {
            let boundaries = [0, 1, scale - 1, scale, scale.saturating_add(1), u64::MAX];
            for value in boundaries
                .into_iter()
                .chain((0..1_024).map(|_| next_u64(&mut state)))
            {
                let numerator = u128::from(value) * u128::from(Q);
                assert_eq!(
                    scaled_magnitude_to_q(value, scale),
                    Ok((
                        numerator / u128::from(scale),
                        numerator % u128::from(scale) != 0,
                    ))
                );
            }
            if index != 19 {
                scale *= 10;
            }
        }
    }
}
