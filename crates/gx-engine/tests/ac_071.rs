//! **AC-071** — an escalation approved by a person (FR-058, DR-11, 43 T-5, INV-S6).
//!
//! 34 AC-071 逐語:
//!
//! > Given: `Escalated`状態のCandidate T（EscalationTicket発行済み）。When:
//! > `gx escalation approve <ticket-id> --reason "reviewed and approved"`（またはAPI …）を実行する。
//! > Then: Tは`Admitted`へ遷移し、以後canonicalize→commitのpipelineが続行可能になる。発行される
//! > receipt trail（journal/Receiptメタデータ）に`Evidence(HumanDecision)`（decision=Admit, reason,
//! > 裁定者actor）が含まれることを確認する。 | integration + E2E | M5/M6
//!
//! # What is in scope, and what the M列 excludes
//!
//! 51 §15's M5 row is 「escalation解決・owner cancel（AC-071〜073, DR-11）の**engineロジック分**pass」
//! and req/78 N-01 keeps `gx escalation approve` out of this milestone entirely. So the trigger here
//! is [`gx_engine::Engine::escalation`] and not a command line; the E2E half is M6's.
//!
//! # 🔴 `Evidence(HumanDecision)` is a type that does not exist, and **E-M2-3** is why
//!
//! > **E-M2-3**: Evidence=42 の 4 variant が正(43/44/34/35 の `HumanDecision` 参照は erratum・
//! > DR-03-1 の HumanApprovalToken が対応物)
//!
//! and gx-witness's own module documentation spells out where the fact goes instead: 「43 T-5's
//! 「人間裁定receipt（署名済み）」 is a receipt」. So 「receipt trail（journal/Receiptメタデータ）に…
//! 含まれる」 is read as the pair this hand writes — the journal's `HumanDecision` record, which
//! carries `decision`, `reason` and the ruler (**M5H6-2**), and the signed
//! [`gx_witness::ReceiptKind::VerdictReceipt`], which carries the ruler's key inside the signature.
//! Both are asserted below, and the reading is raised as **M5H6-7** rather than assumed.

mod support;

use std::sync::Arc;

