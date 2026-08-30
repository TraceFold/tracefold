<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-substrate

**The SubstrateAdapter boundary and its delta types.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | The boundary every substrate meets: the `SubstrateAdapter` trait, the `PlannedDelta` / `AppliedDelta` types, the shared refusal vocabulary (`Error::NotAPosition` and its siblings), and the normative rule for reading a locator. It is the point that keeps a delta's payload opaque to everything underneath the adapter. |
| **What it guarantees** | Locator normalisation is **lexical only** — it folds dot segments, removes duplicate separators and applies one trailing-separator convention, and it performs no I/O while doing so. Nothing here reads a clock, opens a file or computes a digest; the trait states what an adapter promises and implements none of it. A test asserts that absence rather than leaving it to be noticed later. |
| **What it refuses to do** | Symbolic-link and real-path resolution are **not** in this version, and the hole is named rather than papered over: a locator that reaches a protected path through a symbolic link is not closed by lexical normalisation, so an actor who can create links can still choose which spelling the gate sees. A crate that opens no file cannot be an adapter, and this one is not meant to be. |
| **How it is checked** | [`tests/`](tests) — [`substrate_contract.rs`](tests/substrate_contract.rs) asserts the absence of I/O, [`adapter_contract.rs`](tests/adapter_contract.rs) the trait's shape, [`delta_semantics.rs`](tests/delta_semantics.rs) and [`planned_delta_identity.rs`](tests/planned_delta_identity.rs) the delta types, [`scope_elision.rs`](tests/scope_elision.rs) what a scope may leave out. |

---

## Where it sits

Between [`gx-core`](../gx-core) / [`gx-canon`](../gx-canon) and the substrate adapters —
[`gx-adapter-fs`](../gx-adapter-fs), [`gx-adapter-git`](../gx-adapter-git),
[`gx-adapter-mcp`](../gx-adapter-mcp) — each of which implements this trait. The harness that
measures an implementation against this boundary is a separate crate,
[`gx-substrate-conformance`](../gx-substrate-conformance).

## Learn more

- [`src/lib.rs`](src/lib.rs) — the boundary and the normalisation clauses, stated normatively.
- [`docs/LIMITS.md`](../../docs/LIMITS.md) — what this project does not cover, workspace-wide.
