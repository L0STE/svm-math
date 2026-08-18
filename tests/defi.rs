use svm_math::{defi, MathError};

#[test]
fn fee_split_is_exact_and_minimal() {
    for amount in 0_u64..=128 {
        for fee_bps in [0_u16, 1, 30, 9_999, 10_000] {
            let (net, fee) = defi::fee::net_of_fee(amount, fee_bps).unwrap();
            assert_eq!(net + fee, amount);
            assert!(u128::from(fee) * 10_000 >= u128::from(amount) * u128::from(fee_bps));
            if fee > 0 {
                assert!(u128::from(fee - 1) * 10_000 < u128::from(amount) * u128::from(fee_bps));
            }
        }
    }
    assert_eq!(
        defi::fee::net_of_fee(1, 10_001),
        Err(MathError::OutOfDomain)
    );
}

#[test]
fn exact_input_preserves_the_constant_product() {
    for reserve_in in 1_u64..16 {
        for reserve_out in 1_u64..16 {
            for amount_in in 0_u64..16 {
                let (net, _) = defi::fee::net_of_fee(amount_in, 30).unwrap();
                let out =
                    defi::amm::quote_exact_in(reserve_in, reserve_out, amount_in, 30).unwrap();
                assert!(out <= reserve_out);
                assert!(
                    u128::from(reserve_in + net) * u128::from(reserve_out - out)
                        >= u128::from(reserve_in) * u128::from(reserve_out)
                );
                if out < reserve_out {
                    assert!(
                        u128::from(out + 1) * u128::from(reserve_in + net)
                            > u128::from(reserve_out) * u128::from(net)
                    );
                }
            }
        }
    }
    assert_eq!(
        defi::amm::quote_exact_in(0, 1, 1, 0),
        Err(MathError::OutOfDomain)
    );
}

#[test]
fn exact_output_is_the_least_replay_input() {
    for reserve_in in 1_u64..16 {
        for reserve_out in 2_u64..16 {
            for amount_out in 1..reserve_out {
                for fee_bps in [0_u16, 30, 500] {
                    let gross =
                        defi::amm::quote_exact_out(reserve_in, reserve_out, amount_out, fee_bps)
                            .unwrap();
                    let replay =
                        defi::amm::quote_exact_in(reserve_in, reserve_out, gross, fee_bps).unwrap();
                    assert!(replay >= amount_out);
                    if gross > 0 {
                        let prior =
                            defi::amm::quote_exact_in(reserve_in, reserve_out, gross - 1, fee_bps)
                                .unwrap();
                        assert!(prior < amount_out);
                    }
                }
            }
        }
    }
    assert_eq!(defi::amm::quote_exact_out(100, 100, 0, 10_000), Ok(0));
    assert_eq!(
        defi::amm::quote_exact_out(0, 100, 1, 0),
        Err(MathError::OutOfDomain)
    );
}

#[test]
fn initial_shares_are_the_exact_floor_root() {
    for a in 0_u64..64 {
        for b in 0_u64..64 {
            let root = defi::amm::initial_lp_shares_floor(a, b);
            let product = u128::from(a) * u128::from(b);
            assert!(u128::from(root) * u128::from(root) <= product);
            assert!(u128::from(root + 1) * u128::from(root + 1) > product);
        }
    }
    assert_eq!(
        defi::amm::initial_lp_shares_floor(u64::MAX, u64::MAX),
        u64::MAX
    );
}

#[test]
fn lending_domains_and_kink_are_explicit() {
    assert_eq!(defi::lending::utilization_bps(0, 0), Ok(0));
    assert_eq!(
        defi::lending::utilization_bps(1, 0),
        Err(MathError::DivByZero)
    );
    assert_eq!(defi::lending::utilization_bps(1, 2), Ok(5_000));
    assert_eq!(
        defi::lending::utilization_bps(2, 1),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        defi::lending::utilization_bps(u64::MAX, 1),
        Err(MathError::OutOfDomain)
    );

    assert_eq!(
        defi::lending::borrow_rate_bps(4_000, 100, 500, 2_000, 8_000),
        Ok(350)
    );
    assert_eq!(
        defi::lending::borrow_rate_bps(8_000, 100, 500, 2_000, 8_000),
        Ok(600)
    );
    assert_eq!(
        defi::lending::borrow_rate_bps(9_000, 100, 500, 2_000, 8_000),
        Ok(1_600)
    );
    assert_eq!(defi::lending::borrow_rate_bps(1, 0, 1, 1, 3), Ok(1));
    assert_eq!(
        defi::lending::borrow_rate_bps(10_001, 0, 1, 1, 8_000),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        defi::lending::borrow_rate_bps(1, 0, 1, 1, 0),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        defi::lending::borrow_rate_bps(1, u64::MAX, 1, 1, 8_000),
        Err(MathError::Overflow)
    );
}

