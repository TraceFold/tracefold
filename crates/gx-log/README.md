<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-log

**Append-only Merkle tile log: inclusion and consistency proofs.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | What makes a commit checkable by someone other than its issuer. A receipt only its issuer holds proves nothing to anybody else; entries here are appended and never edited, an inclusion proof shows an entry sits in a tree of a stated size, and a consistency proof shows the newer tree still contains the older one. Tile layout follows Rekor v2 and the C2SP transparency-log tile conventions. |
| **What it guarantees** | The durable ledger fsyncs before it answers, replays the file to rebuild the tree on open, and keys an index that makes appending the same entry twice a no-op. What is transcribed from RFC 6962 is the *specification* — no line of any implementation was copied. |
| **What it refuses to do** | The in-memory tile log holds leaves and answers proofs, but nothing in it survives the process; durability belongs to the ledger store alone, and the two are not interchangeable. The inclusion-proof and checkpoint types are not defined here — they live one crate down, so that everyone shares one spelling of them. |
| **How it is checked** | [`tests/`](tests) — [`incremental_inclusion.rs`](tests/incremental_inclusion.rs) and [`tile_wire.rs`](tests/tile_wire.rs) for the proofs and the tile encoding, [`append_idempotence.rs`](tests/append_idempotence.rs) for appending the same entry twice, [`witness_offline.rs`](tests/witness_offline.rs) for verification without the issuer, [`audit_path_length.rs`](tests/audit_path_length.rs) for proof size. |

---

## Where it sits

Above [`gx-core`](../gx-core) and [`gx-canon`](../gx-canon). [`gx-witness`](../gx-witness) and
[`gx-engine`](../gx-engine) depend on it; no substrate adapter does.

## Learn more

- [`src/lib.rs`](src/lib.rs) — why an append-only log is the thing that makes a receipt checkable.
- [`src/store.rs`](src/store.rs) — the durability the in-memory tree does not have.
- [`docs/TRACEFOLD_TR.md`](../../docs/TRACEFOLD_TR.md) — the hash rules and field tables.
