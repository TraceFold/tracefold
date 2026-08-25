// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-26**, engine half: the two producers the widened `invert` gives writers to, driven
//! end to end over a real journal.
//!
//! `req/38` §251-6 registered D24 as having built two seats and no writers:
//! `ReceiptPayload::read_set` (filled with `None` at four coordinates in `pipeline.rs`) and
//! `InverseStatus::Undetermined` (a seventh word whose own documentation named the block —
//! "`Reversibility` does not cross the crate boundary"). E-DR4626-1 widened
//! `SubstrateAdapter::invert` to `Result<InvertOutcome>` and this file is what says the widening
//! **functions** rather than merely exists.
//!
//! What is measured here, and each with its negative control (`req/38` §252-2: take the control
//! first):
//!
//! | arm | positive | negative control |
//! |---|---|---|
//! | **AC-S2** the read-set reaches the signed bytes | an adapter that reports one read: `read_set` is `Some`, `granularity()=="G3"`, `distinct_objects()==1`, `names(locator)==Some(true)` | an adapter that reports none: `read_set` names the *absence* (**DR-46-34**; it was `None` when this file was written — see the controls' own comments) |
//! | **AC-V1** the third status word has a writer | `Reversibility::Unknown` → escrow row answers `Undetermined` | `Reversibility::False` → the same receipt shape (`inverse_delta: null`) answers `Unavailable` |
//! | **the seam itself** | the two facts arrive from **one** call and are digested together | the two words are different **in the signed bytes**, which the escrow row alone cannot deliver |
//!
//! N-13 keeps adapters out of this crate, so the fixture implements `gx-substrate`'s contract
//! directly — the same arrangement `tests/two_phase_escrow.rs` uses. The **shape** it stands in for
//! is `gx-adapter-mcp`'s (`invert.rs`'s four-row table), and the shape is all this file needs: what
//! is under test is the engine's side of the seam.

mod support;

use std::sync::{Arc, Mutex};

use gx_core::{Fingerprint, ReadEntry, Reversibility, SubstrateKind, Timestamp};
use gx_engine::{Engine, InjectedEvidence, InverseStatus, Lifecycle};
use gx_substrate::{InvertOutcome, PlannedDelta, SubstrateAdapter};

use support::{digest_of, gate, intent, scratch, signing_key, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const SUBJECT: &str = "/srv/world";

/// What C-25 answer the fixture's `invert` reports, and whether it reports a read behind it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    /// An inverse, and one object read to build it — `gx-adapter-mcp`'s ordinary escrow road.
    InvertedAfterReading,
    /// An inverse, and no read reported — the shape the fs/git/postgres adapters have, where the
    /// inverse comes out of the snapshot already in hand and no transport was asked anything.
    InvertedWithoutReading,
    /// No inverse exists for this call, and the read that established it is still attested.
    NoneAfterReading,
    /// 🔴 The prior would not be read and the deployment declared `OnReadFailure::Unknown`: nobody
    /// found out, and there is no `{digest, locator}` to attest because the read is what failed.
    Undetermined,
}

/// A world-backed adapter that reports whichever of C-25's answers the arm under test needs.
#[derive(Clone, Debug)]
struct SeamAdapter {
    world: Arc<Mutex<Vec<u8>>>,
    answer: Answer,
}

impl SeamAdapter {
    fn new(world: &str, answer: Answer) -> Self {
        Self {
            world: Arc::new(Mutex::new(world.as_bytes().to_vec())),
            answer,
        }
    }

    fn world(&self) -> Vec<u8> {
        self.world.lock().expect("not poisoned").clone()
    }

    /// The entry an escrow that actually read would attest: the object it read, and the digest of
    /// what the read answered. The same pair `gx_adapter_mcp::invert` builds.
    fn entry(&self) -> ReadEntry {
        ReadEntry {
            digest: digest_of(&self.world()),
            locator: SUBJECT.to_string(),
        }
    }
}

