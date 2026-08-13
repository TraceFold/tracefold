//! Inclusion and consistency proofs, and the bytes a checkpoint signature covers.
//!
//! Spec: 42 §3.11 for `ConsistencyProof` and `Checkpoint`, 32 FR-022/FR-023 for what must be
//! provable, 34 AC-022/AC-023 for how that is judged, RFC 6962 §2.1.1 and §2.1.2 for the two
//! algorithms 42 §3.11 inherits along with the domain separation, and — for the **verifying**
//! halves, which RFC 6962 states only as prose — **RFC 9162 §2.1.3.2 and §2.1.4.2**, whose numbered
//! steps [`reconstruct_root`] and [`verify_consistency`] transcribe. Hand 7 fetched RFC 9162 from
//! the RFC Editor and compared the two symbol by symbol (`req/58_M2_HAND7_AUDIT_2026-08-08.md` §2);
//! the correspondence is `fn` = `node`, `sn` = `last`, `r` = `hash`, `HASH` = `node_hash`, which is
//! BLAKE3 rather than SHA-256 (35 DR-3, a declared and asserted difference — AC-024).
//!
//! # Three rulings shape this file
//!
//! * **E-M2-8** — [`verify_consistency`] takes the [`ConsistencyProof`]. AC-023's
//!   `verify_consistency(root_0, root_1)` is the erratum: two roots carry no information about how
//!   one grew into the other, so the two-argument form could only ever return `true`.
//! * **E-M2-19** — a checkpoint signature covers `{origin, tree_size, root_hash}` and not
//!   `timestamp`, which stays on the struct as an unsigned advisory field (CM-5, clock-free signed
//!   payload). [`checkpoint_signing_bytes`] is where that stops being a sentence.
//! * **E-M2-26** (`req/38_ERRATA_2026-08-07.md` §15) — those bytes are the signed **core**, and the
//!   message is `PAE(payload_type, core)`. The type and the message belong to gx-witness; what stays
//!   here is the core, unchanged byte for byte by the erratum.
//!
//! # Why generation and verification do not share code
//!
//! [`prove_inclusion`] walks the tree; [`verify_inclusion`] walks the *index*, holding only the
//! leaf, the path and the two sizes. They are written separately on purpose. A verifier expressed
//! in terms of the prover would pass whenever the prover was self-consistent, which is exactly the
//! failure `gx-canon`'s `is_canonical` avoids by asking the encoder instead of the parser
//! (ASM-01-2). Here the same discipline is what makes the negatives of AC-022 and AC-023 mean
//! something: the verifier has no access to the tree the proof came from.

use gx_canon::cbor;
use gx_core::{
    Checkpoint, Cid, DsseSignature, InclusionProof, Timestamp, VerdictCheckpoint, VerdictKind,
    VerdictTally,
};
use serde::{Deserialize, Serialize};

use crate::tile::{
    audit_path_len, node_hash, size_to_usize, subproof, LedgerEntry, LedgerLeaf, TileLog,
};
use crate::{Error, Result};

/// A proof that one tree grew out of another (42 §3.11).
///
/// Field order is bytewise-sorted for the same reason [`crate::tile::LedgerLeaf`]'s is; the set is
/// 42 §3.11's `{ old_size, new_size, path }`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConsistencyProof {
    pub new_size: u64,
    pub old_size: u64,
    /// The nodes of the old tree that the new tree's root must be reachable from. Empty when the
    /// two sizes are equal — a tree is consistent with itself and needs no witness for it.
    pub path: Vec<Cid>,
}

// ---------------------------------------------------------------------------
// Inclusion (FR-022, AC-022)
// ---------------------------------------------------------------------------

/// An inclusion proof for `index` against the log as it stands.
///
/// # Errors
/// [`Error::OutOfRange`] when the log has not reached `index`.
pub fn prove_inclusion(log: &TileLog, index: u64) -> Result<InclusionProof> {
    prove_inclusion_at(log, index, log.len())
}

