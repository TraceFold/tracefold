// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **AC-057** (FR-052, "the third-party offline-verification crossover's centerpiece, P-7"; sem: SEM-gx-cli-1071) — two cases, no network.
//!
//! AC-057 verbatim: "Given: export a Committed Transformation's Receipt (DSSE-signed, with an
//! inclusion proof) to a JSON file, and place it in an independent environment with network access
//! completely blocked (e.g. an `unshare --net` container). When: run an offline verification binary
//! (`gx-cli receipt verify --offline` or a newly-established `gx-verify`, either implementation
//! form), without connecting to the gx server, gx-log, or gx-gate at all. Then: signature verification,
//! inclusion proof verification, and canonical CID-consistency checking all pass and it returns
//! `valid`. A receipt tampered by 1 bit still has its tampering detected and fails, even under the same network-blocked environment." (sem: SEM-gx-cli-1070) Judgment method `integration (network-blocked environment E2E, 2 cases)`, M6.
//!
//! # 🔴 The Given did not exist before this hand
//!
//! "a known `Checkpoint`" (sem: SEM-gx-cli-1072) has to be produced by somebody, and req/88 §4 M6-24 measured that
//! `gx_witness::dsse::sign_checkpoint` had **zero callers outside gx-witness**: no signed checkpoint
//! existed anywhere in the shipping code, so AC-057's Given could not be constructed. §47 adopted
//! (b) — the CLI issues one on request — and `gx log checkpoint` is that producer. This suite builds
//! its own Given with it, which is why the first probe runs three commands and not one.
//!
//! # "an independent environment with network access completely blocked" (sem: SEM-gx-cli-1073), twice
//!
//! Two claims, and they are different:
//!
//! 1. **the run**: `tools/verify_m6h2.sh` executes this suite's binary under `unshare -rn`, a
//!    namespace with no interfaces at all (measured: `ip -o link` prints zero lines inside it), and
//!    reports the exit status. That is the E2E the AC's judgment method (sem: SEM-gx-cli-1074) asks for, and it is a *run* rather
//!    than an assertion, so it lives in the battery;
//! 2. **the structure**: [`the_offline_verifier_links_no_network_stack`] asserts that the binary
//!    doing the verifying cannot reach a network in the first place. A test that passed under
//!    `unshare` proves the code did not need the network **on that path, on that input**; a
//!    dependency closure with no networking crate in it is the stronger and more durable statement,
//!    and it is the one that keeps being true when hand 6 adds `gx serve`.
//!
//! The judgment (sem: SEM-gx-cli-1075) belongs to (1). (2) exists because req/88 §6.0-11's lesson is that a check for absence
//! is empty unless something would fire.

mod support;

use support::{keypair, run, scratch, tamper_payload_byte, write_json, write_public_key};

/// Build AC-057's Given: a project with a ledger, a `CommitReceipt` exported to JSON, a signed
/// checkpoint, and a public key — all four in a directory that is not a gx project.
///
/// The export directory deliberately holds **no `.gx/`**. AC-057's verifier is a third party and a
/// third party has no project; a verifier that opened one on the way in would fail in the one place
/// it has to work.
fn given(
    name: &str,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let (project_dir, layout) = support::project(&format!("{name}_project"));
    let export = scratch(&format!("{name}_export"));
    let key = keypair(9);

    // A real ledger file with the receipt's leaf in it, and eight others so the audit path is walked.
    let (receipt, index) = support::seed_ledger(&layout, &key, 57, 8);
    println!("AC057_LEAF_INDEX={index}");

    // The receipt, exported to JSON (AC-057's "export to a JSON file"; sem: SEM-gx-cli-1076).
    let receipt_path = write_json(
        &export.join("receipt.json"),
        &serde_json::to_value(&receipt).expect("serialises"),
    );

    // 🔴 The checkpoint, produced by `gx log checkpoint` — M6-24 adopted (b)'s first caller of (sem: SEM-gx-cli-1077)
    // `sign_checkpoint` outside gx-witness. The signing key is the ledger's owner's, which is the
    // whole of §47's "only the ledger's owner can make one" (sem: SEM-gx-cli-1078).
    let secret = support::secure_scratch(&format!("{name}_key")).join("ledger.key");
    key.save(&secret).expect("save the ledger key");
    let checkpoint_path = export.join("checkpoint.json");
    let out = run(support::gx()
        .arg("--project")
        .arg(&project_dir)
        .arg("log")
        .arg("checkpoint")
        .arg("--key")
        .arg(&secret)
        .arg("--out")
        .arg(&checkpoint_path));
    println!("AC057_CHECKPOINT exit={} {}", out.code, out.json());
    assert_eq!(out.code, 0, "the checkpoint producer runs: {}", out.stderr);
    assert!(checkpoint_path.is_file(), "--out wrote the head");

    let key_path = write_public_key(&export, &key);
    // The secret has no business in the verifier's environment. Removing it is what makes the two
    // probes below a *third party's* run rather than the owner's.
    std::fs::remove_file(&secret).expect("the verifier does not hold the signing key");

    (export, receipt_path, checkpoint_path, key_path)
}

