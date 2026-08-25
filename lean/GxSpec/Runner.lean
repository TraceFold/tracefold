import GxSpec
import Lean.Data.Json

/-!
# GxSpec.Runner — the M8-c differential-test runner

Identity (sem: SEM-lean-122): `req/142_M8_LEAN_CONFORMANCE_REQDEF_2026-08-14.md` §1 M8-c item 2
(`lean/GxSpec/Runner.lean` — Lean executable model — reads the JSONL vectors the Rust generators
of `req/142` §1 M8-c item 1 write and cross-checks each against its `expected` field). Vector
envelope schema is `req/spec/40-architecture/46-verification-plan.md` §3.1 verbatim
(`vector_id`/`kind`/`input`/`expected`/`rust_git_sha`/`lean_toolchain`). `import Lean.Data.Json`
is Lean 4's own JSON support (used by the compiler's LSP), not Mathlib — it does not touch 46 §2's
"does not import Mathlib.CategoryTheory" (sem: SEM-lean-123) boundary (see `Desktop/GitRepo/REFERENCES.md`'s
2026-08-14 entry for the one-time API confirmation this file's design rests on).

## What each `kind` actually cross-checks, and why (P-9 — the denominator, stated once here)

`GxSpec`'s frozen (M8-b, `req/38` §86) modules are executable in exactly one place
(`GxSpec.CanonModel`, `Canon.lean`) and abstract/opaque everywhere else: `Admissible.A`,
`Invariant.I`/`J`/`K`, and `Receipt.Ledger.contains`/`InclusionProof.verifies` are all
`Prop`-valued fields with no concrete definition, because Cedar, the invariant registry and the
DSSE/Merkle primitives are explicit non-goals of the Lean formalisation (46 §1). So there are two
different rigour classes among the five kinds, both real differential tests, neither claimed to be
the other (`req/145_M8C_IMPL_REPORT_2026-08-14.md` states this as the M8-c denominator):

* **`canon_idempotence` / `repr_independence`** — literal independent recomputation. The vector's
  `input` is fed to `GxSpec.CanonModel.canon` (the frozen, proof-complete, `#guard`-checked
  executable model) and the *result* is compared to Rust's `expected` field for structural (key-order)
  equality. A divergence here means the two independently-arrived-at sort/dedupe algorithms
  (Rust's real `gx_canon::cbor::encode` + a real `BTreeMap`, Lean's `CanonModel.ins`) disagree.
* **`admissibility` / `invariant_composition` / `receipt_verify`** — a *law* check over real
  generated data plus a *declared* class/predicate, invoking the frozen definitions
  (`GxSpec.composable`, `GxSpec.OrderBounded`, `GxSpec.HoareTriple`) directly rather than
  reimplementing them, but evaluated against a predicate this file declares (`OrderBounded n`,
  `digestBelow t`, a `List`-backed ledger) — **not** gx-gate's real Cedar/invariant-registry
  verdicts and **not** gx-witness's real DSSE/Merkle verification, both of which are exercised by
  those crates' own existing suites instead. The generators' own module docs
  (`crates/gx-gate/tests/gate_conformance_gen.rs`, `crates/gx-witness/tests/
  witness_conformance_gen.rs`) name the specific reason each kind cannot be the stronger form
  (E-M3-5 / the empty shipped registry / 46 §1's crypto non-goal, respectively).

### §89 correction (no-delete — the two bullets above stand unedited; this paragraph narrows their
wording rather than replacing it. `req/38` §89 ruling 1/ruling 2, `req/147` §2/§3.) (sem: SEM-lean-124)

**A-1 (canon tier)**: "independent recomputation" above is precise only for the *rank-sort* half.
`CanonModel.Field.key` is a Rust-precomputed rank (`Nat`), not the key's raw byte sequence — see
`Canon.lean`'s `CanonModel` namespace doc, which already says the byte sequence itself is never
handled here. The key→rank correspondence is decided once, Rust-side, and never independently
re-derived in Lean; a hand-built vector whose rank inverts the true DAG-CBOR bytewise order passes
either way (`req/147` §2 A-1a, measured: two reversed `{"aa","b"}`-labelling vectors both PASS). No
effect on the present corpus (fixed-length keys only, `req/145` gotcha6) — this narrows the *tier
claim*, not a live defect. Independent bytewise key comparison is **v0.2 reserved**
(`46-verification-plan.md` §8, `DR-46-2`).

**A-2 (law-check tier)**: `admissibility`/`invariant_composition`'s `law_holds` (§2/§3 below, at each
binding site) is a mathematical tautology of the declared class — true for every input, not a
falsifiable closure evaluation (exact identities at each `law_holds` binding; `req/147` §3 A-2).
What these two kinds actually cross-check is the **field-level** cross-language agreement,
confirmed live by `req/147` §2 A-1b's 5/5 field-injection detection. `receipt_verify`'s T4 law is
the same shape one level up: `witness_conformance_gen.rs`'s `receipt := claimed[0]` makes
`conclusion_should_hold` a structural necessity of the generator, one step stronger than
`req/145`'s "tautological on this corpus" disclosure (sem: SEM-lean-125) (`req/147` §3). **Refined 5-kind split**: independent
recomputation = 2 (`canon_idempotence`/`repr_independence`, rank-level only per A-1 above) +
field-match = 3 (`admissibility`/`invariant_composition`/`receipt_verify`, `law_holds` tautological
in all three — not a falsifiable "law-check" in the sense the tier name suggests). Whether a
declared class can be made non-tautological (closure not structurally guaranteed) is **v0.2
reserved** (`46-verification-plan.md` §8, `DR-46-3`).

