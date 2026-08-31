// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **FR-M04** -- the aggregate verdict checkpoint (M7 hand 6, `req/98` §3-3, ruling #2/#3/#14) (sem: SEM-gx-engine-593).
//!
//! # The defect this suite is about
//!
//! `V021_SYNC_MATERIAL_2026-08-11.md:35` verbatim (sem: SEM-gx-engine-594): "since `VerdictReceipt`
//! is fixed at `inclusion_proof=None` (42§3.10) and never appears in the ledger at all, an operator
//! who simply does not export / discards whole `VerdictReceipt`s where a Deny/Escalate occurred can
//! present an external auditor with nothing but a clean record showing a 100% Admit rate". A
//! `CommitReceipt` is anchored in a Merkle tree an auditor can re-derive; a
//! `VerdictReceipt` is a signed sheet of paper its issuer may simply not hand over.
//!
//! FR-M04's answer is **not** to put verdicts in the tree — ASM-14 forbids it (`check_schema`
//! refuses a `VerdictReceipt` carrying an `inclusion_proof`) — but to publish a signed **count**
//! and bind it to something the operator has already committed to. That is option A (`req/98` §3-3:
//! "a VerdictCheckpoint that runs alongside on a new payload_type") (sem: SEM-gx-engine-595), and
//! this file is what makes it a fact.
//!
//! # RED-first
//!
//! This file is committed **before** any of the API it names exists, so its first run is a
//! compile-stage RED (discipline 47 (sem: SEM-gx-engine-596): a 101 that did not compile measured nothing, and this file says so
//! rather than letting the two 101s look alike). The assertion-stage RED of the same hand is in
//! `crates/gx-log/tests/ac_021.rs`, whose declaration lists the new public names before they are
//! written.
//!
//! # The oracle is not the producer
//!
//! `AC-VC-1` compares the checkpoint's claim against a count this file computes **from the journal
//! records**, by a fold written here against 43's transition table. The producer's counter is
//! incremented where the receipt is *issued*. Two roads, and the same reason req/103 §2-3 wrote an
//! RFC 6962 transcription rather than reusing the crate's own walk: a recount that reused the
//! producer's arithmetic would be green whenever the producer was self-consistent.
//!
//! 🔴 **The limit of that independence, declared**: across a restart the producer's counter is
//! rebuilt from the journal too (`Engine::open`), so the two roads share a source there and a
//! journal that lost a `Verdict` record would be invisible to both. Inside one session they are
//! genuinely two. `the_counter_survives_a_restart` measures continuity and is **not** an
//! independence probe; it says so in its own name and its own body.
//!
//! # What this suite deliberately shows FR-M04 **failing** to detect
//!
//! ruling #3 and ruling #14 ask for two limits to be written into the AC rather than discovered later. (sem: SEM-gx-engine-597)
//! Written in a doc they are a claim; here they are two probes that pass **because** the detection
//! does not fire (`policy_relaxation_is_not_detected`, `a_split_view_is_not_detected`). A limit
//! nobody exercised is a limit nobody knows is still there.

mod support;

use std::sync::Arc;

use gx_core::{Checkpoint, FailPosture, Timestamp, VerdictKind, VerdictTally};
use gx_engine::{
    Engine, EngineJournalRecord, EvidenceSource, InjectedEvidence, Lifecycle, UnreachableEvidence,
};
use gx_log::proof::{audit_verdict_chain, ChainBreak};
use gx_log::{detect_verdict_equivocation, VerdictContradiction};
use gx_witness::dsse;
use support::{
    gate, gate_refusing, intent, scratch, signing_key, CommitAdapter, StubAdapter, PERMIT_ALL,
};

/// The one clock, injected (41 §6).
const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// The log namespace a checkpoint is scoped to (42 §3.11's `origin`, for the parallel artefact).
const ORIGIN: &str = "glovrex-verdicts/v1";

/// The payload the fixture deployment's invariant refuses.
const REFUSED: &str = "the change this deployment refuses";
/// A payload it does not.
const ALLOWED: &str = "a change nobody objects to";

// ---------------------------------------------------------------------------
// The oracle: a recount from the journal, written against 43 rather than against the engine
// ---------------------------------------------------------------------------

/// Count the verdicts a journal witnesses, by the rule 43's transition table states.
///
/// Three rows produce a `VerdictReceipt` and this fold names all three:
///
/// * T-4a / T-4b / T-4c — `Verdict { kind, verdict_digest: Some(_) }`, one per gate answer.
/// * T-4e — `Verdict { kind: Admit, verdict_digest: None, fail_posture_engaged: true }`. **No gate
///   ran.** Folding it into `admit` would report an admission nobody decided, which is "not
///   implemented" and "failure" (sem: SEM-gx-engine-598) wearing one face (M4H4-2, refused
///   twice). It gets its own bucket.
/// * T-5 / T-5b — `HumanDecision { kind }`, where a person answered.
///
/// Every other record is skipped, and the skip is not silent: `total` is asserted against the
/// window width, so a row this fold forgot would show up as an arithmetic disagreement rather than
/// as a smaller number that looks plausible.
fn recount_from_the_journal(records: &[EngineJournalRecord]) -> VerdictTally {
    let mut tally = VerdictTally::default();
    for record in records {
        match record {
            EngineJournalRecord::Verdict {
                kind,
                verdict_digest: Some(_),
                ..
            }
            | EngineJournalRecord::HumanDecision { kind, .. } => bump(&mut tally, *kind),
            EngineJournalRecord::Verdict {
                kind: VerdictKind::Admit,
                verdict_digest: None,
                ..
            } => tally.unverdicted += 1,
            EngineJournalRecord::Verdict {
                kind,
                verdict_digest: None,
                ..
            } => panic!(
                "43 T-4e is the only row with no verdict digest and its kind is Admit; found {kind:?}"
            ),
            _ => {}
        }
    }
    tally
}

