//! 🔴 The CLI-side claims **M6 hand 6** owes, measured through the binary.
//!
//! This hand's centre of gravity is `GET /stream` and `gx serve`'s runtime, and four of its rulings
//! land on 44 §1's side. All four are red before the implementation and are therefore the part of
//! this hand that is red-first in the T-27 sense; the HTTP half is not, for the formal reason hand 5
//! recorded (a test client cannot call a router that does not compile) and §13 of the report says so
//! rather than dressing it up.
//!
//! * **E-M6-14** (req/38 §51 M6H4-2 採(a)) — 「`gx draft discard <IntentId>` を 44 §1.1/§1.2 に足す+
//!   『draft 破棄は台帳に載らない操作』を doc に 1 行。実装窓=手6」. Hand 4 raised it when E-M6-1 took
//!   `Draft` out of `gx cancel`'s from-set: a draft has no `TransformationId`, no row in the state
//!   table and no journal record that could carry `Aborted`, so the only honest verb for it is one
//!   that says it is **not** a cancellation.
//! * **M6H5-13** (req/38 §52 採(a)) — 「手6 DoD に『`gx serve --help` が `ABSENCE_NOTICE` を render』
//!   を載せる」. Hand 5 wrote the sentence as a constant and could not print it: the hand with the
//!   flag is this one.
//! * **M6-10 採(b)'s bind policy**, promoted from a string to a refusal. Hand 5 wrote
//!   `gx_api::auth::bind_refusal` and left the enforcement to 「the hand which writes the flag」.
//! * **N-09 / 44 §2.5's v0.2 row** — `--tls-cert`/`--tls-key` are in 44 §1.2's synopsis and mTLS is
//!   ruled out of v0.1. An accepted flag that did nothing would make an operator believe the socket
//!   is encrypted (M4H5-5: 「引数が不正を適用失敗と綴るな」, and its converse).

mod support;

use support::{pipeline, run};

// ---------------------------------------------------------------------------
// E-M6-14 — `gx draft discard`
// ---------------------------------------------------------------------------

/// 🔴 **E-M6-14** — a draft is discarded, and the ledger does not learn about it.
///
/// The two halves are one probe on purpose. 「draft 破棄は台帳に載らない操作」 is the sentence the
/// ruling asks for in the documentation, and a sentence nobody measures is a sentence: the journal's
/// record count before and after is what makes it a fact. E-M6-1 is why the verb exists at all —
/// `gx cancel` refuses a draft because 43 T-7's from-set no longer contains `Draft`, and a user who
/// submitted an intent they no longer want had no way to put it down.
#[test]
fn e_m6_14_discarding_a_draft_removes_the_body_and_writes_no_record() {
    let fixture = pipeline("m6h6_draft_discard", "before\n");
    let submitted = fixture.submit("after\n");
    assert_eq!(submitted.code, 0, "stderr: {}", submitted.stderr);
    let intent = submitted.json()["intent_id"]
        .as_str()
        .expect("44 §1.2: `gx submit` names the intent")
        .to_string();

    let drafts = fixture.project.join(".gx").join("drafts");
    let before_files = std::fs::read_dir(&drafts).expect("the draft store").count();
    let before_records = fixture.journal_records();

    let discarded = run(fixture.gx().args(["draft", "discard", &intent]));
    println!(
        "E_M6_14_DISCARD exit={} stdout={} stderr={}",
        discarded.code,
        discarded.stdout.trim(),
        discarded.stderr.trim()
    );
    assert_eq!(
        discarded.code, 0,
        "E-M6-14: discarding a draft the project holds is a completed operation. stderr: {}",
        discarded.stderr
    );

    let after_files = std::fs::read_dir(&drafts).expect("the draft store").count();
    let after_records = fixture.journal_records();
    println!(
        "E_M6_14_DRAFTS before={before_files} after={after_files} \
         JOURNAL before={before_records} after={after_records}"
    );
    assert_eq!(
        after_files,
        before_files - 1,
        "the body 44 §1.2's `gx plan` would have read is gone"
    );
    assert_eq!(
        after_records, before_records,
        "🔴 「draft 破棄は台帳に載らない操作」 (E-M6-14). M5H6-1 refused a fourteenth journal record \
         for owner-cancelled drafts on the grounds that the vocabulary would grow with nothing to \
         protect, and this is the number that says the refusal held"
    );
}

/// A draft nobody submitted is **6**, not 0 and not 1.
///
/// 44 §1.4's 6 is 「未検出（not-found）」 and this is the case it names. Answering 0 would make
/// `gx draft discard` report success for a name the project never held, which is the shape 「skip と
/// pass を同じ顔にするな」 (req/29 §4) forbids one layer up.
#[test]
fn e_m6_14_discarding_a_draft_that_is_not_there_is_not_found() {
    let fixture = pipeline("m6h6_draft_discard_absent", "before\n");
    let submitted = fixture.submit("after\n");
    let intent = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();
    let first = run(fixture.gx().args(["draft", "discard", &intent]));
    assert_eq!(first.code, 0, "stderr: {}", first.stderr);

    let again = run(fixture.gx().args(["draft", "discard", &intent]));
    println!(
        "E_M6_14_DISCARD_AGAIN exit={} stderr={}",
        again.code,
        again.stderr.trim()
    );
    assert_eq!(
        again.code, 6,
        "44 §1.4: 6 is 「未検出」. stderr: {}",
        again.stderr
    );
}

