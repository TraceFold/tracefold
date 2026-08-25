// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **FR-M7-3** and **FR-M7-4** at the command line: `gx key rotate|revoke`, `gx key gen --record`,
//! and `gx receipt verify --revocations`.
//!
//! # What each of the two requirements asks for, and where it is measured here
//!
//! **FR-M7-3** (ruling #6, req/98 §3-2): "a receipt signed by a revoked key is judged
//! **invalid** by a verify after the revocation time (the retroaction range is a policy setting; the machine checks only post-setting consistency)" (sem: SEM-gx-cli-1053). The library
//! half is `crates/gx-witness/tests/revocation.rs`; this file is the operator's half — a revocation
//! an operator writes with one command, and a verification that consults it.
//!
//! **FR-M7-4** (ruling #7, req/98 §3-2): "`gx key gen --record` records the generated key's keyid into the config, so a
//! boot from a fresh volume has **zero manual copy-down steps**" (sem: SEM-gx-cli-1054). The measurement is the one M6H7-8 raised
//! (`req/95` §4 ③): `.gx/config.toml`'s `engine_signing_keyid` had a reader (`gx serve`) and **no
//! writer**, and the `scratch` container image has no shell to write one with. So the probe is not
//! "the file contains a line" (sem: SEM-gx-cli-1055) — it is that the value the writer wrote is the value the reader reads,
//! with no editor in between.
//!
//! # 44 §1.2 does not have these two verbs, and that is stated rather than smuggled
//!
//! `gx key gen|list` is the whole of 44 §1.2's key section. `rotate` and `revoke` are added by
//! ruling #6 (sem: SEM-gx-cli-1056) ("U-06/13 key rotation = adopted for M7"), which is a ruling and not this hand's idea, and they follow
//! **M6-24 adopted (b)**'s precedent for `gx log checkpoint` — a verb 44 §1.1 does not list, added because a
//! ruling required the capability and the synopsis had nowhere to put it. `--revocations` is the same
//! shape as `--checkpoint-key` (M6H8-11): a flag the AC needs and 44 §1.2 does not write.

mod support;

use std::path::{Path, PathBuf};

use gx_core::{Timestamp, VerdictKind};
use gx_witness::KeyPair;
use support::{gx, run, scratch, secure_scratch, verdict_payload, write_json, Run};

/// A second, in `Timestamp`'s units.
const SECOND: i64 = 1_000_000_000;

/// A home directory holding `~/.gx/keys/`, on a filesystem with unix permissions (M6H2-10).
fn home(name: &str) -> PathBuf {
    secure_scratch(name)
}

/// `gx key ...` with this home.
fn gx_at(home: &Path) -> std::process::Command {
    let mut cmd = gx();
    cmd.env("HOME", home).env("USERPROFILE", home);
    cmd
}

