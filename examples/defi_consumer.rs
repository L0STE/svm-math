use svm_math::{defi, MathError};

fn main() -> Result<(), MathError> {
    let (net, fee) = defi::fee::net_of_fee(1_000_000, 30)?;
    assert_eq!(net + fee, 1_000_000);

    let output = defi::amm::quote_exact_in(5_000_000, 4_000_000, 250_000, 30)?;
    let replay_input = defi::amm::quote_exact_out(5_000_000, 4_000_000, output, 30)?;
    assert!(replay_input <= 250_000);
    assert_eq!(defi::amm::initial_lp_shares_floor(9, 16), 12);

    assert_eq!(defi::lending::utilization_bps(1, 2)?, 5_000);
    assert_eq!(
        defi::lending::borrow_rate_bps(9_000, 200, 400, 6_000, 8_000)?,
        3_600
    );

    let index_lower = defi::staking::reward_index_accrue_lower(10, 1, 3, 10)?;
    let index_upper = defi::staking::reward_index_accrue_upper(10, 1, 3, 10)?;
    assert_eq!((index_lower, index_upper), (13, 14));
    assert_eq!(
        defi::staking::rewards_owed_floor(30, index_lower, 10, 10)?,
        9
    );

    assert_eq!(
        defi::oracle::price_bounds_scaled(123, 3, -2, 100)?,
        (120, 126)
    );
    assert_eq!(defi::schedule::vested_floor(900, 100, 150, 300, 200)?, 300);
    assert_eq!(defi::schedule::linear_interp_floor(100, 0, 1, 3)?, 66);
    assert_eq!(defi::schedule::linear_interp_ceil(100, 0, 1, 3)?, 67);
    Ok(())
}
