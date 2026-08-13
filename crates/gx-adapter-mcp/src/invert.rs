//! `invert`: a compensating call, carrying the body, built while the pre-state is still live.
//!
//! Spec: 41 §4 for the method and DR-1(a), 42 §5 for why an escrow carries a body, 34 AC-048 for what
//! is measured. The rulings are **E-M4-30** (the escrow is constructed **before** `apply`, 43 T-10b),
//! **E-M4-3** (the round trip is quantified at the one `pre` handed in), **E-M4-32** (which facts may
//! take the `Ok(None)` form), **M4-21** (the escrow ceiling) and **E-M4-5** (an engine folds
//! `Some`/`None` into `GateInput.invert_available` at verify time).
//!
//! # 🔴 The inverse of a tool call is another tool call, and only the deployment knows which
//!
//! There is nothing in the MCP protocol that says how to undo `tools/call`. A filesystem inverse is
//! derivable (write the old bytes) and a git inverse is derivable (move the reference back); this one
//! is **declared**, by whoever runs the server, in a [`crate::catalogue::Catalogue`]. The crate root
//! and `catalogue.rs` argue why that is a declaration rather than a second gate.
//!
//! What the inverse carries is the **prior contents of the resource**, which is 42 §5 in as many words:
//! 「digest-onlyでは実際のundoが物理的に不可能なため」. So this module reads the resource -- and 43 T-10b
//! requires the caller to have done so before `apply`, because after the call the prior contents are
//! gone.
//!
//! # The two reasons for `Ok(None)`, and both are real
//!
//! **E-M4-32** fixes which facts may take the form: 「**`Ok(None)` は「同一 object の正当な構成不能」(上限
//! 超過・旧内容破棄済み)に限定**」. This adapter has one of each:
//!
//! * **No restore tool is declared** for the tool being inverted. The change is one gx cannot undo, and
//!   **E-M3-4** asks a person before it happens. An empty catalogue makes every change this shape,
//!   which is the conservative direction and the default.
//! * **The prior contents exceed [`crate::delta::MAX_INVERSE_PAYLOAD_BYTES`]** -- **M4-21**'s own
//!   instance, the one `gx-adapter-git` argued it could never reach. Here it is reachable, because an
//!   MCP server offers no content-addressed store to leave the body in.

use gx_core::{ObjectSnapshot, SubstrateKind};
use gx_substrate::{Error, PlannedDelta, Result};

use crate::catalogue::Catalogue;
use crate::delta::{restore_arguments, McpDelta, McpOp, MAX_INVERSE_PAYLOAD_BYTES};
use crate::locator;
use crate::transport::ToolTransport;

/// Build the delta that undoes `delta` from the state `pre` (41 §4, DR-1(a)).
///
/// # Errors
/// [`Error::LocatorMismatch`] when `pre` is a snapshot of another object (**E-M4-32**): a delta and a
/// snapshot of two different objects is a wiring bug in whoever assembled the call, and answering
/// `Ok(None)` would send it down the escalation path wearing the face of a legitimate business
/// condition (**E-M4-27**'s argument). [`Error::ForeignDelta`] for another adapter's delta,
/// [`Error::PayloadUnreadable`] for bytes this grammar did not write, [`Error::Unimplemented`] for a
/// sequence v0.1 does not run, [`Error::NotAPosition`] for a payload whose locator is not a position,
/// and [`Error::Unreadable`] when the server will not answer for the resource.
pub(crate) fn invert(
    transport: &dyn ToolTransport,
    catalogue: &Catalogue,
    delta: &PlannedDelta,
    pre: &ObjectSnapshot,
) -> Result<Option<PlannedDelta>> {
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

    let named = locator::normalize(pre.locator());
    if named != position.locator() {
        return Err(Error::LocatorMismatch {
            expected: position.locator(),
            got: named,
        });
    }

    let Some(restore) = catalogue.restore_for(op.tool()) else {
        // 「gx does not know how to undo this」, which is a fact about the deployment's declaration and
        // not about this call. E-M3-4 asks a person.
        return Ok(None);
    };

    let contents = transport.read(position.server(), position.resource())?;
    let arguments = restore_arguments(position.resource(), &contents)?;
    let payload = McpDelta::one(McpOp::call(
        position.locator(),
        restore.to_string(),
        arguments,
    ))
    .encode()?;
    if payload.len() > MAX_INVERSE_PAYLOAD_BYTES {
        // **M4-21**: over the ceiling there is no escrow, and E-M3-4 escalates rather than this
        // adapter quietly carrying an unbounded body into a journal.
        return Ok(None);
    }
    PlannedDelta::new(SubstrateKind::Mcp, payload).map(Some)
}
