// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/493` §1 AC-6** — the `confinement` context on receipts the **engine issues**, and on
//! the ones a rebuild re-derives.
//!
//! # What `req/497` §7 left, and why it was not a matter of typing
//!
//! S③ built `gx confine`, measured EACCES against the kernel, and put `{kernel_confined,
//! ruleset_hash}` in the launcher's own report — and then said so:
//!
//! > **But it is not on a real receipt.** … `gx confine` is a launcher: it takes the ruleset and
//! > becomes another program by `exec`. What makes a receipt is `gx commit`, afterwards.
//!
//! Carrying it across that `exec` is one problem; carrying it across a **crash** is the other, and
//! it is the one that decides where the value lives. 43 §7-3b compares a rebuilt payload's digest
//! against the leaf the ledger already holds. A confinement re-read from the environment at rebuild
//! time would therefore answer `payload_mismatch` — the vocabulary's word for tampering — for every
//! recovery of a commit made inside a `gx confine` and repaired outside one. So the value is
//! journalled, in the `ProvenanceDerived` record M5-25 adopted (a) already writes **before the
//! world moves**, and both rebuild roads read it back out of Σ.
//!
//! # The four claims, and the control each one carries
//!
//! 1. a commit made under a declared confinement carries it in the signed bytes — control: the same
//!    commit under the default engine carries `kernel_confined: false` and no hash;
//! 2. the verdict receipt carries it too, because the process is the same process;
//! 3. **the value comes out of Σ and not out of the running engine** — measured by rebuilding a
//!    confined commit in an engine that declares a *different* confinement and asserting the
//!    rebuild is `Filed` (it reproduced the leaf) rather than refused;
//! 4. a journal that predates the erratum rebuilds to `None` rather than to a fabricated `false`.
//!
//! Claim 3 is the one that would be worthless without its control: an implementation that copied
//! the running engine's value into the rebuilt payload would satisfy claims 1, 2 and 4 and would
//! break every crash recovery in the field.
//!
//! N-13 keeps adapters out of this crate, so the fixture implements `gx-substrate`'s contract
//! directly, in the arrangement `tests/dr4634_read_set_absence.rs` uses.

mod support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gx_core::{Fingerprint, SubstrateKind, Timestamp, TransformationId};
use gx_engine::pipeline::Reissued;
use gx_engine::{Engine, EngineJournal, EngineJournalRecord, InjectedEvidence, Lifecycle};
use gx_substrate::{InvertOutcome, PlannedDelta, SubstrateAdapter};
use gx_witness::receipt::{ConfinementContext, ReceiptPayload};

