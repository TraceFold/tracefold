// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Provenance, evidence, and the signed receipt. (sem: SEM-gx-witness-007, SEM-gx-witness-008,
//! SEM-gx-witness-009, SEM-gx-witness-010, SEM-gx-witness-011, SEM-gx-witness-012,
//! SEM-gx-witness-013, SEM-gx-witness-014, SEM-gx-witness-015, SEM-gx-witness-016,
//! SEM-gx-witness-017)
//!
//! Spec: `req/spec/40-architecture/41-architecture.md` §2 for where this crate sits and what it
//! may depend on, 42 §3.7 / §3.9 / §3.10 for the field tables it carries, 32 FR-015..FR-020 for
//! what it must do, 34 AC-015..AC-020 and AC-070 for how that is judged.
//!
//! # What this crate is for
//!
//! P-7's witness: after the gate has decided, something has to be able to say *afterwards, to a
//! third party, offline* what was decided and over what. That is a receipt -- a DSSE envelope over
//! a canonical `ReceiptPayload` -- and the provenance and evidence it refers to.
//!
//! # What is here
//!
//! 41 §2 fixes the module list as `src/{lib,provenance,receipt,evidence,dsse,keys}.rs` and the
//! hands fill it in the order req/49 §5 sets, as corrected by `req/38_ERRATA_2026-08-07.md` §8:
//!
//! | module | types (42 §0) | hand | acceptance |
//! |---|---|---|---|
//! | `provenance.rs` | `Provenance`, `Environment` | 4 (done) | AC-015 |
//! | `evidence.rs` | `Evidence` (**4 variants** -- E-M2-3 keeps 42 §3.7 and makes the `HumanDecision` references of 43/44/34/35 the erratum), `TestOutcome`, `PolicyDecision` | 4 (done) | AC-016 |
//! | `receipt.rs` | `Receipt`, `ReceiptPayload`, `ReceiptKind` | 5 (done) | AC-018, AC-070 |
//! | `dsse.rs` | `DsseEnvelope`, PAE | 5 (done) | AC-018, AC-019 |
//! | `keys.rs` | Ed25519 key generation and file-backed storage | 5 (done) | AC-020 |
//!
//! # `Proof` is published here and defined below (E-M2-12)
//!
//! FR-017 asks gx-witness for a `Proof` while 42 §0 files the nearly identical `ProofRef` under
//! gx-gate (M3). **E-M2-12** (`req/38_ERRATA_2026-08-07.md` §8) put the family in gx-core so the two
//! crates share one set of types, and this crate satisfies FR-017's MUST by re-exporting them
//! rather than by defining a second spelling. AC-017's subject is `gx_witness::Proof`, and
//! `tests/ac_017.rs` asserts that it *is* `gx_core::Proof` rather than a namesake.
//!
//! Two rulings change what those modules will hold, and are recorded here so that hands 4 and 5
//! do not have to rediscover them:
//!
//! * **E-M2-6** -- `ReceiptPayload.issued_at` leaves the signed core. 43 T-11 lists what a receipt
//!   signature covers and no clock is in it; CM-5 states the rule ("exclude reading the clock from
//!   the signed payload"). The timestamp becomes unsigned envelope metadata.
//! * **E-M2-7** -- `fail_posture_engaged: bool` is **added** to `ReceiptPayload`. 35 ASM-13 and
//!   43 T-4e require a verdict receipt to record that the fail-closed posture was engaged, and the
//!   15 fields of 42 §3.10 have nowhere to put it. It is deterministic, so it belongs *inside* the
//!   signed core -- the opposite of `issued_at`, and for the same reason.
//!
//! # What this crate must never be said to prove
//!
//! 46 §2.5's T4 (receipt soundness) and T5 (witness lax composition) are **not proved**:
//! `Receipt.lean` does not exist (`req/38_ERRATA_2026-08-07.md` §7). A receipt this crate issues
//! is a signed record of what the engine decided. It is not backed by T4, and req/49 §1 N-10 marks
//! saying otherwise as the overclaim 45 §4.1 forbids.
//!
//! # The dependency edge, and the one hand 5 added
//!
//! gx-log names gx-core and never gx-witness. 42 §3.10 and §3.11 as written would have made the two
//! crates name **each other**, and E-M2-1 moved the shared data -- `InclusionProof`, `Checkpoint`,
//! `DsseSignature`, `FingerprintBytes`, the `Proof` family -- down into gx-core so that the cycle is
//! absent from the graph rather than forbidden by a convention.
//!
//! Hand 5 adds the edge in the direction that remains: **gx-witness names gx-log**. Two obligations
//! of this hand need the log's arithmetic and cannot be met without it.
//!
//! * AC-070 asks that a `CommitReceipt` be verified "verify the signature + verify the inclusion
//!   proof (cross-check against a known checkpoint), in an offline environment". The audit-path walk
//!   is `gx_log::proof`'s and repeating it here would be
//!   a second verifier, which is exactly the drift E-M2-12 moved the `Proof` family down to avoid.
//! * **H3-7** (`req/38_ERRATA_2026-08-07.md` §12) rules "`Checkpoint.signature` ... resolved in hand
//!   5" over
//!   the byte string hand 2 built for it, `gx_log::proof::checkpoint_signing_bytes`. The ruling
//!   names a gx-witness function calling a gx-log one, so the edge was decided before this hand.
//!
//! E-M2-1 forbade the **cycle**, and cargo still refuses one: gx-log does not name gx-witness, and
//! `tests/ac_018.rs` reads its manifest to keep it that way. 41 §2's informal dep line for this
//! crate ("dep: ed25519-dalek") lists neither gx-log nor the gx-canon hand 4 added; both are raised
//! in req/54 §4 rather than treated as licence.
//!
//! # Errors are typed here (**H4-7**)
//!
//! 41 §6: "errors are typed with thiserror". Hand 4 left this crate with no `Error` at all, because
//! its one public function was infallible, and req/53 §4 H4-7 recorded that AC-019's `Err(
//! SignatureInvalid)` had "no definition exists in any canonical source". `req/38_ERRATA_2026-08-07.md`
//! §13 gave the
//! vocabulary to this hand: [`Error`] is below, `SignatureInvalid` is its variant, and the fact
//! that the word appears in 34 AC-019 and in no data model is raised in req/54 §4. 41 §2 fixes this
//! crate's module list at six and none of them is `error.rs`, so the type lives in the crate root
//! as gx-log's does.

