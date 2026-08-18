//! Fee calculations.

use crate::{kernel::wide::mul_div_ceil, MathError};

use super::BPS_SCALE;

pub(super) fn fee_ceil(amount: u64, fee_bps: u16) -> Result<u64, MathError> {
    let fee_bps = u64::from(fee_bps);
    if fee_bps > BPS_SCALE {
        return Err(MathError::OutOfDomain);
    }
    mul_div_ceil(amount, fee_bps, BPS_SCALE)
}

/// Splits `amount` into `(net, fee)` using an upward-rounded basis-point fee.
///
/// The returned values always sum exactly to `amount`.
pub fn net_of_fee(amount: u64, fee_bps: u16) -> Result<(u64, u64), MathError> {
    let fee = fee_ceil(amount, fee_bps)?;
    Ok((amount - fee, fee))
}
