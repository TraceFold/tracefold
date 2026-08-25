// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `apply`: the one place a tool call is made, and the record that makes a retry a retry.
//!
//! Spec: 41 §4 ("only called after commit approval", "apply is designed to be idempotent"), 51 §7 contract 7, 43 T-10c for what (sem: SEM-gx-adapter-mcp-086)
//! the idempotence is *for*, 42 §3.4 for the observation that comes back. The rulings are **E-M4-3**
//! (the quantifier), **E-M4-31** (`applied_at` is the engine's) and **M4-17** (no clock here).
//!
//! # 🔴 This module is the second premise of AC-051
//!
//! [`crate::transport::Admitted`] and [`crate::transport::ToolCall`] cannot be built outside this
//! crate, which closes every other crate by construction. What is left for a scan to say is that
//! **inside** this crate they are built in one place, and this is that place: the function below is the
//! only one in `src/` that mints either, and `tests/ac_051.rs` derives that by walking `src/` rather
//! than by being handed a list of files.
//!
//! The place is not arbitrary. 41 §4 says `apply` "is only called after commit approval", and Rule 2 (req/78 §3.3) (sem: SEM-gx-adapter-mcp-087)
//! makes `gx-engine`'s `apply_once` the only caller of `SubstrateAdapter::apply` in the workspace. So
//! the composition is: one road above the adapter, one mint inside it, and nothing outside able to
//! build the argument.
//!
//! # The quantifier, and the record that implements it
//!
//! **E-M4-3**: idempotence is quantified over "the same delta re-entering (retry)" and not over every state. (sem: SEM-gx-adapter-mcp-088)
//! `gx-adapter-fs` and `gx-adapter-git` implement it by comparing ("the file already holds these (sem: SEM-gx-adapter-mcp-089)
//! bytes", "the branch already points at this commit"); a tool call declares no state to compare (sem: SEM-gx-adapter-mcp-090)
//! against, so this adapter asks its [`crate::log::CallLog`] instead. What that makes the property
//! depend on -- the proxy's record, not the substrate -- is argued in full where the log is defined,
//! and is not softened here.
//!
//! # What comes back is an observation
//!
//! 41 §4 gives `apply` no pre-state and no post-state, so [`AppliedDelta`] carries what the adapter
//! *saw* afterwards (req/69 §3.1: "post is an observation, not a return value"): the digest of the resource, read (sem: SEM-gx-adapter-mcp-091)
//! back through [`crate::transport::ToolTransport::read`], and a fingerprint over the same position.
//! [`crate::transport::ToolTransport::call`] returned `()` through v0.2 for exactly this reason -- a
//! transport that reported what the tool did would be a second, unverifiable source for a value about
//! to be read.
//!
//! 🔴 **Narrowing correction** (`req/38` §98, ruling 1, two-phase escrow): the sentence above stays the (sem: SEM-gx-adapter-mcp-092)
//! rule for the **post-state** — the fingerprint and digest below are still the read-back and never
//! the tool's report. What `call` now returns is the result's content bytes, and this module carries
//! them on the [`AppliedDelta`]'s *observation seat* — a record of what the server answered, made
//! only when the call was actually issued in this entry (a retry the [`crate::log::CallLog`]
//! short-circuits has no answer to record, which is the crash-window honesty `req/160` 1-0, fact 3 (sem: SEM-gx-adapter-mcp-093)
//! names). The one reader is the escrow completion step (`gx_substrate::InverseCompletion`), for the
//! one value a read-back cannot supply: a do-time server-assigned member of a declared inverse.

use gx_core::{Fingerprint, SubstrateKind, Timestamp};
use gx_substrate::{elide_scope, AppliedDelta, Error, PlannedDelta, Result};

use crate::adapter::{absent_digest, content_digest};
use crate::catalogue::Catalogue;
use crate::delta::McpDelta;
use crate::locator::Position;
use crate::log::CallLog;
use crate::transport::{Admitted, ToolCall, ToolTransport};

/// 🔴 **`req/312` M-01 (R23)** — the sentence a post-apply observation that did not answer carries.
///
/// The one refusal in this crate that is **not** about a declaration: the catalogue may be
/// perfect, the call was admitted, and the call was made. What is missing is the read-back, and the
/// two things this sentence has to do are say that the effect is not in doubt and refuse to let the
/// receipt claim an absence nobody measured.
///
/// # Why the remedy is a re-read and not a retry of the call
///
/// The call is in the [`crate::log::CallLog`], so E-M4-3's quantifier already makes a re-entry of
/// this delta a retry rather than a second effect. What an operator can act on is the read face —
/// `resources/read`, or the `$cas_read` declaration that governs this locator — and R17's wording
/// rule is that a remedy names something the reader can execute.
pub const OBSERVATION_NOT_ANSWERED: &str = "gx made this call and then could not read the object \
     back, so it cannot say what the object now holds. The postcondition is deliberately **not** \
     signed as absence: a receipt saying an object is empty when the read merely failed is a signed \
     statement about a world nobody measured, and every later undo of it is refused with \
     `PRECONDITION_CHANGED` -- a sentence telling the operator that somebody else moved the object \
     when nobody did. The effect itself is not in doubt: it was admitted, it was sent, and the \
     server's own record of it stands. What to fix: make the read face answer for this locator \
     (`resources/read`, or the `$cas_read` face this catalogue declares for it) and run the \
     transformation again -- the call log makes that a retry rather than a second effect \
     (`req/312` M-01)";

