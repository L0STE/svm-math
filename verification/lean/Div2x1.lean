import Lean.Elab.Tactic.Omega
import Lean.Elab.Tactic.Grind

/-!
The wide reciprocal-division proof boundary.

Rust represents `v * d` as `ph * B + pl`, where `B = 2^64`.  The final
reciprocal adjustment uses the machine predicates:

* `d + ph` has no u64 carry: `d + ph < B`;
* `pl + d` has a u64 carry: `B <= pl + d`; and
* adding that carry to the preceding high word carries: `B <= d + ph + 1`.

This file proves that, with the ordinary limb bounds, those operations
characterize the reciprocal interval `(B + v) * d <= B^2 - 1 <
(B + v + 1) * d`. Kani first compares `kernel::wide::widening_mul` with the independent
four-term 32-bit partial-product expansion below, then checks the real Rust
`kernel::wide::{reciprocal, divide_normalized}` helpers at word-level cut points. Lean composes those
cuts into the exact reciprocal error witness, Algorithm 4's signed candidate
bounds, both correction cases, a Euclidean quotient/remainder, and the u64
mantissa divider's directed-rounding state machine.
-/

namespace Div2x1

def base : Nat := 2^64

/-! The exact Nat counterpart of `proofs::widening_mul_u64_matches_u128`.

Writing `a = ah·2^32 + al` and `b = bh·2^32 + bl`, the Kani harness checks the
actual production pair `(hi:lo)` against the right-hand four-term expansion
using only 32-bit limb products. This theorem proves that expansion is the
mathematical product; it does not restate the production carry algorithm. -/
theorem widening_mul_four_term (ah al bh bl : Nat) :
    (ah * 2^32 + al) * (bh * 2^32 + bl) =
      ah * bh * 2^64 + (ah * bl + al * bh) * 2^32 + al * bl := by
  grind

def CarryCertificate (d v ph pl : Nat) : Prop :=
  v * d = ph * base + pl /\
    ph < base /\
      pl < base /\
        d < base /\
          d + ph < base /\
            base <= pl + d /\
              base <= d + ph + 1

def ReciprocalInterval (d v : Nat) : Prop :=
  (base + v) * d <= base * base - 1 /\
    base * base - 1 < (base + v + 1) * d

/-!
The six named input domains below are shared verbatim with the isolated Kani
selector branches in `proofs::reciprocal_u64_matches_definition`.  They are
not a claim that a later stage was run: each branch quantifies only the inputs
to the actual factored Rust helper it calls.

`ErrorWord`, `V3Word`, and `V4Word` model Rust's `wrapping_*` operations as
reduction modulo `B`; `errorMask` models `d0.wrapping_neg()` after the
normalized-divisor parity split (`d0 = 0` or `d0 = 1`).  The theorems below
derive the Granlund--Möller refinement-error bound from the stage equations,
then compose it with Algorithm 4 and the normalized mantissa divider.  No
refinement bound or reciprocal interval is postulated.
-/
def SeedDomain (d9 : Nat) : Prop := 2^8 <= d9 /\ d9 < 2^9
def V1Domain (v0 d40 : Nat) : Prop :=
  2^10 <= v0 /\ v0 < 2^11 /\ 2^39 <= d40 /\ d40 <= 2^40
def V2Domain (v1 d40 : Nat) : Prop :=
  2^20 <= v1 /\ v1 < 2^21 /\ 2^39 <= d40 /\ d40 <= 2^40 /\
    v1 * d40 <= 2^60 /\ 2^60 - v1 * d40 <= 2^42
def ErrorDomain (v2 d0 d63 : Nat) : Prop :=
  v2 < 2^35 /\ d0 <= 1 /\ 2^62 <= d63 /\ d63 <= 2^63
def V3Domain (v2 : Nat) : Prop := v2 < 2^35
def V4Domain (d : Nat) : Prop := 2^63 <= d /\ d < base

def SeedCertificate (d9 v0 : Nat) : Prop :=
  v0 * d9 <= 2^19 - 3 * 2^8 /\ 2^19 - 3 * 2^8 < (v0 + 1) * d9
def V1Equation (v0 d40 v1 : Nat) : Prop :=
  v1 + ((v0 * v0 * d40) / 2^40) + 1 = v0 * 2^11
def V2Equation (v1 d40 v2 : Nat) : Prop :=
  v2 = v1 * 2^13 + (v1 * (2^60 - v1 * d40)) / 2^47
def errorMask (v2 d0 : Nat) : Nat := if d0 = 0 then 0 else v2 / 2
def subMod (a b modulus : Nat) : Nat := (a + modulus - b % modulus) % modulus
def ErrorWord (v2 d0 d63 : Nat) : Nat := subMod (errorMask v2 d0) (v2 * d63) base
def V3Word (v2 _error hi : Nat) : Nat := (v2 * 2^31 + hi / 2) % base
def V4Word (v3 d ph pl : Nat) : Nat :=
  subMod (subMod v3 (ph + (if base <= pl + d then 1 else 0)) base) d base

/-! The scaled integer forms of Theorem 1, equations (7)--(9).  These are
the exact counterparts of the Kani stage contracts; scaling removes all
fractional truncation terms without using a non-machine rational model. -/
def E1Scaled (e0 e1 d40 r1 : Nat) : Prop :=
  2^40 * e1 = e0^2 + (2^40 - r1) * d40 /\ r1 < 2^40
