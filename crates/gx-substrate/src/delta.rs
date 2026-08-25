// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The two delta values an adapter produces: what it plans, and what it observed after applying.
//! (sem: SEM-gx-substrate-060, SEM-gx-substrate-061, SEM-gx-substrate-062, SEM-gx-substrate-063,
//! SEM-gx-substrate-064, SEM-gx-substrate-065, SEM-gx-substrate-066, SEM-gx-substrate-067,
//! SEM-gx-substrate-068)
//!
//! Spec: 42 §3.4 for both field tables, 42 §1.3 for `PlannedDelta`'s projection, 41 §4 for the
//! methods that pass them. 42 §0 files both types in this crate and this is where they are.
//!
//! # The skeleton, and the three things it deliberately leaves open
//!
//! M4 hand 1 defines the values; the trait that moves them is hand 2 and the adapter that fills
//! them is hand 4 (req/69 §6.2). Three questions are therefore visible here as absences, each one
//! with a ruling behind it:
//!
//! * **`reference` is derived, as of hand 3.** 42 §3.4 requires "its own canonical reference
//!   (matches `DeltaRef.cid`)". Hand 1 took it on trust and raised the gap (req/70 §3 M4H1-3); §29
//!   ruled case (a) -- "turn `PlannedDelta::new` into a `Result` and mint `DeltaRef` from the
//!   projection" -- so the constructor now projects through [`gx_canon::cid::IdentityView`] and
//!   mints the CID itself, which is also what completes **E-M4-26**'s gx-canon dependency line. The
//!   agreement is a property of the type rather than a claim in a comment, and "a CID with no
//!   destination to point to" (M4-14) cannot be built here.
//! * **`applied_at` is an argument.** 41 §6, verbatim: "randomness and time are injected at the
//!   engine boundary (for deterministic replay)", and **M4-17** takes case (a): "`applied_at` is
//!   **injected by the engine** (the `AppliedDelta` constructor takes `Timestamp` as an argument)".
//!   An adapter that read a clock would make a replay of the same
//!   commit produce a different record, so the constructor takes the moment rather than finding
//!   one. `substrate_contract.rs` asserts that this crate names no clock at all.
//! * **`postcondition` is a [`gx_core::Fingerprint`] and therefore not comparable by `==`.**
//!   E-M4-15 removed `PartialEq` from that type on purpose, so nothing holding one can derive it
//!   either -- which is why [`AppliedDelta`] has no `PartialEq` and every comparison downstream has
//!   to say whether it is asking 42 §3.5's question (`cas_eq`) or a component's.

use core::fmt;

use gx_canon::cid::{self, IdentityView};
use gx_core::{Cid, DeltaRef, Fingerprint, SubstrateKind, Timestamp};
use serde::{Serialize, Serializer};

use crate::error::{Error, Result};

/// A change an adapter has worked out but not performed (42 §3.4).
///
/// `payload` is "an opaque change description that only the adapter interprets; core/gate/witness
/// handle it only as a byte string (P-6)". This crate does not read it either: interpreting a
/// payload is what an *adapter* does,
/// and the boundary crate is not one. `crates/gx-core/src/planned.rs` carries the same bytes for
/// gx-gate under E-M3-1, and **E-M4-11** made that carrier permanent -- `GateInput.planned` stays
/// `PlannedDeltaBytes` so that no gate learns this type exists.
///
/// Fields are private with accessors (F-6), which here also means a `payload` cannot be edited in
/// place while `reference` keeps naming what it used to be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedDelta {
    substrate: SubstrateKind,
    payload: Vec<u8>,
    reference: DeltaRef,
}

/// The payload as it reaches the digest: a byte string, not a sequence of integers.
///
/// The same wrapper `gx_core::GoalBytes` and `gx_core::FingerprintBytes` carry, for the same reason
/// M2H1-4 gave: a derived `Serialize` over `Vec<u8>` writes each byte as a CBOR integer, whose
/// encoded length depends on its value, so two payloads of the same length would not encode to the
/// same length. A byte string is one header and the bytes.
///
/// Private to the projection: nothing outside reads a payload (P-6), and this type exists only so
/// that the *encoder* sees the right shape.
pub struct PayloadView<'a>(&'a [u8]);

