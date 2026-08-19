use rug::{float::Round, ops::Pow, Float, Integer};
use std::{fmt::Debug, process::ExitCode};
use svm_math::{
    compound_lower, compound_upper, exp2_lower, exp2_upper, log2_lower, log2_upper, mul_div_ceil,
    mul_div_floor, pow_lower, pow_upper, powi_lower, powi_upper, sqrt_ceil, sqrt_floor, MathError,
};

const FIRST_PRECISION: u32 = 256;
const LAST_PRECISION: u32 = 4_096;

#[derive(Debug)]
struct Stats {
    name: &'static str,
    attempted: u64,
    resolved: u64,
    violations: u64,
    unresolved: u64,
    skipped: u64,
    error_checks: u64,
    max_width: u128,
}

impl Stats {
    const fn new(name: &'static str) -> Self {
        Self {
            name,
            attempted: 0,
            resolved: 0,
            violations: 0,
            unresolved: 0,
            skipped: 0,
            error_checks: 0,
            max_width: 0,
        }
    }

    fn record_api_failure(&mut self, label: &str, detail: impl Debug, failures: &mut Vec<String>) {
        self.attempted += 1;
        self.violations += 1;
        failures.push(format!("{} {label}: API failure: {detail:?}", self.name));
    }
}

fn ratio_i64(numerator: i64, denominator: u64, precision: u32, round: Round) -> Float {
    let numerator = Float::with_val(precision, numerator);
    let denominator = Float::with_val(precision, denominator);
    Float::with_val_round(precision, &numerator / &denominator, round).0
}

fn ratio_u64(numerator: u64, denominator: u64, precision: u32, round: Round) -> Float {
    let numerator = Float::with_val(precision, numerator);
    let denominator = Float::with_val(precision, denominator);
    Float::with_val_round(precision, &numerator / &denominator, round).0
}

fn ratio_u128(numerator: u128, denominator: u128, precision: u32, round: Round) -> Float {
    let numerator = Float::with_val(precision, numerator);
    let denominator = Float::with_val(precision, denominator);
    Float::with_val_round(precision, &numerator / &denominator, round).0
}

fn scale_interval(lower: &Float, upper: &Float, scale: u64, precision: u32) -> (Float, Float) {
    (
        Float::with_val_round(precision, lower * scale, Round::Down).0,
        Float::with_val_round(precision, upper * scale, Round::Up).0,
    )
}

enum Resolution {
    Enclosed,
    Violated,
    Ambiguous,
}

fn classify(lower: i128, upper: i128, truth_lower: &Float, truth_upper: &Float) -> Resolution {
    let lower = Float::with_val(truth_lower.prec(), lower);
    let upper = Float::with_val(truth_upper.prec(), upper);
    if lower <= *truth_lower && *truth_upper <= upper {
        Resolution::Enclosed
    } else if lower > *truth_upper || upper < *truth_lower {
        Resolution::Violated
    } else {
        Resolution::Ambiguous
    }
}

fn check_enclosure(
    stats: &mut Stats,
    label: &str,
    lower: i128,
    upper: i128,
    truth: impl Fn(u32) -> (Float, Float),
    failures: &mut Vec<String>,
) {
    stats.attempted += 1;
    if lower > upper {
        stats.violations += 1;
        failures.push(format!(
            "{} {label}: lower {lower} exceeds upper {upper}",
            stats.name
        ));
        return;
    }
    stats.max_width = stats.max_width.max((upper - lower) as u128);

    let mut precision = FIRST_PRECISION;
    loop {
        let (truth_lower, truth_upper) = truth(precision);
        if truth_lower > truth_upper {
            stats.violations += 1;
            failures.push(format!(
                "{} {label}: invalid MPFR interval at {precision} bits",
                stats.name
            ));
            return;
        }
        match classify(lower, upper, &truth_lower, &truth_upper) {
            Resolution::Enclosed => {
                stats.resolved += 1;
                return;
            }
            Resolution::Violated => {
                stats.violations += 1;
                failures.push(format!(
                    "{} {label}: [{lower}, {upper}] does not enclose MPFR [{}, {}] at {precision} bits",
                    stats.name,
                    truth_lower.to_string_radix(10, Some(24)),
                    truth_upper.to_string_radix(10, Some(24)),
                ));
                return;
            }
            Resolution::Ambiguous if precision < LAST_PRECISION => precision *= 2,
            Resolution::Ambiguous => {
                stats.unresolved += 1;
                failures.push(format!(
                    "{} {label}: unresolved at {LAST_PRECISION} bits",
                    stats.name
                ));
                return;
            }
        }
    }
}

