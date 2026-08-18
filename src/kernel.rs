//! Operation-family kernels. Share code only below the point where result,
//! error, domain, and rounding obligations are identical.
//!
//! Evaluated and declined on this cost model; do not relitigate without new
//! measurements: Karatsuba multiplication (a ~2 CU multiply is cheaper than
//! its extra adds), Estrin evaluation (SBF prices instruction count, not
//! latency), Knuth Algorithm D (the Moller-Granlund reciprocal is ~6
//! multiplies), minimax residual polynomials (every retained Taylor term is
//! load-bearing at the 11-bit table size), a truncated single-multiply cubic
//! exp2 term (its error just exceeds the budget; the quartic ships with the
//! trick), a seeded Newton square-root table (~10 CU against a proof-anchored
//! seed stage), `NonZeroU64` scale threading (duplicate checks share one
//! inlined SSA value), and `powi_bounds` (the directed accumulators diverge
//! at the first rounding, so a fused pair has nothing to share).

pub(crate) mod compound;
pub(crate) mod exp2;
pub(crate) mod log2;
pub(crate) mod pow;
pub(crate) mod scale;
pub(crate) mod sqrt;
pub(crate) mod wide;

/// Upper lookup tables ship as deltas over their lower twins: the const
/// assert guarantees bit-identical reconstruction, and each 16 KB table
/// becomes 2 KB of `.rodata`.
pub(crate) const fn upper_delta_table(lower: [u64; 2048], upper: [u64; 2048]) -> [u8; 2048] {
    let mut deltas = [0_u8; 2048];
    let mut index = 0;
    while index < deltas.len() {
        let delta = upper[index] - lower[index];
        assert!(delta <= u8::MAX as u64);
        deltas[index] = delta as u8;
        index += 1;
    }
    deltas
}
