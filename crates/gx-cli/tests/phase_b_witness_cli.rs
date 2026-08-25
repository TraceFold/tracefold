// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **Phase B, the operator surface** (`req/682` §2-2, §2-3, §6) — `gx checkpoint audit` and
//! `gx checkpoint export --note-out`, driven as the `gx` binary.
//!
//! The detector `gx_log::detect_equivocation` and the note codec are unit-tested in
//! `crates/gx-log/tests/phase_b_witness.rs`; this suite is the call site `req/682` §6 requires, the
//! E2E that keeps the library functions from being unreachable code — an operator collecting
//! exported checkpoints and asking "did this log ever equivocate?".
//!
//! red-first: every case here fails against the binary before this lane — `checkpoint audit` and
//! `--note-out` are `clap` errors, and `ledger::audit` / `checkpoint_note_body` do not compile — so
//! the suite is red until the verbs exist. The negative control (AC-B3) is what makes it more than
//! a smoke test: agreeing copies must return exit 0, or the audit cries contradiction where none is.

mod support;

use std::path::{Path, PathBuf};

use gx_witness::KeyPair;

use support::{keypair, run, scratch, secure_scratch, write_public_key};

/// A project seeded to `others + 1` ledger leaves whose last leaf is `seed`, with a checkpoint of it
/// signed by `key` and written to a JSON file in `out_dir`. The `others` leaves are identical across
/// calls, so two calls with different `seed`s produce two roots at the **same** tree size — the
/// equivocation shape.
fn signed_checkpoint(name: &str, key: &KeyPair, seed: u64, others: u64, out_dir: &Path) -> PathBuf {
    let (dir, layout) = support::project(name);
    let _ = support::seed_ledger(&layout, key, seed, others);
    let secret = secure_scratch(&format!("{name}_key")).join("ledger.key");
    key.save(&secret).expect("save the ledger key");
    let cp = out_dir.join(format!("{name}.json"));
    let out = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("log")
        .arg("checkpoint")
        .arg("--key")
        .arg(&secret)
        .arg("--out")
        .arg(&cp));
    assert_eq!(
        out.code, 0,
        "gx log checkpoint signs the tree: {}",
        out.stderr
    );
    // The verifier does not hold the signing key; only the public half reaches the audit.
    std::fs::remove_file(&secret).expect("drop the secret");
    cp
}

/// 🔴 **AC-B1, at the verb** — two collected checkpoints of one origin at one size with different
/// roots are named as equivocation, and the audit exits `VERIFY_FAILED` (7).
#[test]
fn ac_b1_audit_names_equivocation_across_collected_checkpoints() {
    let key = keypair(70);
    let out = scratch("phaseb_cli_equiv");
    let pub_path = write_public_key(&out, &key);
    // Same key, same origin, same size (8 others + 1) — different final leaf, so different roots.
    let a = signed_checkpoint("phaseb_equiv_a", &key, 101, 8, &out);
    let b = signed_checkpoint("phaseb_equiv_b", &key, 202, 8, &out);

    let audited = run(support::gx()
        .arg("checkpoint")
        .arg("audit")
        .arg(&a)
        .arg(&b)
        .arg("--key")
        .arg(&pub_path));

    assert_eq!(
        audited.code, 7,
        "equivocation is VERIFY_FAILED (7): stdout={} stderr={}",
        audited.stdout, audited.stderr
    );
    let json = audited.json();
    assert_eq!(
        json["signatures_verified"], true,
        "the key was given, so signatures were checked"
    );
    let contradictions = json["contradictions"]
        .as_array()
        .expect("a contradictions array");
    assert_eq!(contradictions.len(), 1, "one equivocation: {json}");
    assert_eq!(contradictions[0]["kind"], "equivocation");
    assert_ne!(
        contradictions[0]["root_a"], contradictions[0]["root_b"],
        "the two attested roots at one size are named and they differ"
    );
}

/// 🔴 **AC-B3, at the verb** (negative control) — agreeing copies are not a contradiction. The audit
/// exits 0 with an empty result and the soundness note.
#[test]
fn ac_b3_audit_passes_agreeing_copies() {
    let key = keypair(71);
    let out = scratch("phaseb_cli_twin");
    let pub_path = write_public_key(&out, &key);
    let a = signed_checkpoint("phaseb_twin_a", &key, 303, 8, &out);
    // Two copies of one checkpoint agree; agreement is not equivocation (mirrors ac_b3b).
    let a_copy = out.join("copy.json");
    std::fs::copy(&a, &a_copy).expect("copy the checkpoint");

    let audited = run(support::gx()
        .arg("checkpoint")
        .arg("audit")
        .arg(&a)
        .arg(&a_copy)
        .arg("--key")
        .arg(&pub_path));

    assert_eq!(
        audited.code, 0,
        "two agreeing copies are self-consistent: stdout={} stderr={}",
        audited.stdout, audited.stderr
    );
    let json = audited.json();
    assert!(
        json["contradictions"].as_array().expect("array").is_empty(),
        "no contradiction across agreeing copies: {json}"
    );
    assert_eq!(json["signatures_verified"], true);
    assert!(
        json["note"].is_string(),
        "the soundness note is present rather than implied"
    );
}