/// An inclusion proof for `index` against the tree of exactly `tree_size` leaves.
///
/// The log's own prefix is a tree (`TileLog::root_at`), so a proof can be issued against a
/// checkpoint the log has already passed — which is what a reader holding an older signed head
/// needs. A proof names its `tree_size` for that reason: it verifies against one root and no
/// other, and AC-022's truncation case is exactly a proof presented against the wrong one.
///
/// # Errors
/// [`Error::OutOfRange`] when `index` is not inside a tree of `tree_size` leaves, or when the log
/// has not reached `tree_size`.
pub fn prove_inclusion_at(log: &TileLog, index: u64, tree_size: u64) -> Result<InclusionProof> {
    if tree_size == 0 || tree_size > log.len() || index >= tree_size {
        return Err(Error::OutOfRange { index, tree_size });
    }
    let size = size_to_usize(tree_size, "tree size")?;
    let at = size_to_usize(index, "leaf index")?;

    // 🔴 **FR-M7-2 案 A** (`req/38` §56 追加裁定 a): the walk is the same walk and the fold under it
    // reads the tile cache, so a proof costs `O(n/256)` node hashes instead of `O(n)`. Nothing about
    // the **proof** changed — same siblings, same order, same three fields on the wire — which is
    // what `tests/incremental_inclusion.rs` measures against an independent transcription of
    // RFC 6962 §2.1.1 and what the two arms of `benches/inclusion_proof.rs` measure across two
    // builds (one fingerprint over every proof produced; the arms print the same value).
    let mut path = Vec::new();
    log.audit_path_at(at, size, &mut path);

    Ok(InclusionProof {
        leaf_index: index,
        tree_size,
        audit_path: path,
    })
}

/// Does `entry` sit at `proof.leaf_index` of a tree of `proof.tree_size` leaves whose root is
/// `root`? (AC-022)
///
/// The leaf hash is **recomputed** from the entry's fields; `entry.leaf_cid` is never read. An
/// entry whose contents and whose cached hash were both edited to agree is the tamper this
/// forecloses, and it is the one a verifier that trusted the cache would accept.
///
/// `Ok(false)` covers every way the answer is no — a wrong root, a wrong entry, a path that is too
/// long or too short, an entry whose index the proof does not name. They are one answer because a
/// verifier that distinguished them would be telling an attacker which part of the forgery to fix.
///
/// # Errors
/// [`Error::Canon`] if the entry's leaf has no canonical form, i.e. it could never have been
/// appended at all. That is a different statement from 「this proof does not hold」, so it is a
/// different return.
pub fn verify_inclusion(proof: &InclusionProof, root: &Cid, entry: &LedgerEntry) -> Result<bool> {
    verify_inclusion_of(proof, root, &entry.leaf())
}