### v0.2-c consumption of both reservations (`req/150` §C — the §89 correction above is kept
verbatim as the v0.1 record; this paragraph is what the file now does)

**`DR-46-2` implemented (§1 below)**: every canon-kind field now carries `key_bytes` (the key's
raw UTF-8 bytes) next to the v0.1 rank, and this runner *independently* re-derives the canonical
order from those bytes (`keyBytesLt`: length-first, then bytewise — the abstract-sequence
restatement of 42 §2.1-2's encoded-header order for text keys, no CBOR header re-implementation,
so 46 §1's non-goal stands) and cross-checks it against both the rank-sorted result and Rust's
`expected`. The `req/147` §2 A-1a inverted-labelling vector — which v0.1 passed both ways — now
diverges: the rank labelling is no longer received on trust (v0.1 rank path retained unchanged =
doubled as acceptance + independent recount (sem: SEM-lean-126); the frozen `CanonModel` is untouched).

**`DR-46-3` implemented (§2b below)**: a sixth kind, `step_bounded`, declares a class
(`StepBounded k`, first-digest-byte movement ≤ k) whose closure under `compose`'s src/dst shape
is **not** structurally guaranteed — `law_holds` is genuinely false on real generated inputs
(distribution measured and gated generator-side, `crates/gx-gate/tests/gate_conformance_gen.rs`).
For this kind `law_holds` is cross-checked as **data** (equality with Rust's evaluated value),
never asserted as truth — the M3-05 ruling (closure-as-truth only for multiplicative classes)
is preserved, because the multiplicative-class-limited kind remains `admissibility` and
`step_bounded` asserts nothing about the law's truth. The `#guard` lines at `stepLawHolds` are
the compile-time witness that the valuation is not a tautology of the class.
-/

namespace GxSpec.Runner

open Lean (Json)

-- ===========================================================================================
-- § 0. JSON field access (the four accessors confirmed empirically before this file was
-- written — see the REFERENCES.md entry cited above — plus the two-argument helpers layered on
-- top of them).
-- ===========================================================================================

def objField (j : Json) (k : String) : Except String Json :=
  j.getObjVal? k

def natField (j : Json) (k : String) : Except String Nat := do
  let v ← objField j k
  v.getNat?

def strField (j : Json) (k : String) : Except String String := do
  let v ← objField j k
  v.getStr?

def boolField (j : Json) (k : String) : Except String Bool := do
  let v ← objField j k
  v.getBool?

def arrField (j : Json) (k : String) : Except String (Array Json) := do
  let v ← objField j k
  v.getArr?

-- ===========================================================================================
-- § 1. `canon_idempotence` / `repr_independence` — literal recomputation against
-- `GxSpec.CanonModel` (`lean/GxSpec/Canon.lean`, frozen), plus, since v0.2-c (`DR-46-2`), an
-- *independent* re-derivation of the canonical order from the raw key bytes carried in
-- `key_bytes` — the half A-1a proved the rank-only path could not check. The frozen model is
-- untouched: the rank path below still runs exactly as in v0.1, and the byte path is a second,
-- independent computation cross-checked against the same `expected`.
-- ===========================================================================================

/-- A canon-kind field as carried since v0.2-c: the v0.1 Rust-precomputed rank (`key`), the raw
UTF-8 bytes of the key (`key_bytes`, `DR-46-2`), and the value tag. -/
structure FieldB where
  rank : Nat
  keyBytes : List Nat
  val : Nat

def natListField (j : Json) (k : String) : Except String (List Nat) := do
  let arr ← arrField j k
  arr.toList.mapM (·.getNat?)

def parseFieldB (j : Json) : Except String FieldB := do
  let rank ← natField j "key"
  let keyBytes ← natListField j "key_bytes"
  let val ← natField j "val"
  pure { rank := rank, keyBytes := keyBytes, val := val }

def parseFieldsFrom (j : Json) (k : String) : Except String (List FieldB) := do
  let arr ← arrField j k
  arr.toList.mapM parseFieldB

/-- The rank projection: what the v0.1 vector carried, fed to the frozen `CanonModel` unchanged. -/
def toFlat (l : List FieldB) : GxSpec.CanonModel.Flat :=
  l.map (fun f => { key := f.rank, val := f.val })

/-- `Field` derives `DecidableEq` only (`Canon.lean`, not touched by this file); a `Bool`
comparison is written directly over the two `Nat` projections instead of asking for a `BEq
CanonModel.Field` instance that was never declared. -/
def fieldsEq : GxSpec.CanonModel.Flat → GxSpec.CanonModel.Flat → Bool
  | [], [] => true
  | a :: as, b :: bs => a.key == b.key && a.val == b.val && fieldsEq as bs
  | _, _ => false

def fieldsToString (l : GxSpec.CanonModel.Flat) : String :=
  "[" ++ String.intercalate "," (l.map (fun f => s!"({f.key},{f.val})")) ++ "]"

/-- Strict bytewise lexicographic order on equal-footing byte lists. -/
def bytesLexLt : List Nat → List Nat → Bool
  | [], [] => false
  | [], _ :: _ => true
  | _ :: _, [] => false
  | a :: as, b :: bs => if a < b then true else if b < a then false else bytesLexLt as bs

/-- `DR-46-2`'s independent order: length first, then bytewise. This is the abstract-sequence
restatement of 42 §2.1-2's canonical *encoded-header* order for text-string keys (the major-type-3
header sorts any shorter key's encoding before any longer key's, then content bytes decide — see
`crates/gx-canon/src/cbor.rs`'s `map` doc), written over the raw byte sequence alone: no CBOR
header bytes are re-implemented here, so 46 §1's non-goal (no byte-level algorithm verification
in Lean) stands. Derived here independently; Rust's generator derives the same order from the
real encoder's per-key output — that pair is what makes the comparison a differential test. -/
def keyBytesLt (a b : List Nat) : Bool :=
  if a.length < b.length then true
  else if b.length < a.length then false
  else bytesLexLt a b

/-- Byte-path insertion, mirroring the frozen `CanonModel.ins`'s shape (first-inserted wins on
equal keys) with `keyBytesLt` in place of the rank order. -/
def insB (f : FieldB) : List FieldB → List FieldB
  | [] => [f]
  | g :: t =>
      if keyBytesLt f.keyBytes g.keyBytes then f :: g :: t
      else if f.keyBytes == g.keyBytes then f :: t
      else g :: insB f t

