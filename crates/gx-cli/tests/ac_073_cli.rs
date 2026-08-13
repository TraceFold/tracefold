//! 🔴 **AC-073 (FR-059, DR-11, 43 T-7)** — `gx cancel` through the binary, with **E-M6-1**'s from-set.
//!
//! 34 AC-073 逐語:
//!
//! > Given: `Committing`到達前の任意状態（Draft/Candidate/Verifying/Admitted/Canonicalized/Escalated）
//! > のTransformation T。When: `gx cancel <T.id>`…を実行する。Then: Tは`Aborted(OwnerCancelled)`へ
//! > 遷移する。When: 既に`Committing`以降（Committed含む）のTに対し同コマンドを実行する。Then: 無効
//! > 操作として拒否され既存状態を変更しない。
//!
//! # 🔴 The Given's first word is gone — **E-M6-1**
//!
//! req/38 §47 M6-03 採(c) removed `Draft` from 44 L101's from-set and said the same of this criterion:
//! 「AC-073 の Given も同読み替え(E-M5-14 が 43 でやった形)」. §45 M5H8-2 採(b) — 「M6 の
//! id-resolution が担う」 — was **withdrawn** in the same ruling, with the frame correction recorded:
//! id-resolution solves 「how do I point at it」 and a draft's problem is that there is **no seat**.
//! 43 T-1 leaves a draft without a `TransformationId`, 43 T-7's `Aborted` is keyed on one, and M5-17
//! 採(b) keeps the draft phase in the journal alone.
//!
//! So the last case below is not the criterion failing: it is the criterion as the erratum reads it.
//! `crates/gx-engine/tests/ac_073.rs` has measured the shape from the engine's side since M5
//! (`ac_073_a_draft_has_no_id_to_cancel`); this measures what an operator is **told**.

mod support;

use support::{oversized_before, pipeline, run};

const ABSENT: &str = "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// 🔴 **AC-073's first half** — a `Candidate` cancels, and the file is untouched.
#[test]
fn ac_073_a_candidate_cancels() {
    let fixture = pipeline("m6h4_ac073_candidate", "before\n");
    let tid = fixture.planned_one("after\n");

    let cancelled = run(fixture.gx().args(["cancel", &tid]));
    println!(
        "AC073_CANDIDATE exit={} state={:?} reason={:?} target={:?}",
        cancelled.code,
        cancelled.json()["state"],
        cancelled.json()["reason"],
        fixture.target_contents()
    );
    assert_eq!(
        cancelled.code, 0,
        "44 §1.2 `gx cancel`: 「0=成功」. stderr: {}",
        cancelled.stderr
    );
    // 44 §1.2: 「stdout: `{ "transformation": <id>, "state": "Aborted", "reason": "OwnerCancelled" }`」.
    assert_eq!(cancelled.json()["transformation"], tid);
    assert_eq!(cancelled.json()["state"], "Aborted");
    assert_eq!(cancelled.json()["reason"], "OwnerCancelled");
    assert_eq!(
        fixture.target_contents(),
        "before\n",
        "43 T-7 fires before the critical section, so nothing was escrowed and nothing applied"
    );

    // 43 T-7's idempotency column: 「二重キャンセルは無効操作として無視（既にAborted）」.
    let again = run(fixture.gx().args(["cancel", &tid]));
    println!(
        "AC073_TWICE exit={} state={:?}",
        again.code,
        again.json()["state"]
    );
    assert_eq!(
        again.code, 0,
        "a second cancel is 「無視」 and not an error: {}",
        again.stderr
    );
    assert_eq!(again.json()["state"], "Aborted");
    assert_eq!(again.json()["reason"], "OwnerCancelled");
}

/// 🔴 **AC-073's first half at `Escalated`** — the last state 43 T-7's from-set names.
///
/// The two ends of the from-set are worth measuring separately: `Candidate` is before any verdict and
/// `Escalated` is a transformation a person has been asked about. 43 has exactly two roads out of
/// `Escalated` that are not a human ruling — T-6's expiry and this — and INV-S6 depends on there
/// being no third.
#[test]
fn ac_073_an_escalated_transformation_cancels() {
    let fixture = pipeline("m6h4_ac073_escalated", &oversized_before());
    let tid = fixture.planned_one("after\n");
    let verified = run(fixture.gx().args(["verify", &tid]));
    assert_eq!(
        verified.code, 4,
        "the Given is an `Escalated` row: {}",
        verified.stderr
    );

    let cancelled = run(fixture.gx().args(["cancel", &tid]));
    println!(
        "AC073_ESCALATED exit={} state={:?} reason={:?}",
        cancelled.code,
        cancelled.json()["state"],
        cancelled.json()["reason"]
    );
    assert_eq!(cancelled.code, 0, "stderr: {}", cancelled.stderr);
    assert_eq!(cancelled.json()["state"], "Aborted");
    assert_eq!(cancelled.json()["reason"], "OwnerCancelled");
}

