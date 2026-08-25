// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **discipline 52** (req/38 §48 M6H1-1 adopted (a) / E-M6-2; sem: SEM-gx-cli-1651) and the exit table of this hand's four sections.
//!
//! > Every subcommand goes through `try_parse()` + mapping (usage error → exit **1** "invalid input" · `--help`/`--version` → 0) (sem: SEM-gx-cli-1652).
//! > **44 §1.4's 2 is reserved exclusively for the state machine's "refusal"** (E-M6-2). clap's default `parse()` is forbidden.
//!
//! The rule exists because `clap`'s default status for a usage error is 2 and 44 §1.4 gives 2 to
//! "refused (denied) — Verdict::Deny" (sem: SEM-gx-cli-1653). A binary that took the default would answer a mistyped flag with
//! the code that means "the gate refused your change", and the entire point of a specified exit
//! status is that something branches on it.
//!
//! # What is measured here rather than in the source
//!
//! `probes/doubt/tests/m6_surface_doubt.rs` asserts that the **declared** table equals 44 §1.2's
//! markdown. This suite asserts that the **binary** does what the table says, which is a different
//! claim: a table nobody obeys is a table.
//!
//! It also carries the `gx_code` check. 44 §1.3 sends refusals to stderr as 44 §2.3's problem
//! object, and §2.3 fixes the vocabulary at twelve codes. The twelve are parsed out of the
//! specification and every code this binary emits has to be one of them — the I-11 shape, so that a
//! code invented in `Error::problem` is red rather than plausible.

mod support;

use std::path::{Path, PathBuf};

use support::{project, run, scratch, write_json};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/gx-cli sits two levels under the root")
        .to_path_buf()
}

/// The `gx_code` column of 44 §2.3's table.
fn spec_gx_codes() -> Vec<String> {
    let doc = std::fs::read_to_string(repo_root().join("req/spec/40-architecture/44-api-spec.md"))
        .expect("44 is readable");
    doc.lines()
        .map(str::trim)
        .filter(|l| l.starts_with("| `") && l.matches('|').count() >= 5)
        .filter_map(|l| l.split('`').nth(1).map(str::to_string))
        .filter(|c| c.chars().all(|ch| ch.is_ascii_uppercase() || ch == '_'))
        .collect()
}

/// 🔴 discipline 52 (sem: SEM-gx-cli-1654), on the binary: a usage error is 1, and never clap's 2.
#[test]
fn a_usage_error_exits_one_and_never_two() {
    let cases: Vec<Vec<&str>> = vec![
        vec!["receipt"],                              // a verb with no sub-verb
        vec!["receipt", "show"],                      // a missing required argument
        vec!["receipt", "show", "gx1:not-a-real-id"], // an argument that will not parse
        vec!["log", "proof"],                         // a missing required flag
        vec!["log", "consistency", "--from", "x"],    // a flag whose value will not parse
        vec!["key", "gen", "--alg", "rsa"],           // an algorithm this version cannot write
        vec!["replay", "--from", "0"],                // half of a pair
        vec!["nosuchverb"],                           // no such verb
        vec!["--nosuchflag"],                         // no such flag
    ];
    let mut codes = Vec::new();
    for args in &cases {
        let dir = scratch("exit_usage");
        let out = run(support::gx().arg("--project").arg(&dir).args(args));
        println!("USAGE {args:?} -> exit={}", out.code);
        codes.push(out.code);
        assert_ne!(
            out.code, 2,
            "🔴 discipline 52 / E-M6-2: 2 is 44 §1.4's \"refused (denied)\" (sem: SEM-gx-cli-1655) and belongs to the state machine. \
             `{args:?}` reached it, which means something called clap's `parse()` or `e.exit()`"
        );
        assert_eq!(
            out.code, 1,
            "44 §1.4's \"error (invalid input...)\" (sem: SEM-gx-cli-1656) is 1: {args:?} gave {}",
            out.code
        );
    }
    println!("USAGE_CASES={} EXITS={codes:?}", codes.len());
}

