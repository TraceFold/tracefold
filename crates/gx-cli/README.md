<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-cli

**The `gx` command line: `.gx/` layout, draft store, id-resolution cache.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | The `gx` binary: the on-disk `.gx/` layout, a local draft store, and a cache that resolves short identifiers to full ones. |
| **What it guarantees** | It holds **no semantic authority** — it observes engine state and maps commands onto the engine's entry points. If a command adds meaning of its own, that is an implementation defect and not a design choice. It parses textual content identifiers and never computes one, constructs no verdict, and keeps no lifecycle state of its own. |
| **What it refuses to do** | One asymmetry is stated rather than hidden: the local draft store holds state that never enters the engine's algebra, so "the command line and the HTTP surface behave identically" has to be read as *identical from the candidate stage onward*. Separately, a group of verbs whose mechanism is not part of the public distribution is switched off in the public build — the feature names stay declared, with the reason, instead of being silently dropped. |
| **How it is checked** | [`tests/`](tests) — [`receipt_verify_hermetic.rs`](tests/receipt_verify_hermetic.rs) proves offline verification touches nothing outside its inputs, [`exit_matrix_cli.rs`](tests/exit_matrix_cli.rs) pins every exit code, [`gx_layout.rs`](tests/gx_layout.rs) and [`draft_index.rs`](tests/draft_index.rs) the on-disk shape, [`secret_scan.rs`](tests/secret_scan.rs) with positive fixtures under [`tests/fixtures/`](tests/fixtures), [`limits_sync.rs`](tests/limits_sync.rs) re-counts the numbers printed on the public faces so they cannot drift silently. |

---

## Where it sits

The top surface of the workspace. It depends on the engine, the gate, the witness, the log, the
substrate boundary and all three shipped adapters, and on [`gx-api`](../gx-api) for the `serve`
path. Nothing in the workspace depends on it.

## Learn more

- [`src/lib.rs`](src/lib.rs) — the three absences and the one asymmetry, from the crate's own side.
- [`docs/TUTORIAL.md`](../../docs/TUTORIAL.md) — the commands in the order a first user meets them.
- [`docs/ERROR_TAXONOMY.md`](../../docs/ERROR_TAXONOMY.md) — what each exit code means.
