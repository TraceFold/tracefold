import GxSpec.Core
import GxSpec.Minimality
import GxSpec.EffectAlgebra

/-!
# GxSpec.Attribution — the attribution invariant: what a resuming engine can and cannot decide
about who moved the world (sem: SEM-lean-148)

Identity (sem: SEM-lean-149): `DR-R34-1` (`req/38` §257 ruling 4 item 1, "apply-post fingerprint
record = M5H5-3, the headline one", filed out of `req/451` §5 and unblocked for implementation by
`req/38` §291 ruling 5 -- the Phase R exit). The material lane for this file is the read-only
collection `drr341_materials.md` §1-§4, whose §2-5 states in its own denominator that
"the `lean/` side's dependence on `EngineJournalRecord` is **unconfirmed**". This file is the
answer to that line: the Lean side had **no** model of the journal at all, so the dependence was
zero, and the useful move is not to transcribe the enum but to formalise the *invariant* the
missing record is missing from.

## The one sentence this file is about (sem: SEM-lean-150)

A resume after a crash in the commit window has to answer: *did my own apply land, or did somebody
else move the world?* `req/451` §1-4 establishes, from the Rust source, that the answer is not
constructible today, because the journal records the **pre**-apply fingerprint (`Planned.fp0`,
`store.rs:507`) and the commit path computes the apply-time fingerprint `fp1b` and **discards** it
(`pipeline.rs:6348-6357`). The two hypotheses -- call them `H_self` (our own delta landed) and
`H_third` (a third party moved the world) -- therefore "fall into the same bucket": both are worlds
in which `fp_now != fp0`, and that is the entire observation.

This file makes that argument a machine-checked theorem rather than a prose reading, and pairs it
with the recovery: the same question **is** decidable, soundly and on every faithful scenario, once
the post-apply fingerprint is a recorded value. (sem: SEM-lean-151)

## Discipline (the five conditions of `req/160` §3-1 / `req/38` §98 ruling 3, restated here)

`Injection.lean` established the house form (breakage theorem + projection-recovery theorem on the
same fixtures) and `EffectAlgebra.lean` extended it to vocabulary additions. This file is a
vocabulary addition of the second kind, and the conditions are discharged as follows.
(sem: SEM-lean-152)

* **C1 (conservative extension)**: the frozen modules are used by `import` only -- no frozen file is
  edited, no frozen definition shadowed, no frozen theorem restated in a weaker form. Nothing below
  introduces an axiom, so the axiom set stays `{propext, Quot.sound, GxSpec.composeId}`; the machine
  check is `lake build` of the whole root plus the proof-placeholder grep, and the word that grep
  looks for is avoided in this file's prose exactly as `Injection.lean` avoids it.
  (sem: SEM-lean-153)
* **C2 (counterexample-only vocabulary)**: `Scenario` / `AttrVerdict` / `RecordKind` /
  `JournalFormat` appear in exactly two roles -- the *breakage* side (a design in which the
  post-apply fingerprint is absent, or in which a format classifier carries a catch-all arm) and the
  *recovery* side (the same fixtures with the value recorded, or with the arm written out). **No
  theorem below asserts that gx's running implementation satisfies or violates anything.** The
  subject is always the model; the bridge from the model to `crates/gx-engine` is a reading of
  `req/451` §1-4 and is carried in prose, never in a proposition. (sem: SEM-lean-154)
* **C3 (explicit subject)**: §2's subject is "a recovery procedure whose only fingerprint inputs are
  the planned one and the current one" -- a design, quantified over *all* classifiers of that input
  type. That universal quantifier is what makes §2 an impossibility statement **inside the model**,
  and it is deliberately not an impossibility claim about all conceivable designs: §3 exhibits a
  design outside the quantifier's scope that decides the question. §4's subject is "a format
  classifier with a catch-all arm"; §5's is "a rank function that folds two formats together".
  (sem: SEM-lean-155)
