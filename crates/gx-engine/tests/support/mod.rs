// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Fixtures shared by M5 hand 1's suites.
//!
//! Not a test target: cargo builds one integration binary per `.rs` file directly under `tests/`,
//! and a file in a subdirectory is only ever compiled as a module of one that declares it. So a
//! helper here raises no `test result:` line of its own, which is what keeps the e2e floor of
//! `tools/e2e.sh` counting suites rather than support code. Lifted from `gx-log/tests/support`,
//! where the shape and its reasoning are written out.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gx_core::{
    AbortReason, Cid, Fingerprint, IntentId, ObjectId, SubstrateKind, Timestamp, TransformationId,
    VerdictKind,
};
use gx_engine::store::FingerprintRecord;
use gx_engine::{EngineJournalRecord, Rollback};
use gx_witness::{Environment, Provenance};

/// A distinguishable digest. Not a real hash of anything.
pub fn cid(seed: u64) -> Cid {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&seed.to_be_bytes());
    Cid(raw)
}

/// A distinguishable transformation id, in a range disjoint from the plain digests so a swapped
/// argument shows up as a wrong value rather than as a coincidence.
pub fn tid(seed: u64) -> TransformationId {
    TransformationId(cid(9_000_000 + seed))
}

/// A distinguishable intent id, in a third range.
pub fn iid(seed: u64) -> IntentId {
    IntentId(cid(7_000_000 + seed))
}

/// A fingerprint record with distinguishable components.
pub fn fp(seed: u64) -> FingerprintRecord {
    FingerprintRecord::of(
        &Fingerprint::new(SubstrateKind::Fs, format!("/tmp/scope-{seed}"), cid(seed))
            .expect("a short scope is inside MAX_SCOPE_BYTES"),
    )
}

/// A distinguishable ruler: the `Actor` 43 T-5's human decision is attributed to.
///
/// `Human`, not `Agent`: 43 T-7's guard names "the equivalent of `Actor::Human{key}`" and AC-071 asks
/// for the ruling actor (sem: SEM-gx-engine-928). P-7 keeps accountability independent of the variant, so nothing in the engine branches
/// on it -- the variant is what a deployment writes down about who ruled.
pub fn ruler(seed: u8) -> gx_core::Actor {
    gx_core::Actor::Human {
        key: format!("key-owner-{seed}"),
    }
}

/// A distinguishable provenance record (42 §3.9), for the journal fixtures.
///
/// Built field by field rather than through `Provenance::derive_from`, because the fixtures need
/// values that differ in one place at a time and the deriving function decides two of the fields
/// from a `Transformation`.
pub fn provenance(seed: u64) -> Provenance {
    Provenance {
        environment: Environment {
            host_id: None,
            adapter_kind: SubstrateKind::Fs,
            correlation_id: None,
            engine_version: format!("0.1.{seed}"),
            adapter_version: "test".to_string(),
            confinement: Some(gx_witness::receipt::ConfinementContext::unconfined()),
        },
        input_objects: vec![ObjectId(cid(seed))],
        intent_digest: Some(cid(seed + 1)),
        transformation: tid(1),
    }
}

/// One sample of every variant of [`EngineJournalRecord`], in `JOURNAL_RECORD_KINDS` order.
///
/// A catalogue rather than fifteen literals repeated in four suites: the A-10 key counts, the I-1
/// digest sensitivities, the round-trip property and the vocabulary comparison all need the same
/// fifteen values, and fifteen values written four times are four lists that can drift.
pub fn every_variant() -> Vec<EngineJournalRecord> {
    vec![
        EngineJournalRecord::DraftCreated {
            intent_id: iid(1),
            rng_seed: 42,
            at: Timestamp(100),
        },
        EngineJournalRecord::Planned {
            transformation: tid(1),
            intent_id: iid(1),
            locator: "/tmp/one".to_string(),
            delta_cid: cid(11),
            fp0: fp(1),
            parents: Vec::new(),
            // 🔴 **DR-46-33** — non-`Unknown` so the flag reaches the wire in the sample, in
            // `pending`/`reads_attested`'s reason above (skip_serializing_if drops `Unknown`).
            input_generation: gx_core::BoundaryStage::LlmOriginated,
            // 🔴 **DR-46-45 (`req/973` §B-1)** — `Some` for `input_generation`'s reason one line
            // up: `skip_serializing_if` drops a `None`, and a sample that never fills the seat
            // would let `journal_identity.rs` report a field count that no encoding ever carried.
            // The sample is therefore an undo's plan, which is the only road that fills it.
            undo_witness: Some(gx_witness::receipt::UndoDisposition::Attested),
            at: Timestamp(101),
        },
        EngineJournalRecord::VerifyStarted {
            transformation: tid(1),
            at: Timestamp(102),
        },
        EngineJournalRecord::Verdict {
            transformation: tid(1),
            kind: VerdictKind::Admit,
            verdict_digest: Some(cid(12)),
            fail_posture_engaged: false,
            at: Timestamp(103),
        },
        EngineJournalRecord::HumanDecision {
            transformation: tid(1),
            kind: VerdictKind::Deny,
            reason: "the policy set forbids this locator".to_string(),
            actor: ruler(1),
            // 🔴 **DR-46-31** — `Some` on purpose. The field is `skip_serializing_if`
            // (E-M5-13's shape, so pre-DR-46-31 journals still encode byte-identically), and
            // `None` would take that arm: `journal_identity.rs` compares the encoder's keys
            // against the struct's fields and would find the field declared and never written.
            verdict_digest: Some(cid(17)),
            at: Timestamp(104),
        },
        EngineJournalRecord::Canonicalized {
            transformation: tid(1),
            canonical_cid: cid(13),
            enforced: Some(false),
            at: Timestamp(105),
        },
        EngineJournalRecord::CommittingStarted {
            transformation: tid(1),
            at: Timestamp(106),
        },
        EngineJournalRecord::ProvenanceDerived {
            transformation: tid(1),
            provenance: provenance(20),
            at: Timestamp(106),
        },
        // `pending: true` so the flag reaches the wire in the sample (skip_serializing_if
        // omits `false`, and the identity probes compare encoder keys against struct fields).
        EngineJournalRecord::InverseEscrowed {
            transformation: tid(1),
            inverse_cid: Some(cid(14)),
            pending: true,
            // 🔴 **DR-46-26** — non-empty on purpose. `reads` is `skip_serializing_if` (E-M5-13's
            // shape, so that journals written before this field still encode byte-identically), and
            // an empty vector would take that arm — `journal_identity.rs` compares the encoder's
            // keys against the struct's fields and would find the field declared and never written.
            reads: baseline_reads(),
            // 🔴 **DR-46-26** — `true` here, and this fixture is about the **wire shape** rather
            // than about a reachable engine state. `undetermined` is `skip_serializing_if`, so the
            // default is not written at all and A-10's key count would never see the field; the
            // same reason `pending` is `true` two lines up. The combination `Some(cid) + true` is
            // one this crate's writer never produces (an inverse was constructed, so C-25 answered
            // `True`), and `replay.rs` says exactly what it does with a foreign journal that spells
            // it: folds it to the CID's own side.
            undetermined: true,
            // 🔴 **DR-46-34** — `true` for `undetermined`'s reason one line up: the flag is
            // `skip_serializing_if = "is_false"`, so `false` is not written at all and A-10's key
            // count would never see the field the lane just declared.
            reads_attested: true,
            at: Timestamp(107),
        },
        EngineJournalRecord::ApplyStarted {
            transformation: tid(1),
            delta_cid: cid(11),
            at: Timestamp(108),
        },
        EngineJournalRecord::ApplyObserved {
            transformation: tid(1),
            observation_cid: cid(15),
            at: Timestamp(108),
        },
        EngineJournalRecord::InverseCompleted {
            transformation: tid(1),
            inverse_cid: Some(cid(16)),
            at: Timestamp(108),
        },
        EngineJournalRecord::Committed {
            transformation: tid(1),
            ledger_seq: 7,
            at: Timestamp(109),
        },
        EngineJournalRecord::Aborted {
            transformation: tid(1),
            reason: AbortReason::PreconditionChanged,
            rollback: None,
            at: Timestamp(110),
        },
        EngineJournalRecord::Superseded {
            transformation: tid(1),
            by: tid(2),
            at: Timestamp(111),
        },
    ]
}

