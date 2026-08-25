import GxSpec.Core
import GxSpec.Admissible
import GxSpec.Receipt

/-!
# GxSpec.Minimality — the membrane's minimality (irreducibility) audit

Identity: `req/142_M8_LEAN_CONFORMANCE_REQDEF_2026-08-14.md` §1 M8-d item 3, consuming
`req/116_M8_REQDEF_SKELETON_2026-08-13.md` §2 (the Owner's question "is this truly the
mathematically minimal, rigorously subdivided abstraction?"'s own scope item: "the formal audit
of the membrane's minimality (irreducibility)"). The membrane (`req/112` §line 140 / `req/122`
§line 50, verbatim) is **Rule 1** (the three powers of CID-minting/verdict/transition are each a
single point; a secondary surface carries zero semantic authority) + **Rule 2** (clock/rng
external inputs enter through a single injection point). This file audits Rule 1's three powers.
(sem: SEM-lean-089)

## What is proved here, precisely (P-9 — no overclaim)

Each theorem below is a **counterexample construction** (`req/142`'s own word), not an
impossibility theorem over all conceivable designs. For one of Rule 1's three powers, it exhibits a (sem: SEM-lean-090)
concrete instance of the *existing, frozen* GxSpec vocabulary
(`Admissible`/`OrderBounded`/`Ledger`/`LedgerAdmitsOnlyAdmissible`/`ProofSound`/`compose`,
unchanged imports from `GxSpec.Admissible`/`GxSpec.Receipt`/`GxSpec.Core`) in which every other
structural hypothesis a T4-shaped soundness statement needs is satisfied, and the one hypothesis
that formalises "this power's single point enforces X" is both *absent* and *demonstrably
false* — and in that instance the T4-shaped conclusion is actively false, not merely unproven.
This is what makes the three powers individually load-bearing rather than merely convenient: drop
any one, and a concrete model exists where receipt soundness (the T4/T1 family) fails.

**Completeness is not claimed.** This file does not prove the three powers are the *only* possible
minimal set (an exhaustive search over alternative designs is out of scope, exactly as it is for
a4 below). What is proved is narrower and honest: for *each* of the three named powers, *this
specific* relaxation (letting that power's enforcement point be non-unique/absent) *provably*
breaks a T4-shaped guarantee in a constructed instance.

**a4's position (`req/116` §2 / `req/142` §1 M8-d item 3)**: the known non-covered-enumeration
counterexample (a4, referenced in `12-formal-semantics.md`'s F0 coverage discussion) audits F0's
*coverage* of change kinds — a different axis from the membrane's *power-count* minimality this
file audits. It is referenced, not re-proved, here; its role is only to fix the *stance* both
items share — declare the boundary of what is checked, never claim exhaustiveness -- "making it
checkable whether the known non-covered enumeration is complete" (quoted in SEM-lean-091), the same posture this file takes for the three powers below.

**Rule 2 is out of this file's proved scope, declared rather than silently dropped**: `req/142` §1
groups Rule 1's three powers with Rule 2's single injection point (clock/rng) in one scope sentence, but
`AC-M8-d`'s own minimum is "at least 3 counterexample constructions (one per power)" (quoted in SEM-lean-092) — Rule 1's three powers only. `GxSpec.Core`'s F0
surface carries no time/randomness field at all (`46-verification-plan.md` §1's crypto/time
non-goal), so a Rule 2 counterexample would need to invent structure with no frozen counterpart — (sem: SEM-lean-093)
out of scope for this file. Three counterexamples are delivered below, not four; this is stated
once here rather than left for a reader to notice by absence.
(v0.3-c cross-reference, additive — this file's scope above is unchanged: the Rule 2 counterexample (sem: SEM-lean-094)
now exists on a declared *extension* surface, `GxSpec/Injection.lean` — clock instance only, rng
declared as denominator there — under `DR-46-4` / `req/160` §3-1's five legitimacy conditions.)
(v0.4-d cross-reference, additive — the v0.3-c note above is unchanged as a record: the rng
instance now exists too, `GxSpec/InjectionRng.lean`, same DR-46-4 schema; the "Rule 2's counterexample is complete" (quoted in SEM-lean-095)
claim stays with `req/38` §NN acceptance.)
(U4 cross-reference, additive -- this file's scope above is unchanged: the *field-level* minimality
of the F0 surface itself (which fields of `Transformation`/`ObjectSnapshot`/`TransformationId`/
`Ledger`/`InclusionProof`/`Receipt`/`Verdict`/`Canonicalizer`/`CanonModel.Field` the theorem ledger
actually consumes) is audited in `GxSpec/MinimalityF0.lean` (`req/188` §2 U4, `req/191`), reusing
this file's fixtures `x1`/`x2`/`trivialProof`/`order5Txn` by import; the membrane audit here is
not re-proved there.)

## The three powers, and which frozen hypothesis each one is shown to be necessary for

| power (`req/112`/`req/122`, verbatim) | frozen hypothesis whose necessity is shown | theorem | (sem: SEM-lean-096)
|---|---|---|
| CID minting | `ofId`/`idOf` must resolve an id back to *the one* content that was minted for it | `mintingSinglePoint_counterexample` | (sem: SEM-lean-097)
| verdict | `LedgerAdmitsOnlyAdmissible` (T4's own soundness hypothesis) | `verdictSinglePoint_counterexample` |
| transition (commit) | the same hypothesis, evaluated at a **composed** transformation — the case T1's closure exists to police | `transitionSinglePoint_counterexample` | (sem: SEM-lean-098)
-/

namespace GxSpec.Minimality

open GxSpec

-- =============================================================================================
-- § 0. shared fixtures (reused across all three powers, not a name collision — one small corpus
-- of concrete `ObjectSnapshot`s standing in for "content", as `Runner.lean`'s `snapOf` does).
-- =============================================================================================

/-- Two distinct pieces of content. Everything below that needs "two different things" reuses
these rather than inventing a fresh pair per power. -/
def x1 : ObjectSnapshot := ⟨ByteArray.mk #[1]⟩

def x2 : ObjectSnapshot := ⟨ByteArray.mk #[2]⟩

theorem x1_ne_x2 : x1 ≠ x2 := by decide

-- =============================================================================================
-- § 1. Power 1 — CID minting: the single point is what keeps id→content resolution (sem: SEM-lean-099)
-- well-defined. Relax it (let minting collide) and no resolution function `ofId` can stay
-- faithful to every colliding content at once.
-- =============================================================================================

/- §89 A-3 correction note (sem: SEM-lean-100) (`req/38` §89 ruling 3, `req/147` §4; no-delete = a record of
how this developed): this theorem (`mintingSinglePoint_counterexample`, the theorem name is
unchanged) was originally built in the following shape -- `collidingIdOf := fun _ => ⟨ByteArray.mk
#[0]⟩` (a constant function collapsing every `ObjectSnapshot` to one point -- the maximally
degenerate map) + `ofId := fun _ => identity x2 collidingIdOf` (a resolution function constant
regardless of input, carrying no structural correspondence to `collidingIdOf` at all). The audit's
(A-3) point: sufficient as a witness for the ∃ proposition (the implication that if it breaks under
the maximally degenerate map it must also break under a weaker collision is sound), but as a
depiction of "single point -> two points" in minimal form, one notch weaker than the other two
(Power 2/3). Below, with the same theorem name and the same conclusion shape, the witness is
narrowed to a **minimal pair** (only `x1`/`x2` collide onto the same id; every other
`ObjectSnapshot` is injective because it carries its own `digest` straight through), and `ofId` is
constructed as `collidingIdOf`'s **deliberate inverse** (for any id other than the collision, the
`.cid`/`.digest` round trip correctly recovers `x`; only at the colliding id does it pick `x2` --
and that one choice is exactly what causes the breakage). The old shape is not kept as code, since
Lean does not allow a duplicate definition under the same name; it is kept only in this note.
(sem: SEM-lean-100) -/

/-- The minimal pair's colliding minting function: only `x2` is redirected to `x1`'s original
    id (`⟨x1.digest⟩`) -- the sole collision. Every other `ObjectSnapshot` (including `x1` itself)
    receives the value of `⟨x.digest⟩` -- the map that carries its own `digest` straight through.
    This branch is obviously injective (`ObjectSnapshot` is a one-field `digest` structure, and
    `resolveContent`'s `else` branch directly exhibits its inverse -- see below).
    (sem: SEM-lean-101) -/
def collidingIdOf (x : ObjectSnapshot) : TransformationId :=
  if x = x2 then ⟨x1.digest⟩ else ⟨x.digest⟩

/-- A small theorem that machine-confirms the collision is real (in the style of `x1_ne_x2`).
    (sem: SEM-lean-102) -/
theorem collidingIdOf_x1_eq_x2 : collidingIdOf x1 = collidingIdOf x2 := by
  simp [collidingIdOf, x1_ne_x2]

/-- `collidingIdOf`'s deliberate inverse: it picks `x2` only for the colliding id itself
    (`collidingIdOf x1`) -- this is where the breakage below happens. Every other id gets
    `⟨tid.cid⟩` -- mapping `.cid` straight to `.digest`, which is the correct inverse of
    `collidingIdOf`'s non-colliding branch (`⟨x.digest⟩` when `x ≠ x2`). A construction that
    corresponds to `collidingIdOf`'s structure, not "an uncorrelated resolution function"
    (the response to the weakness §89 A-3 pointed out). (sem: SEM-lean-103) -/
def resolveContent (tid : TransformationId) : ObjectSnapshot :=
  if tid = collidingIdOf x1 then x2 else ⟨tid.cid⟩

/-- Power 1's ofId body: the `TransformationId → Transformation` assembled from `identity`
    (`Core.lean`), `resolveContent`, and `collidingIdOf`. (sem: SEM-lean-104) -/
def ofId (tid : TransformationId) : Transformation :=
  identity (resolveContent tid) collidingIdOf

/-- Admissibility that distinguishes the two colliding contents (only `x1`'s identity
transformation is admissible). The collision itself is invisible to `A`; it only bites once a
*resolution* function has to pick one content per id. -/
def contentA : Transformation → Prop := fun t => t.src = x1

/-- CID-minting counterexample (Power 1): a ledger legitimately admits the id `collidingIdOf x1`
because the identity transformation over `x1` really is `contentA`-admissible. But `x2` also
collides onto that same id (`collidingIdOf_x1_eq_x2`), and `ofId` — built as `collidingIdOf`'s own
intentional inverse (`resolveContent`), not an unrelated constant — is forced to pick one content
at that single id; it picks `x2`, so `LedgerAdmitsOnlyAdmissible` fails on an id a real gate
legitimately admitted, purely because minting was not injective at this one pair. -/
theorem mintingSinglePoint_counterexample :
    ∃ (L : Ledger) (ofId : TransformationId → Transformation),
      L.contains (collidingIdOf x1) Verdict.admit ∧
        ¬ LedgerAdmitsOnlyAdmissible contentA L ofId := by
  refine ⟨⟨fun tid v => tid = collidingIdOf x1 ∧ v = Verdict.admit⟩, ofId, ⟨rfl, rfl⟩, ?_⟩
  intro hLedger
  have h : contentA (ofId (collidingIdOf x1)) := hLedger (collidingIdOf x1) ⟨rfl, rfl⟩
  have hofId : ofId (collidingIdOf x1) = identity x2 collidingIdOf := by
    unfold ofId resolveContent
    rw [if_pos rfl]
  rw [hofId] at h
  exact x1_ne_x2 h.symm

-- =============================================================================================
-- § 2. Power 2 — verdict: the single point is what keeps `LedgerAdmitsOnlyAdmissible` (T4's own
-- soundness hypothesis) true. Relax it (let a second, unpoliced writer mark entries `admit`) and
-- the ledger can admit a transformation that is not `OrderBounded 0`-admissible at all — reusing
-- `OrderBounded`, the same class `Admissible.lean::orderBounded_admissible` already uses as T1's
-- non-vacuity witness, rather than inventing a fresh admissibility notion.
-- =============================================================================================

/-- A ledger that admits *every* id, as an unpoliced second writer would (nothing here consults
`A` before writing `admit`). -/
def anyAdmitLedger : Ledger := ⟨fun _ v => v = Verdict.admit⟩

/-- A trivial inclusion proof whose `verifies` is literally `contains` — `ProofSound` holds for
it outright, so the counterexample below is not smuggling the failure into the proof machinery
(46 §1's crypto non-goal); the failure is entirely in the ledger's *write* policy. -/
def trivialProof : InclusionProof := ⟨fun L t v => L.contains t v⟩

theorem trivialProof_sound : ProofSound trivialProof := fun _ _ _ h => h

/-- A `Receipt` claiming admission for an arbitrary id, verified by `trivialProof`. -/
def anyReceipt (t : TransformationId) : Receipt := ⟨t, Verdict.admit, trivialProof⟩

/-- A concrete non-admissible (order 5, over the class bound 0) transformation — what an
unpoliced verdict-writer admits anyway. -/
def order5Txn : Transformation := { id := ⟨ByteArray.mk #[9]⟩, order := 5, src := x1, dst := x1 }

/-- Verdict counterexample (Power 2): every hypothesis `T4_receipt_soundness` needs *other than*
`LedgerAdmitsOnlyAdmissible` holds here (`ValidReceipt`, `r.v = admit`, `ProofSound`) — and
`LedgerAdmitsOnlyAdmissible` is demonstrably false for `OrderBounded 0` against `order5Txn`
(`5 ≤ 0` does not hold), which is exactly the class T1's own non-vacuity witness uses. Without a
single point enforcing "only write `admit` for something actually checked", the ledger produces a
receipt that verifies (`ValidReceipt`), is proof-sound (`ProofSound`), and claims `admit`, for a
transformation that was never `OrderBounded 0`-admissible. -/
theorem verdictSinglePoint_counterexample (t : TransformationId) :
    ValidReceipt anyAdmitLedger (anyReceipt t) ∧
      (anyReceipt t).v = Verdict.admit ∧
      ProofSound (anyReceipt t).proof ∧
      ¬ LedgerAdmitsOnlyAdmissible (OrderBounded 0) anyAdmitLedger (fun _ => order5Txn) := by
  refine ⟨rfl, rfl, trivialProof_sound, ?_⟩
  intro hLedger
  have h : OrderBounded 0 order5Txn := hLedger t rfl
  have h' : order5Txn.order ≤ 0 := h
  exact absurd h' (by decide)

-- =============================================================================================
-- § 3. Power 3 — transition (commit): the same hypothesis as Power 2, but evaluated at a (sem: SEM-lean-105)
-- *composed* transformation — the exact case `T1_composition_sound`'s closure exists to police
-- (`A f → A g → A (compose f g h)`). Relax single-point commit and a second committer can write a
-- composed transformation's id as `admit` without either constituent having been re-checked —
-- distinct failure surface from Power 2's atomic case, same underlying necessity pattern.
-- =============================================================================================

/-- `fTxn` is `OrderBounded 0`-admissible (order 0); `gTxn` is not (order 5) — composable by
construction (`fTxn.dst = x2 = gTxn.src`). -/
def fTxn : Transformation := { id := ⟨ByteArray.mk #[10]⟩, order := 0, src := x1, dst := x2 }

def gTxn : Transformation := { id := ⟨ByteArray.mk #[11]⟩, order := 5, src := x2, dst := x1 }

theorem fTxn_gTxn_composable : composable fTxn gTxn := rfl

/-- Transition counterexample (Power 3): `fTxn` alone is admissible, `gTxn` alone is not — exactly
the case `T1_composition_sound` refuses to certify a compose for (it needs *both*). If a second,
unpoliced committer writes the composed transformation's id as `admit` regardless (bypassing the
single point that would otherwise run `T1`'s own check before committing), the resulting ledger
again falsifies `LedgerAdmitsOnlyAdmissible` — this time for a `compose`, not an atomic
transformation, showing the single-point requirement is load-bearing at the transition step
specifically and not only at first admission (Power 2). -/
theorem transitionSinglePoint_counterexample :
    OrderBounded 0 fTxn ∧ ¬ OrderBounded 0 gTxn ∧
      ¬ LedgerAdmitsOnlyAdmissible (OrderBounded 0) anyAdmitLedger
        (fun _ => compose fTxn gTxn fTxn_gTxn_composable) := by
  refine ⟨show fTxn.order ≤ 0 by decide, ?_, ?_⟩
  · show ¬ gTxn.order ≤ 0
    decide
  · intro hLedger
    have h := hLedger (compose fTxn gTxn fTxn_gTxn_composable).id rfl
    have horder : (compose fTxn gTxn fTxn_gTxn_composable).order = max fTxn.order gTxn.order := rfl
    have hbound : max fTxn.order gTxn.order ≤ 0 := horder ▸ h
    exact absurd hbound (by decide)

end GxSpec.Minimality
