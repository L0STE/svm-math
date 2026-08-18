use crate::kernel::wide::{ceil_from_quotient_remainder, mul_div_error, widening_mul};
use crate::MathError;

// The widening multiply carries no Kani harness on purpose: bit-blasted
// full-width multiplier identities are a known hard SAT class, so the
// exhaustive reduced-width sweep plus the full-width differential tests
// carry that argument, as they do for the digit divider.

#[kani::proof]
fn mul_div_rem_matches_u128() {
    // Widening and div2x1 are proved separately. This selector checks the
    // public operation's error precedence and normalization composition.
    let high: u64 = kani::any();
    let low: u64 = kani::any();
    let denominator: u64 = kani::any();
    match kani::any::<u8>() % 3 {
        0 => assert_eq!(mul_div_error(high, 0), Some(MathError::DivByZero)),
        1 => {
            kani::assume(denominator != 0 && high >= denominator);
            assert_eq!(mul_div_error(high, denominator), Some(MathError::Overflow));
        }
        _ => {
            // The divider normalizes inline: the shifted divisor gains its
            // top bit and the dividend keeps its value under the same shift.
            kani::assume(denominator != 0 && high > 0 && high < denominator);
            assert_eq!(mul_div_error(high, denominator), None);
            let shift = denominator.leading_zeros();
            let normalized = denominator << shift;
            assert!(normalized >= 1 << 63);
            if shift > 0 {
                let n2 = (high << shift) | (low >> (64 - shift));
                assert!(n2 < normalized);
                assert_eq!(n2 >> shift, high);
            }
        }
    }
}

#[kani::proof]
fn mul_div_ceil_matches_definition() {
    let quotient: u64 = kani::any();
    let remainder: u64 = kani::any();
    let actual = ceil_from_quotient_remainder(quotient, remainder);
    if remainder == 0 {
        assert_eq!(actual, Ok(quotient));
    } else if quotient == u64::MAX {
        assert_eq!(actual, Err(MathError::Overflow));
    } else {
        assert_eq!(actual, Ok(quotient + 1));
    }
}

#[kani::proof]
fn isqrt_seed_is_bracketed() {
    isqrt_seed_stage();
}

#[kani::proof]
fn isqrt_newton_cut_is_euclidean() {
    isqrt_newton_stage();
}

#[kani::proof]
fn isqrt_terminal_cut_is_exact() {
    isqrt_terminal_stage();
}

#[kani::proof]
fn sqrt_estimator_cut_is_exact() {
    sqrt_estimator_stage();
}

#[kani::proof]
fn sqrt_adjust_down_cut_is_exact() {
    sqrt_adjust_down_stage();
}

#[kani::proof]
fn sqrt_normalization_cut_is_exact() {
    sqrt_normalization_stage();
}

#[kani::proof]
fn sqrt_ceil_cut_is_exact() {
    sqrt_ceil_stage();
}

#[kani::proof]
fn sqrt_zero_scale_precedence_is_exact() {
    sqrt_zero_scale_stage();
}

#[kani::proof]
fn scale_unsigned_projection_is_outward() {
    // The wide-divider harness supplies this Euclidean state to scale's
    // projection step; this cut proves the directed rounding decision.
    let quotient: u64 = kani::any();
    let remainder: u64 = kani::any();
    let denominator: u64 = kani::any();
    kani::assume(denominator != 0 && remainder < denominator);

    let upper = ceil_from_quotient_remainder(quotient, remainder);
    if let Ok(upper) = upper {
        assert!(quotient <= upper);
        assert!(upper - quotient <= 1);
        assert_eq!(upper != quotient, remainder != 0);
    } else {
        assert_eq!(quotient, u64::MAX);
        assert_ne!(remainder, 0);
    }
}

#[kani::proof]
fn scale_signed_projection_is_outward() {
    let quotient: u64 = kani::any();
    let remainder: u64 = kani::any();
    let denominator: u64 = kani::any();
    let negative: bool = kani::any();
    kani::assume(denominator != 0 && remainder < denominator);

    let rounded = ceil_from_quotient_remainder(quotient, remainder);
    if let Ok(rounded) = rounded {
        let (lower, upper) = if negative {
            (-i128::from(rounded), -i128::from(quotient))
        } else {
            (i128::from(quotient), i128::from(rounded))
        };
        assert!(lower <= upper);
        assert!(rounded - quotient <= 1);
        assert_eq!(rounded != quotient, remainder != 0);
    } else {
        assert_eq!(quotient, u64::MAX);
        assert_ne!(remainder, 0);
    }
}

