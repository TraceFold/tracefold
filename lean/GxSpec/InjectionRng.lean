import GxSpec.Core
import GxSpec.Receipt
import GxSpec.Injection

/-!
# GxSpec.InjectionRng — Rule 2 (clock/rng single injection point) minimality: the rng counterexample (sem: SEM-lean-064)

Identity (sem: SEM-lean-065): `req/174_V04D_RNG_REPORT_2026-08-15.md` (v0.4-d), consuming `req/38` §106 ruling 1's candidate-box
item — the **second instance of `DR-46-4`'s schema**
(`req/spec/40-architecture/46-verification-plan.md` §8; the schema, its five legitimacy
conditions, and the clock instance are `req/160` §3-1 / `req/38` §98 ruling 3 / `GxSpec/Injection.lean`). (sem: SEM-lean-066)
Rule 2 (`req/112` line 140 / `req/122` line 50, verbatim) legislates that clock **and rng** external inputs (sem: SEM-lean-067)
enter through a single injection point — the semantics itself holds **zero** read points for time
or randomness. `Injection.lean` delivered the clock half and declared rng as its denominator
("not delivered here", sem: SEM-lean-068). This file is that second half: the same counterexample-and-recovery
schema, instantiated for a read map that **consumes a random draw** rather than a timestamp.

`GxSpec.Injection` is imported here for exactly one purpose: the cross-instance contrast
witnesses of § 4 below (machine-checking that this file's gate is *not* the clock gate
re-spelled). The variant, fixtures and theorems of this file inherit nothing from it.

The five conditions of `req/160` §3-1, discharged here by construction (same schema as
`Injection.lean`, restated for this instance):

* **C1 (conservative extension)**: the frozen modules are used by `import` only. No frozen file
  is edited, no frozen definition is shadowed, and every frozen theorem stands unchanged
  (`lake build` of the whole root is the machine check; axiom set stays
  `{propext, Quot.sound, GxSpec.composeId}`, proof-placeholder grep 0 — the word itself is
  avoided here so AC-061's substring gate stays clean).
* **C2 (counterexample-only vocabulary)**: nothing below asserts anything *positive about gx*
  with the extension vocabulary as its subject. `RngTransformation` appears in exactly two roles:
  the broken variant (the counterexample), and the projection that *discards* it (the recovery,
  whose substantive content is the frozen `T4_receipt_soundness` itself).
* **C3 (explicit subject)**: the theorem subject is "the variant that permits injection" (quoted in SEM-lean-069) — a design in which the
  semantics is permitted **one** map that reads a consumed random draw. No impossibility claim is
  made about the frozen membrane itself; the stance is `Minimality.lean`'s own (counterexample
  construction, completeness not claimed, P-9 no overclaim).
* **C4 (minimal injection)**: the extension adds exactly the structure Rule 2 legislates about and (sem: SEM-lean-070)
  nothing else — one `Nat` draw component (`RngTransformation := Transformation × Nat`) and one
  predicate conjunct that reads it (`· % 2 = 0` — the "draw came up even" admission gate). `Nat`
  abstracts the drawn value; no PRNG state, no distribution, no entropy machinery is
  reconstructed (46 §1's non-goal stands, at the same abstraction level the clock instance used
  for time).
* **C5 (projection pair = positive control)**: paired with the breakage theorem, this file
  proves that under the injection-*forgetting* projection (`forget := Prod.fst`) the frozen
  guarantee is recovered — `projection_recovers_T4` is a one-line application of the **frozen**
  `T4_receipt_soundness`, and `drawn_factors_iff` is `Iff.rfl`, i.e. the statement "a drawn
  hypothesis that does not consume the draw *is* the frozen hypothesis" holds **definitionally**.
  The frozen surface is thereby one endpoint of the construction (not an analogy), and the sole
  difference between the sound model and the broken model is the one injected read point.

## What is proved here, precisely (P-9 — no overclaim)