#[test]
fn staking_endpoints_round_outward() {
    assert_eq!(
        defi::staking::reward_index_accrue_lower(10, 1, 3, 10),
        Ok(13)
    );
    assert_eq!(
        defi::staking::reward_index_accrue_upper(10, 1, 3, 10),
        Ok(14)
    );
    assert_eq!(
        defi::staking::reward_index_accrue_lower(10, 7, 0, 10),
        Ok(10)
    );
    assert_eq!(defi::staking::rewards_owed_floor(3, 14, 10, 10), Ok(1));
    assert_eq!(defi::staking::rewards_owed_floor(3, 9, 10, 10), Ok(0));
    assert_eq!(
        defi::staking::reward_index_accrue_lower(u64::MAX, 1, 1, 1),
        Err(MathError::Overflow)
    );
    assert_eq!(
        defi::staking::reward_index_accrue_lower(0, 0, 0, 0),
        Err(MathError::DivByZero)
    );
}

#[test]
fn oracle_bounds_are_outward() {
    assert_eq!(
        defi::oracle::price_bounds_scaled(123, 3, 2, 1),
        Ok((12_000, 12_600))
    );
    assert_eq!(
        defi::oracle::price_bounds_scaled(123, 3, -2, 100),
        Ok((120, 126))
    );
    assert_eq!(defi::oracle::price_bounds_scaled(2, 3, 0, 1), Ok((0, 5)));
    assert_eq!(
        defi::oracle::price_bounds_scaled(-1, 0, 0, 1),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        defi::oracle::price_bounds_scaled(1, 0, 0, 0),
        Err(MathError::DivByZero)
    );
    assert_eq!(
        defi::oracle::price_bounds_scaled(i64::MAX, u64::MAX, -38, u64::MAX),
        Ok((0, 6))
    );
    assert_eq!(
        defi::oracle::price_bounds_scaled(
            9_000_000_000_000_000_000,
            1_000_000_000_000_000_000,
            -19,
            1_000_000_000_000_000_000,
        ),
        Ok((800_000_000_000_000_000, 1_000_000_000_000_000_000))
    );
    assert_eq!(
        defi::oracle::price_bounds_scaled(
            9_000_000_000_000_000_000,
            1_000_000_000_000_000_000,
            -20,
            1_000_000_000_000_000_000,
        ),
        Ok((80_000_000_000_000_000, 100_000_000_000_000_000))
    );

    for price in 0_i64..64 {
        for confidence in 0_u64..64 {
            for output_scale in 1_u64..16 {
                for magnitude in 1_u32..=4 {
                    let denominator = 10_u128.pow(magnitude);
                    let lower_base = u128::from((price as u64).saturating_sub(confidence));
                    let upper_base = u128::from(price as u64) + u128::from(confidence);
                    let lower_numerator = lower_base * u128::from(output_scale);
                    let upper_numerator = upper_base * u128::from(output_scale);
                    let expected = (
                        (lower_numerator / denominator) as u64,
                        upper_numerator.div_ceil(denominator) as u64,
                    );
                    assert_eq!(
                        defi::oracle::price_bounds_scaled(
                            price,
                            confidence,
                            -(magnitude as i32),
                            output_scale,
                        ),
                        Ok(expected)
                    );
                }
            }
        }
    }
}

#[test]
fn schedules_follow_the_exact_line() {
    assert_eq!(defi::schedule::vested_floor(900, 100, 150, 300, 149), Ok(0));
    assert_eq!(
        defi::schedule::vested_floor(900, 100, 150, 300, 200),
        Ok(300)
    );
    assert_eq!(
        defi::schedule::vested_floor(900, 100, 150, 300, 400),
        Ok(900)
    );
    assert_eq!(
        defi::schedule::vested_floor(1, 0, 0, 0, 0),
        Err(MathError::OutOfDomain)
    );
    assert_eq!(
        defi::schedule::vested_floor(9, u64::MAX - 1, 0, 10, u64::MAX),
        Ok(0)
    );

    assert_eq!(defi::schedule::linear_interp_floor(0, 100, 1, 3), Ok(33));
    assert_eq!(defi::schedule::linear_interp_ceil(0, 100, 1, 3), Ok(34));
    assert_eq!(defi::schedule::linear_interp_floor(100, 0, 1, 3), Ok(66));
    assert_eq!(defi::schedule::linear_interp_ceil(100, 0, 1, 3), Ok(67));
    assert_eq!(defi::schedule::linear_interp_floor(0, 100, 3, 3), Ok(100));
    assert_eq!(
        defi::schedule::linear_interp_floor(0, 1, 0, 0),
        Err(MathError::DivByZero)
    );
}

#[test]
fn fused_reward_index_accrue_matches_the_single_direction_pair() {
    let mut state = 0x5354_414b_4544_5045_u64;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state
    };
    for _ in 0..2_048 {
        let scale = 1 + next() % 1_000_000_000_000_000_000;
        let reward = next() % 1_000_000_000_000;
        let total_staked = next() % 1_000_000_000;
        let index_lower = next() % 1_000_000_000_000;
        let index_upper = index_lower + next() % 1_000;
        assert_eq!(
            defi::staking::reward_index_accrue(
                index_lower,
                index_upper,
                reward,
                total_staked,
                scale
            ),
            defi::staking::reward_index_accrue_lower(index_lower, reward, total_staked, scale)
                .and_then(|lower| Ok((
                    lower,
                    defi::staking::reward_index_accrue_upper(
                        index_upper,
                        reward,
                        total_staked,
                        scale
                    )?,
                ))),
            "r={reward} staked={total_staked} S={scale}"
        );
    }
    assert_eq!(
        defi::staking::reward_index_accrue(1, 2, 3, 4, 0),
        Err(MathError::DivByZero)
    );
}
