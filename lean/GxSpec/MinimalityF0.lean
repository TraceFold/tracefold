import GxSpec.Core
import GxSpec.Admissible
import GxSpec.Invariant
import GxSpec.Canon
import GxSpec.Receipt
import GxSpec.Minimality

/-!
# GxSpec.MinimalityF0 — F0 field-level minimality (irreducibility) audit

Identity: the repayment lane for `req/188_BOUNDARY_ORDER_CANON_2026-08-15.md` §2 **U4** ("the
irreducibility of the F0 field set = not audited"), dispatched as parallel lane 5 of `req/38` §124;
report = `req/191_F0_MINIMALITY_AUDIT_2026-08-15.md`. Where `GxSpec.Minimality` (Rule 1's three
powers) and `GxSpec.Injection`/`InjectionRng` (Rule 2's injection point) audited the minimality of
the **membrane**, this file audits the **F0 surface itself**: for every field of every structure
the frozen five modules (`Core`/`Admissible`/`Invariant`/`Canon`/`Receipt`) define, decide whether
on the variant with that one field removed at least one theorem of the theorem ledger that reads
the field (`req/188` §2: T1-T5, `orderBounded_admissible`, `recoverableChain_witness`, the three
Rule 1 counterexamples) stops holding (= **NEEDED**), or whether every theorem still holds with the
field removed (= **REDUNDANT-CANDIDATE**).

## Stance (P-9 — inherited verbatim from `Minimality.lean` / `Injection.lean`)

* Every "NEEDED" verdict below is a **counterexample construction** on a *variant type with exactly
  one field removed* — never an impossibility theorem over all conceivable designs. Every "REDUNDANT-CANDIDATE"
  verdict is a **survival proof**: the variant still satisfies the T-shaped statement, proved
  directly. Completeness of the audited field set is claimed only in the trivial sense that the
  frozen structures have finitely many fields and each is listed in `req/191` §1's table (with the
  ones handled by argument rather than theorem named there, not silently omitted).
* **Frozen modules are used by `import` only** (the same C1 discipline as `Injection.lean`): no
  frozen file is edited, no frozen name is shadowed, every frozen theorem stands unchanged; the
  axiom set stays `{propext, Quot.sound, GxSpec.composeId}` (`#print axioms` in `req/191` §3);
  proof-placeholder grep 0 (the word itself is avoided here so AC-061's substring gate stays clean).
  `GxSpec.Minimality` is imported for its fixtures (`x1`/`x2`/`trivialProof`/`order5Txn`) so that no
  fresh admissibility class or content corpus is invented (P-9 "no new concepts").
* **Two roles for variant vocabulary, declared** (`req/160` §3-1 C2 was written for *injection*
  extensions; this file is *deletion* variants, so the analogous discipline is restated rather than
  borrowed): a variant appears either as the *broken* thing (a counterexample, "NEEDED") or as the
  *surviving* thing ("still holds with the field removed", "REDUNDANT-CANDIDATE"). Neither role asserts a positive property **of gx**;
  the survival theorems assert a property of the *variant*, used only to classify the field. No
  theorem here strengthens, weakens or restates any frozen theorem about the frozen types.
* **Self-kill applied in construction ("has the counterexample slid from field *deletion* into type *weakening*?")**: every
  variant is a literal `structure` with one field fewer, and where the frozen theorem becomes
  *unstatable* on the variant (the field is what the statement is *about*), the file says so and
  machine-checks the **nearest forced consequence** — pinned by a definitional bridge (`rfl`) from
  the variant to that consequence, the same device `Injection.lean` uses (`timed_factors_iff` is
  `Iff.rfl`). Where the frozen theorem stays statable but its hypothesis becomes a *tautology*
  ("trivialised"), the tautology is proved and paired with a frozen-side witness that the hypothesis is
  *not* a tautology there — so "NEEDED (trivialised)" is a machine-checked difference, not a reading.

## Verdict legend (used in `req/191` §1's table; the theorem name is the coordinate)

| verdict | meaning |
|---|---|
| NEEDED (breaks) | a T-shaped statement is **false** on the variant (concrete counterexample) |
| NEEDED (unstatable) | the depending theorem is **unstatable** on the variant; nearest forced consequence machine-checked |
| NEEDED (trivialised) | the depending theorem's hypothesis becomes a **tautology** on the variant (theorem loses content) |
| REDUNDANT-CANDIDATE (theory), DESIGN-REQUIRED | all depending theorems **survive** on the variant (survival proved); the field is required by 41/42/46 for a reason outside T1-T5, cited in `req/191` |

## Relation to the identity surface (42 §1.3 IdentityView — the brief's "TransformationView")

42 §1.3 already practises field-level minimality on the *identity* axis: `created_at` is excluded
from `Transformation`'s IdentityView (ASM-4) because it records *when* and not *what* — and F0's
`Transformation` (`Core.lean`) carries no timestamp field at all. This file is the same question
asked one level down, on the *theorem* axis: for the fields F0 **does** carry, which does the set
of theorems actually consume? `Injection.lean` is the bridge between the two axes: it shows what
happens when a timestamp is added *and read* (T4-shape breaks) — i.e. 42 §1.3's exclusion is the
identity-side face of the same minimality this file audits on the theorem side.
-/

namespace GxSpec.MinimalityF0

open GxSpec
open GxSpec.Minimality (x1 x2 x1_ne_x2 trivialProof trivialProof_sound order5Txn)

-- =============================================================================================
-- § 1. `Transformation.src` / `Transformation.dst` — NEEDED (unstatable): T2 (`HoareTriple`) and
-- `composable` are *about* these fields. Removing either makes `composable` undefinable, so the
-- only composition the variant can carry is the **unchecked** one (no gluing side condition), and
-- T2's shape is false for it. The bridge lemmas pin, by `rfl`, that the variant's compose *is* the
-- unchecked compose seen through the forgetful projection.
-- =============================================================================================

/-- `Transformation` with `src` removed. -/
structure TransformationNoSrc where
  id    : TransformationId
  order : Nat
  dst   : ObjectSnapshot

/-- `Transformation` with `dst` removed. -/
structure TransformationNoDst where
  id    : TransformationId
  order : Nat
  src   : ObjectSnapshot

def forgetSrc (t : Transformation) : TransformationNoSrc := ⟨t.id, t.order, t.dst⟩

def forgetDst (t : Transformation) : TransformationNoDst := ⟨t.id, t.order, t.src⟩

/-- The only composition the `src`-less variant can define: with no `src` on `g` there is nothing
to glue `f.dst` against, so composition is total. -/
noncomputable def composeNoSrc (f g : TransformationNoSrc) : TransformationNoSrc :=
  { id := composeId f.id g.id, order := max f.order g.order, dst := g.dst }

/-- Likewise for the `dst`-less variant (no `f.dst` to glue against `g.src`). -/
noncomputable def composeNoDst (f g : TransformationNoDst) : TransformationNoDst :=
  { id := composeId f.id g.id, order := max f.order g.order, src := f.src }

/-- The frozen-typed image of both variant compositions: `compose` with the gluing side condition
dropped. On composable pairs it *is* the frozen `compose` (`composeUnchecked_eq_compose`); on
non-composable pairs it is what a `src`-less or `dst`-less variant is forced to accept. -/
noncomputable def composeUnchecked (f g : Transformation) : Transformation :=
  { id := composeId f.id g.id, order := max f.order g.order, src := f.src, dst := g.dst }

theorem composeUnchecked_eq_compose (f g : Transformation) (h : composable f g) :
    composeUnchecked f g = compose f g h := rfl

/-- Bridge (`src`): the variant's compose is the unchecked compose, definitionally. -/
theorem noSrc_compose_is_unchecked (f g : Transformation) :
    composeNoSrc (forgetSrc f) (forgetSrc g) = forgetSrc (composeUnchecked f g) := rfl

/-- Bridge (`dst`): the variant's compose is the unchecked compose, definitionally. -/
theorem noDst_compose_is_unchecked (f g : Transformation) :
    composeNoDst (forgetDst f) (forgetDst g) = forgetDst (composeUnchecked f g) := rfl

/-- Two loops on two different contents: individually each preserves "at `x1`" (the first
trivially, the second vacuously), and they are **not** composable (`x1 ≠ x2`). -/
def fLoop : Transformation := { id := ⟨ByteArray.mk #[31]⟩, order := 0, src := x1, dst := x1 }

def gLoop : Transformation := { id := ⟨ByteArray.mk #[32]⟩, order := 0, src := x2, dst := x2 }

/-- The one invariant of this counterexample: "the snapshot is `x1`". -/
def atX1 : Invariant := fun s => s = x1

theorem fLoop_gLoop_not_composable : ¬ composable fLoop gLoop := fun h => x1_ne_x2 h

/-- Field `src` — NEEDED (unstatable). T2 is unstatable on `TransformationNoSrc` (no precondition site, no
gluing site); its forced consequence — composition without the gluing check
(`noSrc_compose_is_unchecked`) — breaks Hoare composition on a concrete pair: `fLoop` and `gLoop`
each preserve `atX1`, they are not composable, and their unchecked composite does not preserve
`atX1` (it starts at `x1` and ends at `x2`). -/
theorem srcField_dropped_gluing_counterexample :
    (∀ f g, composeNoSrc (forgetSrc f) (forgetSrc g) = forgetSrc (composeUnchecked f g)) ∧
      ¬ composable fLoop gLoop ∧
      HoareTriple atX1 fLoop atX1 ∧ HoareTriple atX1 gLoop atX1 ∧
      ¬ HoareTriple atX1 (composeUnchecked fLoop gLoop) atX1 := by
  refine ⟨noSrc_compose_is_unchecked, fLoop_gLoop_not_composable, ?_, ?_, ?_⟩
  · intro h; exact h
  · intro h; exact h
  · intro h
    have h' : (composeUnchecked fLoop gLoop).dst = x1 := h rfl
    exact x1_ne_x2 h'.symm

/-- Field `dst` — NEEDED (unstatable). Same forced consequence via `noDst_compose_is_unchecked` (on the
`dst`-less variant even T2's *postcondition* site is gone); same concrete breakage. -/
theorem dstField_dropped_gluing_counterexample :
    (∀ f g, composeNoDst (forgetDst f) (forgetDst g) = forgetDst (composeUnchecked f g)) ∧
      ¬ composable fLoop gLoop ∧
      HoareTriple atX1 fLoop atX1 ∧ HoareTriple atX1 gLoop atX1 ∧
      ¬ HoareTriple atX1 (composeUnchecked fLoop gLoop) atX1 :=
  ⟨noDst_compose_is_unchecked, srcField_dropped_gluing_counterexample.2.1,
    srcField_dropped_gluing_counterexample.2.2.1, srcField_dropped_gluing_counterexample.2.2.2.1,
    srcField_dropped_gluing_counterexample.2.2.2.2⟩

-- =============================================================================================
-- § 2. `Transformation.order` — REDUNDANT-CANDIDATE (theory), DESIGN-REQUIRED. No T1-T5 statement reads `order`; the
-- variant without it carries composable/compose/identity/Admissible/HoareTriple unchanged and T2
-- survives with the frozen proof verbatim. What *does* read `order`: `orderBounded_admissible`
-- (T1's non-vacuity witness), Minimality Powers 2/3 (`order5Txn`), and the differential-test
-- runner (`Runner.lean` `admissibility` kind). All are *restatable* without it — the witness below
-- shows T1's non-vacuity survives via a content-shaped class — so the field is theoretically
-- redundant for the theorem set and required by design (41 `Transformation.order` / ASM-6 / DR-7:
-- gated meta-changes; `req/191` §1 cites the lines).
-- =============================================================================================

/-- `Transformation` with `order` removed. -/
structure TransformationNoOrder where
  id  : TransformationId
  src : ObjectSnapshot
  dst : ObjectSnapshot

def composableNO (f g : TransformationNoOrder) : Prop := f.dst = g.src

noncomputable def composeNO (f g : TransformationNoOrder) (_h : composableNO f g) :
    TransformationNoOrder :=
  { id := composeId f.id g.id, src := f.src, dst := g.dst }

def identityNO (x : ObjectSnapshot) (idOf : ObjectSnapshot → TransformationId) :
    TransformationNoOrder :=
  { id := idOf x, src := x, dst := x }

def AdmissibleNO (A : TransformationNoOrder → Prop) (idOf : ObjectSnapshot → TransformationId) :
    Prop :=
  (∀ x : ObjectSnapshot, A (identityNO x idOf)) ∧
  (∀ f g : TransformationNoOrder, ∀ h : composableNO f g, A f → A g → A (composeNO f g h))

def HoareTripleNO (I : Invariant) (f : TransformationNoOrder) (J : Invariant) : Prop :=
  I f.src → J f.dst

/-- Field `order` — survival of T2 on the variant (frozen proof, verbatim). -/
theorem orderField_dropped_T2_survives
    (I J K : Invariant) (f g : TransformationNoOrder) (h : composableNO f g) :
    HoareTripleNO I f J → HoareTripleNO J g K → HoareTripleNO I (composeNO f g h) K := by
  intro hf hg hI
  have heq : f.dst = g.src := h
  have hJ : J f.dst := hf hI
  have hJ' : J g.src := heq ▸ hJ
  exact hg hJ'

/-- Field `order` — survival of T1 (definitional projection, as §85 ruled for the frozen T1). -/
theorem orderField_dropped_T1_survives
    {A : TransformationNoOrder → Prop} {idOf : ObjectSnapshot → TransformationId}
    (hA : AdmissibleNO A idOf) (f g : TransformationNoOrder) (h : composableNO f g) :
    A f → A g → A (composeNO f g h) :=
  hA.2 f g h

/-- Field `order` — T1's non-vacuity survives without it: the content-shaped class "loop"
(`src = dst`) is identity-containing and composition-closed on the variant. This is the
restatement that replaces `orderBounded_admissible` when `order` is absent (so the witness role,
too, does not *need* the field). -/
theorem orderField_dropped_admissible_witness (idOf : ObjectSnapshot → TransformationId) :
    AdmissibleNO (fun t => t.src = t.dst) idOf := by
  constructor
  · intro x; rfl
  · intro f g h hf hg
    show f.src = g.dst
    exact hf.trans (h.trans hg)

-- =============================================================================================
-- § 3. `Transformation.id` — REDUNDANT-CANDIDATE (theory), DESIGN-REQUIRED. No T1-T5 statement reads `.id` (T4/T5 key
-- the ledger by `TransformationId` values that arrive through `ofId`/`recover`, never by reading
-- a `Transformation`'s own field; `Runner.lean` sets `id := ⟨#[]⟩` everywhere for the same
-- reason). The variant without it needs **no `composeId`** — the axiom exists solely to fill this
-- field at composition — so its T2 is axiom-free (`#print axioms` in `req/191` §3): U1
-- (`composeId` = axiom) and U4 (this field) are the same debt seen from two sides. Design-required
-- by 42 §1.3 (`Transformation.id` = CID of the IdentityView, Rule 1's minting coordinate; the frozen
-- fixtures of Minimality Power 3 / Injection name their ledger key through it).
-- =============================================================================================

/-- `Transformation` with `id` removed. -/
structure TransformationNoId where
  order : Nat
  src   : ObjectSnapshot
  dst   : ObjectSnapshot

def composableNI (f g : TransformationNoId) : Prop := f.dst = g.src

/-- Computable — no `composeId` needed once there is no `id` to mint for the composite. -/
def composeNI (f g : TransformationNoId) (_h : composableNI f g) : TransformationNoId :=
  { order := max f.order g.order, src := f.src, dst := g.dst }

/-- Needs no `idOf` either. -/
def identityNI (x : ObjectSnapshot) : TransformationNoId := { order := 0, src := x, dst := x }

def AdmissibleNI (A : TransformationNoId → Prop) : Prop :=
  (∀ x : ObjectSnapshot, A (identityNI x)) ∧
  (∀ f g : TransformationNoId, ∀ h : composableNI f g, A f → A g → A (composeNI f g h))

def HoareTripleNI (I : Invariant) (f : TransformationNoId) (J : Invariant) : Prop :=
  I f.src → J f.dst

/-- Field `id` — survival of T2 on the variant, **without the `composeId` axiom**. -/
theorem idField_dropped_T2_survives
    (I J K : Invariant) (f g : TransformationNoId) (h : composableNI f g) :
    HoareTripleNI I f J → HoareTripleNI J g K → HoareTripleNI I (composeNI f g h) K := by
  intro hf hg hI
  have heq : f.dst = g.src := h
  have hJ : J f.dst := hf hI
  have hJ' : J g.src := heq ▸ hJ
  exact hg hJ'

/-- Field `id` — T1's non-vacuity witness (`orderBounded_admissible`) survives verbatim on the
variant, needing neither `id` nor `idOf`. -/
theorem idField_dropped_orderBounded_admissible (n : Nat) :
    AdmissibleNI (fun t => t.order ≤ n) := by
  constructor
  · intro x
    show (identityNI x).order ≤ n
    exact Nat.zero_le n
  · intro f g _h hf hg
    have hf' : f.order ≤ n := hf
    have hg' : g.order ≤ n := hg
    show max f.order g.order ≤ n
    omega

-- =============================================================================================
-- § 4. `ObjectSnapshot.digest` — NEEDED (unstatable). It is the *only* field, so the variant is a one-point
-- type: F0's object identity ("only digest equality carries meaning", `Core.lean`) collapses, no two
-- contents are distinguishable, and Minimality Power 1's whole apparatus (`x1 ≠ x2`, `contentA`,
-- the colliding minting map) has no analogue — the Rule 1 minting audit is unstatable. T1/T2/T4/T5
-- remain *statable* on the collapsed model (and hold), which is exactly why this is "unstatable" of the
-- minting audit and not "breaks" of T1-T5.
-- =============================================================================================

/-- `ObjectSnapshot` with `digest` removed: no fields remain. -/
structure ObjectSnapshotNoDigest where

/-- Field `digest` — every two snapshots of the variant are equal, so no predicate on it can
separate two contents (the second conjunct is the exact shape `contentA`/`x1_ne_x2` need and
cannot have). -/
theorem digestField_dropped_snapshots_collapse :
    (∀ a b : ObjectSnapshotNoDigest, a = b) ∧
      (∀ (P : ObjectSnapshotNoDigest → Prop) (a b : ObjectSnapshotNoDigest), P a → P b) := by
  refine ⟨fun a b => ?_, fun P a b h => ?_⟩
  · cases a; cases b; rfl
  · cases a; cases b; exact h

-- =============================================================================================
-- § 5. `TransformationId.cid` — NEEDED (trivialised). Also the only field; the variant is a one-point id
-- space, and there T5's hypothesis `RecoverableChain` is a **tautology** (every `recover` splits a
-- transformation into "itself and itself", already contained), so T5 says nothing. On the frozen
-- surface the same hypothesis is *not* a tautology (`frozen_T5_hypothesis_not_tautology`), which is
-- the machine-checked difference this verdict rests on.
-- =============================================================================================

/-- `TransformationId` with `cid` removed: no fields remain. -/
structure TransformationIdNoCid where

/-- `Ledger` over the one-point id space (shape unchanged). -/
structure LedgerNC where
  contains : TransformationIdNoCid → Verdict → Prop

/-- T5's hypothesis, restated over the variant (shape unchanged). -/
def RecoverableChainNC
    (L : LedgerNC)
    (recover : TransformationIdNoCid → Option (TransformationIdNoCid × TransformationIdNoCid)) :
    Prop :=
  ∀ tcomp t1 t2 v, recover tcomp = some (t1, t2) → L.contains tcomp v →
    ∃ v1 v2, L.contains t1 v1 ∧ L.contains t2 v2

/-- Field `cid` — on the variant, `RecoverableChainNC` holds for **every** ledger and **every**
`recover`: T5's hypothesis is a tautology, T5 loses its content. -/
theorem cidField_dropped_T5_hypothesis_tautology
    (L : LedgerNC)
    (recover : TransformationIdNoCid → Option (TransformationIdNoCid × TransformationIdNoCid)) :
    RecoverableChainNC L recover := by
  intro tcomp t1 t2 v _ hc
  cases tcomp; cases t1; cases t2
  exact ⟨v, v, hc, hc⟩

/-- Frozen-side contrast: with `cid` present, `RecoverableChain` is falsifiable — the very
`recover` of `RecoverableChainWitness` (splits `idComp` into `(idA, idB)`) against a ledger that
contains only `idComp` (neither constituent) refutes it. So the tautology above is a property the
deletion *creates*, not one the frozen surface already had. -/
theorem frozen_T5_hypothesis_not_tautology :
    ∃ (L : Ledger) (recover : TransformationId → Option (TransformationId × TransformationId)),
      ¬ RecoverableChain L recover := by
  refine ⟨⟨fun tid v => tid = RecoverableChainWitness.idComp ∧ v = Verdict.admit⟩,
    RecoverableChainWitness.recover, ?_⟩
  intro hRec
  have hsplit : RecoverableChainWitness.recover RecoverableChainWitness.idComp =
      some (RecoverableChainWitness.idA, RecoverableChainWitness.idB) := rfl
  obtain ⟨_, _, h1, _⟩ := hRec RecoverableChainWitness.idComp RecoverableChainWitness.idA
    RecoverableChainWitness.idB Verdict.admit hsplit ⟨rfl, rfl⟩
  have hne : RecoverableChainWitness.idA ≠ RecoverableChainWitness.idComp := by decide
  exact hne h1.1

-- =============================================================================================
-- § 6. `Receipt.v` — NEEDED (breaks). Without the receipt's own verdict, validity can only mean "the
-- proof verifies *some* entry for `r.t`", T4's `hAdmit` (the read of `r.v`) has no site, and a
-- receipt over a **deny** entry passes every remaining hypothesis while the transformation is not
-- admissible: the T4-shape is false. Fixtures reuse Minimality's `OrderBounded 0` / `order5Txn`
-- (the same class T1's non-vacuity witness and Powers 2/3 use) — no fresh admissibility notion.
-- =============================================================================================

/-- `Receipt` with `v` removed. -/
structure ReceiptNoV where
  t     : TransformationId
  proof : InclusionProof

/-- The only validity the variant can express: the proof verifies *some* verdict for `r.t`. -/
def ValidReceiptNoV (L : Ledger) (r : ReceiptNoV) : Prop := ∃ v, r.proof.verifies L r.t v

/-- A ledger holding exactly one entry, `(t, deny)`. Nothing here is broken: it never wrote
`admit`, so `LedgerAdmitsOnlyAdmissible` holds for it **vacuously**, for any class and any `ofId`. -/
def denyOnlyLedger (t : TransformationId) : Ledger := ⟨fun tid v => tid = t ∧ v = Verdict.deny⟩

theorem denyOnlyLedger_admitsOnlyAdmissible
    (A : Transformation → Prop) (t : TransformationId) (ofId : TransformationId → Transformation) :
    LedgerAdmitsOnlyAdmissible A (denyOnlyLedger t) ofId := by
  intro tid h
  exact Verdict.noConfusion h.2

/-- Field `v` — T4-shape counterexample on the variant: ledger-soundness holds, the `v`-less
receipt is valid (its `trivialProof` verifies the deny entry), the proof is sound, and yet the
resolved transformation (`order5Txn`) is not `OrderBounded 0`-admissible. -/
theorem receiptV_dropped_T4_counterexample (t : TransformationId) :
    LedgerAdmitsOnlyAdmissible (OrderBounded 0) (denyOnlyLedger t) (fun _ => order5Txn) ∧
      ValidReceiptNoV (denyOnlyLedger t) ⟨t, trivialProof⟩ ∧
      ProofSound trivialProof ∧
      ¬ OrderBounded 0 order5Txn := by
  refine ⟨denyOnlyLedger_admitsOnlyAdmissible _ t _, ⟨Verdict.deny, rfl, rfl⟩,
    trivialProof_sound, ?_⟩
  intro h
  have h' : order5Txn.order ≤ 0 := h
  exact absurd h' (by decide)

-- =============================================================================================
-- § 7. `Verdict.deny` / `Verdict.escalate` — REDUNDANT-CANDIDATE (theory), DESIGN-REQUIRED, each. No theorem statement
-- names either constructor; with both removed (`Verdict` = {admit}) T4 holds and its `hAdmit`
-- becomes derivable. What each *does* on the frozen surface is make `hAdmit` load-bearing: a
-- receipt carrying a non-admit verdict passes every other T4 hypothesis and does **not** certify
-- admissibility. Two contrast theorems, one per constructor (1 field = 1 theorem), plus the
-- survival theorem on the one-constructor variant. Design necessity: 11 §4 (three-valued verdict) / ASM-14
-- (VerdictReceipt for every verdict) / 42 §3.8 (`EscalationTicket`) — cited in `req/191`.
-- =============================================================================================

def denyReceipt (t : TransformationId) : Receipt := ⟨t, Verdict.deny, trivialProof⟩

def escalateOnlyLedger (t : TransformationId) : Ledger :=
  ⟨fun tid v => tid = t ∧ v = Verdict.escalate⟩

def escalateReceipt (t : TransformationId) : Receipt := ⟨t, Verdict.escalate, trivialProof⟩

/-- Constructor `deny` — it is what `hAdmit` guards against: all of T4's hypotheses except
`hAdmit` hold, the receipt says `deny`, and the resolved transformation is not admissible. -/
theorem deny_makes_hAdmit_load_bearing (t : TransformationId) :
    LedgerAdmitsOnlyAdmissible (OrderBounded 0) (denyOnlyLedger t) (fun _ => order5Txn) ∧
      ValidReceipt (denyOnlyLedger t) (denyReceipt t) ∧
      ProofSound (denyReceipt t).proof ∧
      (denyReceipt t).v ≠ Verdict.admit ∧
      ¬ OrderBounded 0 order5Txn := by
  refine ⟨denyOnlyLedger_admitsOnlyAdmissible _ t _, ⟨rfl, rfl⟩, trivialProof_sound,
    fun h => Verdict.noConfusion h, ?_⟩
  intro h
  have h' : order5Txn.order ≤ 0 := h
  exact absurd h' (by decide)

/-- Constructor `escalate` — same role, same shape. -/
theorem escalate_makes_hAdmit_load_bearing (t : TransformationId) :
    LedgerAdmitsOnlyAdmissible (OrderBounded 0) (escalateOnlyLedger t) (fun _ => order5Txn) ∧
      ValidReceipt (escalateOnlyLedger t) (escalateReceipt t) ∧
      ProofSound (escalateReceipt t).proof ∧
      (escalateReceipt t).v ≠ Verdict.admit ∧
      ¬ OrderBounded 0 order5Txn := by
  refine ⟨?_, ⟨rfl, rfl⟩, trivialProof_sound, fun h => Verdict.noConfusion h, ?_⟩
  · intro tid h
    exact Verdict.noConfusion h.2
  · intro h
    have h' : order5Txn.order ≤ 0 := h
    exact absurd h' (by decide)

/-- `Verdict` with `deny` and `escalate` removed (one constructor). -/
inductive VerdictAdmitOnly where
  | admit

structure LedgerAO where
  contains : TransformationId → VerdictAdmitOnly → Prop

structure InclusionProofAO where
  verifies : LedgerAO → TransformationId → VerdictAdmitOnly → Prop

structure ReceiptAO where
  t     : TransformationId
  v     : VerdictAdmitOnly
  proof : InclusionProofAO

def ValidReceiptAO (L : LedgerAO) (r : ReceiptAO) : Prop := r.proof.verifies L r.t r.v

def LedgerAdmitsOnlyAdmissibleAO
    (A : Transformation → Prop) (L : LedgerAO) (ofId : TransformationId → Transformation) : Prop :=
  ∀ tid, L.contains tid VerdictAdmitOnly.admit → A (ofId tid)

def ProofSoundAO (p : InclusionProofAO) : Prop :=
  ∀ (L : LedgerAO) (t : TransformationId) (v : VerdictAdmitOnly), p.verifies L t v → L.contains t v

/-- Constructors `deny`/`escalate` — survival: on the one-constructor variant T4 holds **without**
`hAdmit` (every verdict is `admit`), i.e. the hypothesis the two constructors make load-bearing
is derivable once they are gone. -/
theorem nonAdmitVerdicts_dropped_T4_survives_without_hAdmit
    {A : Transformation → Prop} {L : LedgerAO} {ofId : TransformationId → Transformation}
    (hLedger : LedgerAdmitsOnlyAdmissibleAO A L ofId)
    (r : ReceiptAO) (hv : ValidReceiptAO L r) (hSound : ProofSoundAO r.proof) :
    A (ofId r.t) ∧ L.contains r.t r.v := by
  have hContains : L.contains r.t r.v := hSound L r.t r.v hv
  refine ⟨?_, hContains⟩
  cases hr : r.v
  have hContains' : L.contains r.t VerdictAdmitOnly.admit := hr ▸ hContains
  exact hLedger r.t hContains'

-- =============================================================================================
-- § 8. `Receipt.t` — NEEDED (unstatable). Without the subject id, validity can only be "the proof verifies
-- *some* transformation at `r.v`", and T4's conclusion `A (ofId r.t) ∧ L.contains r.t r.v` has no
-- subject. The forced consequence is machine-checked as an impossibility *on the variant*: no
-- function can recover, from a `t`-less receipt alone, an id the ledger actually contains for it —
-- because one and the same receipt is valid against two ledgers whose sole entries differ.
-- =============================================================================================

/-- `Receipt` with `t` removed. -/
structure ReceiptNoT where
  v     : Verdict
  proof : InclusionProof

/-- The only validity the variant can express. -/
def ValidReceiptNoT (L : Ledger) (r : ReceiptNoT) : Prop := ∃ t, r.proof.verifies L t r.v

/-- Two distinct ids (fresh bytes; `Minimality`/`Injection` corpora untouched). -/
def idP : TransformationId := ⟨ByteArray.mk #[41]⟩

def idQ : TransformationId := ⟨ByteArray.mk #[42]⟩

theorem idP_ne_idQ : idP ≠ idQ := by decide

/-- A ledger holding exactly `(t, admit)`. -/
def admitOnlyLedgerAt (t : TransformationId) : Ledger :=
  ⟨fun tid v => tid = t ∧ v = Verdict.admit⟩

/-- The one `t`-less receipt of the argument: says `admit`, verified by `trivialProof`. -/
def admitReceiptNoT : ReceiptNoT := ⟨Verdict.admit, trivialProof⟩

/-- Field `t` — no subject-recovery function exists on the variant. -/
theorem receiptT_dropped_no_subject_recovery :
    ¬ ∃ pick : ReceiptNoT → TransformationId,
        ∀ (L : Ledger) (r : ReceiptNoT), ValidReceiptNoT L r → L.contains (pick r) r.v := by
  intro ⟨pick, hpick⟩
  by_cases h : pick admitReceiptNoT = idP
  · have hc := hpick (admitOnlyLedgerAt idQ) admitReceiptNoT ⟨idQ, rfl, rfl⟩
    exact idP_ne_idQ (h.symm.trans hc.1)
  · have hc := hpick (admitOnlyLedgerAt idP) admitReceiptNoT ⟨idP, rfl, rfl⟩
    exact h hc.1

-- =============================================================================================
-- § 9. `Receipt.proof` — NEEDED (unstatable). Without the proof, `ValidReceipt`/`ProofSound` have no site
-- and the only validity left is "the ledger contains `(r.t, r.v)`" — the verifier must read the
-- ledger. T4 then holds **without any soundness hypothesis** (below): the receipt has become a
-- claim, and the offline-verification boundary that `ProofSound` marks (46 §1: Merkle proof
-- abstract, algorithm outside Lean) has vanished with the field. "unstatable" of T4's content, not
-- "breaks".
-- =============================================================================================

/-- `Receipt` with `proof` removed. -/
structure ReceiptNoProof where
  t : TransformationId
  v : Verdict

/-- The only validity the variant can express: ledger lookup. -/
def ValidReceiptNoProof (L : Ledger) (r : ReceiptNoProof) : Prop := L.contains r.t r.v

/-- Field `proof` — T4-shape on the variant, provable with `ProofSound` gone (validity *is*
containment). -/
theorem receiptProof_dropped_T4_trivial
    {A : Transformation → Prop} {L : Ledger} {ofId : TransformationId → Transformation}
    (hLedger : LedgerAdmitsOnlyAdmissible A L ofId)
    (r : ReceiptNoProof) (hv : ValidReceiptNoProof L r) (hAdmit : r.v = Verdict.admit) :
    A (ofId r.t) ∧ L.contains r.t r.v := by
  refine ⟨?_, hv⟩
  have hv' : L.contains r.t Verdict.admit := hAdmit ▸ hv
  exact hLedger r.t hv'

-- =============================================================================================
-- § 10. `Canonicalizer.embed` — REDUNDANT-CANDIDATE (theory, on the model path), DESIGN-REQUIRED. `Idempotent` reads `embed`
-- (`canon (embed (canon x)) = canon x`) because in general `Repr ≠ Canonical` and re-applying
-- `canon` needs a way back — on the frozen `CanonModel` `embed = id` and `canon_idem` is stated
-- with no `embed` at all. The `embed`-less variant is only statable when `Repr = Canonical`
-- (46 §2.4's general form is unstatable on it), and there the model satisfies it verbatim.
-- =============================================================================================

/-- `Canonicalizer` with `embed` removed. -/
structure CanonicalizerNoEmbed (Repr Canonical : Type) where
  canon : Repr → Canonical

/-- Idempotence without a way back: statable only on the diagonal `Repr = Canonical`. -/
def IdempotentNoEmbed {R : Type} (C : CanonicalizerNoEmbed R R) : Prop :=
  ∀ x : R, C.canon (C.canon x) = C.canon x

def modelNoEmbed : CanonicalizerNoEmbed CanonModel.Flat CanonModel.Flat := ⟨CanonModel.canon⟩

/-- Field `embed` — survival on the executable model (= frozen `canon_idem`). -/
theorem embedField_dropped_idempotence_survives_on_model : IdempotentNoEmbed modelNoEmbed :=
  CanonModel.canon_idem

-- =============================================================================================
-- § 11. `CanonModel.Field.val` — REDUNDANT-CANDIDATE (theory), DESIGN-REQUIRED. `ins`/`canon`/`Sorted` and every Canon
-- theorem read `key` only; the variant without `val` proves the same theorems. What `val` alone
-- makes observable is 42 §2.1's duplicate-key *resolution* rule ("for equal keys the one that appeared first is kept"):
-- two records equal on `key` are told apart by `val` and by nothing else, so the frozen `#guard`
-- that pins the rule is unstatable on the variant (both orders collapse to the same list).
-- =============================================================================================

/-- `CanonModel.Field` with `val` removed. -/
structure FieldNoVal where
  key : Nat
  deriving DecidableEq

def dropVal (f : CanonModel.Field) : FieldNoVal := ⟨f.key⟩

/-- Field `val` — the duplicate-resolution rule is observable through `val` (first conjunct: the
two insertion orders yield different canonical forms) and invisible without it (second conjunct:
their `val`-less projections coincide). -/
theorem valField_observable_only_through_dedup :
    CanonModel.canon [⟨2, 7⟩, ⟨2, 9⟩] ≠ CanonModel.canon [⟨2, 9⟩, ⟨2, 7⟩] ∧
      (CanonModel.canon [⟨2, 7⟩, ⟨2, 9⟩]).map dropVal =
        (CanonModel.canon [⟨2, 9⟩, ⟨2, 7⟩]).map dropVal := by
  decide

end GxSpec.MinimalityF0
