<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-canon

**Canonical DAG-CBOR form, CID, and the JCS compatibility layer.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | Two faces, deliberately kept apart. The **wire** face encodes and decodes every field, so a value survives a round trip byte for byte. The **identity** face projects a value, encodes that projection on the wire face, and hashes it with BLAKE3. |
| **What it guarantees** | Whether bytes are canonical is decided by the encoder, never by the fact that a decoder accepted them. The projection drops the record's own id and creation time, so the same transformation recorded at two moments has one identifier. Identity is *defined* as that composition, which is what makes the projection unskippable: there is no second road to a content identifier. |
| **What it refuses to do** | It does not treat surviving a round trip as evidence of being canonical — those are two different questions on purpose. Bytes the encoder would not have produced are refused (`Error::NotCanonicalizable`) rather than accepted. Every refusal is a returned value, never a panic, including refusals caused by input a caller controls. The type definitions themselves are not here. |
| **How it is checked** | [`tests/`](tests) — [`canonical_bytes_road.rs`](tests/canonical_bytes_road.rs) and [`golden_vectors.rs`](tests/golden_vectors.rs) / [`negative_vectors.rs`](tests/negative_vectors.rs) for the encoder's decision, [`identity_view.rs`](tests/identity_view.rs) and [`hash_injectivity.rs`](tests/hash_injectivity.rs) for the projection, [`unsafe_forbidden.rs`](tests/unsafe_forbidden.rs) and [`authority_boundary.rs`](tests/authority_boundary.rs) for the two absences this crate polices workspace-wide. |

---

## Where it sits

Directly above [`gx-core`](../gx-core), which supplies the types this crate projects and encodes.
`gx-core` is not aware of this crate. Everything that needs a canonical form or an identifier —
the log, the witness, the gate, the engine — reaches it through here.

## Learn more

- [`src/lib.rs`](src/lib.rs) — the two faces and why they are separate.
- [`src/cid.rs`](src/cid.rs) — the identifier construction itself.
- [`docs/TRACEFOLD_TR.md`](../../docs/TRACEFOLD_TR.md) — the encoding rules in full.
