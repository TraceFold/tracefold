// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-44-4** (`req/38` §144 item 3, `req/207` §1-4) — what an auditor's machine has to be,
//! asserted as a **process**: nothing but three files and one binary.
//!
//! # The claim, and why it needed a suite of its own
//!
//! `req/207` §1-3 measured that `gx receipt verify --offline --checkpoint <file> --key <file>` does
//! not open a `Layout` (`crates/gx-cli/src/main.rs`'s `--offline` + `--checkpoint` branch), so a
//! third party with no `.gx/`, no key store and no server already gets an answer. That is a fact a
//! reader can derive from the source. It was not a fact **anything in this repository asserted**:
//! `ac_057.rs` runs its verifications inside a scratch directory it built, with the test process's
//! own environment inherited, so what it measures is "this input verifies", not "this is all the
//! environment the verifier is given". The difference is `req/188` §3's: having a property and
//! having a machine that says so are two states, and only the second one survives a refactor.
//!
//! So the environment is the subject here. Every probe below runs the binary with
//! [`std::process::Command::env_clear`] — the child's whole environment is one variable, `HOME`,
//! pointing at an **empty** directory — from a working directory that is empty and holds no `.gx/`,
//! and then reads both directories back afterwards and requires them to still be empty. A verifier
//! that reached for a key store, a workspace, a cache or a config file would either fail (because
//! nothing is there) or leave a trace (because it created one), and both are visible here.
//!
//! # 🔴 The negative half comes from a competitor's implementation
//!
//! `req/207` §1-2 read `EctoSpace/EctoLedger`'s `verify-cert` (BUSL-1.1, observed only — no code is
//! copied here, and none of its verification logic is imitated) and found three fail-open shapes,
//! the sharpest being that a certificate **with its signature field removed** prints a warning and
//! exits `0`. [`a_receipt_with_its_signature_removed_does_not_exit_zero`] is the direct inversion of
//! that, kept here permanently: an absent signature is not a weaker check, it is no check, and
//! `44` §1.4's `7` is the answer. [`an_unanchored_commit_receipt_does_not_exit_zero`] is the second
//! inversion — their anchor (an OpenTimestamps stamp) is non-fatal, ours is
//! `crates/gx-cli/src/receipt.rs`'s H5-9: a `CommitReceipt` verified against no anchor reports
//! `inclusion: unanchored` and `Checks::verified` refuses to call that a pass.
//!
//! # What "no network" is asserted as, and what it is not
//!
//! Not by sandboxing: a probe whose claim rests on `unshare` measures the sandbox on the days the
//! sandbox exists and measures nothing on the days it does not. The standing claim is made of two
//! pieces that hold everywhere — (i) the child is given **no environment at all** beyond `HOME`, so
//! there is no address, endpoint, proxy or socket path it could have been told about, and it answers
//! anyway; and (ii) neither directory it can see gains a file, so no server state, cache, lock or
//! socket was created. `gx serve` is the one verb in this binary that binds a port (44 §1.1) and it
//! is not the verb under test; `ac_057.rs`'s dependency-closure probe holds the complementary
//! structural claim, that the network stack reaches this binary through exactly one declared edge.
//!
//! A network namespace is used **in addition** where the host offers one, and its availability is
//! printed as `NETWORK_NAMESPACE=` rather than skipped in silence: the denominator of the run is
//! part of the run.

mod support;

use std::path::{Path, PathBuf};

use support::{keypair, run, scratch, secure_scratch, write_json, write_public_key, Run};

/// The whole of what a third party holds, and the two empty directories the run happens in.
struct Hermetic {
    /// An empty working directory with **no** `.gx/`: the auditor is not in a project.
    cwd: PathBuf,
    /// An empty `HOME`: the auditor has no `~/.gx/keys/`, which is what `--key` stands in for.
    home: PathBuf,
    /// The directory holding the three files, so that the two above can be asserted empty.
    inputs: PathBuf,
    /// The exported `CommitReceipt`.
    receipt: PathBuf,
    /// The signed checkpoint the ledger's owner published.
    checkpoint: PathBuf,
    /// The owner's public key, in `gx key gen`'s shape.
    key: PathBuf,
}

