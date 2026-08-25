import GxSpec.Core

/-!
# GxSpec.Admissible — the admissibility predicate A and T1 (composition soundness) (sem: SEM-lean-007)

Identity: a plain-Prop model of `12-formal-semantics.md` F0's "`A : MorphismProperty C`,
`A.IsMultiplicative` (identity-containing, composition-closed) -> admissible morphisms form a wide
subcategory". Rust-side counterpart: stands abstractly for the composed result of gx-gate (Cedar
evaluation + invariant registry) -- Cedar itself is not formalized in Lean (46 §1 non-goal).
(sem: SEM-lean-008)

T1 (composition soundness: `A(f) ∧ A(g) → A(g∘f)`) states `46-verification-plan.md` §2.2's
proposition verbatim. (sem: SEM-lean-009)

**`req/38` §85 ruling (§85-1, M8-b fix directives 1-2)**: T1 is exactly an application of
`Admissible`'s second component (composition-closure) -- the proof is the one line `hA.2 f g h` --
and was ruled a "definitional projection" (honestly stated, per P-9, as not a deep theorem). As a
frozen-module condition, the non-vacuity witness that its hypothesis `Admissible A idOf` is not
unsatisfiable (a concrete instance witnessing the ∀ is non-empty) is placed alongside it as
`orderBounded_admissible` (the same-named theorem from `req/lean/GxSpec/Admissible.lean`, ported
within the §85 canonical form's scope -- ruling 3, a read-only substrate). (sem: SEM-lean-010)
-/

namespace GxSpec

/-- The admissibility predicate A(f). Stands abstractly for the composed result of gx-gate
    (Cedar evaluation + invariant registry) -- Cedar itself is not formalized (see 46 §1 non-goal).
    (sem: SEM-lean-011) -/
def Admissible (A : Transformation → Prop) (idOf : ObjectSnapshot → TransformationId) : Prop :=
  (∀ x : ObjectSnapshot, A (identity x idOf)) ∧
  (∀ f g : Transformation, ∀ h : composable f g, A f → A g → A (compose f g h))

/-- T1 composition soundness: A(f) ∧ A(g) → A(g∘f). The basis for the gate's composition
    soundness (12 F0). Proof = exactly an application of `Admissible`'s second component
    (a definitional projection, §85 ruling 1). (sem: SEM-lean-012) -/
theorem T1_composition_sound
    {A : Transformation → Prop} {idOf : ObjectSnapshot → TransformationId}
    (hA : Admissible A idOf) (f g : Transformation) (h : composable f g) :
    A f → A g → A (compose f g h) :=
  hA.2 f g h

/-- Admissibility via an order bound (the shape corresponding to ASM-6's order ≤ 2 / DR-7).
    (sem: SEM-lean-013) -/
def OrderBounded (n : Nat) (f : Transformation) : Prop := f.order ≤ n

/-- The non-vacuity witness that T1's hypothesis `Admissible A idOf` is not unsatisfiable
    (§85 ruling 1). Since `compose` takes the max of orders and `identity` has order 0, an order
    bound is both identity-containing and composition-closed -- it does not give the gate's
    semantics itself, only a constructive instance. (sem: SEM-lean-014) -/
theorem orderBounded_admissible (n : Nat) (idOf : ObjectSnapshot → TransformationId) :
    Admissible (OrderBounded n) idOf := by
  constructor
  · intro x
    show (identity x idOf).order ≤ n
    exact Nat.zero_le n
  · intro f g _h hf hg
    have hf' : f.order ≤ n := hf
    have hg' : g.order ≤ n := hg
    show max f.order g.order ≤ n
    omega

end GxSpec
