// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `req/1022` §2-b/§2-g's two `--json` gaps and its top-frequency `PathBuf` stdin gap, repaired
//! (`req/1036` R6). Red-first: every arm below failed against the pre-repair binary — `--json` was
//! an unknown-flag clap usage error (exit 2) on the two commands named in (g), and `-` opened a
//! literal file named `-` (`ENOENT`, exit 1) on the four `PathBuf` positionals picked in (b).

mod support;

use gx_core::VerdictKind;

use support::{commit_receipt, issue, keypair, project, run, scratch, seed_ledger, verdict_payload, write_json};

/// (g): `gx log checkpoint` was the one `LogCmd` leaf with no `--json`, though its siblings
/// (`proof`/`consistency`) both have it and 44 §1.2's flag is vestigial on every other verb (accepted,
/// ignored, output is JSON either way).
#[test]
fn log_checkpoint_accepts_json_like_its_siblings() {
    let (dir, layout) = project("dr1022_log_checkpoint_json");
    let key = keypair(21);
    seed_ledger(&layout, &key, 90, 2);
    let key_dir = support::secure_scratch("dr1022_log_checkpoint_json_key");
    let secret = key_dir.join("ledger.key");
    key.save(&secret).expect("save");

    let out = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("log")
        .arg("checkpoint")
        .arg("--key")
        .arg(&secret)
        .arg("--json"));
    println!(
        "LOG_CHECKPOINT_JSON exit={} stdout={} stderr={}",
        out.code,
        out.stdout.trim(),
        out.stderr.trim()
    );
    assert_eq!(
        out.code, 0,
        "--json is accepted for compatibility, same as every other leaf"
    );
    let checkpoint: gx_core::Checkpoint =
        serde_json::from_value(out.json()).expect("a Checkpoint, --json or not");
    assert_eq!(checkpoint.tree_size, 3);
}

/// (g): `gx receipt verify` was the one `ReceiptCmd` leaf with no `--json`, though its siblings
/// (`show`/`coverage`) both have it.
#[test]
fn receipt_verify_accepts_json_like_its_siblings() {
    let key = keypair(22);
    let (receipt, checkpoint, _log) = commit_receipt(&key, 91, 3);
    let dir = scratch("dr1022_receipt_verify_json");
    let receipt_path = write_json(
        &dir.join("receipt.json"),
        &serde_json::to_value(&receipt).expect("a Receipt serialises"),
    );
    let checkpoint_path = write_json(
        &dir.join("checkpoint.json"),
        &serde_json::to_value(&checkpoint).expect("a Checkpoint serialises"),
    );
    let key_path = support::write_public_key(&dir, &key);

    let out = run(support::gx()
        .arg("receipt")
        .arg("verify")
        .arg(&receipt_path)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(&checkpoint_path)
        .arg("--key")
        .arg(&key_path)
        .arg("--json"));
    println!(
        "RECEIPT_VERIFY_JSON exit={} stdout={} stderr={}",
        out.code,
        out.stdout.trim(),
        out.stderr.trim()
    );
    assert_eq!(
        out.code, 0,
        "--json is accepted for compatibility, same as `receipt show`/`receipt coverage`"
    );
    assert!(
        out.json()["checks"].get("signature").is_some(),
        "the usual verify report, --json or not: {}",
        out.json()
    );
}

/// (b): `policy lint <PATH>`'s positional is a direct sibling of `submit --intent <FILE|->` and
/// `receipt verify <FILE|->`, which already read `-`. Before the repair this was a literal filename
/// `"-"` and failed `ENOENT`.
#[test]
fn policy_lint_reads_the_pack_from_stdin() {
    use std::io::Write;
    use std::process::Stdio;

    let pack = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate sits at <root>/crates/gx-cli")
            .join(gx_gate::packs::FS_PACK_PATH),
    )
    .expect("read the shipped pack");

    let mut child = support::gx()
        .arg("policy")
        .arg("lint")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn gx");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(&pack)
        .expect("write the pack to stdin");
    let out = child.wait_with_output().expect("gx runs");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    println!(
        "POLICY_LINT_STDIN exit={:?} stdout={} stderr={}",
        out.status.code(),
        stdout.trim(),
        stderr.trim()
    );
    assert_eq!(out.status.code(), Some(0), "stderr: {stderr}");
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).expect("one JSON object");
    assert_eq!(
        json["policies"]["count"], 2,
        "the same pack read from a file lints as two statements"
    );
}

