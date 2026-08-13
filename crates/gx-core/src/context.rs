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
    Time,
    Evidence,
    Policy,
    Model,
    Representation,
    Substrate,
    Custom(String),
}

/// A public-key reference. 42 §3.2: `KeyId = String`, "DSSE `keyid`と同一名前空間" -- the same
/// string that names the key in a DSSE signature names the actor here, so a receipt and a
/// transformation can be joined without a translation table.
///
/// AC-006 calls this type `PubKeyRef` as an example (「例:」); the name that 42 §3.2 fixes is the
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
    Human { key: KeyId },
    Agent { key: KeyId, model: String },
    Process { key: KeyId },
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
