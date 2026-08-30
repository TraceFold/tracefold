<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-core

**Core types of the Glovrex transformation calculus. No I/O, deterministic.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | The workspace's shared vocabulary of types: identifiers, intents, deltas, verdicts, ledger shapes, receipt shapes. Definitions, and nothing that acts on them. |
| **What it guarantees** | No I/O, no clock, no `unsafe`. The same input always yields the same value. Nothing declared here signs, hashes, verifies or compares. |
| **What it refuses to do** | It never computes a content identifier and never encodes anything: `IdentityView` and every encoding are `gx-canon`'s, hashing and CID computation are `gx-canon`'s, signing is `gx-witness`'s. It does not know the encoding crate exists. The data comes down; the computation stays up — and because that rule is a shape in the dependency graph rather than a note someone has to remember, there is no cycle to forbid. |
| **How it is checked** | [`tests/`](tests) — [`compose.rs`](tests/compose.rs) and [`compose_range.rs`](tests/compose_range.rs) for composition, [`scope_bound.rs`](tests/scope_bound.rs) and [`value_range_closure.rs`](tests/value_range_closure.rs) for the bounds these types are allowed to take, [`core_error_vocabulary.rs`](tests/core_error_vocabulary.rs) for the refusal vocabulary being whole. |

---

## Where it sits

The floor of the workspace. `gx-core` depends on no other crate here; every other crate depends
on it. The content identifier type is defined here while the hash that fills one in is computed
in [`gx-canon`](../gx-canon).

## Learn more

- [`src/lib.rs`](src/lib.rs) — the crate's own account of why each type sits here and not elsewhere.
- [`docs/TRACEFOLD_TR.md`](../../docs/TRACEFOLD_TR.md) — the calculus these types spell out.
