use svm_math::{
    compound_bounds, compound_lower, compound_upper, exp2_bounds, exp2_lower, exp2_upper,
    log2_bounds, log2_lower, log2_upper, pow_bounds, pow_lower, pow_upper, powi_lower, powi_upper,
    MathError,
};

const S: u64 = 1_000_000;

#[test]
fn scale_errors_have_precedence() {
    assert_eq!(exp2_lower(0, 0), Err(MathError::DivByZero));
    assert_eq!(log2_lower(0, 0), Err(MathError::DivByZero));
    assert_eq!(pow_lower(0, 0, 0), Err(MathError::DivByZero));
    assert_eq!(powi_lower(0, 0, 0), Err(MathError::DivByZero));
    assert_eq!(compound_lower(0, 0, 0, 0), Err(MathError::DivByZero));
}

#[test]
fn exp2_contains_exact_integer_powers() {
    let scale = 1_u64 << 20;
    for integer in -16_i64..=16 {
        let exponent = integer * scale as i64;
        let (lower, upper) = (
            exp2_lower(exponent, scale).unwrap(),
            exp2_upper(exponent, scale).unwrap(),
        );
        let exact = if integer >= 0 {
            scale.checked_shl(integer as u32).unwrap()
        } else {
            scale >> integer.unsigned_abs()
        };
        assert!(
            lower <= exact && exact <= upper,
            "integer={integer}: [{lower}, {upper}]"
        );
    }
}

#[test]
fn negative_exp2_rounds_outward_at_nondecimal_scale() {
    let scale = 7;
    let lower = exp2_lower(-7, scale).unwrap();
    let upper = exp2_upper(-7, scale).unwrap();
    assert!(lower <= 3);
    assert!(upper >= 4);
    assert!(lower <= upper);
}

#[test]
fn exp2_reports_true_overflow_and_encloses_deep_underflow() {
    assert_eq!(exp2_lower(64, 1), Err(MathError::Overflow));
    assert_eq!(exp2_upper(64, 1), Err(MathError::Overflow));
    assert_eq!(exp2_lower(-128, 1), Ok(0));
    assert_eq!(exp2_upper(-128, 1), Ok(1));
}

#[test]
fn log2_contains_exact_powers_and_rejects_zero() {
    assert_eq!(log2_lower(0, S), Err(MathError::OutOfDomain));
    let scale = 1_u64 << 20;
    for integer in -16_i64..=16 {
        let value = if integer >= 0 {
            scale.checked_shl(integer as u32).unwrap()
        } else {
            scale >> integer.unsigned_abs()
        };
        let exact = integer * scale as i64;
        let lower = log2_lower(value, scale).unwrap();
        let upper = log2_upper(value, scale).unwrap();
        assert!(
            lower <= exact && exact <= upper,
            "integer={integer}: [{lower}, {upper}]"
        );
    }
}

#[test]
fn subunit_log2_uses_mathematical_signed_rounding() {
    let lower = log2_lower(3, 7).unwrap();
    let upper = log2_upper(3, 7).unwrap();
    assert!(lower < 0 && upper < 0);
    assert!(lower <= upper);
    assert!(lower <= -8 && -8 <= upper); // log2(3/7) * 7 is about -8.55.
}

#[test]
fn log2_does_not_carry_across_power_of_two_boundaries() {
    let below = 2 * S - 1;
    assert!(log2_lower(below, S).unwrap() < S as i64);
    assert!(log2_upper(below, S).unwrap() <= S as i64);
    assert_eq!(log2_lower(2 * S, S), Ok(S as i64));
    assert_eq!(log2_upper(2 * S, S), Ok(S as i64));
}

#[test]
fn fractional_power_handles_bases_on_both_sides_of_one() {
    for base in [S / 4, S, 4 * S] {
        let lower = pow_lower(base, S / 2, S).unwrap();
        let upper = pow_upper(base, S / 2, S).unwrap();
        let exact = match base {
            x if x == S / 4 => S / 2,
            x if x == S => S,
            _ => 2 * S,
        };
        assert!(
            lower <= exact && exact <= upper,
            "base={base}: [{lower}, {upper}]"
        );
    }
    assert_eq!(pow_lower(0, 0, S), Ok(S));
    assert_eq!(pow_upper(0, 1, S), Ok(0));
}

