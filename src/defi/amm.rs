//! Constant-product AMM calculations.

use crate::{
    kernel::{
        sqrt::isqrt,
        wide::{mul_div_ceil, mul_div_floor},
    },
    MathError,
};

use super::{fee::net_of_fee, BPS_SCALE};

/// Returns the output of a constant-product exact-input swap.
pub fn quote_exact_in(
    reserve_in: u64,
    reserve_out: u64,
    amount_in: u64,
    fee_bps: u16,
) -> Result<u64, MathError> {
    if reserve_in == 0 || reserve_out == 0 {
        return Err(MathError::OutOfDomain);
    }
    let (net, _) = net_of_fee(amount_in, fee_bps)?;
    let denominator = reserve_in.checked_add(net).ok_or(MathError::Overflow)?;
    mul_div_floor(reserve_out, net, denominator)
}

/// Returns the least gross input whose exact-input replay reaches `amount_out`.
pub fn quote_exact_out(
    reserve_in: u64,
    reserve_out: u64,
    amount_out: u64,
    fee_bps: u16,
) -> Result<u64, MathError> {
    let fee = u64::from(fee_bps);
    if fee > BPS_SCALE || reserve_in == 0 || reserve_out == 0 {
        return Err(MathError::OutOfDomain);
    }
    if amount_out == 0 {
        return Ok(0);
    }
    if amount_out >= reserve_out || fee == BPS_SCALE {
        return Err(MathError::OutOfDomain);
    }

    let remaining_out = reserve_out - amount_out;
    let required_net = mul_div_ceil(reserve_in, amount_out, remaining_out)?;
    let gross = mul_div_ceil(required_net, BPS_SCALE, BPS_SCALE - fee)?;
    // Replayability guard: the forward quote divides by reserve_in plus the
    // exact net of this gross input, so that sum must fit u64 for the
    // exact-in replay that defines this quote's minimality to exist.
    let (actual_net, _) = net_of_fee(gross, fee_bps)?;
    reserve_in
        .checked_add(actual_net)
        .ok_or(MathError::Overflow)?;
    Ok(gross)
}

/// Returns `floor(sqrt(deposit_a * deposit_b))`.
pub fn initial_lp_shares_floor(deposit_a: u64, deposit_b: u64) -> u64 {
    isqrt(u128::from(deposit_a) * u128::from(deposit_b)) as u64
}
