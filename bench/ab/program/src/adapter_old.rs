use svm_math::{
    defi::{
        amm,
        fee::{self, FeeRate},
        lending::{self, KinkedRateConfig, KinkedRateModel},
        oracle, schedule,
        staking::{self, RewardIndex},
    },
    Amount, Compounding, ExactValue, Interval, LowerBound, UpperBound,
};

fn interval_from_scaled(value: u64, decimals: u8) -> Interval {
    Interval::try_from_bounds(
        LowerBound::try_from_scaled(u128::from(value), decimals).unwrap(),
        UpperBound::try_from_scaled(u128::from(value), decimals).unwrap(),
    )
    .unwrap()
}

fn reward_index(value: u64) -> RewardIndex<6> {
    RewardIndex::from_raw_interval(interval_from_scaled(value, 9))
}

pub fn mul_div_floor(a: u64, b: u64, denominator: u64) -> u64 {
    svm_math::mul_div_floor(a, b, denominator).unwrap()
}

pub fn mul_div_ceil(a: u64, b: u64, denominator: u64) -> u64 {
    svm_math::mul_div_ceil(a, b, denominator).unwrap()
}

pub fn isqrt(value: u128) -> u64 {
    svm_math::isqrt(value) as u64
}

pub fn sqrt_floor(value: u64, _scale: u64) -> u64 {
    LowerBound::try_from_scaled(u128::from(value), 9)
        .unwrap()
        .sqrt()
        .to_scaled_floor(9)
        .unwrap() as u64
}

pub fn sqrt_ceil(value: u64, _scale: u64) -> u64 {
    UpperBound::try_from_scaled(u128::from(value), 9)
        .unwrap()
        .sqrt()
        .to_scaled_ceil(9)
        .unwrap() as u64
}

pub fn exp2_lower(value: i64, _scale: u64) -> u64 {
    LowerBound::try_from_scaled(value as u128, 9)
        .unwrap()
        .exp2()
        .unwrap()
        .to_scaled_floor(9)
        .unwrap() as u64
}

pub fn exp2_upper(value: i64, _scale: u64) -> u64 {
    UpperBound::try_from_scaled(value as u128, 9)
        .unwrap()
        .exp2()
        .unwrap()
        .to_scaled_ceil(9)
        .unwrap() as u64
}

pub fn log2_lower(value: u64, _scale: u64) -> u64 {
    LowerBound::from(value)
        .log2()
        .unwrap()
        .to_i64_floor()
        .unwrap() as u64
}

pub fn log2_upper(value: u64, _scale: u64) -> u64 {
    UpperBound::from(value)
        .log2()
        .unwrap()
        .to_i64_ceil()
        .unwrap() as u64
}

pub fn pow_lower(base: u64, exponent: u64, scale: u64) -> u64 {
    let exponent = ExactValue::try_from_ratio(exponent, scale).unwrap();
    LowerBound::try_from_scaled(u128::from(base), 9)
        .unwrap()
        .pow(exponent)
        .unwrap()
        .to_scaled_floor(9)
        .unwrap() as u64
}

pub fn pow_upper(base: u64, exponent: u64, scale: u64) -> u64 {
    let exponent = ExactValue::try_from_ratio(exponent, scale).unwrap();
    UpperBound::try_from_scaled(u128::from(base), 9)
        .unwrap()
        .pow(exponent)
        .unwrap()
        .to_scaled_ceil(9)
        .unwrap() as u64
}

pub fn powi_lower(base: u64, exponent: u64, _scale: u64) -> u64 {
    LowerBound::try_from_scaled(u128::from(base), 9)
        .unwrap()
        .powi(exponent)
        .unwrap()
        .to_scaled_floor(9)
        .unwrap() as u64
}

pub fn powi_upper(base: u64, exponent: u64, _scale: u64) -> u64 {
    UpperBound::try_from_scaled(u128::from(base), 9)
        .unwrap()
        .powi(exponent)
        .unwrap()
        .to_scaled_ceil(9)
        .unwrap() as u64
}

pub fn compound_lower(rate: u64, periods: u64, elapsed: u64, _scale: u64) -> u64 {
    interval_from_scaled(rate, 9)
        .compound(Compounding {
            periods_per_year: periods,
            elapsed_periods: elapsed,
        })
        .unwrap()
        .lower()
        .to_scaled_floor(9)
        .unwrap() as u64
}

pub fn compound_upper(rate: u64, periods: u64, elapsed: u64, _scale: u64) -> u64 {
    interval_from_scaled(rate, 9)
        .compound(Compounding {
            periods_per_year: periods,
            elapsed_periods: elapsed,
        })
        .unwrap()
        .upper()
        .to_scaled_ceil(9)
        .unwrap() as u64
}