#[test]
fn pow_with_integer_exponents_matches_direct_exponentiation() {
    for (base, whole) in [(2 * S, 2_u64), (3 * S, 3), (S / 2, 4), (7 * S, 1)] {
        let exponent = whole * S;
        assert_eq!(
            pow_lower(base, exponent, S),
            powi_lower(base, whole, S),
            "base={base}, whole={whole}"
        );
        assert_eq!(
            pow_upper(base, exponent, S),
            powi_upper(base, whole, S),
            "base={base}, whole={whole}"
        );
    }
    // 2^2 at an exact integer exponent is now exact in both directions.
    assert_eq!(pow_lower(2 * S, 2 * S, S), Ok(4 * S));
    assert_eq!(pow_upper(2 * S, 2 * S, S), Ok(4 * S));
}

#[test]
fn directed_powi_contains_exact_values() {
    assert_eq!(powi_lower(0, 0, S), Ok(S));
    assert_eq!(powi_upper(0, 0, S), Ok(S));
    for exponent in 0..=12 {
        let exact = S * (1_u64 << exponent);
        let lower = powi_lower(2 * S, exponent, S).unwrap();
        let upper = powi_upper(2 * S, exponent, S).unwrap();
        assert!(lower <= exact && exact <= upper);
    }

    assert_eq!(powi_lower(3, 2, 2), Ok(4));
    assert_eq!(powi_upper(3, 2, 2), Ok(5));
}

#[test]
fn directed_powi_encloses_small_exact_rationals() {
    for scale in [3_u64, 7, 1_000] {
        for base in [1, scale - 1, scale, scale + 1, 2 * scale] {
            for exponent in 0..=8_u64 {
                let (numerator, denominator) = if exponent == 0 {
                    (u128::from(scale), 1)
                } else {
                    (
                        u128::from(base).pow(exponent as u32),
                        u128::from(scale).pow((exponent - 1) as u32),
                    )
                };
                let floor = numerator / denominator;
                let ceil = floor + u128::from(numerator % denominator != 0);
                let lower = powi_lower(base, exponent, scale).unwrap();
                let upper = powi_upper(base, exponent, scale).unwrap();
                assert!(
                    u128::from(lower) <= floor && ceil <= u128::from(upper),
                    "b={base},n={exponent},S={scale}: [{lower}, {upper}]"
                );
            }
        }
    }
}

#[test]
fn powi_preserves_identities_and_true_overflow() {
    for scale in [1_u64, 3, 1_000_000, u64::MAX] {
        assert_eq!(powi_lower(u64::MAX, 0, scale), Ok(scale));
        assert_eq!(powi_upper(u64::MAX, 1, scale), Ok(u64::MAX));
    }
    assert_eq!(powi_lower(u64::MAX, 2, 1), Err(MathError::Overflow));
    assert_eq!(powi_upper(u64::MAX, 2, 1), Err(MathError::Overflow));
    assert_eq!(powi_lower(3, u64::MAX, 3), Ok(3));
    assert_eq!(powi_upper(3, u64::MAX, 3), Ok(3));
    assert_eq!(powi_lower(2, u64::MAX, 3), Ok(0));
    assert!(powi_upper(2, u64::MAX, 3).unwrap() >= 1);
    assert_eq!(powi_lower(4, u64::MAX, 3), Err(MathError::Overflow));
    assert_eq!(powi_upper(4, u64::MAX, 3), Err(MathError::Overflow));
}

