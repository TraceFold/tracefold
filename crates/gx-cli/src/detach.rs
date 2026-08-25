// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P-1c** (`req/551`) — taking `gx wrap` back out of an agent's configuration, and saying in
//! values what did not come back with it.
//!
//! # What this face answers, and what it refuses to answer
//!
//! `req/535` §2 called an attach "a reversible operation", and this is the hand that has to make
//! that word survive contact with a machine. Three different things could be meant by reversing an
//! adoption, and they do not have the same answer (`req/551` §2-2):
//!
//! * **the entry** — what `mcpServers.<name>` runs. This comes back, and it comes back without any
//!   saved copy, because `--adopt-config` left the original command in the document on purpose
//!   (`gx_mcp_wire::config::adopt`). Every adoption this binary has ever written can be undone.
//! * **the document** — the bytes of the file. This does **not** come back, and it was already gone
//!   before this command ran: the adoption re-serialised the whole file. Said every time, as a
//!   value, never as a silence.
//! * **the tree** — `.gx/` and what it holds. This is not removed, and *not removing it is the
//!   correct behaviour rather than a limitation* (`req/551` D-3). A detach that deleted records
//!   would take the receipts issued while gx was in front of the server with it, and those
//!   receipts are the only durable thing the arrangement produced.
//!
//! # Why there is no word for deletion here
//!
//! `crate::attach`'s three words (`created` / `already-present` / `not-placed`) deliberately have no
//! fourth for "was there and is gone", because no road in this binary builds that state. The words
//! below are the same shape for the same reason: a detach that could report a removal would be a
//! detach that could perform one. [`DETACH_WORDS`] is the whole set, and `p1c_detach.rs` counts it.

use serde_json::{json, Value};

use crate::{Error, Result};

/// The entry runs what it ran before the adoption.
pub const RESTORED: &str = "restored";

/// 🔴 `.gx/` and its records are still there, **by design** — see the module documentation.
pub const LEFT_IN_PLACE: &str = "left-in-place";

/// The entry never routed through gx, so there was nothing here to undo (`req/551` D-5).
pub const NOT_ATTACHED: &str = "not-attached";

/// 🔴 Every word a detach can report. Three, and **not one of them means a removal**.
pub const DETACH_WORDS: [&str; 3] = [RESTORED, LEFT_IN_PLACE, NOT_ATTACHED];

/// What one detach did to the route it was pointed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Undone {
    /// gx was in front of the server, and is not any more.
    Restored,
    /// gx was never in front of this server.
    NotAttached,
}

/// The word an outcome prints as — an exhaustive match, so a fourth state cannot be added without
/// this failing to compile, in the shape `crate::layout::nature_word` uses.
#[must_use]
pub const fn outcome_word(outcome: Undone) -> &'static str {
    match outcome {
        Undone::Restored => RESTORED,
        Undone::NotAttached => NOT_ATTACHED,
    }
}

/// 🔴 What this operation does not touch, said before anybody asks.
///
/// The records keep their own answer (`LEFT_IN_PLACE`) rather than being left out of the report,
/// because "the report did not mention `.gx/`" and "the report says `.gx/` was left alone" are the
/// same document to a reader and different claims to a machine.
pub const RECORDS_ANSWER: &str =
    "the receipts and checkpoints under `.gx/` are left exactly where \
     they are. They are what the arrangement produced, they verify offline without this \
     configuration or any other, and no verb of this binary removes them";

/// Run a detach over one entry of one agent configuration.
///
/// # Errors
/// [`Error::Io`] / [`Error::Malformed`] when the file will not read, and [`Error::Usage`] when the
/// entry runs gx in a shape this cannot read back — which is a refusal to guess, not a failure to
/// try. See `gx_mcp_wire::config::DetachError::Unreadable`.
pub fn run(path: &std::path::Path, name: &str, document: &Value) -> Result<(Value, Value)> {
    let detachment = gx_mcp_wire::config::detach(document, name).map_err(|why| Error::Usage {
        detail: why.to_string(),
    })?;
    let outcome = if detachment.restored.is_some() {
        Undone::Restored
    } else {
        Undone::NotAttached
    };

    // 🔴 **`req/551` D-11** — the coverage after the detach, derived from the route the way
    // `req/544`'s face declaration derives it, and from nothing else. With gx out of the entry the
    // route observes nothing, so all four questions come back `cannot-measure`: the face's coverage
    // is zero, and it is zero as four values rather than as four missing members.
    let after = gx_mcp_wire::config::check(&detachment.config, name);
    let posture = crate::face::posture_from_route(Some(&after));
    let coverage: Vec<Value> = posture
        .iter()
        .map(|(question, posture)| {
            json!({
                "question": crate::face::question_key(*question),
                "posture": crate::face::posture_word(*posture),
            })
        })
        .collect();

    let answer = json!({
        "detached": name,
        "config": path.display().to_string(),
        "outcome": outcome_word(outcome),
        // What the entry runs now. `null` when it never ran gx, because a command reported for an
        // entry this operation did not change would read as a change it made.
        "now_runs": detachment.restored.as_ref().map(|original| json!({
            "command": original.command,
            "args": original.args,
        })),
        // 🔴 The heart of this face: the parts of "reversible" that are not true, by name, every
        // time — including on the runs where everything the operator asked for worked.
        "not_restored": detachment.not_restored,
        "records": LEFT_IN_PLACE,
        "records_note": RECORDS_ANSWER,
        "coverage": coverage,
        "check": after.to_json(),
    });
    Ok((detachment.config, answer))
}
