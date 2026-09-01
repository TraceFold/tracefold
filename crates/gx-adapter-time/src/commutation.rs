// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `commutation`: whether two changes to a schedule are independent.
//!
//! Decided from the two payloads, touching no substrate -- the same shape **M4-25, adopted (a)**
//! gives the fs adapter, and for the same reason: independence is a question about what two deltas
//! name, and an answer that read the world would be an answer that could differ between two moments
//! when the deltas had not changed.
//!
//! # The rule, and the bound on it
//!
//! Two deltas commute exactly when they name **different positions**. One entry is one position;
//! changes at different positions of a schedule reach the same state in either order because
//! neither reads the other's bytes.
//!
//! At the same position they [`Commutation::Conflicts`], and the residual is the second delta's own
//! reference: at one position the later write is the whole of what does not survive reordering.
//!
//! 🔴 What this rule does **not** model: two entries at different positions that the runner would
//! execute in an order that matters, or that write to the same resource when they run. That is
//! independence of *the actions*, which lives in the substrate the actions touch and not in this
//! one. This adapter's `Commutes` is a claim about the schedule, not about what running it does,
//! and reading it as the second would be reading a claim this crate does not make.

use gx_core::Commutation;
use gx_substrate::{Error, PlannedDelta, Result};

use crate::adapter::kind;
use crate::entry::TimeOp;

/// Whether `a` and `b` may be reordered (41 §4).
///
/// # Errors
/// [`Error::ForeignDelta`] for another adapter's delta, [`Error::PayloadUnreadable`] for bytes this
/// grammar did not write, [`Error::NotAPosition`] for a payload whose locator is not a position.
pub fn commutation(a: &PlannedDelta, b: &PlannedDelta) -> Result<Commutation> {
    for delta in [a, b] {
        if delta.substrate() != &kind() {
            return Err(Error::ForeignDelta {
                expected: kind(),
                got: delta.substrate().clone(),
            });
        }
    }
    let left = TimeOp::decode(a.payload())?.position()?;
    let right = TimeOp::decode(b.payload())?.position()?;

    if left == right {
        Ok(Commutation::Conflicts {
            residual: b.reference().clone(),
        })
    } else {
        Ok(Commutation::Commutes)
    }
}
