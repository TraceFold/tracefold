// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `invert`: the delta that puts a position back, built while the old bytes are still there.
//!
//! Spec: 41 §4 for the method and for DR-1(a), 42 §5 for why the escrow carries a body, 34 AC-048
//! and AC-049 for what is measured. The rulings are **E-M4-30** (the escrow is constructed **before**
//! `apply`), **E-M4-3** (the round trip is quantified at the one `pre` handed in), **M4-21, adopted (a)** (sem: SEM-gx-adapter-fs-050)
//! (the payload ceiling and its `Ok(None)`) and **E-M4-5** (an engine folds `Some`/`None` into
//! `GateInput.invert_available` at verify time).
//!
//! # 🔴 Why this module reads the substrate, and why the order is not negotiable
//!
//! **E-M4-30** (req/38 §31 M4H3-1, adopted (a)), verbatim: (sem: SEM-gx-adapter-fs-051)
//!
//! > "escrow (invert) comes before apply (43 T-10b is state machine's canonical source). The reason is physical: the inverse of an overwrite/deletion carries the old content's
//! > body (42 §5's reason the escrow is required), so **invert can only be constructed at the point where pre is observable** --
//! > T-10b's order is the only constructible one, and only an adapter that keeps its own history could satisfy the verbatim order" (sem: SEM-gx-adapter-fs-052)
//!
//! This adapter keeps no history. The inverse of a whole-file replacement is "put back what is here",
//! and "what is here" exists only until `apply` runs. So `invert` (private) reads the position, and the caller (sem: SEM-gx-adapter-fs-053)
//! is required by 43 T-10b to call it first. That is also why 42 §5 wants the **body**: "because a digest-only inverse
//! makes an actual undo physically impossible". (sem: SEM-gx-adapter-fs-054)
//!
//! # The inverse is a function of the state, not of the forward operation's shape
//!
//! Whether the delta writes or removes, the inverse is the same question -- what does this position
//! hold at the escrow point? -- and the answer has two arms:
//!
//! | at the escrow point | inverse | the case it undoes |
//! |---|---|---|
//! | a file with content *c* | write *c* back | change and deletion |
//! | nothing | remove | creation | (sem: SEM-gx-adapter-fs-055)
//!
//! The second arm also gives the right answer to a removal of something that is already gone: the
//! forward delta is a no-op and its inverse is a removal, which is a no-op too. Nothing special-cases
//! it, because nothing has to.
//!
//! # The `pre` argument, and the two things it is
//!
//! `pre` is an [`ObjectSnapshot`]: 42 §3.3 gives it a digest and not the bytes, so it cannot be the
//! source of the body an inverse carries. What it is instead:
//!
//! 1. **the Given the law is quantified at** -- "the one `pre` passed to `invert`" (**E-M4-3**), which (sem: SEM-gx-adapter-fs-056)
//!    the harness writes into the property's message and `tests/ac_049.rs` into each case's;
//! 2. **a statement of which object this question is about**. A `pre` naming another position is a
//!    mis-wired call and is refused as one (**E-M4-32**) -- see below.
//!
//! It is deliberately **not** compared with the position's current digest. **E-M4-5** settled that a
//! prediction going stale between verify and commit is folded by the CAS check into an `Abort`
//! ("a transformation whose prediction went wrong never reaches apply, because of a Fingerprint mismatch"), so an adapter that refused here would (sem: SEM-gx-adapter-fs-057)
//! be turning a state machine transition into an error at a layer that does not own the decision.

use gx_core::{ObjectSnapshot, ReadEntry, SubstrateKind};
use gx_substrate::{Error, InvertOutcome, PlannedDelta, Result};

use crate::adapter::{absent_digest, content_digest};
use crate::delta::{FsDelta, FsOp, MAX_INVERSE_PAYLOAD_BYTES};
use crate::locator;