/// A checkpoint that does not verify under the key given to `--key` stops the audit rather than
/// being folded into the arithmetic — a forged checkpoint must not be able to manufacture a finding.
#[test]
fn audit_refuses_a_checkpoint_that_does_not_verify_under_the_key() {
    let real = keypair(72);
    let wrong = keypair(99);
    let out = scratch("phaseb_cli_wrongkey");
    let a = signed_checkpoint("phaseb_wrongkey_a", &real, 404, 8, &out);
    let wrong_pub = write_public_key(&out, &wrong);

    let audited = run(support::gx()
        .arg("checkpoint")
        .arg("audit")
        .arg(&a)
        .arg("--key")
        .arg(&wrong_pub));

    assert_ne!(
        audited.code, 0,
        "a checkpoint that does not verify under the given key is not evidence: {} {}",
        audited.stdout, audited.stderr
    );
}

/// 🔴 **AC-B5, the round-trip** — the note body carries exactly the signed core, and the core
/// survives a `body -> parse` round-trip well enough that the original signature still verifies over
/// it.
#[test]
fn ac_b5_note_body_round_trips_the_signed_core() {
    use gx_cli::ledger::{checkpoint_note_body, parse_checkpoint_note};

    let key = keypair(73);
    let mut log = gx_log::TileLog::new();
    for i in 0..5u64 {
        log.append(
            gx_core::TransformationId(gx_core::Cid([i as u8; 32])),
            gx_core::Cid([(i + 1) as u8; 32]),
            gx_core::Timestamp(i as i64),
        )
        .expect("canonical");
    }
    let unsigned =
        gx_log::proof::unsigned_checkpoint(&log, "glovrex-ledger/v1", gx_core::Timestamp(2))
            .expect("a non-empty log has a head");
    let signed = gx_witness::dsse::sign_checkpoint(&unsigned, key.signing_key(), key.key_id())
        .expect("sign the checkpoint");

    let body = checkpoint_note_body(&signed);
    // Three newline-terminated lines: origin, size, base64 root (C2SP tlog-checkpoint body).
    assert_eq!(
        body.lines().count(),
        3,
        "a C2SP checkpoint body is three lines: {body:?}"
    );

    let parsed = parse_checkpoint_note(&body).expect("the body round-trips");
    assert_eq!(parsed.origin, signed.origin);
    assert_eq!(parsed.tree_size, signed.tree_size);
    assert_eq!(parsed.root_hash, signed.root_hash);

    // The three parsed fields are exactly what the signature covers, so a checkpoint rebuilt from
    // them still verifies under the key. (The timestamp is unsigned advisory, CM-5, and outside the
    // note; carrying the original one changes nothing the signature protected.)
    let reconstructed = gx_core::Checkpoint {
        origin: parsed.origin,
        tree_size: parsed.tree_size,
        root_hash: parsed.root_hash,
        timestamp: signed.timestamp,
        signature: signed.signature.clone(),
    };
    gx_witness::dsse::verify_checkpoint(&reconstructed, &key.verifying())
        .expect("the signed core survives the note round-trip");
}

/// 🔴 **AC-B5, the export** — `gx checkpoint export --note-out` writes a C2SP body beside the JSON,
/// its three fields match the JSON export, and the JSON export audits as one self-consistent
/// checkpoint.
#[test]
fn ac_b5_export_writes_a_c2sp_note_body_beside_the_json() {
    let project = support::pipeline("phaseb_export_note", "before\n");
    project.commit_one("first");
    let out = scratch("phaseb_export_note_out");
    let cp = out.join("cp.json");
    let note = out.join("cp.note");

    let exported = run(project
        .gx()
        .arg("checkpoint")
        .arg("export")
        .arg(&cp)
        .arg("--note-out")
        .arg(&note));
    assert_eq!(
        exported.code, 0,
        "export: stdout={} stderr={}",
        exported.stdout, exported.stderr
    );
    assert!(cp.is_file(), "the JSON export was written");
    assert!(note.is_file(), "the note body was written");
    assert_eq!(
        exported.json()["note_exported_to"].as_str(),
        Some(note.display().to_string().as_str()),
        "the report names where the note went"
    );

    let body = std::fs::read_to_string(&note).expect("read the note");
    let parsed =
        gx_cli::ledger::parse_checkpoint_note(&body).expect("the note is a valid C2SP body");
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&cp).expect("read the JSON export")).expect("JSON");
    assert_eq!(parsed.origin, json["origin"].as_str().expect("origin"));
    assert_eq!(parsed.tree_size, json["tree_size"].as_u64().expect("size"));
    assert_eq!(
        parsed.root_hash.to_text(),
        json["root_hash"].as_str().expect("root"),
        "the note's root is the same root the JSON export names"
    );

    // A single genuine checkpoint audits clean: one checkpoint cannot contradict itself.
    let audited = run(support::gx().arg("checkpoint").arg("audit").arg(&cp));
    assert_eq!(
        audited.code, 0,
        "one exported checkpoint is self-consistent: {} {}",
        audited.stdout, audited.stderr
    );
    assert_eq!(
        audited.json()["signatures_verified"],
        false,
        "no key was given to this audit"
    );
}
