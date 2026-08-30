<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-api

**The Glovrex HTTP surface: thirteen synchronous endpoints, the gx_code map, Bearer auth and Idempotency-Key.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | The HTTP face of the engine: the synchronous endpoints, the map from refusal kinds onto stable numeric codes, static Bearer authentication with a loopback default, and a persisted `Idempotency-Key`. |
| **What it guarantees** | It holds **no semantic authority**. It performs no canonical encoding, constructs no verdict, and writes no lifecycle state — the same three absences the command line carries, checked by the same authority-boundary test. The refusal-code map lists its folds rather than implying them, the auth module states the *absence* of a check as an explicit value, and the idempotency module says in one line what it does **not** protect. |
| **What it refuses to do** | The endpoint table names fourteen rows and thirteen are implemented; the streaming endpoint, the server runtime with graceful shutdown, and the list endpoints belong to a later stage. That discrepancy is raised as a defect in the open rather than rounded off to a tidier number. |
| **How it is checked** | [`tests/`](tests) — [`endpoints.rs`](tests/endpoints.rs) and [`router.rs`](tests/router.rs) for the surface, [`auth.rs`](tests/auth.rs) and [`idempotency.rs`](tests/idempotency.rs) for the two headers, [`wire_census.rs`](tests/wire_census.rs) for every field on the wire being accounted for, [`shutdown.rs`](tests/shutdown.rs) and [`stream.rs`](tests/stream.rs) for the parts that are not finished. |

---

## Where it sits

A surface over [`gx-engine`](../gx-engine), [`gx-gate`](../gx-gate),
[`gx-witness`](../gx-witness), [`gx-log`](../gx-log) and [`gx-core`](../gx-core). It sits beside
[`gx-cli`](../gx-cli), not above it: both observe the same engine and neither extends it.

## Learn more

- [`src/lib.rs`](src/lib.rs) — the endpoint list and the three absences, from the crate's own side.
- [`docs/ERROR_TAXONOMY.md`](../../docs/ERROR_TAXONOMY.md) — what each refusal code means to a caller.
