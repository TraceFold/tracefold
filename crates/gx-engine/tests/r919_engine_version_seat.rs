// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **A2 (`req/910` A., `req/38` SS830, `req/919` W8, 2026-08-30)** — where a receipt's
//! `engine_version` comes from, measured on the engine.
//!
//! `gx-witness`'s `r919_engine_version_attest.rs` asserts the seat exists and decodes. This suite
//! asserts the thing that actually decides whether the seat is correct: **the rebuild roads read the
//! value out of Σ, not out of the process doing the rebuilding.** The arrangement is
//! `tests/confinement_receipt.rs`'s AC-6 ③ bed, mirrored — deliberately, because the two fields are
//! seated by the same argument (43 §7-3b digests a rebuilt payload against the leaf the ledger
//! already holds, and `gx repair` runs from a shell that is not the process that committed).
//!
//! # 🔴 Why the adversarial arm is a Σ rewrite rather than a second binary
//!
//! `confinement` can be varied between two engines in one test binary because a caller declares it.
//! `engine_version` cannot: it is `env!("CARGO_PKG_VERSION")`, a compile-time constant, so both
//! engines in this process necessarily agree and the naive "different process" bed would pass
//! against a producer that read the live constant. That bed would measure nothing.
//!
//! So the discriminating arm forges the journal instead, and it discriminates completely:
//!
//! * a producer that reads **Σ** rebuilds the forged string, its digest misses the leaf, and
//!   `reissue_receipt` answers `WorldMoved` — the tamper is caught;
//! * a producer that reads **`crate::VERSION`** rebuilds the true string, its digest matches the
//!   leaf, and the re-issue answers `Filed` — **a rewritten journal would have been signed over in
//!   silence.**
//!
//! `Filed` here is therefore not a nicer outcome than `WorldMoved`; it is the defect.

mod support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gx_core::{Fingerprint, SubstrateKind, Timestamp, TransformationId};
use gx_engine::pipeline::Reissued;
use gx_engine::{Engine, EngineJournal, EngineJournalRecord, InjectedEvidence, Lifecycle};
use gx_substrate::{InvertOutcome, PlannedDelta, SubstrateAdapter};
use gx_witness::receipt::ReceiptPayload;

