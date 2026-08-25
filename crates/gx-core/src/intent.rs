// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! What a caller asks for, before anything has been planned.
//!
//! Spec: 42 §3.3 for the five fields, 42 §1.3 for the projection every one of them is inside,
//! 41 §4 for the `plan(&Intent)` that consumes one, ASM-11 for the two-stage identity this type
//! is the first stage of. **E-M4-2** (`req/38_ERRATA_2026-08-07.md` §28) is why it is here.
//!
//! # The type the spec had and the implementation did not
//!
//! req/69 §4 measured it: 42 §3.3 fixes the five fields and 42 §1.3 gives them an IdentityView, but
//! `struct Intent` appeared **zero times** in the workspace -- only `IntentId`, which is the CID of
//! a value nothing could build. `Transformation.intent_id` therefore named a Draft that had no
//! representation, and E-M3-13 ② could refuse the all-zero placeholder without anything being able
//! to supply a real one.
//!
//! # `goal`, and the dependency that is not here
//!
//! 42 §3.3 types `goal` as `serde_json::Value`, and 41 §2 allows this crate "serde, thiserror and
//! not much more". **E-M4-2**, verbatim: "the five `Intent` fields are gx-core; `goal` is a
//! **carrier type of canonical DAG-CBOR opaque bytes** (the same shape as E-M2-2/E-M3-1; zero
//! serde_json dependency)" (quoted in SEM-gx-core-054). So the field is [`GoalBytes`], and it is
//! the third carrier of the same shape in this crate after [`crate::FingerprintBytes`] and
//! [`crate::PlannedDeltaBytes`] -- adapter-defined content the core moves without reading (P-6).
//!
//! The reading loses nothing the spec asserted. 42 §3.3's own gloss of the field is "an
//! adapter-specific description of intent" (sem: SEM-gx-core-055) and its three examples (a diff, a
//! commit intent, tool-call arguments) are all
//! adapter grammar; what a JSON `Value` would have added is a second canonical form beside the
//! DAG-CBOR one 42 §2.1 fixes, in the one crate 41 §6 forbids to know an encoder at all.

use crate::context::{Actor, ChangeContext};
use crate::object::SubstrateKind;

use core::fmt;

/// An adapter's own description of what is wanted, carried and never read (42 §3.3, **E-M4-2**).
///
/// The bytes are canonical DAG-CBOR produced by whoever wrote the intent, which is what makes the
/// `Intent`'s CID a function of content rather than of a formatting choice: two callers asking for
/// the same thing have to agree byte for byte, and 42 §2.1's canonical rules are what let them.
///
/// The field is public because the type is a carrier -- any byte string is a legal value here, and
/// an accessor would suggest an invariant it does not hold. Same sentence [`crate::PlannedDeltaBytes`]
/// carries, for the same reason.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoalBytes(pub Vec<u8>);

/// Opaque, like [`crate::PlannedDeltaBytes`]'s: a goal is an adapter's grammar, so a `{:?}` of it in
/// a log line is both unreadable and the one place where P-6's "handled only as a byte string"
/// (sem: SEM-gx-core-056) would quietly stop being true. The length is printed because the length
/// is not the content.
impl fmt::Debug for GoalBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GoalBytes(opaque, {} bytes)", self.0.len())
    }
}

/// A byte string on the wire and base64 in a human-readable format, which is the pair M2H1-4
/// settled for every raw byte string in this crate. A derive would write the bytes as a sequence of
/// integers, whose encoded length depends on their values (42 §1.1 makes the same point about
/// `Cid`).
///
/// # Why there is no `Deserialize`
///
/// Nothing reads an `Intent` back yet. [`crate::FingerprintBytes`] has both faces because a receipt
/// is decoded by a verifier (42 §3.10) and that verifier exists; the reader of an `Intent` is the
/// `submit` API of 44, which is M6. A `Deserialize` published now would fix the spelling a submit
/// body is parsed from before the hand that has to live with it has looked at 44 -- and the wire
/// face that *is* needed here is the one gx-canon's `IdentityView` projection uses to reach a CID,
/// which is a write.
impl serde::Serialize for GoalBytes {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> core::result::Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&crate::b64::encode(&self.0))
        } else {
            serializer.serialize_bytes(&self.0)
        }
    }
}

/// The submit input: five fields, all of them part of what the intent *is* (42 §3.3, 42 §1.3).
///
/// 42 §1.3's row is "`substrate`, `locator`, `goal`, `context`, `actor`" with no exclusion and the
/// reason "an Intent is itself an independent description of intent, so there is no exclusion rule"
/// (quoted in SEM-gx-core-057). That makes the projection the whole struct,
/// so the two field sets have to stay equal -- `crates/gx-canon/tests/intent_identity.rs` holds
/// them against each other in the A-10 form (**I-1**).
///
/// # Where the identity is, and where it is not
///
/// ASM-11 makes `IntentId` the CID of this value, fixed at `submit` (43 T-1) and immutable after.
/// Computing it means projecting and hashing, both of which live in gx-canon (A-1), so this crate
/// owns the type and none of the arithmetic. There is no `id` field: 42 §1.3-1 excludes a
/// self-referential one everywhere, and here the whole struct is the input to its own name.
///
/// Fields are private with accessors (F-6, `req/46D_AUDIT_RULING_2026-08-07.md` §1): one spelling
/// per field, as [`crate::ObjectSnapshot`] has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Intent {
    substrate: SubstrateKind,
    locator: String,
    goal: GoalBytes,
    context: ChangeContext,
    actor: Actor,
}

impl Intent {
    /// Build one. Infallible: 42 §3.3 relates none of the five fields to each other.
    ///
    /// In particular the `locator` is taken as written. Normalising it is the adapter's, and
    /// **E-M4-12** fixes that contract in `gx-substrate`'s crate documentation -- a core that
    /// normalised would be a core with a path grammar, which is the road M3-10 refused for the gate
    /// and the same road here.
    #[must_use]
    pub fn new(
        substrate: SubstrateKind,
        locator: String,
        goal: GoalBytes,
        context: ChangeContext,
        actor: Actor,
    ) -> Self {
        Self {
            substrate,
            locator,
            goal,
            context,
            actor,
        }
    }

    /// Which substrate the intent is against (42 §3.3).
    #[must_use]
    pub fn substrate(&self) -> &SubstrateKind {
        &self.substrate
    }

    /// The position inside it, in the adapter's own spelling (42 §3.3).
    #[must_use]
    pub fn locator(&self) -> &str {
        &self.locator
    }

    /// The adapter-defined description of what is wanted (42 §3.3).
    #[must_use]
    pub fn goal(&self) -> &GoalBytes {
        &self.goal
    }

    /// Which kind of change this belongs to; maps to `Transformation.context` (42 §3.3).
    #[must_use]
    pub fn context(&self) -> &ChangeContext {
        &self.context
    }

    /// Who is asking; maps to `Transformation.actor` (42 §3.3).
    #[must_use]
    pub fn actor(&self) -> &Actor {
        &self.actor
    }
}
