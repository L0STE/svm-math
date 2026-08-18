//! Staking reward calculations.

use crate::{
    kernel::wide::{ceil_from_quotient_remainder, mul_div, mul_div_ceil, mul_div_floor},
    MathError,
};

fn require_scale(scale: u64) -> Result<(), MathError> {
    if scale == 0 {
        Err(MathError::DivByZero)
    } else {
        Ok(())
    }
}

/// Accrues the lower endpoint of a scaled reward index.
pub fn reward_index_accrue_lower(
    index_lower: u64,
    reward: u64,
    total_staked: u64,
    scale: u64,
) -> Result<u64, MathError> {
    require_scale(scale)?;
    if total_staked == 0 {
        return Ok(index_lower);
    }
    let delta = mul_div_floor(reward, scale, total_staked)?;
    index_lower.checked_add(delta).ok_or(MathError::Overflow)
}

/// Accrues the upper endpoint of a scaled reward index.
pub fn reward_index_accrue_upper(
    index_upper: u64,
    reward: u64,
    total_staked: u64,
    scale: u64,
) -> Result<u64, MathError> {
    require_scale(scale)?;
    if total_staked == 0 {
        return Ok(index_upper);
    }
    let delta = mul_div_ceil(reward, scale, total_staked)?;
    index_upper.checked_add(delta).ok_or(MathError::Overflow)
}

/// Accrues both endpoints of a scaled reward index from one division;
/// exactly `(reward_index_accrue_lower, reward_index_accrue_upper)`.
pub fn reward_index_accrue(
    index_lower: u64,
    index_upper: u64,
    reward: u64,
    total_staked: u64,
    scale: u64,
) -> Result<(u64, u64), MathError> {
    require_scale(scale)?;
    if total_staked == 0 {
        return Ok((index_lower, index_upper));
    }
    let (delta_floor, remainder) = mul_div(reward, scale, total_staked)?;
    let delta_ceil = ceil_from_quotient_remainder(delta_floor, remainder)?;
    Ok((
        index_lower
            .checked_add(delta_floor)
            .ok_or(MathError::Overflow)?,
        index_upper
            .checked_add(delta_ceil)
            .ok_or(MathError::Overflow)?,
    ))
}

/// Returns the floor of rewards owed without paying above the lower index.
pub fn rewards_owed_floor(
    staked: u64,
    index_now_lower: u64,
    snapshot_upper: u64,
    scale: u64,
) -> Result<u64, MathError> {
    require_scale(scale)?;
    let delta = index_now_lower.saturating_sub(snapshot_upper);
    mul_div_floor(staked, delta, scale)
}