impl SubstrateAdapter for SeamAdapter {
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

    /// 🔴 The seam. Every arm goes through one of `InvertOutcome`'s checked constructors, which is
    /// what makes "`True` with no inverse" unrepresentable rather than merely unwritten.
    fn invert(
        &self,
        _delta: &PlannedDelta,
        _pre: &gx_core::ObjectSnapshot,
    ) -> gx_substrate::Result<InvertOutcome> {
        let prior = PlannedDelta::new(SubstrateKind::Fs, self.world())?;
        Ok(match self.answer {
            Answer::InvertedAfterReading => InvertOutcome::inverted(prior, vec![self.entry()]),
            Answer::InvertedWithoutReading => InvertOutcome::inverted(prior, Vec::new()),
            Answer::NoneAfterReading => InvertOutcome::none(vec![self.entry()]),
            Answer::Undetermined => InvertOutcome::undetermined(Vec::new()),
        })
    }

    fn commutation(
        &self,
        _a: &PlannedDelta,
        _b: &PlannedDelta,
    ) -> gx_substrate::Result<gx_core::Commutation> {
        Ok(gx_core::Commutation::Commutes)
    }
}

fn engine_over(dir: &std::path::Path, adapter: &SeamAdapter) -> Engine<InjectedEvidence> {
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the engine opens");
    engine.register_adapter(Arc::new(adapter.clone()), "dr4626-seam-fixture/1");
    engine
}

/// Drive one intent to `Committed` and hand back its id.
///
/// 🔴 The escalation step is not decoration. **E-M3-4** escalates a transformation whose `invert`
/// answers no inverse, so the two arms of AC-V1 -- `Unknown` and `False`, both of which arrive with
/// `inverse.is_none()` -- reach T-4c rather than T-4a, and only a person's T-5 ruling puts them on
/// the commit road. That is the pre-existing rule the seat itself was built around: `req/38` §40
/// records that `InverseEscrowed { inverse_cid: None }` became *reachable* precisely because "T-5
/// is what lets a person approve one". This helper therefore takes whichever door the verdict
/// opened, and asserts that the door was the one the arm's answer implies.
fn commit_one(engine: &mut Engine<InjectedEvidence>, goal: &str) -> gx_core::TransformationId {
    let one = intent(SUBJECT, goal);
    let key = signing_key();
    engine.submit(&one, 7, AT).expect("submit");
    let id = engine.plan(&one, AT).expect("plan");
    match engine.verify(&id, AT, &key, None).expect("verify") {
        Lifecycle::Admitted => {}
        Lifecycle::Escalated => {
            // E-M3-4's road. The ruler is a person with a key of their own (43 T-5's guard).
            let owner_key = gx_witness::KeyPair::from_seed("dr4626-ruler", &[11u8; 32]);
            let ruling = gx_engine::HumanRuling {
                decision: gx_core::VerdictKind::Admit,
                reason: "DR-46-26: the arm under test is a commit with no escrowable inverse"
                    .to_string(),
                actor: support::ruler(1),
            };
            assert_eq!(
                engine
                    .escalation(&id, &ruling, AT, &owner_key)
                    .expect("T-5"),
                Lifecycle::Admitted
            );
        }
        other => panic!("verify answered {other:?}"),
    }
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    assert_eq!(
        engine.commit(&id, AT, &key).expect("commit"),
        Lifecycle::Committed
    );
    id
}

fn payload_of(
    engine: &Engine<InjectedEvidence>,
    id: gx_core::TransformationId,
) -> gx_witness::receipt::ReceiptPayload {
    engine
        .receipt(&id)
        .expect("T-11 issued a commit receipt")
        .payload()
        .expect("the receipt payload decodes")
}

// ---------------------------------------------------------------------------
// AC-S2: the read-set reaches the signed bytes, and its absence is honest
// ---------------------------------------------------------------------------