/// The same fifteen with **one field changed**, one entry per field: `(variant, field, record)`.
///
/// This is the I-1 half of the defence req/78 §6.0-5 asks for -- "two values that differ in exactly
/// one field digest differently" (sem: SEM-gx-engine-929) -- for every field -- and it is written out rather than generated because a generator would
/// need to know the field list, which is the thing being checked.
/// 🔴 **DR-46-26** — the read-set entries `every_variant`'s `InverseEscrowed` carries.
///
/// One function rather than a literal in each place, because `journal_identity.rs`'s I-1 gate
/// requires every mutant to differ from the baseline in **one** field: two copies of this value
/// that drifted apart would make every `InverseEscrowed` mutant a two-field mutant, and the gate
/// would then be measuring nothing while staying green.
///
/// Non-empty on purpose: the field is `skip_serializing_if = "Vec::is_empty"` (E-M5-13's shape),
/// so an empty vector is not written at all and A-10's key count would never see it.
pub fn baseline_reads() -> Vec<gx_core::ReadEntry> {
    vec![gx_core::ReadEntry {
        digest: cid(15),
        locator: "fixture://escrow/read/one".to_string(),
    }]
}

pub fn one_field_changed() -> Vec<(&'static str, &'static str, EngineJournalRecord)> {
    vec![
        (
            "DraftCreated",
            "intent_id",
            EngineJournalRecord::DraftCreated {
                intent_id: iid(2),
                rng_seed: 42,
                at: Timestamp(100),
            },
        ),
        (
            "DraftCreated",
            "rng_seed",
            EngineJournalRecord::DraftCreated {
                intent_id: iid(1),
                rng_seed: 43,
                at: Timestamp(100),
            },
        ),
        (
            "DraftCreated",
            "at",
            EngineJournalRecord::DraftCreated {
                intent_id: iid(1),
                rng_seed: 42,
                at: Timestamp(999),
            },
        ),
        (
            "Planned",
            "transformation",
            EngineJournalRecord::Planned {
                transformation: tid(9),
                intent_id: iid(1),
                locator: "/tmp/one".to_string(),
                delta_cid: cid(11),
                fp0: fp(1),
                parents: Vec::new(),
                input_generation: gx_core::BoundaryStage::LlmOriginated,
                // DR-46-45 (`req/973` §B-1): the baseline's value, so this mutant differs from it
                // in the field it is named for and in nothing else.
                undo_witness: Some(gx_witness::receipt::UndoDisposition::Attested),
                at: Timestamp(101),
            },
        ),
        (
            "Planned",
            "intent_id",
            EngineJournalRecord::Planned {
                transformation: tid(1),
                intent_id: iid(9),
                locator: "/tmp/one".to_string(),
                delta_cid: cid(11),
                fp0: fp(1),
                parents: Vec::new(),
                input_generation: gx_core::BoundaryStage::LlmOriginated,
                // DR-46-45 (`req/973` §B-1): the baseline's value, so this mutant differs from it
                // in the field it is named for and in nothing else.
                undo_witness: Some(gx_witness::receipt::UndoDisposition::Attested),
                at: Timestamp(101),
            },
        ),
        (
            "Planned",
            "delta_cid",
            EngineJournalRecord::Planned {
                transformation: tid(1),
                intent_id: iid(1),
                locator: "/tmp/one".to_string(),
                delta_cid: cid(99),
                fp0: fp(1),
                parents: Vec::new(),
                input_generation: gx_core::BoundaryStage::LlmOriginated,
                // DR-46-45 (`req/973` §B-1): the baseline's value, so this mutant differs from it
                // in the field it is named for and in nothing else.
                undo_witness: Some(gx_witness::receipt::UndoDisposition::Attested),
                at: Timestamp(101),
            },
        ),
        (
            "Planned",
            "fp0",
            EngineJournalRecord::Planned {
                transformation: tid(1),
                intent_id: iid(1),
                locator: "/tmp/one".to_string(),
                delta_cid: cid(11),
                fp0: fp(9),
                parents: Vec::new(),
                input_generation: gx_core::BoundaryStage::LlmOriginated,
                // DR-46-45 (`req/973` §B-1): the baseline's value, so this mutant differs from it
                // in the field it is named for and in nothing else.
                undo_witness: Some(gx_witness::receipt::UndoDisposition::Attested),
                at: Timestamp(101),
            },
        ),
        (
            "Planned",
            "at",
            EngineJournalRecord::Planned {
                transformation: tid(1),
                intent_id: iid(1),
                locator: "/tmp/one".to_string(),
                delta_cid: cid(11),
                fp0: fp(1),
                parents: Vec::new(),
                input_generation: gx_core::BoundaryStage::LlmOriginated,
                // DR-46-45 (`req/973` §B-1): the baseline's value, so this mutant differs from it
                // in the field it is named for and in nothing else.
                undo_witness: Some(gx_witness::receipt::UndoDisposition::Attested),
                at: Timestamp(999),
            },
        ),
        (
            "Planned",
            "locator",
            EngineJournalRecord::Planned {
                transformation: tid(1),
                intent_id: iid(1),
                locator: "/tmp/two".to_string(),
                delta_cid: cid(11),
                fp0: fp(1),
                parents: Vec::new(),
                input_generation: gx_core::BoundaryStage::LlmOriginated,
                // DR-46-45 (`req/973` §B-1): the baseline's value, so this mutant differs from it
                // in the field it is named for and in nothing else.
                undo_witness: Some(gx_witness::receipt::UndoDisposition::Attested),
                at: Timestamp(101),
            },
        ),
        (
            "Planned",
            "parents",
            EngineJournalRecord::Planned {
                transformation: tid(1),
                intent_id: iid(1),
                locator: "/tmp/one".to_string(),
                delta_cid: cid(11),
                fp0: fp(1),
                parents: vec![tid(9)],
                input_generation: gx_core::BoundaryStage::LlmOriginated,
                // DR-46-45 (`req/973` §B-1): the baseline's value, so this mutant differs from it
                // in the field it is named for and in nothing else.
                undo_witness: Some(gx_witness::receipt::UndoDisposition::Attested),
                at: Timestamp(101),
            },
        ),
        (
            // 🔴 **DR-46-33** — the join's result reaches the digest, so a rebuild that reproduced
            // a different `input_generation` would not reproduce the leaf (43 §7-3b).
            "Planned",
            "input_generation",
            EngineJournalRecord::Planned {
                transformation: tid(1),
                intent_id: iid(1),
                locator: "/tmp/one".to_string(),
                delta_cid: cid(11),
                fp0: fp(1),
                parents: Vec::new(),
                input_generation: gx_core::BoundaryStage::DeterministicReplay,
                // DR-46-45 (`req/973` §B-1): the baseline's value, so this mutant differs from it
                // in the field it is named for and in nothing else.
                undo_witness: Some(gx_witness::receipt::UndoDisposition::Attested),
                at: Timestamp(101),
            },
        ),
        (
            // 🔴 **DR-46-45 (`req/973` §B-1)** — the CAS answer reaches the digest, which is what
            // makes the journalled witness reproducible rather than merely recorded: a rebuild that
            // read back a different disposition would not reproduce the leaf (43 §7-3b), and a
            // field that did not move the digest could be edited in a journal without the
            // reconstruction noticing.
            "Planned",
            "undo_witness",
            EngineJournalRecord::Planned {
                transformation: tid(1),
                intent_id: iid(1),
                locator: "/tmp/one".to_string(),
                delta_cid: cid(11),
                fp0: fp(1),
                parents: Vec::new(),
                input_generation: gx_core::BoundaryStage::LlmOriginated,
                // The other arm. `Unobservable` is the one that says the inverse was fired without
                // a compare-and-swap, so this mutant is the pair `req/973` §B-1 exists to keep
                // distinguishable.
                undo_witness: Some(gx_witness::receipt::UndoDisposition::Unobservable {
                    reason: "the commit receipt carries no postcondition".to_string(),
                }),
                at: Timestamp(101),
            },
        ),
        (
            "VerifyStarted",
            "transformation",
            EngineJournalRecord::VerifyStarted {
                transformation: tid(9),
                at: Timestamp(102),
            },
        ),
        (
            "VerifyStarted",
            "at",
            EngineJournalRecord::VerifyStarted {
                transformation: tid(1),
                at: Timestamp(999),
            },
        ),
        (
            "Verdict",
            "transformation",
            EngineJournalRecord::Verdict {
                transformation: tid(9),
                kind: VerdictKind::Admit,
                verdict_digest: Some(cid(12)),
                fail_posture_engaged: false,
                at: Timestamp(103),
            },
        ),
        (
            "Verdict",
            "kind",
            EngineJournalRecord::Verdict {
                transformation: tid(1),
                kind: VerdictKind::Escalate,
                verdict_digest: Some(cid(12)),
                fail_posture_engaged: false,
                at: Timestamp(103),
            },
        ),
        (
            "Verdict",
            "verdict_digest",
            EngineJournalRecord::Verdict {
                transformation: tid(1),
                kind: VerdictKind::Admit,
                verdict_digest: Some(cid(99)),
                fail_posture_engaged: false,
                at: Timestamp(103),
            },
        ),
        // 🔴 The T-4e shape, and the reason `verdict_digest` is an `Option`. Both differences from
        // the baseline are the ones 43 T-4e writes, and both have to move the digest: a journal in
        // which "the gate admitted this" and "nothing was reachable, so we continued" (sem: SEM-gx-engine-930) hash alike is
        // a journal in which INV-S5's distinction is not recorded.
        (
            "Verdict",
            "fail_posture_engaged",
            EngineJournalRecord::Verdict {
                transformation: tid(1),
                kind: VerdictKind::Admit,
                verdict_digest: Some(cid(12)),
                fail_posture_engaged: true,
                at: Timestamp(103),
            },
        ),
        (
            "Verdict",
            "verdict_digest(None)",
            EngineJournalRecord::Verdict {
                transformation: tid(1),
                kind: VerdictKind::Admit,
                verdict_digest: None,
                fail_posture_engaged: false,
                at: Timestamp(103),
            },
        ),
        (
            "Verdict",
            "at",
            EngineJournalRecord::Verdict {
                transformation: tid(1),
                kind: VerdictKind::Admit,
                verdict_digest: Some(cid(12)),
                fail_posture_engaged: false,
                at: Timestamp(999),
            },
        ),
        (
            "HumanDecision",
            "transformation",
            EngineJournalRecord::HumanDecision {
                transformation: tid(9),
                kind: VerdictKind::Deny,
                reason: "the policy set forbids this locator".to_string(),
                actor: ruler(1),
                verdict_digest: Some(cid(17)),
                at: Timestamp(104),
            },
        ),
        (
            "HumanDecision",
            "kind",
            EngineJournalRecord::HumanDecision {
                transformation: tid(1),
                kind: VerdictKind::Admit,
                reason: "the policy set forbids this locator".to_string(),
                actor: ruler(1),
                verdict_digest: Some(cid(17)),
                at: Timestamp(104),
            },
        ),
        // 🔴 AC-071/072 need both new fields in the digest, or two rulings that differ only in
        // what the person said -- or in who said it -- would hash alike, which is the record
        // failing to record the one thing the acceptance criteria ask it to.
        (
            "HumanDecision",
            "reason",
            EngineJournalRecord::HumanDecision {
                transformation: tid(1),
                kind: VerdictKind::Deny,
                reason: "a different sentence entirely".to_string(),
                actor: ruler(1),
                verdict_digest: Some(cid(17)),
                at: Timestamp(104),
            },
        ),
        (
            "HumanDecision",
            "actor",
            EngineJournalRecord::HumanDecision {
                transformation: tid(1),
                kind: VerdictKind::Deny,
                reason: "the policy set forbids this locator".to_string(),
                actor: ruler(2),
                verdict_digest: Some(cid(17)),
                at: Timestamp(104),
            },
        ),
        // 🔴 **DR-46-31** — two mutants, for the same reason `Verdict.verdict_digest` has two.
        // "a different proof" and "no proof recorded" are different records: the second is a
        // pre-DR-46-31 journal, which `replay.rs` degrades rather than repairs, and a journal that
        // hashed the two alike would let one be re-encoded as the other.
        (
            "HumanDecision",
            "verdict_digest",
            EngineJournalRecord::HumanDecision {
                transformation: tid(1),
                kind: VerdictKind::Deny,
                reason: "the policy set forbids this locator".to_string(),
                actor: ruler(1),
                verdict_digest: Some(cid(18)),
                at: Timestamp(104),
            },
        ),
        (
            "HumanDecision",
            "verdict_digest(None)",
            EngineJournalRecord::HumanDecision {
                transformation: tid(1),
                kind: VerdictKind::Deny,
                reason: "the policy set forbids this locator".to_string(),
                actor: ruler(1),
                verdict_digest: None,
                at: Timestamp(104),
            },
        ),
        (
            "HumanDecision",
            "at",
            EngineJournalRecord::HumanDecision {
                transformation: tid(1),
                kind: VerdictKind::Deny,
                reason: "the policy set forbids this locator".to_string(),
                actor: ruler(1),
                verdict_digest: Some(cid(17)),
                at: Timestamp(999),
            },
        ),
        (
            "Canonicalized",
            "transformation",
            EngineJournalRecord::Canonicalized {
                transformation: tid(9),
                canonical_cid: cid(13),
                enforced: Some(false),
                at: Timestamp(105),
            },
        ),
        (
            "Canonicalized",
            "canonical_cid",
            EngineJournalRecord::Canonicalized {
                transformation: tid(1),
                canonical_cid: cid(99),
                enforced: Some(false),
                at: Timestamp(105),
            },
        ),
        (
            "Canonicalized",
            "enforced",
            EngineJournalRecord::Canonicalized {
                transformation: tid(1),
                canonical_cid: cid(13),
                enforced: None,
                at: Timestamp(105),
            },
        ),
        (
            "Canonicalized",
            "at",
            EngineJournalRecord::Canonicalized {
                transformation: tid(1),
                canonical_cid: cid(13),
                enforced: Some(false),
                at: Timestamp(999),
            },
        ),
        (
            "CommittingStarted",
            "transformation",
            EngineJournalRecord::CommittingStarted {
                transformation: tid(9),
                at: Timestamp(106),
            },
        ),
        (
            "CommittingStarted",
            "at",
            EngineJournalRecord::CommittingStarted {
                transformation: tid(1),
                at: Timestamp(999),
            },
        ),
        (
            "ProvenanceDerived",
            "transformation",
            EngineJournalRecord::ProvenanceDerived {
                transformation: tid(9),
                provenance: provenance(20),
                at: Timestamp(106),
            },
        ),
        (
            "ProvenanceDerived",
            "provenance",
            EngineJournalRecord::ProvenanceDerived {
                transformation: tid(1),
                provenance: provenance(21),
                at: Timestamp(106),
            },
        ),
        (
            "ProvenanceDerived",
            "at",
            EngineJournalRecord::ProvenanceDerived {
                transformation: tid(1),
                provenance: provenance(20),
                at: Timestamp(999),
            },
        ),
        (
            "InverseEscrowed",
            "transformation",
            EngineJournalRecord::InverseEscrowed {
                transformation: tid(9),
                inverse_cid: Some(cid(14)),
                pending: true,
                reads: baseline_reads(),
                undetermined: false,
                reads_attested: true,
                at: Timestamp(107),
            },
        ),
        (
            "InverseEscrowed",
            "inverse_cid",
            EngineJournalRecord::InverseEscrowed {
                transformation: tid(1),
                inverse_cid: Some(cid(99)),
                pending: true,
                reads: baseline_reads(),
                undetermined: false,
                reads_attested: true,
                at: Timestamp(107),
            },
        ),
        // 🔴 **E-M5-9**: "an inverse was escrowed" and "we asked and there is none" (sem: SEM-gx-engine-931) must not hash
        // alike. This is the row that says the erratum reached the digest and not only the type.
        (
            "InverseEscrowed",
            "inverse_cid(None)",
            EngineJournalRecord::InverseEscrowed {
                transformation: tid(1),
                inverse_cid: None,
                pending: true,
                reads: baseline_reads(),
                undetermined: false,
                reads_attested: true,
                at: Timestamp(107),
            },
        ),
        // 🔴 Two-phase escrow (req/38 §99, ruling 2-1) (sem: SEM-gx-engine-932): a partial escrow and a complete one must
        // not hash alike — the pending flag reaches the digest, not only the type.
        (
            "InverseEscrowed",
            "pending",
            EngineJournalRecord::InverseEscrowed {
                transformation: tid(1),
                inverse_cid: Some(cid(14)),
                pending: false,
                reads: baseline_reads(),
                undetermined: false,
                reads_attested: true,
                at: Timestamp(107),
            },
        ),
        (
            "InverseEscrowed",
            "at",
            EngineJournalRecord::InverseEscrowed {
                transformation: tid(1),
                inverse_cid: Some(cid(14)),
                pending: true,
                reads: baseline_reads(),
                undetermined: false,
                reads_attested: true,
                at: Timestamp(999),
            },
        ),
        // 🔴 **DR-46-26**: the read-set reaches the digest, and not only the wire. A journal that
        // hashed two escrows alike while they attested different objects would let a rebuild
        // reproduce a leaf from the wrong reads — which is the whole reason the field is here.
        // 🔴 **DR-46-26**: the discriminator reaches the digest. Two escrows that answered
        // different C-25 values must not hash alike, or a replay could not tell them apart from
        // the record either -- which is the defect this field exists to close.
        (
            "InverseEscrowed",
            "undetermined",
            EngineJournalRecord::InverseEscrowed {
                transformation: tid(1),
                inverse_cid: Some(cid(14)),
                pending: true,
                reads: baseline_reads(),
                undetermined: false,
                reads_attested: true,
                at: Timestamp(107),
            },
        ),
        (
            "InverseEscrowed",
            "reads",
            EngineJournalRecord::InverseEscrowed {
                transformation: tid(1),
                inverse_cid: Some(cid(14)),
                pending: true,
                reads: vec![gx_core::ReadEntry {
                    digest: cid(16),
                    locator: "fixture://escrow/read/two".to_string(),
                }],
                undetermined: false,
                reads_attested: true,
                at: Timestamp(107),
            },
        ),
        // 🔴 **DR-46-34**: the *provenance* of an empty `reads` reaches the digest, and not only
        // the wire. `reads` is `skip_serializing_if = "Vec::is_empty"`, so a journal that predates
        // 42 §3.13's field and a journal that recorded an empty read-set encode the same `reads`
        // — this flag is the only thing between them, and a digest that hashed the two alike would
        // let a rebuild attest "the escrow read nothing" on the strength of a gap in a file.
        (
            "InverseEscrowed",
            "reads_attested",
            EngineJournalRecord::InverseEscrowed {
                transformation: tid(1),
                inverse_cid: Some(cid(14)),
                pending: true,
                reads: baseline_reads(),
                undetermined: false,
                reads_attested: false,
                at: Timestamp(107),
            },
        ),
        (
            "ApplyStarted",
            "transformation",
            EngineJournalRecord::ApplyStarted {
                transformation: tid(9),
                delta_cid: cid(11),
                at: Timestamp(108),
            },
        ),
        (
            "ApplyStarted",
            "delta_cid",
            EngineJournalRecord::ApplyStarted {
                transformation: tid(1),
                delta_cid: cid(99),
                at: Timestamp(108),
            },
        ),
        (
            "ApplyStarted",
            "at",
            EngineJournalRecord::ApplyStarted {
                transformation: tid(1),
                delta_cid: cid(11),
                at: Timestamp(999),
            },
        ),
        (
            "ApplyObserved",
            "transformation",
            EngineJournalRecord::ApplyObserved {
                transformation: tid(9),
                observation_cid: cid(15),
                at: Timestamp(108),
            },
        ),
        (
            "ApplyObserved",
            "observation_cid",
            EngineJournalRecord::ApplyObserved {
                transformation: tid(1),
                observation_cid: cid(99),
                at: Timestamp(108),
            },
        ),
        (
            "ApplyObserved",
            "at",
            EngineJournalRecord::ApplyObserved {
                transformation: tid(1),
                observation_cid: cid(15),
                at: Timestamp(999),
            },
        ),
        (
            "InverseCompleted",
            "transformation",
            EngineJournalRecord::InverseCompleted {
                transformation: tid(9),
                inverse_cid: Some(cid(16)),
                at: Timestamp(108),
            },
        ),
        (
            "InverseCompleted",
            "inverse_cid",
            EngineJournalRecord::InverseCompleted {
                transformation: tid(1),
                inverse_cid: Some(cid(99)),
                at: Timestamp(108),
            },
        ),
        // The fold and the completion must not hash alike (§99, ruling 2-4's two outcomes) (sem: SEM-gx-engine-933).
        (
            "InverseCompleted",
            "inverse_cid(None)",
            EngineJournalRecord::InverseCompleted {
                transformation: tid(1),
                inverse_cid: None,
                at: Timestamp(108),
            },
        ),
        (
            "InverseCompleted",
            "at",
            EngineJournalRecord::InverseCompleted {
                transformation: tid(1),
                inverse_cid: Some(cid(16)),
                at: Timestamp(999),
            },
        ),
        (
            "Committed",
            "transformation",
            EngineJournalRecord::Committed {
                transformation: tid(9),
                ledger_seq: 7,
                at: Timestamp(109),
            },
        ),
        (
            "Committed",
            "ledger_seq",
            EngineJournalRecord::Committed {
                transformation: tid(1),
                ledger_seq: 8,
                at: Timestamp(109),
            },
        ),
        (
            "Committed",
            "at",
            EngineJournalRecord::Committed {
                transformation: tid(1),
                ledger_seq: 7,
                at: Timestamp(999),
            },
        ),
        (
            "Aborted",
            "transformation",
            EngineJournalRecord::Aborted {
                transformation: tid(9),
                reason: AbortReason::PreconditionChanged,
                rollback: None,
                at: Timestamp(110),
            },
        ),
        (
            "Aborted",
            "reason",
            EngineJournalRecord::Aborted {
                transformation: tid(1),
                reason: AbortReason::ApplyFailed,
                rollback: None,
                at: Timestamp(110),
            },
        ),
        // 🔴 AC-038's outcome has to move the digest, or a journal in which the rollback succeeded
        // and one in which it was never attempted hash alike -- which is the record failing to
        // record the one thing AC-038 asks it to.
        (
            "Aborted",
            "rollback",
            EngineJournalRecord::Aborted {
                transformation: tid(1),
                reason: AbortReason::PreconditionChanged,
                rollback: Some(Rollback::Succeeded),
                at: Timestamp(110),
            },
        ),
        (
            "Aborted",
            "at",
            EngineJournalRecord::Aborted {
                transformation: tid(1),
                reason: AbortReason::PreconditionChanged,
                rollback: None,
                at: Timestamp(999),
            },
        ),
        (
            "Superseded",
            "transformation",
            EngineJournalRecord::Superseded {
                transformation: tid(9),
                by: tid(2),
                at: Timestamp(111),
            },
        ),
        (
            "Superseded",
            "by",
            EngineJournalRecord::Superseded {
                transformation: tid(1),
                by: tid(9),
                at: Timestamp(111),
            },
        ),
        (
            "Superseded",
            "at",
            EngineJournalRecord::Superseded {
                transformation: tid(1),
                by: tid(2),
                at: Timestamp(999),
            },
        ),
    ]
}

