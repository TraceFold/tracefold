//! 🔴 **DR-2's other half, through the binary** — the E2E hand 3 could not write (M6H3-9).
//!
//! req/38 §50 M6H3-9 採(a): 「書込可能 path を deny する **test 用 policy pack fixture**+
//! `gx verify --policy <PATH>`(44 拡張=E-M6-12)。**record-only E2E の完成は手4 の DoD へ**」.
//!
//! # What hand 3 measured, and where it had to stop
//!
//! 44 §1.2's `gx commit` carries a **normative** paragraph:
//!
//! > **[DR-2感度]** record-onlyモード下でVerdict=Denyの対象を`commit`した場合、適用は通すが
//! > `Receipt.enforced=false`が刻まれ、**exit codeは0**
//!
//! Hand 3 measured the enforcing half through the binary — `gx verify` exits 2, `gx commit` exits 2,
//! and the substrate is byte-identical before and after — and measured T-8r **inside the engine**,
//! writing down why: the shipped pack's one forbid is `/etc`, and under `EnforcementMode::RecordOnly`
//! the commit walks on to `apply`, which on that locator is a write to `/etc`. A suite run as root
//! would have broken the machine.
//!
//! With the fixture pack the denied locator is a file in a temporary directory, so the whole of DR-2
//! can be stated as one comparison: **the same transformation, the same pack, two enforcement
//! postures, two different worlds**.

mod support;

use support::{deny_writable_pack, pipeline_named, run, DENIED_FRAGMENT};

/// 🔴 The **enforcing** posture: exit 2, and the file does not move.
#[test]
fn enforced_a_denied_change_is_refused_and_nothing_is_written() {
    let fixture = pipeline_named(
        "m6h4_dr2_enforce",
        "before\n",
        &format!("{DENIED_FRAGMENT}.txt"),
    );
    let tid = fixture.planned_one("after\n");
    let pack = deny_writable_pack();

    let verified = run(fixture
        .gx()
        .args(["verify", &tid])
        .arg("--policy")
        .arg(&pack));
    let committed = run(fixture.gx().args(["commit", &tid]));
    println!(
        "DR2_ENFORCE verify={} commit={} kind={:?} enforced={:?} target={:?}",
        verified.code,
        committed.code,
        verified.json()["kind"],
        verified.json()["enforced"],
        fixture.target_contents()
    );
    assert_eq!(
        verified.code, 2,
        "44 §1.2: 「2=Deny」. stderr: {}",
        verified.stderr
    );
    assert_eq!(verified.json()["kind"], "Deny");
    assert_eq!(
        committed.code, 2,
        "44 §1.2: 「2=Denyで未Admitのため拒否（non-record-onlyかつVerdict≠Admit）」: {}",
        committed.stdout
    );
    assert_eq!(
        fixture.target_contents(),
        "before\n",
        "T-8 refuses a `Denied` row before `apply` is reached"
    );
}

/// 🔴 **The E2E M6H3-9 asked for** — record-only, exit **0**, `enforced=false`, and the world moves.
///
/// Three facts, and each one of them fails a different plausible wrong implementation:
///
/// * **exit 0** — 44's [DR-2感度] paragraph says so in as many words. An implementation that read
///   `Denied` and refused would make the paragraph name an outcome no `gx` invocation can produce;
/// * **the file moved** — DR-2 is 「適用は**通す**」, so a record-only mode that merely logged and
///   refused would be the enforcing posture wearing a different name;
/// * **`enforced=false` on the receipt** — INV-S5's whole point: 「適用は通ったが、ポリシー上は拒否
///   されていた」 has to survive as a **third-party-verifiable** fact, not as a log line.
#[test]
fn record_only_a_denied_change_is_applied_and_the_receipt_says_it_was_not_enforced() {
    let fixture = pipeline_named(
        "m6h4_dr2_record_only",
        "before\n",
        &format!("{DENIED_FRAGMENT}.txt"),
    );
    let tid = fixture.planned_one("after\n");
    let pack = deny_writable_pack();

    let verified = run(fixture
        .gx()
        .args(["verify", &tid, "--record-only"])
        .arg("--policy")
        .arg(&pack));
    println!(
        "DR2_RECORD_ONLY verify={} kind={:?} state={:?} record_only={:?}",
        verified.code,
        verified.json()["kind"],
        verified.json()["state"],
        verified.json()["record_only"]
    );
    // The verdict is still `Deny` — record-only is not a permission, it is a posture about what to
    // do with a refusal (43 §4: 「`FailPosture`と`EnforcementMode`は独立設定軸」).
    assert_eq!(verified.json()["kind"], "Deny");
    assert_eq!(verified.json()["record_only"], true);

    let committed = run(fixture.gx().args(["commit", &tid, "--record-only"]));
    println!(
        "DR2_RECORD_ONLY commit={} state={:?} enforced={:?} target={:?}",
        committed.code,
        committed.json()["state"],
        committed.json()["enforced"],
        fixture.target_contents()
    );
    assert_eq!(
        committed.code, 0,
        "🔴 44 §1.2 [DR-2感度]: 「適用は通すが`Receipt.enforced=false`が刻まれ、**exit codeは0**」. \
         stdout: {} stderr: {}",
        committed.stdout, committed.stderr
    );
    assert_eq!(committed.json()["state"], "Committed");
    assert_eq!(
        committed.json()["enforced"],
        false,
        "INV-S5: an `enforced=false` Committed has to be distinguishable, and the receipt is where \
         a third party reads it"
    );
    assert_eq!(
        fixture.target_contents(),
        "after\n",
        "「適用は通す」 — a record-only mode that refused would be the enforcing posture renamed"
    );

    // 🔴 And the fact reaches the **stored** receipt, which is what a third party is handed.
    let shown = run(fixture.gx().args(["receipt", "show", &tid, "--json"]));
    println!(
        "DR2_RECEIPT exit={} enforced={:?}",
        shown.code,
        shown.json()["payload"]["enforced"]
    );
    assert_eq!(shown.code, 0, "stderr: {}", shown.stderr);
    assert_eq!(
        shown.json()["payload"]["enforced"],
        false,
        "42 §3.10 puts `enforced` on the payload, inside the signature: {}",
        shown.stdout
    );
}
