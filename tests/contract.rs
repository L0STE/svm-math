//! The public contract, enumerated: every function's guard matrix — each
//! documented precondition violation produces exactly its error, in
//! precedence order — plus the economic guarantees the formula-level tests
//! do not already pin. Property sweeps run over a stated modeled domain of
//! realistic base-unit magnitudes; full-range overflow behavior is pinned
//! by the guard matrix and the kernel proofs.

use svm_math::{defi, MathError};

/// Realistic base-unit magnitudes for property sweeps: one quintillion,
/// a generous upper bound for token supplies at common decimal scales.
const MODELED_DOMAIN: u64 = 1_000_000_000_000_000_000;
const SWEEP: u64 = 2_048;

fn next_u64(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

#[test]
fn exact_arithmetic_guard_matrix() {
    assert_eq!(svm_math::mul_div_floor(1, 1, 0), Err(MathError::DivByZero));
    assert_eq!(svm_math::mul_div_ceil(1, 1, 0), Err(MathError::DivByZero));
    assert_eq!(
        svm_math::mul_div_floor(u64::MAX, u64::MAX, 1),
        Err(MathError::Overflow)
    );
    assert_eq!(
        svm_math::mul_div_ceil(u64::MAX, 2, 1),
        Err(MathError::Overflow)
    );
    // isqrt is total; sqrt_* reject only a zero scale. A ceil overflow is
    // unreachable: the largest product (u64::MAX)^2 has an exact root.
    assert_eq!(svm_math::sqrt_floor(1, 0), Err(MathError::DivByZero));
    assert_eq!(svm_math::sqrt_ceil(1, 0), Err(MathError::DivByZero));
    assert_eq!(svm_math::sqrt_ceil(u64::MAX, u64::MAX), Ok(u64::MAX));
}

#[test]
fn transcendental_guard_matrix() {
    // A zero scale outranks every other failure.
    assert_eq!(svm_math::exp2_lower(i64::MIN, 0), Err(MathError::DivByZero));
    assert_eq!(
        svm_math::exp2_bounds(i64::MAX, 0),
        Err(MathError::DivByZero)
    );
    assert_eq!(svm_math::log2_lower(0, 0), Err(MathError::DivByZero));
    assert_eq!(svm_math::log2_bounds(0, 0), Err(MathError::DivByZero));
    assert_eq!(svm_math::pow_lower(0, 0, 0), Err(MathError::DivByZero));
    assert_eq!(svm_math::pow_bounds(0, 0, 0), Err(MathError::DivByZero));
    assert_eq!(svm_math::powi_upper(0, 0, 0), Err(MathError::DivByZero));
    assert_eq!(
        svm_math::compound_lower(1, 0, 1, 0),
        Err(MathError::DivByZero)
    );
    // Domain violations after the scale gate.
    assert_eq!(svm_math::log2_lower(0, 7), Err(MathError::OutOfDomain));
    assert_eq!(svm_math::log2_bounds(0, 7), Err(MathError::OutOfDomain));
    assert_eq!(
        svm_math::compound_bounds(1, 0, 1, 7),
        Err(MathError::OutOfDomain)
    );
    // Representability failures are overflow.
    assert_eq!(svm_math::exp2_lower(64, 1), Err(MathError::Overflow));
    assert_eq!(
        svm_math::powi_lower(u64::MAX, 2, 1),
        Err(MathError::Overflow)
    );
}

#[test]
fn recipe_guard_matrix() {
    use defi::{amm, fee, lending, oracle, schedule, staking};

    assert_eq!(fee::net_of_fee(1, 10_001), Err(MathError::OutOfDomain));
    assert_eq!(amm::quote_exact_in(0, 1, 1, 0), Err(MathError::OutOfDomain));
    assert_eq!(amm::quote_exact_in(1, 0, 1, 0), Err(MathError::OutOfDomain));
    assert_eq!(
        amm::quote_exact_out(0, 2, 1, 0),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        amm::quote_exact_out(1, 2, 2, 0),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        amm::quote_exact_out(1, 2, 1, 10_000),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        amm::quote_exact_out(u64::MAX, 3, 1, 0),
        Err(MathError::Overflow)
    );
    assert_eq!(lending::utilization_bps(1, 0), Err(MathError::DivByZero));
    assert_eq!(lending::utilization_bps(0, 0), Ok(0));
    assert_eq!(lending::utilization_bps(2, 1), Err(MathError::OutOfDomain));
    assert_eq!(
        lending::borrow_rate_bps(10_001, 0, 0, 0, 5_000),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        lending::borrow_rate_bps(0, 0, 0, 0, 0),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        lending::borrow_rate_bps(0, 0, 0, 0, 10_000),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        schedule::vested_floor(1, 0, 0, 0, 0),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        schedule::linear_interp_floor(0, 1, 0, 0),
        Err(MathError::DivByZero)
    );
    assert_eq!(
        schedule::linear_interp_ceil(1, 0, 0, 0),
        Err(MathError::DivByZero)
    );
    assert_eq!(
        staking::reward_index_accrue_lower(0, 1, 1, 0),
        Err(MathError::DivByZero)
    );
    assert_eq!(
        staking::reward_index_accrue_upper(0, 1, 1, 0),
        Err(MathError::DivByZero)
    );
    assert_eq!(
        staking::reward_index_accrue(0, 0, 1, 1, 0),
        Err(MathError::DivByZero)
    );
    assert_eq!(
        staking::rewards_owed_floor(1, 1, 0, 0),
        Err(MathError::DivByZero)
    );
    assert_eq!(
        oracle::price_bounds_scaled(-1, 0, 0, 1),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        oracle::price_bounds_scaled(1, 0, 0, 0),
        Err(MathError::DivByZero)
    );
}

#[test]
fn quotes_never_drain_the_pool() {
    let mut state = 0x5155_4f54_455f_4f4b;
    for _ in 0..SWEEP {
        let reserve_in = 1 + next_u64(&mut state) % MODELED_DOMAIN;
        let reserve_out = 1 + next_u64(&mut state) % MODELED_DOMAIN;
        let amount_in = next_u64(&mut state) % MODELED_DOMAIN;
        let fee_bps = (next_u64(&mut state) % 10_001) as u16;
        let out = defi::amm::quote_exact_in(reserve_in, reserve_out, amount_in, fee_bps).unwrap();
        assert!(out <= reserve_out, "output exceeds the pool");
        if amount_in == 0 {
            assert_eq!(out, 0, "a zero input bought output");
        }
    }
    assert_eq!(defi::amm::quote_exact_in(1, 1, 0, 0), Ok(0));
}

#[test]
fn borrow_rate_is_floored_by_base_and_capped_by_the_slopes() {
    let mut state = 0x4c45_4e44_5f4f_4b21;
    for _ in 0..SWEEP {
        let utilization = next_u64(&mut state) % 10_001;
        let base = next_u64(&mut state) % 10_000;
        let before = next_u64(&mut state) % 10_000;
        let after = next_u64(&mut state) % 10_000;
        let kink = 1 + next_u64(&mut state) % 9_999;
        let rate = defi::lending::borrow_rate_bps(utilization, base, before, after, kink).unwrap();
        assert!(rate >= base, "rate undercuts base");
        assert!(
            rate <= base + before + after,
            "rate exceeds the slope budget"
        );
    }
}

#[test]
fn vesting_and_interpolation_stay_clamped() {
    let mut state = 0x5645_5354_5f4f_4b21;
    for _ in 0..SWEEP {
        let total = next_u64(&mut state) % MODELED_DOMAIN;
        let start = next_u64(&mut state) % 1_000_000;
        let duration = 1 + next_u64(&mut state) % 1_000_000;
        let cliff = next_u64(&mut state) % 2_000_000;
        let now = next_u64(&mut state) % 3_000_000;
        let vested = defi::schedule::vested_floor(total, start, cliff, duration, now).unwrap();
        assert!(vested <= total, "over-released");
        if now < start || now < cliff {
            assert_eq!(vested, 0, "released before the cliff");
        }
        if now >= cliff && now >= start && now - start >= duration {
            assert_eq!(vested, total, "withheld after the end");
        }

        let from = next_u64(&mut state) % MODELED_DOMAIN;
        let to = next_u64(&mut state) % MODELED_DOMAIN;
        let elapsed = next_u64(&mut state) % (duration + 1);
        let floor = defi::schedule::linear_interp_floor(from, to, elapsed, duration).unwrap();
        let ceil = defi::schedule::linear_interp_ceil(from, to, elapsed, duration).unwrap();
        let (low, high) = (from.min(to), from.max(to));
        assert!(low <= floor && floor <= high, "floor left the segment");
        assert!(low <= ceil && ceil <= high, "ceil left the segment");
        assert!(floor <= ceil || from > to, "rising floor exceeds ceil");
        if elapsed >= duration {
            assert_eq!(floor, to);
            assert_eq!(ceil, to);
        }
    }
}

#[test]
fn reward_index_is_monotone_and_identities_hold() {
    let mut state = 0x5354_414b_455f_4f4b;
    for _ in 0..SWEEP {
        let index = next_u64(&mut state) % MODELED_DOMAIN;
        let reward = next_u64(&mut state) % 1_000_000_000_000;
        let staked = next_u64(&mut state) % 1_000_000_000;
        let scale = 1 + next_u64(&mut state) % 1_000_000_000;
        let accrued =
            defi::staking::reward_index_accrue_lower(index, reward, staked, scale).unwrap();
        assert!(accrued >= index, "the index moved backwards");
        if staked == 0 || reward == 0 {
            assert_eq!(accrued, index, "an empty accrual moved the index");
        }
        assert_eq!(
            defi::staking::rewards_owed_floor(staked, index, index, scale),
            Ok(0),
            "rewards owed at an unmoved index"
        );
    }
}

#[test]
fn directed_pairs_are_ordered_across_the_modeled_domain() {
    let mut state = 0x4f52_4445_525f_4f4b;
    for _ in 0..SWEEP {
        let scale = 1 + next_u64(&mut state) % MODELED_DOMAIN;
        let value = 1 + next_u64(&mut state) % MODELED_DOMAIN;
        let exponent = (next_u64(&mut state) % 8_000_000_000) as i64
            - i64::try_from(next_u64(&mut state) % 4_000_000_000).unwrap();
        if let Ok((lower, upper)) = svm_math::exp2_bounds(exponent, scale) {
            assert!(lower <= upper, "exp2 bounds disordered");
        }
        if let Ok((lower, upper)) = svm_math::log2_bounds(value, scale) {
            assert!(lower <= upper, "log2 bounds disordered");
        }
        if let Ok((lower, upper)) = svm_math::pow_bounds(value, exponent.unsigned_abs(), scale) {
            assert!(lower <= upper, "pow bounds disordered");
        }
        if let Ok((lower, upper)) = svm_math::compound_bounds(
            value % (scale / 256 + 1),
            1 + next_u64(&mut state) % 1_000_000,
            next_u64(&mut state) % 100_000,
            scale,
        ) {
            assert!(lower <= upper, "compound bounds disordered");
        }
    }
}