/-- The `DR-46-2` independent canonicalisation: sort by the raw key bytes, not by the received
rank. A rank labelling that inverts the true bytewise order (the `req/147` §2 A-1a attack) makes
this path and the rank path disagree with `expected`, which is exactly the divergence v0.1 could
not produce. -/
def canonB : List FieldB → List FieldB
  | [] => []
  | f :: t => insB f (canonB t)

def fieldBEq (a b : FieldB) : Bool :=
  a.rank == b.rank && a.keyBytes == b.keyBytes && a.val == b.val

def fieldsBEq : List FieldB → List FieldB → Bool
  | [], [] => true
  | a :: as, b :: bs => fieldBEq a b && fieldsBEq as bs
  | _, _ => false

def bytesToString (l : List Nat) : String :=
  String.intercalate "." (l.map toString)

def fieldsBToString (l : List FieldB) : String :=
  "[" ++ String.intercalate "," (l.map (fun f => s!"({f.rank},{bytesToString f.keyBytes},{f.val})"))
    ++ "]"

def checkCanonIdempotence (input expected : Json) : Except String (Bool × String) := do
  let inFields ← parseFieldsFrom input "fields"
  let expFields ← parseFieldsFrom expected "fields"
  -- v0.1 rank path, unchanged: the frozen model re-sorts the received ranks.
  let computed := GxSpec.CanonModel.canon (toFlat inFields)
  -- T3-a itself: canon(canon(x)) = canon(x), independently recomputed (not assumed from the
  -- frozen `canon_idem` theorem, which only says this always holds -- here it is *evaluated*).
  let idempotent := GxSpec.CanonModel.canon computed
  let rankOk := fieldsEq computed (toFlat expFields) && fieldsEq idempotent (toFlat expFields)
  -- v0.2-c byte path (`DR-46-2`): the canonical order re-derived from the raw key bytes alone,
  -- compared (rank labels included) against the same Rust `expected` — plus its own evaluated
  -- idempotence. Disagreement between the two paths surfaces here as one of them failing the
  -- shared `expected`.
  let byteCanon := canonB inFields
  let byteIdem := canonB byteCanon
  let byteOk := fieldsBEq byteCanon expFields && fieldsBEq byteIdem byteCanon
  pure (rankOk && byteOk, s!"lean_rank_canon={fieldsToString computed} \
    lean_byte_canon={fieldsBToString byteCanon} rust_expected={fieldsBToString expFields}")

