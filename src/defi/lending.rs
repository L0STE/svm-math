//! Lending calculations.

use crate::{
    kernel::wide::{mul_div_ceil, mul_div_floor},
    MathError,
};

use super::BPS_SCALE;

/// Returns utilization in basis points.
///
/// An empty market has zero utilization. Nonzero borrowing with zero supply is
/// undefined and returns [`MathError::DivByZero`].
pub fn utilization_bps(borrowed: u64, supplied: u64) -> Result<u64, MathError> {
    if supplied == 0 {
        return if borrowed == 0 {
            Ok(0)
        } else {
            Err(MathError::DivByZero)
        };
    }
    if borrowed > supplied {
        return Err(MathError::OutOfDomain);
    }
    mul_div_floor(borrowed, BPS_SCALE, supplied)
}

/// Returns the upward-rounded two-leg borrow rate in basis points.
pub fn borrow_rate_bps(
    utilization_bps: u64,
    base_bps: u64,
    slope_before_kink_bps: u64,
    slope_after_kink_bps: u64,
    kink_bps: u64,
) -> Result<u64, MathError> {
    if utilization_bps > BPS_SCALE || kink_bps == 0 || kink_bps >= BPS_SCALE {
        return Err(MathError::OutOfDomain);
    }

    let variable = if utilization_bps <= kink_bps {
        mul_div_ceil(slope_before_kink_bps, utilization_bps, kink_bps)?
    } else {
        let after_kink = mul_div_ceil(
            slope_after_kink_bps,
            utilization_bps - kink_bps,
            BPS_SCALE - kink_bps,
        )?;
        slope_before_kink_bps
            .checked_add(after_kink)
            .ok_or(MathError::Overflow)?
    };

    base_bps.checked_add(variable).ok_or(MathError::Overflow)
}