/// 🔴 AC-057 case 1: the untampered receipt verifies, with no ledger, no server and no project.
#[test]
fn ac_057_case_one_an_exported_receipt_verifies_offline_against_a_known_checkpoint() {
    let (export, receipt_path, checkpoint_path, key_path) = given("ac057_pass");
    assert!(
        !export.join(".gx").exists(),
        "the verifier's directory is not a gx project"
    );

    let out = run(support::gx()
        .current_dir(&export)
        .arg("receipt")
        .arg("verify")
        .arg(&receipt_path)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(&checkpoint_path)
        .arg("--key")
        .arg(&key_path)
        // No home either: a third party has no `~/.gx/keys/`, and `--key` is what stands in for it.
        .env("HOME", export.join("no-such-home")));
    let json = out.json();
    println!(
        "AC057_CASE1 exit={} {json} stderr={:?}",
        out.code,
        out.stderr.trim()
    );

    assert_eq!(
        out.code, 0,
        "AC-057's \"returns `valid`\" (sem: SEM-gx-cli-1079) is 44 §1.2's `0=valid`"
    );
    assert_eq!(json["valid"], serde_json::json!(true));
    assert_eq!(
        json["checks"]["signature"],
        serde_json::json!(true),
        "AC-057's \"signature verification\" (sem: SEM-gx-cli-1080)"
    );
    assert_eq!(
        json["checks"]["canonical_cid"],
        serde_json::json!(true),
        "AC-057's \"canonical CID-consistency checking\" (sem: SEM-gx-cli-1081)"
    );
    assert_eq!(
        json["checks"]["inclusion"],
        serde_json::json!("verified"),
        "AC-057's \"inclusion proof verification\" (sem: SEM-gx-cli-1082), against the checkpoint the file carries"
    );
    assert_eq!(json["anchor"], serde_json::json!("checkpoint-file"));
}

/// 🔴 AC-057 case 2: "a receipt tampered by 1 bit still has its tampering detected and fails, even under the same network-blocked environment" (sem: SEM-gx-cli-1083).
#[test]
fn ac_057_case_two_a_one_bit_tamper_fails_in_the_same_environment() {
    let (export, receipt_path, checkpoint_path, key_path) = given("ac057_fail");

    let raw = std::fs::read(&receipt_path).expect("read the exported receipt");
    let clean: gx_witness::Receipt = serde_json::from_slice(&raw).expect("it is a receipt");
    let tampered = tamper_payload_byte(&clean);
    let tampered_path = write_json(
        &export.join("tampered.json"),
        &serde_json::to_value(&tampered).expect("serialises"),
    );

    let out = run(support::gx()
        .current_dir(&export)
        .arg("receipt")
        .arg("verify")
        .arg(&tampered_path)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(&checkpoint_path)
        .arg("--key")
        .arg(&key_path)
        .env("HOME", export.join("no-such-home")));
    let json = out.json();
    println!("AC057_CASE2 exit={} {json}", out.code);

    assert_eq!(
        out.code, 7,
        "44 §1.2: \"7=invalid (a mismatch in signature/CID/inclusion)\" (sem: SEM-gx-cli-1084)"
    );
    assert_eq!(json["valid"], serde_json::json!(false));
    assert_eq!(
        json["checks"]["signature"],
        serde_json::json!(false),
        "the flip is inside the signed payload, so the signature is what refused"
    );

    // The same command, same environment, on the untouched file: without this the probe above is
    // satisfied by a verifier that fails on everything.
    let out = run(support::gx()
        .current_dir(&export)
        .arg("receipt")
        .arg("verify")
        .arg(&receipt_path)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(&checkpoint_path)
        .arg("--key")
        .arg(&key_path)
        .env("HOME", export.join("no-such-home")));
    println!("AC057_CASE2_CONTROL exit={}", out.code);
    assert_eq!(out.code, 0, "the untampered receipt still verifies");
}