fn bump(tally: &mut VerdictTally, kind: VerdictKind) {
    match kind {
        VerdictKind::Admit => tally.admit += 1,
        VerdictKind::Deny => tally.deny += 1,
        VerdictKind::Escalate => tally.escalate += 1,
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// An engine whose gate denies [`REFUSED`] and admits everything else, with an invertible adapter.
///
/// One engine can hold both answers because the refusal is decided by the **payload** at T-4b,
/// while the adapter is the same for every row. `Escalate` cannot join them: E-M3-4 raises it at
/// T-3 from `invert() == None`, which is an adapter-wide property, so the escalating arm is a
/// second engine (`escalating_engine`) rather than a third payload.
fn mixed_engine(name: &str) -> Engine<InjectedEvidence> {
    let dir = scratch(name);
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate_refusing(PERMIT_ALL, "deployment-refuses-this", REFUSED),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    engine.register_adapter(Arc::new(adapter), "ac-vc-mixed");
    engine
}

/// Drive one transformation to its verdict and answer the id and the state it reached.
fn one_verdict<E: EvidenceSource>(
    engine: &mut Engine<E>,
    locator: &str,
    goal: &str,
) -> (gx_core::TransformationId, Lifecycle) {
    let i = intent(locator, goal);
    engine.submit(&i, 1, AT).expect("T-1");
    let id = engine.plan(&i, AT).expect("T-2");
    let state = engine.verify(&id, AT, &signing_key(), None).expect("T-4");
    (id, state)
}

/// `denies` refusals and `admits` admissions on one engine, interleaved, with every admission
/// carried all the way to a **commit**.
///
/// Interleaved rather than blocked, because a counter that only ever saw a run of one kind would
/// be green under a mutation that latched on the first kind it saw.
///
/// 🔴 The commit is not decoration. `ledger_tree_size` is the field AC-VC-5's second and third
/// cases rest on, and a fixture that stopped at `Admitted` would leave the ledger empty — every
/// chain would trivially be "not behind" (sem: SEM-gx-engine-599) a log of zero leaves, and two of the three absences
/// would be measured against nothing.
///
/// `round` keeps the locators distinct across calls: an intent's id is derived from its locator
/// and goal, so a second call with the same pair asks the state machine to plan a transformation
/// it has already judged.
fn drive<E: EvidenceSource>(engine: &mut Engine<E>, round: usize, admits: usize, denies: usize) {
    let key = signing_key();
    let rows = admits.max(denies);
    for n in 0..rows {
        if n < admits {
            let (id, state) = one_verdict(
                engine,
                &format!("/tmp/ac-vc-admit-{round}-{n}.txt"),
                ALLOWED,
            );
            assert_eq!(
                state,
                Lifecycle::Admitted,
                "the permitting payload at row {n}"
            );
            engine.canonicalize(&id, AT, None).expect("T-8");
            engine.commit(&id, AT, &key).expect("T-10/T-11");
        }
        if n < denies {
            let (_, state) =
                one_verdict(engine, &format!("/tmp/ac-vc-deny-{round}-{n}.txt"), REFUSED);
            assert_eq!(state, Lifecycle::Denied, "the refused payload at row {n}");
        }
    }
}

/// An engine on which every transformation escalates (E-M3-4: `invert() == None` at T-3).
fn escalating_engine(name: &str) -> Engine<InjectedEvidence> {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    engine.register_adapter(Arc::new(StubAdapter::without_inverse()), "ac-vc-escalate");
    engine
}

// ---------------------------------------------------------------------------
// AC-VC-1
// ---------------------------------------------------------------------------

/// **AC-VC-1**: the counts a checkpoint commits to are the counts an independent recount finds.
#[test]
fn ac_vc_1_the_claimed_counts_survive_an_independent_recount() {
    let mut engine = mixed_engine("ac_vc_1");
    drive(&mut engine, 0, 3, 5);

    let key = signing_key();
    let checkpoint = engine
        .verdict_checkpoint(ORIGIN, AT, &key)
        .expect("a checkpoint over eight verdicts");
    let oracle = recount_from_the_journal(engine.journal().records());

    println!(
        "ACVC1_CLAIMED admit={} deny={} escalate={} unverdicted={} window={}..{} ledger={} \
         ACVC1_ORACLE admit={} deny={} escalate={} unverdicted={}",
        checkpoint.tally.admit,
        checkpoint.tally.deny,
        checkpoint.tally.escalate,
        checkpoint.tally.unverdicted,
        checkpoint.window_start,
        checkpoint.window_end,
        checkpoint.ledger_tree_size,
        oracle.admit,
        oracle.deny,
        oracle.escalate,
        oracle.unverdicted,
    );

    assert_eq!(
        checkpoint.tally, oracle,
        "the checkpoint's claim and the journal recount are two roads to one number"
    );
    assert_eq!(checkpoint.tally.deny, 5, "five refusals were driven");
    assert_eq!(checkpoint.tally.admit, 3, "three admissions were driven");
    assert_eq!(
        checkpoint.tally.total(),
        checkpoint.window_end - checkpoint.window_start,
        "the window's width is the number of verdicts inside it; a tally that does not add up to \
         it is a checkpoint about a window it did not cover"
    );
    assert_eq!(checkpoint.window_start, 0, "the first window opens at zero");

    let verified = dsse::verify_verdict_checkpoint(&checkpoint, &key.verifying());
    assert!(
        verified.is_ok(),
        "the engine signs what it publishes: {verified:?}"
    );
}

/// The Escalate bucket, on the engine that can produce one.
#[test]
fn ac_vc_1b_escalations_are_counted_as_escalations() {
    let mut engine = escalating_engine("ac_vc_1b");
    for n in 0..4 {
        let (_, state) = one_verdict(&mut engine, &format!("/tmp/ac-vc-esc-{n}.txt"), ALLOWED);
        assert_eq!(state, Lifecycle::Escalated, "E-M3-4 at row {n}");
    }
    let key = signing_key();
    let checkpoint = engine
        .verdict_checkpoint(ORIGIN, AT, &key)
        .expect("a checkpoint over four escalations");
    let oracle = recount_from_the_journal(engine.journal().records());

    println!(
        "ACVC1B_ESCALATE claimed={} oracle={} admit={} deny={}",
        checkpoint.tally.escalate, oracle.escalate, checkpoint.tally.admit, checkpoint.tally.deny
    );
    assert_eq!(checkpoint.tally, oracle);
    assert_eq!(checkpoint.tally.escalate, 4);
    assert_eq!(
        checkpoint.tally.admit + checkpoint.tally.deny,
        0,
        "an escalation is not an admission with a note on it"
    );
}

/// 🔴 T-4e has its **own** bucket: a gate that never ran did not admit.
#[test]
fn ac_vc_1c_an_admission_no_gate_made_is_not_an_admit() {
    let dir = scratch("ac_vc_1c");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        UnreachableEvidence::new("the collector is down"),
    )
    .expect("a fresh journal opens")
    .with_posture(FailPosture::FailOpen);
    engine.register_adapter(Arc::new(adapter), "ac-vc-t4e");

    for n in 0..3 {
        let (_, state) = one_verdict(&mut engine, &format!("/tmp/ac-vc-t4e-{n}.txt"), ALLOWED);
        assert_eq!(state, Lifecycle::Admitted, "T-4e admits under FailOpen");
    }

    let key = signing_key();
    let checkpoint = engine.verdict_checkpoint(ORIGIN, AT, &key).expect("mint");
    let oracle = recount_from_the_journal(engine.journal().records());
    println!(
        "ACVC1C_T4E unverdicted={} admit={} oracle_unverdicted={}",
        checkpoint.tally.unverdicted, checkpoint.tally.admit, oracle.unverdicted
    );
    assert_eq!(checkpoint.tally, oracle);
    assert_eq!(
        checkpoint.tally.unverdicted, 3,
        "43 T-4e: `enforced=false, fail_posture_engaged=true` and **no verdict was computed**"
    );
    assert_eq!(
        checkpoint.tally.admit, 0,
        "the whole point of the fourth bucket: a reader of `admit` is reading gate answers"
    );
}

// ---------------------------------------------------------------------------
// AC-VC-2
// ---------------------------------------------------------------------------

/// **AC-VC-2**: an operator who signs a smaller number than the verifier can count is detected.
///
/// The lie is **re-signed**, which is the threat model: the operator holds the key. A probe that
/// only flipped a bit would measure AC-VC-3 and call it AC-VC-2.
#[test]
fn ac_vc_2_underreporting_is_detected_even_when_the_lie_is_signed() {
    let mut engine = mixed_engine("ac_vc_2");
    drive(&mut engine, 0, 2, 6);
    let key = signing_key();
    let honest = engine.verdict_checkpoint(ORIGIN, AT, &key).expect("mint");
    let observed = recount_from_the_journal(engine.journal().records());
    let ledger_size = engine.ledger().log().len();

    let clean = audit_verdict_chain(std::slice::from_ref(&honest), &observed, ledger_size);
    assert!(
        clean.is_empty(),
        "the honest chain has nothing to report: {clean:?}"
    );

    let mut lying = honest.clone();
    lying.tally.deny -= 4;
    lying.window_end -= 4;
    let lying = dsse::sign_verdict_checkpoint(&lying, key.signing_key(), key.key_id())
        .expect("the operator holds the key and can sign a smaller number");
    assert!(
        dsse::verify_verdict_checkpoint(&lying, &key.verifying()).is_ok(),
        "the lie is a valid signature over a false statement, which is why a signature check is \
         not the detection"
    );

    let found = audit_verdict_chain(std::slice::from_ref(&lying), &observed, ledger_size);
    println!("ACVC2_FINDINGS {found:?}");
    assert!(
        found.iter().any(|b| matches!(
            b,
            ChainBreak::Underreported {
                kind: VerdictKind::Deny,
                ..
            }
        )),
        "a verifier holding six refusals and shown two must not be told the chain is clean: {found:?}"
    );
}

// ---------------------------------------------------------------------------
// AC-VC-3
// ---------------------------------------------------------------------------

/// **AC-VC-3**: one bit inside the signed core is a signature failure, not a different meaning.
#[test]
fn ac_vc_3_a_changed_count_is_refused_by_the_signature() {
    let mut engine = mixed_engine("ac_vc_3");
    drive(&mut engine, 0, 1, 2);
    let key = signing_key();
    let checkpoint = engine.verdict_checkpoint(ORIGIN, AT, &key).expect("mint");
    assert!(dsse::verify_verdict_checkpoint(&checkpoint, &key.verifying()).is_ok());

    let mut moved = [
        ("deny", checkpoint.clone()),
        ("window_end", checkpoint.clone()),
        ("origin", checkpoint.clone()),
        ("ledger_tree_size", checkpoint.clone()),
    ];
    moved[0].1.tally.deny += 1;
    moved[1].1.window_end += 1;
    moved[2].1.origin = "glovrex-verdicts/v2".to_string();
    moved[3].1.ledger_tree_size += 1;

    for (field, altered) in &moved {
        let answer = dsse::verify_verdict_checkpoint(altered, &key.verifying());
        println!("ACVC3_FIELD {field} answer={answer:?}");
        assert!(
            answer.is_err(),
            "{field} is inside the signed core; a change to it must be a bad signature"
        );
    }

    // The clock is **outside** the core (CM-5, and the same choice `Checkpoint` made), so moving it
    // is not a signature failure. Measured rather than implied: a reader of a verified checkpoint
    // is reading a statement about counts, not about when they were written down.
    let mut later = checkpoint.clone();
    later.timestamp = Timestamp(AT.0 + 60_000_000_000);
    assert!(
        dsse::verify_verdict_checkpoint(&later, &key.verifying()).is_ok(),
        "CM-5: the timestamp is not covered, and this is the probe that keeps that honest"
    );
}

// ---------------------------------------------------------------------------
// AC-VC-4
// ---------------------------------------------------------------------------

/// **AC-VC-4**: the third payload type separates this signature from the other two.
#[test]
fn ac_vc_4_the_payload_type_separates_the_loads() {
    let mut engine = mixed_engine("ac_vc_4");
    drive(&mut engine, 0, 1, 1);
    let key = signing_key();
    let checkpoint = engine.verdict_checkpoint(ORIGIN, AT, &key).expect("mint");

    let types = [
        dsse::RECEIPT_PAYLOAD_TYPE,
        dsse::CHECKPOINT_PAYLOAD_TYPE,
        dsse::REVOCATION_PAYLOAD_TYPE,
        dsse::VERDICT_CHECKPOINT_PAYLOAD_TYPE,
    ];
    let mut distinct = types.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    println!(
        "ACVC4_PAYLOAD_TYPES declared={} distinct={}",
        types.len(),
        distinct.len()
    );
    assert_eq!(
        distinct.len(),
        types.len(),
        "four loads under one key are separated by four length-prefixed strings (E-M2-26)"
    );

    // 🔴 The property itself, over **one** core: the frame is what differs.
    //
    // Signing is not needed to see it and would obscure it — two signatures over two different
    // cores differ for two reasons at once. `pae` is public, so the isolated statement is
    // available: the same bytes, wrapped under two payload types, are two messages.
    let core = gx_log::proof::verdict_checkpoint_signing_bytes(&checkpoint).expect("canonical");
    let as_head = dsse::pae(dsse::CHECKPOINT_PAYLOAD_TYPE, &core);
    let as_verdicts = dsse::pae(dsse::VERDICT_CHECKPOINT_PAYLOAD_TYPE, &core);
    assert_ne!(
        as_head, as_verdicts,
        "E-M2-26: the length-prefixed payload type is inside the signed bytes, so one key signing \
         two loads produces two messages rather than two encodings that happen not to collide"
    );
    assert_eq!(
        dsse::verdict_checkpoint_signing_message(&checkpoint).expect("canonical"),
        as_verdicts,
        "and the message the signer actually uses is that one"
    );

    // And end to end, both ways: a real signature from one road, offered on the other.
    let head = gx_log::proof::unsigned_checkpoint(engine.ledger().log(), ORIGIN, AT)
        .expect("the ledger has a leaf: one admission committed");
    let signed_head =
        dsse::sign_checkpoint(&head, key.signing_key(), key.key_id()).expect("sign the tree head");
    let mut wearing_a_head = checkpoint.clone();
    wearing_a_head.signature = signed_head.signature.clone();
    let answer = dsse::verify_verdict_checkpoint(&wearing_a_head, &key.verifying());
    println!("ACVC4_CROSS_FRAME answer={answer:?}");
    assert!(
        answer.is_err(),
        "a tree head's signature must not verify a verdict checkpoint"
    );

    let mut head_wearing_verdicts: Checkpoint = signed_head;
    head_wearing_verdicts.signature = checkpoint.signature.clone();
    assert!(
        dsse::verify_checkpoint(&head_wearing_verdicts, &key.verifying()).is_err(),
        "and the other direction, so the check is red both ways"
    );
}

// ---------------------------------------------------------------------------
// AC-VC-5
// ---------------------------------------------------------------------------

/// **AC-VC-5**: a checkpoint that was never published does not look like a quiet period.
///
/// Three shapes of absence, and the chain audit names all three:
///
/// 1. a **hole** — window boundaries stop meeting;
/// 2. a **truncated tail** — the chain stops while the ledger it is bound to kept growing;
/// 3. **nothing at all** — an empty chain beside a non-empty ledger.
#[test]
fn ac_vc_5_a_missing_checkpoint_is_observable() {
    let mut engine = mixed_engine("ac_vc_5");
    let key = signing_key();

    let mut chain = Vec::new();
    for round in 0..3 {
        drive(&mut engine, round as usize, 2, 1);
        chain.push(
            engine
                .verdict_checkpoint(ORIGIN, Timestamp(AT.0 + round), &key)
                .expect("mint"),
        );
    }
    let observed = recount_from_the_journal(engine.journal().records());
    let ledger_size = engine.ledger().log().len();
    assert!(
        ledger_size > 0,
        "six admissions committed; without a growing ledger case 2 measures nothing"
    );

    assert!(
        audit_verdict_chain(&chain, &observed, ledger_size).is_empty(),
        "the whole chain is clean before anything is removed"
    );

    // 1. the hole.
    let holed = [chain[0].clone(), chain[2].clone()];
    let found = audit_verdict_chain(&holed, &observed, ledger_size);
    println!("ACVC5_HOLE {found:?}");
    assert!(
        found.iter().any(|b| matches!(b, ChainBreak::Gap { .. })),
        "windows that stop meeting are a checkpoint that was removed: {found:?}"
    );

    // 2. the truncated tail. The operator publishes the first window and stops; the ledger did not.
    let truncated = [chain[0].clone()];
    let found = audit_verdict_chain(&truncated, &observed, ledger_size);
    println!("ACVC5_TRUNCATED {found:?}");
    assert!(
        found
            .iter()
            .any(|b| matches!(b, ChainBreak::BehindTheLedger { .. })),
        "a chain that speaks for fewer admissions than the ledger holds leaves has stopped \
         publishing: {found:?}"
    );

    // 3. nothing at all -- option C's re-entrant defect (sem: SEM-gx-engine-600) (`FRM04_DESIGN_MATERIAL:118`), refused.
    let found = audit_verdict_chain(&[], &observed, ledger_size);
    println!("ACVC5_EMPTY {found:?}");
    assert!(
        found
            .iter()
            .any(|b| matches!(b, ChainBreak::BehindTheLedger { .. })),
        "publishing no checkpoint at all is the cheapest way to have nothing to contradict, and \
         it is the one an empty chain must not be allowed to look like: {found:?}"
    );
}

/// The counter is durable: a restart does not reopen window zero.
///
/// 🔴 **Not an independence probe.** After a restart the producer rebuilds its counter from the
/// journal, which is where the oracle above reads too — this measures that the two windows meet,
/// and says nothing about whether the journal is right.
#[test]
fn the_counter_survives_a_restart() {
    let dir = scratch("ac_vc_restart");
    let journal = dir.join("journal.bin");
    let key = signing_key();

    let first = {
        let (adapter, _counts, _world) = CommitAdapter::new("before");
        let mut engine = Engine::open(
            journal.clone(),
            gate_refusing(PERMIT_ALL, "deployment-refuses-this", REFUSED),
            InjectedEvidence::none(),
        )
        .expect("a fresh journal opens");
        engine.register_adapter(Arc::new(adapter), "ac-vc-restart-1");
        drive(&mut engine, 0, 1, 2);
        engine.verdict_checkpoint(ORIGIN, AT, &key).expect("mint")
    };

    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let mut reopened = Engine::open(
        journal,
        gate_refusing(PERMIT_ALL, "deployment-refuses-this", REFUSED),
        InjectedEvidence::none(),
    )
    .expect("the journal replays");
    reopened.register_adapter(Arc::new(adapter), "ac-vc-restart-2");

    println!(
        "ACVC_RESTART first_window={}..{} reopened_tally_total={} persisted={}",
        first.window_start,
        first.window_end,
        reopened.verdict_tally().total(),
        reopened.verdict_checkpoints().len(),
    );
    assert_eq!(
        reopened.verdict_tally(),
        first.tally,
        "the replayed counter is the one the first process published"
    );
    assert_eq!(
        reopened.verdict_checkpoints().len(),
        1,
        "the checkpoint itself is on the device, not only in the process that made it"
    );

    drive(&mut reopened, 1, 0, 1);
    let second = reopened
        .verdict_checkpoint(ORIGIN, Timestamp(AT.0 + 1), &key)
        .expect("mint");
    assert_eq!(
        second.window_start, first.window_end,
        "the second window opens where the first closed; a restart that reopened window zero \
         would publish a chain full of holes and blame the auditor"
    );
    let observed = recount_from_the_journal(reopened.journal().records());
    assert!(
        audit_verdict_chain(&[first, second], &observed, reopened.ledger().log().len()).is_empty(),
        "and the chain across the restart audits clean"
    );
}

// ---------------------------------------------------------------------------
// The two limits, measured rather than described (ruling #3, ruling #14) (sem: SEM-gx-engine-601)
// ---------------------------------------------------------------------------

/// 🔴 **the limit of ruling #3** (sem: SEM-gx-engine-602): FR-M04 does **not** close policy relaxation, and here is the proof.
///
/// Two engines, the same eight transformations, one deployment that refuses five of them and one
/// that refuses nothing. The second publishes `deny=0` and every check in this file is green about
/// it — because zero refusals is exactly what a gate that refuses nothing produces, and a count is
/// a count. What FR-M04 buys is that **non-disclosure** is detectable; loosening the gate until
/// there is nothing to disclose is a different attack and this is the probe that says so.
#[test]
fn policy_relaxation_is_not_detected() {
    let key = signing_key();

    let mut strict = mixed_engine("ac_vc_limit_strict");
    drive(&mut strict, 0, 3, 5);
    let strict_cp = strict.verdict_checkpoint(ORIGIN, AT, &key).expect("mint");

    let dir = scratch("ac_vc_limit_loose");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let mut loose = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    loose.register_adapter(Arc::new(adapter), "ac-vc-loose");
    for n in 0..8 {
        let (_, state) = one_verdict(&mut loose, &format!("/tmp/ac-vc-loose-{n}.txt"), REFUSED);
        assert_eq!(
            state,
            Lifecycle::Admitted,
            "the loose deployment admits the very payload the strict one refuses"
        );
    }
    let loose_cp = loose.verdict_checkpoint(ORIGIN, AT, &key).expect("mint");
    let loose_observed = recount_from_the_journal(loose.journal().records());
    let findings = audit_verdict_chain(
        std::slice::from_ref(&loose_cp),
        &loose_observed,
        loose.ledger().log().len(),
    );

    println!(
        "ACVC_LIMIT_RELAXATION strict_deny={} loose_deny={} loose_findings={} \
         (ruling #3: policy relaxation is not closed) (sem: SEM-gx-engine-603)",
        strict_cp.tally.deny,
        loose_cp.tally.deny,
        findings.len()
    );
    assert_eq!(strict_cp.tally.deny, 5);
    assert_eq!(loose_cp.tally.deny, 0);
    assert!(
        findings.is_empty(),
        "this assertion is the limit: the loose deployment's chain is clean, and FR-M04 cannot \
         tell \"nothing was refused\" from \"the gate was widened until nothing was\" (sem: SEM-gx-engine-604)"
    );
}

/// 🔴 **the limit of ruling #14** (sem: SEM-gx-engine-605): FR-M04 does **not** close a split view (TH-6 residue, v0.2.1).
///
/// One key signs two internally consistent chains for two verifiers. Each verifier, auditing the
/// chain it was handed against the receipts it was handed, finds nothing — because what the
/// signature attests is "this key stated these counts" and never "these are the only counts this
/// key stated" (sem: SEM-gx-engine-606). Detecting the fork needs the two verifiers to compare,
/// which is the consistency proof window `req/98` §9 row v-6 sends to v0.2.1.
///
/// 🔴 **`req/964` B-6, 2026-08-31 — that comparison exists now, and the assertions below are
/// unchanged because they are still true.** What they measure is *single-view* blindness, which is
/// the half of ruling #14 that stays open: each verifier, alone, finds nothing. The sentence above
/// stays as written (no-delete) with this note carrying the correction. The closing block hands the
/// **same two signed views** to `gx_log::detect_verdict_equivocation` and the fork is named, so this
/// one test now holds both halves: alone, blind; pooled, seen. What no code closes is the pooling.
#[test]
fn a_split_view_is_not_detected() {
    let mut engine = mixed_engine("ac_vc_limit_split");
    drive(&mut engine, 0, 2, 6);
    let key = signing_key();
    let honest = engine.verdict_checkpoint(ORIGIN, AT, &key).expect("mint");
    let full = recount_from_the_journal(engine.journal().records());
    let ledger_size = engine.ledger().log().len();

    // Verifier B is shown a smaller world **and** the smaller set of receipts that agrees with it.
    let mut shrunk = honest.clone();
    shrunk.tally.deny -= 4;
    shrunk.window_end -= 4;
    let shrunk = dsse::sign_verdict_checkpoint(&shrunk, key.signing_key(), key.key_id())
        .expect("the same key signs the second view");
    let partial = VerdictTally {
        deny: full.deny - 4,
        ..full
    };

    let a = audit_verdict_chain(std::slice::from_ref(&honest), &full, ledger_size);
    let b = audit_verdict_chain(std::slice::from_ref(&shrunk), &partial, ledger_size);
    println!(
        "ACVC_LIMIT_SPLIT a_findings={} b_findings={} a_deny={} b_deny={} \
         (ruling #14: a split view is the consistency-proof window = v0.2.1) (sem: SEM-gx-engine-607)",
        a.len(),
        b.len(),
        honest.tally.deny,
        shrunk.tally.deny
    );
    assert!(a.is_empty(), "verifier A is shown a consistent world");
    assert!(
        b.is_empty(),
        "and so is verifier B -- this assertion is the limit, not a success"
    );
    assert_ne!(
        honest.tally.deny, shrunk.tally.deny,
        "the two worlds disagree, and nothing inside either one can see it"
    );

    // 🔴 `req/964` B-6: the same two signed views, pooled. Each was clean on its own above; the
    // union is not, and that is the whole content of "detecting the fork needs the verifiers to
    // compare". Verifier B was shown the same `window_start` with a shorter `window_end`, so the
    // shape named is the overlap rather than the equal-window case -- moving the seam is not an
    // escape. This is a real engine's checkpoint signed by a real key, not a hand-built fixture.
    let pooled = detect_verdict_equivocation(&[honest, shrunk]);
    println!("ACVC_LIMIT_SPLIT_POOLED findings={}", pooled.len());
    assert_eq!(
        pooled.len(),
        1,
        "the pooled views contradict each other exactly once: {pooled:?}"
    );
    assert!(
        matches!(pooled[0], VerdictContradiction::OverlappingWindows { .. }),
        "a shorter window over the same start is two chains, not one: {pooled:?}"
    );
}

/// The window is contiguous by construction: a second checkpoint over no new verdicts is empty
/// rather than a repeat of the first.
#[test]
fn a_window_with_no_verdicts_in_it_is_empty_rather_than_a_repeat() {
    let mut engine = mixed_engine("ac_vc_empty_window");
    drive(&mut engine, 0, 1, 1);
    let key = signing_key();
    let first = engine.verdict_checkpoint(ORIGIN, AT, &key).expect("mint");
    let second = engine
        .verdict_checkpoint(ORIGIN, Timestamp(AT.0 + 1), &key)
        .expect("mint");
    println!(
        "ACVC_EMPTY_WINDOW first={}..{} second={}..{} second_total={}",
        first.window_start,
        first.window_end,
        second.window_start,
        second.window_end,
        second.tally.total()
    );
    assert_eq!(second.window_start, first.window_end);
    assert_eq!(second.window_end, first.window_end);
    assert_eq!(
        second.tally.total(),
        0,
        "a checkpoint that repeated the previous window's counts would double every verdict for \
         any verifier that folds the chain"
    );
    let observed = recount_from_the_journal(engine.journal().records());
    assert!(
        audit_verdict_chain(&[first, second], &observed, engine.ledger().log().len()).is_empty()
    );
}

// ---------------------------------------------------------------------------
// K6 mutant-kill (req/38 §73, priority 9 items (sem: SEM-gx-engine-608), req/159 §B-1): the arms no other probe drives
// ---------------------------------------------------------------------------

/// An engine on which every verify lands in 43 T-4e (collector down, `FailOpen`).
///
/// The same fixture `ac_vc_1c` builds inline, named here because T-4e is the one road to
/// `unverdicted` — and run e's surviving mutants sat in exactly the arithmetic no other suite
/// pushes a nonzero `unverdicted` through. (The restart probe below builds its T-4e session
/// inline instead: its three sessions must share one journal path.)
fn t4e_engine(name: &str) -> Engine<UnreachableEvidence> {
    let dir = scratch(name);
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        UnreachableEvidence::new("the collector is down"),
    )
    .expect("a fresh journal opens")
    .with_posture(FailPosture::FailOpen);
    engine.register_adapter(Arc::new(adapter), "ac-vc-t4e-fixture");
    engine
}

