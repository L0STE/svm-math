use crate::MathError;

const RECIPROCAL_TABLE: [u16; 256] = reciprocal_table();

const fn reciprocal_table() -> [u16; 256] {
    let mut table = [0_u16; 256];
    let mut index = 0;
    while index < table.len() {
        table[index] = (((1_u32 << 19) - 3 * (1 << 8)) / (256 + index as u32)) as u16;
        index += 1;
    }
    table
}

#[inline(always)]
pub(crate) const fn widening_mul(a: u64, b: u64) -> (u64, u64) {
    let (a_high, a_low) = (a >> 32, a & 0xffff_ffff);
    let (b_high, b_low) = (b >> 32, b & 0xffff_ffff);
    let low_low = a_low * b_low;
    let low_high = a_low * b_high;
    let high_low = a_high * b_low;
    let high_high = a_high * b_high;
    let middle = (low_low >> 32) + (low_high & 0xffff_ffff) + (high_low & 0xffff_ffff);
    let low = (middle << 32) | (low_low & 0xffff_ffff);
    let high = high_high + (low_high >> 32) + (high_low >> 32) + (middle >> 32);
    (high, low)
}

#[inline(always)]
pub(crate) const fn multiply_high(a: u64, b: u64) -> u64 {
    widening_mul(a, b).0
}

#[inline(always)]
pub(crate) const fn reciprocal_seed(top_nine: u64) -> u64 {
    RECIPROCAL_TABLE[(top_nine - 256) as usize] as u64
}

#[inline(always)]
pub(crate) const fn reciprocal_v1_from_product(seed: u64, product: u64) -> u64 {
    (seed << 11) - (product >> 40) - 1
}

#[inline(always)]
pub(crate) const fn reciprocal_v2_from_product(first: u64, product: u64) -> u64 {
    (first << 13) + (product >> 47)
}

#[inline(always)]
pub(crate) const fn reciprocal_error_from_mullo(
    second: u64,
    low_bit: u64,
    product_low: u64,
) -> u64 {
    ((second >> 1) & low_bit.wrapping_neg()).wrapping_sub(product_low)
}

#[inline(always)]
pub(crate) const fn reciprocal_v3_from_mulhi(second: u64, product_high: u64) -> u64 {
    (second << 31).wrapping_add(product_high >> 1)
}

#[inline(always)]
pub(crate) const fn reciprocal_v4_from_product(
    third: u64,
    normalized_denominator: u64,
    product_high: u64,
    product_low: u64,
) -> u64 {
    let carry = product_low.overflowing_add(normalized_denominator).1;
    third
        .wrapping_sub(product_high.wrapping_add(carry as u64))
        .wrapping_sub(normalized_denominator)
}

#[inline(always)]
pub(crate) const fn reciprocal(normalized_denominator: u64) -> u64 {
    debug_assert!(normalized_denominator >= 1 << 63);

    let low_bit = normalized_denominator & 1;
    let top_nine = normalized_denominator >> 55;
    let top_forty = (normalized_denominator >> 24) + 1;
    let rounded_half = (normalized_denominator >> 1) + low_bit;

    let seed = reciprocal_seed(top_nine);
    let first = reciprocal_v1_from_product(seed, seed * seed * top_forty);
    let second = reciprocal_v2_from_product(first, first * ((1_u64 << 60) - first * top_forty));
    let error = reciprocal_error_from_mullo(second, low_bit, second.wrapping_mul(rounded_half));
    let third = reciprocal_v3_from_mulhi(second, multiply_high(second, error));
    let (product_high, product_low) = widening_mul(third, normalized_denominator);
    reciprocal_v4_from_product(third, normalized_denominator, product_high, product_low)
}

#[inline(always)]
pub(crate) fn div2x1_estimate_from_product(
    high: u64,
    low: u64,
    denominator: u64,
    product_high: u64,
    product_low: u64,
) -> (u64, u64, u64) {
    let (quotient_low, carry) = product_low.overflowing_add(low);
    let quotient = product_high
        .wrapping_add(high)
        .wrapping_add(u64::from(carry))
        .wrapping_add(1);
    let remainder = low.wrapping_sub(quotient.wrapping_mul(denominator));
    (quotient_low, quotient, remainder)
}

#[inline(always)]
pub(crate) fn div2x1_correct(
    quotient_low: u64,
    mut quotient: u64,
    mut remainder: u64,
    denominator: u64,
) -> (u64, u64) {
    if remainder > quotient_low {
        quotient = quotient.wrapping_sub(1);
        remainder = remainder.wrapping_add(denominator);
    }
    if remainder >= denominator {
        quotient = quotient.wrapping_add(1);
        remainder -= denominator;
    }
    (quotient, remainder)
}

