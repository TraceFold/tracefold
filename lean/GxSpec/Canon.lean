/-!
# GxSpec.Canon — the canonicalizer Canonicalizer and T3 (idempotence + representation-independence)
(sem: SEM-lean-015)

Identity: a plain-Prop model of `12-formal-semantics.md` F0's T3 (canonicalization idempotence
`canon(canon(x)) = canon(x)`, representation-independence `repr₁ ≈ repr₂ → canon(repr₁ x) =
canon(repr₂ x)`). Rust-side counterpart: gx-canon. (sem: SEM-lean-016)

Note (verbatim from `46-verification-plan.md` §2.4): T3 is not a proof that "a `Canonicalizer`
satisfying the property exists"; it is used as the Lean specification of the property gx-canon's
implementation must satisfy, and the differential test (46 §3) checks that the implementation
follows it. (sem: SEM-lean-017)

**`req/38` §85 ruling (§85-3, M8-b fix directive 3)**: below, `T3_canon` (the abstract form)
merely restates the hypotheses `hIdem`/`hIndep` as the conclusion (proof = `⟨hIdem, hIndep⟩`) and
was ruled **vacuous**. Under the no-delete discipline the abstract form itself is kept, not deleted
(it continues to serve as the vocabulary definition of `Idempotent`/`ReprIndependent` that the
differential-test vector schema points to -- only the proof was vacuous). The M8-b canonical form
is `CanonModel` below (the key-ascending insertion-sort executable model from
`req/lean/GxSpec/Canon.lean`, ported as-is within the §85 canonical form's scope -- ruling 3, a
read-only substrate) -- it **proves, with no hypothesis**, `Idempotent CanonModel.canonicalizer` /
`ReprIndependent CanonModel.sameCanon CanonModel.canonicalizer`. (sem: SEM-lean-018)
-/

namespace GxSpec

section Abstract

variable {Repr : Type} {Canonical : Type}

/-- The canonicalization function. canon : representation -> canonical form. embed :
    reinterprets the canonical form back as a representation (for the round trip).
    (sem: SEM-lean-019) -/
structure Canonicalizer (Repr Canonical : Type) where
  canon : Repr → Canonical
  embed : Canonical → Repr

