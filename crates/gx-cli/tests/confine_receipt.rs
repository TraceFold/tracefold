// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/493` §1 AC-6, through the binary** — the road from the kernel's answer to the signed
//! bytes of a receipt, measured at every hop.
//!
//! # The gap this closes, in `req/497` §7's own words
//!
//! > `gx confine` is a **launcher**: it takes the ruleset and becomes another program by `exec`.
//! > What makes a receipt is the `gx commit` the confined agent calls afterwards … So carrying the
//! > confinement context onto a receipt needs a road ("`gx confine` sets an env var + the commit
//! > side reads it") **and** an addition to the receipt structure itself.
//!
//! Three hops, one test each, and each has a control:
//!
//! | hop | measured by | control |
//! |---|---|---|
//! | kernel → environment | [`confine_hands_the_declaration_across_the_exec`] | the same command with no `gx confine` in front of it sees nothing |
//! | environment → engine | [`the_grammar_round_trips_and_a_value_this_build_cannot_read_stops_the_run`] | an unreadable value refuses instead of defaulting |
//! | engine → signed bytes | [`a_commit_under_a_declared_confinement_says_so_on_its_receipt`] | the same commit with no variable set says `kernel_confined: false` |
//!
//! # Why the third one reads the file rather than a JSON field
//!
//! `req/493` §0 asks for the context "on the receipt", and a verb that printed it would satisfy a
//! reading of that sentence without satisfying the sentence: what makes a receipt worth anything is
//! that a stranger with no network can check it. So this suite opens `.gx/receipts/<tid>.commit.json`,
//! decodes the DSSE payload, and reads the member out of **the bytes the signature covers**.

// 🔴 `req/817`: this suite exercises a verb whose mechanism is `gx-confine`,
// one of the four crates `req/789` §3 holds private. The public distribution builds without
// it (`default = []` there), so the whole file compiles away rather than failing to resolve
// a crate that is not in the tree. The private build turns `confine` on by default and runs it.
#![cfg(feature = "confine")]

mod support;

use std::path::Path;

use gx_cli::confine::{read_declaration, CONFINEMENT_ENV};
use gx_witness::receipt::{ConfinementContext, Receipt, ReceiptPayload};

use support::{pipeline, run, Pipeline};

/// The commit receipt this fixture filed, decoded from the store on disk.
///
/// Through `gx_witness::Receipt` rather than through `serde_json::Value`, because the question is
/// what the **payload** carries and the payload is base64 inside the envelope. A reader that dug
/// the field out of the JSON around it would be reading a place no signature reaches.
fn filed_payload(fixture: &Pipeline, tid: &str) -> ReceiptPayload {
    // `ReceiptStore` writes `<TID>.<kind>.json` with the id's `:` replaced, because a colon is not
    // a filename on every platform gx runs on. The separator is not this suite's business, so the
    // file is found by its kind suffix among the receipts this project filed.
    let dir = fixture.project.join(".gx").join("receipts");
    let path = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("the receipt store is at {}: {e}", dir.display()))
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .find(|p| {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            name.ends_with(".commit.json")
                && tid.split(':').next_back().is_some_and(|t| name.contains(t))
        })
        .unwrap_or_else(|| panic!("a commit receipt for {tid} is filed in {}", dir.display()));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the commit receipt is filed at {}: {e}", path.display()));
    let receipt: Receipt = serde_json::from_str(&text).expect("the JSON face decodes");
    receipt.payload().expect("the signed payload decodes")
}

// ---------------------------------------------------------------------------
// hop 3: the engine's value reaches the signed bytes
// ---------------------------------------------------------------------------

/// 🔴 **AC-6** — a commit made under a declared confinement carries it where a stranger can read it.
///
/// The control is the second half: the same fixture, the same intent, no variable. Without it a
/// producer that wrote a constant would pass.
#[test]
fn a_commit_under_a_declared_confinement_says_so_on_its_receipt() {
    let ruleset = "gxleaf1abcdefghijklmnop";

    let held = pipeline("s3ac6_cli_held", "before\n");
    let held_tid = {
        let submitted = held.submit("after\n");
        assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
        let intent_id = submitted.json()["intent_id"]
            .as_str()
            .expect("an intent id")
            .to_string();
        let planned = run(held.gx().args(["plan", &intent_id]));
        assert_eq!(planned.code, 0, "plan: {}", planned.stderr);
        let tid = planned.json()["transformation"]["id"]
            .as_str()
            .expect("a transformation id")
            .to_string();
        let verified = run(held
            .gx()
            .env(
                CONFINEMENT_ENV,
                format!("kernel_confined=1;ruleset_hash={ruleset}"),
            )
            .args(["verify", &tid]));
        assert_eq!(verified.code, 0, "verify: {}", verified.stderr);
        let committed = run(held
            .gx()
            .env(
                CONFINEMENT_ENV,
                format!("kernel_confined=1;ruleset_hash={ruleset}"),
            )
            .args(["commit", &tid]));
        assert_eq!(committed.code, 0, "commit: {}", committed.stderr);
        tid
    };
    let held_payload = filed_payload(&held, &held_tid);

    let loose = pipeline("s3ac6_cli_loose", "before\n");
    let loose_tid = loose.commit_one("after\n");
    let loose_payload = filed_payload(&loose, &loose_tid);

    println!(
        "AC6_CLI held={:?} loose={:?}",
        held_payload.confinement, loose_payload.confinement
    );
    assert_eq!(
        held_payload.confinement,
        Some(ConfinementContext {
            kernel_confined: true,
            ruleset_hash: Some(ruleset.to_string()),
        }),
        "🔴 `req/493` §0: the confinement context is on the receipt, inside the signed bytes"
    );
    assert_eq!(
        loose_payload.confinement,
        Some(ConfinementContext::unconfined()),
        "and a run nobody confined states that, rather than leaving the seat empty"
    );
}