#[inline(always)]
pub(crate) fn divide_normalized(
    high: u64,
    low: u64,
    denominator: u64,
    reciprocal: u64,
) -> (u64, u64) {
    debug_assert!(denominator >= 1 << 63 && high < denominator);

    let (product_high, product_low) = widening_mul(reciprocal, high);
    let (quotient_low, quotient, remainder) =
        div2x1_estimate_from_product(high, low, denominator, product_high, product_low);
    div2x1_correct(quotient_low, quotient, remainder, denominator)
}

#[inline(always)]
const fn normalize_denominator(denominator: u64) -> (u64, u32) {
    let shift = denominator.leading_zeros();
    (denominator << shift, shift)
}

#[inline(always)]
pub(crate) fn normalize_div128(high: u64, low: u64, denominator: u64) -> (u64, u64, u64, u32) {
    let (normalized_denominator, shift) = normalize_denominator(denominator);
    if shift == 0 {
        (normalized_denominator, high, low, shift)
    } else {
        (
            normalized_denominator,
            (high << shift) | (low >> (64 - shift)),
            low << shift,
            shift,
        )
    }
}

#[inline(always)]
pub(crate) fn div_rem_128_by_64(high: u64, low: u64, denominator: u64) -> (u64, u64) {
    debug_assert!(denominator != 0 && high < denominator);
    if high == 0 {
        return (low / denominator, low % denominator);
    }

    let (normalized_denominator, normalized_high, normalized_low, shift) =
        normalize_div128(high, low, denominator);
    let reciprocal = reciprocal(normalized_denominator);
    let (quotient, remainder) = divide_normalized(
        normalized_high,
        normalized_low,
        normalized_denominator,
        reciprocal,
    );
    (quotient, remainder >> shift)
}

#[inline(always)]
pub(crate) fn mul_div_error(high: u64, denominator: u64) -> Option<MathError> {
    if denominator == 0 {
        Some(MathError::DivByZero)
    } else if high >= denominator {
        Some(MathError::Overflow)
    } else {
        None
    }
}

#[inline(always)]
pub(crate) fn ceil_from_quotient_remainder(
    quotient: u64,
    remainder: u64,
) -> Result<u64, MathError> {
    quotient
        .checked_add(u64::from(remainder != 0))
        .ok_or(MathError::Overflow)
}

#[inline(always)]
fn mul_div_narrow(a: u64, b: u64, denominator: u64) -> Option<Result<(u64, u64), MathError>> {
    if a <= u64::from(u32::MAX) && b <= u64::from(u32::MAX) {
        let product = a * b;
        return Some(Ok((product / denominator, product % denominator)));
    }

    let (large, small) = if a >= b { (a, b) } else { (b, a) };
    if small < denominator && denominator <= u64::from(u32::MAX) {
        let remainder_product = (large % denominator) * small;
        return Some(Ok((
            (large / denominator) * small + remainder_product / denominator,
            remainder_product % denominator,
        )));
    }

    if small <= u64::from(u32::MAX) && denominator <= u64::from(u32::MAX) {
        let whole = match (large / denominator).checked_mul(small) {
            Some(whole) => whole,
            None => return Some(Err(MathError::Overflow)),
        };
        let remainder_product = (large % denominator) * small;
        let quotient = match whole.checked_add(remainder_product / denominator) {
            Some(quotient) => quotient,
            None => return Some(Err(MathError::Overflow)),
        };
        return Some(Ok((quotient, remainder_product % denominator)));
    }

    a.checked_mul(b)
        .map(|product| Ok((product / denominator, product % denominator)))
}

pub(crate) struct FixedDivisor {
    denominator: u64,
    normalized_denominator: u64,
    reciprocal: u64,
    shift: u32,
}

impl FixedDivisor {
    #[inline(always)]
    pub(crate) fn new(denominator: u64) -> Result<Self, MathError> {
        if denominator == 0 {
            return Err(MathError::DivByZero);
        }
        Ok(Self {
            denominator,
            normalized_denominator: 0,
            reciprocal: 0,
            shift: 0,
        })
    }

    #[inline(always)]
    pub(crate) fn div_rem(&mut self, high: u64, low: u64) -> Result<(u64, u64), MathError> {
        if high >= self.denominator {
            return Err(MathError::Overflow);
        }
        Ok(self.div_rem_valid(high, low))
    }

