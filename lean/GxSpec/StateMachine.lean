import GxSpec.Core
import GxSpec.Receipt

/-!
# GxSpec.StateMachine — the lifecycle state machine and its seven safety invariants
(sem: SEM-lean-205)

Identity (sem: SEM-lean-206): `req/spec/40-architecture/43-state-machine.md` §1 (the eleven
canonical states), §3 (the transition table T-1..T-13) and **§9 (INV-S1..INV-S7 / INV-L1..INV-L4)**,
whose own preamble names them *"the direct obligation of the property-tests and Lean lemmas"*.
Filed as **Q10** out of `req/38` §SS889 (the Lean-coverage census, R-925-2) and narrowed by
§SS894 to the Lean half only: the property-test half of that obligation was found already
discharged (`crates/gx-engine/tests/ac_042.rs`, 888 lines, model-based proptest, no stubbed or
ignored cases), and reporting it as absent was a compression error on the ledger's side. The Lean
half was the part that was actually zero: before this file, `INV-S` and `INV-L` matched nothing
anywhere under `lean/`.

`req/38` §SS894 also records the asymmetry that made this the oldest hole in the tree. 43 §9.1
split INV-L1..L4 into the three ranks (proved / mechanised / production) on 2026-08-15 and has
disclosed the Lean rank as *unmodelled* ever since. INV-S1..S7 were placed **outside** that note's
scope ("the Safety rows are not the subject of this note"), and a row excluded from the ledger of
what is tracked stops being tracked. It went fifteen days unmeasured, not because anyone claimed
it was done, but because nothing was counting it.

## What kind of object this file adds (sem: SEM-lean-207)

Every module before this one models a *fragment* — a category skeleton, a canonicaliser, a ledger
predicate, an effect algebra. This one models a **labelled transition system**: a `World`, a
partial `step` function driven by an `Event`, and an inductively defined `Reachable`. That is a
different shape of object, and it is the shape 43 §9 needs, because a safety invariant is a
statement about *every reachable state*, which is not expressible until reachability is.

The proof discipline is the usual one: a single inductive invariant `wfB` is shown to be
established by every initial world and preserved by every successful step, and INV-S1..S7 are then
corollaries of it. Two of the seven (INV-S2, INV-S6) are *step-local* rather than reachability
statements — 43 phrases them as constraints on an edge ("the only outgoing transition is…",
"does not transition without…"), and stating them on the edge is both closer to the text and
strictly stronger than stating them on the reachable set.

## Discipline (the five conditions of `req/160` §3-1 / `req/38` §98 ruling 3) (sem: SEM-lean-208)

* **C1 (conservative extension)**: the frozen modules are used by `import` only. `Core.lean` and
  `Receipt.lean` are read, never edited; no frozen definition is shadowed and no frozen theorem is
  restated in a weaker form. `Verdict` is *reused* from `Receipt.lean` rather than redeclared —
  and, because that inductive carries no `DecidableEq`, this file compares it with a local total
  `verdictEq` instead of adding an instance to a frozen type. Nothing below introduces an axiom, so
  the library's axiom set stays `{propext, Quot.sound, GxSpec.composeId}`. The machine check is
  `lake build` of the whole root plus the proof-placeholder grep, and the word that grep looks for
  is avoided in this file's prose exactly as `Injection.lean` avoids it.
  (sem: SEM-lean-209)
* **C2 (counterexample-only vocabulary)**: `Phase`, `Event`, `Row`, `World`, `Entry`, `Rcpt`,
  `Config` exist to state 43 §3 and nothing else. **No theorem below asserts that `crates/gx-engine`
  satisfies or violates anything.** The subject is always the model. The bridge from the model to
  the implementation is §7's refinement statement, and that bridge is a *hypothesis*, never a
  conclusion. (sem: SEM-lean-210)
* **C3 (explicit subject)**: §5's subject is "a world reachable under `step` from an initial
  world". §7's subject is "any transition system together with an abstraction map into this model"
  — quantified over *all* such systems, which is what makes `refinement_transports_safety`
  transport anything at all, and equally what makes it say nothing about any particular
  implementation until its hypothesis is discharged for that implementation.
  (sem: SEM-lean-211)
* **C4 (minimal addition)**: the row carries fourteen fields and every one of them is named by 43 —
  eleven states (§1), seven abort reasons (§1, as corrected for the seventh), the `enforced` and
  `fail_posture_engaged` flags (§4, T-4e), the two fingerprints (T-2, T-10a), `superseded_by`
  (T-12). No clock, no journal, no substrate, no cryptography: 46 §1's non-goals stand unchanged.
  (sem: SEM-lean-212)
* **C5 (positive control paired with every breakage)**: §7's `forged_transition_counterexample` is
  paired with `identity_simulates` on the *same* model — the only moving part is whether the
  abstraction's steps are steps. §6's `liveness_needs_a_clock_counterexample` is paired with
  `wfB_of_reachable`: the same world satisfies every safety invariant and makes no progress, which
  is the whole point. (sem: SEM-lean-213)

## What is proved here, precisely (P-9 — no overclaim) (sem: SEM-lean-214)

Proved, of the model and only of the model:

* (a) INV-S1 — every world whose phase is `committed` or `superseded` is canonicalised and carries
  either an admission, or a denial with `enforced = false`. There is no third door (§5).
* (b) INV-S2 — from `committed`, *every* successful step leaves the ledger and the receipt
  pointwise unchanged, and lands in `superseded` with `superseded_by` set. Stated on the edge, so
  it quantifies over all nineteen events rather than over reachable states (§5).
* (c) INV-S3 — the ledger's keys stay unique, hence at most one entry per `TransformationId`; the
  bound is proved as a `List.filter` length, not as "the list is short" (§5).
* (d) INV-S4 — an entry under this row's key is present exactly when the phase is `committed` or
  `superseded`, so an aborted or (enforced-mode) denied row has none; and any entry of the row that
  carries `deny` carries `enforced = false`, which is the record-only carve-out and the only one
  (§5).
* (e) INV-S5 — the receipt's `enforced` and `fail_posture_engaged` are the row's, and two worlds
  differing only in `enforced` produce receipts that differ. Distinguishability is proved as
  non-equality of the receipts, not asserted as a property of the field (§5).
* (f) INV-S6 — from `escalated`, any successful step landing in `admitted` or `denied` sets
  `humanRuling`. Again on the edge, so no unlisted event can sneak past it (§5).
* (g) INV-S7 — `applied` implies the commit-time fingerprint was observed *and was equal to* the
  planned one; equivalently, a `casCheck` that observes a different fingerprint aborts and leaves
  `applied` false (§5).
* (h) Refinement transports all seven: if an implementation's abstraction maps initial states to
  initial worlds and every implementation step to a step of this model or to a stutter, then every
  implementation-reachable state abstracts to a reachable world, and therefore satisfies (a)-(g)
  (§7). A non-vacuity witness (`identity_simulates`) and a breakage control
  (`forged_transition_counterexample`) sit beside it.

**Not proved, and not claimable from anything below.**

1. **That `crates/gx-engine` refines this model.** §7's hypothesis is not discharged for the Rust
   engine anywhere, and this file does not discharge it. `docs/LIMITS.md` item 8's standing
   sentence — the 1,500-vector differential comparison *finds differences and does not establish
   equivalence*, and no refinement theorem connects the two — is **not** retracted by this file.
   What changes is narrower and worth stating exactly: before, the refinement theorem did not
   exist, so there was no statement of *what would have to be true*. Now the conditional exists and
   its hypothesis is named (`Simulates`), which is the part a differential test could be aimed at.
   A conditional whose hypothesis is unproved proves nothing about the implementation, and the
   temptation to read it otherwise is the exact failure `req/38` §SS854 records: *the effort spent
   proving the conclusion and the effort spent checking the hypothesis are not balanced by anyone*.
2. **INV-L1..INV-L4.** They are not formalised here and cannot be, in this model: every one of them
   says "within finite time", and 46 §1.1 item 6 declares time and randomness outside the model.
   §6 proves *why* rather than leaving it as prose — the transition system admits a stutter step
   (T-10b changes nothing) and carries no clock, so a run that makes no progress satisfies every
   invariant proved in §5. That is a mechanical statement of where the apparatus stops. It is
   recorded as **UNTESTABLE, not as failing** (`req/38` §SS870): a liveness property that this
   model cannot express is not a liveness property this model refutes.
3. **Anything about the concurrency, crash-recovery, durability or multi-writer fragments.** The
   model has one row and one ledger value. 46 §1.1 items 1-5 stand; `DR-46-6` stage A3 is still
   undelivered.