// ---------------------------------------------------------------------------
// hop 2: the grammar, and the refusal that is not a default
// ---------------------------------------------------------------------------

/// 🔴 The two legal values round-trip, and a value this build cannot read **stops the run**.
///
/// The refusal is the interesting half. "Assume unconfined" is the tempting reading of an
/// unparseable value and it is the one that puts an assumption inside a signature: a newer
/// `gx confine` writing a grammar this build does not know would produce receipts silently
/// under-stating a real confinement, and nothing would say so. `req/38` §287-2 named that shape —
/// a field nobody checks is a field, not a claim.
#[test]
fn the_grammar_round_trips_and_a_value_this_build_cannot_read_stops_the_run() {
    for context in [
        ConfinementContext::unconfined(),
        ConfinementContext {
            kernel_confined: true,
            ruleset_hash: Some("gxleaf1zzz".to_string()),
        },
    ] {
        let spelled = if context.kernel_confined {
            format!(
                "kernel_confined=1;ruleset_hash={}",
                context
                    .ruleset_hash
                    .clone()
                    .expect("a confined context names one")
            )
        } else {
            "kernel_confined=0".to_string()
        };
        let back = read_declaration(Some(&spelled)).expect("a legal value reads");
        println!("AC6_GRAMMAR spelled={spelled:?} read={back:?}");
        assert_eq!(back, Some(context));
    }
    assert_eq!(
        read_declaration(None).expect("an unset variable is not an error"),
        None,
        "unset is the ordinary case and the caller states the default, not this function"
    );

    // 🔴 Each of these is a bed for a way of getting the grammar wrong, and each is refused.
    for bad in [
        "",
        "1",
        "true",
        "kernel_confined=yes",
        "kernel_confined=1",               // confined, and no ruleset named
        "kernel_confined=1;ruleset_hash=", // named with nothing
        "kernel_confined=0;ruleset_hash=gxleaf1a", // a ruleset the kernel did not take
        "kernel_confined=1;gxleaf1a",      // the value without its key
        "kernel_confined=1;ruleset_hash=a;extra=1",
        "{\"kernel_confined\":true}",
    ] {
        let refusal = read_declaration(Some(bad));
        println!("AC6_GRAMMAR_REFUSED {bad:?} -> {refusal:?}");
        assert!(
            refusal.is_err(),
            "🔴 `{bad:?}` reads as a confinement, which means this build would sign a claim it \
             derived from a value it does not understand"
        );
    }

    // And the refusal reaches the binary rather than living in a function nobody calls: a `gx`
    // whose environment carries an unreadable value does not commit.
    let fixture = pipeline("s3ac6_cli_bad_env", "before\n");
    let submitted = fixture.submit("after\n");
    assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
    let refused = run(fixture
        .gx()
        .env(CONFINEMENT_ENV, "kernel_confined=maybe")
        .args([
            "plan",
            submitted.json()["intent_id"].as_str().expect("an id"),
        ]));
    println!(
        "AC6_CLI_BAD_ENV code={} stderr={}",
        refused.code, refused.stderr
    );
    assert_ne!(
        refused.code, 0,
        "a run whose confinement cannot be established does not proceed to write a journal"
    );
    assert!(
        refused.stderr.contains(CONFINEMENT_ENV) || refused.stdout.contains(CONFINEMENT_ENV),
        "and it names the variable, which is where the operator can act: {} / {}",
        refused.stdout,
        refused.stderr
    );
}

// ---------------------------------------------------------------------------
// hop 1: the kernel's answer crosses the `exec`
// ---------------------------------------------------------------------------

