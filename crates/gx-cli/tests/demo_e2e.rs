// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
// what : `gx demo` end to end, as a permanent regression test rather than a one-off script run --
//        `tools/verify_p5.sh` re-runs the same walk under `unshare -rn` for AC-P5-1's timing and
//        network-isolation claims; this file is what keeps the walk itself from rotting under
//        ordinary `cargo test --workspace`.
// why  : `req/134` §2 AC-P5-1: "exit 0 under non-interactive, network-blocked conditions; elapsed
//        <5 minutes; prints `DEMO_COMPLETE` plus 4 stages of measured lines; the demo's
//        receipt exits 0 via `gx receipt verify --offline` (a separate process)" (sem: SEM-gx-cli-1600). Ruling 1:
//        "the demo is a real walk through `wrap`" is machine-confirmed by the test -- the assertions below are on the notes
//        server's own arrival count (A-7's technique: a count taken from the far side of the
//        wire, not from the walk's own say-so) and on a **separate** `gx receipt verify` process
//        this test spawns itself, independent of whatever `gx demo` printed.
// where: any platform `gx demo` runs on. The `/etc/hostname` deny arm inside `gx demo` is
//        Unix-only (mirrors `tools/e2e_p3.sh`'s own AC-P3-13 target); this test is written to run
//        wherever the workspace floor runs, which is WSL2 Ubuntu-24.04 per this project's own
//        convention (`crates/gx-cli/src/demo.rs`'s module header).

// 🔴 `req/817`: every test here drives `gx demo`, whose mechanism is
// `gx-mcp-wire` -- one of the four crates `req/789` §3 holds private. The public
// distribution does not carry the verb, so the suite compiles away rather than failing against
// a subcommand that is deliberately absent. The private build runs it exactly as before.
#![cfg(feature = "mcp")]

use std::path::PathBuf;
use std::process::Command;

fn gx() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_gx"))
}

#[test]
fn gx_demo_completes_and_its_receipts_verify_offline_in_a_separate_process() {
    let output = Command::new(gx())
        .arg("demo")
        .output()
        .expect("gx demo runs");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "gx demo exited {:?}\nstdout={stdout}\nstderr={stderr}",
        output.status
    );

    // The four stage headings and the terminating word AC-P5-1 asks for -- `tools/e2e_p3.sh`'s
    // own `=== N. ... ===` convention, reused rather than invented.
    for heading in [
        "=== 1. broke it ===",
        "=== 2. proved it ===",
        "=== 3. restored it ===",
        "=== 4. verified it",
    ] {
        assert!(
            stdout.contains(heading),
            "missing heading {heading:?} in:\n{stdout}"
        );
    }
    assert!(
        stdout.contains("DEMO_COMPLETE"),
        "gx demo did not print DEMO_COMPLETE:\n{stdout}"
    );

    // The trailing JSON summary line (this binary's own `Outcome`, printed by `main`'s generic
    // dispatcher the way `gx wrap`'s summary is) -- parsed to reach the two receipt paths without
    // re-deriving them from prose.
    let json_line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .expect("a trailing JSON summary line");
    let summary: serde_json::Value =
        serde_json::from_str(json_line).expect("the summary line is valid JSON");
    assert_eq!(summary["gx"], "demo");
    assert_eq!(
        summary["arrivals"].as_u64(),
        Some(2),
        "A-7: the notes server's own arrival count should read 2 (the admitted write, then the \
         undo's restore) -- a count taken from the server's side of the wire, not this test's \
         say-so:\n{summary}"
    );

    let commit_receipt = summary["commit_receipt"]
        .as_str()
        .expect("commit_receipt path");
    let undo_receipt = summary["undo_receipt"].as_str().expect("undo_receipt path");
    let commit_checkpoint = summary["commit_checkpoint"]
        .as_str()
        .expect("commit_checkpoint path");
    let undo_checkpoint = summary["undo_checkpoint"]
        .as_str()
        .expect("undo_checkpoint path");
    let public_key = summary["public_key"].as_str().expect("public_key path");
    for path in [
        commit_receipt,
        undo_receipt,
        commit_checkpoint,
        undo_checkpoint,
        public_key,
    ] {
        assert!(
            std::path::Path::new(path).is_file(),
            "{path} does not exist"
        );
    }

    // 🔴 AC-P5-1's "not this walk's own self-report": a **third** process (this test spawns its
    // own `gx receipt verify`, independent of the two `gx demo` already ran internally) checks
    // both receipts offline, each against the checkpoint that was current when it was issued
    // (RFC 6962: an inclusion proof is relative to a tree size -- `tools/e2e_p3.sh` §3b's own
    // reasoning, carried here rather than re-derived).
    for (receipt, checkpoint) in [
        (commit_receipt, commit_checkpoint),
        (undo_receipt, undo_checkpoint),
    ] {
        let verify = Command::new(gx())
            .args([
                "receipt",
                "verify",
                receipt,
                "--offline",
                "--checkpoint",
                checkpoint,
                "--checkpoint-key",
                public_key,
                "--key",
                public_key,
            ])
            .output()
            .expect("gx receipt verify runs");
        assert!(
            verify.status.success(),
            "a separate `gx receipt verify --offline {receipt}` exited {:?}: stdout={} stderr={}",
            verify.status,
            String::from_utf8_lossy(&verify.stdout),
            String::from_utf8_lossy(&verify.stderr)
        );
    }
}
