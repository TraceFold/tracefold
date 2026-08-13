//! 🔴 **AC-071 / AC-072 (FR-058, DR-11, 43 T-5/T-5b)** — the CLI/API wiring 51 §15's M6 row asks for.
//!
//! 34's implementation column for both is `M5/M6`, and 51 §15's M6 row is 逐語 「`gx escalation
//! approve|reject`/`gx cancel` の CLI/API 結線再確認」. M5 measured the engine logic
//! (`crates/gx-engine/tests/ac_071.rs`, `ac_072.rs`); this measures the same two criteria **through
//! the binary**, which is where the id an operator actually types has to resolve.
//!
//! # 🔴 The Given is built without a stub adapter
//!
//! Both criteria start at 「`Escalated`状態のCandidate T（EscalationTicket発行済み）」, and the engine
//! suites reach it by registering an adapter whose `invert` answers `None`. A CLI has the fs adapter
//! and nothing else, so the Given here is built out of the **one real reason** an fs change cannot be
//! undone: the escrow ceiling (`MAX_INVERSE_PAYLOAD_BYTES`, M4-21 採(a)). An inverse carries the whole
//! old file, so a change over a file above one mebibyte is the change an operator is asked about.
//! E-M3-4 turns that into an `Escalate`, and the road runs entirely through shipped code.
//!
//! # 🔴 What M6-04 made possible, and what this asserts about it
//!
//! 44 §1.2's trigger is `gx escalation approve <TICKET_ID>` and until this hand **no mapping existed
//! from a ticket to anything**: `Engine::ticket` was the forward direction and the journal records
//! the verdict rather than the ticket (42 §3.13). Both spellings are asserted below —
//! the `TicketId` 44 §1.2 writes and the `TransformationId` 44 §2.2 uses for the same operation
//! (M6-04 採(c)) — because an asymmetry inside one specification is a place two implementations grow
//! apart.

mod support;

use support::{oversized_before, pipeline, run, Pipeline};

/// AC-071's reason, verbatim.
const APPROVE_REASON: &str = "reviewed and approved";
/// AC-072's reason, verbatim.
const REJECT_REASON: &str = "policy violation";

/// Get to 「`Escalated`状態のCandidate T（EscalationTicket発行済み）」 and hand back the two names.
fn escalated(name: &str) -> (Pipeline, String, String) {
    let fixture = pipeline(name, &oversized_before());
    let tid = fixture.planned_one("after\n");
    let verified = run(fixture.gx().args(["verify", &tid]));
    println!(
        "ESCALATED_GIVEN exit={} kind={:?} ticket={:?}",
        verified.code,
        verified.json()["kind"],
        verified.json()["ticket"]["id"]
    );
    assert_eq!(
        verified.code, 4,
        "44 §1.2 `gx verify`: 「4=Escalate」. The fs adapter declines an inverse above the escrow \
         ceiling (M4-21 採(a)) and E-M3-4 escalates it. stderr: {}",
        verified.stderr
    );
    assert_eq!(verified.json()["kind"], "Escalate");
    let ticket = verified.json()["ticket"]["id"]
        .as_str()
        .expect("T-4c raised a ticket and 44 §1.2 prints it")
        .to_string();
    (fixture, tid, ticket)
}

