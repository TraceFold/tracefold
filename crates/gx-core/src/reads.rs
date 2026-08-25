// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! One object an escrow read, named and digested (**DR-46-24(A)**, relocated by **DR-46-26**).
//!
//! # Why the type is down here and the tree is not
//!
//! `ReadEntry` was declared in `gx-witness` when D24 built the read-set, because the receipt was
//! the only thing that carried one. DR-46-26 gives the read-set a **producer** — an adapter — and
//! an adapter is declared in `gx-substrate`, which does not depend on `gx-witness` and must not:
//! the boundary crate naming the receipt crate inverts the layering and pulls `gx-log` into the
//! boundary's transitive dependencies.
//!
//! So the entry comes down and the tree stays up. `ReadEntry` is `{Cid, String}` and holds no
//! gx-witness-specific dependency of any kind, which is the test 41 §2's module list has been
//! extended by since M2: **the data comes down, the computation stays up.** `ReadSet` —
//! the spill threshold, the RFC 6962 tree, the `gx-log` proof arithmetic it is paired with — stays
//! in `gx-witness`, and `ReadSet::from_reads` remains the only thing that chooses a granularity.
//! An adapter returns `Vec<ReadEntry>`; it may not return a `ReadSet` (`req/441` §4: "spill is the
//! constructor's decision", and a caller who could pick the variant would make the granularity tag
//! a function of the caller's mood rather than of the number of reads).

use serde::{Deserialize, Serialize};

use crate::Cid;

/// One object the escrow read, named and digested (**DR-46-24(A)**).
///
/// `req/350` §2-3 measured which half of this costs: the digest is 32 bytes and the locator is
/// everything else, so the entry is about 102 bytes on the wire in the full-locator form a receipt
/// actually carries. That is why a granularity exists at all.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReadEntry {
    /// The digest of what the read answered, through `gx_canon::cid` and no other road (AC-014).
    pub digest: Cid,
    /// The object the read was about, in the normalised form the adapter names it by.
    pub locator: String,
}