/// `--help` and `--version` are "normal termination" (sem: SEM-gx-cli-1657), at every depth of the verb tree.
#[test]
fn help_and_version_are_zero() {
    let cases: Vec<Vec<&str>> = vec![
        vec!["--help"],
        vec!["--version"],
        vec!["receipt", "--help"],
        vec!["receipt", "show", "--help"],
        vec!["log", "checkpoint", "--help"],
        vec!["key", "gen", "--help"],
        vec!["replay", "--help"],
    ];
    for args in &cases {
        let out = run(support::gx().args(args));
        println!(
            "HELP {args:?} -> exit={} bytes={}",
            out.code,
            out.stdout.len()
        );
        assert_eq!(
            out.code, 0,
            "clap models `--help` as a parse that terminated early; it is not an error"
        );
        assert!(!out.stdout.is_empty(), "{args:?} printed nothing");
    }
}

/// 🔴 `Parser::parse()` appears nowhere, and `exit::DENIED` is returned by nothing.
///
/// The source half of discipline 52 (sem: SEM-gx-cli-1658). A binary that passed the behavioural probes today and reached for
/// `parse()` in hand 3 would reintroduce the collision at the exact moment the state machine starts
/// producing real `2`s — which is when the two become indistinguishable.
#[test]
fn the_source_uses_try_parse_and_returns_denied_from_nowhere() {
    let main = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"))
        .expect("main.rs");
    let code: String = main
        .lines()
        .map(|l| match l.find("//") {
            Some(at) => &l[..at],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n");

    let try_parse = code.matches("try_parse()").count();
    let plain_parse = code.matches("::parse()").count();
    let e_exit = code.matches("e.exit()").count();
    let denied = code.matches("DENIED").count();
    println!(
        "TRY_PARSE={try_parse} PLAIN_PARSE={plain_parse} E_EXIT={e_exit} DENIED_RETURNS={denied}"
    );
    assert_eq!(try_parse, 1, "one parse, and it is the fallible one");
    assert_eq!(
        plain_parse, 0,
        "discipline 52: clap's default `parse()` is forbidden (sem: SEM-gx-cli-1659)"
    );
    assert_eq!(e_exit, 0, "`e.exit()` would use clap's 2");
    assert_eq!(
        denied, 0,
        "nothing on the read side reaches a gate, so nothing may return 44 §1.4's 2"
    );
}

/// 🔴 Every `gx_code` this binary emits is one of 44 §2.3's twelve.
///
/// The vocabulary is 44's, and a CLI that invented a code would be inventing a word the HTTP surface
/// has to share (44 §0: "`gx_code`... is shared between the CLI exit code and the HTTP `problem+json`" (sem: SEM-gx-cli-1660)). Hand 5
/// builds the full mapping (M6-09); what this hand owes is that the words it uses already exist.
#[test]
fn every_emitted_gx_code_is_one_44_declares() {
    let declared = spec_gx_codes();
    println!("SPEC_GX_CODES={} {declared:?}", declared.len());
    assert_eq!(
        declared.len(),
        12,
        "44 §2.3's table has twelve rows; the parser found {}: {declared:?}",
        declared.len()
    );

    let dir = scratch("exit_gx_code");
    let refusals: Vec<Vec<String>> = vec![
        vec![
            "--project".into(),
            dir.display().to_string(),
            "replay".into(),
        ],
        vec![
            "--project".into(),
            dir.display().to_string(),
            "log".into(),
            "proof".into(),
            "--leaf".into(),
            "0".into(),
        ],
        vec![
            "--project".into(),
            dir.display().to_string(),
            "receipt".into(),
            "show".into(),
            "not-an-id".into(),
        ],
    ];
    let mut emitted = Vec::new();
    for args in &refusals {
        let out = run(support::gx().args(args));
        let problem: serde_json::Value =
            serde_json::from_str(out.stderr.trim()).unwrap_or_else(|e| {
                panic!(
                    "44 §1.3 asks for a problem object on stderr: {:?} ({e})",
                    out.stderr
                )
            });
        for field in ["type", "title", "gx_code", "detail"] {
            assert!(
                problem.get(field).is_some(),
                "44 §1.3's four fields; `{field}` is missing from {problem}"
            );
        }
        let code = problem["gx_code"].as_str().expect("a string").to_string();
        println!("REFUSAL {args:?} -> exit={} gx_code={code}", out.code);
        assert!(
            out.stdout.trim().is_empty(),
            "44 §1.3: \"stdout emits nothing\" (sem: SEM-gx-cli-1661)"
        );
        assert!(
            declared.contains(&code),
            "`{code}` is not one of 44 §2.3's twelve: {declared:?}"
        );
        emitted.push(code);
    }
    emitted.sort();
    emitted.dedup();
    println!("EMITTED_GX_CODES={emitted:?}");
    assert!(!emitted.is_empty());
    // 🔴 More than one, and that is the assertion rather than a nicety. A binary that answered
    // `INTERNAL` to everything would satisfy the membership check above while telling an operator
    // nothing — which is req/88 §3 Λ4's quotient collapsed all the way down. The three cases here
    // are "no project here", "no ledger here" and "that is not an id" (sem: SEM-gx-cli-1662), and the first two stopped
    // being `INTERNAL` when `Error::exit_code` learned that an `ErrorKind::NotFound` is 44 §1.4's 6.
    assert!(
        emitted.len() >= 2,
        "three different refusals collapsed to one code: {emitted:?}"
    );
    assert!(emitted.contains(&"NOT_FOUND".to_string()));
}

/// 🔴 44 §1.3's split: a **result** goes to stdout even when it is a refusal; an **error** does not.
///
/// The two are different and the difference is what makes the exit statuses readable. `gx receipt
/// show` on a missing id ran, answered, and exited 6 with an object saying which id was missed;
/// `gx receipt show` on an unparseable id never started, and stdout stays empty.
#[test]
fn a_refusal_that_ran_prints_and_a_refusal_that_did_not_stays_quiet() {
    let (dir, _layout) = project("exit_stdout_split");

    let answered = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("receipt")
        .arg("show")
        .arg(support::tid(1).0.to_text()));
    println!(
        "ANSWERED exit={} stdout={:?}",
        answered.code,
        answered.stdout.trim()
    );
    assert_eq!(answered.code, 6);
    assert_eq!(answered.json()["found"], serde_json::json!(false));
    assert!(answered.stderr.trim().is_empty());

    let refused = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("receipt")
        .arg("show")
        .arg("definitely-not-an-id"));
    println!(
        "REFUSED exit={} stderr={:?}",
        refused.code,
        refused.stderr.trim()
    );
    assert_eq!(refused.code, 1);
    assert!(refused.stdout.trim().is_empty());
    assert!(refused.stderr.contains("VALIDATION_ERROR"));
}