/// (b): `object verify <FILE>`'s positional is the same shape.
#[test]
fn object_verify_reads_the_file_from_stdin() {
    use std::io::Write;
    use std::process::Stdio;

    let (dir, layout) = project("dr1022_object_verify_stdin");
    let key = keypair(24);
    let payload = verdict_payload(VerdictKind::Admit, &key, 93);
    let id = payload.transformation;
    let receipt = issue(&payload, &key);
    let store = gx_cli::receipt::ReceiptStore::in_layout(&layout);
    store
        .put(&id, gx_cli::receipt::StoredKind::Verdict, &receipt)
        .expect("file the receipt");

    let out_path = scratch("dr1022_object_verify_stdin_out").join("object.gx");
    let exported = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .args(["object", "export", &id.0.to_text(), "--out"])
        .arg(&out_path));
    println!(
        "OBJECT_EXPORT exit={} stderr={}",
        exported.code,
        exported.stderr.trim()
    );
    assert_eq!(exported.code, 0, "export the fixture before verifying it");

    let bytes = std::fs::read(&out_path).expect("read the exported object");
    // 🔴 Without `--key`, `object verify` looks the record's key up in *the local store*
    // (`ObjectCmd::Verify`'s own doc: "which a third party does not have") — and `keypair(24)`
    // only ever built this key in memory, never `.save()`d it anywhere. The bare `support::gx()`
    // this test uses (no fixture `HOME`) has no store at all, so an omitted `--key` here answered
    // `NOT_FOUND` ("no key for key-24") regardless of whether stdin worked, which is not what this
    // test measures. `write_public_key` is the file `--key` reads (M6H2-6, `receipt.verify`'s own
    // sibling test does the same).
    let key_path = support::write_public_key(&dir, &key);
    let mut child = support::gx()
        .arg("object")
        .arg("verify")
        .arg("-")
        .arg("--key")
        .arg(&key_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn gx");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(&bytes)
        .expect("write the object to stdin");
    let out = child.wait_with_output().expect("gx runs");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    println!(
        "OBJECT_VERIFY_STDIN exit={:?} stdout={} stderr={}",
        out.status.code(),
        stdout.trim(),
        stderr.trim()
    );
    assert_eq!(out.status.code(), Some(0), "stderr: {stderr}");
}

/// (b): `checkpoint audit <FILES>` is `Vec<PathBuf>`, and any one element may be `-`.
#[test]
fn checkpoint_audit_reads_one_file_from_stdin() {
    use std::io::Write;
    use std::process::Stdio;

    let (dir, layout) = project("dr1022_checkpoint_audit_stdin");
    let key = keypair(25);
    seed_ledger(&layout, &key, 94, 1);
    let key_dir = support::secure_scratch("dr1022_checkpoint_audit_stdin_key");
    let secret = key_dir.join("ledger.key");
    key.save(&secret).expect("save");
    let out_path = scratch("dr1022_checkpoint_audit_stdin_out").join("head.json");

    let exported = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("log")
        .arg("checkpoint")
        .arg("--key")
        .arg(&secret)
        .arg("--out")
        .arg(&out_path));
    assert_eq!(
        exported.code, 0,
        "export a checkpoint to audit: {}",
        exported.stderr
    );

    let bytes = std::fs::read(&out_path).expect("read the exported checkpoint");
    let mut child = support::gx()
        .arg("checkpoint")
        .arg("audit")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn gx");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(&bytes)
        .expect("write the checkpoint to stdin");
    let out = child.wait_with_output().expect("gx runs");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    println!(
        "CHECKPOINT_AUDIT_STDIN exit={:?} stdout={} stderr={}",
        out.status.code(),
        stdout.trim(),
        stderr.trim()
    );
    assert_eq!(out.status.code(), Some(0), "one file, no contradiction possible: {stderr}");
}
