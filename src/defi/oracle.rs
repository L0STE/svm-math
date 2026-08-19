//! Oracle price scaling.

use crate::{
    kernel::wide::{
        ceil_from_quotient_remainder, div_rem_wide_quotient, mul_div, widening_mul, FixedDivisor,
    },
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

/// A word product that must fit a word: two-word multiply, top word rejects.
fn word_product(a: u64, b: u64) -> Result<u64, MathError> {
    let (high, low) = widening_mul(a, b);
    if high != 0 {
        return Err(MathError::Overflow);
    }
    Ok(low)
}

/// `value * scale / (first * second)` where the divisor pair covers a
/// beyond-word power of ten: the nested floors divide by the product
/// exactly, in word divisions.
fn staged_quotient(
    value: u64,
    scale: u64,
    first: u64,
    second: u64,
    round_up: bool,
) -> Result<u64, MathError> {
    let (high, low) = widening_mul(value, scale);
    let (stage_high, stage_low, first_remainder) = div_rem_wide_quotient(high, low, first);
    let (quotient_high, quotient_low, second_remainder) =
        div_rem_wide_quotient(stage_high, stage_low, second);
    if quotient_high != 0 {
        return Err(MathError::Overflow);
    }
    if round_up {
        ceil_from_quotient_remainder(
            quotient_low,
            u64::from(first_remainder != 0) | second_remainder,
        )
    } else {
        Ok(quotient_low)
    }
}

/// Splits `value * scale` into three limbs and divides them by the shared
/// prepared divisor, whose reciprocal is paid once per bounds pair.
fn scaled_quotient_word(
    value: u128,
    scale: u64,
    divisor: &FixedDivisor,
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
        if upper == 0 {
            return Ok((0, 0));
        }
        // A nonzero price bound means the combined factor must itself fit
        // a word, so every product stays in native word multiplies.
        let factor = *POW10
            .get(usize::try_from(expo).map_err(|_| MathError::Overflow)?)
            .ok_or(MathError::Overflow)?;
        let scale_factor = word_product(to_u64(factor)?, output_scale)?;
        return Ok((
            word_product(lower, scale_factor)?,
            word_product(to_u64(upper)?, scale_factor)?,
        ));
    }

    let magnitude = expo.unsigned_abs();
    if magnitude > 38 {
        return Ok((0, u64::from(upper != 0)));
    }
    let denominator = POW10[magnitude as usize];
    if let Ok(word) = u64::try_from(denominator) {
        if output_scale % word == 0 {
            let factor = output_scale / word;
            let lower = word_product(lower, factor)?;
            let upper = word_product(to_u64(upper)?, factor)?;
            return Ok((lower, upper));
        }
        let (lower_quotient, _) = mul_div(lower, output_scale, word)?;
        let upper = match u64::try_from(upper) {
            Ok(upper) => {
                let (quotient, remainder) = mul_div(upper, output_scale, word)?;
                ceil_from_quotient_remainder(quotient, remainder)?
            }
            // Only a confidence-inflated bound beyond one word still needs
            // the three-limb pipeline.
            Err(_) => {
                let divisor = FixedDivisor::new(word)?;
                scaled_quotient_word(upper, output_scale, &divisor, true)?
            }
        };
        return Ok((lower_quotient, upper));
    }
    // A beyond-word power of ten splits into two word factors, so the
    // staged divider covers every word-sized price; the bit-serial walk
    // survives only for a bound beyond one word.
    let second = to_u64(denominator / POW10[19])?;
    let lower = staged_quotient(lower, output_scale, to_u64(POW10[19])?, second, false)?;
    let upper = match u64::try_from(upper) {
        Ok(upper) => staged_quotient(upper, output_scale, to_u64(POW10[19])?, second, true)?,
        Err(_) => scaled_quotient_wide(upper, output_scale, denominator, true)?,
    };
    Ok((lower, upper))
}
