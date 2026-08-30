<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-witness

**Provenance, evidence and DSSE-signed receipts.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | The part that can say *afterwards, to a third party, offline* what was decided and over what. That statement is a receipt — a DSSE envelope over a canonical payload — together with the provenance and evidence it refers to. |
| **What it guarantees** | A receipt is verifiable without the issuer: the envelope, the canonical payload and the key are all a checker needs. The proof types are shared with the gate rather than defined a second time here, so the two crates cannot drift into two spellings of the same thing. |
| **What it refuses to do** | **Receipt soundness and witness composition are not proved.** There is no Lean proof for either, and a receipt this crate issues is a signed record of what the engine decided — nothing stronger. Saying otherwise is exactly the overclaim this project forbids itself, so it is written here rather than left for a reader to discover. |
| **How it is checked** | [`tests/`](tests) — [`frozen_receipt_corpus.rs`](tests/frozen_receipt_corpus.rs) re-verifies receipts issued by earlier versions from [`tests/fixtures/frozen_receipts/`](tests/fixtures/frozen_receipts), [`checkpoint_signature.rs`](tests/checkpoint_signature.rs) and [`leaf_from_signed_bytes.rs`](tests/leaf_from_signed_bytes.rs) for the signature path, [`revocation.rs`](tests/revocation.rs) for key withdrawal, [`witness_error_vocabulary.rs`](tests/witness_error_vocabulary.rs) for the refusal set being whole. |

---

## Where it sits

Above [`gx-core`](../gx-core), [`gx-canon`](../gx-canon) and [`gx-log`](../gx-log).
[`gx-gate`](../gx-gate), [`gx-engine`](../gx-engine) and the command line consume it; the offline
verifier in the SDK checks what it produces.

## Learn more

- [`src/lib.rs`](src/lib.rs) — the receipt shape, and the sentence about what it must never be said to prove.
- [`lean/README.md`](../../lean/README.md) — what *is* machine-checked, and what is not.
- [`docs/LIMITS.md`](../../docs/LIMITS.md) — the same boundary from the project's side.