impl Hermetic {
    /// The binary, with **nothing** in its environment but `HOME`, run from the empty directory.
    ///
    /// `env_clear` is the assertion: `PATH`, `TMPDIR`, `XDG_*`, every `GX_*` and every proxy
    /// variable are gone, so an implementation that found a server, a cache or a key store to talk
    /// to found it without being told, and an implementation that needed one fails here.
    fn command(&self) -> std::process::Command {
        let mut cmd = support::gx();
        cmd.env_clear();
        cmd.env("HOME", &self.home);
        cmd.current_dir(&self.cwd);
        cmd
    }

    /// `gx receipt verify <file> --offline --checkpoint <cp> --key <pub>` — the third party's line.
    fn verify_anchored(&self, receipt: &Path) -> Run {
        run(self
            .command()
            .arg("receipt")
            .arg("verify")
            .arg(receipt)
            .arg("--offline")
            .arg("--checkpoint")
            .arg(&self.checkpoint)
            .arg("--key")
            .arg(&self.key))
    }

    /// The same, with no anchor at all: 44 §1.2 permits it and H5-9 refuses to call it a pass.
    fn verify_unanchored(&self, receipt: &Path) -> Run {
        run(self
            .command()
            .arg("receipt")
            .arg("verify")
            .arg(receipt)
            .arg("--offline")
            .arg("--key")
            .arg(&self.key))
    }

    /// The same, authenticating the anchor itself (M6H8-11 adopted (b)).
    fn verify_with_authenticated_anchor(&self, receipt: &Path, checkpoint: &Path) -> Run {
        run(self
            .command()
            .arg("receipt")
            .arg("verify")
            .arg(receipt)
            .arg("--offline")
            .arg("--checkpoint")
            .arg(checkpoint)
            .arg("--checkpoint-key")
            .arg(&self.key)
            .arg("--key")
            .arg(&self.key))
    }

    /// The exported receipt, as JSON, for a probe that is about to take something out of it.
    fn receipt_json(&self) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(&self.receipt).expect("read the exported receipt"))
            .expect("the export is a receipt")
    }
}