/// 🔴 `gx wrap --check-config`'s 7, run for real (v0.4-e; `req/38` §107 residue 4; sem: SEM-gx-cli-1663, the arm `req/169`
/// §10-2 declared as its own denominator: "wrap's 7 (check-config) is unmeasured by E2E — neither unit nor E2E has a live-run arm
/// (only a primary reading of the main.rs implementation)").
///
/// Until this test, `HAND_P3_EXITS`'s wrap-7 row rested on a source reading; every other 7 in
/// that table has a live measurement (`tools/e2e_p3.sh`'s AC-P3-10 tamper arm). This is wrap's,
/// closing the last read-only row: 44 §1.2 verbatim "7=`--check-config` detected a surviving direct-call
/// path" (sem: SEM-gx-cli-1664), and 44 §1.4's v0.2.6 addendum makes 7 a *kind* (an offline, self-contained check that
/// failed) rather than `gx receipt verify`'s own number.
///
/// Both failing halves of B-1's passing state ("routed through gx **and** no entry starts the
/// server directly" (sem: SEM-gx-cli-1665), `main.rs`) are driven, because either alone keeps the direct road open:
///
/// * the checked entry **is** wrapped but a second entry still starts the same server binary
///   directly — the case `config.rs` names as why `direct` is a list ("two entries name one
///   server and only one was adopted" (sem: SEM-gx-cli-1666)). The refusal must *name* the residual entry: an operator
///   answering an exit 7 needs to know which line of their config to delete.
/// * the checked entry was never adopted at all — `wrapped: false`, and the entry itself is the
///   direct road.
///
/// The wrapped fixture is hand-written to the exact shape `--adopt-config` emits (`gx` running
/// `["wrap", …, "--", <command>, …]` — `config::is_wrapped` keys on `args[0] == "wrap"`); the
/// green test below gets the same shape from the adopt road itself, so the two arms triangulate
/// the fixture against the product's own writer.
/// 🔴 `cfg(feature = "mcp")` (`req/817`) — drives `gx wrap --check-config`, and `gx wrap` is absent
/// from the public distribution (`gx-mcp-wire` is private, `req/789` §3).
#[cfg(feature = "mcp")]
#[test]
fn check_config_with_a_residual_direct_entry_exits_seven_and_names_it() {
    let dir = scratch("exit_check_config_red");
    let config = serde_json::json!({
        "mcpServers": {
            "files": {
                "command": "gx",
                "args": ["wrap", "--actor-key", "key-1", "--actor-model", "claude-fable-5",
                         "--", "server-bin", "--stdio"]
            },
            "files-direct": { "command": "server-bin", "args": ["--stdio"] }
        }
    });
    let path = write_json(&dir.join("agent.json"), &config);

    // Half one: adopted, with a residual direct sibling.
    let out = run(support::gx()
        .arg("wrap")
        .arg("--check-config")
        .arg(&path)
        .args(["--server-name", "files"]));
    println!(
        "CHECK_CONFIG_RESIDUAL exit={} stdout={}",
        out.code,
        out.stdout.trim()
    );
    assert_eq!(
        out.code, 7,
        "44 §1.2 `gx wrap`: \"7=--check-config detected a surviving direct-call path\" (sem: SEM-gx-cli-1667); got {} ({})",
        out.code, out.stderr
    );
    let json = out.json();
    assert_eq!(json["wrapped"], serde_json::json!(true));
    assert_eq!(
        json["direct_entries"],
        serde_json::json!(["files-direct"]),
        "the refusal must name the residual entry, not merely count it"
    );

    // Half two: the checked entry itself was never adopted.
    let unadopted = run(support::gx()
        .arg("wrap")
        .arg("--check-config")
        .arg(&path)
        .args(["--server-name", "files-direct"]));
    println!(
        "CHECK_CONFIG_UNADOPTED exit={} stdout={}",
        unadopted.code,
        unadopted.stdout.trim()
    );
    assert_eq!(
        unadopted.code, 7,
        "an entry that never routed through gx is the direct road itself"
    );
    assert_eq!(unadopted.json()["wrapped"], serde_json::json!(false));
}