def E2Scaled (e1 e2 v2 dPrime r2 d : Nat) : Prop :=
  2^47 * e2 =
    2^24 * e1^2 + (2^47 * v2 + r2) * dPrime + r2 * d /\
      r2 < 2^47
def E3Scaled (e2 e3 v2 epsilon r3 d : Nat) : Prop :=
  2^66 * e3 = e2^2 + v2 * epsilon * d + r3 * d /\
    epsilon < 2 /\ r3 < 2^66

def QR (n divisor quotient remainder : Nat) : Prop :=
  n = quotient * divisor + remainder /\ remainder < divisor

def AbsGap (a b gap : Nat) : Prop := a = b + gap \/ b = a + gap

theorem e1_scaled_from_witnesses
    (v0 d40 e0 q1 r1 v1 e1 : Nat)
    (he0 : AbsGap (2^50) (v0 * d40) e0)
    (hqr : QR (v0 * v0 * d40) (2^40) q1 r1)
    (hv1 : v1 + q1 + 1 = v0 * 2^11)
    (he1 : v1 * d40 + e1 = 2^60) : E1Scaled e0 e1 d40 r1 := by
  rcases hqr with ⟨hproduct, hremainder⟩
  unfold E1Scaled
  constructor
  · have hcomplement : r1 + (2^40 - r1) = 2^40 := by omega
    rcases he0 with he0 | he0 <;> grind (splits := 20)
  · exact hremainder

theorem e2_scaled_from_witnesses
    (v1 d40 e1 q2 r2 v2 d dPrime e2 : Nat)
    (he1 : v1 * d40 + e1 = 2^60)
    (hd : d + dPrime = 2^24 * d40)
    (hqr : QR (v1 * e1) (2^47) q2 r2)
    (hv2 : v2 = v1 * 2^13 + q2)
    (he2 : v2 * d + e2 = 2^97) : E2Scaled e1 e2 v2 dPrime r2 d := by
  rcases hqr with ⟨hproduct, hremainder⟩
  unfold E2Scaled
  constructor
  · grind
  · exact hremainder

set_option maxHeartbeats 1000000 in
theorem e3_scaled_from_witnesses
    (v2 d e2 error epsilon hi lo q3 hiBit v3 e3 r3 : Nat)
    (hrecip : v2 * d + e2 = 2^97)
    (he2 : e2 = 2 * error + epsilon) (hepsilon : epsilon < 2)
    (hproduct : v2 * error = hi * base + lo)
    (hhi : hi = 2 * q3 + hiBit) (hhiBit : hiBit < 2)
    (hv3 : base + v3 = v2 * 2^31 + q3)
    (hr3 : r3 = hiBit * 2^65 + 2 * lo)
    (he3 : (base + v3) * d + e3 = base * base)
    (hr3Bound : r3 < 2^66) : E3Scaled e2 e3 v2 epsilon r3 d := by
  unfold E3Scaled base at *
  constructor
  · grind (splits := 20)
  · exact ⟨hepsilon, hr3Bound⟩

theorem error_halving_from_witnesses
    (v2 d e2 d0 d63 v2Half v2Bit error : Nat)
    (hrecip : v2 * d + e2 = 2^97)
    (hv2 : v2 = 2 * v2Half + v2Bit) (hv2Bit : v2Bit < 2)
    (hd0 : d0 < 2) (hd63 : d + d0 = 2 * d63)
    (herror : error + v2 * d63 = 2^96 + v2Half * d0) :
    e2 = 2 * error + v2Bit * d0 /\ v2Bit * d0 < 2 := by
  constructor
  · grind
  · have hv2BitCases : v2Bit = 0 \/ v2Bit = 1 := by omega
    have hd0Cases : d0 = 0 \/ d0 = 1 := by omega
    rcases hv2BitCases with rfl | rfl <;> rcases hd0Cases with rfl | rfl <;> decide

/-! The second refinement remains a 34-bit word.  This is the correlation
that a loose independent bound on `v1` and `e1` loses: their product is tied
by `v1*d40 + e1 = 2^60`. -/
theorem v2_word_bound (v1 d40 e1 q2 r2 v2 gap : Nat)
    (hgap : v1 + gap = 2^21) (hgapPos : 0 < gap)
    (hd40 : 2^39 <= d40) (he1 : v1 * d40 + e1 = 2^60)
    (hqr : QR (v1 * e1) (2^47) q2 r2)
    (hv2 : v2 = v1 * 2^13 + q2) : v2 < 2^34 := by
  have hmul : v1 * v1 * 2^39 <= v1 * v1 * d40 := by
    exact Nat.mul_le_mul_left (v1 * v1) hd40
  have hsquare :
      v1 * v1 * 2^39 + 2^60 * gap =
        v1 * 2^60 + 2^39 * gap * gap := by
    grind
  have hsquarePos : 0 < 2^39 * gap * gap := by
    exact Nat.mul_pos (Nat.mul_pos (by omega) hgapPos) hgapPos
  have hstrict : v1 * 2^60 < v1 * v1 * d40 + 2^60 * gap := by
    omega
  have hproduct : v1 * e1 + v1 * v1 * d40 = v1 * 2^60 := by
    grind
  have herror : v1 * e1 < 2^60 * gap := by
    omega
  rcases hqr with ⟨hqrEq, hr2⟩
  have hquotientScaled : q2 * 2^47 < 2^60 * gap := by
    omega
  have hquotient : q2 < 2^13 * gap := by
    omega
  have htotal : v1 * 2^13 + gap * 2^13 = 2^34 := by
    grind
  omega