`rngInjection_counterexample` is a **counterexample construction**, not an impossibility theorem
over all conceivable designs: a concrete instance in which every structural hypothesis a
T4-shaped soundness statement needs (`ValidReceipt`, `r.v = admit`, `ProofSound`) is satisfied,
the *content* half of admissibility is satisfied at **both** observations, and yet the drawn
soundness hypothesis is demonstrably false on a pair `(τ, 2)`/`(τ, 3)` that differ **only** in
the consumed draw — same `Transformation`, same minted id (minting is draw-blind: the id is a
function of the projection alone, `rfl` below). The failure is attributable to the draw conjunct
and to nothing else, because the same ledger and the same resolution function *with the draw
read forgotten* satisfy the frozen hypothesis outright (`fixture_projection_recovery`).

## Why this is the rng instance and not the clock instance re-spelled (§ 4's witnesses)

At the `Nat` abstraction both instances share the carrier `Transformation × Nat` — that is
`DR-46-4`'s own wording ("the second instance of the same schema", quoted in SEM-lean-071) and is not hidden. What fixes this file's
injected read as *draw consumption* rather than *clock reading* is machine-checked, not asserted:

1. **The gates have different acceptance structure.** The clock gate's acceptance set is the
   *singleton* {0} — validity means "at the distinguished admission instant"
   (`clockGate_accepts_only_zero`). The rng gate's acceptance and rejection sets each contain at
   least two distinct draws (`rngGate_accepts_two_distinct_draws` /
   `rngGate_rejects_two_distinct_draws`) — validity is an **event over draws** (parity), not a
   distinguished point. An admission keyed to an event that recurs across the draw space is the
   shape of a sampling/probabilistic gate; an admission keyed to one distinguished value is the
   shape of a freshness deadline.
2. **The counterexample's own admitted draw refutes the clock reading.** The valid observation
   below is `(baseTxn, 2)` — admitted because 2 is *even*, not because it is the origin. The
   clock gate rejects that very value (`gates_disagree_on_admitted_draw`, and `#guard
   !(Injection.clockGateB 2)`): on this file's fixtures the two gates give opposite verdicts, so
   no reading of this file's conjunct as "freshness at time 0" survives.