/// 🔴 **AC-S2 positive**: what the escrow read is in the receipt, at G3, naming the object.
///
/// Every clause is one of `req/452` AC-S2's, asked of the **signed** payload rather than of a
/// value in memory: the receipt is decoded from the issued document, which is the only reading in
/// which "attested" means anything.
#[test]
fn dr4626_the_escrows_read_reaches_the_signed_receipt_at_g3() {
    let dir = scratch("dr4626_read_set_g3");
    let adapter = SeamAdapter::new("before", Answer::InvertedAfterReading);
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");

    let payload = payload_of(&engine, id);
    let read_set = payload
        .read_set
        .as_ref()
        .expect("AC-S2: the escrow read one object and the receipt says so");
    println!(
        "AC_S2_READ_SET granularity={} distinct={} names_subject={:?}",
        read_set.granularity(),
        read_set.distinct_objects(),
        read_set.names(SUBJECT)
    );
    assert_eq!(read_set.granularity(), "G3");
    assert_eq!(read_set.distinct_objects(), 1);
    assert_eq!(
        read_set.names(SUBJECT),
        Some(true),
        "G3 decides from the receipt alone, and the object it decides about is the one gx read"
    );
    assert_eq!(
        read_set.names("/srv/somewhere-else"),
        Some(false),
        "and it decides `false` about an object the escrow did not read, which is the half a         one-sided assertion would miss"
    );

    // The verdict travelled in the same call and is in the same signed bytes.
    println!("AC_S2_REVERSIBILITY={:?}", payload.reversibility);
    assert_eq!(payload.reversibility, Some(Reversibility::True));
    assert!(payload.inverse_delta.is_some());
    payload
        .check_schema()
        .expect("a commit receipt may carry both");
}

/// 🔴 **AC-S2 negative control**: an escrow that read nothing carries no read-set.
///
/// `ReadSet::from_reads` answers for an empty set, so the absence is the constructor's and not a
/// caller's decision — which is the same rule that keeps the *granularity* out of the caller's
/// hands (`req/441` §4). Without this arm the positive above would be satisfied by an engine that
/// stamped a read-set onto every receipt.
///
/// 🔴 **DR-46-34 moved the spelling and not the claim** (`req/38` §268 ruling 5). The constructor
/// answered `Ok(None)` when this control was written, and the assertion below was
/// `read_set.is_none()`. That `None` turned out to be the same `None` a rebuild with no journal
/// record produced, so the fact is now `ReadSet::Nothing` and `None` is reserved for a receipt
/// nobody asked. The control is unweakened: what it refuses is a receipt that *claims a read*, and
/// `Nothing` claims none — it claims, positively, that there was none, which is the stronger
/// statement this arm was always trying to make.
#[test]
fn dr4626_an_escrow_that_read_nothing_carries_no_read_set() {
    let dir = scratch("dr4626_read_set_absent");
    let adapter = SeamAdapter::new("before", Answer::InvertedWithoutReading);
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");

    let payload = payload_of(&engine, id);
    println!(
        "AC_S2_CONTROL read_set={:?} reversibility={:?}",
        payload.read_set, payload.reversibility
    );
    assert_eq!(
        payload.read_set,
        Some(gx_witness::receipt::ReadSet::Nothing),
        "an adapter that reported no reads produced a receipt claiming one (DR-46-34)"
    );
    assert!(
        !payload
            .read_set
            .as_ref()
            .expect("named above")
            .is_attested(),
        "and `Nothing` is not an attested read-set, which is what `is_none()` used to say"
    );
    assert_eq!(
        payload.reversibility,
        Some(Reversibility::True),
        "the verdict is orthogonal to the read-set: this commit is reversible and read nothing"
    );
}

// ---------------------------------------------------------------------------
// AC-V1: `Undetermined` has a writer, and `Unavailable` still means what it meant
// ---------------------------------------------------------------------------