/// The same question, asked of a leaf rather than of an entry (hand 5, AC-070).
///
/// [`verify_inclusion`] already reads nothing else from a [`LedgerEntry`]: `leaf_cid` is
/// deliberately ignored and `appended_at` is metadata that no leaf hash covers (ASM-4). So this is
/// the whole of that function, and the entry form is a wrapper.
///
/// # Why the smaller argument is the one that matters
///
/// A third party verifying a `CommitReceipt` offline (AC-070) has the receipt and a checkpoint and
/// **not** the log. 42 §3.11 makes `LedgerLeaf` `{transformation, receipt_digest, index}`, and a
/// receipt carries all three — so the leaf can be rebuilt from the receipt while a `LedgerEntry`
/// cannot be, since its `leaf_cid` and `appended_at` exist only in the log. Taking a `LedgerEntry`
/// would have forced `gx-witness` to fabricate two fields to ask a question that does not depend on
/// them, and a fabricated `leaf_cid` inside a verifier is a lie in the data even when it is unread.
///
/// # Errors
/// [`Error::Canon`] if the leaf has no canonical form.
pub fn verify_inclusion_of(proof: &InclusionProof, root: &Cid, leaf: &LedgerLeaf) -> Result<bool> {
    if leaf.index != proof.leaf_index {
        return Ok(false);
    }
    // **H2-5** (`req/38_ERRATA_2026-08-07.md` §11): 「(leaf_index, tree_size) から path 長は
    // 数学的に一意——hash 計算の**前に**長さ照合し、不一致は即 reject」. Before this the walk below
    // reached the same answer, but only after hashing every element a sender chose to include, so
    // the cost of refusing a proof was set by the proof. `audit_path_length.rs` asserts both the
    // answers and — from the source, because the return value cannot show it — this ordering.
    //
    // # Why this gate decides nothing the RFC's own loop would have decided differently
    //
    // Hand 7 owed a derivation rather than an assertion (`req/58_M2_HAND7_AUDIT_2026-08-08.md` §2).
    // Write `L(m, n)` for `audit_path_len(m, n)` and take a path of `k` siblings.
    //
    // 1. In RFC 9162 §2.1.3.2 the pair `(fn, sn)` is advanced by step 4.b.ii and step 4.c, and
    //    **neither reads a path element** — the elements are only hashed. So the trajectory of
    //    `(fn, sn)`, and therefore the iteration at which `sn` first becomes 0, is a function of
    //    `(m, n)` alone and not of what the sender sent.
    // 2. Every iteration ends in `sn >>= 1` and is entered only when `sn >= 1`, so `sn` strictly
    //    decreases until it reaches 0. Call the iteration count at which that happens `L'(m, n)`.
    // 3. Hence `k < L'` fails at step 5 (`sn != 0`), `k > L'` fails at step 4.a (`sn == 0` with
    //    elements left), and only `k == L'` can reach step 5 with `sn == 0`. The RFC's two
    //    length checks are one check, spelled twice.
    // 4. `L' = L`. Both count the levels of RFC 6962 §2.1's split recursion: `PATH(m, D[n])`
    //    contributes exactly one sibling per level and stops at `n == 1`, and the `fn`/`sn` counters
    //    are that recursion written as index arithmetic. `audit_path_len` computes it by the
    //    recursion directly, and `tile.rs`'s `the_lawful_length_is_the_generated_length` checks the
    //    two agree for every `(index, size)` with `size <= 64`.
    //
    // So the gate changes **when** a wrong-length path is refused (before any hash, in O(log n)
    // integer operations) and never **whether**. What follows from 3 is also why the two `None`
    // arms in `reconstruct_root` below are unreachable from this call site: hand 7 measured that by
    // deleting them, and nothing went red (req/58 §2, mutation M5, and §4 の起票).
    if audit_path_len(proof.leaf_index, proof.tree_size) != Some(proof.audit_path.len()) {
        return Ok(false);
    }
    let hash = crate::tile::leaf_hash(leaf)?;
    Ok(reconstruct_root(proof, &hash).is_some_and(|reached| reached == *root))
}

