# svm-math

Fast checked integer and fixed-point math for Solana programs. The crate is
`no_std`, has zero dependencies, exposes no types beyond one error enum, and
every public operation ships with a measured compute-unit cost and a
verification trail.

## Exact arithmetic

```rust
use svm_math::mul_div_floor;

let output = mul_div_floor(4_000_000, 250_000, 5_250_000)?;
assert_eq!(output, 190_476);
# Ok::<(), svm_math::MathError>(())
```

`mul_div_floor` and `mul_div_ceil` evaluate the full mathematical `u64 × u64`
product before division. `isqrt`, `sqrt_floor`, and `sqrt_ceil` are exact
integer operations too.

## One scale convention

A positive `scale` is the number of raw units in one mathematical unit. With
`scale = 1_000_000`, the raw value `1_500_000` means `1.5`.

```rust
use svm_math::{exp2_bounds, log2_bounds};

const SCALE: u64 = 1_000_000;
let (root_two_lo, root_two_hi) = exp2_bounds(500_000, SCALE)?;
assert!(root_two_lo <= 1_414_214 && root_two_hi >= 1_414_213);

let (one_lo, one_hi) = log2_bounds(2 * SCALE, SCALE)?;
assert!(one_lo <= SCALE as i64 && one_hi >= SCALE as i64);
# Ok::<(), svm_math::MathError>(())
```

A zero scale returns `MathError::DivByZero`.

Names ending in `floor` or `ceil` return that exact integer rounding. Names
ending in `lower` or `upper` return certified one-sided bounds whose interval
may be wider than one output quantum. Names ending in `bounds` return the
`(lower, upper)` pair from one pass — bit-identical to the two calls, cheaper
because the shared work is paid once.

## API and compute cost

Costs are compute units per call, measured on SBF (platform-tools v1.53)
through LiteSVM with a matched control per operation; three repetitions
reproduce every number exactly. `make benchmark` regenerates the table, and
release gates fail if any operation exceeds its recorded reference.

Every operation is measured in two regimes. **Narrow** uses word-sized
operands at a `10^9` scale — products fit one machine word and the fast
paths apply. **Wide** uses operands near the top of `u64` at a `10^18`
scale — every intermediate crosses 128 bits and the cost is dominated by
the 128-by-64 divider. The wide column is why this crate exists.

### Core

| function | narrow | wide | returns |
|---|---:|---:|---|
| `mul_div_floor(a, b, d)` | 11 | 102 | exact `⌊a·b/d⌋`; wide is measured over a top-bit-set divisor, and a divisor needing normalization costs ~140 |
| `mul_div_ceil(a, b, d)` | 19 | 120 | exact `⌈a·b/d⌉` |
| `isqrt(n)` | 142 | 249 | exact `⌊√n⌋` for any `u128`; wide is a 126-bit value |
| `sqrt_floor(v, s)` | 167 | 276 | exact `⌊√(v·s)⌋` |
| `sqrt_ceil(v, s)` | 177 | 286 | exact `⌈√(v·s)⌉` |
| `exp2_lower(e, s)` / `exp2_upper(e, s)` | 193 / 217 | 250 / 274 | one-sided bounds on `2^(e/s)`, negative exponents included |
| `log2_lower(v, s)` / `log2_upper(v, s)` | 381 / 413 | 394 / 426 | one-sided bounds on `log2(v/s)`, signed result |
| `pow_lower(b, e, s)` / `pow_upper(b, e, s)` | 576 / 653 | 669 / 752 | one-sided bounds on `(b/s)^(e/s)`; exact integer exponents route to `powi` |
| `powi_lower(b, n, s)` / `powi_upper(b, n, s)` | 113 / 186 | 781 / 887 | one-sided bounds on `(b/s)^n` by directed squaring; wide operands square in binary fixed point, paying the divider only at the seed and the projection |
| `compound_lower(r, n, t, s)` / `compound_upper(r, n, t, s)` | 1 078 / 1 105 | 1 190 / 1 217 | one-sided bounds on `(1 + (r/s)/n)^t` |
| `exp2_bounds(e, s)` | 387 | 443 | both exp2 bounds, one pass — bit-identical to the two calls, cheaper |
| `log2_bounds(v, s)` | 609 | 633 | both log2 bounds, one pass |
| `pow_bounds(b, e, s)` | 990 | 1104 | both pow bounds, one pass |
| `compound_bounds(r, n, t, s)` | 1 552 | 1 667 | both compound bounds, one pass |