/// 🔴 **AC-V1**: the same receipt shape, two different words — which is the whole of DR-46-13.
///
/// Both commits below have `inverse_delta: null`. Before this lane that was the *entire* record: a
/// reader could not tell a change that has no undo from a change whose undo was never established,
/// and the two have different remedies (the first is a property of the change; the second is a
/// posture a deployment chose and can unchoose). The assertion is that the two rows answer
/// different words **and** that the two receipts carry different answers, because closing only the
/// escrow row would leave a receipt-holder exactly where they were.
#[test]
fn dr4626_unknown_and_false_are_two_words_over_the_same_receipt_shape() {
    let unknown_dir = scratch("dr4626_verdict_unknown");
    let unknown_adapter = SeamAdapter::new("before", Answer::Undetermined);
    let mut unknown_engine = engine_over(&unknown_dir, &unknown_adapter);
    let unknown_id = commit_one(&mut unknown_engine, "after");

    let false_dir = scratch("dr4626_verdict_false");
    let false_adapter = SeamAdapter::new("before", Answer::NoneAfterReading);
    let mut false_engine = engine_over(&false_dir, &false_adapter);
    let false_id = commit_one(&mut false_engine, "after");

    let unknown_status = unknown_engine.inverse_status(&unknown_id);
    let false_status = false_engine.inverse_status(&false_id);
    println!("AC_V1_STATUS unknown={unknown_status:?} false={false_status:?}");
    assert_eq!(
        unknown_status,
        Some(InverseStatus::Undetermined),
        "AC-V1: `OnReadFailure::Unknown`'s answer now has the word D24 seated for it"
    );
    assert_eq!(
        false_status,
        Some(InverseStatus::Unavailable),
        "the negative control: a declared non-inverse still answers what it always answered"
    );

    let unknown_payload = payload_of(&unknown_engine, unknown_id);
    let false_payload = payload_of(&false_engine, false_id);

    // The shape that used to be all there was.
    assert!(unknown_payload.inverse_delta.is_none());
    assert!(false_payload.inverse_delta.is_none());

    println!(
        "AC_V1_RECEIPT unknown={:?} false={:?}",
        unknown_payload.reversibility, false_payload.reversibility
    );
    assert_eq!(
        unknown_payload.reversibility,
        Some(Reversibility::Unknown),
        "the receipt says nobody found out"
    );
    assert_eq!(
        false_payload.reversibility,
        Some(Reversibility::False),
        "the receipt says there is no inverse"
    );
    assert_ne!(
        unknown_payload.reversibility, false_payload.reversibility,
        "if these agree, the receipt-holder is where `req/38` §198 ruling (b) found them"
    );

    // 🔴 And the read is attested on the arm that had one, which is the pair that makes the
    // `False` answer actionable: gx *looked*, and then declined to escrow an inverse.
    println!(
        "AC_V1_READS unknown={:?} false={:?}",
        unknown_payload.read_set, false_payload.read_set
    );
    // 🔴 **DR-46-34** — `Nothing` where this line read `is_none()`. The read is what failed on this
    // arm, so there is no `{digest, locator}` to attest; what changed is that the receipt now says
    // *that* rather than saying nothing at all.
    assert_eq!(
        unknown_payload.read_set,
        Some(gx_witness::receipt::ReadSet::Nothing),
        "the read is what failed on this arm; attesting one would name an object nothing read"
    );
    assert_eq!(
        false_payload
            .read_set
            .as_ref()
            .expect("this arm read the prior and then found no restore to build")
            .names(SUBJECT),
        Some(true)
    );
}

// ---------------------------------------------------------------------------
// The rebuild road: what a receipt says has to be reproducible without the receipt
// ---------------------------------------------------------------------------

