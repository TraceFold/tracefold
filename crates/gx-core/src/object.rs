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
    Fs,
    Git,
    Mcp,
    Custom(String),
}

/// How the object's content happens to be represented. P-10: the core is representation
/// independent, so nothing in this crate may branch on this value -- it is carried, compared
/// and encoded, never interpreted. Interpretation is the adapter's job (42 §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ReprKind {
    Bytes,
    Json,
    Tree,
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

    #[must_use]
    pub fn digest(&self) -> &Cid {
        &self.digest
    }

    #[must_use]
    pub fn substrate(&self) -> &SubstrateKind {
        &self.substrate
    }

    #[must_use]
    pub fn representation(&self) -> &ReprKind {
        &self.representation
    }

    #[must_use]
    pub fn id(&self) -> &ObjectId {
        &self.id
    }
}
