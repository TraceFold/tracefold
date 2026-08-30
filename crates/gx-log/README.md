# gx-log

Append-only Merkle tile log: inclusion and consistency proofs (41 §2 / 42 §3.11).

## What this crate guarantees

Quoted from `src/lib.rs`'s crate-level doc comment:

> A receipt that only its issuer holds proves nothing to anybody else. The log is what makes a
> commit *checkable*: entries are appended and never edited, an inclusion proof shows an entry
> is in a tree of a stated size, and a consistency proof shows that the newer tree still
> contains the older one. Tile layout follows Rekor v2 / C2SP tlog-tiles.

> [`store::LedgerStore`] is the ledger: a file, an fsync before every answer (NFR-009), a
> replay that rebuilds the tree, and the key index that makes `append` idempotent (43
> ASM-43-1).

> gx hashes with BLAKE3 (35 DR-3) while the library brings a second SHA-256 subtree into a
> workspace that needs none ... The dependency was dropped when this hand landed;
> `tests/ac_021.rs` keeps it dropped. What is transcribed in `tile.rs` and `proof.rs` is
> RFC 6962 §2.1, which is a specification — no line of any implementation was copied.

## What this crate does not guarantee

[`tile::TileLog`] holds leaves and answers proofs but "nothing in it survives a process" — the
tree is in-memory only; durability is `store::LedgerStore`'s, which fsyncs before every answer.

## Position

`req/spec/40-architecture/41-architecture.md` §2 (where this crate sits, what it may depend
on); `42-*.md` §3.11 (hash rules, field tables); `32-functional.md` FR-021..FR-024; `34-*.md`
AC-021..AC-024 and AC-069; `33-*.md` NFR-009 (durability `ledger.append` owes).

## Not covered

`InclusionProof` and `Checkpoint` are not defined in this crate (they live in `gx_core::ledger`,
E-M2-1) — this crate defines `ConsistencyProof` and the tile/proof/store machinery only.
