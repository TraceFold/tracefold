import GxSpec.Core
import GxSpec.Admissible
import GxSpec.Receipt
import GxSpec.Minimality

/-!
# GxSpec.EffectAlgebra — the effect algebra: partial inverses (inverse-semigroup laws) and the
escrow lens

Identity: `req/38` §125 ruling 1 **A1** (the first stage of the adjudicated foundation A' of
`DR-46-6`, `46-verification-plan.md` §8 line 394), consuming `req/186` (the material lane:
inverse semigroups via Jacobson 2009's Darcs patch theory, lens laws via Foster et al. TOPLAS
2007 / Diskin et al. JOT 2011, Sagas 1987 as the *non*-model) and `req/188` §1 **T1** (the ground
contract: the smallest unit of the semantics is transformation (effect) + escrow (pre-emptive
stash) + inverse (partial inverse) + receipt; theory line = effect algebra + lens laws + Merkle
ledger + linearizable log).

`req/188` §2 lists **U7** (the axiom system of the effect algebra) among the unproved theory debts,
and `req/186` §4's mapping table onto the running system marks two rows "spec yes / implementation
yes / **theorem no**" that this file addresses: *partial inverse* (`gx-adapter-mcp`
`catalogue.rs`/`invert.rs`, `InverseStatus` in `gx-engine/store.rs`) and *lens laws* (`42` §3.10
`postcondition_fingerprint`). This file moves those two rows from "theorem no" to "theorem yes, on
the observation surface, for the declared part" -- no further.

## The surface this file works on, and why it is not the frozen types themselves

The frozen `Transformation` (`Core.lean`) carries `id`, `order`, `src`, `dst`. The three
inverse-semigroup laws **cannot** hold on the nose for that record: `compose` mints the composite's
id through the `composeId` axiom, which is opaque, so `composeId (composeId a b) a = a` is neither
provable nor refutable, and `order` is `max`-accumulated. This is not an accident of the
formalisation — it is the honest content of `req/186` §3's own warning: a real substrate satisfies
`aa⁻¹a = a` **only on the digest** (GitHub leaves the undo commit behind, Notion does a trash round
trip), so unless the theorem's subject is restricted to *the observation* the claim is an
overclaim. The laws below are therefore stated on `ObsEq` — equality of the pair
`(src, dst)`, i.e. of exactly what a receipt records (`42` §3.10: the pre/post fingerprints) — and
never on identity or provenance metadata. Undo restores *the observed content*; it does not erase
history, and this file does not claim it does.

## Discipline (the five conditions of `req/160` §3-1 / `req/38` §98 ruling 3, restated for this file)

`Injection.lean` established the house form for building on top of the frozen five modules. This
file is an *addition of vocabulary* rather than an injection of structure, so the conditions are
discharged as follows and the analogy is declared rather than assumed:

* **C1 (conservative extension)**: the frozen modules are used by `import` only — no frozen file is
  edited, no frozen definition shadowed, every frozen theorem stands unchanged. The axiom set stays
  `{propext, Quot.sound, GxSpec.composeId}` (`#print axioms` raw in `req/203` §5); the machine check
  is `lake build` of the whole root. `GxSpec.Minimality` is imported for its fixtures
  (`x1`/`x2`/`x1_ne_x2`) so no fresh content corpus is invented — the same precedent
  `MinimalityF0.lean` set and defended (`req/191` §5 attack 3).
* **C2 (vocabulary roles declared)**: `PartialInverse`/`ObsEq`/`EscrowState` appear in exactly two
  roles — the *law* side (a property of a catalogue that satisfies the escrow discipline) and the
  *counterexample* side (a catalogue with one undeclared tool). No theorem here asserts that gx's
  running implementation satisfies anything; the subject is always the model.
* **C3 (explicit subject)**: the subject of §2's laws is *a partial-inverse catalogue that keeps the
  escrow discipline* — a `PartialInverse` that is *reverse-typed wherever it is defined*. Nothing is
  claimed about catalogues in general, and no impossibility claim is made about any design.
