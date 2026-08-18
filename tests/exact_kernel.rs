use svm_math::{isqrt, mul_div_ceil, mul_div_floor, sqrt_ceil, sqrt_floor, MathError};

fn next_u64(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

fn reference_mul_div(a: u64, b: u64, denominator: u64) -> (u128, u128) {
    let numerator = u128::from(a) * u128::from(b);
    let denominator = u128::from(denominator);
    let floor = numerator / denominator;
    let ceil = floor + u128::from(numerator % denominator != 0);
    (floor, ceil)
}

#[test]
fn math_error_codes_and_display_are_stable() {
    assert_eq!(MathError::DivByZero.code(), 0);
    assert_eq!(MathError::Overflow.code(), 2);
    assert_eq!(MathError::OutOfDomain.code(), 4);
    assert_eq!(MathError::DivByZero.to_string(), "division by zero");
    assert_eq!(MathError::Overflow.to_string(), "arithmetic overflow");
    assert_eq!(
        MathError::OutOfDomain.to_string(),
        "input outside operation domain"
    );
}

#[test]
fn mul_div_routes_zero_denominator_before_overflow() {
    assert_eq!(
        mul_div_floor(u64::MAX, u64::MAX, 0),
        Err(MathError::DivByZero)
    );
    assert_eq!(
        mul_div_ceil(u64::MAX, u64::MAX, 0),
        Err(MathError::DivByZero)
    );
}

#[test]
fn mul_div_boundary_vectors_are_exact() {
    let vectors = [
        (0, u64::MAX, 1),
        (1, 1, 1),
        (10, 20, 6),
        (u64::MAX, 1, 1),
        (u64::MAX, u64::MAX, u64::MAX),
        (u64::MAX - 1, u64::MAX - 1, u64::MAX),
        (1_500_000, 1_000_000_000, 1_000_000),
    ];

    for (a, b, denominator) in vectors {
        let (floor, ceil) = reference_mul_div(a, b, denominator);
        assert_eq!(
            mul_div_floor(a, b, denominator),
            u64::try_from(floor).map_err(|_| MathError::Overflow)
        );
        assert_eq!(
            mul_div_ceil(a, b, denominator),
            u64::try_from(ceil).map_err(|_| MathError::Overflow)
        );
    }
}

#[test]
fn mul_div_detects_floor_and_ceil_overflow_separately() {
    assert_eq!(
        mul_div_floor(u64::MAX, u64::MAX, 1),
        Err(MathError::Overflow)
    );
    assert_eq!(
        mul_div_ceil(u64::MAX, u64::MAX, 1),
        Err(MathError::Overflow)
    );

    let a = u64::MAX - 1;
    let b = u64::MAX - 1;
    let denominator = u64::MAX - 2;
    assert_eq!(mul_div_floor(a, b, denominator), Ok(u64::MAX));
    assert_eq!(mul_div_ceil(a, b, denominator), Err(MathError::Overflow));
}

#[test]
fn mul_div_matches_integer_definition_across_deterministic_samples() {
    let mut state = 0x5eed_cafe_f00d_beef;
    for _ in 0..20_000 {
        let a = next_u64(&mut state);
        let b = next_u64(&mut state);
        let denominator = next_u64(&mut state) | 1;
        let (floor, ceil) = reference_mul_div(a, b, denominator);

        assert_eq!(
            mul_div_floor(a, b, denominator),
            u64::try_from(floor).map_err(|_| MathError::Overflow)
        );
        assert_eq!(
            mul_div_ceil(a, b, denominator),
            u64::try_from(ceil).map_err(|_| MathError::Overflow)
        );
    }
}

fn assert_isqrt_law(value: u128) {
    let root = isqrt(value);
    assert!(root <= value / root.max(1));
    if root < u64::MAX as u128 {
        let successor = root + 1;
        assert!(successor > value / successor);
    } else {
        assert_eq!(root, u64::MAX as u128);
    }
}

#[test]
fn isqrt_handles_perfect_squares_adjacent_values_and_extrema() {
    let roots = [
        0_u64,
        1,
        2,
        3,
        255,
        256,
        65_535,
        1_000_000_000,
        u32::MAX as u64,
        u64::MAX - 1,
        u64::MAX,
    ];

    for root in roots {
        let square = u128::from(root) * u128::from(root);
        assert_eq!(isqrt(square), u128::from(root));
        assert_isqrt_law(square);
        if square > 0 {
            assert_isqrt_law(square - 1);
        }
        if square < u128::MAX {
            assert_isqrt_law(square + 1);
        }
    }

    assert_eq!(isqrt(u128::MAX), u64::MAX as u128);
    assert_isqrt_law(u128::MAX);
}

#[test]
fn isqrt_law_holds_across_deterministic_u128_samples() {
    let mut state = 0x1234_5678_9abc_def0;
    for _ in 0..20_000 {
        let high = next_u64(&mut state);
        let low = next_u64(&mut state);
        assert_isqrt_law((u128::from(high) << 64) | u128::from(low));
    }
}

#[test]
fn scaled_sqrt_validates_scale_first() {
    assert_eq!(sqrt_floor(u64::MAX, 0), Err(MathError::DivByZero));
    assert_eq!(sqrt_ceil(u64::MAX, 0), Err(MathError::DivByZero));
}

#[test]
fn scaled_sqrt_boundary_vectors_are_exact_and_minimal() {
    let vectors = [
        (0, 1),
        (1, 1),
        (2, 1),
        (4, 1),
        (2, 1_000_000),
        (1_500_000, 1_000_000),
        (u64::MAX, 1),
        (u64::MAX, u64::MAX),
        (u64::MAX - 1, u64::MAX),
        (9_876_543_210, 37),
    ];

    for (value, scale) in vectors {
        let radicand = u128::from(value) * u128::from(scale);
        let floor = sqrt_floor(value, scale).unwrap();
        let ceil = sqrt_ceil(value, scale).unwrap();
        assert_eq!(u128::from(floor), isqrt(radicand));
        assert!(u128::from(floor) * u128::from(floor) <= radicand);
        assert!(u128::from(ceil) * u128::from(ceil) >= radicand);
        assert!(ceil == floor || ceil == floor + 1);
        assert_eq!(
            ceil == floor,
            u128::from(floor) * u128::from(floor) == radicand
        );
    }
}

#[test]
fn scaled_sqrt_matches_integer_definition_across_deterministic_samples() {
    let mut state = 0x0ddc_0ffe_e15e_beef;
    for _ in 0..20_000 {
        let value = next_u64(&mut state);
        let scale = next_u64(&mut state) | 1;
        let radicand = u128::from(value) * u128::from(scale);
        let reference_floor = u64::try_from(isqrt(radicand)).unwrap();
        let reference_ceil = reference_floor
            + u64::from(u128::from(reference_floor) * u128::from(reference_floor) != radicand);

        assert_eq!(sqrt_floor(value, scale), Ok(reference_floor));
        assert_eq!(sqrt_ceil(value, scale), Ok(reference_ceil));
    }
}