## Denominators (what is bounded, declared rather than silently held) (sem: SEM-lean-215)

1. **One row.** The world holds a single transformation's lifecycle over a ledger that may already
   contain foreign entries. So INV-S3's uniqueness is proved *as preservation of key-uniqueness
   under append*, which is the general statement, but the two-writer race that 43 §8 discusses is
   outside the model (46 §1.1 item 3).
2. **T-1 is the initial state, not an event.** 43's diagram draws `[*] --> Draft: submit(intent)`;
   here `IsInit` *is* that arrow's target. Nineteen events cover T-2..T-13 including T-4a/b/c/d/e,
   T-5/T-5b, T-8/T-8r, T-10a/b/c, T-11, T-12.
3. **T-10a's passing branch has no id in 43 and is given one here.** 43 gives the *failing* CAS an
   id (T-10a) and leaves the passing branch unlabelled, exactly as §1's note about "an edge with no
   id" describes for `PostconditionMismatch`. `Event.casCheck` carries the observed fingerprint and
   both branches fall out of the comparison, which is what makes INV-S7 a theorem about
   fingerprints and not about a flag. The count of transition ids is *not* changed by this
   (`state_machine_coverage`'s denominator is untouched — this file writes no Rust).
4. **`AbortReason.postconditionMismatch` is in the type and reachable by no event here.** 43 §1's
   addendum says the seventh variant is reachable only when `Transformation.target` is `Some`, and
   `target` is not in this model's row. It is carried so the type is 43's type; it is not exercised.
5. **T-4d and T-4e are one event.** 43 gives them separate ids because their guards differ, but the
   *trigger* is identical ("verifier or evidence collector unreachable") and the branch is decided
   by `Config.failPosture`. Modelling the caller as choosing which one fires would have let a
   theorem quantify over a choice the world does not have.
6. **`escrowInverse` (T-10b) is a genuine no-op.** 43 calls it "an internal step; the state does not
   change". It is kept because §6 needs a stutter that is in the model rather than invented for the
   occasion.

## Crosswalk — which implementation property each new theorem is about (sem: SEM-lean-216)

The form is `req/979`'s pillar (b): a row may not claim a correspondence it cannot name. The Rust
column lists coordinates that carry the invariant's id in a named assertion (verified by grep at
the time of writing; the coordinates are the *implementation's* claim, and this file does not
recheck them). `INFERRED` throughout, in that column's sense: the Lean statement and the Rust
assertion are judged by a reader to be about the same sentence of 43 §9. Nothing here is `DIRECT`,
because the Rust tests do not import this model and this model does not read them.

| 43 §9 | Lean theorem (this file) | Rust assertions carrying the id |
|---|---|---|
| INV-S1 | `inv_S1_committed_passed_the_gate` | `gx-engine/tests/ac_041.rs`, `ac_042.rs` |
| INV-S2 | `inv_S2_committed_is_append_only` | `gx-engine/tests/ac_044.rs`, `ac_040.rs` |
| INV-S3 | `inv_S3_at_most_one_ledger_entry` | `gx-engine/tests/ac_035.rs`, `ac_043.rs`, `commit_protocol.rs`, `concurrent_commit.rs` |
| INV-S4 | `inv_S4_only_committed_is_witnessed`, `inv_S4_a_denial_in_the_ledger_is_unenforced` | `gx-engine/tests/ac_034.rs`, `ac_038.rs`, `ac_041.rs`, `ac_072.rs`, `ac_073.rs` |
| INV-S5 | `inv_S5_receipt_carries_enforcement`, `inv_S5_distinguishable` | `gx-engine/tests/ac_033.rs`, `ac_039.rs`, `journal_identity.rs` |
| INV-S6 | `inv_S6_escalation_needs_a_person` | `gx-engine/tests/ac_037.rs`, `ac_071.rs` |
| INV-S7 | `inv_S7_apply_implies_cas_matched`, `inv_S7_mismatch_never_applies` | `gx-engine/tests/ac_031.rs`, `ac_034.rs`, `concurrent_commit.rs`, `binary_e2e.rs` |
| INV-L1..L4 | *none* — `liveness_needs_a_clock_counterexample` states why | 43 §9.1/§9.1.1/§9.1.2 own the three-rank note |

**What a row of this table does not mean.** It does not mean that the Rust assertion and the
Lean statement are the same proposition, nor that either one implies the other. Until §7's
hypothesis is discharged for the engine, the two columns are two independent readings of one
sentence of 43, and the honest description of their relationship is *"they agree, and nothing
mechanical is holding them together"*.
-/

namespace GxSpec
namespace StateMachine

/-! ## §0 — 43 §1's vocabulary (sem: SEM-lean-217) -/

/-- 43 §1's eleven canonical state names, in the order the table lists them.
    (sem: SEM-lean-218) -/
inductive Phase where
  | draft | candidate | verifying | admitted | denied | escalated
  | canonicalized | committing | committed | aborted | superseded
  deriving DecidableEq, Repr

/-- 43 §1's `AbortReason`. Seven variants: the six of the original enum plus
    `postconditionMismatch` (M5-11, the §1 addendum). See denominator 4 — the seventh is in the
    type and reachable by no event here, because `Transformation.target` is not modelled.
    (sem: SEM-lean-219) -/
inductive AbortReason where
  | preconditionChanged | applyFailed | verifierUnavailable | expired
  | ownerCancelled | internalError | postconditionMismatch
  deriving DecidableEq, Repr

/-- 43 §4's two independent configuration axes. `EnforcementMode` decides whether a `Denied` may
    proceed via T-8r; `FailPosture` decides whether an unreachable verifier takes T-4d or T-4e.
    (sem: SEM-lean-220) -/
inductive EnforcementMode where
  | enforce | recordOnly
  deriving DecidableEq, Repr

inductive FailPosture where
  | failClosed | failOpen
  deriving DecidableEq, Repr

structure Config where
  mode        : EnforcementMode
  failPosture : FailPosture
  deriving DecidableEq, Repr

/-- Total equality on the frozen `Verdict` (from `Receipt.lean`). Written here rather than added
    as a `DecidableEq` instance on a frozen inductive — C1. (sem: SEM-lean-221) -/
def verdictEq : Verdict → Verdict → Bool
  | .admit, .admit => true
  | .deny, .deny => true
  | .escalate, .escalate => true
  | _, _ => false

theorem verdictEq_iff {a b : Verdict} : verdictEq a b = true ↔ a = b := by
  cases a <;> cases b <;> simp [verdictEq]

@[simp] theorem verdictEq_refl (a : Verdict) : verdictEq a a = true := by
  cases a <;> rfl

/-- Decidable equality on the frozen id/snapshot types is derived in `Core.lean`; these give it a
    `Bool` face so the well-formedness predicate below is computable. (sem: SEM-lean-222) -/
def tidEq (a b : TransformationId) : Bool := decide (a = b)

def snapEq (a b : ObjectSnapshot) : Bool := decide (a = b)

theorem tidEq_iff {a b : TransformationId} : tidEq a b = true ↔ a = b := by
  simp [tidEq]

theorem snapEq_iff {a b : ObjectSnapshot} : snapEq a b = true ↔ a = b := by
  simp [snapEq]

/-! ## §1 — the world (sem: SEM-lean-223) -/

/-- A ledger entry: 42 §3.11's public witness log reduced to what 43 §9 talks about — a key, the
    verdict that was reached, the enforcement flag, and the canonical CID.
    (sem: SEM-lean-224) -/
structure Entry where
  tid      : TransformationId
  verdict  : Verdict
  enforced : Bool
  canonCid : ObjectSnapshot

/-- The commit receipt, reduced likewise. `Receipt.lean`'s `Receipt` models the same artefact from
    the inclusion-proof side; this one models the fields INV-S5 is about. (sem: SEM-lean-225) -/
structure Rcpt where
  tid                : TransformationId
  verdict            : Verdict
  enforced           : Bool
  failPostureEngaged : Bool
  canonCid           : ObjectSnapshot

/-- One transformation's lifecycle row. Every field is named by 43 — see C4.
    (sem: SEM-lean-226) -/
structure Row where
  tid                : TransformationId
  fp0                : ObjectSnapshot
  fp1                : Option ObjectSnapshot
  phase              : Phase
  admittedOnce       : Bool
  deniedOnce         : Bool
  canonicalized      : Bool
  enforced           : Bool
  failPostureEngaged : Bool
  humanRuling        : Bool
  casOk              : Bool
  applied            : Bool
  supersededBy       : Option TransformationId
  abortReason        : Option AbortReason

