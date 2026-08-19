use crate::MathError;

#[inline(always)]
pub(crate) const fn widening_mul(a: u64, b: u64) -> (u64, u64) {
    let (a_high, a_low) = (a >> 32, a & 0xffff_ffff);
    let (b_high, b_low) = (b >> 32, b & 0xffff_ffff);
    let low_low = a_low * b_low;
    let low_high = a_low * b_high;
    let high_low = a_high * b_low;
    let high_high = a_high * b_high;
    let middle = (low_low >> 32) + (low_high & 0xffff_ffff) + (high_low & 0xffff_ffff);
    // The native multiply already returns the exact low word; the partial
    // sums above survive only to produce the carry into the high word.
    let low = a.wrapping_mul(b);
    let high = high_high + (low_high >> 32) + (high_low >> 32) + (middle >> 32);
    (high, low)
}

/// The top word of `a·b` from three partial products instead of four:
/// dropping the low-by-low partial loses its carry into the top word and
/// truncating each cross product loses its masked half's carry, so the
/// result is `multiply_high(a, b) - e` with `e` in `{0, 1, 2}` — never
/// above the exact word. Series terms whose slack absorbs two downward
/// units use this; anything needing the exact word or the low word's
/// bits (overflow checks, directed rounding, dividend words) does not.
#[inline(always)]
pub(crate) const fn multiply_high_approx(a: u64, b: u64) -> u64 {
    let (a_high, a_low) = (a >> 32, a & 0xffff_ffff);
    let (b_high, b_low) = (b >> 32, b & 0xffff_ffff);
    a_high * b_high + ((a_high * b_low) >> 32) + ((a_low * b_high) >> 32)
}

// Dedicated squaring forms (three multiplies exact, two approximate)
// were evaluated and declined: with equal operands the compiler already
// merges the identical cross partials, so the explicit forms measured as
// pure layout churn.
#[inline(always)]
const fn normalize_denominator(denominator: u64) -> (u64, u32) {
    let shift = denominator.leading_zeros();
    (denominator << shift, shift)
}

#[inline(always)]
pub(crate) fn div_rem_128_by_64(high: u64, low: u64, denominator: u64) -> (u64, u64) {
    debug_assert!(denominator != 0 && high < denominator);
    if high == 0 {
        return (low / denominator, low % denominator);
    }

    // A 128-by-32 schoolbook path for word-half divisors (two native
    // divisions, no normalization) was evaluated and declined: through
    // `mul_div` the narrow schoolbook branches already own every
    // one-wide-operand small-divisor product, and the power-of-two and
    // power-of-ten scale fast paths keep the transcendental entries out
    // of this divider, so the branch measured as pure dispatch tax.
    // Knuth Algorithm D with 32-bit digits: on SBF a native division costs
    // about as much as any instruction, so two digit divisions with bounded
    // fixups beat reciprocal machinery built for targets where division is
    // expensive. `leading_zeros` lowers to a ~20-instruction software
    // sequence here, so an already-normalized divisor skips it outright.
    let (shift, normalized, n2, n10) = if denominator >> 63 != 0 {
        (0, denominator, high, low)
    } else {
        let shift = denominator.leading_zeros();
        (
            shift,
            denominator << shift,
            (high << shift) | (low >> (64 - shift)),
            low << shift,
        )
    };
    let (q1, r1) = divide_digit(n2, n10 >> 32, normalized);
    let (q0, r0) = divide_digit(r1, n10 & 0xffff_ffff, normalized);
    ((q1 << 32) | q0, r0 >> shift)
}

