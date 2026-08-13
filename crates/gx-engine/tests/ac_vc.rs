//! 🔴 **FR-M04** — the aggregate verdict checkpoint (M7 hand 6, `req/98` §3-3, 裁定 #2/#3/#14).
//!
//! # The defect this suite is about
//!
//! `V021_SYNC_MATERIAL_2026-08-11.md:35` 逐語: 「`VerdictReceipt`は`inclusion_proof=None`固定
//! (42§3.10)でありledgerに一切現れないため、運用者がDeny/Escalateの発生した`VerdictReceipt`を
//! 丸ごとエクスポートしない/破棄すれば、Admit率100%のクリーンな記録だけを外部監査人に提示できて
//! しまう」. A `CommitReceipt` is anchored in a Merkle tree an auditor can re-derive; a
//! `VerdictReceipt` is a signed sheet of paper its issuer may simply not hand over.
//!
//! FR-M04's answer is **not** to put verdicts in the tree — ASM-14 forbids it (`check_schema`
//! refuses a `VerdictReceipt` carrying an `inclusion_proof`) — but to publish a signed **count**
//! and bind it to something the operator has already committed to. That is 案 A (`req/98` §3-3:
//! 「新 payload_type で並走する VerdictCheckpoint」), and this file is what makes it a fact.
//!
//! # RED-first
//!
//! This file is committed **before** any of the API it names exists, so its first run is a
//! compile-stage RED (規律47: a 101 that did not compile measured nothing, and this file says so
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
//! 裁定 #3 and 裁定 #14 ask for two limits to be written into the AC rather than discovered later.
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
///   ran.** Folding it into `admit` would report an admission nobody decided, which is 「未実装」と
///   「失敗」 wearing one face (M4H4-2, refused twice). It gets its own bucket.
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
/// chain would trivially be 「not behind」 a log of zero leaves, and two of the three absences
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

    // 3. nothing at all -- 案 C's re-entrant defect (`FRM04_DESIGN_MATERIAL:118`), refused.
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
// The two limits, measured rather than described (裁定 #3, 裁定 #14)
// ---------------------------------------------------------------------------

/// 🔴 **裁定 #3 の限界**: FR-M04 does **not** close policy relaxation, and here is the proof.
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
         (裁定 #3: policy 緩和は塞がない)",
        strict_cp.tally.deny,
        loose_cp.tally.deny,
        findings.len()
    );
    assert_eq!(strict_cp.tally.deny, 5);
    assert_eq!(loose_cp.tally.deny, 0);
    assert!(
        findings.is_empty(),
        "this assertion is the limit: the loose deployment's chain is clean, and FR-M04 cannot \
         tell 「nothing was refused」 from 「the gate was widened until nothing was」"
    );
}

/// 🔴 **裁定 #14 の限界**: FR-M04 does **not** close a split view (TH-6 residue, v0.2.1).
///
/// One key signs two internally consistent chains for two verifiers. Each verifier, auditing the
/// chain it was handed against the receipts it was handed, finds nothing — because what the
/// signature attests is 「this key stated these counts」 and never 「these are the only counts this
/// key stated」. Detecting the fork needs the two verifiers to compare, which is the consistency
/// proof window `req/98` §9 行 v-6 sends to v0.2.1.
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
         (裁定 #14: split view は consistency proof 窓=v0.2.1)",
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