def checkReprIndependence (input expected : Json) : Except String (Bool × String) := do
  let reprAJ ← objField input "repr_a"
  let reprBJ ← objField input "repr_b"
  let reprA ← parseFieldsFrom reprAJ "fields"
  let reprB ← parseFieldsFrom reprBJ "fields"
  let expFields ← parseFieldsFrom expected "fields"
  -- v0.1 rank path, unchanged.
  let canonA := GxSpec.CanonModel.canon (toFlat reprA)
  let canonB' := GxSpec.CanonModel.canon (toFlat reprB)
  let rankOk := fieldsEq canonA canonB' && fieldsEq canonA (toFlat expFields)
  -- v0.2-c byte path (`DR-46-2`), same shape as checkCanonIdempotence above.
  let byteA := canonB reprA
  let byteB := canonB reprB
  let byteOk := fieldsBEq byteA byteB && fieldsBEq byteA expFields
  pure (rankOk && byteOk, s!"lean_rank_canon(repr_a)={fieldsToString canonA} \
    lean_byte_canon(repr_a)={fieldsBToString byteA} lean_byte_canon(repr_b)={fieldsBToString byteB} \
    rust_expected={fieldsBToString expFields}")

-- ===========================================================================================
-- § 2. `admissibility` (T1) — `GxSpec.composable` / `GxSpec.OrderBounded`, the frozen
-- non-vacuity witness's own declared class (`Admissible.lean`), invoked directly.
-- ===========================================================================================

/-- A single-byte digest tag lifted into a real `GxSpec.ObjectSnapshot`. The generator's own
module doc explains why one byte is a faithful instance of the abstract F0 model. -/
def snapOf (b : Nat) : GxSpec.ObjectSnapshot :=
  ⟨ByteArray.mk #[UInt8.ofNat b]⟩

