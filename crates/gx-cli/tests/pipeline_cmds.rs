//! 44 §1.2's pipeline, verb by verb: the draft round-trip, Λ2's journal, the refusals, and the
//! receipt store `gx commit` writes.
//!
//! `ac_054.rs` measures the acceptance criterion and `ac_030_cli.rs` measures identity; this
//! measures the things req/88 §6.2 手 3's DoD names that are neither.

mod support;

use support::{pipeline, run, Pipeline};

/// Drive the whole pipeline and hand back the transformation id.
fn committed(fixture: &Pipeline, goal: &str) -> String {
    let submitted = fixture.submit(goal);
    assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
    let intent = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();
    let planned = run(fixture.gx().args(["plan", &intent]));
    assert_eq!(planned.code, 0, "plan: {}", planned.stderr);
    let tid = planned.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();
    let verified = run(fixture.gx().args(["verify", &tid]));
    assert_eq!(verified.code, 0, "verify: {}", verified.stderr);
    let committed = run(fixture.gx().args(["commit", &tid]));
    assert_eq!(committed.code, 0, "commit: {}", committed.stdout);
    tid
}

/// 🔴 **M6-01 採(a) の実体** — `.gx/drafts/` carries the intent body from one process to the next.
///
/// req/88 §6.2 手 3's DoD: 「`.gx/drafts/` 経由の submit→plan が別 process で通る事」. Hand 1 built the
/// store and could only test it in-process; this is the claim it was built for, and the negative
/// half is what makes it a claim: **delete the draft and `gx plan` cannot run**, because nothing in
/// the system can rebuild an intent body (req/56 §2 gives the directory `Nature::Source` and 「失われ
/// る」 for exactly this reason).
#[test]
fn the_draft_directory_is_what_carries_the_intent_between_processes() {
    let fixture = pipeline("drafts_round_trip", "before\n");
    let submitted = fixture.submit("after\n");
    assert_eq!(submitted.code, 0, "{}", submitted.stderr);
    let intent = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();

    let drafts = fixture.project.join(".gx").join("drafts");
    let filed: Vec<String> = std::fs::read_dir(&drafts)
        .expect("the draft directory exists")
        .filter_map(std::result::Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    println!("DRAFTS_FILED={filed:?} INTENT={intent}");
    assert_eq!(filed.len(), 1, "one submit, one draft: {filed:?}");
    assert!(
        filed[0].starts_with(&intent.replace(':', "_")),
        "the draft is filed under the id the engine minted, not one the CLI chose: {filed:?}"
    );

    // The positive half: another process plans from it.
    let planned = run(fixture.gx().args(["plan", &intent]));
    assert_eq!(planned.code, 0, "{}", planned.stderr);

    // 🔴 The negative half. Take the body away and the transformation is unreachable — which is what
    // `Nature::Source` means and why `Layout::recover` answers `Lost` for this directory rather than
    // `Regenerated`.
    let tid = planned.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();
    for entry in std::fs::read_dir(&drafts).expect("readable") {
        std::fs::remove_file(entry.expect("an entry").path()).expect("remove the draft");
    }
    let orphaned = run(fixture.gx().args(["verify", &tid]));
    println!(
        "VERIFY_WITHOUT_DRAFT={} {}",
        orphaned.code,
        orphaned.stderr.trim()
    );
    assert_eq!(
        orphaned.code, 6,
        "a transformation whose draft is gone is 「未検出」 and not 「内部エラー」: {}",
        orphaned.stderr
    );
}

/// 🔴 **req/88 §3 Λ2, measured** — four processes leave the journal one long-lived engine would.
///
/// > 単発 CLI process の `Σ` は毎回 journal から再構成されるので、「N 回の CLI 実行」と「1 個の長寿命
/// > engine への N 回の呼び出し」は `Σ` について観測等価である
///
/// 43's road from `submit` to `Committed` writes ten records: `DraftCreated`, `Planned`,
/// `VerifyStarted`, `Verdict`, `Canonicalized`, `CommittingStarted`, `ProvenanceDerived`,
/// `InverseEscrowed`, `ApplyStarted`, `Committed`. **Ten is the number this asserts**, and the way a
/// resume could break it is by writing an eleventh: a second `Planned` for a row it rebuilt, or a
/// second `VerifyStarted` for a verdict already recorded. Either would make the equality false in
/// the direction that matters — the CLI claiming the gate was asked twice.
///
/// # 🔴 Where Λ2 still does not hold, said here rather than left to a reader
///
/// The Draft phase. `.gx/drafts/` is state the CLI has and 44 §2.1's `POST /candidates` does not,
/// which is Λ2's own named counter-example and 44 §0's explicit exemption. So **AC-055's 「同一」 is
/// 「同一 from `Candidate` onward」**, and this test is the evidence for the second half of that
/// sentence.
#[test]
fn four_processes_write_the_journal_one_engine_would() {
    let fixture = pipeline("lambda_two", "before\n");
    let tid = committed(&fixture, "after\n");
    let records = fixture.journal_records();
    println!("LAMBDA2_JOURNAL_RECORDS={records} TID={tid}");
    assert_eq!(
        records, 10,
        "43's road from T-1 to T-11 is ten records however many processes drove it"
    );

    // Re-entering every verb changes nothing. `plan` is idempotent by 43 T-2, `commit` returns the
    // `Committed` row by T-9's idempotency column, and `verify` refuses a row that is not a
    // `Candidate` — three different mechanisms, one measurement.
    let replanned = run(fixture.gx().args(["plan", &tid]));
    let recommitted = run(fixture.gx().args(["commit", &tid]));
    let reverified = run(fixture.gx().args(["verify", &tid]));
    println!(
        "LAMBDA2_REENTRY plan={} commit={} verify={} reverified_flag={:?} records={}",
        replanned.code,
        recommitted.code,
        reverified.code,
        reverified.json()["reverified"],
        fixture.journal_records()
    );
    assert_eq!(
        recommitted.code, 0,
        "a re-run commit is 44's 0: {}",
        recommitted.stdout
    );
    assert_eq!(
        recommitted.json()["reentered"],
        true,
        "and it says it did not run the protocol again"
    );
    assert_eq!(
        reverified.json()["reverified"],
        false,
        "43 T-4a's determinism is read from the journal rather than re-evaluated"
    );
    // 🔴 `gx plan` of a committed transformation is **refused**, and that is 43 T-2's from-state
    // (`Draft`) rather than a limitation: the substrate has moved — by this very commit — so the
    // plan that produced this id cannot be reproduced. 43 §8 says the same thing about a committed
    // predecessor.
    assert_ne!(replanned.code, 0, "43 T-2 does not run from `Committed`");
    assert_eq!(
        fixture.journal_records(),
        10,
        "re-entry is not an event; a journal that grew would report re-entry as one"
    );
}

/// 🔴 **M6H2-1 の writer** — `gx commit` puts the receipt in `.gx/receipts/`, and `gx receipt show`
/// finds it.
///
/// req/38 §49: 「`.gx/receipts/`(nature=Source)追認・**writer は `gx commit`(手3)**」. Hand 2 built
/// the store and the four disclosure levels over a fixture; this is the first time the store is
/// filled by the binary, which is what makes 44 §1.2's 「ローカルストア…から`Receipt`を取得し表示」
/// true of a real run.
#[test]
fn commit_is_what_fills_the_receipt_store() {
    let fixture = pipeline("receipt_writer", "before\n");
    let receipts = fixture.project.join(".gx").join("receipts");
    assert!(
        std::fs::read_dir(&receipts).map(|d| d.count()).unwrap_or(0) == 0,
        "the store starts empty"
    );

    let tid = committed(&fixture, "after\n");
    let filed: Vec<String> = std::fs::read_dir(&receipts)
        .expect("the store exists")
        .filter_map(std::result::Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    let shown = run(fixture.gx().args(["receipt", "show", &tid]));
    let level_four = run(fixture.gx().args(["receipt", "show", &tid, "--level", "4"]));
    println!(
        "RECEIPTS_FILED={filed:?} SHOW_RC={} KIND={:?} L4_SIGNATURE_KEYID={:?}",
        shown.code,
        shown.json()["receipt_kind"],
        level_four.json()["signature"]["keyid"]
    );
    // 🔴 **M6H4-7** (M6 hand 5): `<TID>.<kind>.json`, and a committed transformation leaves **two**
    // documents rather than one — ASM-14's `VerdictReceipt` from 43 T-4a and 43 T-11's
    // `CommitReceipt`. Under the old untagged name they shared one slot and the second writer won,
    // which is 「what was decided」 being erased by 「what was applied」.
    let stem = tid.replace(':', "_");
    assert_eq!(
        filed.len(),
        2,
        "one commit, one verdict receipt and one commit receipt: {filed:?}"
    );
    assert!(
        filed.contains(&format!("{stem}.commit.json"))
            && filed.contains(&format!("{stem}.verdict.json")),
        "M6H4-7's two kinds, each under its own name: {filed:?}"
    );
    assert_eq!(shown.code, 0, "`gx receipt show`: {}", shown.stderr);
    assert_eq!(shown.json()["receipt_kind"], "CommitReceipt");
    assert_eq!(shown.json()["verdict"], "Admit");
    assert_eq!(
        level_four.json()["signature"]["keyid"],
        fixture.key_id,
        "M6-22: level 4 asks for the signature the payload's `key_id` names, and this receipt was \
         signed by the actor's key (M6H3-4)"
    );
}

/// 🔴 **M6H3-5** — `--order` and `--parent` are refused rather than ignored.
///
/// 44 §1.2's synopsis has both and v0.1 has nowhere to put either: 42 §3.3's `Intent` is five
/// fields, and `plan` writes `order = 0` with an empty parents list for every transformation of an
/// object. An operator who typed a value and got a Draft that did not carry it would have been lied
/// to by a flag the specification told them about (M4H5-5's rule about arguments that change
/// nothing).
#[test]
fn the_flags_with_nowhere_to_go_are_refused_and_not_ignored() {
    let fixture = pipeline("nowhere_flags", "before\n");
    let goal = fixture.project.join("goal.txt");
    std::fs::write(&goal, "after\n").expect("write the goal");

    let base = |extra: &[&str]| {
        let mut cmd = fixture.gx();
        cmd.arg("submit")
            .args(["--substrate", "fs"])
            .arg("--locator")
            .arg(&fixture.target)
            .arg("--intent")
            .arg(&goal)
            .args(["--context", "Evidence"])
            .args(["--actor-key", &fixture.key_id])
            .args(extra);
        run(&mut cmd)
    };

    let order = base(&["--order", "2"]);
    let parent = base(&[
        "--parent",
        "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ]);
    let plain = base(&[]);
    println!(
        "NOWHERE_FLAGS order={} parent={} plain={} order_detail={:?}",
        order.code,
        parent.code,
        plain.code,
        order.stderr.trim()
    );
    assert_eq!(order.code, 1, "44 §1.4's 1: {}", order.stderr);
    assert_eq!(parent.code, 1, "44 §1.4's 1: {}", parent.stderr);
    assert_eq!(plain.code, 0, "and the same command without them works");
    assert!(
        order.stderr.contains("M6H3-5"),
        "the refusal names the ticket, so an operator can find out whether it is still true"
    );
    // 🔴 The refusal happened **before** anything was written: a flag that refuses after creating a
    // draft would leave a Draft nobody asked for.
    assert_eq!(
        fixture.journal_records(),
        1,
        "only the successful submit reached the journal"
    );
}

/// 🔴 **The world moving between two processes** is refused by name, not by a wrong answer.
///
/// A resume re-plans, and a re-plan consults the substrate. If the file changed since `gx plan`, the
/// recomputed `TransformationId` is a different one — 43 §8's own situation seen from the CLI. What
/// must not happen is verifying **the new one** while the operator asked about the old: that would
/// answer a question nobody asked, with a receipt to match.
#[test]
fn a_transformation_whose_substrate_moved_is_refused_by_name() {
    let fixture = pipeline("world_moved", "before\n");
    let submitted = fixture.submit("after\n");
    let intent = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();
    let planned = run(fixture.gx().args(["plan", &intent]));
    let tid = planned.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();

    std::fs::write(&fixture.target, "somebody else was here\n").expect("move the world");
    let verified = run(fixture.gx().args(["verify", &tid]));
    println!(
        "WORLD_MOVED verify={} detail={:?} records={}",
        verified.code,
        verified.stderr.trim(),
        fixture.journal_records()
    );
    assert_ne!(
        verified.code, 0,
        "the transformation named is not plannable now"
    );
    assert!(
        verified.stderr.contains("43 §8"),
        "and the refusal says which rule it is: {}",
        verified.stderr
    );
}

/// `--idempotency-key` is echoed, derived when absent, and does not change the outcome.
///
/// 44 §1.2: 「未指定時はCLIが`transformation_id`から決定的に導出（同一実行の再試行は自然に冪等）」.
/// The cache 44 §2.4 describes is **hand 5's** (M6-11), and what makes a repeated `gx commit` safe
/// today is 43 T-11's own idempotence. Saying so beside the flag is the difference between a flag
/// that works and one that looks like it does.
#[test]
fn the_idempotency_key_is_derived_when_it_is_not_given() {
    let fixture = pipeline("idempotency", "before\n");
    let tid = committed(&fixture, "after\n");
    let again = run(fixture.gx().args(["commit", &tid]));
    let explicit =
        run(fixture
            .gx()
            .args(["commit", &tid, "--idempotency-key", "chosen-by-a-script"]));
    println!(
        "IDEMPOTENCY derived={:?} explicit={:?} rc={} {}",
        again.json()["idempotency_key"],
        explicit.json()["idempotency_key"],
        again.code,
        explicit.code
    );
    assert_eq!(
        again.json()["idempotency_key"],
        format!("gx-commit:{tid}"),
        "the derivation is from the transformation id, deterministically"
    );
    assert_eq!(explicit.json()["idempotency_key"], "chosen-by-a-script");
    assert_eq!(again.code, 0);
    assert_eq!(explicit.code, 0);
}

/// `gx verify --evidence` reads 42 §3.7 values as JSONL, and a line that is not one is refused.
///
/// 44 §1.2: 「事前収集済み`Evidence`（42 §3.7）をJSONLで追加投入」. Refusing a half-readable file
/// matters because `InjectedEvidence::none` already means 「省略時はgx-gate組込の…評価のみ」: a
/// collector file that decoded partially would put 「we collected nothing」 and 「we collected some of
/// it」 under one face.
#[test]
fn evidence_is_jsonl_and_a_bad_line_is_refused() {
    let fixture = pipeline("evidence_jsonl", "before\n");
    let submitted = fixture.submit("after\n");
    let intent = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();
    let planned = run(fixture.gx().args(["plan", &intent]));
    let tid = planned.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();

    let good = fixture.project.join("evidence.jsonl");
    std::fs::write(
        &good,
        "{\"TestResult\":{\"case\":\"t1\",\"suite\":\"s\",\"outcome\":\"Pass\",\"log_digest\":null,\"duration_ms\":3}}\n\n",
    )
    .expect("write the evidence");
    let bad = fixture.project.join("evidence-bad.jsonl");
    std::fs::write(&bad, "{\"NotAnEvidence\":true}\n").expect("write the evidence");

    let with_evidence = run(fixture
        .gx()
        .args(["verify", &tid])
        .arg("--evidence")
        .arg(&good));
    println!(
        "EVIDENCE ok={} kind={:?}",
        with_evidence.code,
        with_evidence.json()["kind"]
    );
    assert_eq!(with_evidence.code, 0, "{}", with_evidence.stderr);

    let refused = run(fixture
        .gx()
        .args(["verify", &tid])
        .arg("--evidence")
        .arg(&bad));
    println!("EVIDENCE_BAD={} {}", refused.code, refused.stderr.trim());
    assert_eq!(refused.code, 1, "44 §1.4's 1: {}", refused.stderr);
    assert!(
        refused.stderr.contains("evidence-bad.jsonl:1"),
        "and it names the line: {}",
        refused.stderr
    );
}