pub fn net_of_fee(amount: u64, fee_bps: u16) -> u64 {
    let parts = fee::net_of_fee(
        Amount::<6>::new(amount),
        FeeRate::try_from_bps(fee_bps).unwrap(),
    )
    .unwrap();
    parts.net.raw() ^ parts.fee.raw()
}

pub fn quote_exact_in(reserve_in: u64, reserve_out: u64, amount: u64, fee_bps: u16) -> u64 {
    amm::quote_exact_in(
        Amount::<6>::new(reserve_in),
        Amount::<9>::new(reserve_out),
        Amount::<6>::new(amount),
        FeeRate::try_from_bps(fee_bps).unwrap(),
    )
    .unwrap()
    .raw()
}

pub fn quote_exact_out(reserve_in: u64, reserve_out: u64, amount: u64, fee_bps: u16) -> u64 {
    amm::quote_exact_out(
        Amount::<6>::new(reserve_in),
        Amount::<9>::new(reserve_out),
        Amount::<9>::new(amount),
        FeeRate::try_from_bps(fee_bps).unwrap(),
    )
    .unwrap()
    .raw()
}

pub fn initial_lp_shares(a: u64, b: u64) -> u64 {
    amm::initial_lp_shares(Amount::<6>::new(a), Amount::<6>::new(b))
}

pub fn utilization_bps(borrowed: u64, supplied: u64) -> u64 {
    lending::utilization_bps(borrowed, supplied).unwrap()
}

pub fn borrow_rate_bps(utilization: u64, base: u64, before: u64, after: u64, kink: u64) -> u64 {
    KinkedRateModel::try_new(KinkedRateConfig {
        base_bps: base,
        slope_before_kink_bps: before,
        slope_after_kink_bps: after,
        kink_bps: kink,
    })
    .unwrap()
    .borrow_rate_bps(utilization)
    .unwrap()
}

pub fn reward_index_lower(index: u64, reward: u64, stake: u64, _scale: u64) -> u64 {
    staking::reward_index_accrue(reward_index(index), Amount::<6>::new(reward), stake)
        .unwrap()
        .into_raw_interval()
        .lower()
        .to_scaled_floor(9)
        .unwrap() as u64
}

pub fn reward_index_upper(index: u64, reward: u64, stake: u64, _scale: u64) -> u64 {
    staking::reward_index_accrue(reward_index(index), Amount::<6>::new(reward), stake)
        .unwrap()
        .into_raw_interval()
        .upper()
        .to_scaled_ceil(9)
        .unwrap() as u64
}

pub fn rewards_owed(staked: u64, now: u64, snapshot: u64, _scale: u64) -> u64 {
    staking::rewards_owed(staked, reward_index(now), reward_index(snapshot))
        .unwrap()
        .raw()
}

pub fn oracle_bounds(price: i64, confidence: u64, exponent: i32, _scale: u64) -> u64 {
    let bounds = oracle::oracle_price_scaled(price, confidence, exponent).unwrap();
    bounds.lower().to_scaled_floor(9).unwrap() as u64
        ^ bounds.upper().to_scaled_ceil(9).unwrap() as u64
}

pub fn oracle_lower(price: i64, confidence: u64, exponent: i32, _scale: u64) -> u64 {
    oracle::oracle_price_scaled(price, confidence, exponent)
        .unwrap()
        .lower()
        .to_scaled_floor(9)
        .unwrap() as u64
}

pub fn oracle_upper(price: i64, confidence: u64, exponent: i32, _scale: u64) -> u64 {
    oracle::oracle_price_scaled(price, confidence, exponent)
        .unwrap()
        .upper()
        .to_scaled_ceil(9)
        .unwrap() as u64
}

pub fn vested(total: u64, start: u64, cliff: u64, duration: u64, now: u64) -> u64 {
    schedule::VestingSchedule::try_new(schedule::VestingConfig {
        start,
        cliff,
        duration,
    })
    .unwrap()
    .vested(Amount::<6>::new(total), now)
    .unwrap()
    .raw()
}

pub fn interp_floor(from: u64, to: u64, elapsed: u64, duration: u64) -> u64 {
    schedule::linear_interp_floor(from, to, elapsed, duration).unwrap()
}

pub fn interp_ceil(from: u64, to: u64, elapsed: u64, duration: u64) -> u64 {
    schedule::linear_interp_ceil(from, to, elapsed, duration).unwrap()
}