/// Walk the audit path from the leaf to a root (RFC 6962 §2.1.1's verification).
///
/// The walk is driven by two counters rather than by the tree: `node` is the index of the current
/// node within its level and `last` is the index of the last node of that level. Their parity says
/// which side the sibling is on, and `node == last` marks the ragged right edge, where a node is
/// carried up without a sibling. `None` when the path does not fit the tree the proof declares —
/// too long (`last` reaches 0 with siblings left over) or too short (`last` is still non-zero when
/// they run out).
fn reconstruct_root(proof: &InclusionProof, leaf: &Cid) -> Option<Cid> {
    if proof.tree_size == 0 || proof.leaf_index >= proof.tree_size {
        return None;
    }
    let mut node = proof.leaf_index;
    let mut last = proof.tree_size - 1;
    let mut hash = *leaf;

    for sibling in &proof.audit_path {
        if last == 0 {
            return None;
        }
        if node % 2 == 1 || node == last {
            hash = node_hash(sibling, &hash);
            while node != 0 && node.is_multiple_of(2) {
                node /= 2;
                last /= 2;
            }
        } else {
            hash = node_hash(&hash, sibling);
        }
        node /= 2;
        last /= 2;
    }

    if last == 0 {
        Some(hash)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Consistency (FR-023, AC-023)
// ---------------------------------------------------------------------------

/// A proof that the tree of `new_size` leaves contains the tree of `old_size` (RFC 6962 §2.1.2).
///
/// # Errors
/// [`Error::Malformed`] when the sizes cannot describe growth: `old_size == 0` (an empty tree has
/// no root to be consistent with), `old_size > new_size` (that is truncation, and no proof of it
/// exists), or a `new_size` the log has not reached.
pub fn prove_consistency(log: &TileLog, old_size: u64, new_size: u64) -> Result<ConsistencyProof> {
    if old_size == 0 {
        return Err(Error::Malformed {
            detail: "an empty tree has no root to be consistent with".to_string(),
        });
    }
    if old_size > new_size {
        return Err(Error::Malformed {
            detail: format!(
                "old size {old_size} is larger than new size {new_size}; an append-only log does \
                 not shrink, so there is no proof to give"
            ),
        });
    }
    if new_size > log.len() {
        return Err(Error::Malformed {
            detail: format!("the log holds {} entries, not {new_size}", log.len()),
        });
    }

    let new = size_to_usize(new_size, "new size")?;
    let old = size_to_usize(old_size, "old size")?;

    let mut path = Vec::new();
    if old < new {
        subproof(old, &log.leaf_hashes()[..new], true, &mut path);
    }

    Ok(ConsistencyProof {
        new_size,
        old_size,
        path,
    })
}

/// Does the tree whose root is `new_root` contain the tree whose root is `old_root`? (AC-023,
/// **E-M2-8** for the signature.)
///
/// Both roots are rebuilt from the path — the old one and the new one, from the same nodes — and
/// the answer is `true` only when both come out right and the walk lands exactly on the root. A
/// check of the new root alone would accept a proof that reached it from some other old tree.
///
/// # Errors
/// [`Error::Malformed`] when the proof's own sizes cannot describe growth. A malformed proof is
/// not a false statement about two trees; it is not a statement about them at all.
pub fn verify_consistency(
    proof: &ConsistencyProof,
    old_root: &Cid,
    new_root: &Cid,
) -> Result<bool> {
    if proof.old_size == 0 {
        return Err(Error::Malformed {
            detail: "a consistency proof from an empty tree describes nothing".to_string(),
        });
    }
    if proof.old_size > proof.new_size {
        return Err(Error::Malformed {
            detail: format!(
                "the proof claims {} leaves grew into {}; an append-only log does not shrink",
                proof.old_size, proof.new_size
            ),
        });
    }
    if proof.old_size == proof.new_size {
        // A tree is consistent with itself, and needs no witness. A path here would be a witness
        // to something the sizes do not claim.
        return Ok(proof.path.is_empty() && old_root == new_root);
    }

    // The old tree's rightmost leaf sits at `node`; shifting away the odd bits climbs to the
    // highest node that is entirely inside the old tree, which is the first thing the proof can
    // name. When nothing is left to climb (`node == 0`) the old tree is a perfect subtree and its
    // own root is the starting point, so the path does not repeat it.
    let mut node = proof.old_size - 1;
    let mut last = proof.new_size - 1;
    while node % 2 == 1 {
        node /= 2;
        last /= 2;
    }

    let mut path = proof.path.iter();
    let (mut old_reached, mut new_reached) = if node == 0 {
        (*old_root, *old_root)
    } else {
        match path.next() {
            Some(first) => (*first, *first),
            None => return Ok(false),
        }
    };

    for sibling in path {
        if last == 0 {
            return Ok(false);
        }
        if node % 2 == 1 || node == last {
            // A node on the old tree's right edge: both reconstructions take the same sibling on
            // the left, which is what ties the two roots to one path.
            old_reached = node_hash(sibling, &old_reached);
            new_reached = node_hash(sibling, &new_reached);
            while node != 0 && node.is_multiple_of(2) {
                node /= 2;
                last /= 2;
            }
        } else {
            // Only the new tree grows to the right here; the old tree ended before this node.
            new_reached = node_hash(&new_reached, sibling);
        }
        node /= 2;
        last /= 2;
    }

    Ok(last == 0 && old_reached == *old_root && new_reached == *new_root)
}

// ---------------------------------------------------------------------------
// The checkpoint's signed core (E-M2-19)
// ---------------------------------------------------------------------------

/// What a checkpoint signature covers: `{origin, tree_size, root_hash}` (**E-M2-19**).
///
/// A private mirror of the three signed fields of 42 §3.11's `Checkpoint`, in bytewise field order
/// so the encoded map is canonical as written (42 §2.1-2). It exists because the signed message
/// has to be a value in its own right — signing 「the struct minus two fields」 by convention is how
/// two implementations end up signing two different messages.
#[derive(Debug, Serialize)]
struct CheckpointCore<'a> {
    origin: &'a str,
    root_hash: Cid,
    tree_size: u64,
}

/// The **core** a checkpoint signature covers (**E-M2-19**, CM-5) — not the message it travels in.
///
/// `timestamp` and `signature` are excluded. The clock because a signed payload that carries one
/// binds the signature to when the head was written down rather than to what the head says — the
/// same reasoning E-M2-6 applied to `ReceiptPayload.issued_at`, and E-M2-19 extends here for
/// internal consistency: 「同じ原則が receipt にだけ効いて checkpoint に効かない状態は説明不能」.
/// The signature because a message cannot contain itself.
///
/// # These bytes are signed inside a pre-authentication encoding (**E-M2-26**)
///
/// Hand 5 signed this byte string directly. `req/38_ERRATA_2026-08-07.md` §15 ruled that off: the
/// message is `PAE(`「application/vnd.glovrex.checkpoint+dagcbor」`, these bytes)`, so that a
/// checkpoint signature and a receipt signature made under one key are separated by a
/// length-prefixed payload type rather than by two encodings that happen not to collide.
/// `gx_witness::dsse::checkpoint_signing_message` is that message and this function is its body.
///
/// The name is left as it is on purpose. What a signature *covers* is exactly these bytes — the PAE
/// adds a frame around them and covers no other field — so the name stays true, and AC-021's public
/// surface snapshot keeps its meaning across the erratum instead of absorbing a rename that says
/// nothing new.
///
/// # This function does not sign
///
/// Producing a `DsseSignature` needs the PAE encoding and an Ed25519 key, both of which are
/// `gx-witness` (hand 5, AC-018..AC-020). What this hand owes is the core; a caller that signs
/// these bytes has made a claim this crate cannot check, and a `Checkpoint` whose `signature`
/// field is filled in is not thereby verified.
///
/// # Errors
/// [`Error::Canon`] if the core has no canonical form.
pub fn checkpoint_signing_bytes(checkpoint: &Checkpoint) -> Result<Vec<u8>> {
    let core = CheckpointCore {
        origin: &checkpoint.origin,
        root_hash: checkpoint.root_hash,
        tree_size: checkpoint.tree_size,
    };
    Ok(cbor::encode(&core)?)
}

/// The head a log states about itself, before anybody has signed it (**H2-4**).
///
/// req/38 §11 逐語: 「H2-4（Checkpoint 生成関数の不在）→ 手 3: store が木の状態を持って初めて
/// 作れる。unsigned 生成（署名 core byte 列は手 2 の checkpoint_core が既設）を手 3・
/// DsseSignature 装着は手 5」.
///
/// The three signed fields come from the tree — `tree_size` from its length and `root_hash` from
/// its fold, so a head cannot claim a size the log has not reached or a root it does not have.
/// `origin` and `timestamp` come from the caller: the first is the log's namespace (42 §3.11) and
/// the second is a clock, which 41 §6 keeps out of every layer below the engine.
///
/// # What is signed here: nothing
///
/// `signature` is left empty, and empty is not a signature: Ed25519 makes 64 bytes and AC-019 asks
/// a verifier to reject a malformed envelope, so a head that came out of this function is refused
/// by anything that checks one. Making `Checkpoint.signature` an `Option` would say the same thing
/// in the type, and would be a change to 42 §3.11's field table that no ruling has made. Hand 5
/// attaches the real signature over [`checkpoint_signing_bytes`] of this very value (E-M2-19),
/// carried inside a pre-authentication encoding since E-M2-26.
///
/// # The argument is the tree, not the store
///
/// A `LedgerStore` holds a `TileLog` and so does a caller who never opened a file; the head is a
/// statement about the tree either way. Taking the smaller of the two means the durable path has
/// no second implementation of it (`store.log()` is the whole of the difference).
///
/// # Errors
/// [`Error::Malformed`] for an empty log. `TileLog::root` answers `None` there — a log of zero
/// entries has no head worth publishing — and a checkpoint with no root would be a signed
/// statement about nothing.
pub fn unsigned_checkpoint(
    log: &TileLog,
    origin: &str,
    timestamp: Timestamp,
) -> Result<Checkpoint> {
    let root_hash = log.root().ok_or_else(|| Error::Malformed {
        detail: "an empty log has no head to publish".to_string(),
    })?;
    Ok(Checkpoint {
        origin: origin.to_string(),
        tree_size: log.len(),
        root_hash,
        timestamp,
        signature: DsseSignature {
            keyid: String::new(),
            sig: Vec::new(),
        },
    })
}

// ---------------------------------------------------------------------------
// FR-M04 -- the verdict checkpoint's signed core, its mint, and the chain audit (M7 hand 6)
// ---------------------------------------------------------------------------

/// What a verdict checkpoint's signature covers (**FR-M04**).
///
/// The same shape as [`CheckpointCore`] and for the same reason: signing 「the struct minus two
/// fields」 by convention is how two implementations end up signing two different messages, so the
/// signed core is a value in its own right. `timestamp` and `signature` are outside it -- CM-5,
/// which E-M2-19 already extended from the receipt to the tree head, and which has no reason to
/// stop here.
///
/// Fields are declared in **encoded** order (length first, letters second -- E-42-3, measured by
/// `tests/map_key_order.rs`). The encoder sorts regardless; writing them in that order is the
/// convention this crate keeps, so that a reader of the declaration is reading the map.
#[derive(Debug, Serialize)]
struct VerdictCheckpointCore<'a> {
    tally: &'a VerdictTally,
    origin: &'a str,
    window_end: u64,
    window_start: u64,
    ledger_root_hash: Option<Cid>,
    ledger_tree_size: u64,
}