/// Opaque, like [`gx_core::GoalBytes`]'s. A payload is an adapter's grammar, so printing it in a
/// `{:?}` would be the one place where "handled only as a byte string" quietly stops being true.
/// The length is
/// printed because the length is not the content.
impl fmt::Debug for PayloadView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PayloadView(opaque, {} bytes)", self.0.len())
    }
}

impl Serialize for PayloadView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&gx_core::b64::encode(self.0))
        } else {
            serializer.serialize_bytes(self.0)
        }
    }
}

/// `PlannedDelta` minus `reference` (42 §1.3, row 4).
///
/// The row's exclusion column reads "`reference` (= the field that holds its own CID)" with the
/// reason "self-reference",
/// so this is the one projection in this crate that is **not** the whole struct -- including
/// `reference` would ask the value to contain its own digest.
#[derive(Debug, Serialize)]
pub struct PlannedDeltaView<'a> {
    pub substrate: &'a SubstrateKind,
    pub payload: PayloadView<'a>,
}

impl IdentityView for PlannedDelta {
    type View<'a> = PlannedDeltaView<'a>;

    fn identity_view(&self) -> PlannedDeltaView<'_> {
        PlannedDeltaView {
            substrate: &self.substrate,
            payload: PayloadView(&self.payload),
        }
    }
}

impl PlannedDelta {
    /// Build one from an adapter's own encoding of a change, and mint its own reference.
    ///
    /// **M4H1-3, adopted (a)** (req/38 §29): "turn `PlannedDelta::new` into a `Result` and mint
    /// `DeltaRef` from the projection (the same window that completes E-M4-26's gx-canon row). The
    /// same 'CID's destination' bundle as M4-14 (residual)". 42 §3.4 requires `reference` to be "its
    /// own canonical reference (matches `DeltaRef.cid`)" and 42 §1.3 fixes what is
    /// hashed -- `{substrate, payload}` -- so the only way the field can be *right* rather than
    /// *claimed* is for the constructor to compute it.
    ///
    /// # The placeholder, and why it cannot leak
    ///
    /// A value has to exist before [`gx_canon::cid::compute`] can project it, so the struct is built
    /// with an all-zero `cid` and the real one is written over it. That is sound precisely because
    /// 42 §1.3 excludes `reference` from the projection: the placeholder is not in the preimage, so
    /// the digest cannot depend on it. `crates/gx-substrate/tests/planned_delta_identity.rs` measures
    /// that rather than trusting the argument -- two deltas built through this constructor with the
    /// same substrate and payload have the same reference, and the projection has exactly two keys.
    ///
    /// # Errors
    /// [`Error::NotDigestible`] when the projection has no canonical form. A delta that cannot be
    /// encoded has no identity, and saying so is better than naming it with an approximation
    /// (`req/26` §3).
    pub fn new(substrate: SubstrateKind, payload: Vec<u8>) -> Result<Self> {
        let mut delta = Self {
            substrate: substrate.clone(),
            payload,
            reference: DeltaRef {
                substrate: substrate.clone(),
                cid: Cid([0u8; 32]),
            },
        };
        let cid = cid::compute(&delta).map_err(|e| Error::NotDigestible {
            detail: e.to_string(),
        })?;
        delta.reference = DeltaRef { substrate, cid };
        Ok(delta)
    }

    /// Whose grammar the payload is written in (42 §3.4).
    #[must_use]
    pub fn substrate(&self) -> &SubstrateKind {
        &self.substrate
    }

    /// The opaque change description (42 §3.4, P-6).
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// This delta's own canonical reference (42 §3.4).
    #[must_use]
    pub fn reference(&self) -> &DeltaRef {
        &self.reference
    }
}

