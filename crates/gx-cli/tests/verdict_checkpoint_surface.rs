// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-P3-10 / AC-P3-11** — FR-M04 through a surface, and the five judgements it has to keep.
//!
//! 32 §D verbatim: "**there are currently zero surfaces**—`Engine::verdict_checkpoint` was only placed as an API" (sem: SEM-gx-cli-1604). `req/119`
//! §4 is the requirement definition and `req/38` §71 ruling ⑤ is the name. What this file measures is
//! that the **CLI** produces and checks a chain, and that the judgements `crates/gx-engine/tests/
//! ac_vc.rs` makes against the engine API come out the same through a command line.
//!
//! # 🔴 What parity means here, and what it cannot mean
//!
//! AC-VC-1's own limit (sem: SEM-gx-cli-1605) is declared in its suite: "across a restart the producer's counter is rebuilt
//! from the journal too, so the two roads share a source there". A surface does not change that,
//! and `gx verdict-checkpoint verify --recount-from-journal` therefore prints the same sentence in
//! `not_detected` rather than claiming a stronger check than the engine's. Parity is "the same
//! answer" (sem: SEM-gx-cli-1606), not "a better one because it went through a pipe".

mod support;

use support::{pipeline, run, Pipeline, Run};

/// One transformation, all the way to `Committed`.
fn commit_one(fixture: &Pipeline, goal: &str) -> String {
    let submitted = fixture.submit(goal);
    assert_eq!(submitted.code, 0, "{}", submitted.stderr);
    let intent = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();
    let planned = run(fixture.gx().args(["plan", &intent]));
    assert_eq!(planned.code, 0, "{}", planned.stderr);
    let tid = planned.json()["transformation"]["id"]
        .as_str()
        .expect("44 §1.2's plan answers with the transformation")
        .to_string();
    let verified = run(fixture.gx().args(["verify", &tid]));
    assert_eq!(verified.code, 0, "{}", verified.stderr);
    let committed = run(fixture.gx().args(["commit", &tid]));
    assert_eq!(committed.code, 0, "{}", committed.stderr);
    tid
}

/// The key pair `gx key gen` put in this fixture's store — the one that signs and the one a
/// verifier is handed the public half of.
fn signing_pair(fixture: &Pipeline) -> gx_witness::KeyPair {
    gx_witness::KeyPair::load(secret_key(fixture)).expect("the fixture's own key loads")
}

/// Where req/56 §3 puts it.
fn secret_key(fixture: &Pipeline) -> std::path::PathBuf {
    fixture
        .home
        .join(".gx")
        .join("keys")
        .join(format!("{}.key", fixture.key_id))
}

fn issue(fixture: &Pipeline, out: &str) -> Run {
    let key = secret_key(fixture);
    run(fixture
        .gx()
        .args(["verdict-checkpoint", "issue", "--key"])
        .arg(&key)
        .arg("--out")
        .arg(fixture.project.join(out)))
}

