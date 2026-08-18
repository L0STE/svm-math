use svm_math::{defi, MathError};

type U64Result = Result<u64, MathError>;
type I64Result = Result<i64, MathError>;
type PairResult = Result<(u64, u64), MathError>;
type SignedPairResult = Result<(i64, i64), MathError>;

#[test]
fn public_signatures_are_target_shaped() {
    let _: fn(u64, u64, u64) -> U64Result = svm_math::mul_div_floor;
    let _: fn(u64, u64, u64) -> U64Result = svm_math::mul_div_ceil;
    let _: fn(u128) -> u128 = svm_math::isqrt;
    let _: fn(u64, u64) -> U64Result = svm_math::sqrt_floor;
    let _: fn(u64, u64) -> U64Result = svm_math::sqrt_ceil;
    let _: fn(i64, u64) -> U64Result = svm_math::exp2_lower;
    let _: fn(i64, u64) -> U64Result = svm_math::exp2_upper;
    let _: fn(u64, u64) -> I64Result = svm_math::log2_lower;
    let _: fn(u64, u64) -> I64Result = svm_math::log2_upper;
    let _: fn(u64, u64, u64) -> U64Result = svm_math::pow_lower;
    let _: fn(u64, u64, u64) -> U64Result = svm_math::pow_upper;
    let _: fn(u64, u64, u64) -> U64Result = svm_math::powi_lower;
    let _: fn(u64, u64, u64) -> U64Result = svm_math::powi_upper;
    let _: fn(u64, u64, u64, u64) -> U64Result = svm_math::compound_lower;
    let _: fn(u64, u64, u64, u64) -> U64Result = svm_math::compound_upper;
    let _: fn(i64, u64) -> PairResult = svm_math::exp2_bounds;
    let _: fn(u64, u64) -> SignedPairResult = svm_math::log2_bounds;
    let _: fn(u64, u64, u64) -> PairResult = svm_math::pow_bounds;
    let _: fn(u64, u64, u64, u64) -> PairResult = svm_math::compound_bounds;

    let _: fn(u64, u16) -> PairResult = defi::fee::net_of_fee;
    let _: fn(u64, u64, u64, u16) -> U64Result = defi::amm::quote_exact_in;
    let _: fn(u64, u64, u64, u16) -> U64Result = defi::amm::quote_exact_out;
    let _: fn(u64, u64) -> u64 = defi::amm::initial_lp_shares_floor;
    let _: fn(u64, u64) -> U64Result = defi::lending::utilization_bps;
    let _: fn(u64, u64, u64, u64, u64) -> U64Result = defi::lending::borrow_rate_bps;
    let _: fn(u64, u64, u64, u64) -> U64Result = defi::staking::reward_index_accrue_lower;
    let _: fn(u64, u64, u64, u64) -> U64Result = defi::staking::reward_index_accrue_upper;
    let _: fn(u64, u64, u64, u64, u64) -> PairResult = defi::staking::reward_index_accrue;
    let _: fn(u64, u64, u64, u64) -> U64Result = defi::staking::rewards_owed_floor;
    let _: fn(i64, u64, i32, u64) -> PairResult = defi::oracle::price_bounds_scaled;
    let _: fn(u64, u64, u64, u64, u64) -> U64Result = defi::schedule::vested_floor;
    let _: fn(u64, u64, u64, u64) -> U64Result = defi::schedule::linear_interp_floor;
    let _: fn(u64, u64, u64, u64) -> U64Result = defi::schedule::linear_interp_ceil;
}