/// 🔴 K6 mutant-kill (`tally_from_the_journal`: the T-4e arm, `escalate += 1`,
/// `unverdicted += 1`): the rebuilt counter recounts **all four** buckets across a restart.
///
/// `the_counter_survives_a_restart` crosses the restart with admissions and refusals only, so
/// mutants run e (req/38 §73) left four survivors in the fold behind `Engine::open`: delete the
/// `Verdict { verdict_digest: None }` arm (staging pipeline.rs:633), `escalate += 1 -> *= 1`
/// (:642), and `unverdicted += 1 -> -= 1 / *= 1` (:643). Each of those is invisible until a
/// nonzero escalation count and a nonzero T-4e count cross a restart; here both do, and each
/// bucket is asserted **by value** — an arithmetic rewrite in any arm is a different number.
///
/// One journal, three sessions: the counter under test is rebuilt from the journal alone, so
/// the fixture may change gate and evidence source between sessions — the journal is the truth
/// the recount reads, not the engine configuration that happened to write it.
#[test]
fn the_rebuilt_counter_recounts_escalations_and_t4es() {
    let dir = scratch("ac_vc_rebuild_all_buckets");
    let journal = dir.join("journal.bin");

    // Session 1: two escalations (E-M3-4: `invert() == None` at T-3).
    {
        let mut engine = Engine::open(journal.clone(), gate(PERMIT_ALL), InjectedEvidence::none())
            .expect("a fresh journal opens");
        engine.register_adapter(Arc::new(StubAdapter::without_inverse()), "k6-esc");
        for n in 0..2 {
            let (_, state) = one_verdict(&mut engine, &format!("/tmp/k6-esc-{n}.txt"), ALLOWED);
            assert_eq!(state, Lifecycle::Escalated, "the adapter has no inverse");
        }
    }
    // Session 2: three T-4e degraded admissions (collector down, `FailOpen`).
    {
        let (adapter, _counts, _world) = CommitAdapter::new("before");
        let mut engine = Engine::open(
            journal.clone(),
            gate(PERMIT_ALL),
            UnreachableEvidence::new("the collector is down"),
        )
        .expect("the journal reopens")
        .with_posture(FailPosture::FailOpen);
        engine.register_adapter(Arc::new(adapter), "k6-t4e");
        for n in 0..3 {
            let (_, state) = one_verdict(&mut engine, &format!("/tmp/k6-t4e-{n}.txt"), ALLOWED);
            assert_eq!(state, Lifecycle::Admitted, "T-4e admits under FailOpen");
        }
    }
    // Session 3: one admission and one refusal, so the two buckets the old restart probe
    // already drives are nonzero too — a fold that latched on one kind would show here.
    {
        let (adapter, _counts, _world) = CommitAdapter::new("before");
        let mut engine = Engine::open(
            journal.clone(),
            gate_refusing(PERMIT_ALL, "deployment-refuses-this", REFUSED),
            InjectedEvidence::none(),
        )
        .expect("the journal reopens");
        engine.register_adapter(Arc::new(adapter), "k6-mixed");
        let (_, admitted) = one_verdict(&mut engine, "/tmp/k6-admit-0.txt", ALLOWED);
        assert_eq!(admitted, Lifecycle::Admitted);
        let (_, denied) = one_verdict(&mut engine, "/tmp/k6-deny-0.txt", REFUSED);
        assert_eq!(denied, Lifecycle::Denied);
    }

    // The restart under measurement: `open` rebuilds the counter from the journal alone.
    let reopened = Engine::open(journal, gate(PERMIT_ALL), InjectedEvidence::none())
        .expect("the journal replays");
    let tally = reopened.verdict_tally();
    let oracle = recount_from_the_journal(reopened.journal().records());
    println!(
        "K6_REBUILD admit={} deny={} escalate={} unverdicted={} oracle_escalate={} \
         oracle_unverdicted={}",
        tally.admit,
        tally.deny,
        tally.escalate,
        tally.unverdicted,
        oracle.escalate,
        oracle.unverdicted
    );
    assert_eq!(tally, oracle, "the rebuilt counter is the oracle's recount");
    assert_eq!(tally.escalate, 2, "two escalations crossed the restart");
    assert_eq!(
        tally.unverdicted, 3,
        "three T-4e admissions crossed the restart"
    );
    assert_eq!(tally.admit, 1, "one gate admission");
    assert_eq!(tally.deny, 1, "one refusal");
}