/// 🔴 **AC-073's second half** — a `Committed` transformation is refused and nothing changes.
#[test]
fn ac_073_a_committed_transformation_is_refused() {
    let fixture = pipeline("m6h4_ac073_committed", "before\n");
    let tid = fixture.commit_one("after\n");

    let cancelled = run(fixture.gx().args(["cancel", &tid]));
    println!(
        "AC073_COMMITTED exit={} target={:?} detail={:?}",
        cancelled.code,
        fixture.target_contents(),
        cancelled.stderr.trim()
    );
    // 🔴 **E-M6-13** (req/38 §51 M6H4-1 採(a)), implemented in M6 hand 5. Hand 4 asserted **1**
    // here and recorded the disagreement in prose: 「a state machine refusal wearing 44 §1.4's
    // 「エラー」…kept at 1 because that is what §1.2 writes」. §51 ruled the repair and named the
    // hand — 「実装は手5 以降の最初に踏む手」 — so the number moved, and what the move buys is what
    // the ruling is about: 1 says 「you asked wrongly, try again differently」 and 2 says 「the
    // machine refused, and it will refuse the same way for ever」.
    assert_eq!(
        cancelled.code, 2,
        "E-M6-13: 44 §1.4's 2 is 「拒否（denied）」 and 43 T-7's guard is 「`Committing`到達前」, so a \
         cancel of a committed row is 43 §3 saying no. stdout: {}",
        cancelled.stdout
    );
    assert!(
        cancelled.stdout.contains("43 T-7"),
        "🔴 and the refusal is **T-7's**, not a resume's. A `gx cancel` on a committed row would \
         otherwise be answered 「43 §3 has no `plan` from a Committed row」 — true, and an answer to \
         a question nobody asked. 🔴 It is now on **stdout** rather than stderr: E-M6-13 makes this \
         an `Outcome` (「the command ran and answered no」) rather than an `Err` (「the command could \
         not run」), and 44 §1.3 gives the first a JSON object: {}",
        cancelled.stdout
    );
    assert_eq!(
        fixture.target_contents(),
        "after\n",
        "34 AC-073: 「既存状態を変更しない」"
    );
}

/// 🔴 **E-M6-1** — a `Draft` is refused **by name**, and the refusal says what discarding one is.
///
/// The from-set no longer contains `Draft` (req/38 §47 M6-03 採(c)), and the reason an operator gets
/// has to be the real one rather than 「未検出」: the id they typed is a perfectly good `IntentId`
/// that this project is holding a draft under. Telling them it is unknown would be
/// 「読めない」/「無い」 conflated one layer up (E-M4-35).
#[test]
fn e_m6_1_a_draft_is_refused_by_name_and_not_reported_missing() {
    let fixture = pipeline("m6h4_ac073_draft", "before\n");
    let submitted = fixture.submit("after\n");
    assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
    let intent_id = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();

    let cancelled = run(fixture.gx().args(["cancel", &intent_id]));
    println!(
        "AC073_DRAFT exit={} detail={:?}",
        cancelled.code,
        cancelled.stderr.trim()
    );
    assert_eq!(
        cancelled.code, 1,
        "a draft is 「入力不正」 and not 「未検出」: the name resolves, the operation does not exist"
    );
    assert!(
        cancelled.stderr.contains("E-M6-1"),
        "the refusal cites the erratum that removed `Draft` from the from-set: {}",
        cancelled.stderr
    );
    assert!(
        cancelled.stderr.contains("drafts/"),
        "and it says what discarding a draft actually is, since no verb does it (M6H4-2): {}",
        cancelled.stderr
    );
}

/// An id that parses and names nothing is 44 §1.2's 「6=未検出」.
#[test]
fn cancelling_an_unknown_transformation_is_six() {
    let fixture = pipeline("m6h4_ac073_absent", "before\n");
    let cancelled = run(fixture.gx().args(["cancel", ABSENT]));
    println!("AC073_ABSENT exit={}", cancelled.code);
    assert_eq!(cancelled.code, 6, "stderr: {}", cancelled.stderr);
}

/// 🔴 `--actor-key` is refused: 43 T-7's owner guard has no enforcement point in v0.1 (M5H6-4 採(a)).
#[test]
fn the_cancel_flag_with_nowhere_to_go_is_refused() {
    let fixture = pipeline("m6h4_ac073_actor_key", "before\n");
    let tid = fixture.planned_one("after\n");
    let refused = run(fixture
        .gx()
        .args(["cancel", &tid])
        .args(["--actor-key", "key-not-the-owner"]));
    println!(
        "AC073_ACTOR_KEY exit={} detail={:?}",
        refused.code,
        refused.stderr.trim()
    );
    assert_eq!(refused.code, 1);
    assert!(
        refused.stderr.contains("M6H4-3"),
        "an unchecked permission is disclosed rather than implied: {}",
        refused.stderr
    );
}