/// The **core** a verdict checkpoint's signature covers (**FR-M04**) -- not the message it travels
/// in.
///
/// The message is `PAE(`「application/vnd.glovrex.verdict-checkpoint+dagcbor」`, these bytes)`, and
/// `gx_witness::dsse::verdict_checkpoint_signing_message` is that message. Public because a
/// verifier needs it: a third party holding a published count and a key has to rebuild the message
/// from something better than a formula in a doc comment.
///
/// # This function does not sign
///
/// The same standing as [`checkpoint_signing_bytes`]. A `VerdictCheckpoint` whose `signature`
/// field is filled in has not thereby been verified by anything in this crate.
///
/// # Errors
/// [`Error::Canon`] if the core has no canonical form.
pub fn verdict_checkpoint_signing_bytes(checkpoint: &VerdictCheckpoint) -> Result<Vec<u8>> {
    let core = VerdictCheckpointCore {
        tally: &checkpoint.tally,
        origin: &checkpoint.origin,
        window_end: checkpoint.window_end,
        window_start: checkpoint.window_start,
        ledger_root_hash: checkpoint.ledger_root_hash,
        ledger_tree_size: checkpoint.ledger_tree_size,
    };
    Ok(cbor::encode(&core)?)
}

/// The count a deployment states about itself, before anybody has signed it (**FR-M04**).
///
/// The counterpart of [`unsigned_checkpoint`], and it takes the same argument for the same reason:
/// the **tree**, not the store, so the durable path has no second implementation of the head.
///
/// # Infallible, where `unsigned_checkpoint` is not
///
/// That function refuses an empty log — 「a checkpoint with no root would be a signed statement
/// about nothing」. Here an empty ledger is the **interesting** case: a window in which everything
/// was refused commits nothing. So the root is an `Option` and a zero-leaf ledger produces a
/// perfectly meaningful checkpoint saying 「n refusals, nothing committed」.
///
/// # What the caller owes
///
/// `window` is `(start, end)`, and the caller is the only thing that knows them — they count
/// verdicts, and this crate never sees one. The engine holds that counter
/// (`gx_engine::Engine::verdict_checkpoint`), and [`audit_verdict_chain`] is what checks the
/// caller kept its side: the width of the window and the total of the tally are two statements of
/// one quantity, and a caller that got them different has published a checkpoint which does not
/// describe its own window.
#[must_use]
pub fn unsigned_verdict_checkpoint(
    log: &TileLog,
    origin: &str,
    window: (u64, u64),
    tally: VerdictTally,
    timestamp: Timestamp,
) -> VerdictCheckpoint {
    VerdictCheckpoint {
        origin: origin.to_string(),
        tally,
        window_start: window.0,
        window_end: window.1,
        ledger_root_hash: log.root(),
        ledger_tree_size: log.len(),
        timestamp,
        signature: DsseSignature {
            keyid: String::new(),
            sig: Vec::new(),
        },
    }
}

