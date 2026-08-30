// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **req/824 A1 + A2** — the observation-class transformation objects, and the env-var
//! fingerprint codec with its plaintext detector.
//!
//! An observation is *evidence that a record was presented by an attach-source*, never evidence
//! that the operation occurred (`req/824` §1 R-1, LIMITS row shipped with the same commit). The
//! four classes are `req/812` §1's, consumed as-is; the wire projection is
//! `req/wire/schema/observation.schema.json` and the fixture bed is
//! `req/wire/fixtures/observation.jsonl` — the adversarial vectors below are *those* vectors, not a
//! parallel set (`req/824` §8-2a: a declared fold nobody tests is a fold that quietly becomes
//! three codes later; the same holds for a fixture bed nobody drives).
//!
//! # Why this module is in gx-core (`req/824` §2, argued against YAGNI)
//!
//! These are transformation *objects*. gx-core already owns `Object`, `Transformation` and the
//! opaque carriers every other layer exchanges; a `gx-observation` crate would put a
//! transformation type outside the crate that defines what a transformation is, and every consumer
//! (`gx-engine`, `gx-api`, `gx-cli`) already depends on gx-core. A boundary with no independent
//! consumer is not a boundary.
//!
//! # What is deliberately NOT here
//!
//! * **No digest is computed.** A-1 keeps every digest in gx-canon; this crate defines the value
//!   and gx-canon's `cbor::encode` is its canonical form (`crates/gx-canon/tests/
//!   observation_canonical.rs` holds the golden vectors and the bit-equal round trips).
//! * **No `Verdict` is minted.** [`EnvsetAdmission`] is a *classification* this codec reports;
//!   judging stays the gate's (41 §4). The gate/engine map `Deny` onto
//!   `gx_engine::Error::PlaintextSecret` and `Escalate` onto `gx_gate::Error::ChainGap`
//!   (`req/824` A3's rows) when the ingest road lands (A5).
//! * **No route, no registry.** Those are `req/824` A4/A5 and stay in gx-api.

use serde::{Deserialize, Serialize};

/// The four observation classes, fixed by `req/812` §1 (consumed as-is by `req/824` §2-1).
///
/// A wire value outside this enum is **refused at decode, not defaulted** — serde's derive has no
/// fallback variant here on purpose, and `crates/gx-core/tests/observation_class.rs` carries the
/// negative control. A silently-defaulted class would file a deploy as an envset and every
/// downstream receipt would be quietly wrong (`req/824` A1). The refusal's user-visible word is
/// `OBSERVATION_CLASS_UNKNOWN` (`gx_engine::Error::ObservationClassUnknown`, A3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservationClass {
    /// `req/812` §1-1: an ordered set of env-var `(name, value_digest, scope)` — [`EnvsetFingerprint`].
    Envset,
    /// `req/812` §1-2: a deploy record (`deploy_id`, `commit_sha`, artifact digests, target env).
    Deploy,
    /// `req/812` §1-3: a config document — the one class where a real escrow-invert can hold,
    /// *when* its [`ObservationSubstrate`] is `Adapter`.
    Config,
    /// `req/812` §1-4: a merkle-rooted log window, digest-only, append-only
    /// (`gx_engine::Error::AppendOnlyClass` is its typed undo refusal, `req/824` A3).
    LogWindow,
}

/// All four, in `req/812` §1's order, so a test can iterate the closed set rather than remember it.
pub const OBSERVATION_CLASSES: [ObservationClass; 4] = [
    ObservationClass::Envset,
    ObservationClass::Deploy,
    ObservationClass::Config,
    ObservationClass::LogWindow,
];

impl ObservationClass {
    /// The wire spelling (`req/wire/schema/observation.schema.json`'s `class` enum), which is also
    /// what serde writes: the two cannot drift because this match and the derive's `kebab-case`
    /// are compared by the round-trip tests.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            ObservationClass::Envset => "envset",
            ObservationClass::Deploy => "deploy",
            ObservationClass::Config => "config",
            ObservationClass::LogWindow => "log-window",
        }
    }

    /// The decode half of [`ObservationClass::as_wire_str`] (`req/824` A5). `None` for a value
    /// outside the enum — the caller's word for that is `OBSERVATION_CLASS_UNKNOWN`
    /// (`gx_engine::Error::ObservationClassUnknown`, the A3 row), and there is deliberately no
    /// fallback variant to default into.
    #[must_use]
    pub fn from_wire_str(value: &str) -> Option<Self> {
        OBSERVATION_CLASSES
            .into_iter()
            .find(|class| class.as_wire_str() == value)
    }
}

