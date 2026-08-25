// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Why a change happened, and who caused it.
//!
//! Spec: 41 §3 for `ChangeContext`, 42 §0 for this module's contents
//! (`ChangeContext`, `Actor`), 42 §3.2 for the auxiliary type table.

use serde::{Deserialize, Serialize};

/// The kind of change a transformation belongs to (P-3, 41 §3, 42 §3.2).
///
/// This is a classification carried alongside the change, not a cause the core reasons about.
/// `Custom` keeps the enumeration open without turning it into a registry -- the same shape as
/// `SubstrateKind::Custom`, and for the same reason.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ChangeContext {
    /// The change tracks the passage of time (42 §3.2's first row): scheduled or clock-driven.
    Time,
    /// The change responds to new evidence -- an observation arrived and the state follows it.
    Evidence,
    /// The change enacts a policy decision (the gate's vocabulary, not the gate itself).
    Policy,
    /// The change comes from a model update: the thing that decides changed, so the state did.
    Model,
    /// The change is representational only -- same content, different spelling (P-10's axis).
    Representation,
    /// The change originates in the substrate itself, e.g. an external write being reconciled.
    Substrate,
    /// An open classification for contexts 42 §3.2 does not enumerate. A `String` rather than a
    /// registry, for the reason the enum's own doc gives.
    Custom(String),
}

/// A public-key reference. 42 §3.2: `KeyId = String`, "the same namespace as DSSE's `keyid`"
/// (sem: SEM-gx-core-008) -- the same string that names the key in a DSSE signature names the actor
/// here, so a receipt and a transformation can be joined without a translation table.
///
/// AC-006 calls this type `PubKeyRef` as an example (its text says "e.g."; sem: SEM-gx-core-009);
/// the name that 42 §3.2 fixes is the
/// one used. An alias rather than a newtype is the literal reading of `KeyId = String`, and it is
/// also what keeps the DSSE namespace claim true: a wrapper would be a second, gx-only namespace.
pub type KeyId = String;

/// Who caused a change (41 §3, 42 §3.2).
///
/// All three variants carry `key: KeyId`. That sameness is the requirement (FR-006, C-6, P-7):
/// accountability does not depend on whether the actor was a person, an agent or a process, so
/// nothing downstream may have to branch on the variant to find out which key to check.
/// `Agent` adds `model` because that is the one fact about an agent a human reviewer needs and
/// cannot recover from the key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Actor {
    /// A person, named by the key they sign with.
    Human {
        /// The DSSE `keyid` this person is accountable under (FR-006).
        key: KeyId,
    },
    /// An AI agent. The one variant with a second field, because 42 §3.2 asks which model acted
    /// and a key alone cannot answer it.
    Agent {
        /// The DSSE `keyid` the agent is accountable under (FR-006).
        key: KeyId,
        /// The model that acted, e.g. a model id string. Metadata for the human reviewer;
        /// nothing in the core branches on it.
        model: String,
    },
    /// An unattended process -- automation that is neither a person nor a model.
    Process {
        /// The DSSE `keyid` the process is accountable under (FR-006).
        key: KeyId,
    },
}

impl Actor {
    /// The key, whichever variant this is. One signature, no `ReprKind`-style branching.
    #[must_use]
    pub fn key(&self) -> &KeyId {
        match self {
            Actor::Human { key } | Actor::Agent { key, .. } | Actor::Process { key } => key,
        }
    }
}