structure World where
  row     : Row
  ledger  : List Entry
  receipt : Option Rcpt

/-- Terminal-with-a-witness: the two phases in which the ledger and the receipt exist. `superseded`
    is included because T-12 does not remove either — that is the whole content of INV-S2.
    (sem: SEM-lean-227) -/
def committedish : Phase → Bool
  | .committed  => true
  | .superseded => true
  | _           => false

/-- 43's entry state. Named because the invariant has to say that a row which has not been planned
    yet cannot be carrying a commit-time fingerprint comparison — see `wfB`'s last conjunct.
    (sem: SEM-lean-272) -/
def isDraft : Phase → Bool
  | .draft => true
  | _      => false

/-! ## §2 — the ledger's key discipline (ASM-43-1) (sem: SEM-lean-228) -/

/-- Is there already an entry under this key? (sem: SEM-lean-229) -/
def hasTid (l : List Entry) (t : TransformationId) : Bool :=
  l.any (fun e => tidEq e.tid t)

/-- Key-uniqueness of the whole log. (sem: SEM-lean-230) -/
def keyUnique : List Entry → Bool
  | []      => true
  | e :: t  => !hasTid t e.tid && keyUnique t

/-- ASM-43-1's keyed append: appending an entry whose key is already present is a no-op that
    returns the existing log. This is the model of the idempotency column of T-11.
    (sem: SEM-lean-231) -/
def appendKeyed (l : List Entry) (e : Entry) : List Entry :=
  if hasTid l e.tid then l else e :: l

theorem hasTid_cons {l : List Entry} {e : Entry} {t : TransformationId} :
    hasTid (e :: l) t = (tidEq e.tid t || hasTid l t) := by
  simp [hasTid]

theorem not_mem_of_hasTid_false {l : List Entry} {t : TransformationId}
    (h : hasTid l t = false) : ∀ e ∈ l, e.tid ≠ t := by
  intro e he
  simp only [hasTid, List.any_eq_false] at h
  have := h e he
  simpa [tidEq] using this

theorem keyUnique_appendKeyed {l : List Entry} {e : Entry}
    (h : keyUnique l = true) : keyUnique (appendKeyed l e) = true := by
  unfold appendKeyed
  by_cases hc : hasTid l e.tid = true
  · simp [hc, h]
  · simp only [Bool.not_eq_true] at hc
    simp [hc, keyUnique, h]

theorem hasTid_appendKeyed_self {l : List Entry} {e : Entry} :
    hasTid (appendKeyed l e) e.tid = true := by
  unfold appendKeyed
  by_cases hc : hasTid l e.tid = true
  · simp [hc]
  · simp only [Bool.not_eq_true] at hc
    simp [hc, hasTid_cons, tidEq]

/-- The bound INV-S3 actually asks for, phrased as a count and not as "the list is short".
    (sem: SEM-lean-232) -/
theorem filter_nil_of_hasTid_false {t : TransformationId} :
    ∀ l : List Entry, hasTid l t = false → l.filter (fun e => tidEq e.tid t) = [] := by
  intro l
  induction l with
  | nil => intro _; rfl
  | cons e rest ih =>
      intro h
      rw [hasTid_cons] at h
      have h12 := Bool.or_eq_false_iff.mp h
      rw [List.filter_cons, h12.1]
      exact ih h12.2

/-- INV-S3's bound, phrased as a count over the log and not as "the log is short".
    Proved without `Classical.choice`: `List.filter_eq_nil_iff` and the `simp` route both pull it
    in, and the library's declared axiom set is `{propext, Quot.sound, GxSpec.composeId}` — a
    convenience that widens the trusted base is not a convenience. (sem: SEM-lean-232) -/
theorem filter_length_le_one (t : TransformationId) :
    ∀ l : List Entry, keyUnique l = true →
      (l.filter (fun e => tidEq e.tid t)).length ≤ 1 := by
  intro l
  induction l with
  | nil => intro _; exact Nat.zero_le 1
  | cons e rest ih =>
      intro h
      rw [keyUnique] at h
      have h12 := Bool.and_eq_true_iff.mp h
      have hfresh : hasTid rest e.tid = false := Bool.not_eq_true' .. |>.mp h12.1
      have hrest : keyUnique rest = true := h12.2
      cases he : tidEq e.tid t with
      | false =>
          rw [List.filter_cons, he]
          exact ih hrest
      | true =>
          have het : e.tid = t := tidEq_iff.mp he
          have hf : hasTid rest t = false := by rw [← het]; exact hfresh
          rw [List.filter_cons, he, filter_nil_of_hasTid_false rest hf]
          exact Nat.le_refl 1

/-! ## §3 — 43 §3's transition table as an event-driven partial function (sem: SEM-lean-233) -/

/-- The nineteen events of 43 §3, T-2 through T-13. See denominators 2, 3 and 5 for the three
    places where this list is not literally the table's id column. (sem: SEM-lean-234) -/
inductive Event where
  /-- T-2 `plan()`: records `Fingerprint₀`. -/
  | plan (fp0 : ObjectSnapshot)
  /-- T-3 `verify_start`. -/
  | verifyStart
  /-- T-4a `Gate::verify → Admit`. -/
  | verdictAdmit
  /-- T-4b `Gate::verify → Deny`. -/
  | verdictDeny
  /-- T-4c `Gate::verify → Escalate`. -/
  | verdictEscalate
  /-- T-4d / T-4e: the verifier is unreachable; `Config.failPosture` decides which one fires. -/
  | verifierUnreachable
  /-- T-5 human ruling = Admit. -/
  | humanAdmit
  /-- T-5b human ruling = Deny. -/
  | humanDeny
  /-- T-6 TTL. -/
  | expire
  /-- T-7 `ownerCancel`. -/
  | ownerCancel
  /-- T-8 `canonicalize`. -/
  | canonicalize
  /-- T-8r `canonicalize` under `EnforcementMode::RecordOnly`. -/
  | canonicalizeRecordOnly
  /-- T-9 `commit_start`. -/
  | commitStart
  /-- T-10a and its unlabelled passing branch: the commit-time fingerprint is observed. -/
  | casCheck (fp1 : ObjectSnapshot)
  /-- T-10b `adapter.invert` escrow — an internal step; the state does not change. -/
  | escrowInverse
  /-- T-10c `adapter.apply` failed. -/
  | applyFail
  /-- T-11 `adapter.apply` succeeded; ledger append and receipt issue. -/
  | applyCommit (canonCid : ObjectSnapshot)
  /-- T-12 the supersedes edge, drawn by another transformation reaching `Committed`. -/
  | supersede (by_ : TransformationId)
  /-- T-13 an unclassifiable internal failure. -/
  | internalError

/-- The verdict a landed row carries: 43 records the gate's answer, and under T-8r that answer is
    `Deny` even though the row commits. (sem: SEM-lean-235) -/
def landedVerdict (r : Row) : Verdict :=
  if r.admittedOnce then Verdict.admit else Verdict.deny

/-- 43 §3's transition table. `none` means the guard did not hold, which is the model of "no such
    edge from this state". (sem: SEM-lean-236) -/