/// Generate a key through the command under test and answer its id.
fn gen(home: &Path, extra: &[&str]) -> (Run, String) {
    let mut cmd = gx_at(home);
    cmd.arg("key").arg("gen");
    for arg in extra {
        cmd.arg(arg);
    }
    let out = run(&mut cmd);
    let id = out.json()["key_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    (out, id)
}

// ---------------------------------------------------------------------------
// FR-M7-4 — the keyid gets a writer
// ---------------------------------------------------------------------------

/// 🔴 **FR-M7-4**: `gx key gen --record` records the id where `gx serve` looks for it.
///
/// The reader is `gx_cli::serve::recorded_signing_keyid` and it is the one this test uses, rather
/// than a second parser written here: what M6H7-8 found was two halves that had never met, and a
/// probe with its own third parser would leave them unmet.
#[test]
fn fr_m7_4_gen_record_writes_the_keyid_the_server_reads() {
    let home = home("m7h2_gen_record");
    let (project_dir, layout) = support::project("m7h2_gen_record_project");

    let before = gx_cli::serve::recorded_signing_keyid(&layout).expect("readable");
    assert_eq!(before, None, "a fresh project records no key id");

    let (out, key_id) = gen(
        &home,
        &["--record", "--project", &project_dir.display().to_string()],
    );
    println!(
        "FRM74_GEN_RECORD exit={} key_id={key_id} stderr={:?}",
        out.code,
        out.stderr.trim()
    );
    assert_eq!(
        out.code, 0,
        "44 §1.2: \"0=success\" (sem: SEM-gx-cli-1057). stderr: {}",
        out.stderr
    );
    assert!(
        !key_id.is_empty(),
        "the two fields 44 §1.2 fixes are still there"
    );
    assert_eq!(
        out.json()["public_key"].as_str().map(str::len).unwrap_or(0),
        44,
        "and the second one: a base64 Ed25519 public key"
    );

    let after = gx_cli::serve::recorded_signing_keyid(&layout).expect("readable");
    println!("FRM74_RECORDED={after:?}");
    assert_eq!(
        after,
        Some(key_id.clone()),
        "the writer wrote the value the reader reads (M6H7-8: \"there is a reader, but no writer\"; sem: SEM-gx-cli-1058)"
    );

    // 🔴 The whole point of the flag: no hand copying. The config file holds the id and nothing an
    // operator had to type, and the id in it is the id of a key that exists in the store.
    let store = gx_cli::keys::KeyStore::at(home.join(".gx").join("keys"));
    let ids: Vec<String> = store
        .list()
        .expect("list")
        .into_iter()
        .map(|e| e.key_id)
        .collect();
    assert!(
        ids.contains(&key_id),
        "the recorded id names a key this store holds: {ids:?}"
    );
}

/// Without `--record`, nothing is written — the flag is opt-in and the default is unchanged.
///
/// 44 §1.2's `gen` has no project at all, and a command that silently wrote into `.gx/` would be
/// changing a project an operator only meant to make a key in.
#[test]
fn gen_without_record_touches_no_project() {
    let home = home("m7h2_gen_plain");
    let (project_dir, layout) = support::project("m7h2_gen_plain_project");
    let (out, key_id) = gen(&home, &["--project", &project_dir.display().to_string()]);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert!(!key_id.is_empty());
    assert_eq!(
        gx_cli::serve::recorded_signing_keyid(&layout).expect("readable"),
        None,
        "--record is opt-in"
    );
}

/// 🔴 Recording twice replaces the value and **keeps the rest of the file**.
///
/// The ordinary rotation is "a project that already records a key id records the successor's" (sem: SEM-gx-cli-1059), and a
/// writer that truncated would take an operator's other settings with it the first time they
/// rotated. req/56 §2 gives this file one job today, which is exactly why it will be given others.
#[test]
fn recording_twice_replaces_the_value_and_keeps_the_file() {
    let home = home("m7h2_record_twice");
    let (project_dir, layout) = support::project("m7h2_record_twice_project");
    let (_, first) = gen(
        &home,
        &["--record", "--project", &project_dir.display().to_string()],
    );

    // A setting this hand knows nothing about, added by hand between the two runs.
    let config = layout.join("config.toml");
    let text = std::fs::read_to_string(&config).expect("the writer created it");
    std::fs::write(&config, format!("{text}some_other_setting = \"kept\"\n")).expect("write");

    let (_, second) = gen(
        &home,
        &["--record", "--project", &project_dir.display().to_string()],
    );
    let after = std::fs::read_to_string(&config).expect("still there");
    println!("FRM74_REWRITE first={first} second={second} file={after:?}");

    assert_ne!(first, second, "two generations, two keys");
    assert_eq!(
        gx_cli::serve::recorded_signing_keyid(&layout).expect("readable"),
        Some(second.clone()),
        "the reader sees the second id, not the first"
    );
    assert!(
        after.contains("some_other_setting = \"kept\""),
        "a writer that truncated would lose the operator's other settings: {after}"
    );
    assert_eq!(
        after.matches("engine_signing_keyid").count(),
        1,
        "one assignment, not a pile of them: the reader answers with the first it finds, so a \
         second line would be a value that looks in force and is not"
    );
    assert!(
        !after.contains(&first),
        "the superseded id is gone from the slot: {after}"
    );
}

// ---------------------------------------------------------------------------
// FR-M7-3 — revoke, rotate, and the verification that consults them
// ---------------------------------------------------------------------------

/// `gx key revoke` writes a signed entry into the store's revocation list.
#[test]
fn fr_m7_3_revoke_writes_a_signed_entry() {
    let home = home("m7h2_revoke");
    let (_, key_id) = gen(&home, &[]);

    let out = run(gx_at(&home)
        .arg("key")
        .arg("revoke")
        .arg("--key-id")
        .arg(&key_id)
        .arg("--reason")
        .arg("the laptop was lost"));
    let json = out.json();
    println!("FRM73_REVOKE exit={} {json}", out.code);
    assert_eq!(
        out.code, 0,
        "44 §1.2: \"0=success\" (sem: SEM-gx-cli-1060). stderr: {}",
        out.stderr
    );
    assert_eq!(json["key_id"], serde_json::json!(key_id));
    assert!(
        json["revoked_at"].as_i64().unwrap_or(0) > 0,
        "the moment is the verifier's own clock reading, not an argument (Rule 2; sem: SEM-gx-cli-1061)"
    );
    assert_eq!(json["entries"], serde_json::json!(1));

    let list = PathBuf::from(json["revocations"].as_str().expect("the file it wrote"));
    assert!(list.is_file(), "{} exists", list.display());

    // A second revocation of the same key is not an error and does not replace the first: the list
    // is append-only and the earliest statement is the one a verifier applies.
    let again = run(gx_at(&home)
        .arg("key")
        .arg("revoke")
        .arg("--key-id")
        .arg(&key_id)
        .arg("--reason")
        .arg("said twice"));
    println!("FRM73_REVOKE_TWICE exit={} {}", again.code, again.json());
    assert_eq!(again.code, 0);
    assert_eq!(again.json()["entries"], serde_json::json!(2));
}

/// 🔴 Revoking a key the store does not hold is **6**, not 1 (**E-M6-24**'s reading).
///
/// 44 §1.2's key section is `gen|list` and neither can name a key that is not there, so "there is no such
/// key" (sem: SEM-gx-cli-1062) becomes reachable in this section for the first time with `revoke`. §1.4's common table
/// gives not-found the code **6** and every other verb of this binary returns it; folding it into 1
/// would make a script branching on "not found" special-case the key verbs.
/// `gx_cli::exit::SPEC_44_EXIT_ADDITIONS` carries the citation, and this measures the status rather
/// than leaving that table to assert about itself.
#[test]
fn revoking_a_key_the_store_does_not_hold_is_not_found() {
    let home = home("m7h2_revoke_missing");
    let out = run(gx_at(&home)
        .arg("key")
        .arg("revoke")
        .arg("--key-id")
        .arg("ed25519-0000000000000000"));
    println!(
        "FRM73_REVOKE_MISSING exit={} stderr={:?}",
        out.code,
        out.stderr.trim()
    );
    assert_eq!(
        out.code, 6,
        "44 §1.4: \"6=not-found\" (sem: SEM-gx-cli-1063). stderr: {}",
        out.stderr
    );
    assert!(
        out.stdout.trim().is_empty(),
        "44 §1.3: a refusal writes nothing to stdout"
    );
}

/// `gx key rotate` makes the successor and revokes the predecessor, in one command.
///
/// Two verbs would leave a window in which an operator has done half of a rotation, and the half
/// they are most likely to skip is the revocation — which is the half that does anything.
#[test]
fn fr_m7_3_rotate_makes_a_successor_and_revokes_the_predecessor() {
    let home = home("m7h2_rotate");
    let (project_dir, layout) = support::project("m7h2_rotate_project");
    let (_, old) = gen(&home, &[]);

    let out = run(gx_at(&home)
        .arg("--project")
        .arg(&project_dir)
        .arg("key")
        .arg("rotate")
        .arg("--key-id")
        .arg(&old)
        .arg("--reason")
        .arg("scheduled rotation")
        .arg("--record"));
    let json = out.json();
    println!("FRM73_ROTATE exit={} {json}", out.code);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);

    let new = json["key_id"]
        .as_str()
        .expect("the successor's id")
        .to_string();
    assert_ne!(new, old, "a rotation produces a new key");
    assert_eq!(json["revoked"], serde_json::json!(old));
    assert_eq!(
        json["superseded_by"],
        serde_json::json!(new),
        "the entry says which key took over, so a reader of the list can follow the chain"
    );
    assert_eq!(
        gx_cli::serve::recorded_signing_keyid(&layout).expect("readable"),
        Some(new.clone()),
        "--record on a rotation is what makes the server pick up the successor"
    );

    let store = gx_cli::keys::KeyStore::at(home.join(".gx").join("keys"));
    let ids: Vec<String> = store
        .list()
        .expect("list")
        .into_iter()
        .map(|e| e.key_id)
        .collect();
    assert!(ids.contains(&old) && ids.contains(&new), "🔴 the old secret is **kept**: receipts it signed still have to be verifiable, and deleting it would make a revocation indistinguishable from a lost key: {ids:?}");
}

