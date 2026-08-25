import GxSpec.Core
import GxSpec.Receipt

/-!
# GxSpec.Injection — Rule 2 (clock/rng single injection point) minimality: the clock counterexample (sem: SEM-lean-044)

Identity (sem: SEM-lean-045): `req/159_V03_REQDEF_2026-08-15.md` §C-1, consuming `DR-46-4`
(`req/spec/40-architecture/46-verification-plan.md` §8, filed per `req/160_V03A_DR_REPORT_2026-08-15.md`
§3-2 with `req/38` §98 ruling 3 adopting the five legitimacy conditions of `req/160` §3-1). The membrane (sem: SEM-lean-046)
(`req/112` line 140 / `req/122` line 50, verbatim) is Rule 1 (three powers at a single point — audited by (sem: SEM-lean-047)
`GxSpec.Minimality`) + **Rule 2** (clock/rng external inputs enter through a single injection point, (sem: SEM-lean-048)
i.e. the *semantics* itself holds **zero** read points for time or randomness).
`GxSpec.Minimality`'s own scope declaration (its L40-46) states why Rule 2 could not be audited there: (sem: SEM-lean-049)
`GxSpec.Core`'s frozen F0 surface carries no time/randomness field at all (46 §1's crypto/time
non-goal), so a Rule 2 counterexample needs structure with no frozen counterpart. This file is that (sem: SEM-lean-050)
counterexample — built on an **extension** surface whose legitimacy is exactly what `req/160` §3-1's
five conditions condition, and each condition is discharged here by construction:

* **C1 (conservative extension)**: the frozen modules are used by `import` only. No frozen file is
  edited, no frozen definition is shadowed, and every frozen theorem stands unchanged (`lake build`
  of the whole root is the machine check; axiom set stays `{propext, Quot.sound, GxSpec.composeId}`,
  proof-placeholder grep 0 — the word itself is avoided here so AC-061's substring gate stays
  clean).
* **C2 (counterexample-only vocabulary)**: nothing below asserts anything *positive about gx* with
  the extension vocabulary as its subject. `TimedTransformation` appears in exactly two roles: the
  broken variant (the counterexample), and the projection that *discards* it (the recovery, whose
  substantive content is the frozen `T4_receipt_soundness` itself).
* **C3 (explicit subject)**: the theorem subject is "the variant that permits injection" (quoted in SEM-lean-051) — a design in which the
  semantics is permitted **one** map that reads the timestamp. No impossibility claim is made about
  the frozen membrane itself; the stance is `Minimality.lean`'s own (counterexample construction,
  completeness not claimed, P-9 no overclaim).