/// 🔴 K6 mutant-kill (`verdict_checkpoint`: `escalate` window subtraction, staging
/// pipeline.rs:1053 `- -> +`): a second window of escalations holds only its own.
///
/// Every existing two-checkpoint probe crosses the boundary with `published.escalate == 0`,
/// where subtraction and addition agree. Two escalations published, one more escalated, and
/// the second window's claim is `1`: the mutant's `verdicts + published` says `5` here.
#[test]
fn a_second_window_of_escalations_subtracts_the_published_ones() {
    let mut engine = escalating_engine("ac_vc_second_window_escalate");
    let key = signing_key();
    for n in 0..2 {
        let (_, state) = one_verdict(&mut engine, &format!("/tmp/k6-esc2-{n}.txt"), ALLOWED);
        assert_eq!(state, Lifecycle::Escalated);
    }
    let first = engine.verdict_checkpoint(ORIGIN, AT, &key).expect("mint");
    assert_eq!(first.tally.escalate, 2, "the first window publishes both");

    let (_, state) = one_verdict(&mut engine, "/tmp/k6-esc2-2.txt", ALLOWED);
    assert_eq!(state, Lifecycle::Escalated);
    let second = engine
        .verdict_checkpoint(ORIGIN, Timestamp(AT.0 + 1), &key)
        .expect("mint");
    println!(
        "K6_WINDOW_ESCALATE first={} second={} second_total={}",
        first.tally.escalate,
        second.tally.escalate,
        second.tally.total()
    );
    assert_eq!(
        second.tally.escalate, 1,
        "the second window speaks for the one escalation after the boundary, not for the \
         published ones again"
    );
    assert_eq!(second.tally.total(), 1, "and for nothing else");
}