### `defi` — stateless recipes over primitive integers

Each recipe names the party its rounding protects. Callers own asset
identity, decimals, storage, and error conversion.

| function | narrow | wide | returns |
|---|---:|---:|---|
| `fee::net_of_fee(amount, bps)` | 30 | 43 | `(net, fee)` with `net + fee == amount`, fee rounded against the payer |
| `amm::quote_exact_in(rin, rout, in, bps)` | 68 | 163 | constant-product output, floored — the pool never over-pays |
| `amm::quote_exact_out(rin, rout, out, bps)` | 69 | 326 | least input whose replay through `quote_exact_in` reaches `out` |
| `amm::initial_lp_shares_floor(a, b)` | 160 | 270 | `⌊√(a·b)⌋` bootstrap shares — never over-mints |
| `lending::utilization_bps(borrowed, supplied)` | 16 | 137 | utilization in basis points, floored |
| `lending::borrow_rate_bps(u, base, s1, s2, kink)` | 57 | 75 | two-leg kinked rate, ceiled — borrowers never underpay the curve |
| `oracle::price_bounds_scaled(price, conf, expo, s)` | 139 | 296 | outward bounds on `(price ± conf)·10^expo`, floored at zero |
| `schedule::vested_floor(total, start, cliff, dur, now)` | 26 | 167 | cliffed linear vesting, floored — never over-releases |
| `schedule::linear_interp_floor(from, to, t, dur)` | 29 | 175 | clamped interpolation, below the true line |
| `schedule::linear_interp_ceil(from, to, t, dur)` | 25 | 171 | clamped interpolation, above the true line |
| `staking::reward_index_accrue_lower(i, r, staked, s)` | 30 | 175 | index accrual, floored |
| `staking::reward_index_accrue_upper(i, r, staked, s)` | 39 | 198 | index accrual, ceiled |
| `staking::reward_index_accrue(lo, hi, r, staked, s)` | 50 | 209 | both endpoints from one division |
| `staking::rewards_owed_floor(staked, now, snap, s)` | 24 | 166 | rewards owed, floored — the pool never over-pays |

```rust
use svm_math::defi::{amm, fee};

let (net, charged) = fee::net_of_fee(250_000, 30)?;
assert_eq!(net + charged, 250_000);

let output = amm::quote_exact_in(5_000_000, 4_000_000, 250_000, 30)?;
let least_replay = amm::quote_exact_out(5_000_000, 4_000_000, output, 30)?;
assert!(least_replay <= 250_000);
# Ok::<(), svm_math::MathError>(())
```

## Errors

Every fallible function returns `MathError` with stable codes: `DivByZero`
(0) for a zero denominator or scale, `Overflow` (2) when the exact result
does not fit the return type, and `OutOfDomain` (4) for inputs outside an
operation's mathematical domain. `tests/contract.rs` enumerates the complete
guard matrix — every documented precondition of every public function against
its exact error.

## Verification

Exact kernels carry deterministic identities, reduced-width exhaustive
sweeps, Kani harnesses, and a Lean division theorem. Transcendental bounds
are checked against an independent adaptive-precision MPFR oracle across
fifty thousand cases. The contract suite pins the guard matrix and the
economic guarantees — conservation, no-draining, monotonicity, clamping —
over realistic magnitudes. SBF workloads measure every public operation
against a matched control and gate regressions against the recorded profile.

## License

MIT or Apache-2.0.