/// 🔴 An unanchored `CommitReceipt` is **not** a pass (H5-9), and 44's `--offline` permits one.
///
/// `gx receipt verify --offline` with no `--checkpoint` is a legal invocation that checks the
/// signature and cannot check the ledger claim. Reporting that as `valid` would be the fail-open
/// req/29 §4 forbids — "skip and pass do not wear the same face" (sem: SEM-gx-cli-1085) — and it would make AC-057's first case
/// passable **without the checkpoint producer**, i.e. without M6-24.
#[test]
fn ac_057_without_a_checkpoint_the_ledger_claim_is_unanchored_and_not_a_pass() {
    let (export, receipt_path, _checkpoint_path, key_path) = given("ac057_unanchored");

    let out = run(support::gx()
        .current_dir(&export)
        .arg("receipt")
        .arg("verify")
        .arg(&receipt_path)
        .arg("--offline")
        .arg("--key")
        .arg(&key_path)
        .env("HOME", export.join("no-such-home")));
    let json = out.json();
    println!("AC057_UNANCHORED exit={} {json}", out.code);
    assert_eq!(json["checks"]["signature"], serde_json::json!(true));
    assert_eq!(json["checks"]["inclusion"], serde_json::json!("unanchored"));
    assert_eq!(
        json["valid"],
        serde_json::json!(false),
        "H5-9: \"**do not let Unanchored pass**\" (sem: SEM-gx-cli-1086)"
    );
    assert_eq!(out.code, 7);
}

/// 🔴 What the verifier's binary links, now that `gx serve` exists — the number M6-15's D-7 needs.
///
/// **M6-15 adopted (a)** ships the offline verifier as a subcommand of the single binary, and §47
/// registered the fallback (a separate `gx-verify` crate) against a firing condition: "measured at AC-057's E2E
/// that the `gx` binary's dependency closure blocks third-party acceptance" (sem: SEM-gx-cli-1087). Hand 2 wrote this probe when
/// the closure held **no** networking crate at all and predicted its own obsolescence — "When hand 6
/// adds `tokio` for `gx serve`, this probe turns red".
///
/// Hand 6 did, and the prediction was half right, which is why the probe is rewritten rather than
/// deleted. `gx-cli` still declares no networking crate: the runtime lives in gx-api
/// (44 §1.1 makes `serve` a CLI verb, 41 §2 makes the axum surface gx-api's) and `gx_api::serve`
/// blocks. But the **closure** of the `gx` binary now contains axum, tokio, hyper and tower —
/// measured with `tools/m6h6_dep_probe.sh` §6: **126 → 167 packages**, against a verification core
/// (`gx-canon`'s closure) of **41**.
///
/// So the claim worth keeping is not "there is no network stack" (sem: SEM-gx-cli-1088) — that stopped being true and
/// pretending otherwise would be the §30 disease, an absence probe reporting 0 because of where it
/// looks. It is **"the network stack arrives through exactly one declared edge, and that edge is the
/// one 44 §1.1 names"** (sem: SEM-gx-cli-1089): `gx-cli -> gx-api`, because `gx serve` starts gx-api. A networking crate
/// declared anywhere else in this workspace's shipping graph would be a second road, and a second
/// road is the thing nobody would notice.
///
/// 🔴 The condition has **not** fired: no third party has been blocked by the closure, and §3 Λ6's
/// claim ("the distribution form doesn't change how much verification happens; what changes is what the verifier is made to install"; sem: SEM-gx-cli-1090) now has its number
/// on the D-7 ledger rather than an action. Raised as **M6H6-10**.
#[test]
fn the_network_stack_reaches_the_verifier_through_exactly_one_declared_edge() {
    /// Crates that can open a socket, by name.
    const NETWORKED: [&str; 7] = ["axum", "tokio", "hyper", "tower", "reqwest", "ureq", "curl"];

    /// The `[dependencies]` section of one manifest — shipping edges only, never dev.
    fn shipping_deps(manifest: &str) -> String {
        manifest
            .split(
                "
[dependencies]",
            )
            .nth(1)
            .map(|rest| {
                rest.split(
                    "
[",
                )
                .next()
                .unwrap_or(rest)
                .to_string()
            })
            .unwrap_or_default()
    }

    fn declares(deps: &str, name: &str) -> bool {
        deps.lines()
            .map(str::trim)
            .filter(|l| !l.starts_with('#'))
            .any(|l| l.starts_with(name))
    }

    let crates_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/");
    let mut declarers: Vec<(String, Vec<&str>)> = Vec::new();
    for entry in std::fs::read_dir(crates_dir).expect("read crates/") {
        let dir = entry.expect("an entry").path();
        let manifest_path = dir.join("Cargo.toml");
        let Ok(manifest) = std::fs::read_to_string(&manifest_path) else {
            continue;
        };
        let deps = shipping_deps(&manifest);
        let found: Vec<&str> = NETWORKED
            .into_iter()
            .filter(|name| declares(&deps, name))
            .collect();
        if !found.is_empty() {
            let name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            declarers.push((name, found));
        }
    }
    declarers.sort();
    println!(
        "NETWORK_DECLARERS={declarers:?} SCANNED={}",
        NETWORKED.len()
    );

    let names: Vec<&str> = declarers.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        vec!["gx-api"],
        "🔴 exactly one crate in this workspace may open a socket, and it is the one 41 §2 describes          as \"axum HTTP+JSONL stream (conforming to 44)\" (sem: SEM-gx-cli-1091). Anything else here is a second road into the          verifier's binary: {declarers:?}"
    );

    // And the CLI reaches it the way 47 §1(a) says — through `gx-api`, declared, and through nothing
    // it spells itself.
    let cli = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("gx-cli has a manifest");
    let cli_deps = shipping_deps(&cli);
    let direct: Vec<&str> = NETWORKED
        .into_iter()
        .filter(|name| declares(&cli_deps, name))
        .collect();
    println!("GX_CLI_DIRECT_NETWORK_DEPENDENCIES={direct:?}");
    assert!(
        direct.is_empty(),
        "the hand-6 brief: \"gx-cli does not declare tokio (serve is on the gx-api side)\" (sem: SEM-gx-cli-1092). `gx_api::serve`          blocks and builds its own runtime precisely so that this stays true: {direct:?}"
    );
    assert!(
        declares(&cli_deps, "gx-api"),
        "...and the one edge is declared rather than transitive: `gx serve` calls `gx_api::serve`, so          47 §1(a)'s \"gx-cli embeds gx-api's functionality via gx serve\" (sem: SEM-gx-cli-1093) is a line in this manifest"
    );
}

