use crate::kernel::wide::{
    ceil_from_quotient_remainder, div2x1_correct, div2x1_estimate_from_product, mul_div_error,
    normalize_div128, reciprocal_error_from_mullo, reciprocal_seed, reciprocal_v1_from_product,
    reciprocal_v2_from_product, reciprocal_v3_from_mulhi, reciprocal_v4_from_product, widening_mul,
};
use crate::MathError;

#[kani::proof]
fn widening_mul_u64_matches_u128() {
    let a: u64 = kani::any();
    let b: u64 = kani::any();
    let (high, low) = widening_mul(a, b);

    // Independent four-term expansion. Only 32-bit limbs are multiplied, so
    // this does not ask the solver to lower a symbolic 64-by-64 multiply.
    let a_high = a >> 32;
    let a_low = a & 0xffff_ffff;
    let b_high = b >> 32;
    let b_low = b & 0xffff_ffff;
    let low_low = u128::from(a_low) * u128::from(b_low);
    let low_high = u128::from(a_low) * u128::from(b_high);
    let high_low = u128::from(a_high) * u128::from(b_low);
    let high_high = u128::from(a_high) * u128::from(b_high);
    let four_term = (high_high << 64) + ((low_high + high_low) << 32) + low_low;
    let actual = (u128::from(high) << 64) | u128::from(low);
    assert_eq!(actual, four_term);
}

#[kani::proof]
fn reciprocal_u64_matches_definition() {
    // Div2x1.lean composes these actual word-level production cuts into the
    // reciprocal interval. A selector keeps each SAT query at one stage.
    match kani::any::<u8>() % 6 {
        0 => reciprocal_seed_stage(),
        1 => reciprocal_v1_stage(),
        2 => reciprocal_v2_stage(),
        3 => reciprocal_error_stage(),
        4 => reciprocal_v3_stage(),
        _ => reciprocal_v4_stage(),
    }
}

