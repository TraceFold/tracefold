//! The lightweight delta reference.
//!
//! Spec: 41 §3 for the type, 42 §0 and 42 §3.4 for the field table.
//!
//! ASM-16 is the whole reason this type is here rather than in gx-substrate with the
//! `PlannedDelta` it points at. `Transformation.delta` needs *some* delta type, and if that type
//! were the real one, gx-core would depend on gx-substrate while gx-substrate depends on gx-core
//! for `ObjectSnapshot`. A reference cuts the cycle: two fields, no payload, no interpretation.

use crate::object::SubstrateKind;
use crate::Cid;
use serde::{Deserialize, Serialize};

/// A canonical reference to a delta the core does not read (42 §3.4).
///
/// `cid` is the CID of the `PlannedDelta`'s `IdentityView` (`substrate` + `payload`), computed in
/// gx-canon. `substrate` is stored here as well as inside the delta it names, because a reader
/// holding only the reference still has to know whose grammar the payload is written in (P-6).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DeltaRef {
    pub substrate: SubstrateKind,
    pub cid: Cid,
}
