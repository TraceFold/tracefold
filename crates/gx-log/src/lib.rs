// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The append-only Merkle transparency log.
//!
//! Spec: `req/spec/40-architecture/41-architecture.md` §2 for where this crate sits and what it
//! may depend on, 42 §3.11 for the hash rules and the field tables, 32 FR-021..FR-024 for what it
//! must do, 34 AC-021..AC-024 and AC-069 for how that is judged, 33 NFR-009 for the durability
//! `ledger.append` owes.
//!
//! # What this crate is for
//!
//! A receipt that only its issuer holds proves nothing to anybody else. The log is what makes a
//! commit *checkable*: entries are appended and never edited, an inclusion proof shows an entry is
//! in a tree of a stated size, and a consistency proof shows that the newer tree still contains
//! the older one. Tile layout follows Rekor v2 / C2SP tlog-tiles (42 §3.11, research/02 §3).
//!
//! # What is here
//!
//! 41 §2 fixes the module list as `src/{lib,tile,proof,store}.rs` and the hands fill it in the
//! order req/49 §5 sets:
//!
//! | module | types (42 §0) | hand | acceptance |
//! |---|---|---|---|
//! | `tile.rs` | `LedgerEntry`, `LedgerLeaf`, `Tile` | 2 (done) | AC-021, AC-024 |
//! | `proof.rs` | `ConsistencyProof`, proof generation and verification | 2 (done) | AC-022, AC-023 |
//! | `store.rs` | persistence, fsync-on-append | 3 (done) | AC-069 (NFR-009) |
//! | `head.rs` | `PersistedHead`, `HeadWitness`, `HeadFloor`, `AcceptedRollback`, `RolledBack` (R6/R7, not 42 §0) | req/230 R6 (done) | M-02 (req/232 H-01/H-02) |
//! | `witness_audit.rs` | — (detector; no new type) | req/682 Phase B (done) | AC-B1, AC-B2, AC-B3, AC-B6, AC-B7 |
//!
//! 🔴 Both rows are additive (registry gap closed, `req/964` §10 Z-B): 41 §2 fixed the module list
//! as `src/{lib,tile,proof,store}.rs` before `head.rs` (`req/230`, R6 monotonicity) and
//! `witness_audit.rs` (`req/682`, Phase B equivocation) existed; this table previously omitted
//! both even though `pub mod head` / `pub mod witness_audit` (below) already shipped them.
//!
//! The two halves are separate types on purpose. [`tile::TileLog`] is the tree: it holds leaves,
//! folds roots and answers proofs, and nothing in it survives a process. [`store::LedgerStore`] is
//! the ledger: a file, an fsync before every answer (NFR-009), a replay that rebuilds the tree, and
//! the key index that makes `append` idempotent (43 ASM-43-1). A log that is not durable is not a
//! ledger, and a tree that has to be durable cannot be tested without a filesystem — so both exist
//! and the ledger holds the tree.
//!
//! 42 §0 assigns `store.rs` no types at all, so everything in it is **derived** from the sources
//! named in req/49 §2.2 and req/38 §11's H2-3/H2-4/H2-5 rulings rather than transcribed from a
//! field table. That is the same standing as E-M2-17's `CheckerResultRef` (req/38 §9): the
//! derivation is written down in the module's own documentation, and an owner ruling replaces it.
//!
//! # The tree is gx's own
//!
//! **M2H1-6 → hand 2 ruling** (`req/38_ERRATA_2026-08-07.md` §9) rules the merkle implementation (sem: SEM-gx-log-031)
//! self-written and `rs_merkle` observation-only: gx hashes with BLAKE3 (35 DR-3) while the
//! library brings a second SHA-256 subtree into a workspace that needs none, 42 §3.11's domain
//! separation is no library's default, and its licence claim did not check out at three points.
//! The dependency was dropped when this hand landed; `tests/ac_021.rs` keeps it dropped. What is
//! transcribed in `tile.rs` and `proof.rs` is RFC 6962 §2.1, which is a specification — no line of
//! any implementation was copied.
//!
//! `InclusionProof` and `Checkpoint` are **not** in that list any more: E-M2-1
//! (`req/38_ERRATA_2026-08-07.md` §8) moved their definitions to `gx_core::ledger`, because
//! `ReceiptPayload` carries an `InclusionProof` and a `Checkpoint` carries a `DsseSignature`, which
//! as 42 writes them makes gx-witness and gx-log name each other. `ConsistencyProof` stays here:
//! nothing below this crate carries one, so moving it would be symmetry, not a cycle broken.
//!
//! Four rulings shape this crate, and each one is implemented rather than remembered:
//!
//! * **E-M2-12** -- 42 §3.11 hashes with domain separation (`leaf_hash = BLAKE3(0x00 ||
//!   canonical_dagcbor(LedgerLeaf))`, `node_hash = BLAKE3(0x01 || left || right)`) while 42 §1.1's
//!   `Cid` carries no prefix byte, and gx-canon published no raw-bytes hash at all. gx-canon now
//!   has a mint that takes the domain byte (`gx_canon::cid::{mint_leaf, mint_node}`); this crate
//!   calls it and names neither a codec nor a hash, which is what keeps 41 §6's "all canonical
//!   encoding goes through gx-canon alone" (sem: SEM-gx-log-032) true here and `ac_014.rs` meaningful.
//! * **E-M2-8** -- [`proof::verify_consistency`] takes its [`proof::ConsistencyProof`] as an
//!   argument. AC-023's two-argument `verify_consistency(root_0, root_1)` is the erratum (the same
//!   shape as E-AC059-1's `compose(g,f)`).
//! * **E-M2-9** -- AC-024's "hash algorithm ... match" (sem: SEM-gx-log-033) cannot be satisfied: gx is BLAKE3 (35 DR-3)
//!   and Rekor v2 / C2SP tlog-tiles is SHA-256. `tests/ac_024.rs` asserts the *structural*
//!   correspondence (tile height, the domain-separation bytes) and asserts the hash algorithm as a
//!   **declared difference** -- a machine-checked mismatch rather than a comparison quietly dropped.
//! * **E-M2-19** -- a checkpoint signature covers `{origin, tree_size, root_hash}`;
//!   `Checkpoint.timestamp` stays on the struct (42 §3.11) and outside the signed bytes (CM-5).
//!   [`proof::checkpoint_signing_bytes`] is that core; the signing itself is gx-witness (hand 5).
//! * **E-M2-26** (`req/38_ERRATA_2026-08-07.md` §15) -- the core is not the message. A checkpoint
//!   signature is taken over `PAE(payload_type, core)` like a receipt's, so that one key's two
//!   signing roads are told apart by a length-prefixed type inside the signed bytes rather than by
//!   two encodings happening not to collide. The type and the message are gx-witness's
//!   (`gx_witness::dsse::{CHECKPOINT_PAYLOAD_TYPE, checkpoint_signing_message}`); this crate still
//!   owns the core, and owning the core is all it owns.
//!
//! # The dependency edge
//!
//! gx-log names gx-core and never gx-witness. See the note in `gx-witness`'s crate root: the cycle
//! is absent from the graph, not forbidden by a convention someone has to remember.

