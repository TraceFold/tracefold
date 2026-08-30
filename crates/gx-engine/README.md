<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-engine

**The Transformation lifecycle: engine journal, escrow store, deterministic replay.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | The lifecycle itself: the journal that records every transition, the store that holds an escrowed inverse until it is no longer needed, and a replay that rebuilds state from the journal alone. |
| **What it guarantees** | A transformation carries no lifecycle state of its own — state lives in a table keyed by the transformation's identifier, and that table is a cache. Every transition is written to the journal **before** any side effect, which is why a crash cannot leave a change applied with nothing recording it. That ordering is the property this project exists for. |
| **What it refuses to do** | Entry points that are not implemented are **absent rather than stubbed**: a shape test fails on an extra entry point as readily as on a missing one, so the boundary is a measurement instead of an intention. Recovery is a procedure over the journal, not a transition, and not another entry point. No substrate adapter is a dependency of this crate and none may be — an engine that linked one would ship one substrate's grammar to every user of every substrate. (One adapter appears in the test configuration only, never in the built library.) |
| **How it is checked** | [`tests/`](tests) — [`crash_recovery.rs`](tests/crash_recovery.rs) and [`journal_roundtrip.rs`](tests/journal_roundtrip.rs) for the journal-before-effect ordering, [`two_phase_escrow.rs`](tests/two_phase_escrow.rs) and [`commit_protocol.rs`](tests/commit_protocol.rs) for the commit path, [`concurrent_commit.rs`](tests/concurrent_commit.rs) for two changes at once, [`store_shape.rs`](tests/store_shape.rs) for the entry-point boundary, [`sigma_replay.rs`](tests/sigma_replay.rs) for deterministic replay. |

---

## Where it sits

Above [`gx-core`](../gx-core), [`gx-canon`](../gx-canon), [`gx-substrate`](../gx-substrate),
[`gx-log`](../gx-log), [`gx-gate`](../gx-gate) and [`gx-witness`](../gx-witness) — it asks the
gate for a verdict and the witness for evidence. Below the two surfaces,
[`gx-cli`](../gx-cli) and [`gx-api`](../gx-api), which only observe it.

## Learn more

- [`src/lib.rs`](src/lib.rs) — the lifecycle, and which entry points exist today.
- [`docs/RECOVERABILITY.md`](../../docs/RECOVERABILITY.md) — what "reversible" is defined to mean here.