/// Where an observed value actually lives — and therefore which undo semantics can honestly hold
/// (`req/824` A1; the field exists so the two are **never conflated**).
///
/// Named `ObservationSubstrate` rather than `req/824` §4 A1's sketch name `Substrate`, because
/// gx-core already exports [`crate::SubstrateKind`] and a bare `Substrate` beside it would be two
/// near-identical words for two different questions ("which adapter namespace" vs "can an adapter
/// reach this at all"). Declared as a delta in `req/836`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObservationSubstrate {
    /// The observed thing lives where a substrate adapter can read it, so prior-state escrow and
    /// the bit-equal round trip (AC-050) apply as for any file.
    Adapter,
    /// The observed thing exists only platform-side. It degrades to record-level semantics, and
    /// its undo is a **typed refusal** (`gx_engine::Error::InverseNotExecutableAtSubstrate`,
    /// SS273) — a declared-mode value rendered as adapter-mode would promise an undo that cannot
    /// happen.
    Declared,
}

/// The attach-source's **own** identifier for the operation it reports (`req/824` §2-1).
///
/// Carried opaquely — it is not our id, and nothing in this crate reads meaning out of it. It is
/// what makes a CI job's retry an idempotent no-op rather than a second candidate, and `req/824`
/// §2-1 promoted it from deploy-only (`req/812` §1-2) to all four classes because the retry
/// hazard is the same for all four. Wire-level `minLength: 1` is the ingest route's check (A5);
/// this carrier holds any string the way [`crate::FingerprintBytes`] holds any 32 bytes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObservationId(pub String);

/// The declared digest form: `blake3:` + exactly 64 lowercase hex characters
/// (`req/wire/schema/observation.schema.json`, `value_digest`'s pattern).
pub const ENVSET_DIGEST_PREFIX: &str = "blake3:";

/// The exact hex length the declared form requires (BLAKE3-256, 32 bytes).
pub const ENVSET_DIGEST_HEX_LEN: usize = 64;

/// 🔴 **The plaintext detector** (`req/824` A2): is this value of the declared digest form?
///
/// A value field that answers `false` here is **Deny**, not "accepted and warned". The reason is a
/// product ruling and not a validation nicety: *Glovrex refuses to become a secrets store*, so the
/// gate refuses the shape that would make it one. The adversarial bed
/// (`req/wire/fixtures/observation.jsonl` w824-observation-00001..00004) is exactly the set of
/// shapes a naive "looks opaque" check would admit: raw plaintext, hex of the wrong length, valid
/// base64 of a plaintext string wearing the prefix, and an empty value.
///
/// What this predicate **cannot** see, said here rather than found later (`req/824` §5-Q7): a
/// correctly-formed digest of a **low-entropy** value passes, because entropy was never given to
/// us — w824-observation-00005 is the accepted declared-limit vector, and the limit lives in
/// `docs/LIMITS.md`'s A2 row shipped with this commit.
#[must_use]
pub fn is_digest_form(value: &str) -> bool {
    let Some(hex) = value.strip_prefix(ENVSET_DIGEST_PREFIX) else {
        return false;
    };
    hex.len() == ENVSET_DIGEST_HEX_LEN
        && hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// One env-var entry: name in clear, value **only** as a salted digest computed client-side.
///
/// The name travels in clear deliberately — the diff is meaningless without it — and a name that
/// is itself sensitive is a **declared** limit (`req/824` §5-Q7), not an overlooked one. The salt
/// is client-side and never transmitted; `req/824` A9's CLI boundary is where that guarantee is
/// made true, because the value must never reach this type at all.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EnvsetEntry {
    name: String,
    value_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scope_tag: Option<String>,
}

impl EnvsetEntry {
    /// Build one. Infallible on purpose: whether `value_digest` is of the declared form is
    /// [`EnvsetFingerprint::admit`]'s question, asked once for the whole set so the refusal can
    /// name the entry — a constructor refusal here would answer Deny one entry at a time and the
    /// third state (chain gap ⇒ Escalate) could never be reached past the first bad entry.
    #[must_use]
    pub fn new(name: String, value_digest: String, scope_tag: Option<String>) -> Self {
        Self {
            name,
            value_digest,
            scope_tag,
        }
    }

    /// The variable's name, in clear (a declared limit, see the type doc).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The salted digest, `blake3:<64 hex>` when admissible.
    #[must_use]
    pub fn value_digest(&self) -> &str {
        &self.value_digest
    }

    /// The adapter-defined sub-scope tag, when the source declares one.
    #[must_use]
    pub fn scope_tag(&self) -> Option<&str> {
        self.scope_tag.as_deref()
    }
}

/// Which project/environment an envset fingerprint is about (`req/812` §1-1's scope).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EnvsetScope {
    /// The platform's project identifier, as the source spells it.
    pub project: String,
    /// The environment within it (`production`, `staging`, …), as the source spells it.
    pub environment: String,
}