fn check_unsigned_enclosure(
    stats: &mut Stats,
    label: &str,
    lower: Result<u64, MathError>,
    upper: Result<u64, MathError>,
    truth: impl Fn(u32) -> (Float, Float),
    failures: &mut Vec<String>,
) {
    if let (Ok(lower), Ok(upper)) = (lower, upper) {
        return check_enclosure(
            stats,
            label,
            i128::from(lower),
            i128::from(upper),
            truth,
            failures,
        );
    }

    stats.attempted += 1;
    let mut precision = FIRST_PRECISION;
    loop {
        let (truth_lower, truth_upper) = truth(precision);
        let maximum = Float::with_val(precision, u64::MAX);
        let first_overflow = Float::with_val(precision, Integer::from(u64::MAX) + 1);
        let resolution = match (&lower, &upper) {
            (Err(MathError::Overflow), Err(MathError::Overflow)) => {
                if truth_lower >= first_overflow {
                    Resolution::Enclosed
                } else if truth_upper < first_overflow {
                    Resolution::Violated
                } else {
                    Resolution::Ambiguous
                }
            }
            (Ok(lower), Err(MathError::Overflow)) => {
                let lower = Float::with_val(precision, *lower);
                if lower <= truth_lower && truth_lower > maximum {
                    Resolution::Enclosed
                } else if lower > truth_upper || truth_upper <= maximum {
                    Resolution::Violated
                } else {
                    Resolution::Ambiguous
                }
            }
            _ => Resolution::Violated,
        };
        match resolution {
            Resolution::Enclosed => {
                stats.resolved += 1;
                return;
            }
            Resolution::Violated => {
                stats.violations += 1;
                failures.push(format!(
                    "{} {label}: invalid overflow result ({lower:?}, {upper:?}) for MPFR [{}, {}]",
                    stats.name,
                    truth_lower.to_string_radix(10, Some(24)),
                    truth_upper.to_string_radix(10, Some(24)),
                ));
                return;
            }
            Resolution::Ambiguous if precision < LAST_PRECISION => precision *= 2,
            Resolution::Ambiguous => {
                stats.unresolved += 1;
                failures.push(format!(
                    "{} {label}: overflow boundary unresolved at {LAST_PRECISION} bits",
                    stats.name
                ));
                return;
            }
        }
    }
}

fn expect_error<T: Debug + PartialEq>(
    stats: &mut Stats,
    label: &str,
    actual: Result<T, MathError>,
    expected: MathError,
    failures: &mut Vec<String>,
) {
    stats.error_checks += 1;
    if actual != Err(expected) {
        stats.violations += 1;
        failures.push(format!(
            "{} {label}: expected {expected:?}, got {actual:?}",
            stats.name
        ));
    }
}