/// What an adapter observed after applying a delta (42 §3.4).
///
/// The asymmetry with [`PlannedDelta`] is 41 §4's and worth reading out loud: `apply` does not take
/// a pre-state and does not return a post-state, so what comes back is an *observation* -- a
/// fingerprint of the scope and a digest of the object -- rather than the new world. req/69 §3.1
/// puts it as "post is not a return value but an observation".
///
/// No `PartialEq`: `postcondition` is a [`Fingerprint`], whose comparison is
/// [`Fingerprint::cas_eq`] and not `==` (**E-M4-15**).
#[derive(Clone, Debug)]
pub struct AppliedDelta {
    delta: DeltaRef,
    postcondition: Fingerprint,
    resulting_digest: Cid,
    applied_at: Timestamp,
    /// 🔴 The observation seat (`req/38` §98 ruling 1, DR-1 option A / two-phase escrow).
    ///
    /// The raw bytes the substrate's own answer to the applied call carried, when the adapter made
    /// the call **in this entry** — `None` on a retry the adapter's idempotency short-circuited
    /// (the answer is not re-obtainable, which is why an engine that needs it journals it), and
    /// `None` for every adapter that has no do-time answer to report (fs, git: the constructor
    /// leaves the seat empty, so nothing about the existing adapters moves).
    ///
    /// This is deliberately **not** a second source for the post-state (the module's asymmetry
    /// note): `postcondition` and `resulting_digest` stay the read-back. What this carries is
    /// material for the one thing a read-back cannot supply — a do-time server-assigned value a
    /// declared inverse needs (`InverseCompletion`), recorded as an observation rather than
    /// trusted as a state.
    observation: Option<Vec<u8>>,
}

impl AppliedDelta {
    /// Record an application. The moment is injected (**M4-17**, 41 §6).
    #[must_use]
    pub fn new(
        delta: DeltaRef,
        postcondition: Fingerprint,
        resulting_digest: Cid,
        applied_at: Timestamp,
    ) -> Self {
        Self {
            delta,
            postcondition,
            resulting_digest,
            applied_at,
            observation: None,
        }
    }

    /// The same record, carrying what the substrate's answer to the call said (the observation
    /// seat above). A builder rather than a fifth `new` argument so that the three shipped
    /// adapters and every existing constructor call read exactly as before (additive; `req/38`
    /// §98 ruling 1's permitted gx-substrate change).
    #[must_use]
    pub fn with_observation(mut self, observation: Vec<u8>) -> Self {
        self.observation = Some(observation);
        self
    }

    /// The do-time answer, where the adapter captured one. `None` is "no answer was captured in
    /// this entry" — a retry, or an adapter with nothing to report — and is not a failure.
    #[must_use]
    pub fn observation(&self) -> Option<&[u8]> {
        self.observation.as_deref()
    }

    /// Which delta was applied (42 §3.4).
    #[must_use]
    pub fn delta(&self) -> &DeltaRef {
        &self.delta
    }

    /// The substrate-state fingerprint after applying (42 §3.4).
    #[must_use]
    pub fn postcondition(&self) -> &Fingerprint {
        &self.postcondition
    }

    /// The digest of the object's content after applying -- the candidate new
    /// `ObjectSnapshot.digest` (42 §3.4).
    ///
    /// 42 §3.4 is the only place in the canon that names this field, and req/69 §4 M4-06 measured
    /// that nothing compares it with the `target` a `plan` promised. **M4-06** was ruled case (b):
    /// the check becomes a conformance property (L5) in hand 5, and the engine-side refusal is
    /// raised for M5. This accessor exists so that both can be written; this hand asserts nothing
    /// about the value.
    #[must_use]
    pub fn resulting_digest(&self) -> &Cid {
        &self.resulting_digest
    }

    /// When the engine says this was applied (42 §3.4, metadata).
    #[must_use]
    pub fn applied_at(&self) -> Timestamp {
        self.applied_at
    }
}