def step (c : Config) (w : World) : Event → Option World
  | .plan fp0 =>
      if w.row.phase = .draft then
        some { w with row := { w.row with phase := .candidate, fp0 := fp0 } }
      else none
  | .verifyStart =>
      if w.row.phase = .candidate then
        some { w with row := { w.row with phase := .verifying } }
      else none
  | .verdictAdmit =>
      if w.row.phase = .verifying then
        some { w with row := { w.row with phase := .admitted, admittedOnce := true } }
      else none
  | .verdictDeny =>
      if w.row.phase = .verifying then
        some { w with row := { w.row with phase := .denied, deniedOnce := true } }
      else none
  | .verdictEscalate =>
      if w.row.phase = .verifying then
        some { w with row := { w.row with phase := .escalated } }
      else none
  | .verifierUnreachable =>
      if w.row.phase = .verifying then
        match c.failPosture with
        | .failClosed =>
            some { w with row := { w.row with phase := .aborted,
                                              abortReason := some .verifierUnavailable } }
        | .failOpen =>
            some { w with row := { w.row with phase := .admitted, admittedOnce := true,
                                              enforced := false, failPostureEngaged := true } }
      else none
  | .humanAdmit =>
      if w.row.phase = .escalated then
        some { w with row := { w.row with phase := .admitted, admittedOnce := true,
                                          humanRuling := true } }
      else none
  | .humanDeny =>
      if w.row.phase = .escalated then
        some { w with row := { w.row with phase := .denied, deniedOnce := true,
                                          humanRuling := true } }
      else none
  | .expire =>
      match w.row.phase with
      | .candidate | .verifying | .escalated =>
          some { w with row := { w.row with phase := .aborted,
                                            abortReason := some .expired } }
      | _ => none
  | .ownerCancel =>
      match w.row.phase with
      | .draft | .candidate | .verifying | .admitted | .canonicalized | .escalated =>
          some { w with row := { w.row with phase := .aborted,
                                            abortReason := some .ownerCancelled } }
      | _ => none
  | .canonicalize =>
      if w.row.phase = .admitted then
        some { w with row := { w.row with phase := .canonicalized, canonicalized := true } }
      else none
  | .canonicalizeRecordOnly =>
      if w.row.phase = .denied ∧ c.mode = .recordOnly then
        some { w with row := { w.row with phase := .canonicalized, canonicalized := true,
                                          enforced := false } }
      else none
  | .commitStart =>
      if w.row.phase = .canonicalized then
        some { w with row := { w.row with phase := .committing } }
      else none
  | .casCheck fp1 =>
      if w.row.phase = .committing then
        if fp1 = w.row.fp0 then
          some { w with row := { w.row with fp1 := some fp1, casOk := true } }
        else
          some { w with row := { w.row with fp1 := some fp1, casOk := false, phase := .aborted,
                                            abortReason := some .preconditionChanged } }
      else none
  | .escrowInverse =>
      if w.row.phase = .committing ∧ w.row.casOk = true then some w else none
  | .applyFail =>
      if w.row.phase = .committing ∧ w.row.casOk = true then
        some { w with row := { w.row with phase := .aborted, applied := true,
                                          abortReason := some .applyFailed } }
      else none
  | .applyCommit canonCid =>
      if w.row.phase = .committing ∧ w.row.casOk = true ∧ w.row.canonicalized = true then
        some { row     := { w.row with phase := .committed, applied := true }
             , ledger  := appendKeyed w.ledger
                            { tid := w.row.tid, verdict := landedVerdict w.row
                            , enforced := w.row.enforced, canonCid := canonCid }
             , receipt := some { tid := w.row.tid, verdict := landedVerdict w.row
                               , enforced := w.row.enforced
                               , failPostureEngaged := w.row.failPostureEngaged
                               , canonCid := canonCid } }
      else none
  | .supersede u =>
      if w.row.phase = .committed ∧ w.row.supersededBy = none then
        some { w with row := { w.row with phase := .superseded, supersededBy := some u } }
      else none
  | .internalError =>
      match w.row.phase with
      | .committed | .aborted | .superseded => none
      | _ => some { w with row := { w.row with phase := .aborted,
                                               abortReason := some .internalError } }

/-! ## §4 — initial worlds, reachability, and the inductive invariant (sem: SEM-lean-237) -/

/-- 43's `[*] --> Draft: submit(intent)`. The initial world *is* that arrow's target — denominator
    2. The ledger may already hold foreign entries, but not one under this row's key, and its keys
    are unique. (sem: SEM-lean-238) -/
structure IsInit (w : World) : Prop where
  phase        : w.row.phase = .draft
  admitted     : w.row.admittedOnce = false
  denied       : w.row.deniedOnce = false
  canon        : w.row.canonicalized = false
  enforced     : w.row.enforced = true
  fpEngaged    : w.row.failPostureEngaged = false
  human        : w.row.humanRuling = false
  fp1          : w.row.fp1 = none
  cas          : w.row.casOk = false
  applied      : w.row.applied = false
  supersededBy : w.row.supersededBy = none
  receipt      : w.receipt = none
  keys         : keyUnique w.ledger = true
  fresh        : hasTid w.ledger w.row.tid = false