#[kani::proof]
fn div2x1_u64_matches_u128() {
    // Div2x1.lean composes the estimate and signed correction cases into the
    // Euclidean quotient/remainder theorem without symbolic u128 division.
    match kani::any::<u8>() % 3 {
        0 => div2x1_estimate_stage(),
        1 => div2x1_nonnegative_correction_stage(),
        _ => div2x1_negative_correction_stage(),
    }
}

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
            kani::assume(denominator != 0 && high > 0 && high < denominator);
            assert_eq!(mul_div_error(high, denominator), None);
            let (normalized_d, normalized_high, normalized_low, shift) =
                normalize_div128(high, low, denominator);
            assert!(normalized_d >= 1 << 63);
            assert!(normalized_high < normalized_d);
            if shift == 0 {
                assert_eq!(
                    (normalized_d, normalized_high, normalized_low),
                    (denominator, high, low)
                );
            } else {
                assert!(shift < 64);
                assert_eq!(normalized_d, denominator << shift);
                assert_eq!(normalized_high, (high << shift) | (low >> (64 - shift)));
                assert_eq!(normalized_low, low << shift);
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

fn div2x1_estimate_stage() {
    let high: u64 = kani::any();
    let low: u64 = kani::any();
    let denominator: u64 = kani::any();
    let product_high: u64 = kani::any();
    let product_low: u64 = kani::any();
    let (quotient_low, quotient, remainder) =
        div2x1_estimate_from_product(high, low, denominator, product_high, product_low);
    let (expected_low, carry) = product_low.overflowing_add(low);
    let quotient_base = product_high
        .wrapping_add(high)
        .wrapping_add(u64::from(carry));
    assert_eq!(quotient_low, expected_low);
    assert_eq!(quotient, quotient_base.wrapping_add(1));
    assert_eq!(
        remainder,
        low.wrapping_sub(quotient.wrapping_mul(denominator))
    );
}

fn div2x1_nonnegative_correction_stage() {
    let quotient_low: u64 = kani::any();
    let quotient: u64 = kani::any();
    let remainder: u64 = kani::any();
    let denominator: u64 = kani::any();
    kani::assume(denominator >= 1 << 63);
    kani::assume(remainder < core::cmp::max(0_u64.wrapping_sub(denominator), quotient_low));
    if remainder > quotient_low {
        kani::assume(quotient != 0);
    }
    if remainder >= denominator {
        kani::assume(quotient != u64::MAX);
    }

    let actual = div2x1_correct(quotient_low, quotient, remainder, denominator);
    if remainder >= denominator {
        assert_eq!(actual, (quotient + 1, remainder - denominator));
    } else {
        assert_eq!(actual, (quotient, remainder));
    }
}

fn div2x1_negative_correction_stage() {
    let quotient_low: u64 = kani::any();
    let quotient: u64 = kani::any();
    let magnitude: u64 = kani::any();
    let denominator: u64 = kani::any();
    kani::assume(denominator >= 1 << 63);
    kani::assume(magnitude > 0 && magnitude <= denominator);
    let remainder = 0_u64.wrapping_sub(magnitude);
    kani::assume(remainder > quotient_low);

    assert_eq!(
        div2x1_correct(quotient_low, quotient, remainder, denominator),
        (quotient.wrapping_sub(1), denominator - magnitude)
    );
}

fn reciprocal_seed_stage() {
    let top_nine: u64 = kani::any();
    let remainder: u64 = kani::any();
    kani::assume((1 << 8) <= top_nine && top_nine < (1 << 9));
    kani::assume((1..=(1 << 31)).contains(&remainder));
    let top_forty = (top_nine << 31) + remainder;
    let seed = reciprocal_seed(top_nine);
    let error = (1_i128 << 50) - i128::from(seed) * i128::from(top_forty);
    let absolute_error = if error < 0 { -error } else { error };
    assert!(absolute_error < 5 * (1_i128 << 39));
    assert!((1 << 10) <= seed && seed < (1 << 11));
}

fn reciprocal_v1_stage() {
    let seed: u64 = kani::any();
    let quotient: u64 = kani::any();
    let remainder: u64 = kani::any();
    kani::assume((1 << 10) <= seed && seed < (1 << 11));
    kani::assume(quotient < 1 << 22);
    kani::assume(remainder < 1 << 40);
    kani::assume(quotient + 1 <= seed << 11);
    let product = (quotient << 40) | remainder;
    assert_eq!(
        reciprocal_v1_from_product(seed, product),
        (seed << 11) - quotient - 1
    );
}

fn reciprocal_v2_stage() {
    let first: u64 = kani::any();
    let quotient: u64 = kani::any();
    let remainder: u64 = kani::any();
    kani::assume((1 << 20) <= first && first < (1 << 21));
    kani::assume(quotient < 1 << 17);
    kani::assume(remainder < 1 << 47);
    let product = (quotient << 47) | remainder;
    assert_eq!(
        reciprocal_v2_from_product(first, product),
        (first << 13) + quotient
    );
}

fn reciprocal_error_stage() {
    let second: u64 = kani::any();
    let low_bit: u64 = kani::any();
    let product_low: u64 = kani::any();
    kani::assume((1 << 33) <= second && second < (1 << 34));
    kani::assume(low_bit <= 1);
    assert_eq!(
        reciprocal_error_from_mullo(second, low_bit, product_low),
        ((second >> 1) & low_bit.wrapping_neg()).wrapping_sub(product_low)
    );
}

fn reciprocal_v3_stage() {
    let second: u64 = kani::any();
    let product_high: u64 = kani::any();
    kani::assume((1 << 33) <= second && second < (1 << 34));
    assert_eq!(
        reciprocal_v3_from_mulhi(second, product_high),
        (second << 31).wrapping_add(product_high >> 1)
    );
}

fn reciprocal_v4_stage() {
    let third: u64 = kani::any();
    let denominator: u64 = kani::any();
    kani::assume(denominator >= 1 << 63);

    let (product_high, product_low) = widening_mul(third, denominator);
    let (combined_high, overflow) = denominator.overflowing_add(product_high);
    kani::assume(!overflow);
    let error_low = 0_u64.wrapping_sub(product_low);
    let borrow = product_low != 0;
    let error_high = 0_u64
        .wrapping_sub(combined_high)
        .wrapping_sub(u64::from(borrow));
    kani::assume(error_high <= 1);
    let error = (u128::from(error_high) << 64) | u128::from(error_low);
    kani::assume(error > 0 && error < 2 * u128::from(denominator));

    let fourth = reciprocal_v4_from_product(third, denominator, product_high, product_low);
    if error <= u128::from(denominator) {
        assert_eq!(fourth, third);
    } else {
        assert_ne!(third, u64::MAX);
        assert_eq!(fourth, third + 1);
    }
}
