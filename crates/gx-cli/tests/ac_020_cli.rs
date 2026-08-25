// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-020, re-confirmed through the CLI** (51 §15 M6 row; sem: SEM-gx-cli-1024) — and AC-020 asked for this by name.
//!
//! AC-020 verbatim: "Given: `gx_witness::keys`'s Ed25519 key-generation library API (**at M2, verified at the
//! library level since the CLI isn't wired up yet**)... Then: the generate->save->load->verify round trip succeeds. **Re-confirmation
//! at the CLI level via `gx key gen`/`gx receipt verify` is done in M6's E2E AC (AC-054, AC-057)**." (sem: SEM-gx-cli-1025) Judgment method
//! `unit + integration (direct library API calls)`, M2.
//!
//! So the round trip is the same one M2 measured, with the two ends replaced by the commands the AC
//! named: `gx key gen` writes the key file and prints the public document, and `gx receipt verify`
//! reads that document back. What sits between them — a receipt signed by the generated key — is
//! still built in-process, because the hand that signs one from the command line is hand 3
//! (`gx commit`).
//!
//! # 🔴 `HOME` is set for the child
//!
//! req/56 §3 puts the secret in `~/.gx/keys/`, so a suite that generated keys would otherwise write
//! into the operator's own store. The child process gets a scratch `HOME`, which is also what makes
//! the round trip observable: the key the second command loads is the file the first command wrote.

mod support;

use gx_witness::KeyPair;
use support::{issue, run, scratch, secure_scratch, verdict_payload, write_json};

/// generate -> save -> load -> verify (sem: SEM-gx-cli-1026), each arrow a process boundary where the AC allows one.
#[test]
fn ac_020_cli_generate_save_load_verify_round_trips() {
    let home = secure_scratch("ac020_cli_home");
    let work = scratch("ac020_cli_work");

    // generate + save (sem: SEM-gx-cli-1027): `gx key gen`.
    let out = run(support::gx().arg("key").arg("gen").env("HOME", &home));
    let doc = out.json();
    println!(
        "AC020_CLI_GEN exit={} {doc} stderr={:?}",
        out.code,
        out.stderr.trim()
    );
    assert_eq!(
        out.code, 0,
        "44 §1.2: \"exit: 0=success\" (sem: SEM-gx-cli-1028)"
    );
    let key_id = doc["key_id"].as_str().expect("a key id").to_string();
    assert!(
        key_id.starts_with("ed25519-"),
        "the id names the algorithm it was made with: {key_id}"
    );
    assert!(
        doc["public_key"].as_str().is_some(),
        "44 §1.2's second field"
    );

    // The file req/56 §3 asks for, where it asks for it.
    let filed = home.join(".gx").join("keys").join(format!("{key_id}.key"));
    assert!(
        filed.is_file(),
        "req/56 §3: \"secret key = `~/.gx/keys/`\" (sem: SEM-gx-cli-1029) — {}",
        filed.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&filed)
            .expect("stat")
            .permissions()
            .mode()
            & 0o777;
        println!("AC020_CLI_MODE={mode:o}");
        assert_eq!(
            mode & 0o077,
            0,
            "req/56 §3: \"0600\" (sem: SEM-gx-cli-1030)"
        );
    }

    // load (sem: SEM-gx-cli-1031): the same file, through the library the CLI wrote it with, and a receipt signed by it.
    let pair = KeyPair::load(&filed).expect("the file the CLI wrote loads back");
    assert_eq!(
        pair.key_id(),
        &key_id,
        "the file name and the id inside agree"
    );
    let receipt = issue(
        &verdict_payload(gx_core::VerdictKind::Admit, &pair, 400),
        &pair,
    );
    let receipt_path = write_json(
        &work.join("receipt.json"),
        &serde_json::to_value(&receipt).expect("serialises"),
    );

    // verify (sem: SEM-gx-cli-1032): `gx receipt verify`, resolving the key **out of the store** rather than from `--key`.
    // That is the path 44 §1.2's synopsis describes (it has no key argument at all), and it is the
    // only one available to it — see M6H2-6 for why a third party needs the flag instead.
    let out = run(support::gx()
        .arg("receipt")
        .arg("verify")
        .arg(&receipt_path)
        .arg("--offline")
        .env("HOME", &home));
    let json = out.json();
    println!("AC020_CLI_VERIFY exit={} {json}", out.code);
    assert_eq!(
        out.code, 0,
        "AC-020's \"the round trip succeeds\" (sem: SEM-gx-cli-1033)"
    );
    assert_eq!(json["valid"], serde_json::json!(true));
    assert_eq!(json["key_id"], serde_json::json!(key_id));

    // 🔴 The public document the first command printed is the one `--key` reads, which is what
    // makes AC-057's third party possible at all.
    let pub_path = write_json(&work.join("key.pub.json"), &doc);
    let out = run(support::gx()
        .arg("receipt")
        .arg("verify")
        .arg(&receipt_path)
        .arg("--offline")
        .arg("--key")
        .arg(&pub_path)
        .env("HOME", secure_scratch("ac020_cli_empty_home")));
    println!("AC020_CLI_VERIFY_WITH_DOC exit={} {}", out.code, out.json());
    assert_eq!(
        out.code, 0,
        "the `{{key_id, public_key}}` document 44 §1.2 specifies is a usable verification key"
    );
}

/// `gx key list` sees the key `gx key gen` filed, and says whether anybody else can read it.
///
/// M6-29 adopted (a): "the CLI reads the dir... **and warns if it finds a file that is not 0600** = the CLI
/// version of witness's `KeyPermissions` error" (sem: SEM-gx-cli-1034). The warning does not fail the command: an operator debugging a
/// permissions problem is exactly who runs `list`, and a `list` that refused would leave them with
/// nothing.
#[test]
fn ac_020_cli_list_reports_the_store_and_the_permissions() {
    let home = secure_scratch("ac020_cli_list_home");
    let empty = run(support::gx().arg("key").arg("list").env("HOME", &home));
    println!("AC020_CLI_LIST_EMPTY {}", empty.json());
    assert_eq!(empty.code, 0, "no keys yet is not a failure");
    assert_eq!(empty.json()["count"], serde_json::json!(0));

    let gen = run(support::gx().arg("key").arg("gen").env("HOME", &home));
    let key_id = gen.json()["key_id"].as_str().expect("id").to_string();
    let listed = run(support::gx().arg("key").arg("list").env("HOME", &home));
    let json = listed.json();
    println!("AC020_CLI_LIST {json}");
    assert_eq!(json["count"], serde_json::json!(1));
    assert_eq!(json["keys"][0]["key_id"], serde_json::json!(key_id));
    assert_eq!(json["insecure"], serde_json::json!(0));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let filed = home.join(".gx").join("keys").join(format!("{key_id}.key"));
        std::fs::set_permissions(&filed, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        let listed = run(support::gx().arg("key").arg("list").env("HOME", &home));
        let json = listed.json();
        println!("AC020_CLI_LIST_INSECURE {json}");
        assert_eq!(listed.code, 0, "the warning does not fail the listing");
        assert_eq!(json["insecure"], serde_json::json!(1));
        assert_eq!(json["keys"][0]["permissions_ok"], serde_json::json!(false));
    }
}