#![forbid(unsafe_code)]
// NFR-022 (33): the receipt vocabulary is what an offline verifier reads, so every public item
// documents what it attests and under which spec table. `deny` keeps that closed the way
// gx-core's is (req/159 §B-2, req/166).
#![deny(missing_docs)]

// 🔴 **R-928-ATT** — the attach interface (`req/929`): reading what somebody else attested. It
// sits beside `coverage` deliberately — same four questions, and a vocabulary with the `Measured`
// arm removed, because a foreign document is unverified by construction in this build.
pub mod attach;
pub mod coverage;
// 🔴 **R-930-B1** — the `.gx` kind-15 body (`req/939`). It sits beside `gxfile` for the reason
// `attach` sits beside `coverage`: the vocabulary it is written in is that module's, and the rule
// it exists to satisfy (a kind names itself inside the bytes its identity covers) is enforced
// there, over the whole registry at once.
pub mod design_token;
pub mod dsse;
pub mod evidence;
// 🔴 **R-922-F2 phase 1** — the `.gx` object file. A seventh module beside `coverage`, which is
// itself a sixth beyond 41 §2's `src/{lib,provenance,receipt,evidence,dsse,keys}.rs`: the module
// list in 41 §2 has not been re-issued since P-1b added one, and this file does not re-issue it
// either. Raised in the R-922-F2 report rather than corrected here, since a spec line is a spec
// lane's to move.
pub mod gxfile;
pub mod keys;
pub mod provenance;
pub mod receipt;