/// 🔴 **The AC**: a receipt signed after the revocation is refused; one signed before is not.
///
/// Both halves in one run, because the pair is the criterion — a verifier that refused everything
/// would pass the first half alone.
#[test]
fn fr_m7_3_verify_consults_the_revocation_list() {
    let (export, key_path, list_path, before, after) = revoked_key_fixture("m7h2_verify");

    let refused = verify(&export, &after, &key_path, Some(&list_path), None);
    println!(
        "FRM73_VERIFY_AFTER exit={} {}",
        refused.code,
        refused.json()
    );
    assert_eq!(
        refused.code, 7,
        "44 §1.2's \"7=invalid\" (sem: SEM-gx-cli-1064), the code a receipt that does not verify already had. stderr: {}",
        refused.stderr
    );
    assert_eq!(refused.json()["valid"], serde_json::json!(false));
    assert_eq!(
        refused.json()["checks"]["revocation"],
        serde_json::json!("revoked")
    );
    assert_eq!(
        refused.json()["checks"]["signature"],
        serde_json::json!(true),
        "🔴 the signature is still valid, and saying otherwise would send an operator looking for \
         tampering that did not happen"
    );

    let accepted = verify(&export, &before, &key_path, Some(&list_path), None);
    println!(
        "FRM73_VERIFY_BEFORE exit={} {}",
        accepted.code,
        accepted.json()
    );
    assert_eq!(accepted.code, 0, "stderr: {}", accepted.stderr);
    assert_eq!(
        accepted.json()["checks"]["revocation"],
        serde_json::json!("valid_at_issue"),
        "ASM-45-2's DEFAULT: \"a receipt already issued before revocation is not retroactively invalidated\" (sem: SEM-gx-cli-1065)"
    );
}