/// AC-P3-10: issue then verify is exit 0, and one changed bit is exit 7.
#[test]
fn ac_p3_10_issue_then_verify_and_one_changed_count_is_refused() {
    let fixture = pipeline("vc_surface_issue", "before\n");
    commit_one(&fixture, "after\n");

    let issued = issue(&fixture, "vc.json");
    println!("ISSUE code={} {}", issued.code, issued.stdout.trim());
    assert_eq!(issued.code, 0, "{}", issued.stderr);
    let document = issued.json();
    assert_eq!(document["origin"], gx_cli::verdict::DEFAULT_VERDICT_ORIGIN);
    assert_eq!(
        document["tally"]["admit"], 1,
        "one Admit was issued in this window: {document}"
    );

    // 🔴 What a **verifier** holds is the public half in 44 §1.2's `{key_id, public_key}` shape, and
    // never the key file: `KeyPair::load` refuses a secret whose permissions are not 0600 (req/56
    // §3), which is right and which also means a suite that handed one to a verifier would be
    // measuring a permission check instead of a signature. The document is written from the pair
    // the store holds.
    let pub_path = support::write_public_key(&fixture.project, &signing_pair(&fixture));

    let verified = run(fixture
        .gx()
        .args(["verdict-checkpoint", "verify"])
        .arg(fixture.project.join("vc.json"))
        .arg("--key")
        .arg(&pub_path)
        .arg("--recount-from-journal"));
    println!("VERIFY code={} {}", verified.code, verified.stdout.trim());
    assert_eq!(verified.code, 0, "{}", verified.stderr);
    let answer = verified.json();
    assert_eq!(answer["valid"], true);
    assert_eq!(answer["checks"]["signature"], true);
    assert_eq!(answer["checks"]["contiguity"], true);
    assert_eq!(answer["checks"]["recount"], true);
    assert!(
        answer["not_detected"]
            .as_array()
            .is_some_and(|limits| limits.len() >= 3),
        "every run prints what a valid answer does not mean (ruling #3 / ruling #14 (sem: SEM-gx-cli-1607) / AC-VC-1's own \
         limit): {answer}"
    );

    // 🔴 AC-VC-3 through the surface: a changed count, and the signature refuses it.
    let mut tampered = document.clone();
    tampered["tally"]["deny"] = serde_json::json!(1);
    let tampered_path = fixture.project.join("vc_tampered.json");
    std::fs::write(
        &tampered_path,
        serde_json::to_vec_pretty(&tampered).expect("serialises"),
    )
    .expect("write");
    let refused = run(fixture
        .gx()
        .args(["verdict-checkpoint", "verify"])
        .arg(&tampered_path)
        .arg("--key")
        .arg(&pub_path));
    println!("TAMPERED code={} {}", refused.code, refused.stdout.trim());
    assert_eq!(
        refused.code, 7,
        "44 §1.2's 7 is \"invalid\" (sem: SEM-gx-cli-1608), the number `gx receipt verify` already uses: {}",
        refused.stderr
    );
    assert_eq!(refused.json()["valid"], false);
    assert_eq!(refused.json()["checks"]["signature"], false);
}

/// 🔴 AC-VC-2 through the surface: **under-reporting is caught by the recount, not by the key.**
///
/// The operator holds the key, so a smaller number can be signed. What cannot be signed away is the
/// journal, and this is the arm that proves the surface consults it: the signature check is
/// **skipped** (no `--key`), and the answer is still invalid because the recount saw more.
#[test]
fn ac_p3_11_under_reporting_is_caught_by_the_recount_with_no_key_at_all() {
    let fixture = pipeline("vc_surface_underreport", "before\n");
    commit_one(&fixture, "after\n");
    let issued = issue(&fixture, "vc.json");
    assert_eq!(issued.code, 0, "{}", issued.stderr);

    let mut smaller = issued.json();
    smaller["tally"]["admit"] = serde_json::json!(0);
    smaller["window_end"] = serde_json::json!(0);
    let path = fixture.project.join("vc_smaller.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&smaller).expect("serialises"),
    )
    .expect("write");

    let verified = run(fixture
        .gx()
        .args(["verdict-checkpoint", "verify"])
        .arg(&path)
        .arg("--recount-from-journal"));
    println!(
        "UNDERREPORT code={} {}",
        verified.code,
        verified.stdout.trim()
    );
    let answer = verified.json();
    assert_eq!(verified.code, 7, "{}", verified.stderr);
    assert_eq!(answer["valid"], false);
    assert_eq!(
        answer["checks"]["signature"], "skipped",
        "no key was given, and a skip is its own word rather than a pass (req/29 §4): {answer}"
    );
    assert_eq!(answer["checks"]["recount"], false);
    let findings = answer["findings"].as_array().expect("findings");
    assert!(
        findings
            .iter()
            .any(|f| f.as_str().is_some_and(|s| s.contains("recount:"))),
        "the finding names what the verifier counted and what the chain admits to: {findings:?}"
    );
}