/// 🔴 A commit whose escrow **read** can be rebuilt, and the rebuild carries the same read-set.
///
/// # Why this test exists, said plainly
///
/// It exists because the lane's red phase found it missing. `InverseEscrowed.reads` was journalled
/// (42 §3.13, `req/38` §258-6) on the strength of an argument — 43 §7-3b compares a rebuilt
/// payload's digest against the ledger's leaf, and a road that cannot reach one of fourteen fields
/// cannot reproduce a digest — and the break that removes the journalling left every suite green.
/// The beds that *did* catch the original regression (`gx-cli/tests/model_a_probes.rs`) drive the
/// fs adapter, which reports no reads, so they measure the `reversibility` half of the same repair
/// and not this half.
///
/// [`Engine::reissue_receipt`] is R9's road and runs the same arithmetic without staging a crash:
/// it rebuilds the payload from Σ, the journal and a reading of the substrate, and files it **only**
/// if it digests to what the ledger already witnessed. So "the rebuild reproduces the commit" is
/// `Reissued::Filed`, and "it does not" is `Reissued::WorldMoved` — by construction, not by a
/// message.
#[test]
fn dr4626_a_commit_that_read_can_be_rebuilt_and_the_rebuild_carries_the_read_set() {
    let dir = scratch("dr4626_rebuild_read_set");
    let adapter = SeamAdapter::new("before", Answer::InvertedAfterReading);
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");

    // The receipt T-11 issued, and the read-set in its signed bytes.
    let issued = payload_of(&engine, id);
    let issued_read_set = issued
        .read_set
        .clone()
        .expect("the commit road attests what the escrow read");

    // The journal is where a rebuild can reach it. Asserted here rather than assumed, because the
    // whole claim of this test is about that record and not about the receipt.
    let journalled = engine
        .journal()
        .records()
        .iter()
        .find_map(|record| match record {
            gx_engine::EngineJournalRecord::InverseEscrowed {
                transformation,
                reads,
                ..
            } if *transformation == id => Some(reads.clone()),
            _ => None,
        })
        .expect("T-10b journalled an escrow for this commit");
    println!("DR4626_JOURNALLED_READS={}", journalled.len());
    assert_eq!(
        journalled.len(),
        1,
        "42 §3.13's `InverseEscrowed.reads` is what the rebuild road reads; an empty one here is         the field not being written"
    );
    assert_eq!(journalled[0].locator, SUBJECT);

    // R9's road: rebuild the payload and file it only if it digests to the witnessed leaf.
    let reissued = engine
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue runs");
    println!("DR4626_REISSUED={}", reissued.kind());
    let rebuilt = match reissued {
        gx_engine::pipeline::Reissued::Filed(receipt) => {
            receipt.payload().expect("the rebuilt payload decodes")
        }
        other => panic!(
            "the rebuild does not reproduce the commit the ledger witnessed: {}. If this is         `world_moved`, the payload is missing a field the journal was supposed to carry",
            other.kind()
        ),
    };

    let rebuilt_read_set = rebuilt
        .read_set
        .as_ref()
        .expect("a rebuild carries what the commit attested");
    println!(
        "DR4626_REBUILT_READ_SET granularity={} distinct={} names_subject={:?}",
        rebuilt_read_set.granularity(),
        rebuilt_read_set.distinct_objects(),
        rebuilt_read_set.names(SUBJECT)
    );
    assert_eq!(
        rebuilt.read_set, issued.read_set,
        "the rebuilt read-set is not the one the commit signed"
    );
    assert_eq!(
        rebuilt_read_set.granularity(),
        issued_read_set.granularity()
    );
    assert_eq!(
        rebuilt.reversibility,
        Some(Reversibility::True),
        "and the verdict beside it, derived from the escrow row rather than read back"
    );
}

