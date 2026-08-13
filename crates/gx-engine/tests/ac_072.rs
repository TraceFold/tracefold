//! **AC-072** — an escalation rejected by a person (FR-058, DR-11, 43 T-5b).
//!
//! 34 AC-072 逐語:
//!
//! > Given: `Escalated`状態のCandidate T。When: `gx escalation reject <ticket-id> --reason "policy
//! > violation"`（またはAPI同等）を実行する。Then: Tは`Denied`へ遷移し終端となる（record-onlyモード
//! > 以外ではそれ以上commitへ進めない）。journal記録に`Evidence(HumanDecision)`（decision=Deny,
//! > reason）が含まれることを確認する。
//!
//! The parenthesis is the interesting half. 「record-onlyモード**以外では**」 says the terminality of
//! a rejected escalation is the same terminality 43 §1 gives `Denied` — conditional on
//! `EnforcementMode`. So this suite runs the rejection **twice**, once in each mode, and the two
//! answers differ: under `Enforce` the transformation stops, and under `RecordOnly` T-8r carries it
//! through with `enforced=false` on its receipt. A suite that only ran the first would be asserting
//! half the sentence.
//!
//! See `tests/ac_071.rs` for why 「`Evidence(HumanDecision)`」 is read as the journal record plus the
//! signed verdict receipt (**E-M2-3**, **M5H6-2**, **M5H6-7**).

mod support;

use std::sync::Arc;

