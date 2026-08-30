# GxSpec

Glovrex's F0 formal specification in Lean 4 (`req/spec/10-concept/12-formal-semantics.md` F0
section). Its relationship to the Rust implementation (`crates/`) is not a transcription but an
independent model + differential testing; `req/spec/40-architecture/46-verification-plan.md` is
the canonical policy. (sem: SEM-lean-142)

## Current state (M8-b, T1-T5 proved stage) (sem: SEM-lean-143)

- `GxSpec/{Core,Admissible,Invariant,Canon,Receipt}.lean` -- definitions + the T1-T5 proofs
  (applying fix directives 1-5 of `req/38` §85's ruling: T1/T5 = definitional projection +
  non-vacuity witness, T2 = unmodified, T3 = replaced by the `CanonModel` executable model, T4 =
  the `ProofSound` hypothesis added). (sem: SEM-lean-144)
- Mathlib.CategoryTheory is not imported (46 §2 policy). The current count of Mathlib
  selective imports is 0 (see `MATHLIB_IMPORTS.md`). (sem: SEM-lean-145)
- `lean-toolchain` / `lake-manifest.json` / `MATHLIB_IMPORTS.md` are the substance of the
  supply-chain pin (RSK-10). (sem: SEM-lean-146)

This library is outside the critical path (it does not enter the runtime path), and in public
documentation it is treated as staying "Lean, future tense" (`req/116` §1) until it satisfies the
release gate: `lake build` success + zero unproven placeholders (achieved) + a green differential
test (not yet started, M8-c). (sem: SEM-lean-147)

## `DR-R34-1` stage, additive (`req/523`) (sem: SEM-lean-201)

- `GxSpec/Attribution.lean` joins the root -- the attribution invariant: whether a resuming engine
  can tell "my own apply landed" from "a third party moved the world". Same gate shape as every
  stage before it (import-reachable from `GxSpec.lean` and type-checks), same discipline
  (`req/160` §3-1's five conditions), same house form (breakage theorem paired with a recovery
  theorem on the *same* fixtures). The proved content, its denominators, and the reason each
  proposition is worth stating live in that file's header; `req/523` owns the lane record.
  (sem: SEM-lean-202)
- Nothing in the earlier stages is edited by that file. It adds no axiom (its theorems draw on
  `propext` / `Quot.sound` only, and not on `GxSpec.composeId`), so the library's axiom set is
  unchanged. (sem: SEM-lean-203)