    #[inline(never)]
    pub(crate) fn div_rem_valid(&mut self, high: u64, low: u64) -> (u64, u64) {
        debug_assert!(high < self.denominator);
        if high == 0 {
            return (low / self.denominator, low % self.denominator);
        }

        if self.normalized_denominator == 0 {
            (self.normalized_denominator, self.shift) = normalize_denominator(self.denominator);
        }
        let (normalized_high, normalized_low) = if self.shift == 0 {
            (high, low)
        } else {
            (
                (high << self.shift) | (low >> (64 - self.shift)),
                low << self.shift,
            )
        };
        if self.reciprocal == 0 {
            self.reciprocal = reciprocal(self.normalized_denominator);
        }
        let (quotient, remainder) = divide_normalized(
            normalized_high,
            normalized_low,
            self.normalized_denominator,
            self.reciprocal,
        );
        (quotient, remainder >> self.shift)
    }

    #[inline(never)]
    pub(crate) fn decimal(denominator: u64) -> Option<Self> {
        let index = decimal_exponent(denominator)?;
        let (normalized_denominator, shift) = normalize_denominator(denominator);
        Some(Self {
            denominator,
            normalized_denominator,
            reciprocal: DECIMAL_RECIPROCALS[index].reciprocal,
            shift,
        })
    }

    #[inline(always)]
    pub(crate) fn mul_div(&mut self, a: u64, b: u64) -> Result<(u64, u64), MathError> {
        if let Some(result) = mul_div_narrow(a, b, self.denominator) {
            return result;
        }
        let (high, low) = widening_mul(a, b);
        self.div_rem(high, low)
    }

    #[inline(always)]
    pub(crate) fn mul_div_floor(&mut self, a: u64, b: u64) -> Result<u64, MathError> {
        self.mul_div(a, b).map(|(quotient, _)| quotient)
    }

    #[inline(always)]
    pub(crate) fn mul_div_ceil(&mut self, a: u64, b: u64) -> Result<u64, MathError> {
        if let Some(quotient) = ceil_narrow(a, b, self.denominator) {
            return Ok(quotient);
        }
        let (quotient, remainder) = self.mul_div(a, b)?;
        ceil_from_quotient_remainder(quotient, remainder)
    }
}

#[inline(never)]
pub(crate) fn decimal_mul_div(
    a: u64,
    b: u64,
    denominator: u64,
) -> Option<Result<(u64, u64), MathError>> {
    FixedDivisor::decimal(denominator).map(|mut divisor| divisor.mul_div(a, b))
}

#[inline(never)]
pub(crate) fn decimal_div_rem_valid(high: u64, low: u64, denominator: u64) -> Option<(u64, u64)> {
    FixedDivisor::decimal(denominator).map(|mut divisor| divisor.div_rem_valid(high, low))
}

/// The decimal exponent `k` when `value == 10^k`: `10^k = 2^k * 5^k` has
/// exactly `k` binary trailing zeros, so the exponent doubles as the table
/// index and one equality check settles membership.
#[inline(always)]
pub(crate) fn decimal_exponent(value: u64) -> Option<usize> {
    let index = value.trailing_zeros() as usize;
    if index < DECIMAL_RECIPROCALS.len() && DECIMAL_RECIPROCALS[index].denominator == value {
        Some(index)
    } else {
        None
    }
}

#[derive(Clone, Copy)]
struct DecimalReciprocal {
    denominator: u64,
    reciprocal: u64,
}

const DECIMAL_RECIPROCALS: [DecimalReciprocal; 20] = decimal_reciprocals();

const fn decimal_reciprocals() -> [DecimalReciprocal; 20] {
    let mut reciprocals = [DecimalReciprocal {
        denominator: 1,
        reciprocal: reciprocal(1_u64 << 63),
    }; 20];
    let mut denominator = 1_u64;
    let mut index = 0;
    while index < reciprocals.len() {
        let (normalized_denominator, _) = normalize_denominator(denominator);
        reciprocals[index] = DecimalReciprocal {
            denominator,
            reciprocal: reciprocal(normalized_denominator),
        };
        if index + 1 < reciprocals.len() {
            denominator *= 10;
        }
        index += 1;
    }
    reciprocals
}

#[inline(always)]
pub(crate) fn mul_div(a: u64, b: u64, denominator: u64) -> Result<(u64, u64), MathError> {
    if denominator == 0 {
        return Err(MathError::DivByZero);
    }
    if let Some(result) = mul_div_narrow(a, b, denominator) {
        return result;
    }

    let (high, low) = widening_mul(a, b);
    if let Some(error) = mul_div_error(high, denominator) {
        return Err(error);
    }
    Ok(div_rem_128_by_64(high, low, denominator))
}

#[inline(always)]
pub(crate) fn mul_div_floor(a: u64, b: u64, denominator: u64) -> Result<u64, MathError> {
    mul_div(a, b, denominator).map(|(quotient, _)| quotient)
}

