// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Objects: the content-addressed snapshot and the two enumerations that describe it.
//!
//! Spec: 41 §3 for `ObjectSnapshot`, 42 §0 for this module's contents
//! (`ObjectSnapshot`, `SubstrateKind`, `ReprKind`), 42 §3.1 for the field table.
//! `ObjectId` is placed here by the A-1 erratum (`req/38_ERRATA_2026-08-07.md` §1, which adds
//! the `ObjectId` row missing from the 42 §0 table; req/31 §1 chose this module for it).

use crate::Cid;
use serde::{Deserialize, Serialize};

/// 41 §3: `pub struct ObjectId(pub Cid)`. The CID of the snapshot's `IdentityView`
/// (42 §1.3, which excludes `id` itself -- including it would define the value circularly).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub Cid);

/// Which substrate a locator is read against. 42 §3.1 fixes the enumeration at these four and
/// says it is defined in gx-core; `Custom` is the escape hatch for adapters shipped later, and
/// keeping it a `String` is what stops this enum from becoming a registry the core has to hold.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SubstrateKind {
    /// The filesystem adapter's namespace: a locator is a path (42 §3.1).
    Fs,
    /// The git adapter's namespace: a locator is a ref or an object name.
    Git,
    /// The MCP adapter's namespace: a locator names a tool-reachable resource.
    Mcp,
    /// An adapter shipped outside this workspace. The string is its self-chosen name; the core
    /// compares it and never interprets it.
    Custom(String),
}

/// How the object's content happens to be represented. P-10: the core is representation
/// independent, so nothing in this crate may branch on this value -- it is carried, compared
/// and encoded, never interpreted. Interpretation is the adapter's job (42 §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ReprKind {
    /// An opaque byte sequence -- the representation that assumes nothing.
    Bytes,
    /// A JSON document (42 §3.1). A hint for adapters and tools; the core does not parse it.
    Json,
    /// A tree of named children, e.g. a directory or a git tree.
    Tree,
    /// Content the adapter reaches through a reference rather than as bytes in hand. 42 §3.1
    /// enumerates the name and leaves its interpretation to the adapter (P-10), same as the
    /// other three.
    External,
}

/// A content-addressed reference to a substrate state (41 §3, 42 §3.1).
///
/// ASM-9: the bytes themselves are not stored. `digest` is the whole of the claim about
/// content, which is why a snapshot stays small enough to put in a ledger.
///
/// # One way to read a field, not two
///
/// Until F-6 (`req/46D_AUDIT_RULING_2026-08-07.md` §1) all five fields were `pub` **and** had a
/// same-named accessor, so `x.locator` and `x.locator()` both worked and no doc said which to
/// write -- `req/46C_AUDIT_PRACTICAL_2026-08-07.md` §3 W-1 measured that, and measured the
/// asymmetry with `Transformation`, which had no accessors at all. The fields are private now and
/// the accessors are the surface: one spelling per field, and the pair of core structs reads the
/// same way (`Transformation::order` went private for the invariant it carries; these went
/// private for the duplication they carried).
///
/// [`ObjectSnapshot::new`] takes the five values. There is no invariant to check -- a snapshot is
/// five values with no relation between them (`digest` is a claim about content the core cannot
/// verify, ASM-9) -- so unlike [`crate::Transformation::new`] it is infallible.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectSnapshot {
    id: ObjectId,
    substrate: SubstrateKind,
    /// Position inside the substrate: a path, a ref, a tool name. The string convention is the
    /// adapter's (42 §3.1), and the core does not parse it.
    locator: String,
    digest: Cid,
    representation: ReprKind,
}

impl ObjectSnapshot {
    /// Build one. Infallible: 42 §3.1 relates none of the five fields to each other.
    #[must_use]
    pub fn new(
        id: ObjectId,
        substrate: SubstrateKind,
        locator: String,
        digest: Cid,
        representation: ReprKind,
    ) -> Self {
        Self {
            id,
            substrate,
            locator,
            digest,
            representation,
        }
    }

    /// AC-001: one signature for every `ReprKind`. The accessors below take `&self` and return
    /// borrows of fields whose types do not mention `ReprKind`, so a caller holding a `Json`
    /// snapshot writes exactly the code a caller holding a `Bytes` snapshot writes (P-10).
    #[must_use]
    pub fn locator(&self) -> &str {
        &self.locator
    }

    /// The content digest -- ASM-9's whole claim about what is at the locator, since the bytes
    /// themselves are never stored.
    #[must_use]
    pub fn digest(&self) -> &Cid {
        &self.digest
    }

    /// Which substrate the locator is read against (42 §3.1).
    #[must_use]
    pub fn substrate(&self) -> &SubstrateKind {
        &self.substrate
    }

    /// How the content happens to be represented. Carried, never interpreted here (P-10).
    #[must_use]
    pub fn representation(&self) -> &ReprKind {
        &self.representation
    }

    /// The snapshot's identity: the CID of its `IdentityView`, which excludes this field itself
    /// (42 §1.3).
    #[must_use]
    pub fn id(&self) -> &ObjectId {
        &self.id
    }
}