3. **Honest residual, declared rather than smuggled**: no theorem here models randomness *as a
   distribution* (independence, uniformity, PRNG state are all 46 §1 non-goals — exactly as the
   clock instance's `Nat` carries no monotonicity). What Rule 2 legislates — and what is (sem: SEM-lean-072)
   machine-checked here — is the injection-*point* structure: one read map consuming an
   externally drawn value, whose single permitted occurrence is load-bearing. The probabilistic
   *interpretation* of the drawn value lives in this doc and in `req/174`'s adversarial section,
   not in the kernel.

**Denominator and claim discipline** (`DR-46-4`'s own wording, `req/38` §106 ruling 1): with this (sem: SEM-lean-073)
file delivered, **both** instances of the DR-46-4 schema now exist — clock
(`GxSpec/Injection.lean`, v0.3-c) and rng (this file). This file still does **not** claim
"Rule 2's counterexample is complete" (quoted in SEM-lean-074): that declaration is `req/38` §NN acceptance (Fable)'s alone. What this file
claims is exactly: the rng half of Rule 2's injection-point minimality has a machine-checked (sem: SEM-lean-075)
counterexample-and-recovery pair.
-/

namespace GxSpec.InjectionRng

open GxSpec

-- =============================================================================================
-- § 0. The injection variant and its forgetting projection (C4: this is the *entire* extension —
-- one `Nat` component, one projection back).
-- =============================================================================================

/-- Rule 2's injection variant, rng instance: a transformation observed together with the random (sem: SEM-lean-076)
draw its admission consumed. `abbrev` (not a new structure) keeps the extension minimal and the
projections definitionally transparent — the type-level fixation C5 asks for: the variant *is*
the frozen `Transformation` plus one `Nat`, and nothing else. -/
abbrev RngTransformation : Type := Transformation × Nat

/-- The injection-forgetting projection (C5's arrow). Everything the frozen surface can say about
a drawn pair, it says through this map. -/
def forget (rt : RngTransformation) : Transformation := rt.1

/-- The drawn restatement of the frozen `LedgerAdmitsOnlyAdmissible` (T4's own soundness
hypothesis, `Receipt.lean`) with the *variant* as subject (C3): same frozen `Ledger`, same frozen
`Verdict`, same shape — only the resolution target carries the injected draw. -/
def RngLedgerAdmitsOnlyAdmissible
    (Ar : RngTransformation → Prop) (L : Ledger)
    (ofIdR : TransformationId → RngTransformation) : Prop :=
  ∀ tid, L.contains tid Verdict.admit → Ar (ofIdR tid)

-- =============================================================================================
-- § 1. Fixtures: one concrete transformation, observed under two draws.
-- =============================================================================================

/-- One concrete piece of content (`Injection.lean`'s `x0` idiom, fresh byte to avoid narrative
confusion with that file's corpus). -/
def y0 : ObjectSnapshot := ⟨ByteArray.mk #[23]⟩

/-- The one transformation of this counterexample. Its id is minted from the *frozen* surface —
the draw is not part of the id (minting is draw-blind), which is exactly why one admitted id can
later resolve to an observation of the same content under a different draw. -/
def baseTxn : Transformation := { id := ⟨ByteArray.mk #[24]⟩, order := 0, src := y0, dst := y0 }

/-- Frozen-surface admissibility for this instance: `baseTxn` really is the admissible content.
(A predicate on `Transformation` — the 0-injection side's vocabulary.) -/
def baseA : Transformation → Prop := fun t => t = baseTxn

/-- The injected read map, isolated as an executable gate so `#guard` can witness it below: the
semantics deems an observation valid only when the consumed draw is even (a sampling-style
admission event over the draw space — parity — not a distinguished instant). This conjunct — and
nothing else in this file — consumes the draw (C4). -/
def rngGateB (draw : Nat) : Bool := draw % 2 == 0

-- The `#guard`-grade witnesses (compile-time, kernel-evaluated on every `lake build`): the rng
-- gate accepts and rejects on *both sides of the pair it separates*, at more than one draw each —
-- and the clock gate rejects the very draw this file's counterexample admits. A gate that could
-- not take both values would make the "breakage" below vacuous; a gate the clock gate agreed
-- with on these fixtures would leave the "second instance" claim unwitnessed.
#guard rngGateB 0
#guard rngGateB 2
#guard !(rngGateB 1)
#guard !(rngGateB 3)
#guard !(Injection.clockGateB 2)

/-- The drawn admissibility of the variant: the frozen content check (factoring through `forget`)
**plus one draw read**. This is "the semantics permits one map that consumes a random draw" (quoted in SEM-lean-077) in predicate
form — the content half is untouched frozen-shaped vocabulary; the parity conjunct is the single
injected read point. -/
def drawnA : RngTransformation → Prop := fun rt => baseA (forget rt) ∧ rt.2 % 2 = 0

/-- The same transformation observed when the draw came up even (admitted — and note the draw is
`2`, not `0`: validity here is the parity *event*, not the clock's distinguished origin)... -/
def drawnEven : RngTransformation := (baseTxn, 2)

/-- ...and observed when the draw came up odd. Same `Transformation` (hence same minted id),
different consumed draw — the pair Rule 2's minimality is about, rng half. (sem: SEM-lean-078) -/
def drawnOdd : RngTransformation := (baseTxn, 3)

/-- A ledger that legitimately contains `admit` for exactly `baseTxn`'s id — written at admission,
when the consumed draw really was even (`drawnA drawnEven` held). Nothing about this ledger is
broken. -/
def drawnLedger : Ledger := ⟨fun tid v => tid = baseTxn.id ∧ v = Verdict.admit⟩

/-- Same trivial-inclusion-proof idiom as `Injection.lean`'s (itself `Minimality.lean`'s):
`verifies` is literally `contains`, so `ProofSound` holds outright and the failure below cannot
be hiding in the proof machinery (46 §1's crypto non-goal). -/
def trivialProof : InclusionProof := ⟨fun L t v => L.contains t v⟩

theorem trivialProof_sound : ProofSound trivialProof := fun _ _ _ h => h

/-- The receipt for the admitted id, verified by `trivialProof`. -/
def admittedReceipt : Receipt := ⟨baseTxn.id, Verdict.admit, trivialProof⟩

/-- Resolution in the drawn variant: the admitted id resolves to the *odd-draw* observation. On
the frozen projection this function is perfectly faithful — `forget (resolveDrawn tid) = baseTxn`,
the very content that was minted and admitted (the content answer is *right*). The only degree of
freedom it exercises is the draw — which is precisely the injected degree of freedom, and a
resolution function cannot avoid exercising it, because a verifier's own observation consumes its
own draw, and the minted id (draw-blind, `sameId` below) cannot pin what was never in it. -/
def resolveDrawn (_ : TransformationId) : RngTransformation := drawnOdd

/-- Minting is draw-blind: the two observations carry the same minted id (definitionally). This
is why the ledger's one admitted id genuinely reaches `drawnOdd` — the id cannot tell the two
apart. -/
theorem sameId : (forget drawnEven).id = (forget drawnOdd).id := rfl

-- =============================================================================================
-- § 2. The breakage theorem (DR-46-4's breakage theorem, rng instance): one permitted draw-consuming (sem: SEM-lean-079)
-- map, and a same-content different-draw pair breaks the T4-shaped guarantee.
-- =============================================================================================

/-- Rng-injection counterexample (Rule 2, rng instance): in a variant whose semantics is allowed (sem: SEM-lean-080)
**one** map that consumes the random draw (`drawnA`'s parity conjunct), the pair
`drawnEven`/`drawnOdd` — same `Transformation` (first conjunct), same minted id (`sameId`) —
splits admissibility: the even draw is admissible, the odd draw is not, **while the content half
still holds at the odd draw** (the failure is the draw conjunct's and nothing else's, C4). Every
hypothesis a T4-shaped soundness statement needs other than the (drawn) ledger-soundness
hypothesis holds — `ValidReceipt`, `r.v = admit`, `ProofSound` — and that one drawn hypothesis is
demonstrably false: the ledger legitimately admitted the id when the draw came up even, and
resolution meets the same id under an odd draw. Nothing here is broken *except* the injected read
point — which is the point. -/
theorem rngInjection_counterexample :
    forget drawnEven = forget drawnOdd ∧
      drawnA drawnEven ∧
      (baseA (forget drawnOdd) ∧ ¬ drawnA drawnOdd) ∧
      ValidReceipt drawnLedger admittedReceipt ∧
      admittedReceipt.v = Verdict.admit ∧
      ProofSound admittedReceipt.proof ∧
      ¬ RngLedgerAdmitsOnlyAdmissible drawnA drawnLedger resolveDrawn := by
  refine ⟨rfl, ⟨rfl, rfl⟩, ⟨rfl, ?_⟩, ⟨rfl, rfl⟩, rfl, trivialProof_sound, ?_⟩
  · intro h
    exact Nat.one_ne_zero h.2
  · intro hR
    have h := hR baseTxn.id ⟨rfl, rfl⟩
    exact Nat.one_ne_zero h.2

-- =============================================================================================
-- § 3. The projection-recovery theorems (DR-46-4's projection-recovery theorem, C5's positive control): forget (sem: SEM-lean-081)
-- the injection and the frozen guarantee returns — via the frozen theorem itself.
-- =============================================================================================

/-- C5's type-level fixation: a drawn hypothesis whose predicate **factors through `forget`**
(i.e. the semantics does *not* consume the draw — the 0-injection side) *is* the frozen
hypothesis, definitionally (`Iff.rfl`). The difference between the sound model and the broken
model is thereby pinned at the type level to exactly one place: whether the predicate consults
`rt.2`. -/
theorem drawn_factors_iff (A : Transformation → Prop) (L : Ledger)
    (ofIdR : TransformationId → RngTransformation) :
    RngLedgerAdmitsOnlyAdmissible (fun rt => A (forget rt)) L ofIdR ↔
      LedgerAdmitsOnlyAdmissible A L (fun tid => forget (ofIdR tid)) :=
  Iff.rfl

/-- Projection recovery (C5): under the injection-forgetting projection, the frozen
`T4_receipt_soundness` — invoked here *as is*, the frozen surface being an endpoint of this
construction and not an analogy — delivers the full T4 conclusion for any drawn setup whose
admissibility does not consume the draw. Together with `rngInjection_counterexample` this is the
two-directional pair: read point allowed ⇒ T4-shaped guarantee breaks (§2); read point forgotten
⇒ frozen guarantee holds (here). The sole difference is the injection point. -/
theorem projection_recovers_T4
    {A : Transformation → Prop} {L : Ledger} {ofIdR : TransformationId → RngTransformation}
    (hLedger : RngLedgerAdmitsOnlyAdmissible (fun rt => A (forget rt)) L ofIdR)
    (r : Receipt) (hv : ValidReceipt L r) (hAdmit : r.v = Verdict.admit)
    (hSound : ProofSound r.proof) :
    A (forget (ofIdR r.t)) ∧ L.contains r.t r.v :=
  T4_receipt_soundness ((drawn_factors_iff A L ofIdR).mp hLedger) r hv hAdmit hSound

/-- The positive control on the *counterexample's own fixtures*: the very ledger, resolution
function and receipt of §2 — unchanged — satisfy the recovered frozen guarantee once the draw
read is forgotten (`baseA` alone, no `rt.2` conjunct). So the §2 breakage cannot be blamed on the
ledger, the resolution, or the receipt machinery: with the injection point removed and everything
else fixed, soundness returns. The one moving part is the injected read. -/
theorem fixture_projection_recovery :
    baseA (forget (resolveDrawn admittedReceipt.t)) ∧
      drawnLedger.contains admittedReceipt.t admittedReceipt.v :=
  projection_recovers_T4 (fun _ _ => rfl) admittedReceipt ⟨rfl, rfl⟩ rfl trivialProof_sound

-- =============================================================================================
-- § 4. Cross-instance contrast witnesses: this gate is not the clock gate re-spelled. These are
-- counterexample-apparatus meta-theorems (C2-safe: no positive claim about gx has the extension
-- vocabulary as subject); `GxSpec.Injection` is imported for exactly these.
-- =============================================================================================

/-- The clock gate's acceptance set is the singleton {0}: clock validity is "at the distinguished
admission instant". (Proved about `Injection.clockGateB` itself, so the contrast below is with
the delivered clock instance, not with a strawman.) -/
theorem clockGate_accepts_only_zero (n : Nat) (h : Injection.clockGateB n = true) : n = 0 := by
  simpa [Injection.clockGateB] using h

/-- The rng gate accepts at (at least) two distinct draws: rng validity is an *event* recurring
across the draw space, not a distinguished point. Together with `clockGate_accepts_only_zero`
this machine-fixes that the two delivered gates have different acceptance structure. -/
theorem rngGate_accepts_two_distinct_draws :
    rngGateB 0 = true ∧ rngGateB 2 = true ∧ (0 : Nat) ≠ 2 := by decide

/-- ...and rejects at (at least) two distinct draws: the invalid side is likewise an event, so
the gate partitions the draw space into two nontrivial events (parity) rather than isolating one
distinguished value. -/
theorem rngGate_rejects_two_distinct_draws :
    rngGateB 1 = false ∧ rngGateB 3 = false ∧ (1 : Nat) ≠ 3 := by decide

/-- On this file's own admitted fixture the two gates give opposite verdicts: the draw the rng
counterexample admits (`drawnEven.2 = 2`) is *rejected* by the clock gate. No reading of this
file's conjunct as "freshness at time 0" survives this witness. -/
theorem gates_disagree_on_admitted_draw :
    rngGateB drawnEven.2 = true ∧ Injection.clockGateB drawnEven.2 = false := by decide

end GxSpec.InjectionRng