/// What a directory holds, sorted — the shape both "nothing was created" assertions compare.
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("the directory exists")
        .map(|entry| {
            entry
                .expect("a directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// Build the Given: a real ledger, a real inclusion proof, a signed checkpoint, a public key — and
/// then three empty directories, which is the part this suite is about.
///
/// The producing half runs with the ordinary environment because it is the **owner's** half: minting
/// a checkpoint takes the signing key and a project. What is hermetic is the verifying half, and the
/// signing key is deleted before it starts.
fn given(name: &str) -> Hermetic {
    let (project_dir, layout) = support::project(&format!("{name}_project"));
    let inputs = scratch(&format!("{name}_inputs"));
    let cwd = scratch(&format!("{name}_cwd"));
    let home = scratch(&format!("{name}_home"));
    let key = keypair(44);

    let (receipt, index) = support::seed_ledger(&layout, &key, 444, 8);
    println!("DR44_4_LEAF_INDEX={index}");

    let receipt_path = write_json(
        &inputs.join("receipt.json"),
        &serde_json::to_value(&receipt).expect("serialises"),
    );

    let secret = secure_scratch(&format!("{name}_key")).join("ledger.key");
    key.save(&secret).expect("save the ledger key");
    let checkpoint_path = inputs.join("checkpoint.json");
    let out = run(support::gx()
        .arg("--project")
        .arg(&project_dir)
        .arg("log")
        .arg("checkpoint")
        .arg("--key")
        .arg(&secret)
        .arg("--out")
        .arg(&checkpoint_path));
    assert_eq!(out.code, 0, "the checkpoint producer runs: {}", out.stderr);

    let key_path = write_public_key(&inputs, &key);
    // The auditor does not hold the signing key, and a fixture that left it lying beside the public
    // one would be measuring the owner's environment under another name.
    std::fs::remove_file(&secret).expect("the verifier does not hold the signing key");

    Hermetic {
        cwd,
        home,
        inputs,
        receipt: receipt_path,
        checkpoint: checkpoint_path,
        key: key_path,
    }
}

/// Whether this host lets an unprivileged process build a network namespace.
///
/// Reported, never assumed. Where it is true the passing case is run a second time inside a
/// namespace with no interfaces at all; where it is false the standing claim is the cleared
/// environment and the two empty directories, which is what every host can measure.
fn network_namespace_available() -> bool {
    std::process::Command::new("unshare")
        .args(["-rn", "true"])
        .status()
        .is_ok_and(|status| status.success())
}

/// 🔴 **DR-44-4, probe 1** — the passing case, with no `HOME`, no workspace and no environment.
///
/// Three files and one binary, and the two directories the process can see are still empty when it
/// exits: no `.gx/`, no key store, no cache, no socket, no lock. This is the sentence `req/188` §9-3
/// item 3 asks a buyer to be able to check ("a third party verifies a receipt with one command")
/// turned into an exit status.
#[test]
fn verify_offline_runs_with_no_home_no_workspace_no_network() {
    let h = given("dr44_4_pass");
    assert!(
        !h.cwd.join(".gx").exists(),
        "the auditor's working directory is not a gx project"
    );
    assert!(entries(&h.home).is_empty(), "and HOME starts empty");

    let out = h.verify_anchored(&h.receipt);
    let json = out.json();
    println!(
        "HERMETIC_PASS exit={} {json} stderr={:?}",
        out.code,
        out.stderr.trim()
    );

    assert_eq!(
        out.code, 0,
        "44 §1.4's `0`, from an environment holding one variable: {}",
        out.stderr
    );
    assert_eq!(json["valid"], serde_json::json!(true));
    assert_eq!(json["checks"]["signature"], serde_json::json!(true));
    assert_eq!(json["checks"]["canonical_cid"], serde_json::json!(true));
    assert_eq!(
        json["checks"]["inclusion"],
        serde_json::json!("verified"),
        "the ledger claim was checked against the checkpoint the third file carries"
    );
    assert_eq!(json["anchor"], serde_json::json!("checkpoint-file"));
    assert_eq!(
        json["anchor_authenticated"],
        serde_json::json!(false),
        "no `--checkpoint-key` was offered, and the answer says so rather than staying quiet"
    );

    // 🔴 The half that makes this a hermeticity probe rather than a second AC-057: nothing was
    // created anywhere the process could write to by default.
    let after_cwd = entries(&h.cwd);
    let after_home = entries(&h.home);
    let after_inputs = entries(&h.inputs);
    println!("HERMETIC_CWD={after_cwd:?} HERMETIC_HOME={after_home:?} INPUTS={after_inputs:?}");
    assert!(
        after_cwd.is_empty(),
        "the working directory gained {after_cwd:?}; a verifier that writes is a verifier that \
         needs a workspace"
    );
    assert!(
        after_home.is_empty(),
        "HOME gained {after_home:?}; `~/.gx/keys/` is the owner's convenience path and `--key` is \
         what replaces it for a third party"
    );
    assert_eq!(
        after_inputs,
        vec![
            "checkpoint.json".to_string(),
            "key.pub.json".to_string(),
            "receipt.json".to_string()
        ],
        "the three files are the whole of what the auditor was given, and they are unchanged"
    );

    // The strengthening, with its denominator printed either way.
    let namespaced = network_namespace_available();
    println!("NETWORK_NAMESPACE={namespaced}");
    if namespaced {
        let out = run(std::process::Command::new("unshare")
            .arg("-rn")
            .arg(env!("CARGO_BIN_EXE_gx"))
            .arg("receipt")
            .arg("verify")
            .arg(&h.receipt)
            .arg("--offline")
            .arg("--checkpoint")
            .arg(&h.checkpoint)
            .arg("--key")
            .arg(&h.key)
            .current_dir(&h.cwd)
            .env("HOME", &h.home));
        println!(
            "HERMETIC_PASS_IN_NAMESPACE exit={} {}",
            out.code,
            out.json()
        );
        assert_eq!(
            out.code, 0,
            "the same three files inside a namespace with no interfaces: {}",
            out.stderr
        );
    }
}

/// 🔴 **DR-44-4, probe 2** — the inversion of `EctoLedger`'s `verify_cert.rs` fail-open.
///
/// Two shapes of the same removal, because "removed" has two spellings on the wire: the signature
/// list emptied (their case exactly — a document that carries no signature at all) and the signature
/// bytes blanked while the `keyid` stays (a document that says who signed and offers nothing). Both
/// are `7`, and the object on stdout says which check refused.
///
/// # Where the reason is written
///
/// On **stdout**, in the answer. 44 §1.3 makes stdout a single JSON object and stderr the operator's
/// channel, so `checks.signature: false` plus `refusal` is the machine-readable "why" and a probe
/// that demanded prose on stderr would be asserting against the output contract. The control run at
/// the end is what keeps the two refusals meaningful: a verifier that refused everything would pass
/// the first half of this probe.
#[test]
fn a_receipt_with_its_signature_removed_does_not_exit_zero() {
    let h = given("dr44_4_unsigned");

    let mut stripped = h.receipt_json();
    stripped["envelope"]["signatures"] = serde_json::json!([]);
    let stripped_path = write_json(&h.inputs.join("no-signature.json"), &stripped);
    let out = h.verify_anchored(&stripped_path);
    let json = out.json();
    println!(
        "SIGNATURE_REMOVED exit={} {json} stderr={:?}",
        out.code,
        out.stderr.trim()
    );
    assert_ne!(
        out.code, 0,
        "🔴 a receipt with no signature is not valid. `verify_cert.rs:80-81` answers this input \
         with a warning and exit 0; this binary does not"
    );
    assert_eq!(
        out.code, 7,
        "44 §1.4's `7=verification failed`, not `1`: the file parsed, the check refused"
    );
    assert_eq!(json["valid"], serde_json::json!(false));
    assert_eq!(
        json["checks"]["signature"],
        serde_json::json!(false),
        "and the answer names the half that refused rather than reporting a general failure"
    );
    assert!(
        json["refusal"].as_str().is_some_and(|r| !r.is_empty()),
        "44 §1.3 puts the reason in the object: {json}"
    );

    // The second spelling: the `keyid` is still offered, and the bytes under it are zeroed.
    let mut blanked = h.receipt_json();
    let sig = blanked["envelope"]["signatures"][0]["sig"]
        .as_str()
        .expect("42 §3.10's signature, base64 on the JSON face")
        .to_string();
    let width = gx_core::b64::decode(&sig)
        .expect("the signature is base64")
        .len();
    blanked["envelope"]["signatures"][0]["sig"] =
        serde_json::json!(gx_core::b64::encode(&vec![0u8; width]));
    let blanked_path = write_json(&h.inputs.join("blank-signature.json"), &blanked);
    let out = h.verify_anchored(&blanked_path);
    let json = out.json();
    println!("SIGNATURE_BLANKED exit={} {json} width={width}", out.code);
    assert_eq!(
        out.code, 7,
        "a signature of zeroes is a signature nobody made"
    );
    assert_eq!(json["checks"]["signature"], serde_json::json!(false));

    // The control, in the same environment: without it both refusals above are satisfied by a
    // verifier that refuses every input it is given.
    let control = h.verify_anchored(&h.receipt);
    println!("SIGNATURE_CONTROL exit={}", control.code);
    assert_eq!(
        control.code, 0,
        "the untouched receipt still verifies here: {}",
        control.stderr
    );
    assert!(entries(&h.home).is_empty(), "and HOME is still empty");
}

/// 🔴 **DR-44-4, probe 3** — H5-9 asserted as an exit status, in both of its shapes.
///
/// `crates/gx-cli/src/receipt.rs`'s `INCLUSION_JSON` writes the rule (`Unanchored` — a
/// `CommitReceipt` verified with no anchor — is **not** a pass) and
/// `gx_witness::receipt::Checks::verified` implements it, so the library-level claim is held by
/// `ac_057.rs`. What a third party runs is a **process**, and the two ways an anchor fails to be an
/// anchor are:
///
/// 1. **missing** — `--offline` with no `--checkpoint`, which 44 §1.2 permits. The signature still
///    verifies and the ledger claim is unchecked, and an implementation that summed those to "valid"
///    would be the fail-open `req/29` §4 forbids;
/// 2. **unauthenticated** — a checkpoint whose own signature does not hold, offered with
///    `--checkpoint-key`. `req/207` §1-2's reading of `EctoLedger` found the opposite posture there:
///    the trust root travels inside the document being checked. Here the anchor is a separate
///    argument and its authentication is a separate argument again, which is the property DR-44-4
///    declines to trade away for a shorter command line.
#[test]
fn an_unanchored_commit_receipt_does_not_exit_zero() {
    let h = given("dr44_4_unanchored");

    let out = h.verify_unanchored(&h.receipt);
    let json = out.json();
    println!(
        "ANCHOR_MISSING exit={} {json} stderr={:?}",
        out.code,
        out.stderr.trim()
    );
    assert_eq!(
        json["checks"]["signature"],
        serde_json::json!(true),
        "the signature half is fine, which is exactly why the other half has to be reported"
    );
    assert_eq!(
        json["checks"]["inclusion"],
        serde_json::json!("unanchored"),
        "one of the two words 44 §1.2 has no spelling for (M6H2-3)"
    );
    assert_eq!(json["anchor"], serde_json::json!("none"));
    assert_eq!(json["valid"], serde_json::json!(false), "H5-9");
    assert_eq!(
        out.code, 7,
        "and the exit status a script reads agrees with the object a human reads"
    );

    // A checkpoint with one flipped signature byte, offered together with the key it claims to have
    // been signed under: the anchor exists and does not authenticate.
    let mut head: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&h.checkpoint).expect("read the checkpoint"))
            .expect("it is a checkpoint");
    let sig = head["signature"]["sig"]
        .as_str()
        .expect("42 §3.11's signature")
        .to_string();
    let mut bytes = gx_core::b64::decode(&sig).expect("the signature is base64");
    bytes[0] ^= 0x01;
    head["signature"]["sig"] = serde_json::json!(gx_core::b64::encode(&bytes));
    let forged = write_json(&h.inputs.join("forged-checkpoint.json"), &head);

    let out = h.verify_with_authenticated_anchor(&h.receipt, &forged);
    let json = out.json();
    println!("ANCHOR_UNAUTHENTICATED exit={} {json}", out.code);
    assert_eq!(
        out.code, 7,
        "an anchor that does not authenticate is not an anchor: {json}"
    );
    assert_eq!(json["anchor_authenticated"], serde_json::json!(false));

    // The control: the genuine head, authenticated, in the same empty environment.
    let control = h.verify_with_authenticated_anchor(&h.receipt, &h.checkpoint);
    let json = control.json();
    println!("ANCHOR_CONTROL exit={} {json}", control.code);
    assert_eq!(control.code, 0, "the published head verifies: {json}");
    assert_eq!(json["anchor_authenticated"], serde_json::json!(true));
    assert!(entries(&h.cwd).is_empty(), "and nothing was written, again");
}