* **C4 (minimal injection)**: the extension adds exactly the structure Rule 2 legislates about and (sem: SEM-lean-052)
  nothing else — one `Nat` timestamp component (`TimedTransformation := Transformation × Nat`) and
  one predicate conjunct that reads it (`· = 0` — the "still fresh" clock gate). `Nat` abstracts the
  clock value; no cbor/hash/wall-clock machinery is reconstructed (46 §1's non-goal stands).
* **C5 (projection pair = positive control)**: paired with the breakage theorem, this file proves
  that under the injection-*forgetting* projection (`forget := Prod.fst`) the frozen guarantee is
  recovered — `projection_recovers_T4` is a one-line application of the **frozen**
  `T4_receipt_soundness`, and `timed_factors_iff` is `Iff.rfl`, i.e. the statement "a timed
  hypothesis that does not read the clock *is* the frozen hypothesis" holds **definitionally**. The
  frozen surface is thereby one endpoint of the construction (not an analogy), and the sole
  difference between the sound model and the broken model is the one injected read point — which is
  Rule 2's minimality claim (the injection point is load-bearing), machine-checked in contrapositive. (sem: SEM-lean-053)

## What is proved here, precisely (P-9 — no overclaim)

`clockInjection_counterexample` is a **counterexample construction** (`Minimality.lean`'s word),
not an impossibility theorem over all conceivable designs: a concrete instance in which every
structural hypothesis a T4-shaped soundness statement needs (`ValidReceipt`, `r.v = admit`,
`ProofSound`) is satisfied, the *content* half of admissibility is satisfied at **both**
observations, and yet the timed soundness hypothesis is demonstrably false on a pair
`(τ, 0)`/`(τ, 1)` that differ **only** in timestamp — same `Transformation`, same minted id
(minting is time-blind: the id is a function of the projection alone, `rfl` below). The failure is
attributable to the clock conjunct and to nothing else, because the same ledger and the same
resolution function *with the clock read forgotten* satisfy the frozen hypothesis outright
(`fixture_projection_recovery`).

**Denominator, stated once here rather than left to be noticed by absence** (`DR-46-4`'s own
wording): this file's scope is the **clock instance only**. rng is the second instance of the same
schema (a `Transformation × Nat` variant whose read map consumes a random draw rather than a
timestamp); it is **not delivered here**, and per `req/38` §98 ruling 3, "Rule 2's counterexample is complete" (quoted in SEM-lean-054) is *not*
claimed by this file — that claim would require the rng instance delivered or its non-delivery
adjudicated. What this file claims is exactly: the clock half of Rule 2's injection-point minimality (sem: SEM-lean-055)
has a machine-checked counterexample-and-recovery pair.
(v0.4-d cross-reference, additive — the denominator above is unchanged as the v0.3-c record: the
rng second instance is now delivered as its own extension module, `GxSpec/InjectionRng.lean`
(`req/174`, consuming `req/38` §106 ruling 1's candidate-box item), under the same DR-46-4 schema and the (sem: SEM-lean-056)
same `req/160` §3-1 five conditions. This file's own scope is still the clock instance only, and
whether "Rule 2's counterexample is complete" (quoted in SEM-lean-057) may now be claimed is `req/38` §NN acceptance (Fable)'s call, not this
file's.)
-/

namespace GxSpec.Injection

open GxSpec

-- =============================================================================================
-- § 0. The injection variant and its forgetting projection (C4: this is the *entire* extension —
-- one `Nat` component, one projection back).
-- =============================================================================================

/-- Rule 2's injection variant: a transformation observed together with a clock reading. `abbrev` (sem: SEM-lean-058)
(not a new structure) keeps the extension minimal and the projections definitionally transparent —
the type-level fixation C5 asks for: the variant *is* the frozen `Transformation` plus one `Nat`,
and nothing else. -/
abbrev TimedTransformation : Type := Transformation × Nat

/-- The injection-forgetting projection (C5's arrow). Everything the frozen surface can say about
a timed pair, it says through this map. -/
def forget (tt : TimedTransformation) : Transformation := tt.1

/-- The timed restatement of the frozen `LedgerAdmitsOnlyAdmissible` (T4's own soundness
hypothesis, `Receipt.lean`) with the *variant* as subject (C3): same frozen `Ledger`, same frozen
`Verdict`, same shape — only the resolution target carries the injected timestamp. -/
def TimedLedgerAdmitsOnlyAdmissible
    (At : TimedTransformation → Prop) (L : Ledger)
    (ofIdT : TransformationId → TimedTransformation) : Prop :=
  ∀ tid, L.contains tid Verdict.admit → At (ofIdT tid)

-- =============================================================================================
-- § 1. Fixtures: one concrete transformation, observed at two times.
-- =============================================================================================

/-- One concrete piece of content (`Minimality.lean`'s `x1`/`x2` idiom, fresh byte to avoid
narrative confusion with that file's corpus). -/
def x0 : ObjectSnapshot := ⟨ByteArray.mk #[21]⟩

/-- The one transformation of this counterexample. Its id is minted from the *frozen* surface —
the timestamp is not part of the id (minting is time-blind), which is exactly why one admitted id
can later resolve to a differently-timed observation of the same content. -/
def baseTxn : Transformation := { id := ⟨ByteArray.mk #[22]⟩, order := 0, src := x0, dst := x0 }

/-- Frozen-surface admissibility for this instance: `baseTxn` really is the admissible content.
(A predicate on `Transformation` — the 0-injection side's vocabulary.) -/
def baseA : Transformation → Prop := fun t => t = baseTxn

/-- The injected read map, isolated as an executable gate so `#guard` can witness it below: the
semantics deems an observation valid only at time 0 ("still fresh"). This conjunct — and nothing
else in this file — reads the clock (C4). -/
def clockGateB (time : Nat) : Bool := time == 0

-- The `#guard`-grade witnesses (compile-time, kernel-evaluated on every `lake build`): the clock
-- gate really separates the two timestamps the counterexample uses. A gate that could not take
-- both values would make the "breakage" below vacuous.
#guard clockGateB 0
#guard !(clockGateB 1)

/-- The timed admissibility of the variant: the frozen content check (factoring through `forget`)
**plus one clock read**. This is "the semantics permits one map that reads the timestamp" (quoted in SEM-lean-059) in predicate form — the
content half is untouched frozen-shaped vocabulary; the clock conjunct is the single injected read
point. -/
def timedA : TimedTransformation → Prop := fun tt => baseA (forget tt) ∧ tt.2 = 0

/-- The same transformation observed at admission time... -/
def timedFresh : TimedTransformation := (baseTxn, 0)

/-- ...and observed one tick later. Same `Transformation` (hence same minted id), different
timestamp — the pair Rule 2's minimality is about. (sem: SEM-lean-060) -/
def timedStale : TimedTransformation := (baseTxn, 1)

/-- A ledger that legitimately contains `admit` for exactly `baseTxn`'s id — written at admission
time, when `timedA timedFresh` really held. Nothing about this ledger is broken. -/
def timedLedger : Ledger := ⟨fun tid v => tid = baseTxn.id ∧ v = Verdict.admit⟩

/-- Same trivial-inclusion-proof idiom as `Minimality.lean`'s: `verifies` is literally `contains`,
so `ProofSound` holds outright and the failure below cannot be hiding in the proof machinery
(46 §1's crypto non-goal). -/
def trivialProof : InclusionProof := ⟨fun L t v => L.contains t v⟩

theorem trivialProof_sound : ProofSound trivialProof := fun _ _ _ h => h

/-- The receipt for the admitted id, verified by `trivialProof`. -/
def admittedReceipt : Receipt := ⟨baseTxn.id, Verdict.admit, trivialProof⟩

/-- Resolution in the timed variant: the admitted id resolves to the *later* observation. On the
frozen projection this function is perfectly faithful — `forget (resolveTimed tid) = baseTxn`, the
very content that was minted and admitted (no §89-A-3-style unrelated-constant weakness: the
content answer is *right*). The only degree of freedom it exercises is the timestamp — which is
precisely the injected degree of freedom, and a resolution function cannot avoid exercising it,
because the minted id (time-blind, `sameId` below) cannot pin what was never in it. -/
def resolveTimed (_ : TransformationId) : TimedTransformation := timedStale

/-- Minting is time-blind: the two observations carry the same minted id (definitionally). This is
why the ledger's one admitted id genuinely reaches `timedStale` — the id cannot tell the two
apart. -/
theorem sameId : (forget timedFresh).id = (forget timedStale).id := rfl

-- =============================================================================================
-- § 2. The breakage theorem (DR-46-4's breakage theorem): one permitted clock-reading map, and a (sem: SEM-lean-061)
-- same-content different-timestamp pair breaks the T4-shaped guarantee.
-- =============================================================================================

/-- Clock-injection counterexample (Rule 2, clock instance): in a variant whose semantics is allowed (sem: SEM-lean-062)
**one** map that reads the timestamp (`timedA`'s clock conjunct), the pair
`timedFresh`/`timedStale` — same `Transformation` (first conjunct), same minted id (`sameId`) —
splits admissibility: fresh is admissible, stale is not, **while the content half still holds at
stale** (the failure is the clock conjunct's and nothing else's, C4). Every hypothesis a T4-shaped
soundness statement needs other than the (timed) ledger-soundness hypothesis holds — `ValidReceipt`,
`r.v = admit`, `ProofSound` — and that one timed hypothesis is demonstrably false: the ledger
legitimately admitted the id when it was fresh, and resolution meets the same id one tick later.
Nothing here is broken *except* the injected read point — which is the point. -/
theorem clockInjection_counterexample :
    forget timedFresh = forget timedStale ∧
      timedA timedFresh ∧
      (baseA (forget timedStale) ∧ ¬ timedA timedStale) ∧
      ValidReceipt timedLedger admittedReceipt ∧
      admittedReceipt.v = Verdict.admit ∧
      ProofSound admittedReceipt.proof ∧
      ¬ TimedLedgerAdmitsOnlyAdmissible timedA timedLedger resolveTimed := by
  refine ⟨rfl, ⟨rfl, rfl⟩, ⟨rfl, ?_⟩, ⟨rfl, rfl⟩, rfl, trivialProof_sound, ?_⟩
  · intro h
    exact Nat.one_ne_zero h.2
  · intro hT
    have h := hT baseTxn.id ⟨rfl, rfl⟩
    exact Nat.one_ne_zero h.2

-- =============================================================================================
-- § 3. The projection-recovery theorems (DR-46-4's projection-recovery theorem, C5's positive control): forget (sem: SEM-lean-063)
-- the injection and the frozen guarantee returns — via the frozen theorem itself.
-- =============================================================================================

/-- C5's type-level fixation: a timed hypothesis whose predicate **factors through `forget`**
(i.e. the semantics does *not* read the timestamp — the 0-injection side) *is* the frozen
hypothesis, definitionally (`Iff.rfl`). The difference between the sound model and the broken
model is thereby pinned at the type level to exactly one place: whether the predicate consults
`tt.2`. -/
theorem timed_factors_iff (A : Transformation → Prop) (L : Ledger)
    (ofIdT : TransformationId → TimedTransformation) :
    TimedLedgerAdmitsOnlyAdmissible (fun tt => A (forget tt)) L ofIdT ↔
      LedgerAdmitsOnlyAdmissible A L (fun tid => forget (ofIdT tid)) :=
  Iff.rfl

/-- Projection recovery (C5): under the injection-forgetting projection, the frozen
`T4_receipt_soundness` — invoked here *as is*, the frozen surface being an endpoint of this
construction and not an analogy — delivers the full T4 conclusion for any timed setup whose
admissibility does not read the clock. Together with `clockInjection_counterexample` this is the
two-directional pair: read point allowed ⇒ T4-shaped guarantee breaks (§2); read point forgotten ⇒
frozen guarantee holds (here). The sole difference is the injection point. -/
theorem projection_recovers_T4
    {A : Transformation → Prop} {L : Ledger} {ofIdT : TransformationId → TimedTransformation}
    (hLedger : TimedLedgerAdmitsOnlyAdmissible (fun tt => A (forget tt)) L ofIdT)
    (r : Receipt) (hv : ValidReceipt L r) (hAdmit : r.v = Verdict.admit)
    (hSound : ProofSound r.proof) :
    A (forget (ofIdT r.t)) ∧ L.contains r.t r.v :=
  T4_receipt_soundness ((timed_factors_iff A L ofIdT).mp hLedger) r hv hAdmit hSound

/-- The positive control on the *counterexample's own fixtures*: the very ledger, resolution
function and receipt of §2 — unchanged — satisfy the recovered frozen guarantee once the clock
read is forgotten (`baseA` alone, no `tt.2` conjunct). So the §2 breakage cannot be blamed on the
ledger, the resolution, or the receipt machinery: with the injection point removed and everything
else fixed, soundness returns. The one moving part is the injected read. -/
theorem fixture_projection_recovery :
    baseA (forget (resolveTimed admittedReceipt.t)) ∧
      timedLedger.contains admittedReceipt.t admittedReceipt.v :=
  projection_recovers_T4 (fun _ _ => rfl) admittedReceipt ⟨rfl, rfl⟩ rfl trivialProof_sound

end GxSpec.Injection
