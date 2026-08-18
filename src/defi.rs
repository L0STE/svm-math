//! Stateless fixed-point recipes for common DeFi calculations.

pub(super) const BPS_SCALE: u64 = 10_000;

/// Constant-product AMM calculations.
pub mod amm;
/// Fee calculations.
pub mod fee;
/// Lending calculations.
pub mod lending;
/// Oracle scaling calculations.
pub mod oracle;
/// Time-based schedule calculations.
pub mod schedule;
/// Staking reward calculations.
pub mod staking;
