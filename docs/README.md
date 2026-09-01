<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# docs/

Eight documents and three articles. Read them in the order below if you are new; jump straight to
[`LIMITS.md`](LIMITS.md) if what you want to know is what this project cannot do.

---

| Document | What it answers |
| :--- | :--- |
| [`TUTORIAL.md`](TUTORIAL.md) | How to make a change through `gx`, get a receipt, and verify it offline — the shortest path from nothing to a checkable artifact. |
| [`LIMITS.md`](LIMITS.md) | What is **out of scope by construction**, and why each boundary cannot be closed from the inside. The one document to read before trusting any claim on the front page. |
| [`ERROR_TAXONOMY.md`](ERROR_TAXONOMY.md) | Every refusal kind, its exit code, and what a caller should do about it. Refusals are values here, not panics, so this is the full vocabulary. |
| [`RECOVERABILITY.md`](RECOVERABILITY.md) | What "reversible" is defined to mean, state by state — including the states from which a change cannot be put back. |
| [`TRACEFOLD_TR.md`](TRACEFOLD_TR.md) | The technical report: the calculus, the encoding rules, and the receipt format in full. |
| [`DEVELOPMENT_TREE_TESTS.md`](DEVELOPMENT_TREE_TESTS.md) | The taxonomy of the test probes, and what each family is actually measuring. |
| [`API_SDK_LEDGER.md`](API_SDK_LEDGER.md) | What the public HTTP surface and the TypeScript SDK actually cover — a registry, not a pitch: one row per asset, where its number came from, and whether anything turns red if it drifts. |
| `README.md` | This page. |

## Articles

Longer-form pieces, written to be read on their own rather than as reference material.

| Article | Subject |
| :--- | :--- |
| [`articles/verify-ai-agent-actions-offline.md`](articles/verify-ai-agent-actions-offline.md) | The mechanics behind offline receipt verification and tamper detection — flip one byte, the verifier says no. |
| [`articles/tamper-evident-receipts.md`](articles/tamper-evident-receipts.md) | What a receipt has to carry before it proves anything to someone who does not trust the issuer. |
| [`articles/tamper-evident-audit-trails-compared.md`](articles/tamper-evident-audit-trails-compared.md) | How this approach differs from audit logs written after the fact. |

---

## What is not here

Machine-checked statements live in [`lean/`](../lean) rather than in prose, and
[`lean/README.md`](../lean/README.md) says which are proved and which are not. Crate-level
documentation lives beside each crate under [`crates/`](../crates). Neither is duplicated here,
so that a claim has one place to be corrected.
