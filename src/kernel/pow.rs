use crate::kernel::exp2::exp2_from_q;
use crate::kernel::log2::{log2_q, log2_q_bounds};
use crate::kernel::scale::{mul_signed_q, scaled_unsigned_to_q, scaled_unsigned_to_q_bounds};
use crate::kernel::wide::FixedDivisor;
use crate::MathError;

#[inline]
pub(crate) fn pow_lower(base: u64, exponent: u64, scale: u64) -> Result<u64, MathError> {
    pow::<false>(base, exponent, scale)
}

#[inline]
pub(crate) fn pow_upper(base: u64, exponent: u64, scale: u64) -> Result<u64, MathError> {
    pow::<true>(base, exponent, scale)
}

fn pow<const UPPER: bool>(base: u64, exponent: u64, scale: u64) -> Result<u64, MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }
    if exponent == 0 {
        return Ok(scale);
    }
    if base == 0 {
        return Ok(0);
    }
    // An exact integer exponent routes to direct binary exponentiation:
    // strictly tighter than the log2/exp2 composition and far cheaper.
    if exponent % scale == 0 {
        return powi::<UPPER>(base, exponent / scale, scale);
    }

    let logarithm = log2_q::<UPPER>(base, scale)?;
    let exponent_q = if base >= scale {
        if UPPER {
            scaled_unsigned_to_q::<true>(exponent, scale)?
        } else {
            scaled_unsigned_to_q::<false>(exponent, scale)?
        }
    } else if UPPER {
        scaled_unsigned_to_q::<false>(exponent, scale)?
    } else {
        scaled_unsigned_to_q::<true>(exponent, scale)?
    };
    let product = mul_signed_q::<UPPER>(logarithm, exponent_q);
    exp2_from_q::<UPPER>(product, scale)
}

/// Both directed powers in one pass, sharing the logarithm's normalization
/// division and the exponent conversion; exactly `(pow_lower, pow_upper)`.
pub(crate) fn pow_bounds(base: u64, exponent: u64, scale: u64) -> Result<(u64, u64), MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }
    if exponent == 0 {
        return Ok((scale, scale));
    }
    if base == 0 {
        return Ok((0, 0));
    }
    if exponent % scale == 0 {
        let whole = exponent / scale;
        return Ok((
            powi::<false>(base, whole, scale)?,
            powi::<true>(base, whole, scale)?,
        ));
    }

    let (log_lower, log_upper) = log2_q_bounds(base, scale)?;
    let (exponent_lower, exponent_upper) = scaled_unsigned_to_q_bounds(exponent, scale)?;
    // A negative logarithm swaps which exponent endpoint keeps each side
    // one-sided, exactly as the single-direction routing chooses.
    let (chosen_lower, chosen_upper) = if base >= scale {
        (exponent_lower, exponent_upper)
    } else {
        (exponent_upper, exponent_lower)
    };
    Ok((
        exp2_from_q::<false>(mul_signed_q::<false>(log_lower, chosen_lower), scale)?,
        exp2_from_q::<true>(mul_signed_q::<true>(log_upper, chosen_upper), scale)?,
    ))
}

#[inline]
pub(crate) fn powi_lower(base: u64, exponent: u64, scale: u64) -> Result<u64, MathError> {
    powi::<false>(base, exponent, scale)
}

#[inline]
pub(crate) fn powi_upper(base: u64, exponent: u64, scale: u64) -> Result<u64, MathError> {
    powi::<true>(base, exponent, scale)
}

#[inline(always)]
fn powi<const UPPER: bool>(mut base: u64, mut exponent: u64, scale: u64) -> Result<u64, MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }
    if exponent == 0 {
        return Ok(scale);
    }
    let divisor = FixedDivisor::new(scale)?;
    let mut result = scale;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = if UPPER {
                divisor.mul_div_ceil(result, base)?
            } else {
                divisor.mul_div_floor(result, base)?
            };
        }
        exponent >>= 1;
        if exponent != 0 {
            base = if UPPER {
                divisor.mul_div_ceil(base, base)?
            } else {
                divisor.mul_div_floor(base, base)?
            };
        }
    }
    Ok(result)
}