/// Perform a delta a gate has already admitted (41 §4).
///
/// # Errors
/// [`Error::ForeignDelta`] for another adapter's delta, [`Error::PayloadUnreadable`] for bytes this
/// grammar did not write, [`Error::Unimplemented`] for a sequence longer than v0.1 runs,
/// [`Error::NotAPosition`] for a payload whose locator is not a position, and [`Error::ApplyFailed`]
/// when the transport could not make the call or the tool refused. 43 T-11 turns the last into
/// `AbortReason::ApplyFailed`.
pub(crate) fn apply(
    transport: &dyn ToolTransport,
    catalogue: &Catalogue,
    log: &dyn CallLog,
    delta: &PlannedDelta,
) -> Result<AppliedDelta> {
    if delta.substrate() != &SubstrateKind::Mcp {
        return Err(Error::ForeignDelta {
            expected: SubstrateKind::Mcp,
            got: delta.substrate().clone(),
        });
    }
    let decoded = McpDelta::decode(delta.payload())?;
    let op = decoded
        .ops()
        .first()
        .expect("decode refuses the empty sequence");
    let position = op.position()?;

    let observation = if log.applied(delta.reference()) {
        // A retry: the call is not re-issued (E-M4-3's quantifier), so there is no answer to
        // record. `None` is the honest seat — the original answer is the journal's to keep, not
        // this module's to reconstruct (req/160 1-0, fact 3). (sem: SEM-gx-adapter-mcp-094)
        None
    } else {
        // 🔴 The two mints. Nowhere else in `src/` does either, and `tests/ac_051.rs` walks this
        // directory to say so rather than being told which file to look in.
        let call = ToolCall::new(
            position.server(),
            position.resource(),
            op.tool(),
            op.arguments(),
            delta.reference(),
        );
        let admitted = Admitted::for_delta(delta.reference());
        let answered = transport.call(&call, &admitted)?;
        // After the call returned, never before: a record written first would turn a failed call into
        // a change the log claims was made, and there is no way back from that for a retry.
        log.record(delta.reference());
        Some(answered)
    };

    let applied = observe(transport, catalogue, &position, delta)?;
    Ok(match observation {
        Some(bytes) => applied.with_observation(bytes),
        None => applied,
    })
}

/// What the adapter saw after the call (42 §3.4).
///
/// 🔴 **DR-46-16**: the read-back goes through [`crate::cas::read_subject`], which is the same road
/// `snapshot` and `precondition` took for this object. That it is the *same* road is the property:
/// a postcondition read through `resources/read` beside a precondition read through a declared tool
/// would be two functions of two different faces of the server, compared as though they were one.
fn observe(
    transport: &dyn ToolTransport,
    catalogue: &Catalogue,
    position: &Position,
    delta: &PlannedDelta,
) -> Result<AppliedDelta> {
    let digest = match crate::cas::read_subject(transport, catalogue, position) {
        Ok(contents) => content_digest(&contents),
        // A resource a call **removed** is not a failed apply. `Unreadable` is the transport's word
        // for "there is nothing here" as well as for "I could not tell you", so it is folded to the (sem: SEM-gx-adapter-mcp-095)
        // digest of no content -- the same collision `gx-adapter-fs` and `gx-adapter-git` disclose for
        // an absent file and an entry that is not in the tree.
        //
        // 🔴 **`req/312` M-01 (R23)** — and the fold is no longer over both facts. The sentence
        // above was written about a **removal**; the audit drove the other preimage, a read face
        // that died after the call, and the fold signed a postcondition of absence over an object
        // holding 24 bytes. The invariant this arm now carries, in one line:
        //
        // > For every post-apply observation: a read that **failed** — wire error, JSON-RPC error,
        // > `isError` result, an answer with no content — is never signed as absence. It is
        // > refused, fail-closed, so that the undo road is not shut with a
        // > `PRECONDITION_CHANGED` naming a third party who does not exist. `absent_digest` is
        // > true only where the **server answered** that the locator holds nothing
        // > ([`crate::transport::READ_ANSWERED_ABSENT`]).
        //
        // The refusal is `Unreadable` and not `ApplyFailed`: the call was made and the server's own
        // record of it stands, so `AbortReason::ApplyFailed` would be this adapter reporting a
        // failure that did not happen. What failed is the observation, and the sentence says so.
        Err(Error::Unreadable { locator, detail }) => {
            if crate::transport::read_answered_absent(&detail) {
                absent_digest()
            } else {
                return Err(Error::Unreadable {
                    locator,
                    detail: format!("{detail}. {OBSERVATION_NOT_ANSWERED}"),
                });
            }
        }
        Err(other) => return Err(other),
    };
    let postcondition =
        Fingerprint::new(SubstrateKind::Mcp, elide_scope(position.locator())?, digest)?;
    Ok(AppliedDelta::new(
        delta.reference().clone(),
        postcondition,
        digest,
        // **E-M4-31** (req/38 §31): "`applied_at` is overwritten by the engine at commit time... the adapter is a (sem: SEM-gx-adapter-mcp-096)
        // `Timestamp(0)` placeholder". 41 §6 is why an adapter has no clock to read. (sem: SEM-gx-adapter-mcp-097)
        Timestamp(0),
    ))
}