fn next_u64(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

fn check_exp2(stats: &mut Stats, failures: &mut Vec<String>) {
    let scales = [1_u64, 3, 7, 65_535, 1_000_000, u64::from(u32::MAX)];
    let mut state = 0x4558_5032_4f52_4143;
    for index in 0..8_192_u64 {
        let scale = scales[index as usize % scales.len()];
        let scale_i64 = scale as i64;
        let exponent = match index % 12 {
            0 => 0,
            1 => scale_i64,
            2 => -scale_i64,
            3 => 32 * scale_i64,
            4 => -32 * scale_i64,
            5 => scale_i64 - 1,
            6 => scale_i64 + 1,
            7 => 1 - scale_i64,
            8 => -scale_i64 - 1,
            _ => {
                let half_span = 32 * scale;
                (next_u64(&mut state) % (2 * half_span + 1)) as i64 - half_span as i64
            }
        };
        let label = format!("e={exponent},S={scale}");
        match (exp2_lower(exponent, scale), exp2_upper(exponent, scale)) {
            (Ok(lower), Ok(upper)) => check_enclosure(
                stats,
                &label,
                i128::from(lower),
                i128::from(upper),
                |precision| {
                    let x_lower = ratio_i64(exponent, scale, precision, Round::Down);
                    let x_upper = ratio_i64(exponent, scale, precision, Round::Up);
                    let truth_lower =
                        Float::with_val_round(precision, x_lower.exp2_ref(), Round::Down).0;
                    let truth_upper =
                        Float::with_val_round(precision, x_upper.exp2_ref(), Round::Up).0;
                    scale_interval(&truth_lower, &truth_upper, scale, precision)
                },
                failures,
            ),
            pair => stats.record_api_failure(&label, pair, failures),
        }
    }

    expect_error(
        stats,
        "scale precedence lower",
        exp2_lower(i64::MAX, 0),
        MathError::DivByZero,
        failures,
    );
    expect_error(
        stats,
        "scale precedence upper",
        exp2_upper(i64::MIN, 0),
        MathError::DivByZero,
        failures,
    );
    expect_error(
        stats,
        "positive overflow lower",
        exp2_lower(64, 1),
        MathError::Overflow,
        failures,
    );
    expect_error(
        stats,
        "positive overflow upper",
        exp2_upper(64, 1),
        MathError::Overflow,
        failures,
    );
}

fn check_log2(stats: &mut Stats, failures: &mut Vec<String>) {
    let scales = [1_u64, 3, 7, 65_535, 1_000_000, u64::from(u32::MAX)];
    let mut state = 0x4c4f_4732_4f52_4143;
    for index in 0..32_768_u64 {
        let scale = scales[index as usize % scales.len()];
        let doubled = scale * 2;
        let value = match index % 12 {
            0 => 1,
            1 => scale,
            2 => scale.saturating_sub(1).max(1),
            3 => scale + 1,
            4 => doubled - 1,
            5 => doubled,
            6 => doubled + 1,
            7 => u64::MAX,
            8 => 1_u64 << (next_u64(&mut state) % 64),
            _ => next_u64(&mut state) | 1,
        };
        let label = format!("v={value},S={scale}");
        match (log2_lower(value, scale), log2_upper(value, scale)) {
            (Ok(lower), Ok(upper)) => check_enclosure(
                stats,
                &label,
                i128::from(lower),
                i128::from(upper),
                |precision| {
                    let x_lower = ratio_u64(value, scale, precision, Round::Down);
                    let x_upper = ratio_u64(value, scale, precision, Round::Up);
                    let truth_lower =
                        Float::with_val_round(precision, x_lower.log2_ref(), Round::Down).0;
                    let truth_upper =
                        Float::with_val_round(precision, x_upper.log2_ref(), Round::Up).0;
                    scale_interval(&truth_lower, &truth_upper, scale, precision)
                },
                failures,
            ),
            pair => stats.record_api_failure(&label, pair, failures),
        }
    }

    expect_error(
        stats,
        "scale precedence lower",
        log2_lower(0, 0),
        MathError::DivByZero,
        failures,
    );
    expect_error(
        stats,
        "scale precedence upper",
        log2_upper(0, 0),
        MathError::DivByZero,
        failures,
    );
    expect_error(
        stats,
        "zero lower",
        log2_lower(0, 1),
        MathError::OutOfDomain,
        failures,
    );
    expect_error(
        stats,
        "zero upper",
        log2_upper(0, 1),
        MathError::OutOfDomain,
        failures,
    );
}

fn pow_truth(base: u64, exponent: u64, scale: u64, precision: u32) -> (Float, Float) {
    if exponent == 0 {
        return (
            Float::with_val(precision, scale),
            Float::with_val(precision, scale),
        );
    }
    if base == 0 {
        return (Float::with_val(precision, 0), Float::with_val(precision, 0));
    }
    if exponent % scale == 0 {
        // An exact integer exponent has the exact rational truth
        // base^k / scale^(k-1); the sweep keeps k at most 4, so the
        // integers stay small and the directed division is exact.
        let whole = u32::try_from(exponent / scale).expect("sweep keeps integer exponents small");
        let numerator = Integer::from(base).pow(whole);
        let denominator = Integer::from(scale).pow(whole - 1);
        let numerator_lower = Float::with_val_round(precision, &numerator, Round::Down).0;
        let numerator_upper = Float::with_val_round(precision, &numerator, Round::Up).0;
        let lower = Float::with_val_round(precision, &numerator_lower / &denominator, Round::Down).0;
        let upper = Float::with_val_round(precision, &numerator_upper / &denominator, Round::Up).0;
        return (lower, upper);
    }

    let base_lower = ratio_u64(base, scale, precision, Round::Down);
    let base_upper = ratio_u64(base, scale, precision, Round::Up);
    let exponent_lower = ratio_u64(exponent, scale, precision, Round::Down);
    let exponent_upper = ratio_u64(exponent, scale, precision, Round::Up);
    let (lower_exponent, upper_exponent) = if base >= scale {
        (&exponent_lower, &exponent_upper)
    } else {
        (&exponent_upper, &exponent_lower)
    };
    let lower = Float::with_val_round(precision, (&base_lower).pow(lower_exponent), Round::Down).0;
    let upper = Float::with_val_round(precision, (&base_upper).pow(upper_exponent), Round::Up).0;
    scale_interval(&lower, &upper, scale, precision)
}

fn check_pow(stats: &mut Stats, failures: &mut Vec<String>) {
    let scales = [3_u64, 7, 65_535, 1_000_000, u64::from(u32::MAX)];
    let mut state = 0x504f_575f_4f52_4143;
    for index in 0..256_u64 {
        let scale = scales[index as usize % scales.len()];
        let base = match index % 10 {
            0 => 0,
            1 => scale,
            2 => scale - 1,
            3 => scale + 1,
            4 => 4 * scale,
            _ => next_u64(&mut state) % (4 * scale + 1),
        };
        let exponent = match index % 8 {
            0 => 0,
            1 => 1,
            2 => scale,
            3 => scale / 2,
            4 => 4 * scale,
            _ => next_u64(&mut state) % (4 * scale + 1),
        };
        let label = format!("b={base},e={exponent},S={scale}");
        match (
            pow_lower(base, exponent, scale),
            pow_upper(base, exponent, scale),
        ) {
            (Ok(lower), Ok(upper)) => check_enclosure(
                stats,
                &label,
                i128::from(lower),
                i128::from(upper),
                |precision| pow_truth(base, exponent, scale, precision),
                failures,
            ),
            pair => stats.record_api_failure(&label, pair, failures),
        }
    }

    expect_error(
        stats,
        "scale precedence lower",
        pow_lower(0, 0, 0),
        MathError::DivByZero,
        failures,
    );
    expect_error(
        stats,
        "scale precedence upper",
        pow_upper(0, 0, 0),
        MathError::DivByZero,
        failures,
    );
}

fn powi_truth(base: u64, exponent: u64, scale: u64, precision: u32) -> (Float, Float) {
    if exponent == 0 {
        return (
            Float::with_val(precision, scale),
            Float::with_val(precision, scale),
        );
    }
    if exponent == 1 {
        return (
            Float::with_val(precision, base),
            Float::with_val(precision, base),
        );
    }
    let base_lower = ratio_u64(base, scale, precision, Round::Down);
    let base_upper = ratio_u64(base, scale, precision, Round::Up);
    let lower = Float::with_val_round(precision, (&base_lower).pow(exponent), Round::Down).0;
    let upper = Float::with_val_round(precision, (&base_upper).pow(exponent), Round::Up).0;
    scale_interval(&lower, &upper, scale, precision)
}

fn check_powi(stats: &mut Stats, failures: &mut Vec<String>) {
    let scales = [
        1_u64,
        3,
        7,
        65_535,
        1_000_000,
        u64::from(u32::MAX),
        1_000_000_000_000_000_000,
        u64::MAX,
    ];
    let mut state = 0x504f_5749_4f52_4143;
    for index in 0..4_096_u64 {
        let scale = scales[index as usize % scales.len()];
        let twice_scale = scale.saturating_mul(2);
        let base = match index % 10 {
            0 => 0,
            1 => scale,
            2 => scale.saturating_sub(1),
            3 => scale.saturating_add(1),
            4 => twice_scale,
            5 => scale / 2,
            6 => 1,
            _ if twice_scale == u64::MAX => next_u64(&mut state),
            _ => next_u64(&mut state) % (twice_scale + 1),
        };
        let exponent = match index % 8 {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 16,
            5 => 31,
            6 => 63,
            _ => next_u64(&mut state) % 64,
        };
        let label = format!("b={base},n={exponent},S={scale}");
        check_unsigned_enclosure(
            stats,
            &label,
            powi_lower(base, exponent, scale),
            powi_upper(base, exponent, scale),
            |precision| powi_truth(base, exponent, scale, precision),
            failures,
        );
    }

    expect_error(
        stats,
        "scale precedence lower",
        powi_lower(0, 0, 0),
        MathError::DivByZero,
        failures,
    );
    expect_error(
        stats,
        "scale precedence upper",
        powi_upper(0, 0, 0),
        MathError::DivByZero,
        failures,
    );
}

fn compound_truth(
    annual_rate: u64,
    periods: u64,
    elapsed: u64,
    scale: u64,
    precision: u32,
) -> (Float, Float) {
    if elapsed == 0 || annual_rate == 0 {
        let exact = Float::with_val(precision, scale);
        return (exact.clone(), exact);
    }
    if elapsed == 1 {
        let numerator = Integer::from(scale) * periods + annual_rate;
        let denominator = Float::with_val(precision, periods);
        let numerator = Float::with_val(precision, numerator);
        return (
            Float::with_val_round(precision, &numerator / &denominator, Round::Down).0,
            Float::with_val_round(precision, &numerator / &denominator, Round::Up).0,
        );
    }
    let denominator = u128::from(periods) * u128::from(scale);
    let x_lower = ratio_u128(u128::from(annual_rate), denominator, precision, Round::Down);
    let x_upper = ratio_u128(u128::from(annual_rate), denominator, precision, Round::Up);
    let one = Float::with_val(precision, 1);
    let base_lower = Float::with_val_round(precision, &one + &x_lower, Round::Down).0;
    let base_upper = Float::with_val_round(precision, &one + &x_upper, Round::Up).0;
    let lower = Float::with_val_round(precision, (&base_lower).pow(elapsed), Round::Down).0;
    let upper = Float::with_val_round(precision, (&base_upper).pow(elapsed), Round::Up).0;
    scale_interval(&lower, &upper, scale, precision)
}

fn check_compound(stats: &mut Stats, failures: &mut Vec<String>) {
    let scales = [1_u64, 3, 7, 65_535, 1_000_000, u64::from(u32::MAX)];
    let periods_values = [1_u64, 12, 256, 365, 10_000];
    let mut state = 0x434f_4d50_4f52_4143;
    for index in 0..4_096_u64 {
        let scale = scales[index as usize % scales.len()];
        let periods = periods_values[(index as usize / scales.len()) % periods_values.len()];
        let denominator = u128::from(periods) * u128::from(scale);
        let maximum_rate = u64::try_from((denominator / 256).min(u128::from(u64::MAX))).unwrap();
        let annual_rate = match index % 10 {
            0 => 0,
            1 => 1.min(maximum_rate),
            2 => maximum_rate,
            3 => maximum_rate.saturating_sub(1),
            4 => maximum_rate,
            5 => maximum_rate.saturating_sub(1),
            _ if maximum_rate == u64::MAX => next_u64(&mut state),
            _ => next_u64(&mut state) % (maximum_rate + 1),
        };
        let elapsed = match index % 8 {
            0 => 0,
            1 => 1,
            2 => periods.min(1_024),
            3 => 1_024,
            _ => next_u64(&mut state) % 1_025,
        };
        let label = format!("r={annual_rate},n={periods},t={elapsed},S={scale}");
        check_unsigned_enclosure(
            stats,
            &label,
            compound_lower(annual_rate, periods, elapsed, scale),
            compound_upper(annual_rate, periods, elapsed, scale),
            |precision| compound_truth(annual_rate, periods, elapsed, scale, precision),
            failures,
        );
    }

    expect_error(
        stats,
        "scale precedence lower",
        compound_lower(1, 0, 1, 0),
        MathError::DivByZero,
        failures,
    );
    expect_error(
        stats,
        "scale precedence upper",
        compound_upper(1, 0, 1, 0),
        MathError::DivByZero,
        failures,
    );
    expect_error(
        stats,
        "zero periods lower",
        compound_lower(1, 0, 1, 1),
        MathError::OutOfDomain,
        failures,
    );
    expect_error(
        stats,
        "zero periods upper",
        compound_upper(1, 0, 1, 1),
        MathError::OutOfDomain,
        failures,
    );
    // Past the series domain the binary-squaring path takes over; the
    // enclosure contract is identical, so the oracle checks it the same way.
    check_unsigned_enclosure(
        stats,
        "beyond series domain r=2,n=256,t=1,S=1",
        compound_lower(2, 256, 1, 1),
        compound_upper(2, 256, 1, 1),
        |precision| compound_truth(2, 256, 1, 1, precision),
        failures,
    );
    let scales = [1_u64, 3, 7, 65_535, 1_000_000, u64::from(u32::MAX)];
    let periods_values = [1_u64, 12, 256, 365, 10_000];
    let mut state = 0x4249_4e50_4f57_5f43;
    for index in 0..512_u64 {
        let scale = scales[index as usize % scales.len()];
        let periods = periods_values[(index as usize / scales.len()) % periods_values.len()];
        let denominator = u128::from(periods) * u128::from(scale);
        // Strictly beyond the series domain, with the per-period rate capped
        // near 16 so short exponents keep the result inside `u64 / scale`.
        let minimum = denominator / 256 + 1;
        let span = denominator.saturating_mul(15).max(1);
        let annual_rate =
            match u64::try_from((minimum + u128::from(next_u64(&mut state)) % span).min(u128::from(u64::MAX))) {
                Ok(rate) => rate,
                Err(_) => {
                    stats.skipped += 1;
                    continue;
                }
            };
        let elapsed = index % 4;
        let label = format!("beyond r={annual_rate},n={periods},t={elapsed},S={scale}");
        check_unsigned_enclosure(
            stats,
            &label,
            compound_lower(annual_rate, periods, elapsed, scale),
            compound_upper(annual_rate, periods, elapsed, scale),
            |precision| compound_truth(annual_rate, periods, elapsed, scale, precision),
            failures,
        );
    }
}

fn check_exact(stats: &mut Stats, failures: &mut Vec<String>) {
    let mut state = 0x4558_4143_545f_4f52;
    for index in 0..512_u64 {
        let a = next_u64(&mut state);
        let b = next_u64(&mut state);
        let denominator = a.max(b).max(1);
        let numerator = Integer::from(a) * Integer::from(b);
        let quotient = Integer::from(&numerator / denominator);
        let remainder = Integer::from(&numerator % denominator);
        let floor = quotient.to_u64().unwrap();
        let ceil = floor + u64::from(remainder != 0);
        let label = format!("mul-div#{index}");
        stats.attempted += 1;
        if mul_div_floor(a, b, denominator) == Ok(floor)
            && mul_div_ceil(a, b, denominator) == Ok(ceil)
        {
            stats.resolved += 1;
            stats.max_width = stats.max_width.max(u128::from(ceil - floor));
        } else {
            stats.violations += 1;
            failures.push(format!("exact {label}: mismatch"));
        }
    }

    for index in 0..512_u64 {
        let value = next_u64(&mut state);
        let scale = next_u64(&mut state) | 1;
        let radicand = Integer::from(value) * Integer::from(scale);
        let root = radicand.sqrt().to_u64().unwrap();
        let exact = Integer::from(root) * Integer::from(root)
            == Integer::from(value) * Integer::from(scale);
        let ceil = root + u64::from(!exact);
        let label = format!("sqrt#{index}");
        stats.attempted += 1;
        if sqrt_floor(value, scale) == Ok(root) && sqrt_ceil(value, scale) == Ok(ceil) {
            stats.resolved += 1;
            stats.max_width = stats.max_width.max(u128::from(ceil - root));
        } else {
            stats.violations += 1;
            failures.push(format!("exact {label}: mismatch"));
        }
    }

    expect_error(
        stats,
        "mul-div zero",
        mul_div_floor(u64::MAX, u64::MAX, 0),
        MathError::DivByZero,
        failures,
    );
    expect_error(
        stats,
        "sqrt zero scale",
        sqrt_floor(u64::MAX, 0),
        MathError::DivByZero,
        failures,
    );
}

fn main() -> ExitCode {
    let mut failures = Vec::new();
    let mut exact = Stats::new("exact");
    let mut exp2 = Stats::new("exp2");
    let mut log2 = Stats::new("log2");
    let mut pow = Stats::new("pow");
    let mut powi = Stats::new("powi");
    let mut compound = Stats::new("compound");

    check_exact(&mut exact, &mut failures);
    check_exp2(&mut exp2, &mut failures);
    check_log2(&mut log2, &mut failures);
    check_pow(&mut pow, &mut failures);
    check_powi(&mut powi, &mut failures);
    check_compound(&mut compound, &mut failures);

    let stats = [exact, exp2, log2, pow, powi, compound];
    println!(
        "family       attempted resolved violations unresolved skipped error_checks max_width"
    );
    for family in &stats {
        println!(
            "{:<12} {:>9} {:>8} {:>10} {:>10} {:>7} {:>12} {}",
            family.name,
            family.attempted,
            family.resolved,
            family.violations,
            family.unresolved,
            family.skipped,
            family.error_checks,
            family.max_width,
        );
    }

    let minimums = [
        ("exp2", stats[1].resolved, 8_192),
        ("log2", stats[2].resolved, 32_768),
        ("pow", stats[3].resolved, 256),
        ("powi", stats[4].resolved, 4_096),
        ("compound", stats[5].resolved, 4_096),
    ];
    for (name, resolved, minimum) in minimums {
        if resolved < minimum {
            failures.push(format!(
                "{name}: resolved {resolved} cases, below required {minimum}"
            ));
        }
    }
    if stats
        .iter()
        .any(|family| family.unresolved != 0 || family.skipped != 0)
    {
        failures.push("oracle reported unresolved or skipped cases".to_owned());
    }
    for (name, width, maximum) in [
        ("powi", stats[4].max_width, 1_u128 << 61),
        ("compound", stats[5].max_width, 4_u128),
    ] {
        if width > maximum {
            failures.push(format!(
                "{name}: maximum directed width {width} exceeds budget {maximum}"
            ));
        }
    }

    if failures.is_empty() {
        println!("oracle_status=pass unresolved=0 skipped=0");
        ExitCode::SUCCESS
    } else {
        eprintln!("oracle_status=fail failures={}", failures.len());
        for failure in failures.iter().take(100) {
            eprintln!("{failure}");
        }
        if failures.len() > 100 {
            eprintln!("... {} additional failures", failures.len() - 100);
        }
        ExitCode::FAILURE
    }
}