/// What a chain of verdict checkpoints does not add up about (**FR-M04**, AC-VC-2 / AC-VC-5).
///
/// A **derived** type: 42 assigns it no field table, so it has the standing of [`Recovery`] and of
/// `CheckerResultRef` (E-M2-17) — the derivation is written down here and an owner ruling replaces
/// it.
///
/// Every variant carries the two numbers that disagree, because a finding which says only 「this is
/// wrong」 sends its reader back to recompute what the check already knew.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChainBreak {
    /// Two consecutive checkpoints whose windows do not meet: something was published between them
    /// and is not in the chain this verifier was handed.
    Gap { closed_at: u64, reopened_at: u64 },
    /// One checkpoint whose tally does not add up to the width of its own window.
    WindowDoesNotMatchTally { window_width: u64, tally_total: u64 },
    /// A verifier holding more verdicts of some kind than the chain admits to.
    Underreported {
        kind: VerdictKind,
        claimed: u64,
        observed: u64,
    },
    /// The chain has stopped publishing while the ledger it is bound to kept growing.
    BehindTheLedger {
        admits_claimed: u64,
        ledger_tree_size: u64,
    },
    /// A chain whose bound ledger head moves backwards: checkpoints out of order, or two logs'
    /// checkpoints folded into one chain.
    LedgerWentBackwards { from: u64, to: u64 },
}