/// 🔴 K6 mutant-kill (`verdict_checkpoint`: `unverdicted` window subtraction, staging
/// pipeline.rs:1054 `- -> +`): a second window of T-4e admissions holds only its own.
///
/// The same boundary as the probe above, on the fourth bucket: two degraded admissions
/// published, one more admitted under the engaged posture, and the second window claims `1`
/// where the mutant claims `5`.
#[test]
fn a_second_window_of_t4es_subtracts_the_published_ones() {
    let mut engine = t4e_engine("ac_vc_second_window_t4e");
    let key = signing_key();
    for n in 0..2 {
        let (_, state) = one_verdict(&mut engine, &format!("/tmp/k6-t4e2-{n}.txt"), ALLOWED);
        assert_eq!(state, Lifecycle::Admitted, "T-4e admits under FailOpen");
    }
    let first = engine.verdict_checkpoint(ORIGIN, AT, &key).expect("mint");
    assert_eq!(
        first.tally.unverdicted, 2,
        "the first window publishes both"
    );

    let (_, state) = one_verdict(&mut engine, "/tmp/k6-t4e2-2.txt", ALLOWED);
    assert_eq!(state, Lifecycle::Admitted);
    let second = engine
        .verdict_checkpoint(ORIGIN, Timestamp(AT.0 + 1), &key)
        .expect("mint");
    println!(
        "K6_WINDOW_T4E first={} second={} second_total={}",
        first.tally.unverdicted,
        second.tally.unverdicted,
        second.tally.total()
    );
    assert_eq!(
        second.tally.unverdicted, 1,
        "the second window speaks for the one T-4e after the boundary, not for the published \
         ones again"
    );
    assert_eq!(second.tally.total(), 1, "and for nothing else");
}