/// The repository root, from this crate's manifest directory.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
}

/// Read a file under the repository root.
pub fn read_repo(rel: &str) -> String {
    let p = repo_root().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// An empty directory to put a journal file in, under the cargo target directory.
///
/// `CARGO_TARGET_TMPDIR` rather than `std::env::temp_dir`, for the reason written out in
/// `gx-log/tests/support/mod.rs`: on this project's WSL2 setup `/tmp` is cleared while the machine
/// sits idle, and a suite whose fixtures evaporate between runs reports housekeeping as a
/// durability failure. Cleared on entry, not on exit -- a test that fails leaves its journal behind
/// to be read.
pub fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

// ---------------------------------------------------------------------------
// M5 hand 2: the pipeline's collaborators
// ---------------------------------------------------------------------------

/// A `SubstrateAdapter` that answers deterministically and touches nothing.
///
/// 🔴 **Not `gx-adapter-fs`.** N-13 keeps every adapter out of this crate's manifest, dev-dependency
/// included (`ENGINE_ADAPTER_DECLARATIONS=0`, hand 1's measure), and the reason is not only the
/// rule: an engine test that failed because a real filesystem behaved unexpectedly would be
/// measuring the adapter. 41 §4 is a trait, this is an implementation of it, and that is the whole
/// of what T-2 and T-3 need.
///
/// Every answer is a function of its arguments, which is what AC-030 requires of everything under
/// `plan` -- a `TransformationId` cannot be a function of the intent if the adapter's contribution
/// to it is not.
#[derive(Clone, Debug)]
pub struct StubAdapter {
    kind: SubstrateKind,
    invertible: bool,
}

impl Default for StubAdapter {
    fn default() -> Self {
        Self {
            kind: SubstrateKind::Fs,
            invertible: true,
        }
    }
}

impl StubAdapter {
    /// An adapter whose `invert` answers `Ok(None)`.
    ///
    /// **E-M3-4** makes that the one condition producing an `Escalate` in v0.1, so this is how
    /// AC-032's third case is reached without inventing a policy for it.
    pub fn without_inverse() -> Self {
        Self {
            kind: SubstrateKind::Fs,
            invertible: false,
        }
    }
}

impl gx_substrate::SubstrateAdapter for StubAdapter {
    fn kind(&self) -> SubstrateKind {
        self.kind.clone()
    }

    fn snapshot(&self, locator: &str) -> gx_substrate::Result<gx_core::ObjectSnapshot> {
        Ok(gx_core::ObjectSnapshot::new(
            gx_core::ObjectId(digest_of(locator.as_bytes())),
            self.kind.clone(),
            locator.to_string(),
            digest_of(locator.as_bytes()),
            gx_core::ReprKind::Bytes,
        ))
    }

    fn plan(
        &self,
        intent: &gx_core::Intent,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<gx_substrate::PlannedDelta> {
        gx_substrate::PlannedDelta::new(self.kind.clone(), intent.goal().0.clone())
    }

    fn precondition(&self, snap: &gx_core::ObjectSnapshot) -> gx_substrate::Result<Fingerprint> {
        Ok(Fingerprint::new(
            self.kind.clone(),
            snap.locator().to_string(),
            *snap.digest(),
        )?)
    }

    fn apply(
        &self,
        _delta: &gx_substrate::PlannedDelta,
    ) -> gx_substrate::Result<gx_substrate::AppliedDelta> {
        // Hand 2 reaches T-8 and stops. `apply` is T-11's and hand 4's, and an adapter that
        // answered here would let a probe pass that should have failed for calling it.
        Err(gx_substrate::Error::Unimplemented {
            method: "apply".to_string(),
            detail: "M5 hand 2 stops at canonicalize; apply is hand 4's".to_string(),
        })
    }

    fn invert(
        &self,
        delta: &gx_substrate::PlannedDelta,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<gx_substrate::InvertOutcome> {
        // 🔴 **DR-46-26**, amended by **DEFECT-892-1** (`req/895` §1). This used to call
        // `InvertOutcome::from_option` "on purpose: this stub stands in for the read-free
        // adapters". There are no read-free adapters — fs, git, mysql and postgres all read their
        // substrate, and `from_option` was deleted along with the sentence that said otherwise.
        //
        // The stub keeps an **empty** read-set, and that is now a statement it makes in its own
        // source rather than one it inherits: this fixture reads nothing because it holds the
        // answer in `self.invertible` and never touches a substrate at all.
        Ok(match self.invertible.then(|| delta.clone()) {
            Some(inverse) => gx_substrate::InvertOutcome::inverted(inverse, Vec::new()),
            None => gx_substrate::InvertOutcome::none(Vec::new()),
        })
    }

    fn commutation(
        &self,
        _a: &gx_substrate::PlannedDelta,
        _b: &gx_substrate::PlannedDelta,
    ) -> gx_substrate::Result<gx_core::Commutation> {
        Ok(gx_core::Commutation::Commutes)
    }
}

/// A `SubstrateAdapter` that counts every call and delegates to [`StubAdapter`].
///
/// 🔴 The **behavioural** half of **E-M5-2**'s "it does not call an adapter (with a machine check)" (sem: SEM-gx-engine-934). The structural
/// half is `tests/store_shape.rs`, which reads `replay.rs` and finds no adapter named and no adapter
/// parameter; this one runs a scenario, takes the totals, replays, and takes them again. Two
/// instruments because they fail for different reasons: a scan cannot see a call made through a
/// collaborator, and a counter cannot see a road nobody walked in this test.
///
/// The counters are per method rather than one total, because "replay read the substrate" and
/// "replay wrote to the substrate" (sem: SEM-gx-engine-935) are different accidents and a single number would report them
/// as the same one.
#[derive(Clone, Debug, Default)]
pub struct CountingAdapter {
    inner: StubAdapter,
    counts: Arc<Counts>,
}

/// One counter per method of 41 §4's trait.
#[derive(Debug, Default)]
pub struct Counts {
    pub kind: AtomicUsize,
    pub snapshot: AtomicUsize,
    pub plan: AtomicUsize,
    pub precondition: AtomicUsize,
    pub apply: AtomicUsize,
    pub invert: AtomicUsize,
    pub commutation: AtomicUsize,
}

impl Counts {
    /// Every counter, in trait order — the shape a probe compares before and after.
    pub fn totals(&self) -> [usize; 7] {
        [
            self.kind.load(Ordering::SeqCst),
            self.snapshot.load(Ordering::SeqCst),
            self.plan.load(Ordering::SeqCst),
            self.precondition.load(Ordering::SeqCst),
            self.apply.load(Ordering::SeqCst),
            self.invert.load(Ordering::SeqCst),
            self.commutation.load(Ordering::SeqCst),
        ]
    }
}

impl CountingAdapter {
    /// A counting adapter and the counters a probe reads.
    pub fn new() -> (Self, Arc<Counts>) {
        let counts = Arc::new(Counts::default());
        (
            Self {
                inner: StubAdapter::default(),
                counts: Arc::clone(&counts),
            },
            counts,
        )
    }
}

impl gx_substrate::SubstrateAdapter for CountingAdapter {
    fn kind(&self) -> SubstrateKind {
        self.counts.kind.fetch_add(1, Ordering::SeqCst);
        self.inner.kind()
    }

    fn snapshot(&self, locator: &str) -> gx_substrate::Result<gx_core::ObjectSnapshot> {
        self.counts.snapshot.fetch_add(1, Ordering::SeqCst);
        self.inner.snapshot(locator)
    }

    fn plan(
        &self,
        intent: &gx_core::Intent,
        pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<gx_substrate::PlannedDelta> {
        self.counts.plan.fetch_add(1, Ordering::SeqCst);
        self.inner.plan(intent, pre)
    }

    fn precondition(&self, snap: &gx_core::ObjectSnapshot) -> gx_substrate::Result<Fingerprint> {
        self.counts.precondition.fetch_add(1, Ordering::SeqCst);
        self.inner.precondition(snap)
    }

    fn apply(
        &self,
        delta: &gx_substrate::PlannedDelta,
    ) -> gx_substrate::Result<gx_substrate::AppliedDelta> {
        self.counts.apply.fetch_add(1, Ordering::SeqCst);
        self.inner.apply(delta)
    }

    fn invert(
        &self,
        delta: &gx_substrate::PlannedDelta,
        pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<gx_substrate::InvertOutcome> {
        self.counts.invert.fetch_add(1, Ordering::SeqCst);
        self.inner.invert(delta, pre)
    }

    fn commutation(
        &self,
        a: &gx_substrate::PlannedDelta,
        b: &gx_substrate::PlannedDelta,
    ) -> gx_substrate::Result<gx_core::Commutation> {
        self.counts.commutation.fetch_add(1, Ordering::SeqCst);
        self.inner.commutation(a, b)
    }
}

// ---------------------------------------------------------------------------
// M5 hand 4: a substrate that can be changed, and can refuse to change
// ---------------------------------------------------------------------------

/// A `SubstrateAdapter` with a **world that moves** — hand 4's fixture.
///
/// [`StubAdapter`] answers as a function of its arguments, which is what AC-030 needs and what
/// AC-034 cannot use: "inject a situation in which a separate process writes to the target file and
/// `Fingerprint₁ != Fingerprint₀`" (sem: SEM-gx-engine-936) requires a substrate whose state can change between `plan` and `commit`. So this one
/// holds its world in an `Arc<Mutex<Vec<u8>>>` and every snapshot digests what is in it.
///
/// 🔴 **Still not `gx-adapter-fs`** (N-13, `ENGINE_ADAPTER_DECLARATIONS=0`). The injection AC-034
/// asks for is "a separate process" (sem: SEM-gx-engine-937) against a real file; against an in-memory substrate the faithful form
/// is a **separate writer** the engine does not know about, which `tests/ac_034.rs` spawns as a
/// thread. What the criterion is measuring is that the CAS sees a change it did not make, and a
/// thread that mutates the world through a handle the engine never reads is exactly that. The
/// difference is recorded in the report rather than papered over.
///
/// # What can be made to fail, and why it is a script rather than a flag
///
/// AC-038 needs `apply` to fail **and then** the rollback's apply to succeed, and a second case
/// where both fail. One boolean cannot say that, so the refusals are a queue: each call to `apply`
/// pops the next answer, and an empty queue means success. `Rollback::Succeeded` and
/// `Rollback::Failed` are then two fixtures rather than two adapters.
#[derive(Clone, Debug)]
pub struct CommitAdapter {
    kind: SubstrateKind,
    world: Arc<Mutex<Vec<u8>>>,
    refusals: Arc<Mutex<VecDeque<bool>>>,
    /// 🔴 **R30 / `req/372` M-01** — the applications that **do the work and then refuse**.
    ///
    /// [`CommitAdapter::refusing`] returns before it touches the world, so a probe built on it
    /// drives a forward apply that changed nothing — and since R30 the engine tells that case
    /// apart from every other and declines to send a compensation for an effect that does not
    /// exist. The criterion AC-038 is about (the escrowed inverse **is** applied) needs the other
    /// shape, and this is it: the commonest real failure, where the call worked and the answer was
    /// lost coming back.
    fails_after_the_effect: Arc<Mutex<VecDeque<bool>>>,
    counts: Arc<Counts>,
    invertible: bool,
    /// 43 §8's other answer. See [`CommitAdapter::conflicting`].
    conflicting: bool,
    /// Which component of the fingerprint this adapter contradicts itself about, after the first
    /// answer. See [`CommitAdapter::miswired`] and [`CommitAdapter::rescoping`]. **M5 hand 7**.
    drift: Option<Drift>,
    /// How many times `precondition` has been asked, which is how [`Drift`] knows "after the
    /// first" (sem: SEM-gx-engine-938). Its own counter rather than [`Counts::precondition`]: a probe reads that one, and a
    /// fixture whose behaviour depended on a number a probe may also reset would be two things.
    preconditions: Arc<AtomicUsize>,
}

/// What a mis-wired adapter contradicts itself about (**M5 hand 7**, 43 T-13).
///
/// `Fingerprint::cas_eq` has three answers and two of them are refusals (**E-M4-27**): a `substrate`
/// mismatch and, when the substrates agree, a `scope` mismatch. Both are "an implementation error" (sem: SEM-gx-engine-939) rather than
/// facts about the world, and **M5-24 adopted (a)** folds both to `Aborted(InternalError)` — 43 T-13. The
/// two arms are separate variants because the refusals are answered in order and a fixture that
/// only ever produced the first would leave the second unwalked.
#[derive(Clone, Copy, Debug)]
pub enum Drift {
    /// The registry filed this adapter under one substrate and it reports another — one deployment
    /// wiring two adapters into one slot, which is the accident 43 T-13 calls "a bug-class failure" (sem: SEM-gx-engine-940).
    Substrate,
    /// The substrate agrees and the scope does not: the same adapter naming "the surrounding state
    /// that can interfere with the target" (sem: SEM-gx-engine-940) two different ways between `plan` and `commit`, which makes the two fingerprints
    /// incomparable rather than unequal (42 §3.5).
    Scope,
}

impl CommitAdapter {
    /// An adapter over `world`, and the counters and the world handle a probe reads.
    pub fn new(world: &str) -> (Self, Arc<Counts>, Arc<Mutex<Vec<u8>>>) {
        let world = Arc::new(Mutex::new(world.as_bytes().to_vec()));
        let counts = Arc::new(Counts::default());
        (
            Self {
                kind: SubstrateKind::Fs,
                world: Arc::clone(&world),
                refusals: Arc::new(Mutex::new(VecDeque::new())),
                fails_after_the_effect: Arc::new(Mutex::new(VecDeque::new())),
                counts: Arc::clone(&counts),
                invertible: true,
                conflicting: false,
                drift: None,
                preconditions: Arc::new(AtomicUsize::new(0)),
            },
            counts,
            world,
        )
    }

    /// 🔴 **R30 / `req/372` M-01** — make the next `n` applications **perform the change and then
    /// answer an error**.
    ///
    /// The order is the whole point of the method: [`CommitAdapter::refusing`] returns before the
    /// world is touched and this one returns after it. Since R30 the engine reads the object the
    /// instant an apply fails, so those two produce different outcomes — `NotAttempted` with
    /// `WorldNeverMoved` for the first, and the compensation actually running for the second.
    #[must_use]
    pub fn failing_after_the_effect(self, script: &[bool]) -> Self {
        {
            let mut failures = self
                .fails_after_the_effect
                .lock()
                .expect("the post-effect failure script is not poisoned");
            failures.extend(script.iter().copied());
        }
        self
    }

    /// Make the next `n` applications refuse (43 T-10c's injection).
    #[must_use]
    pub fn refusing(self, script: &[bool]) -> Self {
        {
            let mut refusals = self
                .refusals
                .lock()
                .expect("the refusal script is not poisoned");
            refusals.extend(script.iter().copied());
        }
        self
    }

    /// An adapter whose `invert` answers `None` — which E-M3-4 turns into an `Escalate` at T-3.
    ///
    /// 🔴 Hand 4 wrote "it never reaches a commit in v0.1" (sem: SEM-gx-engine-941) beside this. Hand 6 is where that stops
    /// being true: T-5 lets a person approve the escalation, the transformation walks on to
    /// `Committing`, and 43 T-10b's "an inverse can be constructed (`Some`)" (sem: SEM-gx-engine-941) guard does not open — which is the
    /// path **E-M5-9** exists for. `tests/supersede.rs` is where it is walked.
    #[must_use]
    pub fn without_inverse(mut self) -> Self {
        self.invertible = false;
        self
    }

    /// An adapter whose `commutation` answers `Conflicts` for every pair (43 §8).
    ///
    /// AC-045's second clause -- "confirm that even a Transformation waiting because of `Conflicts`
    /// is still subject to the TTL and does not wait indefinitely" (sem: SEM-gx-engine-942) -- cannot be constructed without one: 43 §8's waiting is entered on
    /// this answer and on no other. Every pair rather than a scripted one, because what the
    /// criterion measures is the **TTL while waiting**, and a script would make the waiting depend
    /// on call order.
    #[must_use]
    pub fn conflicting(mut self) -> Self {
        self.conflicting = true;
        self
    }

    /// 🔴 An adapter registered under one substrate that fingerprints under another (43 T-13).
    ///
    /// The **first** `precondition` answers honestly — T-2 has to record a `Fingerprint₀` for there
    /// to be a comparison at all — and every one after it reports [`SubstrateKind::Git`]. So the CAS
    /// at T-10a compares two fingerprints from two substrates, `cas_eq` answers
    /// `Err(FingerprintSubstrateMismatch)` (**E-M4-27**), and **M5-24, adopted (a)**, folds that to (sem: SEM-gx-engine-943)
    /// `Aborted(InternalError)`.
    ///
    /// The drift is here, in a fixture, and **not** in the engine: "constructing it inside the engine
    /// would be a reproduction of a wiring bug" (sem: SEM-gx-engine-944) — a shipping line that could build two disagreeing fingerprints would be the bug T-13
    /// receives, so the injection is made where hand 5 made its recovery shim, inside the test.
    #[must_use]
    pub fn miswired(mut self) -> Self {
        self.drift = Some(Drift::Substrate);
        self
    }

    /// The same accident one clause down: the substrates agree and the scopes do not.
    ///
    /// 42 §3.5 calls a scope mismatch a comparison that "has no meaning" (sem: SEM-gx-engine-945), and `cas_eq` answers
    /// `Err(FingerprintScopeMismatch)` only when the substrates already agree — so this arm is
    /// unreachable while [`CommitAdapter::miswired`] is in force, and the two are separate fixtures
    /// for that reason.
    #[must_use]
    pub fn rescoping(mut self) -> Self {
        self.drift = Some(Drift::Scope);
        self
    }
}

impl gx_substrate::SubstrateAdapter for CommitAdapter {
    fn kind(&self) -> SubstrateKind {
        self.counts.kind.fetch_add(1, Ordering::SeqCst);
        self.kind.clone()
    }

    fn snapshot(&self, locator: &str) -> gx_substrate::Result<gx_core::ObjectSnapshot> {
        self.counts.snapshot.fetch_add(1, Ordering::SeqCst);
        let world = self
            .world
            .lock()
            .expect("the world is not poisoned")
            .clone();
        Ok(gx_core::ObjectSnapshot::new(
            ObjectId(digest_of(locator.as_bytes())),
            self.kind.clone(),
            locator.to_string(),
            digest_of(&world),
            gx_core::ReprKind::Bytes,
        ))
    }

    fn plan(
        &self,
        intent: &gx_core::Intent,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<gx_substrate::PlannedDelta> {
        self.counts.plan.fetch_add(1, Ordering::SeqCst);
        gx_substrate::PlannedDelta::new(self.kind.clone(), intent.goal().0.clone())
    }

    fn precondition(&self, snap: &gx_core::ObjectSnapshot) -> gx_substrate::Result<Fingerprint> {
        self.counts.precondition.fetch_add(1, Ordering::SeqCst);
        // 43 T-13's injection (M5 hand 7). The first answer is honest so that T-2 has a
        // `Fingerprint₀` to record; the drift begins at the second, which is the CAS at T-10a.
        let nth = self.preconditions.fetch_add(1, Ordering::SeqCst);
        let (substrate, scope) = match (self.drift, nth) {
            (Some(Drift::Substrate), 1..) => (SubstrateKind::Git, snap.locator().to_string()),
            (Some(Drift::Scope), 1..) => (
                self.kind.clone(),
                format!("{}#a-second-name-for-the-same-place", snap.locator()),
            ),
            _ => (self.kind.clone(), snap.locator().to_string()),
        };
        Ok(Fingerprint::new(substrate, scope, *snap.digest())?)
    }

    fn apply(
        &self,
        delta: &gx_substrate::PlannedDelta,
    ) -> gx_substrate::Result<gx_substrate::AppliedDelta> {
        self.counts.apply.fetch_add(1, Ordering::SeqCst);
        let refuse = self
            .refusals
            .lock()
            .expect("the refusal script is not poisoned")
            .pop_front()
            .unwrap_or(false);
        if refuse {
            return Err(gx_substrate::Error::ApplyFailed {
                detail: "the injected substrate refused this application (43 T-10c)".to_string(),
            });
        }
        let fail_after_the_effect = self
            .fails_after_the_effect
            .lock()
            .expect("the post-effect failure script is not poisoned")
            .pop_front()
            .unwrap_or(false);
        let mut world = self.world.lock().expect("the world is not poisoned");
        world.clone_from(&delta.payload().to_vec());
        let digest = digest_of(&world);
        // 🔴 **R30 / `req/372` M-01** — the change is made and the answer is an error anyway.
        // `gx-substrate`'s own `ApplyFailed` doc permits exactly this ("a non-atomic `apply` can
        // fail halfway"), and it is what a lost response looks like from the engine's side.
        if fail_after_the_effect {
            drop(world);
            return Err(gx_substrate::Error::ApplyFailed {
                detail: "the injected substrate performed this application and then refused it                          (43 T-10c, req/372 M-01)"
                    .to_string(),
            });
        }
        Ok(gx_substrate::AppliedDelta::new(
            delta.reference().clone(),
            Fingerprint::new(self.kind.clone(), "/tmp/world".to_string(), digest)?,
            digest,
            // 🔴 E-M4-31: an adapter has no clock (41 §6 injects it at the engine boundary), so it
            // answers zero and the engine overwrites. gx-adapter-fs does exactly this
            // (`apply.rs:104`), and a fixture that invented a timestamp would let the engine's
            // overwrite pass unmeasured.
            Timestamp(0),
        ))
    }

    fn invert(
        &self,
        _delta: &gx_substrate::PlannedDelta,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<gx_substrate::InvertOutcome> {
        self.counts.invert.fetch_add(1, Ordering::SeqCst);
        if !self.invertible {
            return Ok(gx_substrate::InvertOutcome::none(Vec::new()));
        }
        // The inverse is "put the world back as it is now" (sem: SEM-gx-engine-946), which is what makes the rollback of
        // 43 T-10c a real undo rather than a token: applying it after a failed forward apply
        // restores the bytes the snapshot was taken over.
        let world = self
            .world
            .lock()
            .expect("the world is not poisoned")
            .clone();
        Ok(gx_substrate::InvertOutcome::inverted(
            gx_substrate::PlannedDelta::new(self.kind.clone(), world)?,
            Vec::new(),
        ))
    }

    fn commutation(
        &self,
        a: &gx_substrate::PlannedDelta,
        _b: &gx_substrate::PlannedDelta,
    ) -> gx_substrate::Result<gx_core::Commutation> {
        self.counts.commutation.fetch_add(1, Ordering::SeqCst);
        if self.conflicting {
            // 42 §3.4's `residual` is "the part of `b` that does not commute past `a`" (sem: SEM-gx-engine-947); a fixture
            // that has no residual to name reports `a`'s own reference, which is enough for 43 §8
            // (the engine reads the discriminant and not the payload -- M4H6-4).
            return Ok(gx_core::Commutation::Conflicts {
                residual: a.reference().clone(),
            });
        }
        Ok(gx_core::Commutation::Commutes)
    }
}

// ---------------------------------------------------------------------------
// M5 hand 5: driving the crash probe (51 §8.1)
// ---------------------------------------------------------------------------

/// Run `crash_probe` to completion and return its stdout, refusing a non-zero exit.
///
/// The binary is located through `CARGO_BIN_EXE_crash_probe`, which cargo sets for an integration
/// test of the package that declares the `[[bin]]` — the same road `tests/ac_030.rs` takes to
/// `engine_id_probe`, so the two E2Es agree on what "a separate process" (sem: SEM-gx-engine-948) costs to arrange.
pub fn probe(args: &[&str]) -> String {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_crash_probe"))
        .args(args)
        .output()
        .expect("the probe binary is built before the integration tests");
    assert!(
        out.status.success(),
        "crash_probe {args:?} exited {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Start `crash_probe run` armed for `point`, wait for it to reach the point, and **kill it**.
///
/// `Child::kill` is `SIGKILL` on Unix, which is 51 §8.1's "send `kill -9` to a real process" (sem: SEM-gx-engine-949) with no
/// signal number written down anywhere. The wait is on the marker file rather than on a duration:
/// the child writes it after everything the injection point promises has been fsynced, so the kill
/// lands in the same place on every one of AC-043's ten trials instead of in a race (51 §13-4).
///
/// # Panics
/// If the child never reaches the point within the timeout — a run that was going to be flaky is a
/// failure here rather than a silent pass with the process still alive.
pub fn kill_at(dir: &Path, point: &str, goal: &str) -> String {
    let ready = dir.join("ready");
    let _ = fs::remove_file(&ready);
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_crash_probe"))
        .args(["run", &dir.display().to_string(), point, goal])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("the probe binary is built before the integration tests");

    // 🔴 The readiness signal is the marker's **content**, not the file's existence.
    //
    // `File::create` publishes the path before anything is written into it, so a parent that polled
    // `ready.exists()` and then read could observe an **empty** file — and the assertion at the call
    // site would compare `""` against `"t9"` and report that the child stopped somewhere else. It
    // did not: it had not finished saying where.
    //
    // Found by M6 hand 6, under `cargo nextest`, and it is a race rather than a flake: this hand
    // added seven suites, two of which spawn a `gx serve` process and four of which run
    // multi-threaded tokio runtimes, so the window between `create` and `write` finally got
    // scheduled through. Retrying the suite would have hidden it (req/88 §8.2: "if it still fails,
    // that is not flaky, it is a real bug", sem: SEM-gx-engine-950) — and its converse, that a failure which retries away is still a defect
    // somewhere). Raised as **M6H6-14**.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let marker = loop {
        if let Ok(marker) = fs::read_to_string(&ready) {
            if !marker.is_empty() {
                break marker;
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "crash_probe never reached {point:?} in {}",
            dir.display()
        );
        assert!(
            child.try_wait().expect("the child can be polled").is_none(),
            "crash_probe exited before reaching {point:?}; the injection point is not where the \
             test thinks it is"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    child.kill().expect("SIGKILL reaches the child");
    child.wait().expect("the killed child is reaped");
    marker
}

/// The value of a `KEY=VALUE` line the probe printed, if it printed one.
pub fn value(stdout: &str, key: &str) -> Option<String> {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")))
        .map(str::to_string)
}

/// The value of a `KEY=VALUE` line, or a panic naming what was missing.
pub fn need(stdout: &str, key: &str) -> String {
    value(stdout, key).unwrap_or_else(|| panic!("the probe printed no {key}= line:\n{stdout}"))
}

/// The frame boundaries of a journal file: the offset each length-prefixed record starts at.
///
/// `crate::replay` writes `[u32 big-endian length][payload]`, and this walks that framing so a test
/// can truncate the file **exactly one record short**. That is how the one window no adapter seam
/// can reach is built: a crash after `ledger.append` and before the `Committed` record (43 §7-3b).
///
/// 🔴 **R5 / DR-43-9** — a journal written by this version begins with `JOURNAL_MAGIC` and stores
/// a 32-byte chain link after every payload, so the walk skips the marker and steps over the link.
/// The marker's own offset is **not** a boundary: truncating there would leave a file with no
/// framing at all, which is a different experiment from "one record short". A journal without the
/// marker is walked in the old framing, because the fixtures that build one by hand are testing
/// exactly that.
pub fn record_boundaries(bytes: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    // 🔴 **R30 / `req/372` M-02** — both chained framings, because this helper walks the file's
    // shape and the shape is identical: what the marker's version distinguishes is the record
    // vocabulary, not the framing. Sniffing only `GXJRNL01` made every journal this build writes
    // look legacy here, and the offsets that follow were then wrong by the marker's eight bytes
    // plus a link per record.
    let chained = bytes.starts_with(gx_engine::JOURNAL_MAGIC)
        || bytes.starts_with(gx_engine::replay::JOURNAL_MAGIC_V2);
    let link = if chained { 32 } else { 0 };
    let mut at = if chained {
        gx_engine::JOURNAL_MAGIC.len()
    } else {
        0
    };
    while at + 4 <= bytes.len() {
        out.push(at);
        let mut header = [0u8; 4];
        header.copy_from_slice(&bytes[at..at + 4]);
        let length = u32::from_be_bytes(header) as usize;
        if length == 0 || at + 4 + length + link > bytes.len() {
            break;
        }
        at += 4 + length + link;
    }
    out
}

/// Copy a directory tree (one level deep plus one subdirectory level, which is all the engine
/// makes: the journal, the blob directory, the ledger and the world).
///
/// Used to run two recoveries against **the same crashed bytes** — the Λ4 control experiment.
pub fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the copy's directory");
    for entry in fs::read_dir(from).expect("the source directory is readable") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a file is copied");
        }
    }
}

/// The signing key T-11's receipt is issued under (42 §3.10's `key_id`).
///
/// From a fixed seed rather than generated: a receipt whose key changed between runs would make
/// `tests/ac_038.rs` and `tests/commit_protocol.rs` compare values that differ for a reason that is
/// not the one under test.
pub fn signing_key() -> gx_witness::KeyPair {
    gx_witness::KeyPair::from_seed("key-engine-1", &[7u8; 32])
}

/// A distinguishable digest of some bytes. Deterministic across processes, which is all AC-030 asks.
pub fn digest_of(bytes: &[u8]) -> Cid {
    let mut raw = [0u8; 32];
    for (i, b) in bytes.iter().enumerate() {
        raw[i % 32] ^= *b;
    }
    Cid(raw)
}

/// An intent for `locator`, with everything else held fixed.
///
/// Held fixed on purpose: `IntentId` is the CID of all five fields (42 §1.3 row 2, "no exclusion
/// rule", sem: SEM-gx-engine-951), so a fixture that varied one silently would make AC-030's "the same intent" mean something weaker
/// than it says.
pub fn intent(locator: &str, goal: &str) -> gx_core::Intent {
    gx_core::Intent::new(
        SubstrateKind::Fs,
        locator.to_string(),
        gx_core::GoalBytes(goal.as_bytes().to_vec()),
        gx_core::ChangeContext::Policy,
        gx_core::Actor::Agent {
            key: "key-agent-1".to_string(),
            model: "claude-fable-5".to_string(),
        },
    )
}

/// A policy set that admits everything, so that a verdict is decided by the invariants and by
/// `invert_available` rather than by Cedar.
pub const PERMIT_ALL: &str = r#"@id("permit-everything")
permit (principal, action, resource);
"#;

/// The same, with a clause that refuses anything under `/etc` — AC-032's `Deny` case.
pub const FORBID_ETC: &str = r#"@id("permit-everything")
permit (principal, action, resource);

@id("forbid-etc")
forbid (principal, action, resource)
when { resource.locator like "/etc/*" };
"#;

/// A gate that decides with `policies` and no invariants.
pub fn gate(policies: &str) -> gx_gate::Gate {
    gx_gate::Gate::with_policies(
        gx_gate::PolicyEngine::parse(policies).expect("the fixture policy set parses"),
    )
}

/// Either of hand 2's two evidence sources, chosen at run time.
///
/// `Engine` is generic over its [`gx_engine::EvidenceSource`], so a table-driven suite that wants
/// one reachable row and one unreachable row cannot put both engines in one `Vec`. This is the
/// smallest thing that fixes that without a `Box<dyn>`: the trait has no `dyn`-safe blanket impl on
/// `Box`, and adding one to the crate for a test's convenience would be shipping API for a fixture.
///
/// AC-037's "every mode x every verdict combination" (sem: SEM-gx-engine-952) is what needs it — the degraded admission of 43 T-4e is one
/// of the rows, and it is reached by a collector that cannot be reached.
pub enum MaybeEvidence {
    Reachable(gx_engine::InjectedEvidence),
    Unreachable(gx_engine::UnreachableEvidence),
}

impl MaybeEvidence {
    /// A source that answers with nothing, successfully (44 §1.2's "when omitted", sem: SEM-gx-engine-953).
    pub fn none() -> Self {
        MaybeEvidence::Reachable(gx_engine::InjectedEvidence::none())
    }

    /// A source that cannot be reached — 43 T-4d/T-4e's precondition (**E-M5-4**).
    pub fn down() -> Self {
        MaybeEvidence::Unreachable(gx_engine::UnreachableEvidence::new(
            "the collector cannot be reached",
        ))
    }
}

impl gx_engine::EvidenceSource for MaybeEvidence {
    fn collect(
        &self,
        t: &gx_core::Transformation,
        pre: &gx_core::ObjectSnapshot,
    ) -> gx_engine::Result<Vec<gx_witness::Evidence>> {
        match self {
            MaybeEvidence::Reachable(inner) => inner.collect(t, pre),
            MaybeEvidence::Unreachable(inner) => inner.collect(t, pre),
        }
    }
}

/// An invariant that refuses **one exact planned payload** (41 §4, 42 §3.8).
///
/// 🔴 AC-040's second case needs a gate that admits a change and denies its undo:
/// "in a separate case where the invariant/policy corresponding to `T_u` is deliberately set to Deny,
/// confirm that `T_u` does not reach `Committed` and stays `Denied`, and that undo is not exempted
/// from gating either" (sem: SEM-gx-engine-954). Cedar cannot express it. ASM-60-1's
/// request carries a locator, a substrate, an order and `invert_available`, and an undo agrees with
/// the change it undoes on **all four** — same object, same substrate, order 0, invertible. The one
/// thing that differs is the delta itself, and 41 §4 hands that to an [`gx_gate::InvariantCheck`]
/// as `GateInput.planned`.
///
/// So the discriminator is the payload, which is honest for this fixture: the forward delta writes
/// "after" and the escrowed inverse writes "before" (sem: SEM-gx-engine-955), and a deployment refusing "put it back" is
/// exactly the policy AC-040 asks to be exercised. P-6 keeps the engine from reading those bytes;
/// an invariant a deployment registered is the layer that may.
pub struct DenyPayload {
    id: String,
    payload: Vec<u8>,
}

impl DenyPayload {
    /// An invariant under `id` that refuses a planned delta whose payload is exactly `payload`.
    pub fn new(id: &str, payload: &str) -> Self {
        Self {
            id: id.to_string(),
            payload: payload.as_bytes().to_vec(),
        }
    }
}

impl gx_gate::InvariantCheck for DenyPayload {
    fn id(&self) -> &str {
        &self.id
    }

    fn check(&self, input: &gx_gate::GateInput<'_>) -> gx_gate::Result<gx_gate::InvariantResult> {
        let holds = input.planned.0 != self.payload;
        gx_gate::InvariantResult::new(
            self.id.clone(),
            holds,
            Some(if holds {
                "the planned payload is not the refused one".to_string()
            } else {
                "this deployment refuses that exact change".to_string()
            }),
        )
    }
}

/// A gate that admits by policy and refuses one planned payload by invariant.
pub fn gate_refusing(policies: &str, id: &str, payload: &str) -> gx_gate::Gate {
    let mut registry = gx_gate::InvariantRegistry::new();
    registry
        .register(Box::new(DenyPayload::new(id, payload)))
        .expect("one invariant, one id");
    gate(policies).with_invariants(registry)
}

/// A `Canonicalizer` whose bytes gx-canon would not have written — AC-033's abnormal case.
///
/// The bytes are a DAG-CBOR map whose two keys are **out of canonical order** (42 §2.1-2 sorts map
/// keys bytewise), which `gx_canon::cbor::is_canonical` refuses. That is a real non-fixed-point and
/// not a sentinel: an encoder with this bug would produce a different byte string on a second pass,
/// which is exactly `canon(canon(x)) != canon(x)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct BrokenCanon;

impl gx_engine::Canonicalizer for BrokenCanon {
    fn canonical_form(&self, _t: &gx_core::Transformation) -> gx_engine::Result<Vec<u8>> {
        // {"b": 1, "a": 1} -- two one-byte keys, in the wrong order.
        Ok(vec![0xa2, 0x61, 0x62, 0x01, 0x61, 0x61, 0x01])
    }
}