inductive Reachable (c : Config) : World → Prop where
  | init  {w : World} : IsInit w → Reachable c w
  | step  {w w' : World} (e : Event) : Reachable c w → step c w e = some w' → Reachable c w'

/-- The receipt agrees with the row about everything INV-S5 is about. (sem: SEM-lean-239) -/
def rcptFaithful (r : Rcpt) (row : Row) : Bool :=
  tidEq r.tid row.tid && (r.enforced == row.enforced) &&
  (r.failPostureEngaged == row.failPostureEngaged) && verdictEq r.verdict (landedVerdict row)

/-- Likewise for an entry filed under this row's key. (sem: SEM-lean-240) -/
def entryFaithful (e : Entry) (row : Row) : Bool :=
  (e.enforced == row.enforced) && verdictEq e.verdict (landedVerdict row)

/-- A row that landed carries a verdict the gate actually reached: an admission, or a denial that
    was demoted to `enforced = false` by T-8r or T-4e. (sem: SEM-lean-241) -/
def gateAnswered (row : Row) : Bool :=
  row.admittedOnce || (row.deniedOnce && !row.enforced)

/-- **What each state already knows about its own past.** 43 §1 does not spell this out because it
    is a consequence of §3's table rather than a row of it, but every safety invariant below leans
    on it: a row in `Admitted` has been admitted, a row in `Denied` has been denied, nothing before
    `Canonicalized` has been canonicalised, and nothing before `Committing` has compared a
    fingerprint. Written as one predicate rather than seven side conditions, because seven side
    conditions are seven places to forget one. (sem: SEM-lean-241a) -/
def phaseOk (r : Row) : Bool :=
  match r.phase with
  | .draft         => !r.admittedOnce && !r.deniedOnce && !r.canonicalized && !r.casOk && !r.applied
  | .candidate     => !r.canonicalized && !r.casOk && !r.applied
  | .verifying     => !r.canonicalized && !r.casOk && !r.applied
  | .escalated     => !r.canonicalized && !r.casOk && !r.applied
  | .admitted      => r.admittedOnce && !r.canonicalized && !r.casOk && !r.applied
  | .denied        => r.deniedOnce && !r.canonicalized && !r.casOk && !r.applied
  | .canonicalized => r.canonicalized && !r.casOk && !r.applied
  | .committing    => r.canonicalized && !r.applied
  | .committed     => r.canonicalized && r.casOk && r.applied
  | .superseded    => r.canonicalized && r.casOk && r.applied
  | .aborted       => true

/-- Every entry filed under this row's key tells the truth about the row. (sem: SEM-lean-272a) -/
def ownEntriesFaithful (w : World) : Bool :=
  w.ledger.all (fun e => !tidEq e.tid w.row.tid || entryFaithful e w.row)

/-- The receipt, if there is one, tells the truth about the row. (sem: SEM-lean-272b) -/
def rcptOk (w : World) : Bool :=
  match w.receipt with | none => true | some r => rcptFaithful r w.row

/-- The commit-time fingerprint was observed and matched the planned one. (sem: SEM-lean-272c) -/
def fp1Matches (r : Row) : Bool :=
  match r.fp1 with | some f => snapEq f r.fp0 | none => false

/-- Neither the ledger's nor the receipt's faithfulness can see any field a post-commit edge is
    allowed to move, so both are literally unchanged across T-12. Stated once, so the T-12 case of
    the preservation proof does not have to re-derive them. (sem: SEM-lean-272d) -/
@[simp] theorem ownEntriesFaithful_congr {w : World} {r : Row}
    (htid : r.tid = w.row.tid) (henf : r.enforced = w.row.enforced)
    (hadm : r.admittedOnce = w.row.admittedOnce) :
    ownEntriesFaithful { w with row := r } = ownEntriesFaithful w := by
  simp [ownEntriesFaithful, entryFaithful, landedVerdict, htid, henf, hadm]

@[simp] theorem rcptOk_congr {w : World} {r : Row}
    (htid : r.tid = w.row.tid) (henf : r.enforced = w.row.enforced)
    (hfpe : r.failPostureEngaged = w.row.failPostureEngaged)
    (hadm : r.admittedOnce = w.row.admittedOnce) :
    rcptOk { w with row := r } = rcptOk w := by
  simp [rcptOk, rcptFaithful, landedVerdict, htid, henf, hfpe, hadm]

/-- The single inductive invariant. INV-S1, S3, S4, S5 and S7 are read out of it in §5; INV-S2 and
    INV-S6 are proved on the edge instead and do not need it.

    The faithfulness of the ledger entry and the receipt sits *inside* the `committedish` guard on
    purpose: before a row lands there is no entry under its key and no receipt (conjuncts two and
    three say so), and a clause that has to be re-established at every one of the eighteen
    pre-commit edges is a clause that will eventually be re-established wrongly.
    (sem: SEM-lean-242) -/
def wfB (w : World) : Bool :=
  keyUnique w.ledger &&
  (hasTid w.ledger w.row.tid == committedish w.row.phase) &&
  (w.receipt.isSome == committedish w.row.phase) &&
  (!committedish w.row.phase ||
     (w.row.canonicalized && gateAnswered w.row && ownEntriesFaithful w && rcptOk w)) &&
  (!w.row.canonicalized || gateAnswered w.row) &&
  (!w.row.applied || w.row.casOk) &&
  (!w.row.casOk || fp1Matches w.row) &&
  phaseOk w.row

/-- What `committedish` already implies about the row, read out of `phaseOk` once instead of at
    each use site. (sem: SEM-lean-242a) -/
theorem committed_row_facts {r : Row} (h : phaseOk r = true) (hc : committedish r.phase = true) :
    r.canonicalized = true ∧ r.casOk = true ∧ r.applied = true := by
  revert h hc
  cases hph : r.phase <;> simp [phaseOk, committedish, hph] <;>
    intro h1 h2 h3 <;> exact ⟨h1, h2, h3⟩

theorem all_of_hasTid_false {l : List Entry} {t : TransformationId} {P : Entry → Bool}
    (h : hasTid l t = false) : l.all (fun e => !tidEq e.tid t || P e) = true := by
  simp only [List.all_eq_true]
  intro e he
  have : e.tid ≠ t := not_mem_of_hasTid_false h e he
  simp [tidEq, this]

theorem wfB_of_isInit {w : World} (h : IsInit w) : wfB w = true := by
  simp [wfB, h.phase, h.admitted, h.denied, h.canon, h.applied, h.cas, h.receipt, h.fresh,
        h.keys, committedish, phaseOk]

/-- Preservation. One case per event; the shape is always the same — read the guard, discharge the
    `none` branch, then recompute the nine conjuncts on the successor. (sem: SEM-lean-243) -/
theorem wfB_step {c : Config} {w w' : World} (e : Event)
    (hw : wfB w = true) (hs : step c w e = some w') : wfB w' = true := by
  -- Unpack the conjuncts of the hypothesis once.
  simp only [wfB, Bool.and_eq_true, beq_iff_eq, Bool.or_eq_true, Bool.not_eq_true'] at hw
  obtain ⟨⟨⟨⟨⟨⟨hKeys, hOwn⟩, hRcptSome⟩, hLanded⟩, hCanon⟩, hApplied⟩, hCas⟩ := hw.1
  have hPhaseOk := hw.2
  cases e with
  | plan fp0 =>
      simp only [step] at hs
      split at hs
      · injection hs with hs; subst hs
        simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | verifyStart =>
      simp only [step] at hs
      split at hs
      · injection hs with hs; subst hs
        simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | verdictAdmit =>
      simp only [step] at hs
      split at hs
      · injection hs with hs; subst hs
        simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | verdictDeny =>
      simp only [step] at hs
      split at hs
      · injection hs with hs; subst hs
        simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | verdictEscalate =>
      simp only [step] at hs
      split at hs
      · injection hs with hs; subst hs
        simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | verifierUnreachable =>
      simp only [step] at hs
      split at hs
      · split at hs
        · injection hs with hs; subst hs
          simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
        · injection hs with hs; subst hs
          simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | humanAdmit =>
      simp only [step] at hs
      split at hs
      · injection hs with hs; subst hs
        simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | humanDeny =>
      simp only [step] at hs
      split at hs
      · injection hs with hs; subst hs
        simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | expire =>
      simp only [step] at hs
      split at hs
      case _ => injection hs with hs; subst hs; simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      case _ => injection hs with hs; subst hs; simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      case _ => injection hs with hs; subst hs; simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      case _ => simp at hs
  | ownerCancel =>
      simp only [step] at hs
      split at hs
      case _ => injection hs with hs; subst hs; simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      case _ => injection hs with hs; subst hs; simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      case _ => injection hs with hs; subst hs; simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      case _ => injection hs with hs; subst hs; simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      case _ => injection hs with hs; subst hs; simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      case _ => injection hs with hs; subst hs; simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      case _ => simp at hs
  | canonicalize =>
      simp only [step] at hs
      split at hs
      · injection hs with hs; subst hs
        simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | canonicalizeRecordOnly =>
      simp only [step] at hs
      split at hs
      · injection hs with hs; subst hs
        simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | commitStart =>
      simp only [step] at hs
      split at hs
      · injection hs with hs; subst hs
        simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | casCheck fp1 =>
      simp only [step] at hs
      split at hs
      · split at hs
        · rename_i heq
          injection hs with hs; subst hs
          simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk, snapEq]
        · injection hs with hs; subst hs
          simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | escrowInverse =>
      simp only [step] at hs
      split at hs
      · injection hs with hs; subst hs
        simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | applyFail =>
      simp only [step] at hs
      split at hs
      · rename_i hg
        injection hs with hs; subst hs
        simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk]
      · simp at hs
  | applyCommit canonCid =>
      simp only [step] at hs
      split at hs
      · rename_i hg
        obtain ⟨hPhase, hCasOk, hCanonTrue⟩ := hg
        injection hs with hs; subst hs
        have hFresh : hasTid w.ledger w.row.tid = false := by
          rw [hOwn, hPhase]; rfl
        have hGate : gateAnswered w.row = true := by
          rcases hCanon with hf | hg'
          · rw [hCanonTrue] at hf; exact absurd hf (by simp)
          · exact hg'
        have hfp : fp1Matches w.row = true := by
          rcases hCas with hf | hg'
          · rw [hCasOk] at hf; exact absurd hf (by simp)
          · exact hg'
        have htail : ∀ x ∈ w.ledger, ¬ (x.tid = w.row.tid) := not_mem_of_hasTid_false hFresh
        have hGate' : w.row.admittedOnce = true ∨
            (w.row.deniedOnce = true ∧ w.row.enforced = false) := by
          simp only [gateAnswered, Bool.or_eq_true, Bool.and_eq_true, Bool.not_eq_true'] at hGate
          exact hGate
        simp [wfB, appendKeyed, hFresh, committedish, phaseOk, keyUnique,
              hasTid_cons, tidEq, hKeys, hCasOk, hCanonTrue,
              ownEntriesFaithful, rcptOk, rcptFaithful, entryFaithful, gateAnswered,
              landedVerdict, htail]
        exact ⟨⟨⟨hGate', fun x hx => Or.inl (htail x hx)⟩, hGate'⟩, hfp⟩
      · simp at hs
  | supersede u =>
      simp only [step] at hs
      split at hs
      · rename_i hg
        obtain ⟨hPhase, _⟩ := hg
        injection hs with hs; subst hs
        have hC : committedish w.row.phase = true := by rw [hPhase]; rfl
        have hgood : w.row.canonicalized = true ∧ gateAnswered w.row = true ∧
            ownEntriesFaithful w = true ∧ rcptOk w = true := by
          rcases hLanded with hf | hg2
          · rw [hC] at hf; exact absurd hf (by simp)
          · exact ⟨hg2.1.1.1, hg2.1.1.2, hg2.1.2, hg2.2⟩
        obtain ⟨hcanon, hgate, hown, hrcpt⟩ := hgood
        obtain ⟨_, hcas, happ⟩ := committed_row_facts hPhaseOk hC
        have hfp : fp1Matches w.row = true := by
          rcases hCas with hf | hg2
          · rw [hcas] at hf; exact absurd hf (by simp)
          · exact hg2
        have hgate' : w.row.admittedOnce = true ∨
            (w.row.deniedOnce = true ∧ w.row.enforced = false) := by
          simp only [gateAnswered, Bool.or_eq_true, Bool.and_eq_true, Bool.not_eq_true'] at hgate
          exact hgate
        rw [hC] at hOwn hRcptSome
        simp [wfB, committedish, phaseOk, gateAnswered, fp1Matches, hKeys, hOwn, hRcptSome,
              hcanon, hcas, happ, hown, hrcpt]
        exact ⟨hgate', hfp⟩
      · simp at hs
  | internalError =>
      simp only [step] at hs
      split at hs
      case _ => simp at hs
      case _ => simp at hs
      case _ => simp at hs
      all_goals (injection hs with hs; subst hs; simp_all [wfB, committedish, gateAnswered, phaseOk, fp1Matches, ownEntriesFaithful, rcptOk])

theorem wfB_of_reachable {c : Config} {w : World} (h : Reachable c w) : wfB w = true := by
  induction h with
  | init hi => exact wfB_of_isInit hi
  | step e _ hstep ih => exact wfB_step e ih hstep

/-! ## §5 — 43 §9's seven safety invariants (sem: SEM-lean-244) -/

/-- Projections of the inductive invariant, so the seven statements below read as themselves.
    (sem: SEM-lean-245) -/
theorem wf_parts {w : World} (h : wfB w = true) :
    keyUnique w.ledger = true ∧
    hasTid w.ledger w.row.tid = committedish w.row.phase ∧
    w.receipt.isSome = committedish w.row.phase ∧
    (committedish w.row.phase = true →
      w.row.canonicalized = true ∧ gateAnswered w.row = true ∧
      ownEntriesFaithful w = true ∧ rcptOk w = true) ∧
    (w.row.canonicalized = true → gateAnswered w.row = true) ∧
    (w.row.applied = true → w.row.casOk = true) ∧
    (w.row.casOk = true → ∃ f, w.row.fp1 = some f ∧ f = w.row.fp0) ∧
    phaseOk w.row = true := by
  simp only [wfB, Bool.and_eq_true, beq_iff_eq, Bool.or_eq_true, Bool.not_eq_true'] at h
  obtain ⟨⟨⟨⟨⟨⟨hKeys, hOwn⟩, hRcptSome⟩, hLanded⟩, hCanon⟩, hApplied⟩, hCas⟩ := h.1
  refine ⟨hKeys, hOwn, hRcptSome, ?_, ?_, ?_, ?_, h.2⟩
  · intro hc
    rcases hLanded with hfalse | hgood
    · rw [hc] at hfalse; exact absurd hfalse (by simp)
    · exact ⟨hgood.1.1.1, hgood.1.1.2, hgood.1.2, hgood.2⟩
  · intro hcanon
    rcases hCanon with hfalse | hgood
    · rw [hcanon] at hfalse; exact absurd hfalse (by simp)
    · exact hgood
  · intro happ
    rcases hApplied with hfalse | hgood
    · rw [happ] at hfalse; exact absurd hfalse (by simp)
    · exact hgood
  · intro hcas
    rcases hCas with hfalse | hgood
    · rw [hcas] at hfalse; exact absurd hfalse (by simp)
    · revert hgood
      simp only [fp1Matches]
      cases hfp : w.row.fp1 with
      | none => intro hg; exact absurd hg (by simp)
      | some f => intro hg; exact ⟨f, rfl, snapEq_iff.mp hg⟩

/-- Ledger entries under this row's key tell the truth about it. Only available once the row has
    landed — before that there are none, which conjunct two of `wfB` states directly.
    (sem: SEM-lean-245a) -/
theorem entry_faithful_of_wf {w : World} (h : wfB w = true)
    (hc : committedish w.row.phase = true) (e : Entry) (he : e ∈ w.ledger)
    (ht : e.tid = w.row.tid) : entryFaithful e w.row = true := by
  obtain ⟨_, _, _, hLanded, _⟩ := wf_parts h
  obtain ⟨_, _, hown, _⟩ := hLanded hc
  simp only [ownEntriesFaithful, List.all_eq_true] at hown
  have := hown e he
  simp only [Bool.or_eq_true, Bool.not_eq_true'] at this
  rcases this with hfalse | hgood
  · rw [tidEq] at hfalse
    simp only [decide_eq_false_iff_not] at hfalse
    exact absurd ht hfalse
  · exact hgood

/-- The receipt tells the truth about the row. (sem: SEM-lean-245b) -/
theorem rcpt_faithful_of_wf {w : World} {r : Rcpt} (h : wfB w = true)
    (hr : w.receipt = some r) : rcptFaithful r w.row = true := by
  obtain ⟨_, _, hRcptSome, hLanded, _⟩ := wf_parts h
  have hc : committedish w.row.phase = true := by
    rw [← hRcptSome, hr]; rfl
  obtain ⟨_, _, _, hok⟩ := hLanded hc
  simp only [rcptOk, hr] at hok
  exact hok

/-- **INV-S1** (43 §9). Every path into `Committed` passes through `Admitted ∧ Canonicalized`, or —
    under the T-8r record-only exception — through `Denied ∧ enforced = false ∧ Canonicalized`.
    There is no third door. Rust: `gx-engine/tests/ac_041.rs`, `ac_042.rs`. (sem: SEM-lean-246) -/
theorem inv_S1_committed_passed_the_gate {c : Config} {w : World}
    (h : Reachable c w) (hc : committedish w.row.phase = true) :
    w.row.canonicalized = true ∧
      (w.row.admittedOnce = true ∨ (w.row.deniedOnce = true ∧ w.row.enforced = false)) := by
  obtain ⟨_, _, _, hLanded, _, _, _, _⟩ := wf_parts (wfB_of_reachable h)
  obtain ⟨hcanon, hgate, _, _⟩ := hLanded hc
  refine ⟨hcanon, ?_⟩
  simp only [gateAnswered, Bool.or_eq_true, Bool.and_eq_true, Bool.not_eq_true'] at hgate
  exact hgate

/-- **INV-S2** (43 §9, 43 §5-4). `Committed` is immutable. Stated on the edge: from `committed`,
    *every* successful step leaves the ledger and the receipt pointwise unchanged and lands in
    `superseded` with `superseded_by` set — so the quantifier is over all nineteen events, not over
    the reachable set. Rust: `gx-engine/tests/ac_044.rs`, `ac_040.rs`. (sem: SEM-lean-247) -/
theorem inv_S2_committed_is_append_only {c : Config} {w w' : World} (e : Event)
    (hp : w.row.phase = .committed) (hs : step c w e = some w') :
    w'.ledger = w.ledger ∧ w'.receipt = w.receipt ∧
      w'.row.phase = .superseded ∧ w'.row.supersededBy.isSome = true := by
  cases e
  case supersede u =>
    simp only [step, hp] at hs
    split at hs
    · injection hs with hs; subst hs; exact ⟨rfl, rfl, rfl, rfl⟩
    · simp at hs
  all_goals (simp [step, hp] at hs)

/-- **INV-S3** (43 §9). At most one ledger entry per `TransformationId` — exactly-once, proved as
    a count over the log and not as "the log is short". Rust: `gx-engine/tests/ac_035.rs`,
    `ac_043.rs`, `commit_protocol.rs`, `concurrent_commit.rs`. (sem: SEM-lean-248) -/
theorem inv_S3_at_most_one_ledger_entry {c : Config} {w : World}
    (h : Reachable c w) (t : TransformationId) :
    (w.ledger.filter (fun e => tidEq e.tid t)).length ≤ 1 := by
  obtain ⟨hKeys, _⟩ := wf_parts (wfB_of_reachable h)
  exact filter_length_le_one t w.ledger hKeys

/-- **INV-S4** (43 §9). An `Aborted` or (enforce-mode) `Denied` transformation does not appear in
    the ledger: an entry under this row's key exists exactly when the row landed. Rust:
    `gx-engine/tests/ac_034.rs`, `ac_038.rs`, `ac_041.rs`, `ac_072.rs`, `ac_073.rs`.
    (sem: SEM-lean-249) -/
theorem inv_S4_only_committed_is_witnessed {c : Config} {w : World}
    (h : Reachable c w) :
    hasTid w.ledger w.row.tid = committedish w.row.phase := by
  obtain ⟨_, hOwn, _⟩ := wf_parts (wfB_of_reachable h)
  exact hOwn

/-- **INV-S4**, the carve-out named rather than left implicit: the one way a `Deny` reaches the
    ledger is T-8r, and T-8r stamps `enforced = false`. So an entry of this row that carries `deny`
    carries `enforced = false`. (sem: SEM-lean-250) -/
theorem inv_S4_a_denial_in_the_ledger_is_unenforced {c : Config} {w : World}
    (h : Reachable c w) (e : Entry) (he : e ∈ w.ledger) (ht : e.tid = w.row.tid)
    (hv : e.verdict = Verdict.deny) : e.enforced = false := by
  have hwf := wfB_of_reachable h
  -- the row is in the ledger, so it landed
  have hc : committedish w.row.phase = true := by
    obtain ⟨_, hOwn, _⟩ := wf_parts hwf
    have hpresent : hasTid w.ledger w.row.tid = true := by
      simp only [hasTid, List.any_eq_true]
      exact ⟨e, he, by simp [tidEq, ht]⟩
    rw [← hOwn]; exact hpresent
  have hfaith := entry_faithful_of_wf hwf hc e he ht
  simp only [entryFaithful, Bool.and_eq_true, beq_iff_eq] at hfaith
  obtain ⟨henf, hverd⟩ := hfaith
  have hlv : landedVerdict w.row = Verdict.deny := by
    rw [← verdictEq_iff.mp hverd, hv]
  simp only [landedVerdict] at hlv
  by_cases hadm : w.row.admittedOnce = true
  · rw [hadm] at hlv; exact absurd hlv (by simp)
  · simp only [Bool.not_eq_true] at hadm
    obtain ⟨_, hgate, _, _⟩ := (wf_parts hwf).2.2.2.1 hc
    simp only [gateAnswered, hadm, Bool.false_or, Bool.and_eq_true, Bool.not_eq_true'] at hgate
    rw [henf]; exact hgate.2

/-- **INV-S5** (43 §9). A `Committed` with `enforced = false` is inscribed in the receipt in a form
    that keeps it distinguishable from `enforced = true`. First half: the receipt's flags are the
    row's. Rust: `gx-engine/tests/ac_033.rs`, `ac_039.rs`, `journal_identity.rs`.
    (sem: SEM-lean-251) -/
theorem inv_S5_receipt_carries_enforcement {c : Config} {w : World} {r : Rcpt}
    (h : Reachable c w) (hr : w.receipt = some r) :
    r.enforced = w.row.enforced ∧ r.failPostureEngaged = w.row.failPostureEngaged := by
  have := rcpt_faithful_of_wf (wfB_of_reachable h) hr
  simp only [rcptFaithful, Bool.and_eq_true, beq_iff_eq] at this
  exact ⟨this.1.1.2, this.1.2⟩

/-- **INV-S5**, second half — the part that makes the first half mean something. Two commits that
    differ only in `enforced` produce receipts that are not equal, so the audit distinction survives
    canonicalisation rather than being a field nobody reads. (sem: SEM-lean-252) -/
theorem inv_S5_distinguishable {c : Config} {w₁ w₂ w₁' w₂' : World} {r₁ r₂ : Rcpt}
    {canonCid : ObjectSnapshot}
    (h₁ : step c w₁ (.applyCommit canonCid) = some w₁')
    (h₂ : step c w₂ (.applyCommit canonCid) = some w₂')
    (hr₁ : w₁'.receipt = some r₁) (hr₂ : w₂'.receipt = some r₂)
    (hne : w₁.row.enforced ≠ w₂.row.enforced) : r₁ ≠ r₂ := by
  simp only [step] at h₁ h₂
  split at h₁
  · injection h₁ with h₁; subst h₁
    split at h₂
    · injection h₂ with h₂; subst h₂
      simp only [Option.some.injEq] at hr₁ hr₂
      subst hr₁; subst hr₂
      intro hcontra
      exact hne (congrArg Rcpt.enforced hcontra)
    · exact absurd h₂ (by simp)
  · exact absurd h₁ (by simp)

/-- **INV-S6** (43 §9). `Escalated` does not move to `Admitted` or `Denied` without a signed human
    ruling (T-5 / T-5b). On the edge, so no unlisted event can slip past it. Rust:
    `gx-engine/tests/ac_037.rs`, `ac_071.rs`. (sem: SEM-lean-253) -/
theorem inv_S6_escalation_needs_a_person {c : Config} {w w' : World} (e : Event)
    (hp : w.row.phase = .escalated) (hs : step c w e = some w')
    (hlands : w'.row.phase = .admitted ∨ w'.row.phase = .denied) :
    w'.row.humanRuling = true := by
  cases e <;> simp [step, hp] at hs <;> subst hs <;> simp_all

/-- **INV-S7** (43 §9, CON-2). `adapter.apply` is never called when `Fingerprint₁ ≠ Fingerprint₀`.
    Read forwards: if the row applied, then a commit-time fingerprint was observed and it equalled
    the planned one. Rust: `gx-engine/tests/ac_031.rs`, `ac_034.rs`, `concurrent_commit.rs`,
    `binary_e2e.rs`. (sem: SEM-lean-254) -/
theorem inv_S7_apply_implies_cas_matched {c : Config} {w : World}
    (h : Reachable c w) (ha : w.row.applied = true) :
    w.row.fp1 = some w.row.fp0 := by
  obtain ⟨_, _, _, _, _, hApplied, hCas, _⟩ := wf_parts (wfB_of_reachable h)
  obtain ⟨f, hf, hfe⟩ := hCas (hApplied ha)
  rw [hf, hfe]

/-- **INV-S7**, the contrapositive on the edge: a `casCheck` that observes a fingerprint other than
    the planned one aborts with `PreconditionChanged`, and `applied` is untouched. This is the
    sentence 43 T-10a writes as "apply not executed". (sem: SEM-lean-255) -/
theorem inv_S7_mismatch_never_applies {c : Config} {w w' : World} {fp1 : ObjectSnapshot}
    (hne : fp1 ≠ w.row.fp0) (hs : step c w (.casCheck fp1) = some w') :
    w'.row.phase = .aborted ∧ w'.row.abortReason = some .preconditionChanged ∧
      w'.row.applied = w.row.applied ∧ w'.row.casOk = false := by
  simp only [step] at hs
  split at hs
  · injection hs with hs; subst hs
    exact ⟨rfl, rfl, rfl, rfl⟩
  · simp at hs

/-! ## §6 — why INV-L1..INV-L4 are not here (sem: SEM-lean-256) -/

/-! Fixtures for §6 and §7. Byte constants, in the shape `Receipt.lean`'s non-vacuity witness
uses. (sem: SEM-lean-257) -/
namespace Fixture

def tidA : TransformationId := ⟨ByteArray.mk #[11]⟩
def snapA : ObjectSnapshot := ⟨ByteArray.mk #[21]⟩

def cfg : Config := { mode := .enforce, failPosture := .failClosed }

/-- A world at `Draft` over an empty ledger. (sem: SEM-lean-258) -/
def w0 : World :=
  { row := { tid := tidA, fp0 := snapA, fp1 := none, phase := .draft
           , admittedOnce := false, deniedOnce := false, canonicalized := false
           , enforced := true, failPostureEngaged := false, humanRuling := false
           , casOk := false, applied := false, supersededBy := none, abortReason := none }
  , ledger := []
  , receipt := none }

theorem w0_isInit : IsInit w0 :=
  { phase := rfl, admitted := rfl, denied := rfl, canon := rfl, enforced := rfl
  , fpEngaged := rfl, human := rfl, fp1 := rfl, cas := rfl, applied := rfl
  , supersededBy := rfl, receipt := rfl, keys := rfl, fresh := rfl }

/-- The same world one step on: `Candidate`, which 43 §1 lists as non-terminal.
    (sem: SEM-lean-259) -/
def w1 : World := { w0 with row := { w0.row with phase := .candidate, fp0 := snapA } }

theorem w1_reachable : Reachable cfg w1 :=
  Reachable.step (.plan snapA) (Reachable.init w0_isInit) rfl

end Fixture

/-- **Why INV-L1..INV-L4 are absent, stated mechanically rather than as prose.**

    43 §9's four liveness rows all say "within finite time". 46 §1.1 item 6 puts time and
    randomness outside the model, and this file does not smuggle them back in. The consequence is
    not an opinion, and here it is as a proposition: there is a reachable world sitting in the
    non-terminal state `Candidate` which satisfies **every** invariant §5 proves, and the model
    admits a step from `Committing` that changes nothing at all (T-10b, denominator 6). A property
    of reachable *states* therefore cannot distinguish a run that progresses from one that does
    not, and "eventually leaves `Candidate`" is not a property of reachable states.

    Recorded as **UNTESTABLE, not as failing** (`req/38` §SS870): an invariant this model cannot
    express is not an invariant this model refutes. The three ranks for INV-L1..L4 are 43 §9.1's
    to keep, and its Lean column has read *unmodelled* since 2026-08-15 — which, after this file,
    is still the correct entry. (sem: SEM-lean-260) -/
theorem liveness_needs_a_clock_counterexample :
    Reachable Fixture.cfg Fixture.w1 ∧
    Fixture.w1.row.phase = Phase.candidate ∧
    wfB Fixture.w1 = true ∧
    (∀ (w : World), w.row.phase = Phase.committing → w.row.casOk = true →
        step Fixture.cfg w .escrowInverse = some w) := by
  refine ⟨Fixture.w1_reachable, rfl, wfB_of_reachable Fixture.w1_reachable, ?_⟩
  intro w hp hc
  simp [step, hp, hc]

/-! ## §7 — the refinement statement (sem: SEM-lean-261) -/

/-- An implementation, for the purpose of this section, is any transition system over an opaque
    state type together with an abstraction map into this model. Nothing is assumed about the
    carrier — this is deliberately the weakest hypothesis that lets §5's theorems travel.
    (sem: SEM-lean-262) -/
structure Impl (c : Config) where
  Carrier : Type
  init    : Carrier → Prop
  next    : Carrier → Carrier → Prop
  abs     : Carrier → World

inductive ImplReachable {c : Config} (M : Impl c) : M.Carrier → Prop where
  | init {s} : M.init s → ImplReachable M s
  | step {s s'} : ImplReachable M s → M.next s s' → ImplReachable M s'

/-- **The refinement hypothesis, named.** An implementation refines this model when its initial
    states abstract to initial worlds and each of its steps abstracts either to a step of this
    model or to no change at all (stuttering).

    This is the proposition `docs/LIMITS.md` item 8 says nothing connects to `crates/gx-engine`.
    It is still unproved for the engine after this file; what this file changes is that the
    proposition now exists and can be aimed at. (sem: SEM-lean-263) -/
def Simulates {c : Config} (M : Impl c) : Prop :=
  (∀ s, M.init s → IsInit (M.abs s)) ∧
  (∀ s s', M.next s s' → M.abs s' = M.abs s ∨ ∃ e, step c (M.abs s) e = some (M.abs s'))

/-- **Refinement transports every safety invariant of §5.** If `M` simulates the model, then every
    `M`-reachable state abstracts to a reachable world, hence satisfies INV-S1..INV-S7 by the
    corollaries above. This is the theorem; the content is entirely in its hypothesis, which is why
    `forged_transition_counterexample` sits directly below it. (sem: SEM-lean-264) -/
theorem refinement_transports_safety {c : Config} (M : Impl c) (hsim : Simulates M) :
    ∀ s, ImplReachable M s → Reachable c (M.abs s) := by
  intro s h
  induction h with
  | init hi => exact Reachable.init (hsim.1 _ hi)
  | step _ hnext ih =>
      rcases hsim.2 _ _ hnext with hstutter | ⟨e, he⟩
      · rw [hstutter]; exact ih
      · exact Reachable.step e ih he

/-- The corollary a reader of §5 wants: under simulation, INV-S1 is a statement about the
    implementation's own reachable states. The same one-liner gives INV-S3, S4, S5, S7 — they are
    not repeated. (sem: SEM-lean-265) -/
theorem refined_inv_S1 {c : Config} (M : Impl c) (hsim : Simulates M)
    (s : M.Carrier) (h : ImplReachable M s)
    (hc : committedish (M.abs s).row.phase = true) :
    (M.abs s).row.canonicalized = true ∧
      ((M.abs s).row.admittedOnce = true ∨
        ((M.abs s).row.deniedOnce = true ∧ (M.abs s).row.enforced = false)) :=
  inv_S1_committed_passed_the_gate (refinement_transports_safety M hsim s h) hc

/-- **Non-vacuity**: the hypothesis is satisfiable. The model simulates itself, taking `next` to be
    "some event fires". Paired with the counterexample below on the same model — the only moving
    part between them is whether the abstraction's steps are steps. (sem: SEM-lean-266) -/
def identityImpl (c : Config) : Impl c :=
  { Carrier := World
  , init    := fun w => IsInit w
  , next    := fun w w' => ∃ e, step c w e = some w'
  , abs     := id }

theorem identity_simulates (c : Config) : Simulates (identityImpl c) := by
  constructor
  · intro s hi; exact hi
  · intro s s' h
    obtain ⟨e, he⟩ := h
    exact Or.inr ⟨e, he⟩

/-- **The hypothesis is load-bearing, and here is what it is holding up.** `forgedImpl` is a
    transition system that jumps straight from an initial world to a `Committed` one that was never
    canonicalised and never passed a gate. It is `ImplReachable`-total, it violates INV-S1, and —
    necessarily — it is not a `Simulates`.

    Read this as the standing warning of `req/38` §SS854 in mechanical form: a conditional theorem
    is only as strong as the hypothesis nobody checks, and the effort spent proving
    `refinement_transports_safety` says exactly nothing about `crates/gx-engine` until somebody
    discharges `Simulates` for it. (sem: SEM-lean-267) -/
def forgedWorld : World :=
  { Fixture.w0 with
      row := { Fixture.w0.row with phase := .committed, applied := true, casOk := true } }

def forgedImpl (c : Config) : Impl c :=
  { Carrier := Bool
  , init    := fun b => b = false
  , next    := fun b b' => b = false ∧ b' = true
  , abs     := fun b => if b then forgedWorld else Fixture.w0 }

theorem forged_transition_counterexample (c : Config) :
    ImplReachable (forgedImpl c) true ∧
    committedish ((forgedImpl c).abs true).row.phase = true ∧
    ((forgedImpl c).abs true).row.canonicalized = false ∧
    ¬ Simulates (forgedImpl c) := by
  refine ⟨ImplReachable.step (ImplReachable.init rfl) ⟨rfl, rfl⟩, rfl, rfl, ?_⟩
  intro hsim
  have hreach : Reachable c ((forgedImpl c).abs true) :=
    refinement_transports_safety (forgedImpl c) hsim true
      (ImplReachable.step (ImplReachable.init rfl) ⟨rfl, rfl⟩)
  have := inv_S1_committed_passed_the_gate hreach (by rfl)
  exact absurd this.1 (by simp [forgedImpl, forgedWorld, Fixture.w0])

/-! ## §8 — executable sanity checks (sem: SEM-lean-268)

`Canon.lean` puts `#guard`s beside its model so a reader can see the definitions run rather than
take the proofs' word for it. The same here: the happy path, the record-only path, and the CAS
mismatch, each evaluated. A `#guard` that fails is a build failure, so these are checks and not
comments. -/

namespace Guards

open Fixture

def runFrom (c : Config) (w : World) : List Event → Option World
  | []      => some w
  | e :: es => match step c w e with
               | some w' => runFrom c w' es
               | none    => none

/-- The enforce-mode happy path: plan, verify, admit, canonicalise, commit-start, CAS pass, escrow,
    apply. (sem: SEM-lean-269) -/
def happyPath : List Event :=
  [.plan snapA, .verifyStart, .verdictAdmit, .canonicalize, .commitStart, .casCheck snapA,
   .escrowInverse, .applyCommit snapA]

#guard (runFrom cfg w0 happyPath).map (fun w => w.row.phase) == some Phase.committed
#guard (runFrom cfg w0 happyPath).map (fun w => w.ledger.length) == some 1
#guard (runFrom cfg w0 happyPath).map (fun w => w.row.enforced) == some true
#guard (runFrom cfg w0 happyPath).map wfB == some true

/-- T-8r is refused under `Enforce` and taken under `RecordOnly`; the landed row is stamped
    `enforced = false`, which is INV-S5's whole subject. (sem: SEM-lean-270) -/
def recordOnlyPath : List Event :=
  [.plan snapA, .verifyStart, .verdictDeny, .canonicalizeRecordOnly, .commitStart,
   .casCheck snapA, .applyCommit snapA]

#guard (runFrom cfg w0 recordOnlyPath).isNone
#guard (runFrom { mode := .recordOnly, failPosture := .failClosed } w0 recordOnlyPath).map
         (fun w => w.row.phase) == some Phase.committed
#guard (runFrom { mode := .recordOnly, failPosture := .failClosed } w0 recordOnlyPath).map
         (fun w => w.row.enforced) == some false
#guard (runFrom { mode := .recordOnly, failPosture := .failClosed } w0 recordOnlyPath).map
         wfB == some true

/-- A CAS that observes a different fingerprint aborts, and the ledger stays empty — INV-S7 and
    INV-S4 in the same trace. (sem: SEM-lean-271) -/
def casMismatchPath : List Event :=
  [.plan snapA, .verifyStart, .verdictAdmit, .canonicalize, .commitStart,
   .casCheck ⟨ByteArray.mk #[99]⟩]

#guard (runFrom cfg w0 casMismatchPath).map (fun w => w.row.phase) == some Phase.aborted
#guard (runFrom cfg w0 casMismatchPath).map (fun w => w.row.applied) == some false
#guard (runFrom cfg w0 casMismatchPath).map (fun w => w.ledger.length) == some 0
#guard (runFrom cfg w0 casMismatchPath).map wfB == some true

end Guards

end StateMachine
end GxSpec