/// `--retroaction all` is the other setting, and it changes the same receipt's answer.
///
/// "the retroaction range is a policy setting; the machine checks only post-setting consistency" (sem: SEM-gx-cli-1066) — the machine checks that each
/// setting answers the way its definition says, and the choice between them is the operator's.
#[test]
fn fr_m7_3_the_retroaction_setting_is_the_operators() {
    let (export, key_path, list_path, before, _) = revoked_key_fixture("m7h2_retroaction");

    let default = verify(&export, &before, &key_path, Some(&list_path), None);
    let all = verify(&export, &before, &key_path, Some(&list_path), Some("all"));
    println!(
        "FRM73_SETTINGS default={:?} all={:?}",
        default.json()["checks"]["revocation"],
        all.json()["checks"]["revocation"]
    );
    assert_eq!(default.code, 0);
    assert_eq!(all.code, 7, "stderr: {}", all.stderr);
    assert_eq!(
        all.json()["checks"]["revocation"],
        serde_json::json!("revoked")
    );
}

/// Without `--revocations` the answer says `not_consulted`, and it is not a pass in disguise.
///
/// ASM-45-2 makes consulting the list the verifier's option — "consulting the revocation list is optional, at the verifier's discretion" (sem: SEM-gx-cli-1067) —
/// so the run succeeds; what it must not do is print a word that reads as "checked, and clean".
/// This is `anchor_authenticated`'s lesson (M6H8-11 adopted (a); sem: SEM-gx-cli-1103) applied to the second thing a verification
/// can skip: what was **not** checked belongs on the wire.
#[test]
fn a_verification_that_consults_no_list_says_so() {
    let (export, key_path, _, before, after) = revoked_key_fixture("m7h2_unconsulted");

    for receipt in [&before, &after] {
        let out = verify(&export, receipt, &key_path, None, None);
        println!(
            "FRM73_UNCONSULTED exit={} revocation={:?}",
            out.code,
            out.json()["checks"]["revocation"]
        );
        assert_eq!(out.code, 0, "stderr: {}", out.stderr);
        assert_eq!(
            out.json()["checks"]["revocation"],
            serde_json::json!("not_consulted"),
            "req/29 §4: skip (sem: SEM-gx-cli-1068) and pass do not wear the same face"
        );
    }
}

