// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Whether two deltas are independent.
//!
//! Spec: 41 §3 for the type, 42 §0 and 42 §3.6 for the field table (ASM-2, C-4, 12 F1).

use crate::delta::DeltaRef;
use serde::{Deserialize, Serialize};

/// The answer an adapter gives when asked whether two deltas commute (41 §3).
///
/// `Conflicts` carries the part that is *not* independent as a `residual` (42 §3.6). Making the
/// residual part of the answer rather than a separate lookup is what keeps a conflict reportable:
/// the caller learns which delta to look at, not merely that something clashed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Commutation {
    /// The two deltas are independent: applying them in either order reaches the same state
    /// (ASM-2's reading of 12 F1), so the engine may reorder or parallelise them.
    Commutes,
    /// The two deltas are not independent, and `residual` names the part that clashes.
    Conflicts {
        /// The delta carrying the non-independent part (42 §3.6). A reference rather than a
        /// payload, for the same reason `Transformation.delta` is one: the core does not read
        /// delta contents (ASM-16).
        residual: DeltaRef,
    },
}