/// 🔴 **M6H8-11 adopted (a)** (sem: SEM-gx-cli-1094) — every answer says whether the anchor was authenticated, and by default it
/// was not.
///
/// Hand 8's question (§49 reserved it for the adversarial audit): "does it accept a forged checkpoint" (sem: SEM-gx-cli-1095).
/// The answer is **yes** — `verify_offline` reads `root_hash` off the checkpoint and nothing else,
/// and `gx_witness::dsse::verify_checkpoint` had no caller in shipping code — and 44's own text is
/// satisfied by that, because 44 calls the checkpoint "known" (sem: SEM-gx-cli-1096). What was missing is the record: a
/// verifier who was handed both files by the same party got `valid: true` with no field saying that
/// the anchor was taken on trust. This probe is that field, on the passing path.
#[test]
fn an_unauthenticated_anchor_is_reported_as_unauthenticated() {
    let (export, receipt_path, checkpoint_path, key_path) = given("ac057_anchor_unauth");

    let out = run(support::gx()
        .current_dir(&export)
        .arg("receipt")
        .arg("verify")
        .arg(&receipt_path)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(&checkpoint_path)
        .arg("--key")
        .arg(&key_path)
        .env("HOME", export.join("no-such-home")));
    let json = out.json();
    println!("ANCHOR_UNAUTHENTICATED exit={} {json}", out.code);
    assert_eq!(out.code, 0, "the receipt itself verifies");
    assert_eq!(json["valid"], serde_json::json!(true));
    assert_eq!(
        json["anchor_authenticated"],
        serde_json::json!(false),
        "🔴 M6H8-11 adopted (a) (sem: SEM-gx-cli-1097): \"what was not checked\" belongs on the wire, not in a document"
    );
}