/// 🔴 The seventh word survives a **replay**, which is where the lane's own defect was.
///
/// # What was wrong, found by the bed above rather than by reasoning
///
/// `Engine::commit` wrote `InverseStatus::Undetermined` into the live escrow row, and the journal
/// record beside it carried only `inverse_cid: None` — which `replay.rs` has mapped to
/// `Unavailable` since **E-M5-9**, correctly, for as long as `SubstrateAdapter::invert` had two
/// values. DR-46-26 gives that absence a *second* preimage, so a restarted process was reporting
/// "we asked and there is none" about a change nobody had established anything about. That is
/// DR-46-13's own defect, arriving one road over, inside the lane that closed it.
///
/// The discriminator is `InverseEscrowed.undetermined` (42 §3.13, `pending`'s `serde(default)`
/// shape), and this is the bed for it: Σ is reconstructed from the journal alone — no engine
/// memory — and asked what the escrow row says.
#[test]
fn dr4626_undetermined_survives_a_replay_of_the_journal_alone() {
    let dir = scratch("dr4626_replay_undetermined");
    let unknown_adapter = SeamAdapter::new("before", Answer::Undetermined);
    let mut engine = engine_over(&dir, &unknown_adapter);
    let unknown_id = commit_one(&mut engine, "after");

    let control_dir = scratch("dr4626_replay_unavailable");
    let false_adapter = SeamAdapter::new("before", Answer::NoneAfterReading);
    let mut control = engine_over(&control_dir, &false_adapter);
    let false_id = commit_one(&mut control, "after");

    // The live rows, before anything is replayed.
    assert_eq!(
        engine.inverse_status(&unknown_id),
        Some(InverseStatus::Undetermined)
    );
    assert_eq!(
        control.inverse_status(&false_id),
        Some(InverseStatus::Unavailable)
    );

    // 🔴 And now from the journal alone. `Engine::replay` is E-M5-2's read-only reconstruction: it
    // calls no adapter and holds no memory of the process that wrote the records.
    let replayed = gx_engine::reconstruct(engine.journal().records());
    let control_replayed = gx_engine::reconstruct(control.journal().records());

    let status_of = |sigma: &gx_engine::Sigma, id: gx_core::TransformationId| {
        sigma
            .escrow()
            .iter()
            .find(|row| row.transformation == id)
            .map(|row| row.status)
    };
    let unknown_status = status_of(&replayed, unknown_id);
    let false_status = status_of(&control_replayed, false_id);
    println!("DR4626_REPLAYED unknown={unknown_status:?} false={false_status:?}");
    assert_eq!(
        unknown_status,
        Some(InverseStatus::Undetermined),
        "the seventh word did not survive the journal: a restart reports `Unavailable` about a         change nobody established anything about, which is DR-46-13 with extra steps"
    );
    assert_eq!(
        false_status,
        Some(InverseStatus::Unavailable),
        "the negative control: E-M5-9's absence still means what it meant, and the discriminator         did not turn every empty escrow into an undetermined one"
    );
    assert_ne!(unknown_status, false_status);
}

