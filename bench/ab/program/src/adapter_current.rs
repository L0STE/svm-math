use svm_math::defi::{amm, fee, lending, oracle, schedule, staking};

pub fn mul_div_floor(a: u64, b: u64, denominator: u64) -> u64 {
    svm_math::mul_div_floor(a, b, denominator).unwrap()
}

pub fn mul_div_ceil(a: u64, b: u64, denominator: u64) -> u64 {
    svm_math::mul_div_ceil(a, b, denominator).unwrap()
}

pub fn isqrt(value: u128) -> u64 {
    svm_math::isqrt(value) as u64
}

pub fn sqrt_floor(value: u64, scale: u64) -> u64 {
    svm_math::sqrt_floor(value, scale).unwrap()
}

pub fn sqrt_ceil(value: u64, scale: u64) -> u64 {
    svm_math::sqrt_ceil(value, scale).unwrap()
}

pub fn exp2_lower(value: i64, scale: u64) -> u64 {
    svm_math::exp2_lower(value, scale).unwrap()
}

pub fn exp2_upper(value: i64, scale: u64) -> u64 {
    svm_math::exp2_upper(value, scale).unwrap()
}

pub fn log2_lower(value: u64, scale: u64) -> u64 {
    svm_math::log2_lower(value, scale).unwrap() as u64
}

pub fn log2_upper(value: u64, scale: u64) -> u64 {
    svm_math::log2_upper(value, scale).unwrap() as u64
}

pub fn pow_lower(base: u64, exponent: u64, scale: u64) -> u64 {
    svm_math::pow_lower(base, exponent, scale).unwrap()
}

pub fn pow_upper(base: u64, exponent: u64, scale: u64) -> u64 {
    svm_math::pow_upper(base, exponent, scale).unwrap()
}

pub fn powi_lower(base: u64, exponent: u64, scale: u64) -> u64 {
    svm_math::powi_lower(base, exponent, scale).unwrap()
}

pub fn powi_upper(base: u64, exponent: u64, scale: u64) -> u64 {
    svm_math::powi_upper(base, exponent, scale).unwrap()
}

pub fn compound_lower(rate: u64, periods: u64, elapsed: u64, scale: u64) -> u64 {
    svm_math::compound_lower(rate, periods, elapsed, scale).unwrap()
}

pub fn compound_upper(rate: u64, periods: u64, elapsed: u64, scale: u64) -> u64 {
    svm_math::compound_upper(rate, periods, elapsed, scale).unwrap()
}

pub fn net_of_fee(amount: u64, fee_bps: u16) -> u64 {
    let (net, fee) = fee::net_of_fee(amount, fee_bps).unwrap();
    net ^ fee
}

pub fn quote_exact_in(reserve_in: u64, reserve_out: u64, amount: u64, fee_bps: u16) -> u64 {
    amm::quote_exact_in(reserve_in, reserve_out, amount, fee_bps).unwrap()
}

pub fn quote_exact_out(reserve_in: u64, reserve_out: u64, amount: u64, fee_bps: u16) -> u64 {
    amm::quote_exact_out(reserve_in, reserve_out, amount, fee_bps).unwrap()
}

pub fn initial_lp_shares(a: u64, b: u64) -> u64 {
    amm::initial_lp_shares_floor(a, b)
}

pub fn utilization_bps(borrowed: u64, supplied: u64) -> u64 {
    lending::utilization_bps(borrowed, supplied).unwrap()
}

pub fn borrow_rate_bps(utilization: u64, base: u64, before: u64, after: u64, kink: u64) -> u64 {
    lending::borrow_rate_bps(utilization, base, before, after, kink).unwrap()
}

pub fn reward_index_lower(index: u64, reward: u64, stake: u64, scale: u64) -> u64 {
    staking::reward_index_accrue_lower(index, reward, stake, scale).unwrap()
}

pub fn reward_index_upper(index: u64, reward: u64, stake: u64, scale: u64) -> u64 {
    staking::reward_index_accrue_upper(index, reward, stake, scale).unwrap()
}

pub fn rewards_owed(staked: u64, now: u64, snapshot: u64, scale: u64) -> u64 {
    staking::rewards_owed_floor(staked, now, snapshot, scale).unwrap()
}

pub fn oracle_bounds(price: i64, confidence: u64, exponent: i32, scale: u64) -> u64 {
    let (lower, upper) = oracle::price_bounds_scaled(price, confidence, exponent, scale).unwrap();
    lower ^ upper
}

pub fn oracle_lower(price: i64, confidence: u64, exponent: i32, scale: u64) -> u64 {
    oracle::price_bounds_scaled(price, confidence, exponent, scale)
        .unwrap()
        .0
}

pub fn oracle_upper(price: i64, confidence: u64, exponent: i32, scale: u64) -> u64 {
    oracle::price_bounds_scaled(price, confidence, exponent, scale)
        .unwrap()
        .1
}

pub fn vested(total: u64, start: u64, cliff: u64, duration: u64, now: u64) -> u64 {
    schedule::vested_floor(total, start, cliff, duration, now).unwrap()
}

pub fn interp_floor(from: u64, to: u64, elapsed: u64, duration: u64) -> u64 {
    schedule::linear_interp_floor(from, to, elapsed, duration).unwrap()
}

pub fn interp_ceil(from: u64, to: u64, elapsed: u64, duration: u64) -> u64 {
    schedule::linear_interp_ceil(from, to, elapsed, duration).unwrap()
}