// 🔴 **P-1b** — the coverage table is a projection of `ReceiptPayload` and travels with it.
pub use coverage::{
    Answer, Declared, DeclaredAnswer, FacePosture, Measured, Question, ReceiptAnswer,
    ReceiptCoverage, Unknown,
};
pub use dsse::{
    pae, DsseEnvelope, CHECKPOINT_PAYLOAD_TYPE, RECEIPT_PAYLOAD_TYPE, STANDING_PAYLOAD_TYPE,
};
pub use evidence::{Evidence, InTotoStatementRef, PolicyDecision, TestOutcome};
pub use keys::{
    KeyPair, PublicKey, Retroaction, RevocationEntry, RevocationLedger, Standing, StandingEntry,
    StandingLedger, VerifyingKeyRef,
};
pub use provenance::{Environment, Provenance, ProvenanceInputs};
pub use receipt::{
    revocation_status, verify_offline, verify_offline_against, verify_offline_consulting,
    Anchorage, Checks, InclusionCheck, Receipt, ReceiptKind, ReceiptPayload, RevocationCheck,
    RevocationPolicy, VerdictSummary, CURRENT_PAYLOAD_VERSION,
};

// FR-017's `Proof`, published rather than redefined (E-M2-12; see the crate note above).
// `VerdictKind` joins them for the same reason and by the same rule (**E-M3-2**): the type
// `ReceiptPayload.verdict` is written in belongs one layer down, or gx-gate and this crate would
// have to name each other.
pub use gx_core::{CheckerResultRef, Proof, ProofRef, TheoremId, VerdictKind};