use gx_core::{Timestamp, VerdictKind};
use gx_engine::{Engine, HumanRuling, InjectedEvidence, Lifecycle};
use support::{gate, intent, ruler, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const RULED_AT: Timestamp = Timestamp(1_754_000_060_000_000_000);

/// The reason AC-071 writes, verbatim.
const REASON: &str = "reviewed and approved";

/// 🔴 AC-071: approve, and the whole pipeline continues from where the person left it.
#[test]
fn ac_071_an_approved_escalation_becomes_admitted_and_commits() {
    let dir = scratch("ac071");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    // **E-M3-4**: an adapter that cannot build an inverse is the one condition producing an
    // `Escalate` in v0.1, which is how a `Escalated` Given is constructed without inventing a policy.
    let (adapter, counts, world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter.without_inverse()), "commit-adapter-1");

    let i = intent("/tmp/escalated.txt", "after");
    engine.submit(&i, 42, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    let escalated = engine.verify(&id, AT, &signing_key(), None).expect("T-4c");
    let ticket = engine.ticket(&id).cloned().expect("T-4c raised one");

    // The ruler is not the submitter, and the key is not the engine's. 43 T-5's guard is
    // 「裁定者が有効な署名鍵を保持」, so the ruling is signed under a key of the person's own.
    let owner_key = gx_witness::KeyPair::from_seed("key-owner-1", &[11u8; 32]);
    let ruling = HumanRuling {
        decision: VerdictKind::Admit,
        reason: REASON.to_string(),
        actor: ruler(1),
    };
    let admitted = engine
        .escalation(&id, &ruling, RULED_AT, &owner_key)
        .expect("T-5");

    // 「以後canonicalize→commitのpipelineが続行可能になる」 -- so the pipeline is continued.
    engine.canonicalize(&id, RULED_AT, None).expect("T-8");
    let committed = engine.commit(&id, RULED_AT, &signing_key()).expect("T-11");

    let decisions: Vec<_> = engine
        .journal()
        .records()
        .iter()
        .filter(|r| r.kind() == "HumanDecision")
        .collect();
    let receipts = engine.verdict_receipts(&id);
    let human = receipts
        .last()
        .expect("T-5 issued one")
        .payload()
        .expect("decodes");

    println!(
        "AC071 escalated={escalated:?} ticket={:?} admitted={admitted:?} committed={committed:?} \
         human_decisions={} verdict_receipts={} human_key={:?} human_verdict={:?} \
         applies={} leaves={} world={:?}",
        ticket.id,
        decisions.len(),
        receipts.len(),
        human.key_id,
        human.verdict.as_ref().map(|v| v.kind),
        counts.totals()[4],
        engine.ledger().log().len(),
        String::from_utf8_lossy(&world.lock().expect("the world is not poisoned"))
    );

    assert_eq!(escalated, Lifecycle::Escalated, "the Given");
    assert_eq!(admitted, Lifecycle::Admitted, "43 T-5's to-state");
    assert_eq!(committed, Lifecycle::Committed, "「以後…続行可能になる」");
    assert_eq!(engine.ledger().log().len(), 1);
    assert_eq!(counts.totals()[4], 1, "the change was applied exactly once");

    // The journal half of the trail.
    assert_eq!(decisions.len(), 1, "one ruling, one record");
    match decisions[0] {
        gx_engine::EngineJournalRecord::HumanDecision {
            kind,
            reason,
            actor,
            at,
            ..
        } => {
            assert_eq!(*kind, VerdictKind::Admit, "decision=Admit");
            assert_eq!(reason, REASON, "AC-071's `reason` reaches the record");
            assert_eq!(*actor, ruler(1), "AC-071's 裁定者actor");
            assert_eq!(*at, RULED_AT, "and the clock 41 §6 injected");
        }
        other => panic!("the filter admitted a {other:?}"),
    }

    // The receipt half. Two of them: T-4c's, signed by the engine's key, and T-5's, signed by the
    // ruler's -- 43 T-5's 「provenance鎖に追記」 as a chain rather than a replacement.
    assert_eq!(receipts.len(), 2, "T-4c then T-5");
    assert_eq!(
        receipts[0]
            .payload()
            .expect("decodes")
            .verdict
            .map(|v| v.kind),
        Some(VerdictKind::Escalate)
    );
    assert_eq!(
        human.verdict.as_ref().map(|v| v.kind),
        Some(VerdictKind::Admit)
    );
    assert_eq!(
        human.key_id,
        *owner_key.key_id(),
        "43 T-5: 「裁定者が有効な署名鍵を保持」 -- the receipt names whose"
    );
    assert_eq!(
        human.receipt_kind,
        gx_witness::ReceiptKind::VerdictReceipt,
        "ASM-14's first kind; the commit's is the second"
    );
    assert!(
        gx_witness::receipt::verify_offline(
            receipts.last().expect("one"),
            &owner_key.verifying(),
            None
        )
        .expect("a signed ruling verifies")
        .verified(),
        "a human ruling nobody can check is not a witness of anything"
    );
}

/// 🔴 **INV-S6, the other direction**: a ruling on something nobody escalated is refused.
///
/// The probe below says 「nothing but a ruling moves an `Escalated`」. This says 「a ruling moves
/// nothing else」, and the two together are what INV-S6 asks for. Without it, widening 43 T-5's
/// from-state guard would let a person admit a `Candidate` **no gate has ever seen** — an approval
/// that skips T-3, T-4a..e and the whole of FR-032 — and every probe in this hand would stay green.
///
/// Found by the mutation battery rather than by design: row (m) of `tools/verify_m5h6.sh` replaced
/// the guard with `if false` and **nothing failed**. That is what a battery is for, and the honest
/// record is that this probe exists because a mutation survived.
#[test]
fn ac_071_a_ruling_on_a_transformation_nobody_escalated_is_refused() {
    let dir = scratch("ac071_from_state");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let ruling = HumanRuling {
        decision: VerdictKind::Admit,
        reason: REASON.to_string(),
        actor: ruler(1),
    };
    let i = intent("/tmp/never-escalated.txt", "after");
    engine.submit(&i, 46, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");

    // Four from-states 43 T-5 does not offer, each read with the journal length beside it: a guard
    // that refused *after* writing the record would satisfy the state check and break the log.
    let mut refusals: Vec<(&str, Option<String>, usize)> = Vec::new();
    let probe = |engine: &mut Engine<InjectedEvidence>, name: &'static str| {
        let before = engine.journal().len();
        let refusal = engine
            .escalation(&id, &ruling, RULED_AT, &signing_key())
            .err()
            .map(|e| e.kind().to_string());
        (name, refusal, engine.journal().len() - before)
    };
    refusals.push(probe(&mut engine, "Candidate"));
    engine.verify(&id, AT, &signing_key(), None).expect("T-4a");
    refusals.push(probe(&mut engine, "Admitted"));
    engine.canonicalize(&id, AT, None).expect("T-8");
    refusals.push(probe(&mut engine, "Canonicalized"));
    engine.commit(&id, AT, &signing_key()).expect("T-11");
    refusals.push(probe(&mut engine, "Committed"));

    println!(
        "AC071_FROM_STATE refusals={refusals:?} human_decisions={} state={:?} applies={} \
         leaves={} world={:?}",
        engine
            .journal()
            .records()
            .iter()
            .filter(|r| r.kind() == "HumanDecision")
            .count(),
        engine.state(&id),
        counts.totals()[4],
        engine.ledger().log().len(),
        String::from_utf8_lossy(&world.lock().expect("the world is not poisoned"))
    );
    for (name, refusal, written) in &refusals {
        assert_eq!(
            refusal.as_deref(),
            Some("InvalidState"),
            "43 T-5's from-state is `Escalated`, and `{name}` is not it"
        );
        assert_eq!(*written, 0, "a refused ruling from `{name}` wrote a record");
    }
    assert_eq!(
        engine
            .journal()
            .records()
            .iter()
            .filter(|r| r.kind() == "HumanDecision")
            .count(),
        0,
        "no person ruled on this transformation at any point"
    );
    assert_eq!(engine.state(&id), Some(Lifecycle::Committed), "unchanged");
}

/// 🔴 **INV-S6**: nothing else moves an `Escalated` to `Admitted`.
///
/// > `Escalated`はT-5/T-5bの署名済み人間裁定receiptを経由せずに`Admitted`/`Denied`へ自動遷移しない
///
/// The absence, measured from four directions: `verify` refuses it (T-3 is a `Candidate`'s),
/// `canonicalize` refuses it (43 T-8's from-state is `Admitted`), `commit` refuses it, and the
/// reaper leaves it alone until its deadline. The one road that works is T-5, and the probe above
/// walks it.
#[test]
fn ac_071_nothing_but_a_ruling_moves_an_escalated_transformation() {
    let dir = scratch("ac071_invs6");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter.without_inverse()), "commit-adapter-1");

    let i = intent("/tmp/stuck.txt", "after");
    engine.submit(&i, 43, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    assert_eq!(
        engine.verify(&id, AT, &signing_key(), None).expect("T-4c"),
        Lifecycle::Escalated
    );

    let refusals = [
        (
            "verify",
            engine
                .verify(&id, AT, &signing_key(), None)
                .err()
                .map(|e| e.kind().to_string()),
        ),
        (
            "canonicalize",
            engine
                .canonicalize(&id, AT, None)
                .err()
                .map(|e| e.kind().to_string()),
        ),
        (
            "commit",
            engine
                .commit(&id, AT, &signing_key())
                .err()
                .map(|e| e.kind().to_string()),
        ),
    ];
    let swept = engine.reap(AT).expect("a sweep well inside the deadline");
    println!(
        "AC071_INVS6 refusals={refusals:?} swept={} state={:?} applies={} leaves={}",
        swept.len(),
        engine.state(&id),
        counts.totals()[4],
        engine.ledger().log().len()
    );
    for (name, refusal) in &refusals {
        assert_eq!(
            refusal.as_deref(),
            Some("InvalidState"),
            "`{name}` moved an Escalated transformation"
        );
    }
    assert!(swept.is_empty(), "the deadline is 72 h away");
    assert_eq!(engine.state(&id), Some(Lifecycle::Escalated));
    assert_eq!(counts.totals()[4], 0);
    assert_eq!(engine.ledger().log().len(), 0);
}