/// 🔴 AC-VC-5 through the surface: a chain that stopped publishing is visible against the ledger.
///
/// Every ledger leaf is downstream of an admission, so a chain admitting to fewer admissions than
/// the ledger holds leaves has stopped. The surface reads the ledger's size from a **signed head**
/// (`gx log checkpoint`) rather than from the operator's word, which is what `--ledger-checkpoint`
/// is for.
#[test]
fn ac_p3_11_a_chain_that_stopped_publishing_is_visible_against_a_signed_head() {
    let fixture = pipeline("vc_surface_behind", "before\n");
    commit_one(&fixture, "after\n");
    let issued = issue(&fixture, "vc.json");
    assert_eq!(issued.code, 0, "{}", issued.stderr);

    // A second commit, and **no** second checkpoint: the chain now describes less than the ledger.
    commit_one(&fixture, "after-again\n");
    let key = secret_key(&fixture);
    let head = run(fixture
        .gx()
        .args(["log", "checkpoint", "--key"])
        .arg(&key)
        .arg("--out")
        .arg(fixture.project.join("head.json")));
    assert_eq!(head.code, 0, "{}", head.stderr);

    let verified = run(fixture
        .gx()
        .args(["verdict-checkpoint", "verify"])
        .arg(fixture.project.join("vc.json"))
        .arg("--ledger-checkpoint")
        .arg(fixture.project.join("head.json")));
    println!("BEHIND code={} {}", verified.code, verified.stdout.trim());
    let answer = verified.json();
    assert_eq!(verified.code, 7, "{}", verified.stderr);
    assert_eq!(answer["checks"]["ledger_binding"], false);
    assert!(
        answer["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|f| f.as_str().is_some_and(|s| s.contains("ledger_binding:"))),
        "and the finding carries both numbers: {answer}"
    );
}

/// 🔴 **AC-VC-4 through the surface** (P3.1 repair item ⑤, `req/38` §74 riding along; sem: SEM-gx-cli-1609, `req/120` §5/§8's
/// residual: "AC-VC-4's surface re-run"). `crates/gx-engine/tests/ac_vc.rs::ac_vc_4_...` proves the
/// property against the engine API directly (a `Checkpoint`'s signature swapped onto a
/// `VerdictCheckpoint` fails to verify, and the reverse); this is the same swap against the two
/// **documents `gx` actually writes to disk** -- `gx verdict-checkpoint issue`'s output and
/// `gx log checkpoint`'s -- checked with the one verb an operator holding a suspicious file would
/// reach for, `gx verdict-checkpoint verify`.
#[test]
fn ac_p3_11_ac_vc_4_a_ledger_heads_signature_does_not_verify_a_verdict_checkpoint() {
    let fixture = pipeline("vc_surface_acvc4", "before\n");
    commit_one(&fixture, "after\n");

    let issued = issue(&fixture, "vc.json");
    assert_eq!(issued.code, 0, "{}", issued.stderr);
    let verdict_checkpoint = issued.json();

    let key = secret_key(&fixture);
    let head = run(fixture
        .gx()
        .args(["log", "checkpoint", "--key"])
        .arg(&key)
        .arg("--out")
        .arg(fixture.project.join("head.json")));
    assert_eq!(head.code, 0, "{}", head.stderr);
    let ledger_head = head.json();

    // The swap `ac_vc_4` makes against the two Rust values, made here against the two documents:
    // both carry a `signature` field of the same shape (one key, one DSSE envelope), and nothing
    // else about either JSON object changes.
    let mut wearing_the_heads_signature = verdict_checkpoint.clone();
    wearing_the_heads_signature["signature"] = ledger_head["signature"].clone();
    let tampered_path = fixture.project.join("vc_wearing_head_signature.json");
    std::fs::write(
        &tampered_path,
        serde_json::to_vec_pretty(&wearing_the_heads_signature).expect("serialises"),
    )
    .expect("write");

    let pub_path = support::write_public_key(&fixture.project, &signing_pair(&fixture));
    let verified = run(fixture
        .gx()
        .args(["verdict-checkpoint", "verify"])
        .arg(&tampered_path)
        .arg("--key")
        .arg(&pub_path));
    println!(
        "ACVC4_SURFACE code={} {}",
        verified.code,
        verified.stdout.trim()
    );
    assert_eq!(
        verified.code, 7,
        "44 §1.2's 7 is \"invalid\" (sem: SEM-gx-cli-1610) -- a real signature from the same key, over the wrong frame: {}",
        verified.stderr
    );
    let answer = verified.json();
    assert_eq!(answer["valid"], false);
    assert_eq!(
        answer["checks"]["signature"], false,
        "the length-prefixed payload type inside the signed bytes (E-M2-26) is what fails here, \
         not a key mismatch -- both documents are signed by {}",
        fixture.key_id
    );
}

/// The two surfaces name one namespace.
///
/// `crates/gx-cli/tests/ac_055.rs` compares the **ledger** origin across the same boundary for the
/// same reason: two crates that cannot see each other spell a constant twice, and a chain issued
/// through one surface has to fold with a chain issued through the other.
#[test]
fn the_cli_and_http_surfaces_spell_one_origin() {
    println!(
        "CLI={} HTTP={}",
        gx_cli::verdict::DEFAULT_VERDICT_ORIGIN,
        gx_api::verdict_checkpoints::DEFAULT_VERDICT_ORIGIN
    );
    assert_eq!(
        gx_cli::verdict::DEFAULT_VERDICT_ORIGIN,
        gx_api::verdict_checkpoints::DEFAULT_VERDICT_ORIGIN
    );
    assert_ne!(
        gx_cli::verdict::DEFAULT_VERDICT_ORIGIN,
        gx_cli::ledger::DEFAULT_ORIGIN,
        "and it is **not** the ledger's: a count of verdicts and a tree head are two artefacts \
         travelling under two payload types (AC-VC-4), so one namespace for both would let a \
         verifier fold documents that are not about the same thing"
    );
}

/// 🔴 **`req/792` §2b / `req/801`** — the `--json` spelling of AC-P3-10's tamper arm.
///
/// `req/792` measured `gx verdict-checkpoint verify <tampered> --key <k> --json` at exit **0**
/// with the failure encoded only in the body's `valid: false`, and `req/799` D-2 carried that as
/// "with `--json` present, it always exits 0". The library never had such a branch —
/// `gx_cli::verdict::verify` returns `Outcome::refused(_, VERIFY_FAILED)` for an invalid chain and
/// `main` propagates `outcome.code` unconditionally; the subcommand's `json` flag is parsed and
/// deliberately unread (JSON is already the only thing this binary speaks, 44 §1.3). So the
/// exit-0 measurement can only have been a binary other than this tree's. This test is the pin
/// that keeps the question settled on the artefact rather than in prose: with `--json`, clean is
/// **0** and one changed count is **7**, exactly as without it.
#[test]
fn the_json_flag_does_not_soften_the_tamper_exit() {
    let fixture = pipeline("vc_surface_json_flag", "before\n");
    commit_one(&fixture, "after\n");

    let issued = issue(&fixture, "vc.json");
    assert_eq!(issued.code, 0, "{}", issued.stderr);
    let document = issued.json();
    let pub_path = support::write_public_key(&fixture.project, &signing_pair(&fixture));

    // Positive control: the clean document with `--json` is exit 0.
    let clean = run(fixture
        .gx()
        .args(["verdict-checkpoint", "verify"])
        .arg(fixture.project.join("vc.json"))
        .arg("--key")
        .arg(&pub_path)
        .arg("--json"));
    println!("JSON_CLEAN code={} {}", clean.code, clean.stdout.trim());
    assert_eq!(clean.code, 0, "{}", clean.stderr);
    assert_eq!(clean.json()["valid"], true);

    // Negative control: one changed count with `--json` is exit 7 — the body and the status say
    // one thing, which is what `req/792`'s demo draft needed to be true and measured as false on
    // whatever binary it held.
    let mut tampered = document.clone();
    tampered["tally"]["admit"] = serde_json::json!(9);
    let tampered_path = fixture.project.join("vc_tampered_json_flag.json");
    std::fs::write(
        &tampered_path,
        serde_json::to_vec_pretty(&tampered).expect("serialises"),
    )
    .expect("write");
    let refused = run(fixture
        .gx()
        .args(["verdict-checkpoint", "verify"])
        .arg(&tampered_path)
        .arg("--key")
        .arg(&pub_path)
        .arg("--json"));
    println!(
        "JSON_TAMPERED code={} {}",
        refused.code,
        refused.stdout.trim()
    );
    assert_eq!(
        refused.code, 7,
        "the `--json` flag must not soften 44 §1.2's 7: {}",
        refused.stderr
    );
    assert_eq!(refused.json()["valid"], false);
}
