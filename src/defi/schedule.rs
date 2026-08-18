//! Time-based schedule calculations.

use crate::{
    kernel::wide::{mul_div_ceil, mul_div_floor},
    MathError,
};

/// Returns the floor of vested value under a cliffed linear schedule.
pub fn vested_floor(
    total: u64,
    start: u64,
    cliff: u64,
    duration: u64,
    now: u64,
) -> Result<u64, MathError> {
    if duration == 0 {
        return Err(MathError::OutOfDomain);
    }
    if now < start || now < cliff {
        return Ok(0);
    }
    let elapsed = now - start;
    if elapsed >= duration {
        return Ok(total);
    }
    mul_div_floor(total, elapsed, duration)
}

/// Returns the floor of a clamped linear interpolation.
pub fn linear_interp_floor(
    from: u64,
    to: u64,
    elapsed: u64,
    duration: u64,
) -> Result<u64, MathError> {
    if duration == 0 {
        return Err(MathError::DivByZero);
    }
    if elapsed >= duration {
        return Ok(to);
    }
    if from <= to {
        let step = mul_div_floor(to - from, elapsed, duration)?;
        from.checked_add(step).ok_or(MathError::Overflow)
    } else {
        let step = mul_div_ceil(from - to, elapsed, duration)?;
        from.checked_sub(step).ok_or(MathError::Overflow)
    }
}

/// Returns the ceil of a clamped linear interpolation.
pub fn linear_interp_ceil(
    from: u64,
    to: u64,
    elapsed: u64,
    duration: u64,
) -> Result<u64, MathError> {
    if duration == 0 {
        return Err(MathError::DivByZero);
    }
    if elapsed >= duration {
        return Ok(to);
    }
    if from <= to {
        let step = mul_div_ceil(to - from, elapsed, duration)?;
        from.checked_add(step).ok_or(MathError::Overflow)
    } else {
        let step = mul_div_floor(from - to, elapsed, duration)?;
        from.checked_sub(step).ok_or(MathError::Overflow)
    }
}