/// 🔴 **Was declared open, and DR-46-31 opened it**: the rebuild's `Unknown` arm, reached.
///
/// # The history, kept because it is the reason this test has the shape it has
///
/// ~~Declared open: the rebuild's verdict map has two arms no bed in this tree can reach.
/// [`Engine::rebuilt_attest`] maps an escrow row's status to C-25's answer, and the bed above
/// reaches the `True` arm through `reissue_receipt`. The `False` and `Unknown` arms are **not**
/// reachable, and the reason is not this lane's: (1) every commit with no inverse is escalated —
/// **E-M3-4**, and [`commit_one`] takes that road for both negative arms; (2)
/// `Engine::reissue_receipt` cannot reproduce an escalated commit's payload, because `replay.rs`'s
/// `HumanDecision` arm sets `StateRow.verdict` and leaves `verdict_digest` at the T-4c `Escalate`
/// proof's digest. So this test asserts the blocker rather than the arm, which is the honest form:
/// it fails the day somebody fixes `verdict_digest`, and the fix is the day the arms above become
/// measurable.~~
///
/// **That day is DR-46-31.** `req/453` §10 raised the blocker rather than leaving it in a comment,
/// `req/470` §4-3 confirmed it at the coordinate, `req/38` §261 ruling 2b numbered it, and
/// `HumanDecision` now carries the ruling's digest. The instruction the struck paragraph left —
/// "do that instead of asserting the blocker" — is what this test now does: it measures the arm.
/// `crates/gx-engine/tests/dr4631_escalated_reissue.rs` is the repair's own bed; this one is the
/// half that says DR-46-26's declared-open arm stopped being open.
#[test]
fn dr4626_the_verdict_maps_unknown_arm_is_reached_through_an_escalated_reissue() {
    let dir = scratch("dr4626_blocked_arm");
    let adapter = SeamAdapter::new("before", Answer::Undetermined);
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");

    let reissued = engine
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue runs");
    println!("DR4626_UNKNOWN_ARM reissue={}", reissued.kind());
    let rebuilt = match reissued {
        gx_engine::pipeline::Reissued::Filed(receipt) => {
            receipt.payload().expect("the rebuilt payload decodes")
        }
        other => panic!(
            "the escalated re-issue answered `{}` -- DR-46-31's repair has been backed out and \
             this arm is unreachable again",
            other.kind()
        ),
    };
    assert_eq!(
        rebuilt.reversibility,
        Some(Reversibility::Unknown),
        "the rebuild carries the third word, which is the arm that was declared open"
    );
    // 🔴 **DR-46-34** — and the rebuild reproduces the *same* member the issue road chose, which
    // is what lets the payload digest to the leaf the ledger already holds.
    assert_eq!(
        rebuilt.read_set,
        Some(gx_witness::receipt::ReadSet::Nothing),
        "the read is what failed, so there is no {{digest, locator}} to attest"
    );

    // What used to be the blocker, now read from the other side: Σ and the receipt agree on
    // **both** halves of the verdict. The `assert_ne!` this line replaces is what DR-46-31 fixed.
    let sigma = gx_engine::reconstruct(engine.journal().records());
    let row = sigma.state_of(&id).cloned().expect("Σ holds the row");
    let issued = payload_of(&engine, id);
    println!(
        "DR4626_UNKNOWN_ARM sigma_verdict={:?} sigma_digest={:?} receipt_verdict={:?}",
        row.verdict,
        row.verdict_digest.is_some(),
        issued.verdict.as_ref().map(|v| v.kind)
    );
    assert_eq!(
        row.verdict,
        issued.verdict.as_ref().map(|v| v.kind),
        "Σ and the receipt agree on the verdict **kind**"
    );
    assert_eq!(
        row.verdict_digest,
        issued.verdict.as_ref().map(|v| v.proof_digest),
        "🔴 **DR-46-31** -- and now on its proof digest, which is what unblocked this arm"
    );
}

/// 🔴 The negative control: a rebuild of a commit whose escrow read **nothing** carries nothing.
///
/// Without this the test above is satisfied by a rebuild that stamps a read-set onto everything,
/// which is the same failure mode `dr4626_an_escrow_that_read_nothing_carries_no_read_set` guards
/// on the issue road. `ReadSet::from_reads` answers the same member for an empty set on both
/// roads, and that is the point: the rebuild re-derives through the same constructor.
///
/// 🔴 **DR-46-34** — the member is `ReadSet::Nothing` where this control read `is_none()`. The
/// rebuild reaching it is *not* automatic any more: it reaches it because the journal record says
/// `reads_attested`, and `gx-engine/tests/dr4634_read_set_absence.rs` holds the bed where that
/// flag is `false` and the same road is refused.
#[test]
fn dr4626_a_rebuild_of_a_commit_that_read_nothing_carries_no_read_set() {
    let dir = scratch("dr4626_rebuild_no_reads");
    let adapter = SeamAdapter::new("before", Answer::InvertedWithoutReading);
    let mut engine = engine_over(&dir, &adapter);
    let id = commit_one(&mut engine, "after");

    let reissued = engine
        .reissue_receipt(&id, AT, &signing_key())
        .expect("the re-issue runs");
    println!("DR4626_REISSUED_CONTROL={}", reissued.kind());
    let rebuilt = match reissued {
        gx_engine::pipeline::Reissued::Filed(receipt) => {
            receipt.payload().expect("the rebuilt payload decodes")
        }
        other => panic!(
            "the control's rebuild did not reproduce the commit: {}",
            other.kind()
        ),
    };
    assert_eq!(
        rebuilt.read_set,
        Some(gx_witness::receipt::ReadSet::Nothing),
        "a rebuild invented a read-set for an escrow that read nothing"
    );
    assert_eq!(rebuilt.reversibility, Some(Reversibility::True));
}

