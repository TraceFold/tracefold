import GxSpec.Core

/-!
# GxSpec.Receipt — the abstract Ledger/Receipt, T4 (soundness), and T5 (witness recovery)
(sem: SEM-lean-106)

Identity: a model that concretizes `12-formal-semantics.md` F0's witness/receipt construction
(the lax functor `W`) as an abstract ledger (Ledger) and receipt verification (ValidReceipt).
Rust-side counterpart: gx-witness/gx-log. (sem: SEM-lean-107)

**Correspondence with the two receipt kinds (ASM-14, verbatim from `46-verification-plan.md`
§2.5)**: the Rust side distinguishes `VerdictReceipt` (issued for every verdict = Admit/Deny/
Escalate, DSSE-signed, no inclusion proof) from `CommitReceipt` (only on commit success,
inclusion proof required). This file's `Receipt` (requiring `InclusionProof`) models the
`CommitReceipt` side, and what `T4_receipt_soundness` says -- "receipt verification accepts ⇒
inclusion in the ledger" -- holds **only for `CommitReceipt`**. The differential-test vectors
(M8-c) preserve this distinction by `kind`, and a `VerdictReceipt` verification vector is never
repurposed for the T4 soundness claim. (sem: SEM-lean-108)

**`req/38` §85 ruling (§85-4/5, M8-b fix directives 4, 2)**: T4 was ruled unprovable in its
original shape (no hypothesis exists to bridge `verifies` to `contains`, so the conclusion cannot
be derived) and is fixed by adding the `ProofSound` hypothesis (below -- ASM-14's
CommitReceipt-only note is already documented and stays in force). T5 was ruled a definitional
projection of the same shape as T1 (one line applying `hRec`), and a non-vacuity witness
(`RecoverableChainWitness`, below) is placed alongside it. (sem: SEM-lean-109)
-/

namespace GxSpec

/-- Verdict (conforms to the glossary, 11 §4). (sem: SEM-lean-110) -/
inductive Verdict where
  | admit
  | deny
  | escalate

/-- The abstract ledger. Does not imitate the actual Merkle tile log's structure (gx-log);
    it carries only the predicate "does this (TransformationId, Verdict) pair appear".
    (sem: SEM-lean-111) -/
structure Ledger where
  contains : TransformationId → Verdict → Prop

/-- The abstract inclusion proof. The actual Merkle proof algorithm is out of scope for Lean
    (46 §1 non-goal). (sem: SEM-lean-112) -/
structure InclusionProof where
  verifies : Ledger → TransformationId → Verdict → Prop

structure Receipt where
  t     : TransformationId
  v     : Verdict
  proof : InclusionProof

def ValidReceipt (L : Ledger) (r : Receipt) : Prop :=
  r.proof.verifies L r.t r.v

/-- T4 receipt soundness: receipt verification accepts ⇒ the corresponding transformation
    satisfies A and is included in the ledger (on the model). "The corresponding transformation
    satisfies A" is required as a hypothesis via the ledger-side property that only entries
    admitted at the time L was constructed satisfy contains (LedgerAdmitsOnlyAdmissible).
    (sem: SEM-lean-113) -/
def LedgerAdmitsOnlyAdmissible
    (A : Transformation → Prop) (L : Ledger) (ofId : TransformationId → Transformation) : Prop :=
  ∀ tid, L.contains tid Verdict.admit → A (ofId tid)

/-- The proof-soundness hypothesis (§85 ruling 4) that `InclusionProof.verifies` correctly
    reflects actual inclusion in the ledger. The correctness of the real Merkle proof algorithm
    itself is out of scope for Lean (46 §1 non-goal) -- this hypothesis makes explicit the
    abstraction boundary at which the model grants that the algorithm is sound. **Why a separate
    hypothesis**: it is added as an independent hypothesis rather than folded into `ValidReceipt`'s
    own definition (`r.proof.verifies L r.t r.v`) because `ValidReceipt` is the shared vocabulary
    (ASM-14) that both `VerdictReceipt` and `CommitReceipt` pass through as the receipt-
    verification predicate, and mixing soundness into it would erase the meaning of that
    distinction -- it is required locally only for T4 (the `CommitReceipt`-side claim; see the
    header). (sem: SEM-lean-114) -/
def ProofSound (p : InclusionProof) : Prop :=
  ∀ (L : Ledger) (t : TransformationId) (v : Verdict), p.verifies L t v → L.contains t v

