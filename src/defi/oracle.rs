//! Oracle price scaling.

use crate::{
    kernel::wide::{ceil_from_quotient_remainder, FixedDivisor},
    MathError,
};

/// Every exact power of ten a `u128` can hold, so exponent scaling is one
/// table load instead of a multiplication loop.
const POW10: [u128; 39] = pow10_table();

const fn pow10_table() -> [u128; 39] {
    let mut table = [1_u128; 39];
    let mut index = 1;
    while index < table.len() {
        table[index] = table[index - 1] * 10;
        index += 1;
    }
    table
}

fn to_u64(value: u128) -> Result<u64, MathError> {
    u64::try_from(value).map_err(|_| MathError::Overflow)
}

/// Splits `value * scale` into three limbs and divides them by the shared
/// prepared divisor, whose reciprocal is paid once per bounds pair.
fn scaled_quotient_word(
    value: u128,
    scale: u64,
    divisor: &mut FixedDivisor,
    round_up: bool,
) -> Result<u64, MathError> {
    let limbs = product_limbs(value, scale);
    let mut quotient = 0;
    let mut remainder = 0;
    for (index, limb) in limbs.into_iter().enumerate() {
        let (next_quotient, next_remainder) = divisor.div_rem_valid(remainder, limb);
        if index != limbs.len() - 1 && next_quotient != 0 {
            return Err(MathError::Overflow);
        }
        quotient = next_quotient;
        remainder = next_remainder;
    }
    if round_up {
        ceil_from_quotient_remainder(quotient, remainder)
    } else {
        Ok(quotient)
    }
}

/// The bit-serial fallback for denominators beyond one word (`|expo| > 19`).
fn scaled_quotient_wide(
    value: u128,
    scale: u64,
    denominator: u128,
    round_up: bool,
) -> Result<u64, MathError> {
    debug_assert!(denominator > 0);
    debug_assert!(denominator < (1_u128 << 127));

    let limbs = product_limbs(value, scale);
    let mut quotient = 0_u128;
    let mut remainder = 0_u128;
    for limb in limbs {
        for bit_index in (0..64).rev() {
            remainder = (remainder << 1) | u128::from((limb >> bit_index) & 1);
            let quotient_bit = u128::from(remainder >= denominator);
            if quotient_bit != 0 {
                remainder -= denominator;
            }
            quotient = quotient
                .checked_mul(2)
                .and_then(|q| q.checked_add(quotient_bit))
                .ok_or(MathError::Overflow)?;
        }
    }
    if round_up && remainder != 0 {
        quotient = quotient.checked_add(1).ok_or(MathError::Overflow)?;
    }
    to_u64(quotient)
}

#[inline(always)]
fn product_limbs(value: u128, scale: u64) -> [u64; 3] {
    let low_product = (value as u64 as u128) * u128::from(scale);
    let upper_product = (value >> 64) * u128::from(scale) + (low_product >> 64);
    [
        (upper_product >> 64) as u64,
        upper_product as u64,
        low_product as u64,
    ]
}

/// Returns outward integer bounds on a confidence-adjusted scaled price.
pub fn price_bounds_scaled(
    price: i64,
    confidence: u64,
    expo: i32,
    output_scale: u64,
) -> Result<(u64, u64), MathError> {
    if output_scale == 0 {
        return Err(MathError::DivByZero);
    }
    let price = u64::try_from(price).map_err(|_| MathError::OutOfDomain)?;
    let lower = price.saturating_sub(confidence);
    let upper = u128::from(price)
        .checked_add(u128::from(confidence))
        .ok_or(MathError::Overflow)?;

    if expo >= 0 {
        let factor = *POW10
            .get(usize::try_from(expo).map_err(|_| MathError::Overflow)?)
            .ok_or(MathError::Overflow)?;
        let scale_factor = factor
            .checked_mul(u128::from(output_scale))
            .ok_or(MathError::Overflow)?;
        let lower = u128::from(lower)
            .checked_mul(scale_factor)
            .ok_or(MathError::Overflow)?;
        let upper = upper.checked_mul(scale_factor).ok_or(MathError::Overflow)?;
        return Ok((to_u64(lower)?, to_u64(upper)?));
    }

    let magnitude = expo.unsigned_abs();
    if magnitude > 38 {
        return Ok((0, u64::from(upper != 0)));
    }
    let denominator = POW10[magnitude as usize];
    if let Ok(word) = u64::try_from(denominator) {
        if output_scale % word == 0 {
            let factor = output_scale / word;
            let lower = lower.checked_mul(factor).ok_or(MathError::Overflow)?;
            let upper = upper
                .checked_mul(u128::from(factor))
                .ok_or(MathError::Overflow)?;
            return Ok((lower, to_u64(upper)?));
        }
        let mut divisor = match FixedDivisor::decimal(word) {
            Some(divisor) => divisor,
            None => FixedDivisor::new(word)?,
        };
        return Ok((
            scaled_quotient_word(u128::from(lower), output_scale, &mut divisor, false)?,
            scaled_quotient_word(upper, output_scale, &mut divisor, true)?,
        ));
    }
    Ok((
        scaled_quotient_wide(u128::from(lower), output_scale, denominator, false)?,
        scaled_quotient_wide(upper, output_scale, denominator, true)?,
    ))
}
