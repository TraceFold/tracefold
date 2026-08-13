//! `apply`: the one place a tool call is made, and the record that makes a retry a retry.
//!
//! Spec: 41 §4 (「commit承認後にのみ呼ばれる」, 「適用は冪等に設計する」), 51 §7 contract 7, 43 T-10c for what
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
//! The place is not arbitrary. 41 §4 says `apply` 「は commit承認後にのみ呼ばれる」, and 則 2 (req/78 §3.3)
//! makes `gx-engine`'s `apply_once` the only caller of `SubstrateAdapter::apply` in the workspace. So
//! the composition is: one road above the adapter, one mint inside it, and nothing outside able to
//! build the argument.
//!
//! # The quantifier, and the record that implements it
//!
//! **E-M4-3**: idempotence is quantified over 「同一 delta の再入(retry)」 and not over every state.
//! `gx-adapter-fs` and `gx-adapter-git` implement it by comparing (「the file already holds these
//! bytes」, 「the branch already points at this commit」); a tool call declares no state to compare
//! against, so this adapter asks its [`crate::log::CallLog`] instead. What that makes the property
//! depend on -- the proxy's record, not the substrate -- is argued in full where the log is defined,
//! and is not softened here.
//!
//! # What comes back is an observation
//!
//! 41 §4 gives `apply` no pre-state and no post-state, so [`AppliedDelta`] carries what the adapter
//! *saw* afterwards (req/69 §3.1: 「post は返り値でなく観測値である」): the digest of the resource, read
//! back through [`crate::transport::ToolTransport::read`], and a fingerprint over the same position.
//! [`crate::transport::ToolTransport::call`] returns `()` for exactly this reason -- a transport that
//! reported what the tool did would be a second, unverifiable source for a value about to be read.

use gx_core::{Fingerprint, SubstrateKind, Timestamp};
use gx_substrate::{elide_scope, AppliedDelta, Error, PlannedDelta, Result};

use crate::adapter::{absent_digest, content_digest};
use crate::delta::McpDelta;
use crate::locator::Position;
use crate::log::CallLog;
use crate::transport::{Admitted, ToolCall, ToolTransport};

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

    if !log.applied(delta.reference()) {
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
        transport.call(&call, &admitted)?;
        // After the call returned, never before: a record written first would turn a failed call into
        // a change the log claims was made, and there is no way back from that for a retry.
        log.record(delta.reference());
    }

    observe(transport, &position, delta)
}

/// What the adapter saw after the call (42 §3.4).
fn observe(
    transport: &dyn ToolTransport,
    position: &Position,
    delta: &PlannedDelta,
) -> Result<AppliedDelta> {
    let digest = match transport.read(position.server(), position.resource()) {
        Ok(contents) => content_digest(&contents),
        // A resource a call **removed** is not a failed apply. `Unreadable` is the transport's word
        // for 「there is nothing here」 as well as for 「I could not tell you」, so it is folded to the
        // digest of no content -- the same collision `gx-adapter-fs` and `gx-adapter-git` disclose for
        // an absent file and an entry that is not in the tree.
        Err(Error::Unreadable { .. }) => absent_digest(),
        Err(other) => return Err(other),
    };
    let postcondition =
        Fingerprint::new(SubstrateKind::Mcp, elide_scope(position.locator())?, digest)?;
    Ok(AppliedDelta::new(
        delta.reference().clone(),
        postcondition,
        digest,
        // **E-M4-31** (req/38 §31): 「`applied_at` は engine が commit 時に上書きする…adapter は
        // `Timestamp(0)` placeholder」. 41 §6 is why an adapter has no clock to read.
        Timestamp(0),
    ))
}
