use crate::{kernel::wide::widening_mul, MathError};

#[inline(always)]
pub(crate) fn isqrt_newton_from_quotient(root: u64, quotient: u64) -> u64 {
    // Kani's `isqrt_newton_stage` proves this is the exact Euclidean
    // recurrence consumed by `Sqrt.newton_excess_halves`.
    (root + quotient) >> 1
}

#[inline(always)]
pub(crate) fn isqrt_seed(value: u64) -> u64 {
    // Kani's `isqrt_seed_stage` proves the power-of-two bracket and 2^32
    // bound consumed by `Sqrt.seed_excess_below_word`.
    let root_bits = (64 - value.leading_zeros()).div_ceil(2);
    1_u64 << root_bits
}

#[inline(always)]
fn isqrt_adjust_down(value: u64, root: u64) -> u64 {
    // Rust's unsigned division is the trusted primitive boundary; the
    // branch on its quotient is the proved stage below.
    isqrt_adjust_down_from_quotient(root, value / root)
}

#[inline(always)]
pub(crate) fn isqrt_adjust_down_from_quotient(root: u64, quotient: u64) -> u64 {
    // Kani's `isqrt_terminal_stage` proves this exact branch; Lean's
    // adjacent-candidate theorem supplies why one decrement is sufficient.
    if root > quotient {
        root - 1
    } else {
        root
    }
}

#[inline]
fn isqrt_u64(value: u64) -> u64 {
    if value < 2 {
        return value;
    }

    let mut root = isqrt_seed(value);
    // The seed is in `[floor(sqrt(value)), 2 * floor(sqrt(value))]`. While it
    // is at least two above the floor, one integer Newton step halves that
    // distance. The fixed cap therefore covers every u64 input; the early
    // return avoids paying for the unused portion of that cap.
    for _ in 0..32 {
        let next = isqrt_newton_from_quotient(root, value / root);
        if next >= root {
            break;
        }
        root = next;
    }

    // Integer Newton can finish on the upper member of its terminal two-cycle.
    // The proved cap leaves `root` at the floor or one above it.
    isqrt_adjust_down(value, root)
}

#[inline(always)]
pub(crate) fn sqrt_adjust_down_from_square(
    high: u64,
    low: u64,
    root: u64,
    square_high: u64,
    square_low: u64,
) -> Option<u64> {
    // Kani's `sqrt_adjust_down_stage` proves this comparison/decrement cut;
    // `Sqrt.one_downward_correction_is_floor` composes its adjacency premise.
    if (square_high, square_low) > (high, low) {
        Some(root - 1)
    } else {
        None
    }
}

#[inline(always)]
fn sqrt_estimate_from_high_root(high: u64, low: u64, high_root: u64) -> u64 {
    // Rust's unsigned division is the trusted primitive boundary; the cap
    // and combine word logic is the proved stage below. Sqrt.lean bounds
    // the omitted q^2 and discarded low-word terms by the neighboring
    // square gaps.
    let high_remainder = high - high_root * high_root;
    let low_high = low >> 32;
    let quotient = ((high_remainder << 31) | (low_high >> 1)) / high_root;
    sqrt_estimate_from_quotient(high_root, quotient)
}

#[inline(always)]
pub(crate) fn sqrt_estimate_from_quotient(high_root: u64, quotient: u64) -> u64 {
    // Kani's `sqrt_estimator_stage` proves this cap branch and combine.
    let low_root = quotient.min((1 << 32) - 1);
    (high_root << 32).wrapping_add(low_root).max(1 << 63)
}

#[inline(always)]
fn sqrt_adjust_down(high: u64, low: u64, root: u64) -> u64 {
    let (square_high, square_low) = widening_mul(root, root);
    if let Some(next) = sqrt_adjust_down_from_square(high, low, root, square_high, square_low) {
        next
    } else {
        root
    }
}

#[inline]
fn sqrt_normalized(high: u64, low: u64) -> u64 {
    debug_assert!(high >= 1 << 62);

    let high_root = isqrt_u64(high);
    let root = sqrt_estimate_from_high_root(high, low, high_root);

    // The two-word estimator is the floor or one above it. Its omitted q^2
    // term can only cause that single-unit overshoot, so one correction is the
    // complete production path.
    sqrt_adjust_down(high, low, root)
}

#[inline(always)]
pub(crate) fn normalize_sqrt_words(high: u64, low: u64) -> (u64, u64, u32) {
    // Kani's `sqrt_normalization_stage` proves the even shift and normalized
    // high-word range used by the estimator's Lean premises.
    let shift = high.leading_zeros() & !1;
    if shift == 0 {
        (high, low, shift)
    } else {
        ((high << shift) | (low >> (64 - shift)), low << shift, shift)
    }
}

#[inline]
fn sqrt_words(high: u64, low: u64) -> (u64, bool) {
    let root = if high == 0 {
        isqrt_u64(low)
    } else {
        let (normalized_high, normalized_low, shift) = normalize_sqrt_words(high, low);
        sqrt_normalized(normalized_high, normalized_low) >> (shift / 2)
    };
    (root, widening_mul(root, root) == (high, low))
}

#[inline]
pub(crate) fn isqrt(value: u128) -> u128 {
    let high = (value >> 64) as u64;
    let low = value as u64;
    u128::from(sqrt_words(high, low).0)
}

#[inline]
fn sqrt_scaled(value: u64, scale: u64) -> Result<(u64, bool), MathError> {
    if scale == 0 {
        return Err(MathError::DivByZero);
    }
    let (high, low) = widening_mul(value, scale);
    Ok(sqrt_words(high, low))
}

#[inline]
pub(crate) fn sqrt_floor(value: u64, scale: u64) -> Result<u64, MathError> {
    sqrt_scaled(value, scale).map(|(root, _)| root)
}

#[inline(always)]
pub(crate) fn sqrt_ceil_from_floor(root: u64, exact: bool) -> Result<u64, MathError> {
    root.checked_add(u64::from(!exact))
        .ok_or(MathError::Overflow)
}

#[inline]
pub(crate) fn sqrt_ceil(value: u64, scale: u64) -> Result<u64, MathError> {
    let (root, exact) = sqrt_scaled(value, scale)?;
    sqrt_ceil_from_floor(root, exact)
}

#[cfg(test)]
mod tests {
    use super::isqrt;

    #[test]
    #[ignore = "exhaustive reduced-width release gate"]
    fn exhaustive_isqrt_u16_every_input() {
        for value in u16::MIN..=u16::MAX {
            let value = u128::from(value);
            let root = isqrt(value);
            assert!(root * root <= value);
            let successor = root + 1;
            assert!(successor * successor > value);
        }
    }
}