/// The clean half of the pair: a config whose direct road is gone exits **0** — and the clean
/// fixture is not hand-written, it is what `--adopt-config` itself produced.
///
/// That choice is the point of the test's shape: `--check-config` is B-1's machine confirmation
/// of what `--adopt-config` did (44 §1.2 verbatim "machine-checks whether it has been substituted... (**B-1's machine check**)" (sem: SEM-gx-cli-1668)),
/// so the green arm drives the two flags as the pair they are documented to be. A hand-built
/// "clean" (sem: SEM-gx-cli-1669) fixture could drift from what adopt actually writes and this test would never know;
/// this way, if adopt's output ever stops satisfying its own checker, the pair goes RED here.
/// 🔴 `cfg(feature = "mcp")` (`req/817`) — drives `gx wrap --adopt-config` / `--check-config` as a
/// pair; both are absent from the public distribution.
#[cfg(feature = "mcp")]
#[test]
fn check_config_after_adopt_config_exits_zero() {
    let dir = scratch("exit_check_config_green");
    let config = serde_json::json!({
        "mcpServers": {
            "files": { "command": "server-bin", "args": ["--stdio"] }
        }
    });
    let path = write_json(&dir.join("agent.json"), &config);

    let adopted = run(support::gx()
        .arg("wrap")
        .arg("--adopt-config")
        .arg(&path)
        .args(["--server-name", "files"]));
    println!(
        "ADOPT_CONFIG exit={} stdout={}",
        adopted.code,
        adopted.stdout.trim()
    );
    assert_eq!(adopted.code, 0, "adopt: {}", adopted.stderr);

    let out = run(support::gx()
        .arg("wrap")
        .arg("--check-config")
        .arg(&path)
        .args(["--server-name", "files"]));
    println!(
        "CHECK_CONFIG_CLEAN exit={} stdout={}",
        out.code,
        out.stdout.trim()
    );
    assert_eq!(
        out.code, 0,
        "44 §1.2: exit 0 = the direct road is gone; got {} ({})",
        out.code, out.stderr
    );
    let json = out.json();
    assert_eq!(json["wrapped"], serde_json::json!(true));
    assert_eq!(json["direct_entries"], serde_json::json!([]));
}

// ---------------------------------------------------------------------------
// 🔴 P-2 I — `gx receipt coverage`'s exit set, on the binary
// (`req/571` Part I AC-2/AC-3, `req/38` §341 ruling (A))
// ---------------------------------------------------------------------------