use support::{copy_tree, digest_of, gate, intent, scratch, signing_key, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const SUBJECT: &str = "/srv/world";

/// A ruleset hash of the shape `gx_confine::ConfinePlan::ruleset_hash` mints. Spelled here because
/// `gx-engine` does not name `gx-confine` and must not: the confinement reaches this crate as a
/// value the caller declares, never as a kernel call this crate makes.
fn hash(seed: &str) -> String {
    gx_canon::cid::to_text(&gx_canon::cid::mint(
        gx_canon::cid::Domain::Leaf,
        &[seed.as_bytes()],
    ))
}

fn confined(seed: &str) -> ConfinementContext {
    ConfinementContext {
        kernel_confined: true,
        ruleset_hash: Some(hash(seed)),
    }
}

// ---------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------

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

/// An engine over `dir`, declaring `context` as what the kernel is holding it to.
///
/// `None` means the caller declared nothing, which is the road `gx-api` and every test that
/// predates this erratum take — and which must still produce a `Some` on the receipt, because
/// "nobody confined this process" is a sentence and not a silence.
fn engine_over(
    dir: &Path,
    adapter: &QuietAdapter,
    context: Option<ConfinementContext>,
) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the engine opens");
    if let Some(context) = context {
        engine = engine.with_confinement(context);
    }
    engine.register_adapter(Arc::new(adapter.clone()), "s3ac6-quiet-fixture/1");
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

/// Copy a committed project and rewrite its journal, passing every record through `edit`.
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
// ① the producer, and its negative control
// ---------------------------------------------------------------------------

/// 🔴 **AC-6 ①** — what the kernel held is in the **signed** bytes of the commit receipt.
///
/// The control is the whole test. `gx` produced a receipt before this erratum and it said nothing
/// about confinement; a suite that only asserted the confined case would go green against a
/// producer that hard-coded `kernel_confined: true`. So the same commit is made twice, over the
/// same fixture and the same intent, and the only difference is what the caller declared.
#[test]
fn ac6_a_commit_carries_the_confinement_its_caller_declared() {
    let ruleset = hash("face\tdeclared\nwrite\t/srv/workspace\n");

    let held_dir = scratch("s3ac6_held");
    let adapter = QuietAdapter::new("before");
    let mut held = engine_over(
        &held_dir,
        &adapter,
        Some(confined("face\tdeclared\nwrite\t/srv/workspace\n")),
    );
    let held_id = commit_one(&mut held, "after");
    let held_payload = payload_of(&held, &held_id);

    let loose_dir = scratch("s3ac6_loose");
    let loose_adapter = QuietAdapter::new("before");
    let mut loose = engine_over(&loose_dir, &loose_adapter, None);
    let loose_id = commit_one(&mut loose, "after");
    let loose_payload = payload_of(&loose, &loose_id);

    println!(
        "AC6_PRODUCER held={:?} loose={:?}",
        held_payload.confinement, loose_payload.confinement
    );
    assert_eq!(
        held_payload.confinement,
        Some(ConfinementContext {
            kernel_confined: true,
            ruleset_hash: Some(ruleset),
        }),
        "the receipt names what the kernel was holding and which ruleset it was"
    );
    assert_eq!(
        loose_payload.confinement,
        Some(ConfinementContext::unconfined()),
        "🔴 a process nobody confined says so. `None` here would mean 'these bytes predate the \
         erratum', which is a different sentence and one this binary is never entitled to write"
    );

    // 🔴 In the **signed** bytes, not beside them. `ledger_digest` is what the leaf carries and
    // what `verify_offline` recomputes, so a member outside it would be a claim a stranger could
    // strip. The two digests differ **because** the member differs and for no other reason: the
    // two payloads are otherwise built from the same intent over the same world.
    let restated = ReceiptPayload {
        confinement: loose_payload.confinement.clone(),
        ..held_payload.clone()
    };
    assert_ne!(
        held_payload.ledger_digest().expect("it digests"),
        restated.ledger_digest().expect("it digests"),
        "the confinement is inside what the ledger commits to"
    );
}

/// 🔴 **AC-6 ②** — and the verdict receipt carries it, on the same road and for the same reason.
///
/// The three seats added by DR-46-24 / DR-46-26 are absent on a `VerdictReceipt` because the escrow
/// that answers them runs at 43 T-10b, inside commit. This one is not like them: the question is
/// asked of the **process**, and the process that signs at T-4a is the process that signs at T-11.
/// A kind-dependent rule here would be a rule about nothing.
#[test]
fn ac6_a_verdict_receipt_carries_it_too() {
    let dir = scratch("s3ac6_verdict");
    let adapter = QuietAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter, Some(confined("verdict-bed")));
    let one = intent(SUBJECT, "after");
    let key = signing_key();
    engine.submit(&one, 7, AT).expect("submit");
    let id = engine.plan(&one, AT).expect("plan");
    engine.verify(&id, AT, &key, None).expect("verify");

    let receipts = engine.verdict_receipts(&id);
    assert!(
        !receipts.is_empty(),
        "T-4a issues a verdict receipt for every verdict"
    );
    for receipt in receipts {
        let payload = receipt.payload().expect("it decodes");
        println!("AC6_VERDICT confinement={:?}", payload.confinement);
        assert_eq!(payload.confinement, Some(confined("verdict-bed")));
        // The three seats that *are* kind-dependent, asserted beside it so the difference is
        // measured rather than described.
        assert!(payload.read_set.is_none());
        assert!(payload.reversibility.is_none());
    }
}

// ---------------------------------------------------------------------------
// ③ the rebuild, and the control that makes it a claim
// ---------------------------------------------------------------------------