/// Everything gx-witness can refuse to do (**H4-7**, 41 §6).
///
/// The two layers below are carried transparently rather than restated. gx-canon's `Error` already
/// names which clause of 42 §2.1 caught a value and gx-log's names which shape of proof was
/// malformed; flattening either into a string here would lose the thing req/38 §7's "crushing
/// gx-canon Error's cause into an opaque string" backlog item complains about.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// AC-019, verbatim: "Then: `Err(SignatureInvalid)`".
    ///
    /// # One variant for four failures, on purpose
    ///
    /// A flipped bit in the payload, in the payload type, in the signature, or in the `keyid` all
    /// arrive here, and [`dsse::DsseEnvelope::verify`] explains why they are not told apart: a
    /// verifier that named the part which failed would be telling a forger which part to fix. The
    /// same argument `gx_log::proof::verify_inclusion` makes for answering every bad proof with one
    /// `false`.
    ///
    /// `key_id` is the key the check was made **against**, not a key from the receipt: naming a
    /// value the attacker controls in an error message is how a log line becomes an injection.
    #[error("no valid signature under key {key_id:?} (34 AC-019)")]
    SignatureInvalid {
        /// The key the check was made **against** -- the verifier's, never one named by the
        /// (attacker-controlled) receipt. See the variant doc.
        key_id: String,
    },

    /// A receipt that is well-formed CBOR and not a legal receipt: ASM-14's kind-dependent fields
    /// (42 §3.10), a verdict kind outside the three, or a `key_id` disagreeing with the signature.
    /// AC-070's "schema invalid".
    #[error("the receipt is not a legal one: {detail}")]
    Schema {
        /// Which of ASM-14's rules the receipt broke, in words, for the human reading the
        /// refusal.
        detail: String,
    },

    /// The canonical layer refused. Carried transparently (see the type's note).
    #[error(transparent)]
    Canon(#[from] gx_canon::Error),

    /// The ledger layer refused: a malformed proof shape, or a leaf with no canonical form.
    #[error(transparent)]
    Log(#[from] gx_log::Error),

    /// The filesystem refused, in [`keys`]. Same four fields as `gx_log::Error::Io` and for the
    /// same reason: "could not open the key" and "could not fsync it" are one `ErrorKind` and two
    /// completely different facts.
    #[error("{action} ({path}): {detail}")]
    Io {
        /// What was being attempted when the filesystem refused, e.g. `"open"` or `"fsync"` --
        /// the half a bare `ErrorKind` loses.
        action: &'static str,
        /// The file involved.
        path: std::path::PathBuf,
        /// The operating system's classification of the failure.
        kind: std::io::ErrorKind,
        /// The operating system's message, verbatim.
        detail: String,
    },

    /// A key file that decoded but is not a key this version can use.
    #[error("the key file is not one this version reads: {detail}")]
    KeyFormat {
        /// What about the file's shape this version could not read, in words.
        detail: String,
    },

    /// A key file somebody other than its owner can read (unix only; see [`keys`]).
    #[error("{path} is mode {mode:o}: a secret key must not be readable by group or other")]
    KeyPermissions {
        /// The offending key file.
        path: std::path::PathBuf,
        /// Its unix mode bits as found, so the message can show what to change.
        mode: u32,
    },

    /// The operating system would not supply randomness for [`KeyPair::generate`].
    #[error("the operating system supplied no entropy for a new key: {detail}")]
    Entropy {
        /// The operating system's message, verbatim.
        detail: String,
    },

    /// 🔴 **P2 item2** (`req/130` §1) — [`keys::KeyPair::load`] found a key
    /// [`keys::KeyPair::save_encrypted`] wrote. Not [`Error::KeyFormat`]: the file is not corrupt,
    /// it is a shape this function does not read at all — [`keys::KeyPair::load_encrypted`] is the
    /// road, and the message names it rather than describing the bytes as broken.
    #[error("{path} is encrypted (key_id {key_id:?}); use load_encrypted with its passphrase")]
    KeyEncrypted {
        /// The encrypted key file `load` refused to read.
        path: std::path::PathBuf,
        /// The key's id, readable from the unencrypted header -- named so the caller knows
        /// *which* key wants its passphrase.
        key_id: String,
    },

    /// 🔴 **P2 item2** — [`keys::KeyPair::load_encrypted`]'s AEAD authentication failed: the
    /// passphrase was wrong, or the ciphertext was tampered with. The two are indistinguishable by
    /// construction (that is what an authentication tag is for), and ruling 1 asks only that this face
    /// be named apart from [`Error::KeyFormat`]'s -- a file whose *structure* this version cannot
    /// read at all, decided before any passphrase is used.
    #[error("{path}: the passphrase is wrong, or the file is corrupted (decryption did not authenticate)")]
    WrongPassphrase {
        /// The key file whose decryption did not authenticate.
        path: std::path::PathBuf,
    },
}

/// The vocabulary of [`Error`], declared once (**E-M2-23**).
///
/// # 🔴 Why this arrives in M6 rather than in M2
///
/// E-M2-23's rule is "declare, per crate, **one place** for the Error vocabulary table (the
/// substance of what E-M2-23 filed), and refuse anything outside the declaration at construction
/// time", and gx-core, gx-engine, gx-gate and gx-substrate each grew one. This crate and gx-log did
/// not, and nothing noticed until something had to **map** the vocabulary: M6-09 asks 44 §2.3's
/// twelve `gx_code`s to cover every refusal the workspace can produce, and a map without a
/// denominator is a map nobody can call complete. So the array is the denominator, and
/// `crates/gx-api/src/gx_code.rs` const-asserts its length against its own table.
///
/// In declaration order, which is also the order of the `match` in [`Error::kind`].
pub const ERROR_KINDS: [&str; 10] = [
    "SignatureInvalid",
    "Schema",
    "Canon",
    "Log",
    "Io",
    "KeyFormat",
    "KeyPermissions",
    "Entropy",
    "KeyEncrypted",
    "WrongPassphrase",
];

impl Error {
    /// Which of [`ERROR_KINDS`] this refusal is.
    ///
    /// No `_` arm: the enum is `#[non_exhaustive]` **to other crates** and exhaustive here, which is
    /// what makes this the right place for the match. A variant added tomorrow stops this function
    /// from compiling, and a consumer mapping on the string gets the compile error second-hand
    /// through the array's length.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Error::SignatureInvalid { .. } => "SignatureInvalid",
            Error::Schema { .. } => "Schema",
            Error::Canon(_) => "Canon",
            Error::Log(_) => "Log",
            Error::Io { .. } => "Io",
            Error::KeyFormat { .. } => "KeyFormat",
            Error::KeyPermissions { .. } => "KeyPermissions",
            Error::Entropy { .. } => "Entropy",
            Error::KeyEncrypted { .. } => "KeyEncrypted",
            Error::WrongPassphrase { .. } => "WrongPassphrase",
        }
    }
}

/// The crate's result alias.
pub type Result<T> = core::result::Result<T, Error>;
