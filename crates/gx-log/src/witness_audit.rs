// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Offline detection of contradictions between signed checkpoints of one log.
//!
//! # What this closes, and what it does not
//!
//! `gx-log/src/head.rs` (§40-67) writes the boundary this module lives on. Against a **Model A**
//! adversary — a crash, a partial write, a stale binary, a third party who cannot write `.gx/` —
//! `head::compare` is a true detector: it holds the one recorded head and refuses a tree that went
//! backwards. Against a **Model B** adversary — one that *can* write `.gx/` — every witness inside
//! `.gx/` shares the subject's write scope and can be deleted, swapped, or rolled back to an older
//! genuine version. `req/38 §2699` (H-02) measured the core of it: a signed *older* head put back
//! verifies perfectly, because a signature says *who* and never *when*.
//!
//! What Model B cannot forge is a **contradiction between two copies that already left the machine**.
//! Two signed checkpoints of one `origin` that name the same `tree_size` but different roots are two
//! attested histories of identical length: no key, no consistency proof, and no freshness can
//! reconcile them. That is the one fact this module states, and the only one. It is the
//! commit-ledger / persisted-head analogue of what `proof::audit_verdict_chain` (§624) already does
//! for the verdict-count chain, and it lives here for the same reason that function does: the
//! judgement is **arithmetic no signature can rescue**, so it stays in gx-log and never reaches for
//! gx-witness (which would close the `ac_018` layering cycle). Signature verification is the
//! caller's job, done through `gx_witness::dsse` *before* a checkpoint is handed here.
//!
//! # Soundness
//!
//! An empty result means "nothing in this set contradicts itself", and **never** "this is the true
//! history" — the same note `audit_verdict_chain` carries (sem: SEM-gx-log-051). A set of zero or
//! one checkpoint is trivially self-consistent. The one contradiction this module cannot see is a
//! **split view** (a key signing two self-consistent chains shown to two verifiers who never meet):
//! `gx-core/src/ledger.rs:151` (ruling #14) declares that limit open, and this module does not claim
//! to close it. Closing it needs the verifiers to pool their checkpoints — which is exactly the set
//! this module then audits.

use gx_core::{Checkpoint, Cid};

use crate::proof::{verify_consistency, ConsistencyProof};
use crate::Error;
use crate::Result;

/// A contradiction between two signed checkpoints that both claim one `origin`.
///
/// Not a bool: like `head::HeadComparison::RolledBack` and `proof::ChainBreak`, a finding that says
/// only "these disagree" sends its reader back to recompute what the check already knew, so every
/// variant carries the values that disagree. `#[non_exhaustive]` because a later ruling may name a
/// third shape (e.g. a timestamped freshness fork) without breaking a matcher.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CheckpointContradiction {
    /// Two checkpoints of one `origin` sign the **same** `tree_size` to **different** roots: two
    /// attested histories of identical length. A decisive contradiction in the signed fields alone
    /// — no consistency proof exists that could make both true.
    Equivocation {
        /// The namespace both checkpoints claim.
        origin: String,
        /// The length both attest to.
        tree_size: u64,
        /// One signed root at that length.
        root_a: Cid,
        /// The other, incompatible signed root at the same length.
        root_b: Cid,
    },
    /// Two checkpoints of one `origin` at **different** sizes whose consistency proof refutes the
    /// shorter tree being a prefix of the longer: a diverged fork rather than an append-only
    /// extension.
    Fork {
        /// The namespace both checkpoints claim.
        origin: String,
        /// The smaller tree's length.
        old_size: u64,
        /// The larger tree's length.
        new_size: u64,
        /// The smaller tree's signed root.
        old_root: Cid,
        /// The larger tree's signed root, which the smaller is not a prefix of.
        new_root: Cid,
    },
}

