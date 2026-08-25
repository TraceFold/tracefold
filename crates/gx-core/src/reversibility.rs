// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! C-25's three-valued answer to "can this call be undone?" (relocated by **DR-46-26**).
//!
//! # Why the type is down here
//!
//! It was declared in `gx-adapter-mcp` while it was one adapter's private arithmetic. DR-46-26
//! makes it part of `SubstrateAdapter::invert`'s return, and **the relocation is forced rather
//! than preferred**: `gx-adapter-mcp` depends on `gx-substrate`, so a trait in `gx-substrate` that
//! named `gx_adapter_mcp::Reversibility` would cycle.
//!
//! `gx-substrate` would have been far enough for the trait alone. It is **not** far enough for the
//! seat `req/38`'s S1 ruling 5 adds to 42 §3.10: `gx-witness` depends on `gx-core`, `gx-canon` and
//! `gx-log` and **not** on `gx-substrate`, so a receipt payload that names this type can only name
//! it here. One type, two crates that must see it, exactly one crate both of them already depend
//! on — which is the same argument that put `VerdictKind` here in M3 (**E-M3-2**), and the same
//! rule: the data comes down, the computation stays up. Nothing here reads, decides or compares;
//! `gx_adapter_mcp::invert_with_verdict` is still the only thing that works the answer out.

use serde::{Deserialize, Serialize};

/// 🔴 **C-25**'s three values: whether a call can be undone, as a first-class answer.
///
/// 11 §5-2 C-25 puts it as plainly as it can be put — the product's first-class output is not
/// "the change was reversed" but "here is whether it can be, with the reason and the boundary".
/// Two values cannot carry that: "no inverse exists" and "we could not find out" are different
/// facts with different remedies, and folding them loses exactly the information an operator acts
/// on.
///
/// `gx_adapter_mcp::McpAdapter::reversibility` answers it for one planned call, and
/// `gx_substrate::InvertOutcome` is how it crosses the adapter boundary (**DR-46-26**).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Reversibility {
    /// An inverse was constructed. `invert` answered `Some`.
    True,
    /// No inverse exists for this call: no tool is declared to undo it, or the declaration cannot
    /// be resolved from this call's material, or the body is over the escrow ceiling.
    False,
    /// 🔴 **DR-46-9 A-4**: the prior could not be read, so whether an inverse exists was never
    /// established. Reachable only under `OnReadFailure::Unknown` — under the default the effect
    /// is refused and no transformation is committed at all.
    Unknown,
}

impl Reversibility {
    /// The three, in the order C-25 states them. A count a probe can hold a vocabulary against.
    pub const ALL: [&'static str; 3] = ["true", "false", "unknown"];

    /// The word, for a report line and for a receipt reader.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Reversibility::True => "true",
            Reversibility::False => "false",
            Reversibility::Unknown => "unknown",
        }
    }
}