/// 🔴 The env-var fingerprint (`req/824` A2): an **ordered** set of entries, chained to the last
/// committed fingerprint for its scope.
///
/// # Ordering is the constructor's, so the canonical form cannot depend on arrival order
///
/// [`EnvsetFingerprint::new`] sorts entries by `(name, value_digest, scope_tag)`. Two sources
/// reporting the same set in different orders therefore produce byte-identical canonical
/// encodings — the golden-vector test in gx-canon is the receipt. Name *uniqueness* is not this
/// type's invariant: the wire schema owns the request shape and the ingest route (A5) owns its
/// validation; a duplicated name here sorts deterministically and changes nothing about the
/// encoding discipline. Declared in `req/836` rather than silently decided.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvsetFingerprint {
    scope: EnvsetScope,
    entries: Vec<EnvsetEntry>,
    /// The last committed fingerprint reference for this scope, as the source last saw it —
    /// `None` claims "this is the first". A claim the ledger disagrees with is a **gap**, and a
    /// gap is [`EnvsetAdmission::Escalate`]: the third state, never silent accept (that would
    /// fabricate a chain) and never Deny (that would discard real evidence). `req/765` §3-8: K8s'
    /// admission webhook has no third state and we do.
    prev: Option<String>,
}

impl EnvsetFingerprint {
    /// Build one, sorting entries into the canonical order (see the type doc).
    #[must_use]
    pub fn new(scope: EnvsetScope, mut entries: Vec<EnvsetEntry>, prev: Option<String>) -> Self {
        entries.sort();
        Self {
            scope,
            entries,
            prev,
        }
    }

    /// The scope this fingerprint is about.
    #[must_use]
    pub fn scope(&self) -> &EnvsetScope {
        &self.scope
    }

    /// The entries, in canonical order.
    #[must_use]
    pub fn entries(&self) -> &[EnvsetEntry] {
        &self.entries
    }

    /// The chain reference (see the field doc).
    #[must_use]
    pub fn prev(&self) -> Option<&str> {
        self.prev.as_deref()
    }

    /// 🔴 **The three-way classification** (`req/824` A2's AC, all three asserted distinctly):
    ///
    /// 1. any value not of the declared digest form ⇒ [`EnvsetAdmission::Deny`], naming the entry
    ///    (`PLAINTEXT_SECRET_REFUSED` at the surface);
    /// 2. otherwise, a chain that does not continue `last_committed` ⇒
    ///    [`EnvsetAdmission::Escalate`] (`CHAIN_GAP_ESCALATE`, a 2xx — the escalation is evidence
    ///    admitted into the third state, not a refusal);
    /// 3. otherwise ⇒ [`EnvsetAdmission::Allow`].
    ///
    /// Deny is checked first on purpose: a plaintext value must never ride an Escalate into the
    /// ledger. `last_committed` is what the caller's ledger holds for this scope — this crate has
    /// no I/O and asks for the fact rather than finding it (41 §6).
    #[must_use]
    pub fn admit(&self, last_committed: Option<&str>) -> EnvsetAdmission {
        for entry in &self.entries {
            if !is_digest_form(&entry.value_digest) {
                return EnvsetAdmission::Deny {
                    name: entry.name.clone(),
                };
            }
        }
        let continuous = match (self.prev.as_deref(), last_committed) {
            (None, None) => true,
            (Some(claimed), Some(held)) => claimed == held,
            _ => false,
        };
        if continuous {
            EnvsetAdmission::Allow
        } else {
            EnvsetAdmission::Escalate {
                claimed_prev: self.prev.clone(),
                last_committed: last_committed.map(str::to_string),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// req/824 A5 — the other three record shapes, typed (req/wire/schema/observation.schema.json)
// ---------------------------------------------------------------------------
//
// 🔴 `deny_unknown_fields` on every one of these IS req/824 §5-Q2's execution-field fence at the
// type level: a payload smuggling `command`, `script`, `entrypoint`, `image`, `cron`, `schedule`,
// `callback_url` or `exec` is refused at decode because *no field outside the schema is accepted
// at all* — the stronger property, of which the eight-word blocklist is a corollary. Fixture
// w824-observation-00016 drives it through the ingest route.

/// `req/812` §1-2's deploy record. Checks that cannot run are typed-absent downstream
/// (`presented_only`), never silently skipped: `commit_sha` resolves only when a git adapter is
/// attached, and [`DeployRecord::attestation`] verifies only when one was supplied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeployRecord {
    /// The platform's own deploy id — also this record's `observation_id` in practice.
    pub deploy_id: String,
    /// The commit the source says it built.
    pub commit_sha: String,
    /// Artifact digests, as the source spells them (`sha256:<hex>` etc.). Carried, not verified
    /// here: verification against served bytes is unobservable (the A6 LIMITS row).
    pub artifact_digests: Vec<String>,
    /// The environment the source says it deployed to.
    pub target_env: String,
    /// The envset fingerprint this deploy ran under, when the source chains one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envset_fingerprint_ref: Option<String>,
    /// A digest over platform metadata, when the source supplies one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_metadata_digest: Option<String>,
    /// A DSSE/in-toto envelope, when the source supplies one (`req/805` P-12: gx-witness is
    /// already on BuildKit attestations' standard).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<DsseEnvelope>,
}

/// A DSSE envelope as a deploy attestation carries one — typed, minimal, and **not verified by
/// this type**: verification is a check that runs where a key is, and an unrun check lands in
/// `presented_only`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DsseEnvelope {
    /// DSSE's `payloadType` (e.g. `application/vnd.in-toto+json`).
    #[serde(rename = "payloadType")]
    pub payload_type: String,
    /// The base64 payload, when carried inline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    /// The signatures, each a `(keyid, sig)` pair.
    pub signatures: Vec<DsseEnvelopeSignature>,
}

/// One signature of a [`DsseEnvelope`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DsseEnvelopeSignature {
    /// DSSE's `keyid`.
    pub keyid: String,
    /// The signature bytes, base64.
    pub sig: String,
}

/// `req/812` §1-3's config record. [`ObservationSubstrate`] keeps adapter-mode and declared-mode
/// from ever being conflated — a declared-mode config rendered as adapter-mode would promise an
/// undo that cannot happen (`req/824` A1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigRecord {
    /// The canonical-encoded config blob. Structural diff is derived engine-side, never sent.
    pub document: String,
    /// Where the config actually lives — which undo semantics can honestly hold.
    pub substrate: ObservationSubstrate,
}