* **C4 (minimal addition)**: the additions are exactly what contract T1 legislates about — one
  partial map `Transformation → InvertOutcome` (mirroring `SubstrateAdapter::invert`'s own
  signature, whose `inverse` component is the `Option` this file's laws quantify over), one
  observation projection, and one two-component escrow state. No cbor/hash/timestamp machinery is
  reconstructed (`46` §1's non-goals stand).

  🔴 **DR-46-26 (`req/38` §258, E-DR4626-1) rewrote this clause, and the rewrite is the point.**
  Until that erratum the clause read "one partial map `Transformation → Option Transformation` ...
  whose `None` is `InverseStatus::Unavailable`, `gx-engine/src/store.rs:626`", and that sentence
  became **false** the moment the implementation gained a writer for
  `InverseStatus::Undetermined`: `none` is now the image of *two* engine states, "no inverse
  exists" (`Reversibility::false`) and "nobody found out" (`Reversibility::unknown`). The model
  follows the implementation rather than the other way round, so `PartialInverse` answers an
  `InvertOutcome` and the laws take the `inverse` **projection** of it — design fork **L-1**,
  adopted over L-2 (collapsing the `Option` into a three-valued sum) for the reason §5's
  counterexample gives: what that counterexample asserts is that *partiality is essential and
  infects a chain*, and a three-valued conclusion would make it assert two things at once. The
  `verdict` component is therefore **carried and never used by a law** — see denominator 6.
* **C5 (positive control paired with every breakage)**: §5's counterexample is paired with
  `fixture_declared_recovery` on the *same fixtures* — the declared half of the very catalogue whose
  undeclared half breaks. §4's lens counterexample is likewise paired with §3's proved laws, and the
  only moving part between them is the no-op guard.

## What is proved here, precisely (P-9 — no overclaim)

Proved: (a) on observations, an escrow-disciplined partial inverse satisfies the three
inverse-semigroup laws — `x·x⁻¹·x ≈ x`, `x⁻¹·x·x⁻¹ ≈ x⁻¹`, idempotents commute — plus uniqueness of
the inverse up to observation and the anti-homomorphism `(f·g)⁻¹ ≈ g⁻¹·f⁻¹`; (b) the escrow state
with its no-op guard is a well-behaved lens (GetPut and PutGet), and dropping the guard keeps PutGet
but breaks GetPut; (c) the two are linked: on a no-op effect the lens's GetPut and the inverse's
observation coincide, and apply-then-undo returns the observation to the escrowed value; (d) a
catalogue with one undeclared tool satisfies the escrow discipline yet is not totally invertible,
and — the point — one undeclared step **removes the whole chain from the domain of the laws**.

Not proved, and not claimable from anything below: that any concrete substrate satisfies these laws
(GitHub/Notion/e-mail do not — `req/186` §3); that undo restores history rather than observed
content; that the *declared* part of a real catalogue is non-empty (that is an implementation fact
measured by `gx-adapter-mcp`'s tests, not a theorem); that the laws hold on identity or order
metadata (they demonstrably do not, see above); anything about durability, crash or concurrency
(`46` §1 non-goals, `req/188` §2 U3, `DR-46-6` stage A3). The word "algebra" here names a set of
laws on a partial operation, not a completeness result: **no claim is made that these are the only
laws, nor that they axiomatise the effect algebra**. `req/186` §2's A2 (ledger prefix monotonicity)
and A3 (crash-prefix recovery) are separate stages and are **not** delivered here.

## Denominator, stated once here rather than left to be noticed by absence

1. **Observation only.** Every law is modulo `ObsEq`. The `id`/`order` components are not equated
   and provably cannot be (`composeId` is opaque). A reader wanting `aa⁻¹a = a` on the nose will not
   find it, and it is not true of gx.
2. **Declared part only.** `Recoverable` is proved for elements the catalogue declares. Its domain
   is the catalogue (`req/186` §4: the domain *is* the catalogue -- partiality is of the essence),
   and §5 shows the domain is
   *not* closed under composition for the counterexample catalogue. Whether some *other* catalogue
   has a composition-closed domain is neither proved nor refuted here (`ComposeClosed` is defined
   so the question has a name, and left open on purpose).
3. **Two of `InverseStatus`'s seven values are modelled.** `some`/`none` correspond to
   `Available`/`Unavailable`. `Consumed { by }` (one-shot escrow), `Expired` (retention),
   `Pending` (two-phase escrow, `req/38` §98 ruling 1), `BodyMissing` (R8 / `req/234` B-5) and
   `Undetermined` (DR-46-13, given a writer by DR-46-26) have no counterpart below: they are
   lifecycle states of the escrow row, not of the mathematical inverse, and modelling them needs the
   journal (stage A3).

   🔴 **The number was wrong before this lane, and is corrected here rather than quietly.** This
   denominator said *five* from the day it was written; the vocabulary reached six at R8 and seven
   in D24's erratum batch (`InverseStatus::ALL_KINDS.len() == 7`,
   `gx-engine/tests/lifecycle_transitions.rs`). The fraction modelled did not change — it was
   `2/5` on paper and `2/7` in fact — so nothing below moves, but a denominator that nobody
   recomputed is exactly the kind of number this section exists to stop. Corrected in DR-46-26's
   window because that is the lane that touches this type at all, rather than in a window of its
   own (`req/452` §5-3).
4. **No commutation.** `req/186` §4's `Commutation` row (F1, `ASM-2`, DPO independence) is untouched;
   `ObsIdempotent`-commutation below is the inverse-semigroup axiom about *idempotents*, not the
   independence of two arbitrary effects.
5. **`escrowInverse` mints through a caller-supplied function**, not through `composeId`. The model
   therefore says nothing about how a real inverse delta's CID is derived.
6. 🔴 **The verdict is recorded and the laws do not use it** (DR-46-26, fork L-1). `InvertOutcome`
   carries C-25's three values so that the model's `PartialInverse` is the implementation's
   signature rather than half of it, and **every theorem in §2–§6 quantifies over the `inverse`
   projection alone**. Nothing below is a claim about when an implementation may answer `unknown`
   rather than `false`; that is a property of a deployment's read posture
   (`OnReadFailure`, `gx-adapter-mcp`), and a law about it would be a law about a configuration.

## Theorem map (name = coordinate)

| law / claim | theorem |
|---|---|
| `x·x⁻¹·x ≈ x` (declared elements are recoverable) | `law1_declared_recoverable` |
| `x⁻¹·x·x⁻¹ ≈ x⁻¹` | `law2_inverse_recovers_itself` |
| idempotents commute | `law3_obsIdempotents_commute` |
| `x·x⁻¹` is idempotent (escrow round trip) | `escrowRoundTrip_obsIdempotent` |
| the inverse is unique up to observation | `inverse_unique_up_to_observation` |
| `(f·g)⁻¹ ≈ g⁻¹·f⁻¹` | `inverse_antihomomorphism` |
| the escrow discipline is satisfiable (non-vacuity) | `totalCatalogue_reverses` |
| lens GetPut / PutGet | `lens_getPut` / `lens_putGet` |
| PutGet = the receipt's postcondition fingerprint | `putGet_is_postcondition_observation` |
| unguarded escrow keeps PutGet but breaks GetPut | `commitAlways_putGet_survives` / `commitAlways_getPut_counterexample` |
| lens ↔ inverse: escrow holds what the inverse restores | `escrow_holds_the_inverse_target` |
| lens ↔ inverse: apply-then-undo restores the observation | `apply_then_undo_recovers_observation` |
| lens ↔ inverse: on a no-op the two laws coincide | `noop_effect_lens_and_inverse_agree` |
| **minimality**: an undeclared tool breaks the laws for the whole chain | `undeclaredInverse_counterexample` |
| positive control on the same fixtures | `fixture_declared_recovery` |
-/

namespace GxSpec.EffectAlgebra

open GxSpec
open GxSpec.Minimality (x1 x2 x1_ne_x2)

-- =============================================================================================
-- § 0. The observation surface — what a receipt records, and the only surface the laws speak on.
-- =============================================================================================

/-- The observation of a transformation: the pre/post pair. This is exactly what `42` §3.10 records
in a receipt (`precondition_fingerprint`/`postcondition_fingerprint`) and exactly what an undo can
restore. `id` and `order` are deliberately absent: they are provenance, and undo does not rewrite
provenance (`11`/`12` P-5: an undo is a *new* commit). -/
def obs (f : Transformation) : ObjectSnapshot × ObjectSnapshot := (f.src, f.dst)

/-- Observational equality — the equivalence the laws of §2 are stated modulo. Writing `≈` for it
would invite reading it as equality of transformations; the long name is deliberate. -/
def ObsEq (f g : Transformation) : Prop := obs f = obs g

theorem obsEq_refl (f : Transformation) : ObsEq f f := rfl

theorem obsEq_symm {f g : Transformation} (h : ObsEq f g) : ObsEq g f := h.symm

theorem obsEq_trans {f g k : Transformation} (h₁ : ObsEq f g) (h₂ : ObsEq g k) : ObsEq f k :=
  h₁.trans h₂

/-- An effect that is observationally a loop. This is the inverse-semigroup notion of *idempotent*
read on observations: `e` composed with itself observes as `e` again. Named `ObsIdempotent` rather
than `Idempotent` to keep it distinct from `GxSpec.Idempotent` in `Canon.lean`, which is about the
canonicalizer's round trip and is an unrelated notion. -/
def ObsIdempotent (e : Transformation) : Prop := e.src = e.dst

-- =============================================================================================
-- § 1. The partial inverse (the catalogue) and the escrow discipline.
-- =============================================================================================

/-- 🔴 **C-25's three values** (`11` §5-2), as the implementation carries them
(`gx_core::Reversibility`, relocated there by DR-46-26). Modelled so that `PartialInverse` below is
the *whole* of what `SubstrateAdapter::invert` answers; **no law in this file consults it**
(denominator 6). -/
inductive Reversibility : Type where
  /-- An inverse was constructed. -/
  | true
  /-- No inverse exists for this call. -/
  | false
  /-- The prior could not be read, so whether an inverse exists was never established. -/
  | unknown
  deriving DecidableEq, Repr

/-- 🔴 What `SubstrateAdapter::invert` answers after **E-DR4626-1** (`req/38` §258): the inverse,
and C-25's verdict beside it.

The `Option` is kept **inside** rather than collapsed into the verdict, and the reason is the same
one the implementation gives: `inverse = some _` and `verdict = true` are one fact, and the laws of
§2 are about the first of them. Collapsing would make §5's counterexample assert two things — that
partiality is essential *and* which of `false`/`unknown` a refusal is — where it asserts one. -/
structure InvertOutcome : Type where
  /-- The escrowed inverse, when one was constructed. -/
  inverse : Option Transformation
  /-- C-25's answer for this call. Carried; never used by a law. -/
  verdict : Reversibility

/-- A partial inverse = what a substrate adapter's `invert` is: `Transformation → InvertOutcome`.
The `Option` inside is not a modelling convenience — it is the `inverse` component of
`SubstrateAdapter::invert`'s own signature, and `gx-adapter-mcp/src/invert.rs` lists "No restore
tool is declared" as the first of its three legitimate reasons for answering with none. Partiality
is the subject matter, not an edge case.

🔴 **DR-46-26 (fork L-1)**: this was `Transformation → Option Transformation` until E-DR4626-1
widened the signature it mirrors. Every law below reaches through `.inverse`, which is what makes
the change a *projection* rather than a re-statement: the six laws' proofs are unchanged in content
because they were only ever driven by `Reverses`. -/
abbrev PartialInverse : Type := Transformation → InvertOutcome

/-- `f` is in the catalogue's domain: an inverse was constructed and escrowed for it. -/
def Declared (inv : PartialInverse) (f : Transformation) : Prop := ∃ f', (inv f).inverse = some f'

/-- The escrow discipline: **wherever the catalogue answers, its answer reverses the observation**.
This is `43` T-10b/E-M4-30's escrow-before-commit read as a typing rule — the escrowed delta takes
the post-state back to the pre-state that was stashed before the effect fired. Everything in §2 is
derived from this one condition; nothing else about the catalogue is assumed. -/
def Reverses (inv : PartialInverse) : Prop :=
  ∀ f f', (inv f).inverse = some f' → f'.src = f.dst ∧ f'.dst = f.src

/-- Whether the declared domain is closed under composition. Deliberately *defined but not proved*:
§5 exhibits a catalogue whose domain is not closed (one undeclared step in a chain), and whether
some other catalogue closes it is left open — naming the property is what lets the open question be
pointed at instead of glossed. Compare `Admissible`'s second conjunct (`Admissible.lean`), which is
the same shape for the gate and *is* assumed there. -/
def ComposeClosed (inv : PartialInverse) : Prop :=
  ∀ f g (h : composable f g), Declared inv f → Declared inv g → Declared inv (compose f g h)

/-- The group-like reading — every effect has an inverse. §5 refutes it for the counterexample
catalogue; `req/186` §1-1's reading of Sagas 1987 is why no real catalogue is expected to satisfy it
(compensation is semantic undo, not a group inverse). -/
def TotallyInvertible (inv : PartialInverse) : Prop := ∀ f, Declared inv f

/-- The canonical escrowed inverse of `f`: the same order, observation reversed, id minted by a
caller-supplied function (the model says nothing about how a real inverse delta's CID is derived —
denominator 5). -/
def escrowInverse (mint : Transformation → TransformationId) (f : Transformation) : Transformation :=
  { id := mint f, order := f.order, src := f.dst, dst := f.src }

/-- The total catalogue: every tool declares a restore template. Non-vacuity witness for `Reverses`
in the sense `Admissible.lean`'s `orderBounded_admissible` and `Receipt.lean`'s
`recoverableChain_witness` are witnesses for their hypotheses — the escrow discipline of §1 is
satisfiable outright, so §2's laws are not vacuously about the empty catalogue. (That a *real*
catalogue is total is exactly what §5 denies.) -/
def totalCatalogue (mint : Transformation → TransformationId) : PartialInverse :=
  fun f => { inverse := some (escrowInverse mint f), verdict := Reversibility.true }

theorem totalCatalogue_reverses (mint : Transformation → TransformationId) :
    Reverses (totalCatalogue mint) := by
  intro f f' h
  have h' : escrowInverse mint f = f' := by
    injection h
  exact ⟨h' ▸ rfl, h' ▸ rfl⟩

theorem totalCatalogue_totallyInvertible (mint : Transformation → TransformationId) :
    TotallyInvertible (totalCatalogue mint) := fun f => ⟨escrowInverse mint f, rfl⟩

-- =============================================================================================
-- § 2. The three inverse-semigroup laws, on observations (Jacobson 2009's axioms for Darcs patch
-- theory, `req/186` §1-7: each element `a` has a unique quasi-inverse `a⁻¹`, `aa⁻¹a = a`, and
-- idempotents commute).
-- =============================================================================================

/-- Law 1 in bundled form: `f` composed with its escrowed inverse and then with `f` again observes
as `f`. Bundling the two composability side conditions into the statement is not bookkeeping — it is
half the content: the escrow discipline **guarantees the gluing succeeds** (`composable`, the F0
side condition `MinimalityF0.lean` shows `src`/`dst` exist to carry), so an undo can always be
sequenced against the effect it undoes. -/
def Recoverable (inv : PartialInverse) (f : Transformation) : Prop :=
  ∃ f', (inv f).inverse = some f' ∧ ∃ (h₁ : composable f f') (h₂ : composable (compose f f' h₁) f),
    ObsEq (compose (compose f f' h₁) f h₂) f

/-- **Law 1** (`x·x⁻¹·x = x`, on observations): every element the catalogue declares is recoverable.
Why this is the load-bearing law for gx: it is the formal content of "declare the range you can undo,
and undo only within that range" (`req/188` §9-2) — the *declared* part of the catalogue is
exactly the part where
apply-undo-apply is observationally the original effect. `req/186` §1-7 / Jacobson 2009 (CAM 09-83,
Darcs patch theory in inverse semigroups) is the source of the axiom's shape; `req/186` §5 records
that its text was unreachable (HTTP 403), so the axiom is taken in the standard textbook form quoted
there, not from the primary. -/
theorem law1_declared_recoverable {inv : PartialInverse} (hRev : Reverses inv) (f : Transformation)
    (hd : Declared inv f) : Recoverable inv f := by
  obtain ⟨f', hf⟩ := hd
  have hr := hRev f f' hf
  exact ⟨f', hf, hr.1.symm, hr.2, rfl⟩

/-- **Law 2** (`x⁻¹·x·x⁻¹ = x⁻¹`, on observations): the escrowed inverse is itself recovered by the
round trip through the effect it undoes. Read operationally: re-running an undo after the effect has
been re-applied lands on the same observation as the original undo — an undo is not consumed by
being sequenced (`InverseStatus::Consumed` is a *ledger* rule about one-shot escrow rows, denominator
3, and is a strictly stronger operational restriction than this law). -/
theorem law2_inverse_recovers_itself {inv : PartialInverse} (hRev : Reverses inv)
    {f f' : Transformation} (hf : (inv f).inverse = some f') :
    ∃ (h₁ : composable f' f) (h₂ : composable (compose f' f h₁) f'),
      ObsEq (compose (compose f' f h₁) f' h₂) f' := by
  have hr := hRev f f' hf
  exact ⟨hr.2, hr.1.symm, rfl⟩

/-- `x·x⁻¹` is idempotent — the standard inverse-semigroup fact, and the reason law 3 is *about*
idempotents. Operationally: an apply-then-undo pair is a loop on the observed content, so doing it
twice observes the same as doing it once. -/
theorem escrowRoundTrip_obsIdempotent {inv : PartialInverse} (hRev : Reverses inv)
    {f f' : Transformation} (hf : (inv f).inverse = some f') (h₁ : composable f f') :
    ObsIdempotent (compose f f' h₁) :=
  (hRev f f' hf).2.symm

/-- **Law 3** (idempotents commute): two observational loops that can be sequenced at all observe
the same in either order. This is the axiom that separates inverse semigroups from arbitrary regular
semigroups, and it is what makes the *inverse unique* (next theorem). Note the conclusion also
delivers `composable f e` — the reversed sequencing is always available, never a gluing failure. -/
theorem law3_obsIdempotents_commute {e f : Transformation} (he : ObsIdempotent e)
    (hf : ObsIdempotent f) (h : composable e f) :
    ∃ (h' : composable f e), ObsEq (compose e f h) (compose f e h') := by
  refine ⟨hf.symm.trans (h.symm.trans he.symm), ?_⟩
  show ((e.src, f.dst) : ObjectSnapshot × ObjectSnapshot) = (f.src, e.dst)
  rw [he.trans h, hf.symm.trans h.symm]

/-- Uniqueness of the inverse up to observation — the third law's usual equivalent form. Its gx
reading is concrete and useful: **two adapters that both declare a restore for the same effect are
observationally interchangeable**, so a catalogue change that swaps one `RestoreTemplate` for
another cannot change what an undo restores (it can change ids, order and provenance, which is why
the statement is `ObsEq` and not `Eq` — denominator 1). -/
theorem inverse_unique_up_to_observation {f a b : Transformation}
    (ha : a.src = f.dst ∧ a.dst = f.src) (hb : b.src = f.dst ∧ b.dst = f.src) : ObsEq a b := by
  show ((a.src, a.dst) : ObjectSnapshot × ObjectSnapshot) = (b.src, b.dst)
  rw [ha.1, ha.2, hb.1, hb.2]

/-- Anti-homomorphism `(f·g)⁻¹ = g⁻¹·f⁻¹`, on observations: **when the composite is declared**, its
escrowed inverse observes as the reverse-ordered composite of the constituents' inverses. This is
the law that makes undo of a plan well-defined — you may undo a chain either by holding one escrow
for the whole chain or by replaying the per-step escrows backwards, and the observed result is the
same. The hypothesis `inv (compose f g h) = some c` is exactly what §5's counterexample destroys:
the law is true, and *has no instances* on a chain containing an undeclared tool. -/
theorem inverse_antihomomorphism {inv : PartialInverse} (hRev : Reverses inv)
    {f g fi gi c : Transformation} (hfg : composable f g)
    (hf : (inv f).inverse = some fi) (hg : (inv g).inverse = some gi)
    (hc : (inv (compose f g hfg)).inverse = some c) :
    ∃ (h : composable gi fi), ObsEq c (compose gi fi h) := by
  have hrf := hRev f fi hf
  have hrg := hRev g gi hg
  have hrc := hRev (compose f g hfg) c hc
  have hglue : composable gi fi := by
    show gi.dst = fi.src
    rw [hrg.2, hrf.1]
    exact hfg.symm
  refine ⟨hglue, ?_⟩
  show ((c.src, c.dst) : ObjectSnapshot × ObjectSnapshot) = (gi.src, fi.dst)
  rw [hrc.1, hrc.2, hrg.1, hrf.2]
  rfl

-- =============================================================================================
-- § 3. The escrow lens — the (effect, escrow) pair as a well-behaved bidirectional transformation
-- (Foster et al. TOPLAS 2007's GetPut/PutGet, `req/186` §1-6).
-- =============================================================================================

/-- The state the lens acts on: the current observation of the resource, paired with the escrowed
pre-state. `42` §3.12's `EscrowedInverse` is this pair's implementation (its `inverse_delta` carries
the second component's content); here only the observation is modelled. -/
abbrev EscrowState : Type := ObjectSnapshot × ObjectSnapshot

/-- `get`: the observable half. What a reader (or a receipt's `postcondition_fingerprint`) sees; the
escrow is not part of the view. -/
def observe (s : EscrowState) : ObjectSnapshot := s.1

/-- `put`: commit an effect that moves the observation to `v`, **escrowing the previous observation
first** (escrow-before-commit, `43` T-10b) — *unless the effect moves nothing*, in which case the
escrow is left alone. That guard is the whole design content of this definition and is exactly what
GetPut legislates: a no-op must not rotate the escrow slot, or the last real change would lose its
inverse. §4 removes the guard and shows GetPut break. -/
def commitWithEscrow (s : EscrowState) (v : ObjectSnapshot) : EscrowState :=
  if v = s.1 then s else (v, s.1)

/-- **GetPut** (`put s (get s) = s`): committing the observation the state already has changes
nothing — neither the observation nor the escrow. gx reading: an undo that changes nothing does not
move the world (`req/186` §4's GetPut row), and, sharper, a no-op does not consume the escrow. -/
theorem lens_getPut (s : EscrowState) : commitWithEscrow s (observe s) = s := by
  unfold commitWithEscrow observe
  rw [if_pos rfl]

/-- **PutGet** (`get (put s v) = v`): after committing `v`, the observation *is* `v`. gx reading:
the receipt's postcondition fingerprint is the effect's declared post-state — the property
`settle` pre-flight waits for on an eventually consistent substrate (`req/186` §4's PutGet row;
that waiting is "an improvement, not a guarantee", `req/38` §120 ruling 4, and is not modelled
here). -/
theorem lens_putGet (s : EscrowState) (v : ObjectSnapshot) : observe (commitWithEscrow s v) = v := by
  unfold commitWithEscrow observe
  by_cases h : v = s.1
  · rw [if_pos h]; exact h.symm
  · rw [if_neg h]

/-- PutGet named at its gx coordinate: applying `f` from its own pre-state yields the observation
`f` declared, i.e. the value a receipt records as `postcondition_fingerprint` (`42` §3.10). -/
theorem putGet_is_postcondition_observation (f : Transformation) (esc : ObjectSnapshot) :
    observe (commitWithEscrow (f.src, esc) f.dst) = f.dst :=
  lens_putGet _ _

-- =============================================================================================
-- § 4. Minimality of the no-op guard: drop it and GetPut breaks (PutGet survives, so the failure
-- is attributable to the guard and to nothing else — `Injection.lean`'s one-moving-part form).
-- =============================================================================================

/-- The unguarded commit: escrow the previous observation unconditionally. -/
def commitAlways (s : EscrowState) (v : ObjectSnapshot) : EscrowState := (v, s.1)

/-- PutGet survives the removal — the two `put`s are indistinguishable through `get`. -/
theorem commitAlways_putGet_survives (s : EscrowState) (v : ObjectSnapshot) :
    observe (commitAlways s v) = v := rfl

/-- GetPut breaks: a no-op commit rotates the escrow, discarding the pre-state of the last real
change. Concrete witness on `Minimality.lean`'s corpus: a state observing `x1` with `x2` escrowed
becomes `x1` with `x1` escrowed — the undo target is gone, and nothing about the effect was
irreversible. -/
theorem commitAlways_getPut_counterexample :
    ∃ s : EscrowState, commitAlways s (observe s) ≠ s := by
  refine ⟨(x1, x2), ?_⟩
  intro h
  exact x1_ne_x2 (congrArg Prod.snd h)

-- =============================================================================================
-- § 5. The lens and the partial inverse are the same discipline seen twice.
-- =============================================================================================

/-- What the escrow slot holds after a real (observation-moving) commit is exactly what the escrowed
inverse restores. The lens's second component and `escrowInverse`'s `dst` are the same snapshot —
this is why "escrow (snapshot)" and "inverse (partial inverse)" are one contract in `req/188` §1 T1
and not two. -/
theorem escrow_holds_the_inverse_target (mint : Transformation → TransformationId)
    (f : Transformation) (esc : ObjectSnapshot) (hne : f.dst ≠ f.src) :
    (commitWithEscrow (f.src, esc) f.dst).2 = (escrowInverse mint f).dst := by
  unfold commitWithEscrow
  rw [if_neg hne]
  rfl

/-- Apply, then undo: the observation returns to the pre-state. The undo is driven by
`escrowInverse`'s `dst` — i.e. by the escrowed value, not by a fresh read of the world — which is
the operational meaning of law 1 for a caller. -/
theorem apply_then_undo_recovers_observation (mint : Transformation → TransformationId)
    (f : Transformation) (esc : ObjectSnapshot) (hne : f.dst ≠ f.src) :
    observe (commitWithEscrow (commitWithEscrow (f.src, esc) f.dst)
      (escrowInverse mint f).dst) = f.src := by
  have h₁ : commitWithEscrow (f.src, esc) f.dst = (f.dst, f.src) := by
    unfold commitWithEscrow
    rw [if_neg hne]
  have h₂ : commitWithEscrow (f.dst, f.src) (escrowInverse mint f).dst = (f.src, f.dst) := by
    unfold commitWithEscrow escrowInverse
    rw [if_neg (Ne.symm hne)]
  rw [h₁, h₂]
  rfl

/-- On a no-op effect the two laws coincide: GetPut says the state (escrow included) does not move,
and the effect's escrowed inverse observes as the effect itself. So the lens law and the
inverse-semigroup law are not two independent requirements — on `ObsIdempotent` elements they are the
same statement, which is the relation `req/38` §125 ruling 1 asks this file to establish. -/
theorem noop_effect_lens_and_inverse_agree (mint : Transformation → TransformationId)
    (f : Transformation) (he : ObsIdempotent f) (esc : ObjectSnapshot) :
    commitWithEscrow (f.src, esc) f.dst = (f.src, esc) ∧ ObsEq (escrowInverse mint f) f := by
  constructor
  · unfold commitWithEscrow
    rw [if_pos he.symm]
  · show ((f.dst, f.src) : ObjectSnapshot × ObjectSnapshot) = (f.src, f.dst)
    rw [he]

-- =============================================================================================
-- § 6. The minimality counterexample: a tool that declares no inverse (`invert()` → `None` →
-- `InverseStatus::Unavailable`). Partiality is essential, and it is contagious along a chain.
-- =============================================================================================

/-- Three observations: a draft, the saved document, and the state in which the mail has left the
building. Fresh bytes, disjoint from `Minimality.lean`'s `x1`/`x2` corpus, so the narrative of the
two files does not blur. -/
def draftDoc : ObjectSnapshot := ⟨ByteArray.mk #[51]⟩

def savedDoc : ObjectSnapshot := ⟨ByteArray.mk #[52]⟩

def sentMail : ObjectSnapshot := ⟨ByteArray.mk #[53]⟩

/-- The irreversibility discriminator, isolated as an executable gate so `#guard` can witness it
(the `Injection.lean` idiom). A catalogue that could not tell the two states apart would make the
counterexample vacuous. -/
def sentMailGate (s : ObjectSnapshot) : Bool := decide (s = sentMail)

#guard sentMailGate sentMail
#guard !(sentMailGate savedDoc)

/-- A total minting function for the counterexample's escrowed inverses — no axiom involved. -/
def mintInv (f : Transformation) : TransformationId := ⟨f.id.cid⟩

/-- The declarable tool: writing the document. Its prior contents can be restored. -/
def writeTxn : Transformation :=
  { id := ⟨ByteArray.mk #[54]⟩, order := 0, src := draftDoc, dst := savedDoc }

/-- The undeclarable tool: sending the mail. `gx-adapter-mcp/src/invert.rs` verbatim for exactly this
case: "No restore tool is declared for the tool being inverted. The change is one gx cannot undo". -/
def sendTxn : Transformation :=
  { id := ⟨ByteArray.mk #[55]⟩, order := 0, src := savedDoc, dst := sentMail }

theorem writeTxn_sendTxn_composable : composable writeTxn sendTxn := rfl

/-- The counterexample catalogue: everything is invertible except what ends with the mail sent.
Nothing about this catalogue is sloppy — it satisfies the escrow discipline (`catalogue_reverses`),
and it answers `none` for exactly the honest reason `invert.rs` names. -/
def catalogue (f : Transformation) : InvertOutcome :=
  if f.dst = sentMail then { inverse := none, verdict := Reversibility.false }
  else { inverse := some (escrowInverse mintInv f), verdict := Reversibility.true }

theorem catalogue_reverses : Reverses catalogue := by
  intro f f' h
  unfold catalogue at h
  by_cases hc : f.dst = sentMail
  · rw [if_pos hc] at h
    exact absurd h (by simp)
  · rw [if_neg hc] at h
    have h' : escrowInverse mintInv f = f' := by injection h
    exact ⟨h' ▸ rfl, h' ▸ rfl⟩

theorem catalogue_declares_writeTxn : Declared catalogue writeTxn := by
  refine ⟨escrowInverse mintInv writeTxn, ?_⟩
  unfold catalogue
  rw [if_neg (by decide : ¬ (writeTxn.dst = sentMail))]

theorem catalogue_refuses_sendTxn : (catalogue sendTxn).inverse = none := by
  have h : sendTxn.dst = sentMail := rfl
  unfold catalogue
  rw [if_pos h]

/-- 🔴 **DR-46-26**: and the refusal above is `false`, not `unknown`.

The distinction is the whole of C-25 and it is stated here rather than left implicit: this catalogue
*declares no restore* for a tool that sends mail, which is a fact about the change
(`Reversibility.false`, `gx-adapter-mcp/src/invert.rs`'s first legitimate reason). `unknown` is a
fact about a **read that did not answer** under a posture a deployment chose. No law consults this
(denominator 6) — it is asserted so that the model's counterexample cannot be read as being about
the other refusal. -/
theorem catalogue_refuses_sendTxn_as_false :
    (catalogue sendTxn).verdict = Reversibility.false := by
  have h : sendTxn.dst = sentMail := rfl
  unfold catalogue
  rw [if_pos h]

/-- The chain's own dst is the mail-sent state, so the catalogue refuses the composite too — by the
same honest rule, not by a second one. -/
theorem catalogue_refuses_chain :
    (catalogue (compose writeTxn sendTxn writeTxn_sendTxn_composable)).inverse = none := by
  have h : (compose writeTxn sendTxn writeTxn_sendTxn_composable).dst = sentMail := rfl
  unfold catalogue
  rw [if_pos h]

/-- **Minimality counterexample (partiality is essential)**: a catalogue that keeps the escrow
discipline in full, declares one tool, and declines one, is (i) not totally invertible, (ii) leaves
its declined element outside law 1 — `Recoverable` has no witness there, so the law is not merely
unproved but *inapplicable* — and (iii) **loses the whole chain**: `writeTxn` alone is recoverable,
yet `writeTxn` followed by `sendTxn` is not, although the chain is perfectly composable and its
first half was undoable a moment earlier. This is why the effect algebra is an *inverse semigroup*
with a partial operation and not a group, and why `req/186` §4 records that the domain *is* the
catalogue -- partiality is of the essence. It is also the exact shape of gx's operational rule
(`E-M3-4`: a change gx cannot undo is
escalated to a person *before* it happens) — the theorem says what that rule is protecting.

P-9: this is a counterexample construction, not an impossibility theorem. It does not show that no
catalogue can be total; it shows that *the escrow discipline does not by itself make one total*, and
that the laws' domain is the declared part. -/
theorem undeclaredInverse_counterexample :
    Reverses catalogue ∧
      Declared catalogue writeTxn ∧
      Recoverable catalogue writeTxn ∧
      ¬ Declared catalogue sendTxn ∧
      ¬ Recoverable catalogue sendTxn ∧
      composable writeTxn sendTxn ∧
      ¬ Declared catalogue (compose writeTxn sendTxn writeTxn_sendTxn_composable) ∧
      ¬ Recoverable catalogue (compose writeTxn sendTxn writeTxn_sendTxn_composable) ∧
      ¬ TotallyInvertible catalogue := by
  have hSend : ¬ Declared catalogue sendTxn := by
    rintro ⟨f', hf'⟩
    rw [catalogue_refuses_sendTxn] at hf'
    exact absurd hf' (by simp)
  have hChain : ¬ Declared catalogue (compose writeTxn sendTxn writeTxn_sendTxn_composable) := by
    rintro ⟨f', hf'⟩
    rw [catalogue_refuses_chain] at hf'
    exact absurd hf' (by simp)
  refine ⟨catalogue_reverses, catalogue_declares_writeTxn,
    law1_declared_recoverable catalogue_reverses writeTxn catalogue_declares_writeTxn,
    hSend, ?_, writeTxn_sendTxn_composable, hChain, ?_, ?_⟩
  · rintro ⟨f', hf', _⟩
    exact hSend ⟨f', hf'⟩
  · rintro ⟨f', hf', _⟩
    exact hChain ⟨f', hf'⟩
  · intro hTotal
    exact hSend (hTotal sendTxn)

/-- Positive control on the *same fixtures* (C5): with the tool declared, everything returns — law 1
holds for `writeTxn`, and the lens round trip on `writeTxn`'s own observations restores the
pre-state. So the §6 failure cannot be blamed on the fixtures, the escrow discipline or the lens; the
one moving part is whether the tool declared a restore. -/
theorem fixture_declared_recovery :
    Recoverable catalogue writeTxn ∧
      observe (commitWithEscrow (commitWithEscrow (writeTxn.src, draftDoc) writeTxn.dst)
        (escrowInverse mintInv writeTxn).dst) = writeTxn.src :=
  ⟨law1_declared_recoverable catalogue_reverses writeTxn catalogue_declares_writeTxn,
    apply_then_undo_recovers_observation mintInv writeTxn draftDoc (by decide)⟩

end GxSpec.EffectAlgebra
