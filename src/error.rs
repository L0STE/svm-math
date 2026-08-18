use core::fmt;

/// Errors returned by primitive math operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
pub enum MathError {
    /// A denominator or fixed-point scale was zero.
    DivByZero = 0,
    /// The exact result cannot be represented by the function's return type.
    Overflow = 2,
    /// The input lies outside the mathematical domain of the operation.
    OutOfDomain = 4,
}

impl MathError {
    /// Returns the stable numeric error code.
    #[inline]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for MathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DivByZero => "division by zero",
            Self::Overflow => "arithmetic overflow",
            Self::OutOfDomain => "input outside operation domain",
        })
    }
}

impl core::error::Error for MathError {}