#[inline(always)]
pub(crate) fn mul_div_ceil(a: u64, b: u64, denominator: u64) -> Result<u64, MathError> {
    if denominator == 0 {
        return Err(MathError::DivByZero);
    }
    if let Some(quotient) = ceil_narrow(a, b, denominator) {
        return Ok(quotient);
    }
    let (quotient, remainder) = mul_div(a, b, denominator)?;
    ceil_from_quotient_remainder(quotient, remainder)
}

/// Direct ceiling for narrow operands: the exact product fits one word, so
/// `div_ceil` needs neither the quotient/remainder plumbing nor an overflow
/// branch — a nonzero divisor makes `ceil(product / d) <= product`.
#[inline(always)]
fn ceil_narrow(a: u64, b: u64, denominator: u64) -> Option<u64> {
    debug_assert!(denominator != 0);
    if a <= u64::from(u32::MAX) && b <= u64::from(u32::MAX) {
        Some((a * b).div_ceil(denominator))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        div_rem_128_by_64, mul_div, normalize_denominator, reciprocal, widening_mul, FixedDivisor,
    };

    fn next_u64(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    #[test]
    fn decimal_divisors_are_const_generated_and_exact() {
        let mut denominator = 1_u64;
        let mut state = 0xdec1_a1d0_5eed_f00d;
        for index in 0..20 {
            let mut divisor = FixedDivisor::decimal(denominator).unwrap();
            let (normalized, shift) = normalize_denominator(denominator);
            assert_eq!(divisor.denominator, denominator);
            assert_eq!(divisor.normalized_denominator, normalized);
            assert_eq!(divisor.reciprocal, reciprocal(normalized));
            assert_eq!(divisor.shift, shift);

            for (high, low) in [
                (0, 0),
                (0, u64::MAX),
                (denominator - 1, 0),
                (denominator - 1, u64::MAX),
            ] {
                let numerator = (u128::from(high) << 64) | u128::from(low);
                assert_eq!(
                    divisor.div_rem_valid(high, low),
                    (
                        (numerator / u128::from(denominator)) as u64,
                        (numerator % u128::from(denominator)) as u64,
                    )
                );
            }
            for _ in 0..1_024 {
                let high = next_u64(&mut state) % denominator;
                let low = next_u64(&mut state);
                let numerator = (u128::from(high) << 64) | u128::from(low);
                assert_eq!(
                    divisor.div_rem_valid(high, low),
                    (
                        (numerator / u128::from(denominator)) as u64,
                        (numerator % u128::from(denominator)) as u64,
                    )
                );
            }

            if index != 19 {
                denominator *= 10;
            }
        }

        for denominator in [3, 11, 999, u64::from(u32::MAX), u64::MAX] {
            assert!(FixedDivisor::decimal(denominator).is_none());
        }
    }

    #[test]
    fn fixed_divisor_matches_independent_calls() {
        assert!(FixedDivisor::new(0).is_err());
        for denominator in [1, u64::from(u32::MAX), 1_u64 << 63, u64::MAX] {
            let mut divisor = FixedDivisor::new(denominator).unwrap();
            for (a, b) in [
                (0, u64::MAX),
                (u64::from(u32::MAX) + 1, u64::from(u32::MAX)),
                (u64::MAX - 1, u64::MAX - 1),
                (u64::MAX, u64::MAX),
            ] {
                assert_eq!(divisor.mul_div(a, b), mul_div(a, b, denominator));
            }
        }
    }

    #[test]
    #[ignore = "exhaustive reduced-width release gate"]
    fn exhaustive_widening_mul_u8_every_pair() {
        for a in u8::MIN..=u8::MAX {
            for b in u8::MIN..=u8::MAX {
                let actual = widening_mul(u64::from(a), u64::from(b));
                let product = u128::from(a) * u128::from(b);
                assert_eq!(actual, ((product >> 64) as u64, product as u64));
            }
        }
    }

    #[test]
    #[ignore = "exhaustive reduced-width release gate"]
    fn exhaustive_div_rem_u8_every_valid_input() {
        for denominator in 1_u64..=u64::from(u8::MAX) {
            for high in 0..denominator {
                for low in 0_u64..=u64::from(u8::MAX) {
                    let numerator = (u128::from(high) << 64) | u128::from(low);
                    assert_eq!(
                        div_rem_128_by_64(high, low, denominator),
                        (
                            (numerator / u128::from(denominator)) as u64,
                            (numerator % u128::from(denominator)) as u64,
                        )
                    );
                }
            }
        }
    }
}