/-- The fixed form of T4 receipt soundness (§85 ruling 4): derived from a two-way decomposition
    of trust -- `ProofSound` (proof soundness) and `LedgerAdmitsOnlyAdmissible` (the ledger's
    admit-only property). (sem: SEM-lean-115) -/
theorem T4_receipt_soundness
    {A : Transformation → Prop} {L : Ledger} {ofId : TransformationId → Transformation}
    (hLedger : LedgerAdmitsOnlyAdmissible A L ofId)
    (r : Receipt) (hv : ValidReceipt L r) (hAdmit : r.v = Verdict.admit)
    (hSound : ProofSound r.proof) :
    A (ofId r.t) ∧ L.contains r.t r.v := by
  have hContains : L.contains r.t r.v := hSound L r.t r.v hv
  refine ⟨?_, hContains⟩
  have hContains' : L.contains r.t Verdict.admit := hAdmit ▸ hContains
  exact hLedger r.t hContains'

/-- T5 witness lax composition: the verdict of each stage can be recovered from a composed
    transformation's receipt chain. "Recoverable" is expressed as the existence of a function that
    uniquely determines both constituent receipts' verdicts from the composed receipt.
    (sem: SEM-lean-116) -/
def RecoverableChain
    (L : Ledger) (recover : TransformationId → Option (TransformationId × TransformationId)) : Prop :=
  ∀ tcomp t1 t2 v, recover tcomp = some (t1, t2) → L.contains tcomp v →
    ∃ v1 v2, L.contains t1 v1 ∧ L.contains t2 v2

/-- Of the same shape as T1 (T5, §85 ruling 5): the proof is exactly an application of
    `RecoverableChain`'s definition (a definitional projection, one line applying `hRec`).
    Non-vacuity is confirmed constructively by
    `RecoverableChainWitness.recoverableChain_witness` (below). (sem: SEM-lean-117) -/
theorem T5_witness_recovery
    {L : Ledger} {recover : TransformationId → Option (TransformationId × TransformationId)}
    (hRec : RecoverableChain L recover)
    (tcomp t1 t2 : TransformationId) (v : Verdict)
    (hsplit : recover tcomp = some (t1, t2)) (hcontains : L.contains tcomp v) :
    ∃ v1 v2, L.contains t1 v1 ∧ L.contains t2 v2 :=
  hRec tcomp t1 t2 v hsplit hcontains

/- The non-vacuity witness that T5's hypothesis `RecoverableChain L recover` is not
   unsatisfiable (§85 ruling 5): against a concrete `recover` that records only that the compose
   target `idComp` splits into the constituents `(idA, idB)`, and a concrete `ledger` that records
   only that both constituents are admitted, `RecoverableChain` is satisfied constructively, with
   no hypothesis. (sem: SEM-lean-118) -/
namespace RecoverableChainWitness

/-- Three concrete TransformationIds (one compose target, two constituents). (sem: SEM-lean-119) -/
def idA : TransformationId := ⟨ByteArray.mk #[1]⟩
def idB : TransformationId := ⟨ByteArray.mk #[2]⟩
def idComp : TransformationId := ⟨ByteArray.mk #[3]⟩

/-- The recover function that records that only idComp splits into (idA, idB). (sem: SEM-lean-120) -/
def recover (tid : TransformationId) : Option (TransformationId × TransformationId) :=
  if tid = idComp then some (idA, idB) else none

/-- The ledger that records only that idA/idB are already admitted. (sem: SEM-lean-121) -/
def ledger : Ledger :=
  { contains := fun tid v => v = Verdict.admit ∧ (tid = idA ∨ tid = idB) }

theorem recoverableChain_witness : RecoverableChain ledger recover := by
  intro tcomp t1 t2 v hsplit _hcontains
  by_cases h : tcomp = idComp
  · subst h
    have hstep : recover idComp = some (idA, idB) := rfl
    rw [hstep] at hsplit
    injection hsplit with hpair
    injection hpair with ht1 ht2
    subst ht1; subst ht2
    exact ⟨Verdict.admit, Verdict.admit, ⟨rfl, Or.inl rfl⟩, ⟨rfl, Or.inr rfl⟩⟩
  · have hstep : recover tcomp = none := by
      unfold recover
      rw [if_neg h]
    rw [hstep] at hsplit
    exact absurd hsplit (by simp)

end RecoverableChainWitness

end GxSpec