def transformationOf (order srcB dstB : Nat) : GxSpec.Transformation :=
  { id := ⟨ByteArray.mk #[]⟩, order := order, src := snapOf srcB, dst := snapOf dstB }

instance orderBoundedDecidable (n : Nat) (t : GxSpec.Transformation) :
    Decidable (GxSpec.OrderBounded n t) :=
  show Decidable (t.order ≤ n) from inferInstance

instance composableDecidable (f g : GxSpec.Transformation) : Decidable (GxSpec.composable f g) :=
  show Decidable (f.dst = g.src) from inferInstance

def checkAdmissibility (input expected : Json) : Except String (Bool × String) := do
  let n ← natField input "n"
  let fj ← objField input "f"
  let gj ← objField input "g"
  let f := transformationOf (← natField fj "order") (← natField fj "src_digest")
    (← natField fj "dst_digest")
  let g := transformationOf (← natField gj "order") (← natField gj "src_digest")
    (← natField gj "dst_digest")
  let composable := decide (GxSpec.composable f g)
  let admitF := decide (GxSpec.OrderBounded n f)
  let admitG := decide (GxSpec.OrderBounded n g)
  -- The composed transformation's shape, by `GxSpec.compose`'s own formula (src := f.src,
  -- dst := g.dst, order := max f.order g.order) -- built directly rather than through
  -- `GxSpec.compose` itself, which is `noncomputable` (its `id` field routes through the
  -- axiom `composeId`, `Core.lean`) and so cannot be evaluated by an executable runner. The
  -- `id` field is immaterial to `OrderBounded`, which reads only `.order`.
  let composedT : GxSpec.Transformation :=
    { id := ⟨ByteArray.mk #[]⟩, order := Nat.max f.order g.order, src := f.src, dst := g.dst }
  let admitCompose := decide (GxSpec.OrderBounded n composedT)
  let expComposable ← boolField expected "composable"
  let expAdmitF ← boolField expected "admit_f"
  let expAdmitG ← boolField expected "admit_g"
  let expAdmitCompose ← boolField expected "admit_compose"
  let ok := composable == expComposable && admitF == expAdmitF && admitG == expAdmitG
    && admitCompose == expAdmitCompose
  -- T1's law itself, over the real generated data (not the frozen theorem's hypothesis --
  -- an evaluated instance of it): composable ∧ admit_f ∧ admit_g → admit_compose.
  -- §89 correction (`req/38` §89 ruling 2, `req/147` §3 A-2, no-delete -- the note above stands): (sem: SEM-lean-127)
  -- `admitCompose` decides `OrderBounded n composedT` and `composedT.order = max f.order g.order`
  -- (two lines up), so `admitF ∧ admitG → admitCompose` unfolds to
  -- `f.order ≤ n ∧ g.order ≤ n → max f.order g.order ≤ n` -- a `Nat.max`-closure identity, true
  -- for every `n`/`f`/`g`, not an evaluated instance of a law that could fail. `lawHolds` below is
  -- therefore always `true`; the cross-language check this `kind` actually performs is the four
  -- `Bool` field comparisons above (`ok`), confirmed live by `req/147` §2 A-1b's field-injection.
  let lawHolds := !(composable && admitF && admitG) || admitCompose
  pure (ok && lawHolds, s!"lean composable={composable} admit_f={admitF} admit_g={admitG} \
    admit_compose={admitCompose} law_holds={lawHolds}")

-- ===========================================================================================
-- § 2b. `step_bounded` (v0.2-c, `DR-46-3`) — a declared class whose closure is NOT structurally
-- guaranteed, making `law_holds` genuinely falsifiable (the resolution of `req/38` §89 A-2 /
-- `req/147` §3). Unlike §2/§3, `law_holds` here is cross-checked as *data* (equality with
-- Rust's independently evaluated value) and never conjoined into `ok` as a truth — asserting it
-- would be wrong (the law really fails on real inputs) and would violate the M3-05 ruling
-- (closure-as-truth only for multiplicative classes; this class is deliberately not one).
-- ===========================================================================================

/-- |a − b| on `Nat` — the distance the declared class bounds. -/
def natDist (a b : Nat) : Nat := max a b - min a b

/-- The declared class (`DR-46-3`): "the first digest byte moves by at most `k` across the
transformation". Real function of the real generated digest bytes, computed identically and
independently on both sides — and, unlike `OrderBounded`/`digestBelow`, **not** closed under
`compose`'s src/dst shape: `f` within `k` and `g` within `k` bound the composed movement only by
`2k`, so membership of the composition genuinely fails on real inputs. -/
def StepBounded (k : Nat) (f : GxSpec.Transformation) : Prop :=
  natDist (f.src.digest.data.getD 0 0).toNat (f.dst.digest.data.getD 0 0).toNat ≤ k

instance stepBoundedDecidable (k : Nat) (f : GxSpec.Transformation) :
    Decidable (StepBounded k f) :=
  show Decidable (natDist _ _ ≤ k) from inferInstance

/-- The law valuation over digest bytes `a → b → c` with bound `k`, in one executable line —
what `checkStepBounded` computes vectorwise, exposed standalone so the two `#guard`s below can
pin its non-tautology at compile time. -/
def stepLawHolds (k a b c : Nat) : Bool :=
  !(natDist a b ≤ k && natDist b c ≤ k) || natDist a c ≤ k

-- The compile-time witnesses (`DR-46-3` / AC-V2C-1): the valuation really takes both values —
-- false on 0→1→2 with k=1 (each step within 1, the composition moves 2), true when the chain
-- folds back. An A-2-style tautology could not pass the first line.
#guard stepLawHolds 1 0 1 2 == false
#guard stepLawHolds 1 0 1 0 == true

def checkStepBounded (input expected : Json) : Except String (Bool × String) := do
  let k ← natField input "k"
  let fj ← objField input "f"
  let gj ← objField input "g"
  let f := transformationOf 0 (← natField fj "src_digest") (← natField fj "dst_digest")
  let g := transformationOf 0 (← natField gj "src_digest") (← natField gj "dst_digest")
  let composable := decide (GxSpec.composable f g)
  let withinF := decide (StepBounded k f)
  let withinG := decide (StepBounded k g)
  -- The composed transformation's shape, `GxSpec.compose`'s own formula -- see the
  -- noncomputability note in `checkAdmissibility` above.
  let composedT : GxSpec.Transformation :=
    { id := ⟨ByteArray.mk #[]⟩, order := 0, src := f.src, dst := g.dst }
  let withinCompose := decide (StepBounded k composedT)
  let lawHolds := !(composable && withinF && withinG) || withinCompose
  let expComposable ← boolField expected "composable"
  let expWithinF ← boolField expected "within_f"
  let expWithinG ← boolField expected "within_g"
  let expWithinCompose ← boolField expected "within_compose"
  let expLawHolds ← boolField expected "law_holds"
  -- `lawHolds == expLawHolds`, not `&& lawHolds` (section doc): false is a legitimate value the
  -- two languages must *agree on*, and its presence in the real corpus is `DR-46-3`'s point.
  let ok := composable == expComposable && withinF == expWithinF && withinG == expWithinG
    && withinCompose == expWithinCompose && lawHolds == expLawHolds
  pure (ok, s!"lean composable={composable} within_f={withinF} within_g={withinG} \
    within_compose={withinCompose} law_holds={lawHolds}")

-- ===========================================================================================
-- § 3. `invariant_composition` (T2) — `GxSpec.HoareTriple`, a declared `digestBelow` class.
-- ===========================================================================================

/-- The declared invariant class: "the snapshot's digest byte is below a threshold". A real
function of the real (generated) digest byte, computed identically and independently on both
sides -- not gx-gate's real invariant registry, which ships empty by default
(`crates/gx-gate/src/invariant.rs`; see `gate_conformance_gen.rs`'s module doc). -/
def digestBelow (t : Nat) (x : GxSpec.ObjectSnapshot) : Prop :=
  (x.digest.data.getD 0 0).toNat < t

instance digestBelowDecidable (t : Nat) (x : GxSpec.ObjectSnapshot) :
    Decidable (digestBelow t x) :=
  show Decidable ((x.digest.data.getD 0 0).toNat < t) from inferInstance

/-- `GxSpec.HoareTriple I f J` unfolds (by its own definition, `Invariant.lean`) to
`I f.src → J f.dst`; this evaluates that arrow directly as a `Bool` from the two decided halves
rather than asking instance search to look *through* the `HoareTriple` name for a class whose
type parameters are themselves partially-applied functions (`digestBelow t`), which Lean's
instance search does not resolve reliably (measured: `lake build` rejects a `Decidable
(HoareTriple ..)` instance stated the direct way; `hoareTriple` below is definitionally the same
proposition, so nothing about *what* is checked changes, only how the `Bool` is produced). -/
def hoareTriple (i j : Bool) : Bool := !i || j

def checkInvariantComposition (input expected : Json) : Except String (Bool × String) := do
  let tI ← natField input "t_i"
  let tJ ← natField input "t_j"
  let tK ← natField input "t_k"
  let fj ← objField input "f"
  let gj ← objField input "g"
  let f := transformationOf 0 (← natField fj "src_digest") (← natField fj "dst_digest")
  let g := transformationOf 0 (← natField gj "src_digest") (← natField gj "dst_digest")
  let composable := decide (GxSpec.composable f g)
  let tripleF := hoareTriple (decide (digestBelow tI f.src)) (decide (digestBelow tJ f.dst))
  let tripleG := hoareTriple (decide (digestBelow tJ g.src)) (decide (digestBelow tK g.dst))
  -- The composed transformation's shape, `GxSpec.compose`'s own formula -- see the same
  -- noncomputability note in `checkAdmissibility` above.
  let composedT : GxSpec.Transformation :=
    { id := ⟨ByteArray.mk #[]⟩, order := 0, src := f.src, dst := g.dst }
  let composedHolds :=
    hoareTriple (decide (digestBelow tI composedT.src)) (decide (digestBelow tK composedT.dst))
  let expComposable ← boolField expected "composable"
  let expTripleF ← boolField expected "triple_f_holds"
  let expTripleG ← boolField expected "triple_g_holds"
  let expComposed ← boolField expected "composed_holds"
  let ok := composable == expComposable && tripleF == expTripleF && tripleG == expTripleG
    && composedHolds == expComposed
  -- T2's law, evaluated: {I}f{J} ∧ {J}g{K} → {I}(g∘f){K}, over the composed shape above.
  -- §89 correction (`req/38` §89 ruling 2, `req/147` §3 A-2, no-delete -- the note above stands): (sem: SEM-lean-127)
  -- `tripleF`/`tripleG`/`composedHolds` all route through the same `mid`-independent
  -- `hoareTriple` shape (`!i || j`) over shared `tI`/`tJ`/`tK`, so
  -- `tripleF ∧ tripleG → composedHolds` is a hypothetical syllogism (`(a→b) ∧ (b→c) → (a→c)`) --
  -- a propositional tautology, true for every `tI`/`tJ`/`tK`/digest byte, not an evaluated
  -- instance of a law that could fail. `lawHolds` below is therefore always `true`; the
  -- cross-language check this `kind` actually performs is the four `Bool` field comparisons
  -- above (`ok`), confirmed live by `req/147` §2 A-1b's field-injection.
  let lawHolds := !(composable && tripleF && tripleG) || composedHolds
  pure (ok && lawHolds, s!"lean composable={composable} triple_f={tripleF} triple_g={tripleG} \
    composed_holds={composedHolds} law_holds={lawHolds}")

-- ===========================================================================================
-- § 4. `receipt_verify` (T4 + T5) — a declared, `List`-backed ledger. `GxSpec.Ledger`/
-- `InclusionProof` are `Prop`-valued with no concrete definition (46 §1's crypto non-goal), so
-- this section computes T4/T5's law directly over real generated `(tag, verdict)` data rather
-- than instantiating those abstract structures (`witness_conformance_gen.rs`'s module doc).
-- ===========================================================================================

def verdictEq : GxSpec.Verdict → GxSpec.Verdict → Bool
  | .admit, .admit => true
  | .deny, .deny => true
  | .escalate, .escalate => true
  | _, _ => false

def parseVerdict (s : String) : Except String GxSpec.Verdict :=
  match s with
  | "admit" => pure .admit
  | "deny" => pure .deny
  | "escalate" => pure .escalate
  | other => throw s!"unknown verdict {other}"

def parsePair (j : Json) : Except String (Nat × GxSpec.Verdict) := do
  let t ← natField j "t"
  let vs ← strField j "v"
  let v ← parseVerdict vs
  pure (t, v)

def parsePairs (j : Json) : Except String (List (Nat × GxSpec.Verdict)) := do
  let arr ← j.getArr?
  arr.toList.mapM parsePair

/-- `Ledger.contains`'s declared concrete form: exact `(tag, verdict)` membership. -/
def containsPair (entries : List (Nat × GxSpec.Verdict)) (t : Nat) (v : GxSpec.Verdict) : Bool :=
  entries.any (fun (et, ev) => et == t && verdictEq ev v)

/-- T5's actual shape is existential over the verdict (`∃ v1 v2, L.contains t1 v1 ∧ ...`), so the
tag alone decides membership here -- the verdict paired with `t1`/`t2` in the vector's `entries`
is not itself part of the check. -/
def containsTag (entries : List (Nat × GxSpec.Verdict)) (t : Nat) : Bool :=
  entries.any (fun (et, _) => et == t)

def checkReceiptVerify (input expected : Json) : Except String (Bool × String) := do
  let entries ← objField input "entries" >>= parsePairs
  let claimed ← objField input "claimed" >>= parsePairs
  let (rt, rv) ← objField input "receipt" >>= parsePair
  let recoverJ ← objField input "recover"
  let t1 ← natField recoverJ "t1"
  let t2 ← natField recoverJ "t2"

  -- --- T4: ProofSound (claimed ⊆ entries, by exact pair) ∧ is_admit ∧ ValidReceipt (r ∈
  -- claimed, true by the generator's own construction) → contains(r.t, r.v). ---
  let proofSound := claimed.all (fun (ct, cv) => containsPair entries ct cv)
  let isAdmit := verdictEq rv .admit
  let rContains := containsPair entries rt rv
  let t4Law := !(isAdmit && proofSound) || rContains
  let t4J ← objField expected "t4"
  let expIsAdmit ← boolField t4J "is_admit"
  let expProofSound ← boolField t4J "proof_sound"
  let expContains ← boolField t4J "contains"
  let expT4Holds ← boolField t4J "conclusion_should_hold"
  let t4Ok := isAdmit == expIsAdmit && proofSound == expProofSound && rContains == expContains
    && t4Law == expT4Holds

  -- --- T5: recover(tcomp) = some(t1,t2) → contains(tcomp,_) → ∃v1 v2, contains(t1,v1) ∧
  -- contains(t2,v2). Constructed by the same by-construction style
  -- `RecoverableChainWitness` (`Receipt.lean`, frozen) already uses for its own non-vacuity
  -- witness, so the tag-existence checks below are expected to hold on every vector. ---
  let t1Contains := containsTag entries t1
  let t2Contains := containsTag entries t2
  let recovers := t1Contains && t2Contains
  let t5J ← objField expected "t5"
  let expT1 ← boolField t5J "t1_contains"
  let expT2 ← boolField t5J "t2_contains"
  let expRecovers ← boolField t5J "recovers"
  let t5Ok := t1Contains == expT1 && t2Contains == expT2 && recovers == expRecovers

  pure (t4Ok && t5Ok,
    s!"t4[is_admit={isAdmit} proof_sound={proofSound} contains={rContains} law={t4Law}] \
      t5[t1_contains={t1Contains} t2_contains={t2Contains} recovers={recovers}]")

-- ===========================================================================================
-- § 5. dispatch + divergence reporting + `main`.
-- ===========================================================================================

/-- 46 §3.1's kind → theorem mapping, restated so a divergence report names the theorem under
question without a reader having to cross-reference the spec by hand. -/
def theoremIdFor : String → String
  | "canon_idempotence" => "T3-a (GxSpec.CanonModel.canon_idem, literal recompute)"
  | "repr_independence" => "T3-b (GxSpec.CanonModel.canonicalizer_reprIndependent, literal recompute)"
  | "admissibility" => "T1 (GxSpec.T1_composition_sound, declared OrderBounded class)"
  | "step_bounded" =>
      "T1-family (declared StepBounded class, non-multiplicative -- law_holds falsifiable, DR-46-3)"
  | "invariant_composition" => "T2 (GxSpec.T2_invariant_composition, declared digestBelow class)"
  | "receipt_verify" => "T4/T5 (GxSpec.T4_receipt_soundness / T5_witness_recovery, declared ledger)"
  | other => s!"unknown-kind:{other}"

structure Outcome where
  vectorId : String
  kind : String
  ok : Bool
  detail : String

def checkLine (line : String) : Except String Outcome := do
  let j ← Json.parse line
  let vectorId ← strField j "vector_id"
  let kind ← strField j "kind"
  let input ← objField j "input"
  let expected ← objField j "expected"
  let (ok, detail) ←
    match kind with
    | "canon_idempotence" => checkCanonIdempotence input expected
    | "repr_independence" => checkReprIndependence input expected
    | "admissibility" => checkAdmissibility input expected
    | "step_bounded" => checkStepBounded input expected
    | "invariant_composition" => checkInvariantComposition input expected
    | "receipt_verify" => checkReceiptVerify input expected
    | other => throw s!"unrecognised kind '{other}'"
  pure { vectorId, kind, ok, detail }

/-- `conformance/divergences/<date>-<vector_id>.md`, in 46 §3.3's four-part form: (a) the vector
verbatim (b) Rust's `expected` (already inside (a)) (c) Lean's computed detail (d) the theorem ID. -/
def writeDivergence (conformanceDir date : String) (line : String) (o : Outcome) : IO Unit := do
  let dir : System.FilePath := conformanceDir ++ "/divergences"
  IO.FS.createDirAll dir
  let path := dir / s!"{date}-{o.vectorId}.md"
  let body := s!"# divergence: {o.vectorId} ({o.kind})\n\n\
    Identity (sem: SEM-lean-128): `req/142` §1 M8-c item 3 / 46 §3.3 (divergence triage protocol). Generated by \
    `lean/GxSpec/Runner.lean`.\n\n\
    ## (a) vector (verbatim JSONL line, Rust's `expected` is inside it)\n\n\
    ```json\n{line}\n```\n\n\
    ## (c) Lean's independently computed detail\n\n\
    ```\n{o.detail}\n```\n\n\
    ## (d) theorem under question\n\n{theoremIdFor o.kind}\n\n\
    ## classification (46 §3.3 step 3, to be filled by Fable adjudication)\n\n\
    (a) Lean model coverage gap / (b) Rust implementation bug / (c) vector defect — unclassified.\n"
  IO.FS.writeFile path body

structure KindTally where
  total : Nat := 0
  pass : Nat := 0

def bump (t : KindTally) (ok : Bool) : KindTally :=
  { total := t.total + 1, pass := t.pass + (if ok then 1 else 0) }

partial def processFile (conformanceDir date path : String) (tallies : Std.HashMap String KindTally)
    (failures : Nat) : IO (Std.HashMap String KindTally × Nat) := do
  let contents ← IO.FS.readFile path
  let lines := (contents.splitOn "\n").filter (fun l => !l.trim.isEmpty)
  let mut tallies := tallies
  let mut failures := failures
  for line in lines do
    match checkLine line with
    | .error e =>
      IO.eprintln s!"RUNNER_PARSE_ERROR file={path} error={e}"
      failures := failures + 1
    | .ok o =>
      tallies := tallies.insert o.kind (bump (tallies.getD o.kind {}) o.ok)
      if !o.ok then
        IO.eprintln s!"RUNNER_DIVERGENCE vector_id={o.vectorId} kind={o.kind} detail={o.detail}"
        writeDivergence conformanceDir date line o
        failures := failures + 1
  pure (tallies, failures)

/-- `runMain` does the actual work; `main` below is a one-line unwrap of it, kept outside this
namespace only because Lean's `lean_exe` codegen needs the entry point to be literally named
`main` in the *root* namespace (see the note at `main`'s own definition, below). -/
def runMain (args : List String) : IO UInt32 := do
  match args with
  | date :: conformanceDir :: files@(_ :: _) =>
    let start ← IO.monoMsNow
    let mut tallies : Std.HashMap String KindTally := {}
    let mut failures := 0
    for f in files do
      let (t, fl) ← processFile conformanceDir date f tallies failures
      tallies := t
      failures := fl
    let elapsed ← IO.monoMsNow
    for (kind, t) in tallies.toList do
      IO.println s!"RUNNER_KIND kind={kind} total={t.total} pass={t.pass} fail={t.total - t.pass}"
    let total := (tallies.toList.map (fun (_, t) => t.total)).foldl (· + ·) 0
    IO.println s!"RUNNER_TOTAL total={total} divergences={failures} wall_ms={elapsed - start}"
    pure (if failures == 0 then 0 else 1)
  | _ =>
    IO.eprintln "usage: runner <date> <conformance-dir> <file1.jsonl> [<file2.jsonl> ...]"
    pure 2

end GxSpec.Runner

/-- `lake exe runner <date> <conformance-dir-absolute> <file1.jsonl> [<file2.jsonl> ...]`. The
conformance directory is passed explicitly (not derived from CWD) because `lake exe` runs with
whatever directory it was invoked from as CWD, not this file's own location (`req/142` §3's
WSL2/Windows bridge is `conformance/` at the repo root, not at `lean/`).

**Not namespaced** (unlike everything above, `runMain` included): Lean's `lean_exe` codegen only
emits the C `int main` wrapper for a definition literally named `main` in the *root* namespace —
the empirical gotcha this file's own build history hit (see
`req/145_M8C_IMPL_REPORT_2026-08-14.md`'s gotcha section: leaving `main` inside `namespace
GxSpec.Runner` compiles and *links* to a mystifying Windows-only `undefined symbol: WinMain`,
because with no recognised `main` present mingw's linker falls back to the GUI-subsystem CRT
entry point instead of failing on the real cause). -/
def main (args : List String) : IO UInt32 :=
  GxSpec.Runner.runMain args