/// Detect equivocation across a set of signed checkpoints (Model B, offline).
///
/// **Signatures are not checked here.** The caller passes only checkpoints whose signatures already
/// verified (`gx_witness::dsse::verify_checkpoint`); what this returns is the arithmetic that a
/// valid signature cannot rescue. For every pair sharing an `origin`, a matching `tree_size` with a
/// differing `root_hash` is reported as [`CheckpointContradiction::Equivocation`].
///
/// The scan is `O(n²)` in the number of checkpoints, which is the number an operator has physically
/// collected out of band — small by construction. An empty result is the soundness note above, not
/// a proof of truth.
#[must_use]
pub fn detect_equivocation(checkpoints: &[Checkpoint]) -> Vec<CheckpointContradiction> {
    let mut found = Vec::new();
    // O(n²) over a set an operator has physically collected out of band (§2-2), so `n` is small.
    // Each unordered pair is visited once: `j` starts past `i`.
    for (i, a) in checkpoints.iter().enumerate() {
        for b in &checkpoints[i + 1..] {
            // A different `origin` is a different log (42 §3.11): its size collisions carry no
            // meaning here, and folding them in would be the "one key, two namespaces" false
            // positive AC-B1b guards.
            if a.origin != b.origin {
                continue;
            }
            // Same length, different root: two attested histories of identical length. The one
            // failure a transparency log exists to make impossible, and no signature, proof, or
            // freshness reconciles it.
            if a.tree_size == b.tree_size && a.root_hash != b.root_hash {
                found.push(CheckpointContradiction::Equivocation {
                    origin: a.origin.clone(),
                    tree_size: a.tree_size,
                    root_a: a.root_hash,
                    root_b: b.root_hash,
                });
            }
        }
    }
    found
}

/// Classify whether `new` is an append-only extension of `old`, given a consistency proof.
///
/// Both must share an `origin`, `old.tree_size` must be below `new.tree_size`, and `proof` must be
/// the consistency proof over exactly those two sizes. Returns
/// [`CheckpointContradiction::Fork`] when the proof refutes the prefix relation, and `None` when it
/// confirms it. Offline with no proof a size difference is undecidable, so callers that hold no
/// proof simply do not call this — silence is not a contradiction.
///
/// # Errors
/// [`Error::Malformed`] when the two checkpoints do not share an `origin`, are not strictly
/// ordered by size, or the proof's sizes do not match the checkpoints'.
pub fn classify_extension(
    old: &Checkpoint,
    new: &Checkpoint,
    proof: &ConsistencyProof,
) -> Result<Option<CheckpointContradiction>> {
    if old.origin != new.origin {
        return Err(Error::Malformed {
            detail: format!(
                "two checkpoints under different origins ({} and {}) are not one log to compare; \
                 a checkpoint of one log carries no claim about another's",
                old.origin, new.origin
            ),
        });
    }
    if old.tree_size >= new.tree_size {
        return Err(Error::Malformed {
            detail: format!(
                "an extension needs the old tree strictly shorter than the new: {} is not below {}",
                old.tree_size, new.tree_size
            ),
        });
    }
    // The proof must be the one over exactly these two heads, or the answer would grade the wrong
    // pair — gotcha61/73's error, one step out: a consistency proof is relative to two `tree_size`s.
    if proof.old_size != old.tree_size || proof.new_size != new.tree_size {
        return Err(Error::Malformed {
            detail: format!(
                "the proof bridges {}..{}, but the checkpoints are {} and {}; it does not grade \
                 this pair",
                proof.old_size, proof.new_size, old.tree_size, new.tree_size
            ),
        });
    }
    // `verify_consistency` rebuilds both roots from the one path: it is `true` only when the old
    // checkpoint's root really is the size-`old` prefix of the new checkpoint's tree. `false` means
    // the older head is not a prefix of the newer — a diverged fork, not an append-only extension.
    if verify_consistency(proof, &old.root_hash, &new.root_hash)? {
        Ok(None)
    } else {
        Ok(Some(CheckpointContradiction::Fork {
            origin: old.origin.clone(),
            old_size: old.tree_size,
            new_size: new.tree_size,
            old_root: old.root_hash,
            new_root: new.root_hash,
        }))
    }
}