#[test]
fn compound_identity_domain_and_direction() {
    assert_eq!(compound_lower(0, 365, 365, S), Ok(S));
    assert_eq!(compound_upper(1, 365, 0, S), Ok(S));
    assert_eq!(compound_lower(1, 0, 1, S), Err(MathError::OutOfDomain));
    assert!(compound_upper(S, 256, 1, S).unwrap() >= S);
    // Just past the series domain the binary-squaring path takes over:
    // one period of rate (S+1)/(256·S) is exactly S + (S+1)/256.
    let lower = compound_lower(S + 1, 256, 1, S).unwrap();
    let upper = compound_upper(S + 1, 256, 1, S).unwrap();
    assert!(
        lower <= S + 3_906 && S + 3_907 <= upper,
        "[{lower}, {upper}]"
    );
    assert!(upper - lower <= 4, "width {}", upper - lower);

    let lower = compound_lower(50_000, 365, 365, S).unwrap();
    let upper = compound_upper(50_000, 365, 365, S).unwrap();
    assert!(S < lower && lower <= upper);

    let huge_scale = u64::MAX;
    assert_eq!(compound_lower(1, u64::MAX, 1, huge_scale), Ok(huge_scale));
    assert!(compound_upper(1, u64::MAX, 1, huge_scale).is_err());
}

#[test]
fn compound_one_period_encloses_the_exact_formula_at_the_series_boundary() {
    for (periods, scale) in [
        (1_u64, 256_u64),
        (12, 1_000_000),
        (365, 1_000_000),
        (10_000, 7),
    ] {
        let denominator = u128::from(periods) * u128::from(scale);
        let annual_rate = u64::try_from(denominator / 256).unwrap();
        let numerator = u128::from(annual_rate);
        let floor = u128::from(scale) + numerator / u128::from(periods);
        let ceil = floor + u128::from(numerator % u128::from(periods) != 0);
        let lower = compound_lower(annual_rate, periods, 1, scale).unwrap();
        let upper = compound_upper(annual_rate, periods, 1, scale).unwrap();
        assert!(
            u128::from(lower) <= floor && ceil <= u128::from(upper),
            "r={annual_rate},n={periods},S={scale}: [{lower}, {upper}]"
        );
        // One rate unit past the boundary crosses into the binary-squaring
        // path; the same exact one-period formula must stay enclosed.
        let past = annual_rate + 1;
        let floor = u128::from(scale) + u128::from(past) / u128::from(periods);
        let ceil = floor + u128::from(u128::from(past) % u128::from(periods) != 0);
        let lower = compound_lower(past, periods, 1, scale).unwrap();
        let upper = compound_upper(past, periods, 1, scale).unwrap();
        assert!(
            u128::from(lower) <= floor && ceil <= u128::from(upper),
            "r={past},n={periods},S={scale}: [{lower}, {upper}]"
        );
    }
}

#[test]
fn compound_beyond_the_series_domain_encloses_host_references() {
    let scale = 1_000_000_000_u64;
    let scale_f = scale as f64;
    // (rate, periods, elapsed): monthly 5% APR for one and five years,
    // annual 5% and 100%, and a 100% rate compounded annually for 30 years.
    for (rate, periods, elapsed) in [
        (50_000_000_u64, 12_u64, 12_u64),
        (50_000_000, 12, 60),
        (50_000_000, 1, 1),
        (1_000_000_000, 1, 1),
        (1_000_000_000, 1, 30),
    ] {
        let truth = (1.0 + rate as f64 / scale_f / periods as f64).powi(elapsed as i32) * scale_f;
        let lower = compound_lower(rate, periods, elapsed, scale).unwrap();
        let upper = compound_upper(rate, periods, elapsed, scale).unwrap();
        assert!(
            (lower as f64) <= truth && truth <= upper as f64,
            "r={rate},n={periods},t={elapsed}: [{lower}, {upper}] vs {truth}"
        );
        assert!(
            upper - lower <= 1 + upper / 1_000_000_000,
            "r={rate},n={periods},t={elapsed}: width {}",
            upper - lower
        );
    }
}

#[test]
fn compound_slot_rate_enclosure_width_does_not_regress() {
    // 7% annual at per-slot compounding for one year, scale 1e9. The true
    // value is 1.0725081812...; a scale-unit-quantized per-period rate
    // once widened this enclosure to 6.5% — this pins the repaired width.
    let scale = 1_000_000_000_u64;
    let lower = compound_lower(70_000_000, 63_072_000, 63_072_000, scale).unwrap();
    let upper = compound_upper(70_000_000, 63_072_000, 63_072_000, scale).unwrap();
    assert!(lower <= 1_072_508_181, "lower {lower}");
    assert!(upper >= 1_072_508_182, "upper {upper}");
    assert!(upper - lower <= 10, "width {}", upper - lower);
}