/// `req/812` §1-4's log-window record. Digest-only: log **lines are never stored** (storage would
/// be the F5① cloud seat, out of this phase's scope). `line_count_census` is a census, never a
/// billing input (F4/F6; `req/824` A7 owns the machine gate).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogWindowRecord {
    /// The source's stream identifier.
    pub stream_id: String,
    /// The window bounds, as the source states them.
    pub window: LogWindowBounds,
    /// The merkle root over the exported lines, computed client-side.
    pub merkle_root: String,
    /// How many lines the window covered. A census (see the type doc).
    pub line_count_census: u64,
    /// `true` is admissible and is drawn as a declared hole (`req/784` A-26/I-5), never smoothed
    /// over.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<bool>,
}

/// A log window's bounds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogWindowBounds {
    /// RFC 3339, as the source spells it — carried opaquely (timing skew between platform time
    /// and report time is a declared limit, `req/824` A2).
    pub t_start: String,
    /// The window's end, same spelling.
    pub t_end: String,
}

/// One observation, typed by class — what the ingest road (`req/824` A5) hands the engine after
/// the wire JSON has been parsed at the membrane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationRecord {
    /// An env-var fingerprint (`req/812` §1-1), already in canonical entry order.
    Envset(EnvsetFingerprint),
    /// A deploy record (`req/812` §1-2).
    Deploy(DeployRecord),
    /// A config record (`req/812` §1-3).
    Config(ConfigRecord),
    /// A log-window record (`req/812` §1-4).
    LogWindow(LogWindowRecord),
}

impl ObservationRecord {
    /// Which class this record is — total, so the two enums cannot drift.
    #[must_use]
    pub const fn class(&self) -> ObservationClass {
        match self {
            ObservationRecord::Envset(_) => ObservationClass::Envset,
            ObservationRecord::Deploy(_) => ObservationClass::Deploy,
            ObservationRecord::Config(_) => ObservationClass::Config,
            ObservationRecord::LogWindow(_) => ObservationClass::LogWindow,
        }
    }
}

/// What [`EnvsetFingerprint::admit`] answers — a classification, not a `Verdict` (41 §4 keeps the
/// one judgement in `Gate::verify`; this is the codec reporting a fact the gate consumes).
///
/// Three variants on purpose, and the tests assert each **as its own variant, never as
/// `!= Allow`** — folding `Escalate` into `Deny` is exactly the collapse `req/824` §5-Q6 exists to
/// prevent, and deriving it from "not allowed" is how that collapse starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnvsetAdmission {
    /// Every value is of the declared form and the chain continues.
    Allow,
    /// A value field is not of the declared digest form. `name` is the offending entry, so the
    /// refusal can say which variable without ever having seen its value.
    Deny {
        /// The entry whose value was refused.
        name: String,
    },
    /// The chain does not continue what the ledger holds: a **gap**, admitted into the third
    /// state. Both sides of the disagreement are carried so the escalation can print them.
    Escalate {
        /// What the source claimed as its predecessor (`None` = claimed first).
        claimed_prev: Option<String>,
        /// What the caller's ledger actually holds for this scope (`None` = nothing).
        last_committed: Option<String>,
    },
}