/// 🔴 **M6H8-11 adopted (b)** (sem: SEM-gx-cli-1098) — `--checkpoint-key` authenticates the anchor, and refuses a forged one.
///
/// Three runs and each is a different claim:
///
/// 1. a checkpoint whose signature was flipped is **accepted** without the flag — the audit's
///    finding, kept as a measurement so that a future change of mind about 44's "known" (sem: SEM-gx-cli-1099) is visible;
/// 2. the same forged checkpoint **with** the flag exits `7`, and the object says which half
///    refused;
/// 3. the genuine checkpoint with the flag reports `anchor_authenticated: true` — without which (2)
///    would be satisfied by a flag that refuses everything.
#[test]
fn a_checkpoint_key_authenticates_the_anchor_and_refuses_a_forged_one() {
    let (export, receipt_path, checkpoint_path, key_path) = given("ac057_anchor_key");

    // A checkpoint with one flipped byte of signature: the same tamper AC-057 case 2 applies to a
    // receipt, applied to the anchor instead.
    let mut head: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&checkpoint_path).expect("read the checkpoint"))
            .expect("it is a checkpoint");
    let sig = head["signature"]["sig"]
        .as_str()
        .expect("42 §3.11's signature")
        .to_string();
    let mut bytes = gx_core::b64::decode(&sig).expect("the signature is base64");
    bytes[0] ^= 0x01;
    head["signature"]["sig"] = serde_json::json!(gx_core::b64::encode(&bytes));
    let forged_path = write_json(&export.join("forged-checkpoint.json"), &head);

    let accepted = run(support::gx()
        .current_dir(&export)
        .arg("receipt")
        .arg("verify")
        .arg(&receipt_path)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(&forged_path)
        .arg("--key")
        .arg(&key_path)
        .env("HOME", export.join("no-such-home")));
    println!(
        "FORGED_ANCHOR_WITHOUT_KEY exit={} {}",
        accepted.code,
        accepted.json()
    );
    assert_eq!(
        accepted.code, 0,
        "🔴 the measured answer to §49's question: a checkpoint nobody authenticated is accepted, \
         because 44 §1.2 calls it \"known\" (sem: SEM-gx-cli-1100) — and the flag below is what a third party uses instead of \
         believing that word"
    );
    assert_eq!(
        accepted.json()["anchor_authenticated"],
        serde_json::json!(false)
    );

    let refused = run(support::gx()
        .current_dir(&export)
        .arg("receipt")
        .arg("verify")
        .arg(&receipt_path)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(&forged_path)
        .arg("--checkpoint-key")
        .arg(&key_path)
        .arg("--key")
        .arg(&key_path)
        .env("HOME", export.join("no-such-home")));
    let json = refused.json();
    println!("FORGED_ANCHOR_WITH_KEY exit={} {json}", refused.code);
    assert_eq!(
        refused.code, 7,
        "44 §1.2's \"7=invalid\" (sem: SEM-gx-cli-1101): {json}"
    );
    assert_eq!(json["anchor_authenticated"], serde_json::json!(false));
    assert!(
        json["refusal"]
            .as_str()
            .is_some_and(|r| r.contains("--checkpoint-key")),
        "the object says which half refused: {json}"
    );

    let authenticated = run(support::gx()
        .current_dir(&export)
        .arg("receipt")
        .arg("verify")
        .arg(&receipt_path)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(&checkpoint_path)
        .arg("--checkpoint-key")
        .arg(&key_path)
        .arg("--key")
        .arg(&key_path)
        .env("HOME", export.join("no-such-home")));
    let json = authenticated.json();
    println!("AUTHENTICATED_ANCHOR exit={} {json}", authenticated.code);
    assert_eq!(authenticated.code, 0, "the genuine head verifies: {json}");
    assert_eq!(json["valid"], serde_json::json!(true));
    assert_eq!(
        json["anchor_authenticated"],
        serde_json::json!(true),
        "🔴 the one caller of `gx_witness::dsse::verify_checkpoint` in shipping code"
    );
}

/// `--checkpoint-key` without a `--checkpoint` is a usage error, not a quiet no-op.
#[test]
fn a_checkpoint_key_with_no_checkpoint_is_refused() {
    let (export, receipt_path, _checkpoint_path, key_path) = given("ac057_anchor_key_alone");
    let out = run(support::gx()
        .current_dir(&export)
        .arg("receipt")
        .arg("verify")
        .arg(&receipt_path)
        .arg("--offline")
        .arg("--checkpoint-key")
        .arg(&key_path)
        .arg("--key")
        .arg(&key_path)
        .env("HOME", export.join("no-such-home")));
    println!(
        "ANCHOR_KEY_ALONE exit={} stderr={:?}",
        out.code,
        out.stderr.trim()
    );
    assert_eq!(
        out.code, 1,
        "discipline 52 (sem: SEM-gx-cli-1102): a usage error is 1 — authenticating an anchor that was not given is no check at all"
    );
}