* **C4 (minimal addition)**: the additions are exactly what `DR-R34-1` legislates about and nothing
  else -- three `ObjectSnapshot`-valued fingerprints and one ground-truth tag (§0), one three-valued
  verdict (§1), and, for §4-§5, three record kinds and three journal formats. `ObjectSnapshot` is
  the frozen type reused as the fingerprint carrier (in F0 only digest equality is meaningful, which
  is exactly what a fingerprint comparison uses); no cbor/hash/fsync machinery is reconstructed
  (`46` §1's non-goals stand, and durability is `DR-46-6` stage A3, still undelivered).
  (sem: SEM-lean-156)
* **C5 (positive control paired with every breakage)**: §2's `applyFingerprint_absence_counterexample`
  is paired with §3's `record_separates_the_fixtures` on **the same two scenarios** -- the only
  moving part between them is whether `fpSelf` is in the observation. §4's
  `defaultedArm_counterexample` is paired with `exhaustiveArm_recovers` on the same kind. §5's
  `rankFold_admits_chainedOnly_into_legacy` is paired with `rankSplit_separates_chainedOnly` on the
  same hypothesis. §6's collapse is paired with `reconstruction_decides_ObsEq`. (sem: SEM-lean-157)

## What is proved here, precisely (P-9 -- no overclaim) (sem: SEM-lean-158)

Proved: (a) two faithful scenarios that differ in ground truth produce **identical** observations
under the pre-`DR-R34-1` observation map, hence *every* function of that observation gives them the
same verdict and no function of it is sound (§2); (b) with the post-apply fingerprint in the
observation, one concrete classifier is sound on **every** faithful scenario, not merely on the
fixtures (§3); (c) the residual crash window -- apply returned, the record is not yet durable --
maps to `undetermined`, never to either affirmative verdict, and `undetermined` occurs **if and only
if** the record is missing, so the fail-closed bucket neither leaks nor absorbs (§3); (d) a format
classifier with a catch-all arm agrees with the exhaustive one on every kind that existed when the
arm was written, and disagrees exactly on the new kind, which it admits into a v1 journal (§4);
(e) a rank function that folds two formats to the same number cannot refuse any record from either,
and un-folding it restores the refusal (§5); (f) the pre/post observation pair that
`EffectAlgebra`'s laws are stated on is reconstructible from a journal exactly when the post value
is recorded, and the reconstruction decides `ObsEq` (§6).

Not proved, and not claimable from anything below: that `crates/gx-engine` has or lacks any of this
(the file has no Rust in it); that the separation is unconditional (see denominator 1); that the
window has any particular *size* (that is a measurement, and `req/388` §4's retraction of a
reconstructed number is the standing warning -- see denominator 4); that the format vocabulary below
is the real one (denominator 3); anything about durability, concurrency or crash *recovery* as such
(`46` §1 non-goals, `DR-46-6` stage A3, still undelivered -- this file formalises the *attribution*
fragment of A3's territory and claims no more of it).

## Denominator, stated once here rather than left to be noticed by absence (sem: SEM-lean-159)

1. **The separation is conditional on the absence of a fingerprint collision.** `Faithful`'s third
   clause says that in a third-party world the current fingerprint differs from the one our own
   delta would have left. A third party that happened to land on exactly our post-fingerprint is
   *excluded by hypothesis*, not refuted -- collision resistance is `46` §1's crypto non-goal and
   Lean neither proves nor could prove it here. §3 therefore says "the record makes the separation
   **constructible**", not "the separation is unconditional".
2. **Two hypotheses, not the full lifecycle.** `Authorship` has two constructors. "Nothing landed at
   all" (`fp_now = fp0`) is excluded by `Faithful`'s first clause, deliberately: it is the case
   `req/451` §1-4 says is *already* decidable, and including it would let a classifier score points
   on a bucket that was never in dispute. `Aborted`, `Superseded` and the rest of the fifteen
   journal kinds have no counterpart here.
3. **Three record kinds and three formats, not fifteen and not the real names.** §4-§5 model the
   *shape* of `minimum_format` / `vocabulary_rank` (`store.rs:805`, `replay.rs:306-312`) with the
   smallest vocabulary in which the catch-all defect and the fold defect are expressible: one kind
   that predates both, one that is the `Aborted{rollback: Diverged}` precedent, one that is the new
   one. `JournalFormat.chained` has, in the real system, *no* record kind that requires exactly it;
   §5's theorems are accordingly stated with that kind's existence as a **hypothesis** (`mf k =
   chained`), which is the honest form of "this gap is structural and currently unexercised" --
   the reading `req/449` L-01 gave it.
4. **No window size.** §3 says where the window's verdict lands, not how wide it is. The width is an
   engine-driven measurement on two filesystems (`drr341_materials.md` §4-2, restating `req/388`
   §4's retraction of an adapter-side reconstruction), and no number appears below.
5. **The bridge to Rust is prose.** Every coordinate cited in this header is cited as the *reason a
   proposition is worth stating*, never as a premise of one. If `req/451` §1-4's reading of
   `pipeline.rs` were wrong, every theorem below would still be true and none of them would still be
   interesting -- which is the correct dependency direction for a model.
-/

namespace GxSpec.Attribution

open GxSpec
open GxSpec.Minimality (x1 x2 x1_ne_x2)
open GxSpec.EffectAlgebra (obs ObsEq)

-- =============================================================================================
-- § 0. The world: ground truth and the three fingerprints (C4 -- this is the entire addition on
-- the attribution side).
-- =============================================================================================

/-- Ground truth about the commit window: which of the two competing hypotheses actually holds.
    `req/451` §1-4's `H_self` and `H_third`, and nothing else (denominator 2). Not observable --
    that is the whole difficulty. (sem: SEM-lean-160) -/
inductive Authorship where
  /-- Our own delta landed in the substrate. -/
  | selfApplied
  /-- Something that is not our delta moved the substrate. -/
  | thirdParty
  deriving DecidableEq, Repr

/-- The verdict a resume procedure returns. Three-valued because the residual window (apply
    returned, the record is not yet durable) must land somewhere that is neither affirmative
    answer -- `req/38` §257 ruling 4's "falls into 'none of them' and is fail-closed".
    (sem: SEM-lean-161) -/
inductive AttrVerdict where
  | selfApplied
  | thirdParty
  | undetermined
  deriving DecidableEq, Repr

/-- One recovery situation. `fp0` is the fingerprint the plan was made against (`Planned.fp0`, the
    one value the journal does carry). `fpSelf` is the fingerprint our own delta leaves behind --
    `fp_post(delta)` in `req/451` §1-4's notation, the value `DR-R34-1` proposes to record and that
    is today computed and thrown away. `fpNow` is what the substrate reads at resume time.
    `ObjectSnapshot` is reused as the fingerprint carrier because in F0 only digest equality is
    meaningful, which is exactly the operation a fingerprint comparison performs (C4).
    (sem: SEM-lean-162) -/
structure Scenario where
  truth  : Authorship
  fp0    : ObjectSnapshot
  fpSelf : ObjectSnapshot
  fpNow  : ObjectSnapshot

/-- A scenario is *faithful* when its fingerprints are consistent with its ground truth: the world
    has already moved (first clause -- the bucket `req/451` §1-4 says is unsplittable, denominator
    2); a self-applied world reads back our own post-fingerprint (second clause); a third-party
    world does not (third clause -- the no-collision hypothesis, denominator 1). Everything proved
    below is quantified over faithful scenarios, so the hypothesis is visible at every use site
    rather than buried in a fixture. (sem: SEM-lean-163) -/
def Faithful (s : Scenario) : Prop :=
  s.fp0 ≠ s.fpNow ∧
    (s.truth = Authorship.selfApplied → s.fpNow = s.fpSelf) ∧
    (s.truth = Authorship.thirdParty → s.fpNow ≠ s.fpSelf)

-- =============================================================================================
-- § 1. The two observation maps -- the *only* thing that differs between the broken design and
-- the sound one.
-- =============================================================================================

/-- What a resuming engine can see today: the planned fingerprint and the current one. This is
    `req/451` §1-4's inventory verbatim -- `Planned.fp0` is the journal's only fingerprint record,
    and `fp1b` is computed and discarded, so it is not in the image of this map.
    (sem: SEM-lean-164) -/
def obsToday (s : Scenario) : ObjectSnapshot × ObjectSnapshot := (s.fp0, s.fpNow)

/-- What `DR-R34-1`'s record adds: the post-apply fingerprint joins the observation. The `Option`
    is not decoration -- it is the residual crash window (§3), the one place where the record can
    legitimately be missing after the change lands. (sem: SEM-lean-165) -/
def obsWithRecord (s : Scenario) :
    ObjectSnapshot × Option ObjectSnapshot × ObjectSnapshot :=
  (s.fp0, some s.fpSelf, s.fpNow)

/-- The classifier `DR-R34-1` makes constructible. `none` -- the record was never made durable --
    returns `undetermined`; otherwise the current fingerprint is compared against the recorded
    post-apply one. Note what it does **not** consult: `fp0`. Once `fpSelf` is available, the
    planned fingerprint carries no further information for this question, which is a compact way of
    saying that `fp_post` is the value that was missing and not merely *a* missing value.
    (sem: SEM-lean-166) -/
def classify (fpPost : Option ObjectSnapshot) (fpNow : ObjectSnapshot) : AttrVerdict :=
  match fpPost with
  | none => AttrVerdict.undetermined
  | some fp => if fpNow = fp then AttrVerdict.selfApplied else AttrVerdict.thirdParty

/-- The classifier applied to a scenario whose record survived. (sem: SEM-lean-167) -/
def classifyRec (s : Scenario) : AttrVerdict := classify (some s.fpSelf) s.fpNow

-- =============================================================================================
-- § 2. Fixtures and the breakage theorem: without the record, every classifier is blind.
-- =============================================================================================

/-- One fresh piece of content. Two of the three fingerprints this file needs are the shared corpus
    (`Minimality.x1`/`x2`, reused per the precedent `EffectAlgebra.lean` set and `req/191` §5
    attack 3 defended); the counterexample needs a **third** distinct value -- the post-fingerprint
    of a delta that never landed -- and this is exactly that one byte, not a fresh corpus.
    (sem: SEM-lean-168) -/
def x3 : ObjectSnapshot := ⟨ByteArray.mk #[31]⟩

theorem x1_ne_x3 : x1 ≠ x3 := by decide

theorem x2_ne_x3 : x2 ≠ x3 := by decide

/-- `H_self`: the plan was made against `x1`, our delta landed, and the world reads back `x2` --
    which is precisely the fingerprint our delta leaves. (sem: SEM-lean-169) -/
def sSelf : Scenario :=
  { truth := Authorship.selfApplied, fp0 := x1, fpSelf := x2, fpNow := x2 }

/-- `H_third`: the same plan against the same `x1`, our delta did **not** land (it would have left
    `x3`), and a third party moved the world to `x2`. Same `fp0`, same `fpNow`, opposite ground
    truth. (sem: SEM-lean-170) -/
def sThird : Scenario :=
  { truth := Authorship.thirdParty, fp0 := x1, fpSelf := x3, fpNow := x2 }

theorem sSelf_faithful : Faithful sSelf := by
  refine ⟨?_, ?_, ?_⟩
  · show x1 ≠ x2
    exact x1_ne_x2
  · intro _
    rfl
  · intro h
    exact absurd h (by decide)

theorem sThird_faithful : Faithful sThird := by
  refine ⟨?_, ?_, ?_⟩
  · show x1 ≠ x2
    exact x1_ne_x2
  · intro h
    exact absurd h (by decide)
  · intro _
    show x2 ≠ x3
    exact x2_ne_x3

/-- The collision at the heart of `req/451` §1-4: two worlds with opposite authorship, both
    faithful, both reading `(fp0, fpNow) = (x1, x2)`. (sem: SEM-lean-171) -/
theorem today_observations_collide : obsToday sSelf = obsToday sThird := rfl

-- The `#guard`-grade witnesses (compile-time, kernel-evaluated on every `lake build`): the two
-- scenarios really are distinguishable *once the record is present*, so §2's collapse below is a
-- statement about the observation map and not about the fixtures being secretly identical.
#guard decide (obsWithRecord sSelf ≠ obsWithRecord sThird)
#guard decide (classifyRec sSelf = AttrVerdict.selfApplied)
#guard decide (classifyRec sThird = AttrVerdict.thirdParty)
#guard decide (classify none x2 = AttrVerdict.undetermined)

/-- **The breakage theorem** (`DR-R34-1`'s reason to exist, machine-checked). In a design whose
    only fingerprint inputs are the planned one and the current one, the two hypotheses `H_self`
    and `H_third` are not separable: `sSelf` and `sThird` differ in ground truth, both satisfy
    `Faithful`, and their observations are **equal** -- hence *every* function of that observation,
    without exception, returns the same verdict for both. This is `req/451` §1-4's "both fall into
    the same bucket" as a universally quantified proposition rather than a reading of the current
    code: adding a seat for a fingerprint to `ApplyStarted` would not help, because the theorem
    quantifies over the classifier, not over one implementation of it. What escapes the quantifier
    is a *different observation map* -- which is §3, and which is exactly what `DR-R34-1` files.
    (sem: SEM-lean-172) -/
theorem applyFingerprint_absence_counterexample :
    sSelf.truth ≠ sThird.truth ∧
      Faithful sSelf ∧
      Faithful sThird ∧
      obsToday sSelf = obsToday sThird ∧
      ∀ c : ObjectSnapshot × ObjectSnapshot → AttrVerdict,
        c (obsToday sSelf) = c (obsToday sThird) := by
  refine ⟨by decide, sSelf_faithful, sThird_faithful, today_observations_collide, ?_⟩
  intro c
  rw [today_observations_collide]

/-- The same fact in the form that names the harm: no classifier of the pre-`DR-R34-1` observation
    is sound, because soundness on these two faithful scenarios requires two different answers to
    one input. (sem: SEM-lean-173) -/
theorem today_no_sound_classifier :
    ¬ ∃ c : ObjectSnapshot × ObjectSnapshot → AttrVerdict,
        c (obsToday sSelf) = AttrVerdict.selfApplied ∧
          c (obsToday sThird) = AttrVerdict.thirdParty := by
  rintro ⟨c, h1, h2⟩
  rw [today_observations_collide] at h1
  exact absurd (h1.symm.trans h2) (by decide)

-- =============================================================================================
-- § 3. The recovery theorems (C5's positive control): with the record, the question is decidable
-- soundly -- and the residual window lands fail-closed.
-- =============================================================================================

/-- Soundness on the `H_self` side, for **every** faithful scenario -- not only the fixture. The
    hypothesis used is exactly `Faithful`'s second clause, so nothing is smuggled in.
    (sem: SEM-lean-174) -/
theorem classify_sound_self (s : Scenario) (hF : Faithful s)
    (ht : s.truth = Authorship.selfApplied) :
    classifyRec s = AttrVerdict.selfApplied := by
  have h : s.fpNow = s.fpSelf := hF.2.1 ht
  show (if s.fpNow = s.fpSelf then AttrVerdict.selfApplied else AttrVerdict.thirdParty)
      = AttrVerdict.selfApplied
  exact if_pos h

/-- Soundness on the `H_third` side, for every faithful scenario. The hypothesis used is
    `Faithful`'s third clause -- i.e. denominator 1's no-collision assumption, and it is used here
    and nowhere else, which is where a reader should look to judge how much the recovery costs.
    (sem: SEM-lean-175) -/
theorem classify_sound_third (s : Scenario) (hF : Faithful s)
    (ht : s.truth = Authorship.thirdParty) :
    classifyRec s = AttrVerdict.thirdParty := by
  have h : s.fpNow ≠ s.fpSelf := hF.2.2 ht
  show (if s.fpNow = s.fpSelf then AttrVerdict.selfApplied else AttrVerdict.thirdParty)
      = AttrVerdict.thirdParty
  exact if_neg h

/-- Completeness in the shape that matters operationally: a faithful scenario whose record survived
    is classified back to its own ground truth. Read together with §2, this is the whole content of
    `DR-R34-1` -- the *same* two worlds, the *same* faithfulness, and the verdict flips from
    "provably indistinguishable" to "provably recovered" on the strength of one recorded value.
    (sem: SEM-lean-176) -/
theorem classify_recovers_truth (s : Scenario) (hF : Faithful s) :
    (s.truth = Authorship.selfApplied ∧ classifyRec s = AttrVerdict.selfApplied) ∨
      (s.truth = Authorship.thirdParty ∧ classifyRec s = AttrVerdict.thirdParty) := by
  have hsplit : s.truth = Authorship.selfApplied ∨ s.truth = Authorship.thirdParty := by
    cases s.truth with
    | selfApplied => exact Or.inl rfl
    | thirdParty => exact Or.inr rfl
  rcases hsplit with ht | ht
  · exact Or.inl ⟨ht, classify_sound_self s hF ht⟩
  · exact Or.inr ⟨ht, classify_sound_third s hF ht⟩

/-- C5's positive control on §2's own fixtures, unchanged: the very pair whose observations collide
    is separated the moment the post-apply fingerprint is in the observation. Nothing else about the
    two scenarios moved. (sem: SEM-lean-177) -/
theorem record_separates_the_fixtures :
    classifyRec sSelf = AttrVerdict.selfApplied ∧
      classifyRec sThird = AttrVerdict.thirdParty :=
  ⟨classify_sound_self sSelf sSelf_faithful rfl,
   classify_sound_third sThird sThird_faithful rfl⟩

/-- The residual crash window (`req/38` §257 ruling 4, and `drr341_materials.md` §4-1's definition
    of it: apply has returned, the record is not yet durable). Its verdict is `undetermined`.
    (sem: SEM-lean-178) -/
theorem window_is_undetermined (fpNow : ObjectSnapshot) :
    classify none fpNow = AttrVerdict.undetermined := rfl

/-- Fail-closed, stated as the property rather than asserted: in the window, neither affirmative
    verdict is reachable. A window that could return `selfApplied` would be worse than the current
    silence, because it would be a *confident* wrong answer. (sem: SEM-lean-179) -/
theorem window_never_affirms (fpNow : ObjectSnapshot) :
    classify none fpNow ≠ AttrVerdict.selfApplied ∧
      classify none fpNow ≠ AttrVerdict.thirdParty := by
  rw [window_is_undetermined]
  exact ⟨by decide, by decide⟩

/-- The window leaks nothing either: its verdict is independent of the world. Together with
    `window_never_affirms` this pins the residual window to a constant -- it is a refusal, not a
    guess, and not a side channel. (sem: SEM-lean-180) -/
theorem window_verdict_carries_no_information (fpNow fpNow' : ObjectSnapshot) :
    classify none fpNow = classify none fpNow' := rfl

/-- The fail-closed bucket is *exact*: `undetermined` is returned if and only if the record is
    missing. One direction says the classifier never hides a decidable case behind a refusal (no
    over-refusal); the other says it never affirms without a record (no under-refusal). Neither
    direction is free -- a classifier that returned `undetermined` on `fpNow = fp0`, say, would fail
    the first. (sem: SEM-lean-181) -/
theorem undetermined_iff_no_record (fpPost : Option ObjectSnapshot) (fpNow : ObjectSnapshot) :
    classify fpPost fpNow = AttrVerdict.undetermined ↔ fpPost = none := by
  cases fpPost with
  | none => exact ⟨fun _ => rfl, fun _ => rfl⟩
  | some fp =>
      refine ⟨?_, fun h => absurd h (Option.some_ne_none fp)⟩
      intro h
      have hred : classify (some fp) fpNow
          = if fpNow = fp then AttrVerdict.selfApplied else AttrVerdict.thirdParty := rfl
      rw [hred] at h
      by_cases hx : fpNow = fp
      · rw [if_pos hx] at h
        exact absurd h (by decide)
      · rw [if_neg hx] at h
        exact absurd h (by decide)

/-- Totality: every input receives one of the three verdicts. Trivial in Lean (the function is
    total by construction), and stated anyway because the Rust-side counterpart is a `match` whose
    exhaustiveness is the subject of §4 -- the place where totality stops being free.
    (sem: SEM-lean-182) -/
theorem classify_total (fpPost : Option ObjectSnapshot) (fpNow : ObjectSnapshot) :
    classify fpPost fpNow = AttrVerdict.selfApplied ∨
      classify fpPost fpNow = AttrVerdict.thirdParty ∨
      classify fpPost fpNow = AttrVerdict.undetermined := by
  cases fpPost with
  | none => exact Or.inr (Or.inr rfl)
  | some fp =>
      by_cases hx : fpNow = fp
      · exact Or.inl (if_pos hx)
      · exact Or.inr (Or.inl (if_neg hx))

-- =============================================================================================
-- § 4. The catch-all arm: why a new record kind is the moment a silent default becomes a defect.
-- =============================================================================================

/-- Journal framing generations. Three constructors, the smallest vocabulary in which both §4's and
    §5's defects are expressible (denominator 3). (sem: SEM-lean-183) -/
inductive JournalFormat where
  /-- No framing marker. -/
  | legacy
  /-- Chain-linked framing. -/
  | chained
  /-- Chain-linked framing able to carry the vocabulary a later generation added. -/
  | chainedV2
  deriving DecidableEq, Repr

/-- Record kinds. Three, by the same denominator: one that predates every framing change, one
    standing for the precedent in which a *new value inside an existing kind* forced a framing bump,
    and one standing for `DR-R34-1`'s **new kind**. (sem: SEM-lean-184) -/
inductive RecordKind where
  | planned
  | abortedDiverged
  | applyCompleted
  deriving DecidableEq, Repr

/-- The rank function as it stands: two of the three formats fold to the same number.
    (sem: SEM-lean-185) -/
def rankFolded : JournalFormat → Nat
  | JournalFormat.legacy => 1
  | JournalFormat.chained => 1
  | JournalFormat.chainedV2 => 2

/-- The un-folded alternative, used only as §5's positive control. (sem: SEM-lean-186) -/
def rankSplit : JournalFormat → Nat
  | JournalFormat.legacy => 1
  | JournalFormat.chained => 2
  | JournalFormat.chainedV2 => 3

/-- The exhaustive classifier: one arm per kind, no catch-all. Adding a fourth constructor to
    `RecordKind` would make this definition fail to compile until an arm is written -- which is the
    entire property §4 is about. (sem: SEM-lean-187) -/
def minFormatExhaustive : RecordKind → JournalFormat
  | RecordKind.planned => JournalFormat.legacy
  | RecordKind.abortedDiverged => JournalFormat.chainedV2
  | RecordKind.applyCompleted => JournalFormat.chainedV2

/-- The defaulted classifier: the one declared non-legacy arm, and a catch-all that sweeps
    everything else to `legacy`. A kind whose arm is never written lands here **silently** -- the
    compiler asks nothing. (sem: SEM-lean-188) -/
def minFormatDefaulted : RecordKind → JournalFormat
  | RecordKind.abortedDiverged => JournalFormat.chainedV2
  | _ => JournalFormat.legacy

/-- The append gate: a record is accepted into a journal when the format it needs ranks no higher
    than the journal's own. (sem: SEM-lean-189) -/
def acceptsBy (rk : JournalFormat → Nat) (jf : JournalFormat)
    (mf : RecordKind → JournalFormat) (k : RecordKind) : Bool :=
  decide (rk (mf k) ≤ rk jf)

/-- The gate under the rank function as it stands. (sem: SEM-lean-190) -/
def accepts : JournalFormat → (RecordKind → JournalFormat) → RecordKind → Bool :=
  acceptsBy rankFolded

#guard accepts JournalFormat.legacy minFormatDefaulted RecordKind.applyCompleted
#guard !(accepts JournalFormat.legacy minFormatExhaustive RecordKind.applyCompleted)
#guard accepts JournalFormat.chainedV2 minFormatExhaustive RecordKind.applyCompleted

/-- **The catch-all counterexample.** The first conjunct is the reason the defect is invisible until
    it bites: on every kind that existed when the catch-all was written, the defaulted classifier
    and the exhaustive one are *the same function*, so no test, review or audit of the vocabulary as
    it stood could tell them apart. The second and third conjuncts are the bite: on the new kind
    they disagree, and the disagreement is in the dangerous direction -- the defaulted classifier
    lets a record that needs the newer framing be appended to a legacy journal, while the exhaustive
    one refuses it. This is a statement about two functions in this file; the reading that it is
    also the shape of a real hazard belongs to `drr341_materials.md` §3-2 and stays in prose (C2).
    (sem: SEM-lean-191) -/
theorem defaultedArm_counterexample :
    (∀ k : RecordKind, k ≠ RecordKind.applyCompleted →
        minFormatDefaulted k = minFormatExhaustive k) ∧
      accepts JournalFormat.legacy minFormatDefaulted RecordKind.applyCompleted = true ∧
      accepts JournalFormat.legacy minFormatExhaustive RecordKind.applyCompleted = false := by
  refine ⟨?_, rfl, rfl⟩
  intro k hk
  cases k with
  | planned => rfl
  | abortedDiverged => rfl
  | applyCompleted => exact absurd rfl hk

/-- C5's control for §4: with the arm written out, the new kind is refused by both v1 generations
    and accepted by the generation that can carry it. The refusal is per-append -- the journal file
    is not damaged -- which is the shape the precedent established. (sem: SEM-lean-192) -/
theorem exhaustiveArm_recovers :
    accepts JournalFormat.legacy minFormatExhaustive RecordKind.applyCompleted = false ∧
      accepts JournalFormat.chained minFormatExhaustive RecordKind.applyCompleted = false ∧
      accepts JournalFormat.chainedV2 minFormatExhaustive RecordKind.applyCompleted = true :=
  ⟨rfl, rfl, rfl⟩

-- =============================================================================================
-- § 5. The rank fold: a second, independent defect that the new kind neither causes nor repairs.
-- =============================================================================================

/-- The fold, and its consequence: `legacy` and `chained` rank equally, so the gate cannot separate
    them **at all** -- for no classifier and no kind does the answer differ between the two
    journals. A gate whose two inputs are provably interchangeable is not distinguishing the
    generations it names. (sem: SEM-lean-193) -/
theorem rankFold_conflates :
    rankFolded JournalFormat.legacy = rankFolded JournalFormat.chained ∧
      ∀ (mf : RecordKind → JournalFormat) (k : RecordKind),
        accepts JournalFormat.legacy mf k = accepts JournalFormat.chained mf k :=
  ⟨rfl, fun _ _ => rfl⟩

/-- The harm, stated on the hypothesis that a kind requiring exactly the `chained` framing exists.
    It does not exist today (denominator 3), which is why this is a *structural* gap and not a live
    failure -- and why the hypothesis is written out instead of a fixture being invented to hide it.
    Under the fold, such a record is admitted into a marker-less journal. (sem: SEM-lean-194) -/
theorem rankFold_admits_chainedOnly_into_legacy
    (mf : RecordKind → JournalFormat) (k : RecordKind) (h : mf k = JournalFormat.chained) :
    accepts JournalFormat.legacy mf k = true := by
  show decide (rankFolded (mf k) ≤ rankFolded JournalFormat.legacy) = true
  rw [h]
  rfl

/-- C5's control for §5, on the same hypothesis: un-fold the rank and the refusal appears exactly
    where it should -- refused by a marker-less journal, accepted by a chain-linked one. So the gap
    is the fold's and nothing else's; the surrounding gate machinery is sound. Note what this does
    **not** say: that un-folding is the right repair, or that it is free. `DR-R34-1` neither
    deepens this gap nor closes it, and the pairing here is what licenses that sentence.
    (sem: SEM-lean-195) -/
theorem rankSplit_separates_chainedOnly
    (mf : RecordKind → JournalFormat) (k : RecordKind) (h : mf k = JournalFormat.chained) :
    acceptsBy rankSplit JournalFormat.legacy mf k = false ∧
      acceptsBy rankSplit JournalFormat.chained mf k = true := by
  refine ⟨?_, ?_⟩
  · show decide (rankSplit (mf k) ≤ rankSplit JournalFormat.legacy) = false
    rw [h]
    rfl
  · show decide (rankSplit (mf k) ≤ rankSplit JournalFormat.chained) = true
    rw [h]
    rfl

/-- The new kind is orthogonal to §5's gap: under both rank functions it is refused by both v1
    generations and accepted by the newest one, so `DR-R34-1` is not the thing that has to carry
    the fold's repair. (sem: SEM-lean-196) -/
theorem newKind_orthogonal_to_the_fold :
    acceptsBy rankSplit JournalFormat.legacy minFormatExhaustive RecordKind.applyCompleted = false ∧
      acceptsBy rankSplit JournalFormat.chained minFormatExhaustive RecordKind.applyCompleted
        = false ∧
      acceptsBy rankSplit JournalFormat.chainedV2 minFormatExhaustive RecordKind.applyCompleted
        = true :=
  ⟨rfl, rfl, rfl⟩

-- =============================================================================================
-- § 6. The bridge to the frozen observation surface: what `EffectAlgebra`'s laws are stated on is
-- exactly what the missing record is.
-- =============================================================================================

/-- Reconstruct, from a journal, the pre/post observation pair that `EffectAlgebra.obs` names. The
    pre half is `Planned.fp0`; the post half is the value `DR-R34-1` proposes to record. With no
    record there is no pair. (sem: SEM-lean-197) -/
def reconstructObs (fp0 : ObjectSnapshot) :
    Option ObjectSnapshot → Option (ObjectSnapshot × ObjectSnapshot)
  | none => none
  | some fpPost => some (fp0, fpPost)

/-- With the record, the reconstruction is the frozen observation itself -- definitionally, not by
    analogy. `EffectAlgebra.lean`'s denominator 3 says of the journal-dependent part of its subject
    that "modelling them needs the journal"; this equation is the smallest instance of that
    dependency being met. (sem: SEM-lean-198) -/
theorem obs_reconstructible_with_record (f : Transformation) :
    reconstructObs f.src (some f.dst) = some (obs f) := rfl

/-- Without the record, the reconstruction is a constant: it maps every transformation to the same
    answer, exactly as `obsToday` maps `sSelf` and `sThird` to the same answer. The two collapses in
    this file are the same collapse seen at two levels. (sem: SEM-lean-199) -/
theorem obs_unreconstructible_without_record (f g : Transformation) :
    reconstructObs f.src none = reconstructObs g.src none := rfl

/-- C5's control for §6: with the record present, the reconstruction **decides** `ObsEq` -- the
    relation every law in `EffectAlgebra` is stated modulo. So the post-apply fingerprint is not
    merely one more field: it is the half of the frozen observation surface that a journal without
    it cannot supply, and recording it is what makes that surface checkable against a journal at
    all. (sem: SEM-lean-200) -/
theorem reconstruction_decides_ObsEq (f g : Transformation) :
    reconstructObs f.src (some f.dst) = reconstructObs g.src (some g.dst) ↔ ObsEq f g := by
  constructor
  · intro h
    exact Option.some.inj h
  · intro h
    show some (f.src, f.dst) = some (g.src, g.dst)
    exact congrArg some h

end GxSpec.Attribution