-- The equivalence relation (≈) between representations. Domain-specific, but F0 assumes it
-- abstractly (per 12 F0's note). (sem: SEM-lean-020)
variable (equiv : Repr → Repr → Prop)

/-- T3-a canonicalization idempotence: canon(canon(x)) = canon(x) (idempotence after a
    round trip through embed). (sem: SEM-lean-021) -/
def Idempotent (C : Canonicalizer Repr Canonical) : Prop :=
  ∀ x : Repr, C.canon (C.embed (C.canon x)) = C.canon x

/-- T3-b representation-independence: repr₁ ≈ repr₂ → canon(repr₁) = canon(repr₂).
    (sem: SEM-lean-022) -/
def ReprIndependent (C : Canonicalizer Repr Canonical) : Prop :=
  ∀ r1 r2 : Repr, equiv r1 r2 → C.canon r1 = C.canon r2

/-- T3 canonicalization idempotence + representation-independence (abstract form). **§85 ruling
    = vacuous** (a shape short of even a definitional projection, merely restating the hypotheses
    as the conclusion -- `⟨hIdem, hIndep⟩`). The replacement canonical form is `CanonModel`'s
    `T3_canon_model` (below). (sem: SEM-lean-023) -/
theorem T3_canon
    (C : Canonicalizer Repr Canonical)
    (hIdem : Idempotent C) (hIndep : ReprIndependent equiv C) :
    Idempotent C ∧ ReprIndependent equiv C :=
  ⟨hIdem, hIndep⟩

end Abstract

/- The executable canon model that the 46 §3.1 differential-test runner (kind = `canon_idempotence`)
   calls into. The actual DAG-CBOR encode is not written in Lean (46 ASM-3 = the independent-model
   approach, a non-goal stated at the top of §2). What is modeled here is only 42 §2.1 rule 2 --
   "a map's keys are ascending in bytewise lexicographic order; duplicate keys of the same name are
   not allowed" -- with the key abstracted to a Nat standing for its rank in byte-dictionary order,
   and the value abstracted to a Nat standing for that subtree's digest. The byte string itself is
   never handled. The `CanonModel` namespace from `req/lean/GxSpec/Canon.lean` is ported as-is
   within the §85 canonical form's scope (ruling 3, a read-only substrate). (sem: SEM-lean-024)

   **§89 A-1a correction note (sem: SEM-lean-025) (`req/38` §89 ruling 1, `req/147` §2; no-delete = the description
   above is unchanged)**: what the design above -- "the key is abstracted to a Nat standing for
   its rank in byte-dictionary order" -- actually means: the runner never receives the key's raw
   byte string at all; the key -> rank mapping is decided solely on the Rust side
   (`crates/gx-canon/tests/conformance_gen.rs`). So what `canon_idempotence`/`repr_independence`
   independently recomputes is only "re-sorting after treating the rank as the key", not an
   independent check of the actual byte-string order of the keys (DAG-CBOR's bytewise canonical
   order) itself -- measured: a hand-built reversed-labeling vector PASSes in both directions
   (`req/147` §2 A-1a). The current corpus uses fixed-length keys only, so this causes no actual
   harm (`req/145` gotcha 6). An independent, Lean-side bytewise comparison of the key byte
   string (or UTF-8 code-point sequence) is **reserved for v0.2** (`46-verification-plan.md` §8
   `DR-46-2`, pending a fit check against 46 §1's non-goal of not formalizing this in Lean).
   (sem: SEM-lean-025)

   **v0.2-c addendum (DR-46-2 discharged, `req/156`; comment-only -- this module's definitions/
   proofs are unchanged)**: the reservation above has been discharged at the instrument layer --
   the vector now carries `key_bytes` (the key's raw UTF-8 byte string), and `Runner.lean`
   independently re-derives the order with a length-first-then-bytewise abstract-sequence
   comparison (`keyBytesLt`), cross-checking it against the same `expected` as this model's rank
   re-sort (a doubled acceptance + independent-recount check). `CanonModel` itself stays frozen,
   unchanged in its rank abstraction -- it is the Runner-side byte path that catches A-1a's
   reversed-labeling vector. (sem: SEM-lean-026) -/
namespace CanonModel

structure Field where
  key : Nat
  val : Nat
  deriving DecidableEq

abbrev Flat := List Field

/-- Inserts one entry while keeping key order strictly ascending. For an equal key, the one
    that appeared first is kept (how the no-duplicates rule is resolved). (sem: SEM-lean-027) -/
def ins (f : Field) : Flat → Flat
  | [] => [f]
  | g :: t =>
      if f.key < g.key then f :: g :: t
      else if f.key = g.key then f :: t
      else g :: ins f t

/-- The body of canon. Total, deterministic, `#eval`-able. (sem: SEM-lean-028) -/
def canon : Flat → Flat
  | [] => []
  | f :: t => ins f (canon t)

/-- A strict lower bound on the head key (no constraint for an empty list). (sem: SEM-lean-029) -/
def ltHead (k : Nat) : Flat → Prop
  | [] => True
  | g :: _ => k < g.key

/-- Strictly increasing by key. This is the definition of the canonical form itself.
    (sem: SEM-lean-030) -/
inductive Sorted : Flat → Prop
  | nil : Sorted []
  | cons (f : Field) (t : Flat) : ltHead f.key t → Sorted t → Sorted (f :: t)

theorem ltHead_ins {k : Nat} {f : Field} {l : Flat} (hl : ltHead k l) (hf : k < f.key) :
    ltHead k (ins f l) := by
  cases l with
  | nil => exact hf
  | cons g t =>
      by_cases h1 : f.key < g.key
      · simp only [ins, if_pos h1]; exact hf
      · by_cases h2 : f.key = g.key
        · simp only [ins, if_neg h1, if_pos h2]; exact hf
        · simp only [ins, if_neg h1, if_neg h2]; exact hl

theorem sorted_ins (f : Field) : ∀ l : Flat, Sorted l → Sorted (ins f l) := by
  intro l
  induction l with
  | nil => intro _; exact Sorted.cons f [] trivial Sorted.nil
  | cons g t ih =>
      intro h
      cases h with
      | cons _ _ hgt hst =>
        by_cases h1 : f.key < g.key
        · simp only [ins, if_pos h1]
          exact Sorted.cons f (g :: t) h1 (Sorted.cons g t hgt hst)
        · by_cases h2 : f.key = g.key
          · simp only [ins, if_neg h1, if_pos h2]
            refine Sorted.cons f t ?_ hst
            rw [h2]; exact hgt
          · have h3 : g.key < f.key := by omega
            simp only [ins, if_neg h1, if_neg h2]
            exact Sorted.cons g (ins f t) (ltHead_ins hgt h3) (ih hst)

theorem sorted_canon : ∀ l : Flat, Sorted (canon l)
  | [] => Sorted.nil
  | f :: t => sorted_ins f (canon t) (sorted_canon t)

theorem ins_of_ltHead {f : Field} : ∀ l : Flat, ltHead f.key l → ins f l = f :: l := by
  intro l h
  cases l with
  | nil => rfl
  | cons g t =>
      have h' : f.key < g.key := h
      simp only [ins, if_pos h']

theorem canon_of_sorted : ∀ l : Flat, Sorted l → canon l = l := by
  intro l
  induction l with
  | nil => intro _; rfl
  | cons f t ih =>
      intro h
      cases h with
      | cons _ _ hlt hst =>
        show ins f (canon t) = f :: t
        rw [ih hst]
        exact ins_of_ltHead t hlt

/-- T3-a (executable-model side): canon is idempotent. Follows from the construction (carries
    no unproven hole). (sem: SEM-lean-031) -/
theorem canon_idem (l : Flat) : canon (canon l) = canon l :=
  canon_of_sorted (canon l) (sorted_canon l)

/-- The connection to the abstract spec (§2.4). embed is the identity, since it only sends the
    canonical form back to a representation. (sem: SEM-lean-032) -/
def canonicalizer : Canonicalizer Flat Flat :=
  { canon := canon, embed := id }

theorem canonicalizer_idempotent : Idempotent canonicalizer := canon_idem

/-- The model-side representation equivalence is "the canonical forms agree". Since this is
    canon's kernel, `ReprIndependent` follows from the definition. "Two semantically identical but
    differently-byte-encoded representations land on the same CID" cannot be claimed by this model;
    that is the job of the 46 §3 differential test (kind = `repr_independence`). (sem: SEM-lean-033) -/
def sameCanon (r1 r2 : Flat) : Prop := canon r1 = canon r2

theorem canonicalizer_reprIndependent : ReprIndependent sameCanon canonicalizer :=
  fun _ _ h => h

/-- The witness that T3 (§2.4's shape) is actually satisfied by this model (confirming the
    hypothesis is not vacuous). (sem: SEM-lean-034) -/
theorem T3_canon_model :
    Idempotent canonicalizer ∧ ReprIndependent sameCanon canonicalizer :=
  T3_canon sameCanon canonicalizer canonicalizer_idempotent canonicalizer_reprIndependent

/-- The deterministic check the runner calls per vector (46 §3.1 kind = `canon_idempotence`).
    (sem: SEM-lean-035) -/
def idempotenceCheck (l : Flat) : Bool := canon (canon l) == canon l

theorem idempotenceCheck_true (l : Flat) : idempotenceCheck l = true := by
  simp only [idempotenceCheck, canon_idem, beq_self_eq_true]

#guard canon [⟨2, 7⟩, ⟨1, 5⟩, ⟨2, 9⟩] == [⟨1, 5⟩, ⟨2, 7⟩]
#guard idempotenceCheck [⟨3, 1⟩, ⟨1, 2⟩, ⟨3, 4⟩, ⟨2, 0⟩]

end CanonModel

end GxSpec