#![forbid(unsafe_code)]

pub mod head;
pub mod proof;
pub mod store;
pub mod tile;
pub mod witness_audit;

pub use head::{
    AcceptedRollback, HeadFloor, HeadStore, HeadWitness, PersistedHead, RolledBack, HEAD_FILE,
    HEAD_VERSION,
};
pub use proof::ConsistencyProof;
pub use store::{AppendOutcome, LedgerStore, Recovery, MAX_RECORD_BYTES};
pub use tile::{LedgerEntry, LedgerLeaf, Tile, TileLog, TILE_WIDTH};
pub use witness_audit::{classify_extension, detect_equivocation, CheckpointContradiction};

/// The retention floor `req/spec/30-requirements/33-non-functional.md` NFR-027 states and 35
/// ASM-33-2 (§F, `req/38` §69) calls immovable: **180 days**, EU AI Act Art.19/26's "a minimum of 6 months" (sem: SEM-gx-log-034)
/// read as a number. ASM-33-2 draws a line between this floor and the retention *upper bound*
/// (the cost of unlimited retention), which stays Owner/v0.2 territory -- this constant is the
/// floor's half only.
///
/// v0.1 meets it **structurally rather than by counting days**: FR-021 requires gx-log's public
/// surface to offer no function that edits or removes an entry, and `crates/gx-log/tests/ac_021.rs`
/// (`ac_021_no_public_function_is_named_for_a_mutation`) already scans that surface and asserts
/// exactly that, for every function this crate exports -- see that file's own note that the claim
/// is about the *API a caller is handed*, not about whether the bytes on disk could be edited by a
/// tool outside this crate. An entry that no public function can remove cannot be removed before
/// 180 days because it cannot be removed through this crate at all, so the floor holds for every
/// entry ever appended. `tests/nfr_027.rs` is what a v0.2 deletion/pruning API would have to answer
/// to before it could ship without silently undercutting this constant (`req/38` §81 coverage v2,
/// v0.2.7 batch Lane B item 4). (sem: SEM-gx-log-035)
pub const NFR_027_MINIMUM_RETENTION_DAYS: u32 = 180;

