/-!
# GxSpec.Core — F0 base category skeleton

Identity: a plain-Prop model of `req/spec/10-concept/12-formal-semantics.md` F0's category `C`
(object = ObjectSnapshot, morphism = Transformation). Mathlib.CategoryTheory is not imported
(policy of `req/spec/40-architecture/46-verification-plan.md` §2, ASM-46-1). (sem: SEM-lean-036)

This file defines only composable morphisms (`Transformation`) and their composition (`compose`)
and identity (`identity`). The admissibility predicate (`A`), the invariant (`I`), witness/Θ, and
the receipt are layered on in the modules that follow (`Admissible.lean`/`Invariant.lean`/
`Canon.lean`/`Receipt.lean`). (sem: SEM-lean-037)

M8-a stage: definitions only (no theorems). There is no unproven placeholder (because no
theorem exists in this file). (sem: SEM-lean-038)
-/

namespace GxSpec

/-- A content-addressed snapshot. In F0, only the equality of digest is meaningful
    (a semantic subset of 42's ObjectSnapshot structure). (sem: SEM-lean-039) -/
structure ObjectSnapshot where
  digest : ByteArray
  deriving DecidableEq

structure TransformationId where
  cid : ByteArray
  deriving DecidableEq

/-- A transformation = a first-class object (P-1, P-2). order corresponds to 41's
    Transformation.order. provenance/context/actor and the like carry no weight for F0's theorems,
    so they are omitted from the type as opaque metadata rather than carried
    (a deliberate narrowing of the formalization scope, see 46 §1 non-goal). (sem: SEM-lean-040) -/
structure Transformation where
  id    : TransformationId
  order : Nat
  src   : ObjectSnapshot
  dst   : ObjectSnapshot

/-- Composability: composition is possible only when f.dst = g.src (morphism composition
    of a wide subcategory). (sem: SEM-lean-041) -/
def composable (f g : Transformation) : Prop := f.dst = g.src

/-- Composition. Since id is computed on the Rust side as the canonical CID, the Lean side
    carries, as a hypothesis, a function `composeId` that yields the post-composition id
    (used on the `Admissible.lean` side). (sem: SEM-lean-042) -/
axiom composeId : TransformationId → TransformationId → TransformationId

-- Since composeId is an axiom (carries no computational content), compose cannot be a
-- codegen target either. This skeleton is proof-only within the model and is never run, so
-- noncomputable is correct. (sem: SEM-lean-043)
noncomputable def compose (f g : Transformation) (h : composable f g) : Transformation :=
  { id := composeId f.id g.id, order := max f.order g.order, src := f.src, dst := g.dst }

def identity (x : ObjectSnapshot) (idOf : ObjectSnapshot → TransformationId) : Transformation :=
  { id := idOf x, order := 0, src := x, dst := x }

end GxSpec