/// 🔴 **AC-071** — approve by **ticket id**, and the pipeline continues from where the person left it.
#[test]
fn ac_071_an_approved_escalation_commits() {
    let (fixture, tid, ticket) = escalated("m6h4_ac071");
    // 🔴 **The ruler is not the submitter**, and the fixture makes them different keys on purpose.
    // 42 §3.13's `HumanDecision.actor` is 「the ruler, which is **not** `Transformation.actor` (the
    // submitter)」, and a suite that ruled with the submitter's own key would be unable to tell a
    // correct implementation from one that signed the ruling with the key of the party being ruled
    // on — measured: the battery's mutation (l) survived until this fixture grew a second key.
    let ruler = fixture.another_key();
    assert_ne!(ruler, fixture.key_id, "the ruler is somebody else");

    let approved = run(fixture
        .gx()
        .args(["escalation", "approve", &ticket])
        .args(["--reason", APPROVE_REASON])
        .args(["--actor-key", &ruler]));
    let key = ruler.clone();
    println!(
        "AC071_APPROVE exit={} state={:?} ticket={:?} reason={:?} receipts={:?} signed_by={:?} \
         submitter={}",
        approved.code,
        approved.json()["state"],
        approved.json()["ticket"],
        approved.json()["reason"],
        approved.json()["verdict_receipts"],
        approved.json()["signed_by"],
        fixture.key_id
    );
    assert_eq!(
        approved.code, 0,
        "44 §1.2 `gx escalation approve`: 「0=成功」. stderr: {}",
        approved.stderr
    );
    assert_eq!(
        approved.json()["state"],
        "Admitted",
        "34 AC-071: 「Tは`Admitted`へ遷移し」"
    );
    // 「発行されるreceipt trail（journal/Receiptメタデータ）に`Evidence(HumanDecision)`（decision=
    // Admit, reason, 裁定者actor）が含まれる」. E-M2-3 retired the `Evidence` variant, so the pair is
    // the journal's `HumanDecision` record and the signed `VerdictReceipt` (M5H6-7's reading, which
    // §43 追認'd). The receipt count is what the CLI can see; the record is measured by the engine
    // suite that owns the journal.
    assert_eq!(approved.json()["reason"], APPROVE_REASON);
    assert_eq!(approved.json()["decision"], "Admit");
    assert_eq!(
        approved.json()["ruled_by"]["Human"]["key"],
        key,
        "42 §3.13's `HumanDecision.actor` is the **ruler**"
    );
    assert_eq!(
        approved.json()["verdict_receipts"], 1,
        "🔴 43 T-5 issues a signed receipt and this process holds **one** — not two. T-4c's was \
         issued in the `gx verify` process and `Engine::open` does not rebuild the trail (the \
         rehydration recovers the *ticket*, which is a function of the id, and cannot recover a \
         signature nothing stored). `.gx/receipts/` is keyed on the transformation and holds one \
         receipt, so a `VerdictReceipt` has nowhere to go — **M6H3-11**, whose window req/38 §50 \
         採(c) put at 「手4 の escalation receipt が出た時」. This is that moment, and the number is \
         the material: {}",
        approved.stdout
    );
    assert_eq!(
        approved.json()["signed_by"].as_str(),
        Some(ruler.as_str()),
        "🔴 43 T-5's receipt is signed with the **ruler's** key. Signing it with the submitter's \
         would attest that the party being ruled on approved themselves, which is the one thing \
         INV-S6 exists to prevent — and at this surface the two are indistinguishable unless the \
         key id is reported and the two keys differ: {}",
        approved.stdout
    );
    assert_eq!(
        approved.json()["ticket"].as_str(),
        Some(ticket.as_str()),
        "🔴 the ticket the ruling was filed against is the one the operator named — the rebuilt \
         ticket of M6H3-10, since this process planned nothing until it resumed"
    );

    // 「以後canonicalize→commitのpipelineが続行可能になる」, through the binary.
    let committed = run(fixture.gx().args(["commit", &tid]));
    println!(
        "AC071_CONTINUES exit={} target_len={}",
        committed.code,
        fixture.target_contents().len()
    );
    assert_eq!(
        committed.code, 0,
        "34 AC-071: 「以後canonicalize→commitのpipelineが続行可能になる」. stdout: {} stderr: {}",
        committed.stdout, committed.stderr
    );
    assert_eq!(
        fixture.target_contents(),
        "after\n",
        "and the change an operator approved is the change that was applied"
    );
}