/// 🔴 The two facts arrive from **one** adapter call, which is why they can be digested together.
///
/// `req/38` §195 clause ⑤ bounds the T-10b critical section at one server round trip, and
/// `gx-adapter-mcp`'s `invert_with_verdict` exists as one function rather than two for exactly this
/// reason ("so that answering the verdict costs the same single read the escrow costs, never two").
/// The engine's side of that is that it calls `invert` once per commit; this counts it.
#[test]
fn dr4626_the_verdict_and_the_read_set_cost_one_invert_call() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct Counting {
        inner: SeamAdapter,
        calls: Arc<AtomicUsize>,
    }

    impl SubstrateAdapter for Counting {
        fn kind(&self) -> SubstrateKind {
            self.inner.kind()
        }
        fn snapshot(&self, locator: &str) -> gx_substrate::Result<gx_core::ObjectSnapshot> {
            self.inner.snapshot(locator)
        }
        fn plan(
            &self,
            intent: &gx_core::Intent,
            pre: &gx_core::ObjectSnapshot,
        ) -> gx_substrate::Result<PlannedDelta> {
            self.inner.plan(intent, pre)
        }
        fn precondition(
            &self,
            snap: &gx_core::ObjectSnapshot,
        ) -> gx_substrate::Result<Fingerprint> {
            self.inner.precondition(snap)
        }
        fn apply(&self, delta: &PlannedDelta) -> gx_substrate::Result<gx_substrate::AppliedDelta> {
            self.inner.apply(delta)
        }
        fn invert(
            &self,
            delta: &PlannedDelta,
            pre: &gx_core::ObjectSnapshot,
        ) -> gx_substrate::Result<InvertOutcome> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inner.invert(delta, pre)
        }
        fn commutation(
            &self,
            a: &PlannedDelta,
            b: &PlannedDelta,
        ) -> gx_substrate::Result<gx_core::Commutation> {
            self.inner.commutation(a, b)
        }
    }

    let dir = scratch("dr4626_one_call");
    let calls = Arc::new(AtomicUsize::new(0));
    let adapter = Counting {
        inner: SeamAdapter::new("before", Answer::InvertedAfterReading),
        calls: Arc::clone(&calls),
    };
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("the engine opens");
    engine.register_adapter(Arc::new(adapter), "dr4626-counting-fixture/1");

    let before = calls.load(Ordering::SeqCst);
    let one = intent(SUBJECT, "after");
    let key = signing_key();
    engine.submit(&one, 7, AT).expect("submit");
    let id = engine.plan(&one, AT).expect("plan");
    engine.verify(&id, AT, &key, None).expect("verify");
    let after_verify = calls.load(Ordering::SeqCst);
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    engine.commit(&id, AT, &key).expect("commit");
    let after_commit = calls.load(Ordering::SeqCst);

    println!("DR4626_INVERT_CALLS before={before} after_verify={after_verify} after_commit={after_commit}");
    assert_eq!(before, 0);
    assert_eq!(
        after_verify, 1,
        "E-M4-5: verify folds one `invert` into `invert_available`, and the widening did not add a         second call beside it"
    );
    assert_eq!(
        after_commit, 2,
        "T-10b's escrow is the second and last: the verdict and the read-set ride the call that         was already being made, which is `req/38` §195 clause ⑤ left where it was"
    );

    let payload = payload_of(&engine, id);
    assert_eq!(payload.reversibility, Some(Reversibility::True));
    assert!(payload.read_set.is_some());
}
