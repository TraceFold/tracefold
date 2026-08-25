// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The opaque planned-delta carrier M3 moves without interpreting.
//!
//! Spec: 42 §3.4 for what a `PlannedDelta` is, 41 §4 for the `GateInput` field that needs one,
//! **E-M3-1** (`req/38_ERRATA_2026-08-07.md` §19) for why this type exists instead.
//!
//! 41 §4 types `GateInput.planned` as `&'a PlannedDelta` and makes it one of the five fields a gate
//! is handed; 42 §0 files `PlannedDelta` under `gx-substrate/delta.rs` -- M4. A gate that cannot be
//! constructed until M4 is a gate M3 cannot build, which is exactly the shape M2 met with
//! `Fingerprint`: "M3-02 (adopted = option a): `PlannedDelta` puts an opaque carrier type of the
//! same shape as E-M2-2 in gx-core and interprets it in M4. `InvariantCheck` takes the same type"
//! (quoted in SEM-gx-core-074). So the ruling carries the payload bytes now
//! and lets M4 supply the meaning, and the two carriers sit side by side in this crate rather than
//! one of them being invented twice.

use core::fmt;

/// The bytes of a planned change, which no crate below `gx-substrate` may read (**E-M3-1**, P-6).
///
/// # Why bytes and not a struct
///
/// 42 §3.4 gives `PlannedDelta` three fields -- `substrate`, `payload`, `reference` -- of which
/// "`payload` | an **opaque change description that only the adapter interprets; core/gate/witness
/// handle it only as a byte string (P-6)**" (quoted in SEM-gx-core-075). This type is that one
/// field. The other two are not modelled and their absence is
/// the point:
///
/// * `substrate` is already on the gate's input twice over -- `GateInput.pre.substrate`
///   (`ObjectSnapshot`, 41 §3) is the substrate of the object being changed, and a delta for a
///   different one would be a bug in the caller rather than a case the gate decides. Carrying a
///   second copy would create the possibility of the two disagreeing, which is a state no ruling
///   has given a meaning to.
/// * `reference` is the delta's own CID (42 §1.3 excludes it from the IdentityView as
///   self-referential), and minting one goes through `gx-canon` -- which this crate may not name
///   (A-1, ASM-16).
///
/// So this is a carrier and not an early `PlannedDelta`. **A gate that compares two of these for
/// equality has compared byte strings, not deltas**: 42 §3.4's equality would be over the
/// IdentityView `{substrate, payload}`, and one of those two is missing here. That is the same
/// sentence [`crate::FingerprintBytes`] carries about the CAS check, and for the same reason.
///
/// # Why no serde
///
/// `FingerprintBytes` has a wire face because `ReceiptPayload` carries one and receipts are signed
/// bytes (42 §3.10). Nothing in M3 encodes a planned delta: 42 §1.3 gives `PlannedDelta` an
/// IdentityView, but the value that gets canonicalised is M4's three-field struct and not this
/// stand-in for one of its fields. Deriving `Serialize` here would publish a spelling for
/// `PlannedDelta`'s payload that no document has fixed, and a spelling published early is one that
/// has to be kept.
///
/// The bytes are public because the type is a carrier: any byte string is a legal value, and an
/// accessor would suggest an invariant it does not hold.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlannedDeltaBytes(pub Vec<u8>);

/// Opaque, like [`crate::FingerprintBytes`]'s and for a stronger reason: a `payload` is an
/// adapter's own encoding of a change, so a `{:?}` of it in a log line is both unreadable and the
/// one place where P-6's "handled only as a byte string" (sem: SEM-gx-core-076) would quietly stop
/// being true. The length is
/// printed because the length is not the content, and a reader debugging an empty delta needs it.
impl fmt::Debug for PlannedDeltaBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PlannedDeltaBytes(opaque, {} bytes)", self.0.len())
    }
}
