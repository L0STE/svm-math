use svm_math::{
    compound_lower, compound_upper, exp2_lower, exp2_upper, isqrt, log2_lower, log2_upper,
    mul_div_ceil, mul_div_floor, pow_lower, pow_upper, powi_lower, powi_upper, sqrt_ceil,
    sqrt_floor, MathError,
};

const SCALE: u64 = 1_000_000;

fn main() -> Result<(), MathError> {
    let one_third = mul_div_floor(SCALE, 1, 3)?;
    assert_eq!(one_third, 333_333);
    assert_eq!(mul_div_ceil(SCALE, 1, 3)?, 333_334);
    assert_eq!(isqrt(10), 3);

    let root_two_lower = sqrt_floor(2 * SCALE, SCALE)?;
    let root_two_upper = sqrt_ceil(2 * SCALE, SCALE)?;
    assert_eq!((root_two_lower, root_two_upper), (1_414_213, 1_414_214));

    let exp_lower = exp2_lower((SCALE / 2) as i64, SCALE)?;
    let exp_upper = exp2_upper((SCALE / 2) as i64, SCALE)?;
    assert!(exp_lower <= root_two_upper && exp_upper >= root_two_lower);

    let log_lower = log2_lower(2 * SCALE, SCALE)?;
    let log_upper = log2_upper(2 * SCALE, SCALE)?;
    assert!(log_lower <= SCALE as i64 && log_upper >= SCALE as i64);

    let power_lower = pow_lower(2 * SCALE, SCALE / 2, SCALE)?;
    let power_upper = pow_upper(2 * SCALE, SCALE / 2, SCALE)?;
    assert!(power_lower <= root_two_upper && power_upper >= root_two_lower);
    assert_eq!(powi_lower(2 * SCALE, 3, SCALE)?, 8 * SCALE);
    assert_eq!(powi_upper(2 * SCALE, 3, SCALE)?, 8 * SCALE);

    let growth_lower = compound_lower(50_000, 365, 365, SCALE)?;
    let growth_upper = compound_upper(50_000, 365, 365, SCALE)?;
    assert!(growth_lower > SCALE && growth_lower <= growth_upper);
    Ok(())
}