/// One file from the specimen P-1b froze, by name.
fn frozen(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("attach_face_frozen")
        .join("issued_2026_08_22")
        .join(name)
}

/// 🔴 **AC-2** — the three statuses `gx receipt coverage` returns, and the one it does not.
///
/// 44 §1.2 gained a section for this verb in the same commit as the `SPEC_44_EXITS` row, and
/// `m6_exit_matrix.rs` checks that the two texts say `[0, 1, 6]`. Two texts agreeing is still two
/// texts: this is the binary, which is the half `exit_map.rs` exists for.
///
/// # 🔴 The fifth arm is the one that matters, and it carries its own control
///
/// The interesting claim is negative — **7 does not come out of `coverage`** — and a negative claim
/// measured on a healthy specimen is vacuous, because a healthy receipt would not draw a 7 out of
/// `gx receipt verify` either. So the specimen is broken *first* and the break is *demonstrated*:
/// the same file goes to `gx receipt verify --offline`, which answers **7**, and then to
/// `gx receipt coverage`, which answers **0**. One file, two verbs, two answers — that is the
/// difference between "coverage did not fail" and "coverage does not verify".
#[test]
fn receipt_coverage_returns_zero_one_and_six_and_never_seven() {
    let dir = scratch("exit_receipt_coverage");

    // 0 — a receipt this binary can read.
    let ok = run(support::gx()
        .arg("receipt")
        .arg("coverage")
        .arg(frozen("commit_receipt.json")));
    println!("COVERAGE_OK exit={}", ok.code);
    assert_eq!(
        ok.code, 0,
        "44 §1.2: a coverage table is an answer, `unknown` rows included: {}",
        ok.stderr
    );

    // 6 — 44 §1.4's "not found", `Error::Io{NotFound}` through `lib.rs`'s `exit_code`.
    let missing = run(support::gx()
        .arg("receipt")
        .arg("coverage")
        .arg(dir.join("no-such-receipt.json")));
    println!("COVERAGE_MISSING exit={}", missing.code);
    assert_eq!(
        missing.code, 6,
        "a file that is not there is 44 §1.4's 6, the same as every other verb in this binary: {}",
        missing.stderr
    );

    // 1 — bytes that are not a receipt.
    let rubbish = dir.join("not-a-receipt.json");
    std::fs::write(&rubbish, b"this is not a DSSE envelope").expect("write the rubbish");
    let refused = run(support::gx().arg("receipt").arg("coverage").arg(&rubbish));
    println!("COVERAGE_RUBBISH exit={}", refused.code);
    assert_eq!(
        refused.code, 1,
        "`Error::Usage` is 44 §1.4's \"invalid input\", not a verification failure: {}",
        refused.stderr
    );

    // 1 — a readable receipt beside a `--face` file that is not JSON (`Error::Malformed`).
    let bad_face = dir.join("face-not-json.json");
    std::fs::write(&bad_face, b"{ not json").expect("write the malformed face");
    let malformed = run(support::gx()
        .arg("receipt")
        .arg("coverage")
        .arg(frozen("commit_receipt.json"))
        .arg("--face")
        .arg(&bad_face));
    println!("COVERAGE_BAD_FACE exit={}", malformed.code);
    assert_eq!(
        malformed.code, 1,
        "a face that will not parse is invalid input, and the receipt beside it is untouched: {}",
        malformed.stderr
    );

    // 🔴 The fifth arm. A receipt whose signature is a signature, and is the wrong one.
    let raw = std::fs::read(frozen("commit_receipt.json")).expect("the frozen receipt is here");
    let mut document: serde_json::Value = serde_json::from_slice(&raw).expect("it is JSON");
    let sig = document["envelope"]["signatures"][0]["sig"]
        .as_str()
        .expect("the envelope carries a signature")
        .to_string();
    // 🔴 One character of the body, not the whole string reversed and not a replacement.
    //
    // The first draft reversed it, and `gx receipt verify` answered **1** — the base64 padding
    // landed at the front, so the envelope stopped being *readable* and the run never reached a
    // signature at all. A specimen that fails to parse cannot demonstrate that a verb which does
    // verify says 7, so it cannot be the control for a verb which does not. Changing the leading
    // character keeps the length, the padding, the alphabet and the key id, and moves only the
    // decoded bytes — the file is still a receipt, and it is signed by nothing.
    let mut chars: Vec<char> = sig.chars().collect();
    chars[0] = if chars[0] == 'A' { 'B' } else { 'A' };
    let broken: String = chars.into_iter().collect();
    assert_ne!(broken, sig, "the edit has to actually change the bytes");
    assert_eq!(
        broken.len(),
        sig.len(),
        "and has to leave a signature-shaped string"
    );
    document["envelope"]["signatures"][0]["sig"] = serde_json::json!(broken);
    let tampered = write_json(&dir.join("tampered_receipt.json"), &document);

    // The control: this file really is broken, and something in this binary says so.
    let verified = run(support::gx()
        .arg("receipt")
        .arg("verify")
        .arg(&tampered)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(frozen("checkpoint.json"))
        .arg("--key")
        .arg(frozen("key.pub.json")));
    println!(
        "TAMPERED_VERIFY exit={} stdout={}",
        verified.code,
        verified.stdout.trim()
    );
    assert_eq!(
        verified.code, 7,
        "🔴 the specimen must be genuinely broken or the arm below proves nothing: \
         `gx receipt verify` answered {} instead of 44 §1.4's 7 ({})",
        verified.code, verified.stderr
    );

    // The claim: the same file, read rather than verified.
    let covered = run(support::gx().arg("receipt").arg("coverage").arg(&tampered));
    println!("TAMPERED_COVERAGE exit={}", covered.code);
    assert_ne!(
        covered.code, 7,
        "🔴 `gx receipt coverage` returned 44 §1.4's \"offline verification failure\" for a \
         document it does not verify. 44 §1.2's section for this verb writes `[0, 1, 6]` and \
         `SPEC_44_EXITS` declares the same three; a 7 here would make both of them false"
    );
    assert_eq!(
        covered.code, 0,
        "a broken signature is not a coverage question — the table is a projection of the \
         payload, and the payload still decodes: {}",
        covered.stderr
    );

    // And the same shape from the inclusion side, with no bytes touched: an anchorless verify is
    // 7 (H5-9, `crates/gx-cli/src/receipt.rs`'s `Unanchored` is not a PASS) while coverage of the
    // untouched original was 0, above.
    let unanchored = run(support::gx()
        .arg("receipt")
        .arg("verify")
        .arg(frozen("commit_receipt.json"))
        .arg("--offline")
        .arg("--key")
        .arg(frozen("key.pub.json")));
    println!("UNANCHORED_VERIFY exit={}", unanchored.code);
    assert_eq!(
        unanchored.code, 7,
        "H5-9: a receipt with nothing to be included in does not pass: {}",
        unanchored.stderr
    );
    assert_eq!(
        ok.code, 0,
        "and coverage of that same untouched receipt was 0"
    );
}