/// Audit a chain of verdict checkpoints against what a verifier can see for itself.
///
/// **Signatures are not checked here.** `gx_witness::dsse::verify_verdict_checkpoint` is that, and
/// the separation is the point: an operator who wants to under-report holds the key and will sign
/// the smaller number, so a valid signature says nothing whatever about whether the count is true.
/// What this function checks is the arithmetic no signature can rescue.
///
/// * `observed` — what the verifier counted from the receipts, journal or export it was handed. A
///   verifier holding nothing counts zero, and zero is never above a claim, so an empty `observed`
///   silently disables the AC-VC-2 half. That is honest rather than convenient: a verifier that
///   has seen no receipts cannot detect the under-reporting of receipts, and this function does
///   not pretend to do it for them.
/// * `ledger_tree_size` — the size of the commit ledger's head, which a verifier reads from a
///   signed [`Checkpoint`] rather than from the operator's word.
///
/// An empty answer means 「nothing here contradicts itself」 and never 「the counts are true」. The
/// two limits that stay open whatever this function returns are written on [`VerdictCheckpoint`].
#[must_use]
pub fn audit_verdict_chain(
    checkpoints: &[VerdictCheckpoint],
    observed: &VerdictTally,
    ledger_tree_size: u64,
) -> Vec<ChainBreak> {
    let mut found = Vec::new();
    let mut folded = VerdictTally::default();
    let mut previous_end: Option<u64> = None;
    let mut previous_ledger: Option<u64> = None;

    for checkpoint in checkpoints {
        let width = checkpoint
            .window_end
            .saturating_sub(checkpoint.window_start);
        if width != checkpoint.tally.total() {
            found.push(ChainBreak::WindowDoesNotMatchTally {
                window_width: width,
                tally_total: checkpoint.tally.total(),
            });
        }
        if let Some(end) = previous_end {
            if end != checkpoint.window_start {
                found.push(ChainBreak::Gap {
                    closed_at: end,
                    reopened_at: checkpoint.window_start,
                });
            }
        }
        if let Some(before) = previous_ledger {
            if checkpoint.ledger_tree_size < before {
                found.push(ChainBreak::LedgerWentBackwards {
                    from: before,
                    to: checkpoint.ledger_tree_size,
                });
            }
        }
        previous_end = Some(checkpoint.window_end);
        previous_ledger = Some(checkpoint.ledger_tree_size);
        folded.deny += checkpoint.tally.deny;
        folded.admit += checkpoint.tally.admit;
        folded.escalate += checkpoint.tally.escalate;
        folded.unverdicted += checkpoint.tally.unverdicted;
    }

    for (kind, claimed, seen) in [
        (VerdictKind::Deny, folded.deny, observed.deny),
        (VerdictKind::Admit, folded.admit, observed.admit),
        (VerdictKind::Escalate, folded.escalate, observed.escalate),
    ] {
        if seen > claimed {
            found.push(ChainBreak::Underreported {
                kind,
                claimed,
                observed: seen,
            });
        }
    }

    // 🔴 The tail. Every leaf of the commit ledger is downstream of an admission — 43 puts T-4
    // before T-10 on every road — so a chain that admits to fewer admissions than the ledger holds
    // leaves has stopped publishing, and an empty chain beside a non-empty ledger is the extreme
    // of the same statement. `unverdicted` counts here too: 43 T-4e reaches a commit as surely as
    // T-4a does, and leaving it out would make a fail-open deployment look like a truncated chain.
    let admitted = folded.admit + folded.unverdicted;
    if admitted < ledger_tree_size {
        found.push(ChainBreak::BehindTheLedger {
            admits_claimed: admitted,
            ledger_tree_size,
        });
    }
    found
}

