import GxSpec.Core

/-!
# GxSpec.Invariant — the invariant I, Hoare triples, and T2 (composition) (sem: SEM-lean-082)

Identity: a plain-Prop model of `12-formal-semantics.md` F0's "the invariant is an indexed
family of the spec (fibration-wise), `I : Obj → Prop`; the sequential composition rule for the
Hoare triple `{I} f {J}`". Rust-side counterpart: the abstraction of the predicates 43's state
machine must preserve. (sem: SEM-lean-083)

T2 (Hoare composition: `{I}f{J} ∧ {J}g{K} → {I}(g∘f){K}`) states `46-verification-plan.md`
§2.3's proposition verbatim. (sem: SEM-lean-084)

**`req/38` §85 ruling (§85-2, M8-b fix directive 1)**: T2 = substantive, frozen unmodified --
the proof genuinely goes through `compose`'s src/dst equation (`composable`'s rewrite), making it
non-trivial as a wiring check of the Core model (unlike T1/T5, it is not a definitional
projection). It is proved exactly as-is, the same proposition it carried when frozen.
(sem: SEM-lean-085)
-/

namespace GxSpec

/-- The invariant I(X). The abstraction of the predicates 43's state machine must preserve.
    (sem: SEM-lean-086) -/
def Invariant := ObjectSnapshot → Prop

/-- The Hoare triple {I} f {J}. (sem: SEM-lean-087) -/
def HoareTriple (I : Invariant) (f : Transformation) (J : Invariant) : Prop :=
  I f.src → J f.dst

/-- T2 invariant composition: {I}f{J} ∧ {J}g{K} → {I}(g∘f){K}. Rewrites `J f.dst` to `J g.src`
    via `composable f g` (`f.dst = g.src`), then chains into `hg` (§85 ruling 2 = a wiring check
    of the Core model, frozen unmodified). (sem: SEM-lean-088) -/
theorem T2_invariant_composition
    (I J K : Invariant) (f g : Transformation) (h : composable f g) :
    HoareTriple I f J → HoareTriple J g K → HoareTriple I (compose f g h) K := by
  intro hf hg hI
  have heq : f.dst = g.src := h
  have hJ : J f.dst := hf hI
  have hJ' : J g.src := heq ▸ hJ
  exact hg hJ'

end GxSpec