/// Build the delta that undoes `delta` from the state `pre` (41 §4, DR-1(a)).
///
/// # The one reason for `Ok(None)` (**E-M4-32** narrowed it to this)
///
/// 41 §4 separates "the question itself cannot be answered" (`Err`) from "the answer is no inverse" (sem: SEM-gx-adapter-fs-058)
/// (`Ok(None)`), and **E-M3-4** makes the second an escalation to a human rather than a refusal to
/// act. §33 fixed which facts may take the second form: "**`Ok(None)` is limited to 'a legitimate construction of the
/// same object is not possible' (over the ceiling, or the old content already discarded)**". For this adapter, in v0.1, that leaves exactly one: (sem: SEM-gx-adapter-fs-059)
///
/// * **The escrow ceiling** (**M4-21, adopted (a)**, "the 1st reason AC-048's `None` actually occurs"). An inverse (sem: SEM-gx-adapter-fs-060)
///   carries the whole old file, so an undoable change over a large file costs its size twice. Over
///   [`MAX_INVERSE_PAYLOAD_BYTES`] this adapter declines, and the change is escalated instead of
///   being quietly made unundoable.
///
/// Hand 5 had a second `Ok(None)` here -- a `pre` naming another position -- and raised the reading
/// against itself (req/74 §2 M4H5-1). **E-M4-32** took the other case, and the argument is
/// **E-M4-27**'s: a delta and a snapshot of two different objects is a wiring bug in whoever
/// assembled the call, and answering `Ok(None)` would send it down the escalation path wearing the
/// face of a legitimate business condition. An operator asked "this change cannot be undone, proceed?" (sem: SEM-gx-adapter-fs-061)
/// would be answering the wrong question about a call that should never have been made.
///
/// # Errors
/// [`Error::LocatorMismatch`] when `pre` is a snapshot of another object (**E-M4-32**),
/// [`Error::ForeignDelta`] for another adapter's delta, [`Error::PayloadUnreadable`] for bytes this
/// grammar did not write, [`Error::NotAPosition`] for a payload whose locator is not a position,
/// [`Error::Unimplemented`] for a sequence v0.1 does not run, and [`Error::Unreadable`] when the
/// position exists and will not answer.
///
/// # 🔴 **DEFECT-892-1** (`req/895` §1) — why this answers an [`InvertOutcome`] and not an `Option`
///
/// It used to answer `Result<Option<PlannedDelta>>`, and `adapter.rs` folded that into an outcome
/// with `InvertOutcome::from_option`, which fixed an **empty read-set** on both arms. The engine
/// carries that list into `InverseEscrowed.reads` and from there into `ReadSet::from_reads`, so
/// every fs commit produced a signed receipt whose `read_set` was `ReadSet::Nothing` — a member
/// `gx-witness` documents as answering "was this locator read?" with `Some(false)` **for every
/// locator in the universe**. `read_if_present` below reads the position. The receipt denied it.
///
/// The read entry is therefore built **at the one place in this function where a read has
/// answered** — the shape `gx-adapter-mcp/src/invert.rs` already used — so that every road out of
/// here carries it and none of them can forget to. `locator` is the normalised position, which is
/// the same object `snapshot`, `precondition` and the compare-and-set are quantified over.
pub(crate) fn invert(delta: &PlannedDelta, pre: &ObjectSnapshot) -> Result<InvertOutcome> {
    if delta.substrate() != &SubstrateKind::Fs {
        return Err(Error::ForeignDelta {
            expected: SubstrateKind::Fs,
            got: delta.substrate().clone(),
        });
    }
    let decoded = FsDelta::decode(delta.payload())?;
    let op = decoded
        .ops()
        .first()
        .expect("decode refuses the empty sequence");
    let position = op.position()?;

    let named = locator::normalize(pre.locator());
    if named != position {
        return Err(Error::LocatorMismatch {
            expected: position,
            got: named,
        });
    }

    // 🔴 **DEFECT-892-1** — the read, and the entry that attests it, in one place.
    //
    // Both arms are reads that answered: bytes, or "there is nothing here". The absent arm digests
    // through [`absent_digest`], whose collision with an empty file this crate discloses rather than
    // papering over with an invented marker — the same residue `gx-adapter-git`'s `repo` module and
    // `gx-adapter-postgres`'s `row` module already carry, and the same one `Fingerprint` is where
    // it would be closed. The entry is minted before `old` moves into the operation, because the
    // digest is of what the read answered and not of what is done with it afterwards.
    let (restore, reads) = match read_if_present(&position)? {
        Some(old) => {
            let entry = ReadEntry {
                digest: content_digest(&old),
                locator: position.clone(),
            };
            (FsOp::write(position, old), vec![entry])
        }
        None => {
            let entry = ReadEntry {
                digest: absent_digest(),
                locator: position.clone(),
            };
            (FsOp::remove(position), vec![entry])
        }
    };
    let payload = FsDelta::one(restore).encode()?;
    if payload.len() > MAX_INVERSE_PAYLOAD_BYTES {
        // **M4-21**: over the ceiling there is no escrow. 🔴 The read still happened and is still
        // attested — "gx read your file and then declined to escrow an inverse for it" is a
        // different fact from "gx never looked", and this arm used to say the second.
        return Ok(InvertOutcome::none(reads));
    }
    PlannedDelta::new(SubstrateKind::Fs, payload)
        .map(|inverse| InvertOutcome::inverted(inverse, reads))
}

/// The bytes at a position, or `None` when there are none.
///
/// The absence is a state and not a failure: it is what a creation is planned against, and its
/// inverse is a removal.
fn read_if_present(position: &str) -> Result<Option<Vec<u8>>> {
    match std::fs::read(position) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::Unreadable {
            locator: position.to_string(),
            detail: e.to_string(),
        }),
    }
}