/// 🔴 K6 mutant-kill (`Engine::open`'s published fold: `escalate`/`unverdicted` legs, staging
/// pipeline.rs:828/:829 `+ -> -` and `+ -> *`, mutants run e, `req/38` §73): a reopened chain
/// does not republish the escalations and T-4es it already published.
///
/// FR-M04 rebuilds `published` at `open` by folding the **whole checkpoint chain** (the last
/// entry carries its own window, not the sum before it). `the_counter_survives_a_restart`
/// crosses the restart with `admit`/`deny` only, and the two second-window probes above never
/// restart — so all four rewrites of the fold's third and fourth legs survived: under `-` the
/// fold underflows the moment a published count is nonzero (a debug reopen dies right there),
/// and under `*` the accumulator stays pinned at zero, so the next window re-claims everything
/// ever counted. Here one escalation and one T-4e are published *before* the restart, one more
/// of each follows it, and each post-restart window must speak for exactly one.
#[test]
fn a_reopened_chain_does_not_republish_escalations_or_t4es() {
    let dir = scratch("ac_vc_reopen_fold");
    let journal = dir.join("journal.bin");
    let key = signing_key();

    // Session 1: one escalation (E-M3-4: `invert() == None` at T-3), published as checkpoint 1.
    {
        let mut engine = Engine::open(journal.clone(), gate(PERMIT_ALL), InjectedEvidence::none())
            .expect("a fresh journal opens");
        engine.register_adapter(Arc::new(StubAdapter::without_inverse()), "k6-fold-esc");
        let (_, state) = one_verdict(&mut engine, "/tmp/k6-fold-esc-0.txt", ALLOWED);
        assert_eq!(state, Lifecycle::Escalated);
        let first = engine.verdict_checkpoint(ORIGIN, AT, &key).expect("mint");
        assert_eq!(first.tally.escalate, 1, "the first window publishes it");
    }
    // Session 2: one T-4e degraded admission (collector down, `FailOpen`), checkpoint 2. This
    // reopen already folds checkpoint 1, so the escalate leg's `-` dies here in a debug build.
    {
        let (adapter, _counts, _world) = CommitAdapter::new("before");
        let mut engine = Engine::open(
            journal.clone(),
            gate(PERMIT_ALL),
            UnreachableEvidence::new("the collector is down"),
        )
        .expect("the chain reopens across its first published escalation")
        .with_posture(FailPosture::FailOpen);
        engine.register_adapter(Arc::new(adapter), "k6-fold-t4e");
        let (_, state) = one_verdict(&mut engine, "/tmp/k6-fold-t4e-0.txt", ALLOWED);
        assert_eq!(state, Lifecycle::Admitted, "T-4e admits under FailOpen");
        let second = engine
            .verdict_checkpoint(ORIGIN, Timestamp(AT.0 + 1), &key)
            .expect("mint");
        assert_eq!(
            second.tally.unverdicted, 1,
            "the second window publishes it"
        );
    }
    // Session 3 (escalate leg, under measurement): the third window holds only its own.
    {
        let mut engine = Engine::open(journal.clone(), gate(PERMIT_ALL), InjectedEvidence::none())
            .expect("the chain reopens across a published escalation and a published T-4e");
        engine.register_adapter(Arc::new(StubAdapter::without_inverse()), "k6-fold-esc-2");
        let (_, state) = one_verdict(&mut engine, "/tmp/k6-fold-esc-1.txt", ALLOWED);
        assert_eq!(state, Lifecycle::Escalated);
        let third = engine
            .verdict_checkpoint(ORIGIN, Timestamp(AT.0 + 2), &key)
            .expect("mint");
        println!(
            "K6_FOLD_ESC third_escalate={} third_total={}",
            third.tally.escalate,
            third.tally.total()
        );
        assert_eq!(
            third.tally.escalate, 1,
            "the reopened fold recovered the published escalation, so the window after the \
             restart speaks for the one new escalation alone"
        );
        assert_eq!(third.tally.total(), 1, "and for nothing else");
    }
    // Session 4 (unverdicted leg, the same restart): the fourth window holds only its own.
    {
        let (adapter, _counts, _world) = CommitAdapter::new("before");
        let mut engine = Engine::open(
            journal,
            gate(PERMIT_ALL),
            UnreachableEvidence::new("the collector is down"),
        )
        .expect("the chain reopens a third time")
        .with_posture(FailPosture::FailOpen);
        engine.register_adapter(Arc::new(adapter), "k6-fold-t4e-2");
        let (_, state) = one_verdict(&mut engine, "/tmp/k6-fold-t4e-1.txt", ALLOWED);
        assert_eq!(state, Lifecycle::Admitted);
        let fourth = engine
            .verdict_checkpoint(ORIGIN, Timestamp(AT.0 + 3), &key)
            .expect("mint");
        println!(
            "K6_FOLD_T4E fourth_unverdicted={} fourth_total={}",
            fourth.tally.unverdicted,
            fourth.tally.total()
        );
        assert_eq!(
            fourth.tally.unverdicted, 1,
            "the reopened fold recovered the published T-4e count, so the window after the \
             restart speaks for the one new degraded admission alone"
        );
        assert_eq!(fourth.tally.total(), 1, "and for nothing else");
    }
}