/// Everything the log can refuse to do.
///
/// 41 §6 types errors with `thiserror` and counts a panic as a bug, so every refusal here is a
/// returned value. The three are kept apart because they answer different questions: the canonical
/// layer would not encode a leaf, the caller named a leaf the tree does not have, or a proof's own
/// declared shape does not describe growth. "This proof does not hold" (sem: SEM-gx-log-036) is none of them -- that is
/// `Ok(false)`, and conflating it with an error would let a caller treat a failed verification as
/// a transient fault.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The canonical layer refused. Carried transparently: gx-canon's `Error` already names which
    /// clause of 42 §2.1 caught the value, and restating it as a string here would lose that.
    #[error(transparent)]
    Canon(#[from] gx_canon::Error),

    /// A leaf index outside the tree it was asked about.
    #[error("leaf {index} is outside a tree of {tree_size} leaves")]
    OutOfRange { index: u64, tree_size: u64 },

    /// Sizes that cannot describe an append-only tree growing.
    #[error("the proof is malformed: {detail}")]
    Malformed { detail: String },

    /// The filesystem refused (hand 3, `store.rs`).
    ///
    /// The kind is carried as a value and the message as a string because this enum is `Clone`
    /// and `PartialEq` while `std::io::Error` is neither. `action` says what was being attempted,
    /// so a reader can tell "could not open the ledger" from "could not fsync it" (sem: SEM-gx-log-037) — which are the
    /// same `ErrorKind` and completely different facts about durability.
    #[error("{action} ({path}): {detail}")]
    Io {
        action: &'static str,
        path: std::path::PathBuf,
        kind: std::io::ErrorKind,
        detail: String,
    },

    /// A second, different answer under one key (43 ASM-43-1).
    ///
    /// Not a failed write and not a duplicate: the log already holds a receipt for this
    /// transformation and a different one was offered. Choosing between them is not this crate's
    /// to make, so both are named and neither is written.
    #[error(
        "transformation {transformation:?} is already in the ledger with receipt {recorded:?}; \
         {offered:?} was offered"
    )]
    Conflict {
        transformation: gx_core::TransformationId,
        recorded: gx_core::Cid,
        offered: gx_core::Cid,
    },
}

/// The vocabulary of [`Error`], declared once (**E-M2-23**).
///
/// Arrives in M6 for the reason gx-witness's does (see that array): M6-09 needs a **denominator**
/// for 44 §2.3's twelve `gx_code`s, and a refusal vocabulary nobody declared is a refusal vocabulary
/// nobody can prove was covered. `crates/gx-api/src/gx_code.rs` const-asserts against this length.
///
/// In declaration order, which is also the order of the `match` in [`Error::kind`].
pub const ERROR_KINDS: [&str; 5] = ["Canon", "OutOfRange", "Malformed", "Io", "Conflict"];

impl Error {
    /// Which of [`ERROR_KINDS`] this refusal is.
    ///
    /// No `_` arm: `#[non_exhaustive]` binds other crates and not this one, which is why the match
    /// belongs here rather than at the consumer that needs it.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Error::Canon(_) => "Canon",
            Error::OutOfRange { .. } => "OutOfRange",
            Error::Malformed { .. } => "Malformed",
            Error::Io { .. } => "Io",
            Error::Conflict { .. } => "Conflict",
        }
    }
}

/// The crate's result alias.
pub type Result<T> = core::result::Result<T, Error>;