use gx_core::{EnforcementMode, Timestamp, VerdictKind};
use gx_engine::{Engine, HumanRuling, InjectedEvidence, Lifecycle};
use support::{gate, intent, ruler, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const RULED_AT: Timestamp = Timestamp(1_754_000_060_000_000_000);
const REASON: &str = "policy violation";

/// What one rejection left behind.
#[derive(Debug)]
struct Rejected {
    state: Lifecycle,
    after_canonicalize: Result<Lifecycle, String>,
    committed: Option<Lifecycle>,
    receipt_enforced: Option<bool>,
    reason_in_journal: Option<String>,
    decision_in_journal: Option<VerdictKind>,
    applies: usize,
    leaves: u64,
}

fn reject_under(mode: EnforcementMode) -> Rejected {
    let dir = scratch(&format!("ac072_{}", mode.as_str()));
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens")
    .with_mode(mode);
    let (adapter, counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter.without_inverse()), "commit-adapter-1");

    let i = intent("/tmp/rejected.txt", "after");
    engine.submit(&i, 44, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    assert_eq!(
        engine.verify(&id, AT, &signing_key(), None).expect("T-4c"),
        Lifecycle::Escalated,
        "the Given"
    );

    let owner_key = gx_witness::KeyPair::from_seed("key-owner-2", &[12u8; 32]);
    let ruling = HumanRuling {
        decision: VerdictKind::Deny,
        reason: REASON.to_string(),
        actor: ruler(2),
    };
    let state = engine
        .escalation(&id, &ruling, RULED_AT, &owner_key)
        .expect("T-5b");

    let after_canonicalize = engine
        .canonicalize(&id, RULED_AT, None)
        .map_err(|e| e.kind().to_string());
    let committed = after_canonicalize
        .is_ok()
        .then(|| engine.commit(&id, RULED_AT, &signing_key()).expect("T-11"));

    let (decision_in_journal, reason_in_journal) = engine
        .journal()
        .records()
        .iter()
        .find_map(|r| match r {
            gx_engine::EngineJournalRecord::HumanDecision { kind, reason, .. } => {
                Some((Some(*kind), Some(reason.clone())))
            }
            _ => None,
        })
        .unwrap_or((None, None));

    Rejected {
        state,
        after_canonicalize,
        committed,
        receipt_enforced: engine
            .receipt(&id)
            .map(|r| r.payload().expect("decodes").enforced),
        reason_in_journal,
        decision_in_journal,
        applies: counts.totals()[4],
        leaves: engine.ledger().log().len(),
    }
}

/// 🔴 AC-072 under `Enforce`: `Denied`, terminal, nothing applied.
#[test]
fn ac_072_a_rejected_escalation_is_denied_and_goes_no_further() {
    let out = reject_under(EnforcementMode::Enforce);
    println!("AC072_ENFORCE {out:?}");
    assert_eq!(out.state, Lifecycle::Denied, "43 T-5b's to-state");
    assert_eq!(
        out.after_canonicalize.as_ref().err().map(String::as_str),
        Some("InvalidState"),
        "「record-onlyモード以外ではそれ以上commitへ進めない」"
    );
    assert!(out.committed.is_none());
    assert_eq!(out.applies, 0);
    assert_eq!(out.leaves, 0, "INV-S4");
    assert_eq!(out.decision_in_journal, Some(VerdictKind::Deny));
    assert_eq!(
        out.reason_in_journal.as_deref(),
        Some(REASON),
        "AC-072: 「journal記録に…（decision=Deny, reason）が含まれる」"
    );
}

/// 🔴 AC-072's parenthesis: under `RecordOnly` the same rejection is carried through.
///
/// 43 T-8r opens from `Denied` 「`EnforcementMode = RecordOnly`（substrate単位または全体設定, DR-2
/// 併設モード）」, and a rejection by a person is a `Denied` like any other — 43 gives T-5b the same
/// to-state as T-4b and no separate arm. So the receipt records 「適用は通ったが、ポリシー上は拒否
/// されていた」 for a person's refusal exactly as it does for a policy's, which is the property
/// AC-037 states and this is the human-ruling instance of it.
#[test]
fn ac_072_the_same_rejection_under_record_only_commits_with_enforced_false() {
    let out = reject_under(EnforcementMode::RecordOnly);
    println!("AC072_RECORD_ONLY {out:?}");
    assert_eq!(out.state, Lifecycle::Denied);
    assert_eq!(
        out.after_canonicalize.as_ref().ok(),
        Some(&Lifecycle::Canonicalized),
        "T-8r opens"
    );
    assert_eq!(out.committed, Some(Lifecycle::Committed));
    assert_eq!(
        out.receipt_enforced,
        Some(false),
        "43 §4: 「receipt には必ず `enforced=false` を刻む」"
    );
    assert_eq!(out.applies, 1);
    assert_eq!(out.leaves, 1);
    assert_eq!(out.decision_in_journal, Some(VerdictKind::Deny));
}

/// 43 has no `Escalated → Escalated` edge, and 42 §3.13 says the record's kind is Admit or Deny.
///
/// The refusal is a value rather than a panic, because a caller passing a third verdict is asking
/// for a transition rather than writing a bug: 44's `POST /v1/candidates/{id}/escalation` takes
/// `{decision}` from a request body, and M6 will hand whatever arrives to this function.
#[test]
fn ac_072_a_person_cannot_escalate_an_escalation() {
    let dir = scratch("ac072_third");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter.without_inverse()), "commit-adapter-1");

    let i = intent("/tmp/third.txt", "after");
    engine.submit(&i, 45, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    engine.verify(&id, AT, &signing_key(), None).expect("T-4c");

    let records = engine.journal().len();
    let refused = engine
        .escalation(
            &id,
            &HumanRuling {
                decision: VerdictKind::Escalate,
                reason: "kick it upstairs".to_string(),
                actor: ruler(3),
            },
            RULED_AT,
            &signing_key(),
        )
        .expect_err("42 §3.13: 「kindはAdmit|Denyのみ」");
    // And an empty reason: 44 §1.2's trigger carries `--reason <text>`, and a ruling nobody can
    // audit is what `Verdict::deny` refuses one layer down for an empty `Vec<Reason>`.
    let empty = engine
        .escalation(
            &id,
            &HumanRuling {
                decision: VerdictKind::Admit,
                reason: "   ".to_string(),
                actor: ruler(3),
            },
            RULED_AT,
            &signing_key(),
        )
        .expect_err("an unexplained ruling");
    println!(
        "AC072_REFUSALS third={:?} empty={:?} records_before={records} records_after={} state={:?}",
        refused.kind(),
        empty.kind(),
        engine.journal().len(),
        engine.state(&id)
    );
    assert_eq!(refused.kind(), "InvalidState");
    assert_eq!(empty.kind(), "Malformed");
    assert_eq!(
        engine.journal().len(),
        records,
        "a refused ruling writes nothing: journal-first means the record follows the decision"
    );
    assert_eq!(engine.state(&id), Some(Lifecycle::Escalated));
}