/// A revocation list naming a **different** key leaves this receipt alone.
///
/// The entry cannot be authenticated by this verifier (it holds one public key), and it says nothing
/// about this receipt's key either way. Ignoring it is not the same as accepting it: the answer is
/// `not_revoked`, which means "a list was consulted and this key is not in it" (sem: SEM-gx-cli-1069).
#[test]
fn a_list_about_other_keys_does_not_revoke_this_one() {
    let (export, key_path, _, before, _) = revoked_key_fixture("m7h2_other_keys");
    let others = write_json(
        &export.join("others.json"),
        &serde_json::json!([signed_revocation(
            &support::keypair(200),
            Timestamp(SECOND),
            "theirs"
        )]),
    );

    let out = verify(&export, &before, &key_path, Some(&others), None);
    println!("FRM73_OTHER_KEYS exit={} {}", out.code, out.json());
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert_eq!(
        out.json()["checks"]["revocation"],
        serde_json::json!("not_revoked")
    );
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A signed revocation entry, as the JSON face a list file carries.
fn signed_revocation(key: &KeyPair, at: Timestamp, reason: &str) -> serde_json::Value {
    let entry = gx_witness::keys::RevocationEntry::new(key.key_id().clone(), at, reason);
    serde_json::to_value(entry.signed_by(key).expect("encodable")).expect("serialises")
}

/// A third party's environment: two receipts by one key, that key's public document, and a
/// revocation list naming it.
///
/// The two receipts differ only in `issued_at` — one before the revocation and one after — which is
/// the pair the AC is about.
fn revoked_key_fixture(name: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
    let export = scratch(name);
    let key = support::keypair(73);
    let payload = verdict_payload(VerdictKind::Admit, &key, 73);

    let before = write_json(
        &export.join("before.json"),
        &serde_json::to_value(
            gx_witness::Receipt::issue(&payload, Timestamp(5 * SECOND), &key).expect("legal"),
        )
        .expect("serialises"),
    );
    let after = write_json(
        &export.join("after.json"),
        &serde_json::to_value(
            gx_witness::Receipt::issue(&payload, Timestamp(70 * SECOND), &key).expect("legal"),
        )
        .expect("serialises"),
    );
    let key_path = write_json(
        &export.join("key.pub.json"),
        &serde_json::json!({
            "key_id": key.key_id(),
            "public_key": gx_core::b64::encode(&key.public().to_bytes()),
        }),
    );
    let list = write_json(
        &export.join("revocations.json"),
        &serde_json::json!([signed_revocation(
            &key,
            Timestamp(10 * SECOND),
            "compromised"
        )]),
    );
    (export, key_path, list, before, after)
}

/// `gx receipt verify --offline` with an optional revocation list, from a directory that is not a
/// project and a home that holds no keys (AC-057's environment).
fn verify(
    export: &Path,
    receipt: &Path,
    key: &Path,
    revocations: Option<&Path>,
    retroaction: Option<&str>,
) -> Run {
    let mut cmd = gx();
    cmd.current_dir(export)
        .arg("receipt")
        .arg("verify")
        .arg(receipt)
        .arg("--offline")
        .arg("--key")
        .arg(key)
        .env("HOME", export.join("no-such-home"));
    if let Some(list) = revocations {
        cmd.arg("--revocations").arg(list);
    }
    if let Some(setting) = retroaction {
        cmd.arg("--retroaction").arg(setting);
    }
    run(&mut cmd)
}