/// 🔴 **AC-6 ③ — the value comes out of Σ, not out of the process doing the rebuilding.**
///
/// The bed: a commit made under one confinement, re-issued by an engine that declares a **different**
/// one. If the rebuild road read `self.confinement`, the payload it rebuilds would carry the second
/// value, its digest would not be the leaf's, and `reissue_receipt` would answer
/// `Reissued::WorldMoved`. That it answers `Filed` — and that the filed receipt carries the
/// **first** value — is the measurement.
///
/// This is not a hypothetical arrangement. `gx repair` runs after a crash, from a shell, and the
/// shell that repairs is not the `gx confine` that committed. Under a producer that re-read the
/// environment, every such recovery would report the vocabulary's word for tampering about a
/// receipt nobody touched.
#[test]
fn ac6_a_rebuild_reproduces_the_confinement_the_commit_was_made_under() {
    let dir = scratch("s3ac6_rebuild_source");
    let adapter = QuietAdapter::new("before");
    let committed_under = confined("the-ruleset-that-committed");
    let mut engine = engine_over(&dir, &adapter, Some(committed_under.clone()));
    let id = commit_one(&mut engine, "after");
    let issued = payload_of(&engine, &id);
    drop(engine);

    // A different process, declaring a different confinement — the repair shell.
    let copy = project_with_journal(&dir, "s3ac6_rebuild", |record| Some(record.clone()));
    let repairing_under = confined("a-completely-different-ruleset");
    assert_ne!(committed_under, repairing_under);
    let mut repairing = engine_over(&copy, &adapter, Some(repairing_under.clone()));
    let outcome = repairing
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue road runs");

    let Reissued::Filed(receipt) = outcome else {
        panic!(
            "🔴 the rebuild did not reproduce the leaf the ledger holds: {outcome:?}. If this is \
             `WorldMoved`, the confinement is being re-read from the rebuilding process instead of \
             out of Σ, and every crash recovery of a confined commit now reports tampering"
        );
    };
    let rebuilt = receipt.payload().expect("it decodes");
    println!(
        "AC6_REBUILD issued={:?} rebuilt={:?} repairing_engine_declared={:?}",
        issued.confinement, rebuilt.confinement, repairing_under
    );
    assert_eq!(
        rebuilt.confinement,
        Some(committed_under),
        "a rebuild states what the commit was made under"
    );
    assert_ne!(
        rebuilt.confinement,
        Some(repairing_under),
        "and not what the process doing the rebuilding is under"
    );
    assert_eq!(
        rebuilt.ledger_digest().expect("it digests"),
        issued.ledger_digest().expect("it digests")
    );
}

/// 🔴 **AC-6 ④** — a journal written before the erratum rebuilds to an absence, not to a `false`.
///
/// The bed drops `ProvenanceDerived`, which is exactly what a journal from a build older than
/// M5-25 holds and what one from a build older than *this* erratum decodes to after
/// `#[serde(default)]` has run. The rebuild has no answer, and the honest form of no answer is
/// `None` — `Some(unconfined)` would be a claim about a process nobody observed, which is the
/// four-way-`null` defect DR-46-34 spent a lane closing, run in reverse.
///
/// The refusal is the expected landing rather than a disappointment: `req/38` §294 already records
/// that a receipt this product issued in August 2026 no longer decodes against this build, so a
/// rebuild over a journal that predates the seat cannot reproduce the leaf either. What is measured
/// is that it **refuses** instead of signing a fabricated answer.
#[test]
fn ac6_a_journal_that_predates_the_seat_rebuilds_to_an_absence() {
    let dir = scratch("s3ac6_old_journal_source");
    let adapter = QuietAdapter::new("before");
    let mut engine = engine_over(&dir, &adapter, Some(confined("old-journal-bed")));
    let id = commit_one(&mut engine, "after");
    drop(engine);

    let stripped = project_with_journal(&dir, "s3ac6_old_journal", |record| match record {
        EngineJournalRecord::ProvenanceDerived { .. } => None,
        other => Some(other.clone()),
    });
    let mut reopened = engine_over(&stripped, &adapter, Some(confined("old-journal-bed")));
    let outcome = reopened
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue road runs");
    println!("AC6_OLD_JOURNAL reissue={outcome:?}");
    assert!(
        !matches!(outcome, Reissued::Filed(_)),
        "🔴 a road holding no record of what confined the commit must not sign a receipt about it. \
         A `Filed` here means the rebuild fell back to the running process, which is the failure \
         `ac6_a_rebuild_reproduces_the_confinement_the_commit_was_made_under` is the pair of"
    );
}
