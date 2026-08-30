# lean/GxSpec — Mathlib selective-import list (RSK-10 discharged) (sem: SEM-lean-129)

Identity: the concretization of `req/spec/30-requirements/35-open-questions.md` RSK-10 (pinning
the Lean toolchain / mathlib supply chain) and `req/spec/40-architecture/46-verification-plan.md`
§6 (the mathlib line's "minimize dependencies" policy). This file records the selective-import
list for the Mathlib package declared as a lake dependency (46 §6: `import Mathlib` in full is
forbidden; only the module a needed lemma belongs to is selectively imported). (sem: SEM-lean-130)

## Current state (as of M8-a) (sem: SEM-lean-131)

**Current import count = 0**. (sem: SEM-lean-132)

Per `46-verification-plan.md` §2's policy, `lean/GxSpec/{Core,Admissible,Invariant,Canon,
Receipt}.lean` model F0's core directly with plain inductive/structure definitions, without
importing Mathlib.CategoryTheory. As a result the lakefile (`lean/lakefile.toml`) does not
declare Mathlib itself as a dependency -- `lean/lake-manifest.json`'s `packages` array is empty
(no Mathlib dependency = no supply-chain pin target exists, which is the current correct state).
(sem: SEM-lean-133)

## Future addition conditions (verbatim from 46 §6) (sem: SEM-lean-134)

Only if a Mathlib lemma (e.g. `DecidableEq`-related, `List` lemmas) becomes necessary for a
proof in Canon.lean/Invariant.lean etc. (the actual T3-T5 proofs, M8-b) is the Mathlib module that
lemma belongs to selectively imported (at the granularity of e.g. `import Mathlib.Data.List.Basic`).
`import Mathlib` (in full) remains forbidden. (sem: SEM-lean-135)

When adding a Mathlib dependency, append the following to this file: (sem: SEM-lean-136)

- The added module's name (e.g. `Mathlib.Data.List.Basic`)
- The reason for adding it (which T / which lemma)
- Mathlib's pinned commit hash at the time of addition (from `lake-manifest.json`'s corresponding
  package entry)
- Whether the adding PR updates lean-toolchain (46 §6: a lakefile lock update is allowed only in
  the same PR as a toolchain update)
(sem: SEM-lean-137)

## Candidates (not yet added, reference record only) (sem: SEM-lean-138)

First candidates if a proof wall is hit when M8-b starts (46 §6's own examples): (sem: SEM-lean-139)

- `Mathlib.Data.List.Basic` (if a List-related lemma becomes necessary)
- A `DecidableEq`-related module (Core.lean's `deriving DecidableEq` is currently covered by Lean
  core alone and needs no addition; consider only if the proof side needs an extension)
(sem: SEM-lean-140)

Neither is settled; nothing is added until it is actually needed (YAGNI, 46 §6 policy).
(sem: SEM-lean-141)