theorem e1_bounds_from_scaled (e0 e1 d40 r1 : Nat)
    (h : E1Scaled e0 e1 d40 r1) (hd : 2^39 <= d40) (hd' : d40 <= 2^40)
    (he0 : e0^2 < 25 * 2^78)
    (hfactorUpper : (2^40 - r1) * d40 <= 2^80) :
    0 < e1 /\ e1 < 29 * 2^38 := by
  rcases h with ⟨hscale, hr1⟩
  have hfactor : 0 < (2^40 - r1) * d40 := by
    have : 0 < 2^40 - r1 := by omega
    exact Nat.mul_pos this (by omega)
  omega

theorem e2_bounds_from_scaled (e1 e2 v2 dPrime r2 d : Nat)
    (h : E2Scaled e1 e2 v2 dPrime r2 d)
    (he1 : 0 < e1 /\ e1 < 29 * 2^38) (hv2 : 0 < v2 /\ v2 < 2^34)
    (hdPrime : 0 < dPrime /\ dPrime <= 2^24)
    (hd : 0 < d /\ d < 2^64) :
    0 < e2 /\ e2 < 873 * 2^53 + d := by
  rcases h with ⟨hscale, hr2⟩
  rcases he1 with ⟨he1pos, he1lt⟩
  rcases hv2 with ⟨hv2pos, hv2lt⟩
  rcases hdPrime with ⟨hdPrimePos, hdPrimeLe⟩
  rcases hd with ⟨hdpos, hdlt⟩
  have he1square : e1 * e1 < (29 * 2^38) * (29 * 2^38) := by
    have hleft : e1 * e1 < (29 * 2^38) * e1 :=
      (Nat.mul_lt_mul_right he1pos).2 he1lt
    have hright : (29 * 2^38) * e1 < (29 * 2^38) * (29 * 2^38) :=
      (Nat.mul_lt_mul_left (by omega)).2 he1lt
    omega
  have he1term : 2^24 * e1^2 < 2^47 * (841 * 2^53) := by
    have hscaled := (Nat.mul_lt_mul_left (a := 2^24) (by omega)).2 he1square
    simp only [Nat.pow_two]
    omega
  have hcoefficient : 2^47 * v2 + r2 < 2^47 * 2^34 := by omega
  have hv2term : (2^47 * v2 + r2) * dPrime < 2^47 * (32 * 2^53) := by
    have hleft :
        (2^47 * v2 + r2) * dPrime < (2^47 * 2^34) * dPrime :=
      (Nat.mul_lt_mul_right hdPrimePos).2 hcoefficient
    have hright :
        (2^47 * 2^34) * dPrime <= (2^47 * 2^34) * 2^24 :=
      Nat.mul_le_mul_left (2^47 * 2^34) hdPrimeLe
    omega
  have hr2term : r2 * d < 2^47 * d :=
    (Nat.mul_lt_mul_right hdpos).2 hr2
  have hpositive : 0 < (2^47 * v2 + r2) * dPrime := by
    exact Nat.mul_pos (by omega) hdPrimePos
  constructor
  · omega
  · omega

theorem e3_bounds_from_scaled (e2 e3 v2 epsilon r3 d : Nat)
    (h : E3Scaled e2 e3 v2 epsilon r3 d)
    (he2 : 0 < e2 /\ e2 < 873 * 2^53 + d)
    (hv2 : v2 < 2^34) (hd : 2^63 <= d /\ d < 2^64) :
    0 < e3 /\ e3 < 2 * d := by
  rcases h with ⟨hscale, hepsilon, hr3⟩
  rcases he2 with ⟨he2pos, he2lt⟩
  rcases hd with ⟨hdLower, hdUpper⟩
  have hdpos : 0 < d := by omega
  have hcoarse : 8 * e2 < 15 * d := by
    have hconstant : 8 * (873 * 2^53) < 7 * d := by omega
    omega
  have hsquareRaw : (8 * e2) * (8 * e2) < (15 * d) * (15 * d) := by
    have hleft : (8 * e2) * (8 * e2) < (15 * d) * (8 * e2) :=
      (Nat.mul_lt_mul_right (by omega)).2 hcoarse
    have hright : (15 * d) * (8 * e2) < (15 * d) * (15 * d) :=
      (Nat.mul_lt_mul_left (by omega)).2 hcoarse
    omega
  have hsquare : 64 * e2^2 < 225 * d^2 := by
    simp only [Nat.pow_two]
    grind
  have hdsquare : d * d < 2^64 * d :=
    (Nat.mul_lt_mul_right hdpos).2 hdUpper
  have he2term : e2^2 < 225 * 2^58 * d := by
    have hscaled : 225 * (d * d) < 225 * (2^64 * d) :=
      (Nat.mul_lt_mul_left (by omega)).2 hdsquare
    have hsquare' : 64 * (e2 * e2) < 225 * (d * d) := by
      simpa only [Nat.pow_two] using hsquare
    have hcombined : 64 * (e2 * e2) < 225 * (2^64 * d) := by omega
    have htarget : e2 * e2 < 225 * 2^58 * d := by omega
    simpa only [Nat.pow_two] using htarget
  have hv2epsilon : v2 * epsilon < 2^35 := by
    by_cases hepsilonZero : epsilon = 0
    · simp [hepsilonZero]
    · have hepsilonOne : epsilon = 1 := by omega
      simp [hepsilonOne]
      omega
  have hwordTerm : (v2 * epsilon + r3) * d < (2^35 + 2^66) * d := by
    have hsum : v2 * epsilon + r3 < 2^35 + 2^66 := by omega
    exact (Nat.mul_lt_mul_right hdpos).2 hsum
  have htotal :
      e2^2 + v2 * epsilon * d + r3 * d < 2^67 * d := by
    have hwords :
        e2^2 + v2 * epsilon * d + r3 * d =
          e2^2 + (v2 * epsilon + r3) * d := by
      grind
    rw [hwords]
    have hconstant : 225 * 2^58 + 2^35 + 2^66 < 2^67 := by omega
    have hconstantScaled :
        (225 * 2^58 + 2^35 + 2^66) * d < 2^67 * d :=
      (Nat.mul_lt_mul_right hdpos).2 hconstant
    omega
  constructor
  · have he2squarePos : 0 < e2^2 := by
      simp only [Nat.pow_two]
      exact Nat.mul_pos he2pos he2pos
    omega
  · omega

/-! The final adjustment increments the approximation exactly when the
preceding error is larger than the divisor.  Stating the result this way
isolates the machine carry proof from the arithmetic consequence. -/
theorem adjusted_error_exists (d v3 v4 e3 : Nat)
    (herror : base * base = (base + v3) * d + e3)
    (he3 : 0 < e3 /\ e3 < 2 * d)
    (hadjust : v4 = if e3 <= d then v3 else v3 + 1) :
    ∃ e4, base * base = (base + v4) * d + e4 /\ 0 < e4 /\ e4 <= d := by
  rcases he3 with ⟨he3pos, he3lt⟩
  by_cases hle : e3 <= d
  · simp [hle] at hadjust
    subst v4
    exact ⟨e3, herror, he3pos, hle⟩
  · have hdlt : d < e3 := by omega
    have hsplit : d + (e3 - d) = e3 := by omega
    simp [hle] at hadjust
    subst v4
    refine ⟨e3 - d, ?_, by omega, by omega⟩
    have hassoc : base + (v3 + 1) = (base + v3) + 1 := by omega
    rw [hassoc, Nat.add_mul, Nat.one_mul]
    omega

/-! The final adjustment is deliberately stated without subtraction.  Kani
checks the actual machine helper's keep-or-increment rule; once composed with
`adjusted_error_exists`, the paper's Eq. (10) is exactly this interval. -/
theorem final_error_implies_interval (d v4 e4 : Nat)
    (h : base * base = (base + v4) * d + e4)
    (he4 : 0 < e4 /\ e4 <= d) : ReciprocalInterval d v4 := by
  rcases he4 with ⟨hpositive, hle⟩
  unfold ReciprocalInterval base at *
  simp only [Nat.add_mul] at h
  simp only [Nat.add_mul, Nat.one_mul]
  omega

/-! The complete Algorithm 2 composition.  Each hypothesis is either an
input decomposition or one of the word-level relations checked at the Kani
cut points; no refinement-error bound is assumed. -/
set_option maxHeartbeats 1000000 in
theorem algorithm2_correct
    (v0 d40 e0 q1 r1 v1 e1 gap q2 r2 v2 d dPrime e2
      d0 d63 v2Half v2Bit error epsilon hi lo q3 hiBit v3 e3 r3 v4 : Nat)
    (he0Relation : AbsGap (2^50) (v0 * d40) e0)
    (he0Bound : e0 < 5 * 2^39)
    (hv0Product : QR (v0 * v0 * d40) (2^40) q1 r1)
    (hv1 : v1 + q1 + 1 = v0 * 2^11)
    (he1Relation : v1 * d40 + e1 = 2^60)
    (hv1Positive : 0 < v1)
    (hgap : v1 + gap = 2^21) (hgapPositive : 0 < gap)
    (hd40 : 2^39 <= d40 /\ d40 <= 2^40)
    (hv1Error : QR (v1 * e1) (2^47) q2 r2)
    (hv2 : v2 = v1 * 2^13 + q2)
    (hdPrime : d + dPrime = 2^24 * d40)
    (hdPrimeBounds : 0 < dPrime /\ dPrime <= 2^24)
    (he2Relation : v2 * d + e2 = 2^97)
    (hd : 2^63 <= d /\ d < 2^64)
    (hv2Decomp : v2 = 2 * v2Half + v2Bit) (hv2Bit : v2Bit < 2)
    (hd0 : d0 < 2) (hd63 : d + d0 = 2 * d63)
    (herrorWord : error + v2 * d63 = 2^96 + v2Half * d0)
    (hepsilonDef : epsilon = v2Bit * d0)
    (hv2Error : v2 * error = hi * base + lo)
    (hhi : hi = 2 * q3 + hiBit) (hhiBit : hiBit < 2)
    (hv3 : base + v3 = v2 * 2^31 + q3)
    (hr3 : r3 = hiBit * 2^65 + 2 * lo) (hr3Bound : r3 < 2^66)
    (he3Relation : (base + v3) * d + e3 = base * base)
    (hadjust : v4 = if e3 <= d then v3 else v3 + 1) :
    ReciprocalInterval d v4 /\
      ∃ e4, base * base = (base + v4) * d + e4 /\ 0 < e4 /\ e4 <= d := by
  have he0Square : e0^2 < 25 * 2^78 := by
    by_cases he0Zero : e0 = 0
    · simp [he0Zero]
    · have he0Positive : 0 < e0 := by omega
      have hleft : e0 * e0 < (5 * 2^39) * e0 :=
        (Nat.mul_lt_mul_right he0Positive).2 he0Bound
      have hright : (5 * 2^39) * e0 < (5 * 2^39) * (5 * 2^39) :=
        (Nat.mul_lt_mul_left (by omega)).2 he0Bound
      simp only [Nat.pow_two]
      omega
  rcases hd40 with ⟨hd40Lower, hd40Upper⟩
  have hfactorUpper : (2^40 - r1) * d40 <= 2^80 := by
    have hrComplement : 2^40 - r1 <= 2^40 := Nat.sub_le (2^40) r1
    have hmul := Nat.mul_le_mul hrComplement hd40Upper
    omega
  have he1Scaled := e1_scaled_from_witnesses v0 d40 e0 q1 r1 v1 e1
    he0Relation hv0Product hv1 he1Relation
  have he1Bounds := e1_bounds_from_scaled e0 e1 d40 r1 he1Scaled
    hd40Lower hd40Upper he0Square hfactorUpper
  rcases he1Bounds with ⟨he1Positive, he1Upper⟩
  have hv2Upper := v2_word_bound v1 d40 e1 q2 r2 v2 gap
    hgap hgapPositive hd40Lower he1Relation hv1Error hv2
  have hv2Positive : 0 < v2 := by
    have hterm : 0 < v1 * 2^13 := Nat.mul_pos hv1Positive (by omega)
    omega
  have he2Scaled := e2_scaled_from_witnesses
    v1 d40 e1 q2 r2 v2 d dPrime e2
    he1Relation hdPrime hv1Error hv2 he2Relation
  have he2Bounds := e2_bounds_from_scaled e1 e2 v2 dPrime r2 d
    he2Scaled ⟨he1Positive, he1Upper⟩ ⟨hv2Positive, hv2Upper⟩
    hdPrimeBounds ⟨by omega, hd.2⟩
  have hhalving := error_halving_from_witnesses
    v2 d e2 d0 d63 v2Half v2Bit error
    he2Relation hv2Decomp hv2Bit hd0 hd63 herrorWord
  have he2Half : e2 = 2 * error + epsilon := by
    rw [hepsilonDef]
    exact hhalving.1
  have hepsilon : epsilon < 2 := by
    rw [hepsilonDef]
    exact hhalving.2
  have he3Scaled := e3_scaled_from_witnesses
    v2 d e2 error epsilon hi lo q3 hiBit v3 e3 r3
    he2Relation he2Half hepsilon hv2Error hhi hhiBit hv3 hr3
    he3Relation hr3Bound
  have he3Bounds := e3_bounds_from_scaled e2 e3 v2 epsilon r3 d
    he3Scaled he2Bounds hv2Upper hd
  obtain ⟨e4, he4Relation, he4Bounds⟩ :=
    adjusted_error_exists d v3 v4 e3 he3Relation.symm he3Bounds hadjust
  exact ⟨
    final_error_implies_interval d v4 e4 he4Relation he4Bounds,
    ⟨e4, he4Relation, he4Bounds⟩
  ⟩

theorem seed_domain_bounds (d9 : Nat) (h : SeedDomain d9) : d9 < 2^9 := h.2
theorem v1_domain_bounds (v0 d40 : Nat) (h : V1Domain v0 d40) :
    v0 < 2^11 /\ d40 <= 2^40 := ⟨h.2.1, h.2.2.2⟩
theorem v2_domain_no_underflow (v1 d40 : Nat) (h : V2Domain v1 d40) :
    v1 * d40 <= 2^60 := h.2.2.2.2.1
theorem error_domain_parity (v2 d0 d63 : Nat) (h : ErrorDomain v2 d0 d63) :
    d0 = 0 \/ d0 = 1 := by
  unfold ErrorDomain at h
  omega
theorem v3_domain_bounds (v2 : Nat) (h : V3Domain v2) : v2 < 2^35 := h
theorem v4_domain_normalized (d : Nat) (h : V4Domain d) : 0 < d := by
  unfold V4Domain at h
  omega

theorem carry_certificate_implies_interval (d v ph pl : Nat)
    (h : CarryCertificate d v ph pl) : ReciprocalInterval d v := by
  rcases h with ⟨hprod, hph, hpl, hd, hhigh, hlow, hnext⟩
  unfold ReciprocalInterval
  simp only [Nat.add_mul, Nat.one_mul, hprod]
  unfold base at *
  omega

theorem interval_implies_carry_certificate (d v ph pl : Nat)
    (hprod : v * d = ph * base + pl) (_hph : ph < base) (hpl : pl < base)
    (hd : d < base) (hinterval : ReciprocalInterval d v) :
    d + ph < base /\ base <= pl + d /\ base <= d + ph + 1 := by
  unfold ReciprocalInterval at hinterval
  simp only [Nat.add_mul, Nat.one_mul, hprod] at hinterval
  unfold base at *
  omega

theorem reciprocal_certificate_characterization (d v ph pl : Nat)
    (hprod : v * d = ph * base + pl) (hph : ph < base) (hpl : pl < base)
    (hd : d < base) :
    CarryCertificate d v ph pl <-> ReciprocalInterval d v := by
  constructor
  · intro h
    rcases h with ⟨hprod', hph', hpl', hd', hhigh, hlow, hnext⟩
    exact carry_certificate_implies_interval d v ph pl
      ⟨hprod', hph', hpl', hd', hhigh, hlow, hnext⟩
  · intro hinterval
    have hcarries :=
      interval_implies_carry_certificate d v ph pl hprod hph hpl hd hinterval
    exact ⟨hprod, hph, hpl, hd, hcarries.1, hcarries.2.1, hcarries.2.2⟩

/-! Theorem 2's candidate-remainder bounds without signed arithmetic.

If `s` is the nonnegative quantity `rTilde + d`, the paper's identity is the
`hscaled` hypothesis below.  The two conclusions are precisely the facts the
two Kani correction branches consume: a nonnegative candidate is below
`max (B-d) q0`; a negative candidate `-(d-s)` has magnitude at most `d` and
its word residue `B-(d-s)` is strictly greater than `q0`. -/
theorem algorithm4_candidate_bounds
    (B d u1 u0 k q0 s : Nat)
    (hB : 0 < B) (hd : 0 < d) (hdB : d < B)
    (hu1 : u1 < d) (hu0 : u0 < B)
    (hk : 0 < k) (hkd : k <= d) (hq0 : q0 < B)
    (hscaled : B * s = u1 * k + u0 * (B - d) + q0 * d) :
    (d <= s -> s - d < max (B - d) q0) /\
      (s < d -> d - s <= d /\ q0 < B - (d - s)) := by
  have hdiff : 0 < B - d := by omega
  have hu1kLeft : u1 * k < d * k :=
    (Nat.mul_lt_mul_right hk).2 hu1
  have hu1k : u1 * k < d * d := by
    have := Nat.mul_le_mul_left d hkd
    omega
  have hu0Term : u0 * (B - d) < B * (B - d) :=
    (Nat.mul_lt_mul_right hdiff).2 hu0
  have hBsplit : d + (B - d) = B := by omega
  constructor
  · intro hds
    by_cases hq : q0 <= B - d
    · have hnumerator :
          u1 * k + u0 * (B - d) + q0 * d < B * B := by
        have hqTerm := Nat.mul_le_mul_right d hq
        have hceiling :
            d * d + B * (B - d) + (B - d) * d = B * B := by
          grind
        omega
      have hscaledLt : B * s < B * B := by omega
      have hsB : s < B := (Nat.mul_lt_mul_left hB).1 hscaledLt
      rw [Nat.max_eq_left hq]
      omega
    · have hqStrict : B - d < q0 := by omega
      have hgap : 0 < (B - d) * (q0 - (B - d)) :=
        Nat.mul_pos hdiff (by omega)
      have hqSplit : (B - d) + (q0 - (B - d)) = q0 := by omega
      have hidentity :
          B * (d + q0) =
            d * d + B * (B - d) + q0 * d +
              (B - d) * (q0 - (B - d)) := by
        grind
      have hnumerator :
          u1 * k + u0 * (B - d) + q0 * d < B * (d + q0) := by
        omega
      have hscaledLt : B * s < B * (d + q0) := by omega
      have hs : s < d + q0 := (Nat.mul_lt_mul_left hB).1 hscaledLt
      rw [Nat.max_eq_right (by omega)]
      omega
  · intro hsd
    constructor
    · omega
    · by_cases hgoal : q0 < B - (d - s)
      · exact hgoal
      · have hsum : B + s <= q0 + d := by omega
        let x := q0 + d - B
        have hx : x + B = q0 + d := by
          dsimp [x]
          omega
        have hsx : s <= x := by
          dsimp [x]
          omega
        have hqSplit : q0 + (B - q0) = B := by omega
        have hpositive : 0 < (B - d) * (B - q0) :=
          Nat.mul_pos hdiff (by omega)
        have hidentity : q0 * d = B * x + (B - d) * (B - q0) := by
          grind
        have hscaledLe : q0 * d <= B * s := by
          rw [hscaled]
          omega
        have hsxScaled : B * s <= B * x := Nat.mul_le_mul_left B hsx
        omega

/-! The reciprocal equation and the two-word estimate imply the scaled
candidate identity used above.  This is Theorem 2's substitution step, kept
subtraction-free so it is valid for both signs of the candidate remainder. -/
theorem algorithm4_scaled_identity
    (B d v k u1 u0 qBase q0 : Nat)
    (hB : 0 < B) (hdB : d < B)
    (hreciprocal : (B + v) * d + k = B * B)
    (hestimate : (B + v) * u1 + u0 = qBase * B + q0) :
    ∃ s,
      (qBase + 1) * d + s = B * u1 + u0 + d /\
        B * s = u1 * k + u0 * (B - d) + q0 * d := by
  have hBsplit : d + (B - d) = B := by omega
  have hbalance :
      B * (B * u1 + u0 + d) =
        B * ((qBase + 1) * d) +
          (u1 * k + u0 * (B - d) + q0 * d) := by
    grind
  have hcandidateScaled : B * ((qBase + 1) * d) <= B * (B * u1 + u0 + d) := by
    omega
  have hcandidate : (qBase + 1) * d <= B * u1 + u0 + d :=
    Nat.le_of_mul_le_mul_left hcandidateScaled hB
  let s := B * u1 + u0 + d - (qBase + 1) * d
  have hs : (qBase + 1) * d + s = B * u1 + u0 + d := by
    dsimp [s]
    omega
  refine ⟨s, hs, ?_⟩
  grind

/-! The full candidate-bound statement from Theorem 2, specialized to the
subtraction-free witness consumed by the Kani correction cases. -/
theorem algorithm4_theorem2
    (B d v k u1 u0 qBase q0 : Nat)
    (hB : 0 < B) (hd : 0 < d) (hdB : d < B)
    (hu1 : u1 < d) (hu0 : u0 < B)
    (hk : 0 < k) (hkd : k <= d) (hq0 : q0 < B)
    (hreciprocal : (B + v) * d + k = B * B)
    (hestimate : (B + v) * u1 + u0 = qBase * B + q0) :
    ∃ s,
      (qBase + 1) * d + s = B * u1 + u0 + d /\
        (d <= s -> s - d < max (B - d) q0) /\
          (s < d -> d - s <= d /\ q0 < B - (d - s)) := by
  obtain ⟨s, hs, hscaled⟩ :=
    algorithm4_scaled_identity B d v k u1 u0 qBase q0
      hB hdB hreciprocal hestimate
  have hbounds := algorithm4_candidate_bounds B d u1 u0 k q0 s
    hB hd hdB hu1 hu0 hk hkd hq0 hscaled
  exact ⟨s, hs, hbounds⟩

/-! Lean-side meaning of the two Kani correction outcomes.  In the
nonnegative case the first machine correction is either skipped or transient;
the second returns the Euclidean pair. -/
theorem algorithm4_nonnegative_result
    (B d u1 u0 q0 candidate s : Nat)
    (hd : 0 < d) (hdB : d < B) (hhalf : B <= 2 * d)
    (hu1 : u1 < d) (hu0 : u0 < B) (hq0 : q0 < B)
    (hcandidatePositive : 0 < candidate)
    (hcandidate : candidate * d + s = B * u1 + u0 + d)
    (hds : d <= s) (hrUpper : s - d < max (B - d) q0) :
    let r := s - d
    (r < d -> candidate * d + r = B * u1 + u0) /\
      (d <= r ->
        candidate + 1 < B /\
          (candidate + 1) * d + (r - d) = B * u1 + u0 /\ r - d < d) := by
  dsimp
  have hsSplit : d + (s - d) = s := by omega
  have hbase : candidate * d + (s - d) = B * u1 + u0 := by omega
  have hmaxB : max (B - d) q0 < B := by
    exact Nat.max_lt.mpr ⟨by omega, hq0⟩
  have hrB : s - d < B := by omega
  constructor
  · intro hr
    exact hbase
  · intro hdr
    have hdividend : B * u1 + u0 < B * d := by
      have hu1Succ : u1 + 1 <= d := by omega
      have hu1Scaled := Nat.mul_le_mul_left B hu1Succ
      have hstep : B * (u1 + 1) = B * u1 + B := by grind
      omega
    have hquotientScaled : (candidate + 1) * d <= B * u1 + u0 := by
      rw [Nat.add_mul, Nat.one_mul]
      omega
    have hquotient : candidate + 1 < B := by
      by_cases hge : B <= candidate + 1
      · have := Nat.mul_le_mul_right d hge
        omega
      · omega
    refine ⟨hquotient, ?_, by omega⟩
    rw [Nat.add_mul, Nat.one_mul]
    omega

/-! For a negative candidate `-(d-s)`, Theorem 2 makes the first correction
mandatory.  Its wrapping decrement represents the natural predecessor below,
and the corrected remainder is already in range. -/
theorem algorithm4_negative_result
    (B d u1 u0 candidate s : Nat)
    (_hd : 0 < d)
    (hcandidate : candidate * d + s = B * u1 + u0 + d)
    (hsd : s < d) :
    0 < candidate /\
      (candidate - 1) * d + s = B * u1 + u0 /\ s < d := by
  have hcandidatePositive : 0 < candidate := by
    by_cases hzero : candidate = 0
    · simp [hzero] at hcandidate
      omega
    · omega
  have hpred : candidate - 1 + 1 = candidate := by omega
  constructor
  · exact hcandidatePositive
  · constructor
    · rw [← hpred, Nat.add_mul, Nat.one_mul] at hcandidate
      omega
    · exact hsd

/-! Normalizing numerator and divisor by the same power-of-two factor cannot
change the Euclidean quotient.  The normalized remainder is necessarily a
multiple of that factor, so the production right shift recovers the original
remainder. -/
theorem scaled_euclidean_descends
    (factor n d q normalizedRemainder : Nat)
    (hfactor : 0 < factor)
    (hequation : q * (factor * d) + normalizedRemainder = factor * n)
    (hremainder : normalizedRemainder < factor * d) :
    ∃ r,
      normalizedRemainder = factor * r /\
        q * d + r = n /\ r < d := by
  have hqScaled : factor * (q * d) <= factor * n := by
    have : q * (factor * d) <= factor * n := by omega
    simpa [Nat.mul_assoc, Nat.mul_comm, Nat.mul_left_comm] using this
  have hq : q * d <= n := Nat.le_of_mul_le_mul_left hqScaled hfactor
  let r := n - q * d
  have hqr : q * d + r = n := by
    dsimp [r]
    omega
  have hrScale : normalizedRemainder = factor * r := by
    grind
  have hr : r < d := by
    have hscaled : factor * r < factor * d := by
      rw [← hrScale]
      exact hremainder
    exact (Nat.mul_lt_mul_left hfactor).1 hscaled
  exact ⟨r, hrScale, hqr, hr⟩

/-! The u64 mantissa divider computes `(a * 2^63) / d` and, only in the
lower binade, recovers one additional quotient bit from the remainder.  These
lemmas justify the modeled domain and post-divider cut used by
`proofs::div_norm_u64_correctly_rounded`. -/
theorem div_norm_u64_quotient_range (a d q r : Nat)
    (haLower : 2^63 <= a) (haUpper : a < 2^64)
    (hdLower : 2^63 <= d) (hdUpper : d < 2^64)
    (hr : r < d) (hequation : q * d + r = a * 2^63) :
    2^62 <= q /\ q < 2^64 := by
  constructor
  · by_cases hq : 2^62 <= q
    · exact hq
    · have hstep : q + 1 <= 2^62 := by omega
      have hquotient : (q + 1) * d <= 2^62 * d :=
        Nat.mul_le_mul_right d hstep
      have hsum : q * d + r < (q + 1) * d := by
        rw [Nat.add_mul, Nat.one_mul]
        omega
      have hdScaled : 2^62 * d < 2^62 * 2^64 :=
        (Nat.mul_lt_mul_left (by omega : 0 < (2^62 : Nat))).2 hdUpper
      have haScaled : 2^63 * 2^63 <= a * 2^63 :=
        Nat.mul_le_mul_right (2^63) haLower
      omega
  · by_cases hq : q < 2^64
    · exact hq
    · have hproduct : 2^64 * 2^63 <= q * d :=
        Nat.mul_le_mul (by omega) hdLower
      have haScaled : a * 2^63 < 2^64 * 2^63 :=
        (Nat.mul_lt_mul_right (by omega : 0 < (2^63 : Nat))).2 haUpper
      omega

theorem div_norm_u64_extra_bit (q d r : Nat)
    (hd : 0 < d) (hr : r < d) :
    ∃ bit r2,
      bit < 2 /\
        (bit = 1 <-> d <= 2 * r) /\
          (2 * q + bit) * d + r2 = 2 * (q * d + r) /\ r2 < d := by
  by_cases hbit : d <= 2 * r
  · refine ⟨1, 2 * r - d, by omega, by omega, ?_, by omega⟩
    grind
  · refine ⟨0, 2 * r, by omega, by omega, ?_, by omega⟩
    grind

theorem div_norm_round_down (q d r : Nat) (hr : r < d) :
    q * d <= q * d + r /\ q * d + r - q * d < d := by
  omega

theorem div_norm_round_up (q d r : Nat) (hr : r < d) :
    q * d + r <= (if r = 0 then q * d else (q + 1) * d) /\
      (if r = 0 then q * d else (q + 1) * d) - (q * d + r) < d := by
  by_cases hr0 : r = 0 <;> simp [hr0, Nat.add_mul, Nat.one_mul] <;> omega

theorem div_norm_round_nearest_down (q d r : Nat)
    (_hr : r < d) (hbelow : 2 * r < d) :
    2 * (q * d + r - q * d) < d := by
  omega

theorem div_norm_round_nearest_up (q d r : Nat)
    (hr : r < d) (habove : d <= 2 * r) :
    2 * ((q + 1) * d - (q * d + r)) <= d := by
  rw [Nat.add_mul, Nat.one_mul]
  omega

theorem euclidean_unique (n d q r q' r' : Nat)
    (_hd : 0 < d) (hr : r < d) (hr' : r' < d)
    (hqr : q * d + r = n) (hqr' : q' * d + r' = n) :
    q = q' /\ r = r' := by
  have q_le_q' : q <= q' := by
    by_cases hlt : q' < q
    · have hstep : q' + 1 <= q := by omega
      have hmul : (q' + 1) * d <= q * d := Nat.mul_le_mul_right d hstep
      have hbound : q' * d + r' < (q' + 1) * d := by
        rw [Nat.add_mul, Nat.one_mul]
        omega
      omega
    · omega
  have q'_le_q : q' <= q := by
    by_cases hlt : q < q'
    · have hstep : q + 1 <= q' := by omega
      have hmul : (q + 1) * d <= q' * d := Nat.mul_le_mul_right d hstep
      have hbound : q * d + r < (q + 1) * d := by
        rw [Nat.add_mul, Nat.one_mul]
        omega
      omega
    · omega
  have hq : q = q' := Nat.le_antisymm q_le_q' q'_le_q
  subst q'
  constructor
  · rfl
  · omega

end Div2x1