// ---------------------------------------------------------------------------
// The tree the proofs are about, checked from inside the crate
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::{mth, split_point, TileLog};
    use gx_core::{Timestamp, TransformationId};

    fn cid(seed: u64) -> Cid {
        let mut raw = [0u8; 32];
        raw[..8].copy_from_slice(&seed.to_be_bytes());
        Cid(raw)
    }

    fn log_of(n: u64) -> TileLog {
        let mut log = TileLog::new();
        for i in 0..n {
            log.append(
                TransformationId(cid(i)),
                cid(1_000 + i),
                Timestamp(i as i64),
            )
            .expect("canonical");
        }
        log
    }

    /// RFC 6962 §2.1's `k`: strictly below `n`, so a left subtree is always complete.
    #[test]
    fn the_split_is_strictly_below_the_size() {
        assert_eq!(split_point(2), 1);
        assert_eq!(split_point(3), 2);
        assert_eq!(split_point(4), 2);
        assert_eq!(split_point(5), 4);
        assert_eq!(split_point(8), 4);
        assert_eq!(split_point(9), 8);
    }

    /// The root is a fold over the leaves and nothing else: rebuilding it by hand for a tree of
    /// three agrees with `mth`. Written out because a tree of three is the smallest ragged one,
    /// where a wrong split point would still produce *a* root.
    #[test]
    fn the_root_of_three_leaves_is_the_written_out_one() {
        let log = log_of(3);
        let l = log.leaf_hashes();
        let by_hand = node_hash(&node_hash(&l[0], &l[1]), &l[2]);
        assert_eq!(log.root(), Some(by_hand));
    }

    /// Appending never changes an existing leaf hash — the property the tile layout rests on.
    #[test]
    fn appending_does_not_move_an_earlier_leaf() {
        let before = log_of(7);
        let after = log_of(9);
        assert_eq!(
            before.leaf_hashes(),
            &after.leaf_hashes()[..before.leaf_hashes().len()]
        );
        assert_eq!(before.root(), after.root_at(7));
    }

    /// An empty log has no root, and every proof over it is refused rather than answered.
    #[test]
    fn an_empty_log_has_no_head() {
        let log = TileLog::new();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
        assert_eq!(log.root(), None);
        assert_eq!(log.root_at(0), None);
        assert_eq!(log.root_at(1), None);
        assert!(prove_inclusion(&log, 0).is_err());
        assert!(prove_consistency(&log, 1, 1).is_err());
    }

    /// `mth` over a prefix is `root_at` over the same size — the two roads to a historical root
    /// agree, which is what lets a proof be issued against a checkpoint the log has passed.
    #[test]
    fn a_prefix_root_is_the_root_of_the_prefix() {
        let log = log_of(11);
        for size in 1..=11usize {
            assert_eq!(
                log.root_at(size as u64),
                mth(&log.leaf_hashes()[..size]),
                "prefix of {size}"
            );
        }
    }
}