/// 🔴 **AC-3** — the declared row for `gx receipt coverage` carries no 7, and the check can tell.
///
/// The negative control is the point: a predicate that only ever looks at the true row passes
/// whatever the row says. So the same predicate is run against
/// `gx receipt show` / `gx receipt verify`, whose row **does** carry 7, and it has to come back the
/// other way. A grep that cannot go red on a row containing 7 is not measuring 7.
#[test]
fn the_coverage_row_declares_no_offline_verification_failure() {
    let row = gx_cli::exit::SPEC_44_EXITS
        .iter()
        .find(|(section, _)| *section == "`gx receipt coverage`")
        .expect(
            "🔴 `SPEC_44_EXITS` has no row for `gx receipt coverage`; 44 §1.2 has a section for \
             it, and the two are compared in `probes/doubt/tests/m6_exit_matrix.rs`",
        );
    println!("COVERAGE_ROW={:?}", row.1);
    assert_eq!(
        row.1,
        &[0u8, 1, 6],
        "44 §1.2's section for this verb writes exactly these three"
    );
    assert!(
        !row.1.contains(&gx_cli::exit::VERIFY_FAILED),
        "🔴 7 is 44 §1.4's \"offline verification failure\" and this verb performs no verification"
    );

    let control = gx_cli::exit::SPEC_44_EXITS
        .iter()
        .find(|(section, _)| *section == "`gx receipt show` / `gx receipt verify`")
        .expect("the older receipt section is still declared");
    println!("CONTROL_ROW={:?}", control.1);
    assert!(
        control.1.contains(&gx_cli::exit::VERIFY_FAILED),
        "🔴 the control did not carry 7, so the assertion above would have passed on any row and \
         is measuring nothing: {:?}",
        control.1
    );
}