use support::{copy_tree, digest_of, gate, intent, scratch, signing_key, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const SUBJECT: &str = "/srv/world";

// The fixture, byte-identical to `tests/confinement_receipt.rs`'s. That file's header records
// where the arrangement came from (`tests/dr4634_read_set_absence.rs`) and why it is per-suite
// (N-13: adapters do not live in this crate). Copied rather than re-derived because this lane
// first wrote one from memory and the compiler found sixteen ways it was not the contract.
#[derive(Clone, Debug)]
struct QuietAdapter {
    world: Arc<Mutex<Vec<u8>>>,
}

impl QuietAdapter {
    fn new(world: &str) -> Self {
        Self {
            world: Arc::new(Mutex::new(world.as_bytes().to_vec())),
        }
    }

    fn world(&self) -> Vec<u8> {
        self.world.lock().expect("not poisoned").clone()
    }
}

impl SubstrateAdapter for QuietAdapter {
    fn kind(&self) -> SubstrateKind {
        SubstrateKind::Fs
    }

    fn snapshot(&self, locator: &str) -> gx_substrate::Result<gx_core::ObjectSnapshot> {
        let world = self.world();
        Ok(gx_core::ObjectSnapshot::new(
            gx_core::ObjectId(digest_of(locator.as_bytes())),
            SubstrateKind::Fs,
            locator.to_string(),
            digest_of(&world),
            gx_core::ReprKind::Bytes,
        ))
    }

    fn plan(
        &self,
        intent: &gx_core::Intent,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<PlannedDelta> {
        PlannedDelta::new(SubstrateKind::Fs, intent.goal().0.clone())
    }

    fn precondition(&self, snap: &gx_core::ObjectSnapshot) -> gx_substrate::Result<Fingerprint> {
        Ok(Fingerprint::new(
            SubstrateKind::Fs,
            snap.locator().to_string(),
            *snap.digest(),
        )?)
    }

    fn apply(&self, delta: &PlannedDelta) -> gx_substrate::Result<gx_substrate::AppliedDelta> {
        let mut world = self.world.lock().expect("not poisoned");
        world.clone_from(&delta.payload().to_vec());
        let digest = digest_of(&world);
        Ok(gx_substrate::AppliedDelta::new(
            delta.reference().clone(),
            Fingerprint::new(SubstrateKind::Fs, SUBJECT.to_string(), digest)?,
            digest,
            Timestamp(0),
        ))
    }

    fn invert(
        &self,
        _delta: &PlannedDelta,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<InvertOutcome> {
        let prior = PlannedDelta::new(SubstrateKind::Fs, self.world())?;
        Ok(InvertOutcome::inverted(prior, Vec::new()))
    }

    fn commutation(
        &self,
        _a: &PlannedDelta,
        _b: &PlannedDelta,
    ) -> gx_substrate::Result<gx_core::Commutation> {
        Ok(gx_core::Commutation::Commutes)
    }
}

fn engine_over(dir: &Path, adapter: &QuietAdapter) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the engine opens");
    engine.register_adapter(Arc::new(adapter.clone()), "r919-quiet-fixture/1");
    engine
}

fn commit_one(engine: &mut Engine<InjectedEvidence>, goal: &str) -> TransformationId {
    let one = intent(SUBJECT, goal);
    let key = signing_key();
    engine.submit(&one, 7, AT).expect("submit");
    let id = engine.plan(&one, AT).expect("plan");
    assert_eq!(
        engine.verify(&id, AT, &key, None).expect("verify"),
        Lifecycle::Admitted
    );
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    assert_eq!(
        engine.commit(&id, AT, &key).expect("commit"),
        Lifecycle::Committed
    );
    id
}

fn payload_of(engine: &Engine<InjectedEvidence>, id: &TransformationId) -> ReceiptPayload {
    engine
        .receipt(id)
        .expect("T-11 issued a commit receipt")
        .payload()
        .expect("the receipt payload decodes")
}

fn project_with_journal(
    from: &Path,
    name: &str,
    edit: impl Fn(&EngineJournalRecord) -> Option<EngineJournalRecord>,
) -> PathBuf {
    let to = scratch(name);
    copy_tree(from, &to);
    let path = to.join("journal.bin");
    let rewritten: Vec<EngineJournalRecord> = {
        let journal = EngineJournal::open(&path).expect("the copied journal opens");
        journal.records().iter().filter_map(&edit).collect()
    };
    std::fs::remove_file(&path).expect("the old journal is removed");
    let mut journal = EngineJournal::open(&path).expect("a fresh journal is created");
    for record in rewritten {
        journal.append(record).expect("the record appends");
    }
    to
}

// ---------------------------------------------------------------------------
// ① the producer
// ---------------------------------------------------------------------------

/// A commit receipt names the engine that produced it, and it is the same string the journal holds.
///
/// Both halves matter. That the receipt is `Some` is the seat; that it **equals what
/// `derive_provenance` journalled** is what makes the rebuild roads' answer reproducible, and a
/// suite asserting only the first would go green against a producer that minted a second spelling.
#[test]
fn a2_a_commit_receipt_names_the_engine_and_agrees_with_sigma() {
    let dir = scratch("r919_a2_producer");
    let adapter = QuietAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");
    let issued = payload_of(&engine, &id);
    let journalled = engine
        .provenance(&id)
        .expect("M5-25 journalled a provenance for a committed transformation")
        .environment
        .engine_version
        .clone();

    println!(
        "A2_PRODUCER receipt={:?} journal={:?} constant={:?}",
        issued.engine_version,
        journalled,
        gx_engine::VERSION
    );
    assert_eq!(
        issued.engine_version.as_deref(),
        Some(gx_engine::VERSION),
        "every receipt this build issues names this build"
    );
    assert_eq!(
        issued.engine_version, Some(journalled),
        "🔴 the receipt and Σ carry one spelling of one fact. Two spellings would make the rebuild \
         roads' digest comparison fail for a reason the producer invented"
    );
}

// ---------------------------------------------------------------------------
// ② the rebuild road, and the arm that discriminates
// ---------------------------------------------------------------------------

/// The control: an untouched journal re-issues to the leaf the ledger already holds.
#[test]
fn a2_an_untouched_rebuild_reproduces_the_leaf() {
    let dir = scratch("r919_a2_rebuild_source");
    let adapter = QuietAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");
    let issued = payload_of(&engine, &id);
    drop(engine);

    let copy = project_with_journal(&dir, "r919_a2_rebuild", |record| Some(record.clone()));
    let mut repairing = engine_over(&copy, &adapter);
    let outcome = repairing
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue road runs");
    let Reissued::Filed(receipt) = outcome else {
        panic!("the control arm must reproduce the leaf, and answered {outcome:?}");
    };
    let rebuilt = receipt.payload().expect("it decodes");
    println!("A2_REBUILD_CONTROL rebuilt={:?}", rebuilt.engine_version);
    assert_eq!(rebuilt.engine_version, issued.engine_version);
    assert_eq!(
        rebuilt.ledger_digest().expect("it digests"),
        issued.ledger_digest().expect("it digests")
    );
}

/// 🔴 **The adversarial arm — a forged engine version in Σ.**
///
/// The journal's `ProvenanceDerived` record is rewritten to name an engine that never existed, and
/// nothing else about the project is touched. See this file's header for why `WorldMoved` is the
/// pass and `Filed` is the defect: only a producer reading Σ can notice, and a producer reading its
/// own constant would sign over the rewrite without a word.
#[test]
fn a2_a_forged_engine_version_in_sigma_is_not_signed_over_in_silence() {
    let dir = scratch("r919_a2_forge_source");
    let adapter = QuietAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");
    let issued = payload_of(&engine, &id);
    drop(engine);

    const FORGED: &str = "gx-engine 9.9.9-a-build-that-never-existed";
    assert_ne!(FORGED, gx_engine::VERSION);
    let copy = project_with_journal(&dir, "r919_a2_forge", |record| match record {
        EngineJournalRecord::ProvenanceDerived {
            transformation,
            provenance,
            at,
        } => {
            let mut forged = provenance.clone();
            forged.environment.engine_version = FORGED.to_string();
            Some(EngineJournalRecord::ProvenanceDerived {
                transformation: *transformation,
                provenance: forged,
                at: *at,
            })
        }
        other => Some(other.clone()),
    });

    let mut repairing = engine_over(&copy, &adapter);
    let outcome = repairing
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue road runs");
    println!(
        "A2_FORGED_SIGMA outcome={outcome:?} issued={:?}",
        issued.engine_version
    );

    match outcome {
        Reissued::Filed(receipt) => {
            let rebuilt = receipt.payload().expect("it decodes");
            panic!(
                "🔴 the re-issue signed a receipt over a rewritten journal. rebuilt={:?}, issued={:?}. \
                 If `rebuilt` is the true version rather than {FORGED:?}, the seat is reading \
                 `crate::VERSION` instead of Σ and the forgery is invisible to the digest check 43 \
                 §7-3b relies on",
                rebuilt.engine_version, issued.engine_version
            );
        }
        other => {
            // The rewrite moved the payload, so the digest misses the leaf and the road refuses.
            // That refusal is the measurement: Σ is what was read.
            println!("A2_FORGED_SIGMA_REFUSED={other:?}");
        }
    }
}

/// A journal with no `ProvenanceDerived` rebuilds to an absence, not to this process's version.
///
/// Mirrors `confinement_receipt.rs`'s AC-6 ④ for the same reason: a build older than M5-25 wrote no
/// such record, and `Some(crate::VERSION)` there would be a claim about a process nobody observed.
/// The re-issue is expected to refuse (the payload cannot reproduce the leaf), and what this
/// asserts is that it refuses rather than fabricating.
#[test]
fn a2_a_journal_that_predates_the_provenance_record_does_not_fabricate_a_version() {
    let dir = scratch("r919_a2_old_source");
    let adapter = QuietAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");
    drop(engine);

    let copy = project_with_journal(&dir, "r919_a2_old", |record| match record {
        EngineJournalRecord::ProvenanceDerived { .. } => None,
        other => Some(other.clone()),
    });
    let mut repairing = engine_over(&copy, &adapter);
    let outcome = repairing
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue road runs");
    println!("A2_NO_PROVENANCE outcome={outcome:?}");
    // 🔴 Asserted unconditionally rather than inside an `if let`. The first draft of this test put
    // the whole claim under `if let Reissued::Filed(..)`, and since the road refuses, the body never
    // ran: a test that passed while measuring nothing, which is the failure this workspace names as
    // a green that lies. The refusal *is* the landing (AC-6 ④'s precedent), so it is what is pinned.
    match outcome {
        Reissued::Filed(receipt) => {
            let rebuilt = receipt.payload().expect("it decodes");
            assert_eq!(
                rebuilt.engine_version, None,
                "🔴 with no provenance in Σ there is no version to state, and the honest form of \
                 no answer is an absence -- not the version of whatever build is doing the \
                 rebuilding"
            );
        }
        other => assert!(
            matches!(other, Reissued::WorldMoved),
            "with no provenance in Σ the rebuilt payload cannot reproduce the leaf, so the road \
             refuses; what must never happen is a signed receipt carrying a fabricated version, \
             and the only two acceptable answers are that refusal or a `Filed` whose payload says \
             `None`. Got {other:?}"
        ),
    }
}