/// 🔴 **The hop `req/497` §7 named** — `gx confine` hands its answer to the process it becomes.
///
/// Linux only, and for the reason the rest of S③ is: `gx_confine::apply` refuses on any other
/// platform, so there is no confinement to declare and nothing to measure. The control runs the
/// same command **without** `gx confine` in front of it and asserts the variable is not there, so a
/// green cannot be produced by a shell that happened to have it set.
#[test]
#[cfg(target_os = "linux")]
fn confine_hands_the_declaration_across_the_exec() {
    let fixture = pipeline("s3ac6_cli_exec_hop", "before\n");
    let work = fixture.project.join("work");
    std::fs::create_dir_all(&work).expect("the granted directory");
    let catalogue = fixture.project.join("catalogue.json");
    std::fs::write(
        &catalogue,
        r#"{ "notes/write": { "restored_by": "notes/restore" } }"#,
    )
    .expect("the catalogue");

    // The control first: the payload with nothing in front of it.
    let bare = run(std::process::Command::new("/bin/sh")
        .args(["-c", "echo \"seen=[${GX_CONFINEMENT:-}]\""])
        .env_remove(CONFINEMENT_ENV));
    println!("AC6_EXEC_CONTROL {}", bare.stdout.trim());
    assert!(
        bare.stdout.contains("seen=[]"),
        "the control has to start from an environment that does not carry the variable: {}",
        bare.stdout
    );

    // What the plan says the hash is, taken from the verb's own report rather than recomputed here.
    let planned = run(fixture
        .gx()
        .arg("--mcp-restore-catalogue")
        .arg(&catalogue)
        .args(["confine", "--tool", "notes/write", "--allow-write"])
        .arg(&work)
        .arg("--plan-only"));
    assert_eq!(planned.code, 0, "--plan-only: {}", planned.stderr);
    let expected_hash = planned.json()["plan"]["ruleset_hash"]
        .as_str()
        .unwrap_or_else(|| panic!("the plan names its ruleset hash: {}", planned.stdout))
        .to_string();

    let confined = run(fixture
        .gx()
        .arg("--mcp-restore-catalogue")
        .arg(&catalogue)
        .args(["confine", "--tool", "notes/write", "--allow-write"])
        .arg(&work)
        .arg("--")
        .args(["/bin/sh", "-c", "echo \"seen=[${GX_CONFINEMENT:-}]\""])
        .env_remove(CONFINEMENT_ENV));
    println!(
        "AC6_EXEC_HOP code={} stdout={} expected_hash={expected_hash}",
        confined.code,
        confined.stdout.trim()
    );
    assert_eq!(confined.code, 0, "gx confine: {}", confined.stderr);
    let seen = confined
        .stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix("seen=["))
        .and_then(|l| l.strip_suffix(']'))
        .unwrap_or_else(|| panic!("the payload printed the variable: {}", confined.stdout))
        .to_string();
    assert_eq!(
        read_declaration(Some(&seen)).expect("what `gx confine` writes, this build reads"),
        Some(ConfinementContext {
            kernel_confined: true,
            ruleset_hash: Some(expected_hash),
        }),
        "🔴 the value that crossed the `exec` names the ruleset the kernel actually took"
    );
}

/// 🔴 The bit is the **kernel's** answer and not the verb's intention.
///
/// `gx_confine::apply` can return with the fs face `NotEnforced` — an older kernel, or one without
/// `CONFIG_SECURITY_LANDLOCK`. `req/493` §1 AC-3 forbids collapsing that into "confined", and this
/// is where the collapse would happen: a declaration built before the call, or from the plan rather
/// than from the answer, would say `1` for a face nothing is holding.
#[test]
fn the_declaration_reads_the_face_the_kernel_took_and_not_the_plan() {
    let source =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/confine.rs"))
            .expect("the module is here");
    let body = source
        .split("pub fn declaration(")
        .nth(1)
        .expect("the function is declared")
        .split("\n}")
        .next()
        .expect("it closes");
    println!(
        "AC6_DECLARATION_SOURCE reads_face={} reads_plan={}",
        body.contains("fs.is_enforcing()"),
        body.contains("ConfinePlan")
    );
    assert!(
        body.contains("fs.is_enforcing()"),
        "🔴 `req/493` §1 AC-3: the bit is `FaceStatus::is_enforcing` on the face this build \
         constructs a ruleset for. Anything else — a boolean the verb set before calling, the \
         plan's existence, `--allow-write` being non-empty — is the collapse AC-3 forbids"
    );
    // And the ordering: the declaration is built from the value `apply` returned. If the call site
    // moved above `gx_confine::apply`, the value would be a plan and not an answer.
    let run_body = source
        .split("pub fn run(")
        .nth(1)
        .expect("`run` is declared")
        .to_string();
    let applied_at = run_body.find("gx_confine::apply").expect("`run` applies");
    let declared_at = run_body
        .find("declaration(&confinement)")
        .expect("`run` declares what was applied");
    assert!(
        applied_at < declared_at,
        "the kernel answers before the answer is written down"
    );
}