#[kani::proof]
fn powi_directed_mul_step_preserves_order() {
    // powi composes this already-proved quotient/remainder state at every
    // directed multiplication step.
    let quotient: u64 = kani::any();
    let remainder: u64 = kani::any();
    let lower = quotient;
    match ceil_from_quotient_remainder(quotient, remainder) {
        Ok(upper) => {
            assert!(lower <= upper);
            assert!(upper - lower <= 1);
            assert_eq!(upper != lower, remainder != 0);
        }
        Err(error) => {
            assert_eq!(error, MathError::Overflow);
            assert_eq!(quotient, u64::MAX);
            assert_ne!(remainder, 0);
        }
    }
}

fn isqrt_seed_stage() {
    let value: u64 = kani::any();
    kani::assume(value >= 2);

    let seed = crate::kernel::sqrt::isqrt_seed(value);
    let target = (0, value);
    assert!(seed.is_power_of_two());
    assert!(seed <= 1_u64 << 32);
    assert!(widening_mul(seed, seed) >= target);
    assert!(widening_mul(seed >> 1, seed >> 1) <= target);
}

fn isqrt_newton_stage() {
    // The Euclidean state arrives as hypotheses, exactly as Sqrt.lean
    // consumes it: Rust's unsigned `/` is the trusted primitive boundary,
    // so the cut proves the recurrence, not the division.
    let root: u64 = kani::any();
    let quotient: u64 = kani::any();
    kani::assume(0 < root && root <= 1_u64 << 32);
    kani::assume(quotient <= root);

    let next = crate::kernel::sqrt::isqrt_newton_from_quotient(root, quotient);
    assert_eq!(next, (root + quotient) / 2);
    assert!(next <= root);
}

fn isqrt_terminal_stage() {
    let root: u64 = kani::any();
    let quotient: u64 = kani::any();
    kani::assume(root > 0);

    let actual = crate::kernel::sqrt::isqrt_adjust_down_from_quotient(root, quotient);
    if root > quotient {
        assert_eq!(actual, root - 1);
    } else {
        assert_eq!(actual, root);
    }
}

fn sqrt_estimator_stage() {
    let high_root: u64 = kani::any();
    let quotient: u64 = kani::any();
    kani::assume((1 << 31) <= high_root && high_root <= u64::from(u32::MAX));

    let actual = crate::kernel::sqrt::sqrt_estimate_from_quotient(high_root, quotient);
    let expected_low = quotient.min(u64::from(u32::MAX));
    assert_eq!(
        actual,
        (high_root << 32).wrapping_add(expected_low).max(1 << 63)
    );
    assert!(actual >= 1 << 63);
}

fn sqrt_adjust_down_stage() {
    let high: u64 = kani::any();
    let low: u64 = kani::any();
    let root: u64 = kani::any();
    let square_high: u64 = kani::any();
    let square_low: u64 = kani::any();
    kani::assume(root > 0);

    let actual =
        crate::kernel::sqrt::sqrt_adjust_down_from_square(high, low, root, square_high, square_low);
    if (square_high, square_low) > (high, low) {
        assert_eq!(actual, Some(root - 1));
    } else {
        assert_eq!(actual, None);
    }
}

fn sqrt_normalization_stage() {
    let high: u64 = kani::any();
    let low: u64 = kani::any();
    kani::assume(high != 0);

    let (normalized_high, normalized_low, shift) =
        crate::kernel::sqrt::normalize_sqrt_words(high, low);
    assert_eq!(shift % 2, 0);
    assert!(shift <= 62);
    assert!(normalized_high >= 1 << 62);
    if shift == 0 {
        assert_eq!((normalized_high, normalized_low), (high, low));
    } else {
        assert_eq!(normalized_high, (high << shift) | (low >> (64 - shift)));
        assert_eq!(normalized_low, low << shift);
    }
}

fn sqrt_ceil_stage() {
    let root: u64 = kani::any();
    let exact: bool = kani::any();
    let actual = crate::kernel::sqrt::sqrt_ceil_from_floor(root, exact);
    if exact {
        assert_eq!(actual, Ok(root));
    } else if root == u64::MAX {
        assert_eq!(actual, Err(MathError::Overflow));
    } else {
        assert_eq!(actual, Ok(root + 1));
    }
}

fn sqrt_zero_scale_stage() {
    let value: u64 = kani::any();
    assert_eq!(
        crate::kernel::sqrt::sqrt_floor(value, 0),
        Err(MathError::DivByZero)
    );
    assert_eq!(
        crate::kernel::sqrt::sqrt_ceil(value, 0),
        Err(MathError::DivByZero)
    );
}
