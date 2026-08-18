import Lean.Elab.Tactic.Omega

/-!
The mathematical boundary for the production integer square-root loops.

Rust/Kani checks the actual seed, Newton-step, normalization, estimate, and
correction helpers over symbolic machine words. This file composes their
integer premises into the fixed 32-step convergence bound and the normalized
two-word estimator's one-correction result.
-/

namespace Sqrt

def excess (candidate floor : Nat) : Nat := candidate - floor - 1

/-- The production seed is at most `2^32`; therefore its excess above any
positive floor fits strictly below the 32-bit halving budget. Kani's seed cut
proves the machine-word bound and brackets the input between `(seed/2)^2` and
`seed^2`. -/
theorem seed_excess_below_word
    (seed floor : Nat)
    (hseed : seed <= 2^32) :
    excess seed floor < 2^32 := by
  unfold excess
  omega

/-- If every still-live Newton step at least halves the distance above
`floor + 1`, 32 steps exhaust every u32-sized initial distance. This is the
exact recurrence needed by the production cap; early termination only makes
the trace shorter. -/
theorem halving_trace_32
    (error : Nat -> Nat)
    (h0 : error 0 < 2^32)
    (hstep : forall i, i < 32 -> error (i + 1) <= error i / 2) :
    error 32 = 0 := by
  have bound : forall i, i <= 32 -> error i <= error 0 / 2^i := by
    intro i hi
    induction i with
    | zero => simp
    | succ i ih =>
      have hi' : i < 32 := by omega
      calc
        error (i + 1) <= error i / 2 := hstep i hi'
        _ <= (error 0 / 2^i) / 2 := Nat.div_le_div_right (ih (by omega))
        _ = error 0 / 2^(i + 1) := by
          rw [Nat.div_div_eq_div_mul, Nat.pow_succ]
  have hfinal := bound 32 (by omega)
  have hdiv : error 0 / 2^32 = 0 := Nat.div_eq_of_lt h0
  omega

/-- Composition theorem for the bounded production loop. The trace is made
total by repeating the terminal candidate after the Rust early exit. The
lower invariant and the live-step recurrence imply that step 32 is the floor
or its successor, so the production terminal correction is sufficient. -/
theorem bounded_newton_reaches_adjacent
    (root : Nat -> Nat)
    (floor : Nat)
    (hseed : root 0 <= 2^32)
    (hlower : floor <= root 32)
    (hstep : forall i, i < 32 ->
      excess (root (i + 1)) floor <= excess (root i) floor / 2) :
    root 32 = floor \/ root 32 = floor + 1 := by
  have h0 := seed_excess_below_word (root 0) floor hseed
  have hfinal := halving_trace_32 (fun i => excess (root i) floor) h0 hstep
  unfold excess at hfinal
  have hdiff : root 32 - floor <= 1 := Nat.sub_eq_zero_iff_le.mp hfinal
  omega

/-- For the exact integer-floor recurrence, `n <= m*(m+2)` follows from
`m^2 <= n < (m+1)^2`. While `x >= m+2`, Euclidean division gives `n/x <= m`,
so the next excess above `m` is at most half the previous excess. Kani's
Newton cut proves the production helper computes `(x + n/x)/2` without word
overflow on its reachable domain. -/
theorem newton_excess_halves
    (n m x quotient : Nat)
    (hm : 0 < m)
    (hx : m + 2 <= x)
    (hx_repr : x = m + (x - m))
    (hq : quotient = n / x)
    (hn_upper : n <= m * (m + 2)) :
    ((x + quotient) / 2 - m) <= (x - m) / 2 := by
  have hx_pos : 0 < x := by omega
  have hquotient : quotient <= m := by
    rw [hq]
    apply (Nat.div_le_iff_le_mul_add_pred hx_pos).2
    have hn_upper' : n <= (m + 2) * m := by
      simpa [Nat.mul_comm] using hn_upper
    have hmul : (m + 2) * m <= x * m := Nat.mul_le_mul_right m hx
    omega
  omega

/-- The low 32-bit tail omitted by the normalized estimate is strictly less
than one `2*s*B` root gap. -/
theorem normalized_tail_lt_root_gap
    (B s d tail : Nat)
    (hd : d < 2 * s)
    (htail : tail < B) :
    d * B + tail < 2 * s * B := by
  have hd' : d + 1 <= 2 * s := by omega
  have hmul : (d + 1) * B <= (2 * s) * B := Nat.mul_le_mul_right B hd'
  rw [Nat.add_mul, Nat.one_mul] at hmul
  omega

/-- The estimator's omitted `q^2` term is smaller than one square gap when
`q < B` and the normalized high-word root satisfies `B <= 2*s`. -/
theorem normalized_omitted_square_lt_root_gap
    (B s q : Nat)
    (hq : q < B)
    (hs : B <= 2 * s) :
    q * q < 2 * s * B := by
  have hqB : q * q < B * B := Nat.mul_self_lt_mul_self hq
  have hBB : B * B <= (2 * s) * B := Nat.mul_le_mul_right B hs
  exact Nat.lt_of_lt_of_le hqB hBB

/-- The two directional estimator errors put its candidate strictly between
the neighboring squares. `omittedSquare` is the `q^2` term and `tailBound` is
the discarded low-word contribution; the preceding lemmas discharge their
respective gap bounds for `B=2^32`. -/
theorem normalized_estimator_is_adjacent
    (n predecessorSquare square successorSquare omittedSquare tailBound
      downGap upGap : Nat)
    (hover : square <= n + omittedSquare)
    (homitted : omittedSquare <= downGap)
    (hunder : n < square + tailBound)
    (htail : tailBound <= upGap)
    (hpred : predecessorSquare + downGap = square)
    (hsucc : square + upGap = successorSquare) :
    predecessorSquare <= n /\ n < successorSquare := by
  constructor <;> omega

/-- Given adjacency, the production square comparison and one downward
correction return the floor. Kani checks the comparison/decrement branch on
the actual helper; widening multiplication is proved by its own word harness. -/
theorem one_downward_correction_is_floor
    (n candidate square predecessorSquare successorSquare downGap : Nat)
    (hcandidate : 0 < candidate)
    (hsquare : square = candidate * candidate)
    (hpredSquare : predecessorSquare = (candidate - 1) * (candidate - 1))
    (hsuccSquare : successorSquare = (candidate + 1) * (candidate + 1))
    (hpred : predecessorSquare + downGap = square)
    (hsucc : n < successorSquare)
    (hgap : square <= n + downGap) :
    let root := if square > n then candidate - 1 else candidate
    root * root <= n /\ n < (root + 1) * (root + 1) := by
  intro root
  simp only [root]
  split <;> rename_i h
  · constructor
    · have hpredLower : predecessorSquare <= n := by omega
      simpa [hpredSquare] using hpredLower
    · have hc : candidate - 1 + 1 = candidate := by omega
      simpa [hc, hsquare] using h
  · constructor
    · simpa [hsquare] using (show square <= n by omega)
    · simpa [hsuccSquare] using hsucc

end Sqrt