/// 🔴 **AC-072** — reject by **transformation id** (M6-04 採(c)'s second spelling), and it is terminal.
#[test]
fn ac_072_a_rejected_escalation_is_denied_and_terminal() {
    let (fixture, tid, _ticket) = escalated("m6h4_ac072");
    let ruler = fixture.another_key();

    let rejected = run(fixture
        .gx()
        .args(["escalation", "reject", &tid])
        .args(["--reason", REJECT_REASON])
        .args(["--actor-key", &ruler]));
    println!(
        "AC072_REJECT exit={} state={:?} decision={:?} reason={:?}",
        rejected.code,
        rejected.json()["state"],
        rejected.json()["decision"],
        rejected.json()["reason"]
    );
    assert_eq!(
        rejected.code, 0,
        "44 §1.2: 「0=成功」. stderr: {}",
        rejected.stderr
    );
    assert_eq!(
        rejected.json()["state"],
        "Denied",
        "34 AC-072: 「Tは`Denied`へ遷移し終端となる」"
    );
    assert_eq!(rejected.json()["decision"], "Deny");
    assert_eq!(rejected.json()["reason"], REJECT_REASON);
    assert_eq!(
        rejected.json()["transformation"],
        tid,
        "🔴 M6-04 採(c): 44 §1.2 writes `<TICKET_ID>` and 44 §2.2 writes `{{id}}`; both name one \
         thing and this surface accepts either"
    );

    // 「（record-onlyモード以外ではそれ以上commitへ進めない）」.
    let committed = run(fixture.gx().args(["commit", &tid]));
    println!("AC072_TERMINAL commit_exit={}", committed.code);
    assert_eq!(
        committed.code, 2,
        "44 §1.2 `gx commit`: 「2=Denyで未Admitのため拒否（non-record-onlyかつVerdict≠Admit）」. \
         stdout: {}",
        committed.stdout
    );
    assert_eq!(
        fixture.target_contents().len(),
        oversized_before().len(),
        "and nothing was applied"
    );
}

/// 🔴 A ticket id nothing has raised is 44 §1.2's 「6=未検出（チケット不明）」.
///
/// The status M6-04 made reachable at all: before `Engine::transformation_of_ticket` a
/// `<TICKET_ID>` resolved to nothing **whatever it named**, so 「未検出」 and 「your ticket is fine
/// and this build cannot find it」 were the same answer.
#[test]
fn an_unknown_ticket_is_not_found() {
    let fixture = pipeline("m6h4_ticket_absent", "before\n");
    let absent = "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let key = fixture.key_id.clone();
    let ruled = run(fixture
        .gx()
        .args(["escalation", "approve", absent])
        .args(["--reason", APPROVE_REASON])
        .args(["--actor-key", &key]));
    println!("ESCALATION_ABSENT exit={}", ruled.code);
    assert_eq!(
        ruled.code, 6,
        "44 §1.2: 「6=未検出（チケット不明）」. stderr: {}",
        ruled.stderr
    );
}

/// 🔴 A blank `--reason` is refused before a project is opened (44 §1.2: 「裁定理由（必須）」).
#[test]
fn a_ruling_that_says_nothing_is_refused() {
    let fixture = pipeline("m6h4_reason_blank", "before\n");
    let key = fixture.key_id.clone();
    let refused = run(fixture
        .gx()
        .args([
            "escalation",
            "approve",
            "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ])
        .args(["--reason", "   "])
        .args(["--actor-key", &key]));
    println!("ESCALATION_BLANK_REASON exit={}", refused.code);
    assert_eq!(
        refused.code, 1,
        "規律52: 「入力不正」. AC-071/072 both require the reason to reach the trail, and a refusal \
         that says nothing is a refusal nobody can audit"
    );
}

/// 🔴 **M6H4-6** — the ruler has to be named, although 44 §1.2 writes `--actor-key` optional.
///
/// 42 §3.13's `HumanDecision.actor` is 「the ruler, which is **not** `Transformation.actor` (the
/// submitter)」, and INV-S6 is why an escalation exists at all: it records **who allowed** a change.
/// A default that fell back to the submitter's key would file the ruling under the name of the party
/// the ruling is about.
#[test]
fn the_ruler_has_to_be_named() {
    let fixture = pipeline("m6h4_no_ruler", "before\n");
    let refused = run(fixture
        .gx()
        .args([
            "escalation",
            "approve",
            "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ])
        .args(["--reason", APPROVE_REASON]));
    println!(
        "ESCALATION_NO_RULER exit={} detail={:?}",
        refused.code,
        refused.stderr.trim()
    );
    assert_eq!(refused.code, 1);
    assert!(
        refused.stderr.contains("M6H4-6"),
        "the refusal names the ticket that carries the divergence from 44 §1.2: {}",
        refused.stderr
    );
}