/// One 32-bit quotient digit: a native division by the divisor's top digit
/// estimates within two of the true digit (Knuth, TAOCP 4.3.1 Theorem B for
/// a normalized divisor), and each loop pass fixes one overestimate. The
/// exit when `rest` reaches `2^32` is sound because the compare can then
/// never fire again. Returns the digit and the exact partial remainder.
#[inline(always)]
fn divide_digit(u_high: u64, u_low: u64, normalized: u64) -> (u64, u64) {
    let top = normalized >> 32;
    let bottom = normalized & 0xffff_ffff;
    let mut digit = u_high / top;
    let mut rest = u_high % top;
    while digit >= 1 << 32 || digit * bottom > ((rest << 32) | u_low) {
        digit -= 1;
        rest += top;
        if rest >= 1 << 32 {
            break;
        }
    }
    // The true partial remainder is below the divisor, so it survives the
    // wrapping subtraction of the two-word product intact.
    let remainder = ((u_high << 32) | u_low).wrapping_sub(digit.wrapping_mul(normalized));
    (digit, remainder)
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
    let a_high = a >> 32;
    let b_high = b >> 32;
    if a_high | b_high == 0 {
        let product = a * b;
        return Some(Ok((product / denominator, product % denominator)));
    }

    // Two wide operands defeat both remaining branches (each needs one
    // operand inside a word half), so big-by-big products exit at once.
    if a_high != 0 && b_high != 0 {
        return None;
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

    // One wide operand with a denominator above a word half: `checked_mul`
    // here lowers to an overflow-checked multiply builtin call, and on
    // overflow the wide route multiplies again. Declining outright is
    // faster on both sides: the wide route's zero-high path already
    // divides a fitting product natively (measured: -66 on the wide
    // utilization row against +3 on the big-by-big row).
    None
}

pub(crate) struct FixedDivisor {
    denominator: u64,
    normalized: u64,
    shift: u32,
}

impl FixedDivisor {
    #[inline(always)]
    pub(crate) fn new(denominator: u64) -> Result<Self, MathError> {
        if denominator == 0 {
            return Err(MathError::DivByZero);
        }
        let (normalized, shift) = normalize_denominator(denominator);
        Ok(Self {
            denominator,
            normalized,
            shift,
        })
    }

    #[inline(always)]
    pub(crate) fn div_rem(&self, high: u64, low: u64) -> Result<(u64, u64), MathError> {
        if high >= self.denominator {
            return Err(MathError::Overflow);
        }
        Ok(self.div_rem_valid(high, low))
    }

    #[inline(never)]
    pub(crate) fn div_rem_valid(&self, high: u64, low: u64) -> (u64, u64) {
        debug_assert!(high < self.denominator);
        if high == 0 {
            return (low / self.denominator, low % self.denominator);
        }
        let (n2, n10) = if self.shift == 0 {
            (high, low)
        } else {
            (
                (high << self.shift) | (low >> (64 - self.shift)),
                low << self.shift,
            )
        };
        let (q1, r1) = divide_digit(n2, n10 >> 32, self.normalized);
        let (q0, r0) = divide_digit(r1, n10 & 0xffff_ffff, self.normalized);
        ((q1 << 32) | q0, r0 >> self.shift)
    }

    #[inline(always)]
    pub(crate) fn mul_div(&self, a: u64, b: u64) -> Result<(u64, u64), MathError> {
        if let Some(result) = mul_div_narrow(a, b, self.denominator) {
            return result;
        }
        let (high, low) = widening_mul(a, b);
        self.div_rem(high, low)
    }

    #[inline(always)]
    pub(crate) fn mul_div_floor(&self, a: u64, b: u64) -> Result<u64, MathError> {
        self.mul_div(a, b).map(|(quotient, _)| quotient)
    }

    #[inline(always)]
    pub(crate) fn mul_div_ceil(&self, a: u64, b: u64) -> Result<u64, MathError> {
        if let Some(quotient) = ceil_narrow(a, b, self.denominator) {
            return Ok(quotient);
        }
        let (quotient, remainder) = self.mul_div(a, b)?;
        ceil_from_quotient_remainder(quotient, remainder)
    }
}

/// The decimal exponent `k` when `value == 10^k`: `10^k = 2^k * 5^k` has
/// exactly `k` binary trailing zeros, so the exponent doubles as the table
/// index and one equality check settles membership.
#[inline(always)]
pub(crate) fn decimal_exponent(value: u64) -> Option<usize> {
    let index = value.trailing_zeros() as usize;
    if index < POW10.len() && POW10[index] == value {
        Some(index)
    } else {
        None
    }
}

/// Every power of ten a `u64` can hold.
const POW10: [u64; 20] = pow10_table();

const fn pow10_table() -> [u64; 20] {
    let mut table = [1_u64; 20];
    let mut index = 1;
    while index < table.len() {
        table[index] = table[index - 1] * 10;
        index += 1;
    }
    table
}

#[inline(always)]
pub(crate) fn mul_div(a: u64, b: u64, denominator: u64) -> Result<(u64, u64), MathError> {
    if denominator == 0 {
        return Err(MathError::DivByZero);
    }
    if let Some(result) = mul_div_narrow(a, b, denominator) {
        return result;
    }
    // The pair widening stays inline; the native u128 product lowers to a
    // compiler-builtin call with a stack-passed result on SBF (measured in
    // the disassembly), which costs more than the four partial products.
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
    use super::{div_rem_128_by_64, mul_div, widening_mul, FixedDivisor};

    fn next_u64(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    #[test]
    fn fixed_divisor_matches_independent_calls() {
        assert!(FixedDivisor::new(0).is_err());
        for denominator in [1, u64::from(u32::MAX), 1_u64 << 63, u64::MAX] {
            let divisor = FixedDivisor::new(denominator).unwrap();
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
    fn approximate_top_word_is_within_two_below_exact() {
        let mut state = 0x6d68_6170_7072_6f78;
        for _ in 0..1_000_000 {
            let a = next_u64(&mut state);
            let b = next_u64(&mut state);
            let exact = super::widening_mul(a, b).0;
            let approx = super::multiply_high_approx(a, b);
            assert!(exact - approx <= 2, "a={a:#x} b={b:#x}");
        }
        for a in [0, 1, u64::MAX, 1 << 32, (1 << 32) - 1, u64::MAX << 32] {
            for b in [0, 1, u64::MAX, 1 << 32, (1 << 32) - 1, u64::MAX << 32] {
                let exact = super::widening_mul(a, b).0;
                let approx = super::multiply_high_approx(a, b);
                assert!(exact - approx <= 2, "a={a:#x} b={b:#x}");
            }
        }
    }

    #[test]
    fn knuth_divider_matches_reciprocal_and_u128() {
        let mut state = 0x6b6e_7574_685f_6421;
        for index in 0..1_000_000_u64 {
            let a = next_u64(&mut state);
            let b = next_u64(&mut state);
            let raw = next_u64(&mut state);
            // Sweep divisor widths so every shift and digit shape appears.
            let denominator = (raw >> (index % 64)).max(1);
            let numerator = u128::from(a) * u128::from(b);
            let high = (numerator >> 64) as u64;
            let low = numerator as u64;
            if high >= denominator {
                continue;
            }
            let expected = (
                (numerator / u128::from(denominator)) as u64,
                (numerator % u128::from(denominator)) as u64,
            );
            assert_eq!(
                super::div_rem_128_by_64(high, low, denominator),
                expected,
                "a={a:#x} b={b:#x} d={denominator:#x}"
            );
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