// ---------------------------------------------------------------------------
// M6H5-13 — the absence, rendered where the operator is
// ---------------------------------------------------------------------------

/// 🔴 **M6H5-13** — `gx serve --help` prints hand 5's `ABSENCE_NOTICE`.
///
/// 卓-5's form is that 「検査の不在を隠さない」 means the absence is **said** where the operator is,
/// not only where the reviewer is. Hand 5 could write the sentence and not print it; this is the
/// print. The assertion is on the words rather than on the whole constant so that a re-wording keeps
/// the probe honest about what it is checking: the three facts (a single static Bearer, no
/// authorization layer, a loopback default) have to reach the terminal.
#[test]
fn m6h5_13_serve_help_renders_the_absence_notice() {
    let fixture = pipeline("m6h6_serve_help", "before\n");
    let helped = run(fixture.gx().args(["serve", "--help"]));
    println!(
        "M6H5_13_HELP exit={} bytes={}",
        helped.code,
        helped.stdout.len()
    );
    assert_eq!(
        helped.code, 0,
        "規律52 (E-M6-2): an explicit `--help` is 0. stderr: {}",
        helped.stderr
    );
    for phrase in [
        "Bearer",
        "no authorization",
        "loopback",
        "127.0.0.1",
        "cancel",
    ] {
        assert!(
            helped.stdout.contains(phrase),
            "M6H5-13: 「`gx serve --help` が ABSENCE_NOTICE を render」 — {phrase:?} is missing from \
             the help text:\n{}",
            helped.stdout
        );
    }
}

// ---------------------------------------------------------------------------
// M6-10 採(b) — the bind policy, as a refusal rather than a string
// ---------------------------------------------------------------------------

/// 🔴 A non-loopback `--bind` is refused without the explicit flag.
///
/// req/88 M6-10 named the consequence of 44 §1.2 not writing a default: 「未定義のまま実装すると
/// `0.0.0.0` が既定になりうる(authorization 無しで公開 network に出る)」. v0.1 has no authorization
/// layer at all, so a surface on a public interface is a surface anyone on the network can `cancel`
/// through. The refusal is 44 §1.4's 1 (「起動失敗」 in §1.2's gloss).
#[test]
fn m6_10_a_public_bind_is_refused() {
    let fixture = pipeline("m6h6_bind_refusal", "before\n");
    let refused = run(fixture.gx().args(["serve", "--bind", "0.0.0.0:8787"]));
    println!(
        "M6_10_BIND exit={} stderr={}",
        refused.code,
        refused.stderr.trim()
    );
    assert_eq!(
        refused.code, 1,
        "44 §1.2's `gx serve` exit column: 1 = 起動失敗. stderr: {}",
        refused.stderr
    );
    assert!(
        refused.stderr.contains("loopback"),
        "the refusal has to say what it wants: {}",
        refused.stderr
    );
    assert!(
        refused.stdout.trim().is_empty(),
        "44 §1.3: a refusal leaves stdout empty"
    );
}

// ---------------------------------------------------------------------------
// N-09 — a flag 44 lists and v0.1 does not implement
// ---------------------------------------------------------------------------

/// 🔴 `--tls-cert` is **refused**, not ignored.
///
/// 44 §1.2's synopsis carries `[--tls-cert <PATH> --tls-key <PATH>]` and req/88 §1 N-09 keeps mTLS
/// out of v0.1 (44 §2.5: 「v0.2（予告）: mTLS」). The two together leave a flag with no implementation,
/// and the dangerous form is the silent one: an operator who passes `--tls-cert` and is answered
/// with a running server believes the socket is encrypted. E-M6-8 settled the shape for exactly this
/// case on `--order`/`--parent` — refuse, and read the synopsis back in the refusal.
#[test]
fn n_09_tls_flags_are_refused_rather_than_ignored() {
    let fixture = pipeline("m6h6_tls_refusal", "before\n");
    let cert = fixture.project.join("cert.pem");
    std::fs::write(&cert, "not a certificate").expect("write the file");
    let refused = run(fixture.gx().args([
        "serve",
        "--tls-cert",
        &cert.display().to_string(),
        "--tls-key",
        &cert.display().to_string(),
    ]));
    println!(
        "N_09_TLS exit={} stderr={}",
        refused.code,
        refused.stderr.trim()
    );
    assert_eq!(
        refused.code, 1,
        "a flag with no implementation is a start-up failure and not a running server. stderr: {}",
        refused.stderr
    );
    assert!(
        refused.stderr.contains("TLS") || refused.stderr.contains("tls"),
        "the refusal names the flag it refuses: {}",
        refused.stderr
    );
}