#[test]
fn independent_host_reference_samples_are_enclosed() {
    let scale_f = S as f64;
    for exponent in [-2_750_001_i64, -333_333, 125_001, 3_500_001] {
        let truth = 2_f64.powf(exponent as f64 / scale_f) * scale_f;
        let lower = exp2_lower(exponent, S).unwrap() as f64;
        let upper = exp2_upper(exponent, S).unwrap() as f64;
        assert!(lower <= truth && truth <= upper);
    }

    for value in [1_u64, 333_333, 999_999, 1_000_001, 3_141_593, 17_000_001] {
        let truth = (value as f64 / scale_f).log2() * scale_f;
        let lower = log2_lower(value, S).unwrap() as f64;
        let upper = log2_upper(value, S).unwrap() as f64;
        assert!(lower <= truth && truth <= upper);
    }

    for (base, exponent) in [
        (125_000_u64, 333_333_u64),
        (750_001, 2_500_001),
        (1_250_001, 750_001),
        (3_500_001, 1_250_001),
    ] {
        let truth = (base as f64 / scale_f).powf(exponent as f64 / scale_f) * scale_f;
        let lower = pow_lower(base, exponent, S).unwrap() as f64;
        let upper = pow_upper(base, exponent, S).unwrap() as f64;
        assert!(lower <= truth && truth <= upper);
    }

    let truth = (1_f64 + 50_000_f64 / scale_f / 365_f64).powi(365) * scale_f;
    assert!(compound_lower(50_000, 365, 365, S).unwrap() as f64 <= truth);
    assert!(truth <= compound_upper(50_000, 365, 365, S).unwrap() as f64);
}

fn next_u64(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

/// The fused pair functions promise bit-identical results to the pair of
/// single-direction calls, across successes and errors alike.
#[test]
fn bounds_functions_match_their_single_direction_pairs() {
    let scales = [1_u64, 3, 7, 10_000, 1_000_000, 1_000_000_000, u64::MAX];
    let mut state = 0x424f_554e_4453_5f45;
    for index in 0..4_096_u64 {
        let scale = scales[index as usize % scales.len()];
        let value = next_u64(&mut state) % scale.saturating_mul(4).max(1);
        let signed = (next_u64(&mut state) as i64) % 8_000_000_000;
        let exponent = next_u64(&mut state) % scale.saturating_mul(4).max(1);
        let periods = 1 + next_u64(&mut state) % 100_000;
        let elapsed = next_u64(&mut state) % 100_000;

        assert_eq!(
            exp2_bounds(signed, scale),
            exp2_lower(signed, scale).and_then(|lower| Ok((lower, exp2_upper(signed, scale)?))),
            "exp2 e={signed} S={scale}"
        );
        if value != 0 {
            assert_eq!(
                log2_bounds(value, scale),
                log2_lower(value, scale).and_then(|lower| Ok((lower, log2_upper(value, scale)?))),
                "log2 v={value} S={scale}"
            );
        }
        assert_eq!(
            pow_bounds(value, exponent, scale),
            pow_lower(value, exponent, scale)
                .and_then(|lower| Ok((lower, pow_upper(value, exponent, scale)?))),
            "pow b={value} e={exponent} S={scale}"
        );
        assert_eq!(
            compound_bounds(value, periods, elapsed, scale),
            compound_lower(value, periods, elapsed, scale)
                .and_then(|lower| Ok((lower, compound_upper(value, periods, elapsed, scale)?))),
            "compound r={value} n={periods} t={elapsed} S={scale}"
        );
    }
    // Error parity on the shared boundary cases.
    assert_eq!(exp2_bounds(1, 0), Err(MathError::DivByZero));
    assert_eq!(log2_bounds(0, S), Err(MathError::OutOfDomain));
    assert_eq!(pow_bounds(1, 1, 0), Err(MathError::DivByZero));
    assert_eq!(compound_bounds(1, 0, 1, S), Err(MathError::OutOfDomain));
}
