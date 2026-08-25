// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P1** (`req/506` §1 P1, §2 P1 AC) — receipt-format conformance, expressed as the machine
//! check `docs/LIMITS.md` #8's primary claim asks a buyer to run: *a third party verifies a receipt
//! with three files and one binary, and the answer separates "I do not trust this issuer" from
//! "this was tampered with".*
//!
//! # What this suite adds over `receipt_verify_hermetic.rs`
//!
//! `receipt_verify_hermetic.rs` (DR-44-4) established the **environment** as a subject — the verifier
//! is given nothing but `HOME` and two empty directories — and inverted a competitor's three fail-open
//! shapes (absent signature, non-fatal anchor). This suite establishes the **verdict vocabulary** as a
//! subject: `req/369` §5's A-90 splits a passing answer into two tiers, and a buyer needs the split to
//! be real rather than a word.
//!
//! * **compatible** — the document's wire form matches the declared receipt shape (42 §3.10:
//!   `payload_type`, a base64 `payload`, an ed25519 `signatures` list). Checkable by anyone, with no
//!   trust key: it says "this is a receipt", not "this receipt is good".
//! * **verified** — compatible, **and** the semantics (signature, canonical CID, inclusion, anchor
//!   authentication) hold under the third party's trust set. This is `44` §1.2's `valid: true`.
//! * **refuted** — compatible in form, but a declared check fails **even under the issuer's own key**.
//!   That is the honest name for tamper, and it is the one an operator reads as an accusation, so the
//!   suite is careful never to hand it to a receipt whose only problem is an untrusted issuer.
//!
//! The discriminator between `compatible` and `refuted` is **self-consistency**: a genuine receipt
//! signed by a key the verifier does not hold still verifies against *its own* issuer key
//! (`compatible`, recoverable — obtain the key); a tampered one does not verify against any key
//! (`refuted`, unrecoverable). A verifier that collapsed these two — answering `valid: false` for
//! both and stopping — would leak a buyer's genuine receipt into the same bucket as a forgery. The
//! catalogue below is the standing proof that this build does not.
//!
//! # 🔴 The catalogue, and why each entry is a negative control
//!
//! A conformance suite that only ever runs the passing case measures a verifier that answers
//! `valid: true` unconditionally exactly as well as a correct one. Every check here is paired with a
//! tamper it must refuse — signature, anchor `tree_size`, anchor origin — and one non-tamper it must
//! **not** refuse (the untrusted issuer). [`the_three_verdicts_are_distinct`] is the anti-false-
//! positive gate: it fails unless all three of `verified`, `compatible` and `refuted` are actually
//! produced by the one binary under test, so a build that lost the split fails this file rather than
//! passing it quietly. The tamper shapes are the standard transparency-log ones (RFC 6962 / 9162
//! inclusion against a signed checkpoint over `{origin, tree_size, root_hash}`); no external code is
//! read or copied — the checks are derived from this repository's own receipt and checkpoint
//! mechanism and the RFC that mechanism already implements.

mod support;

use std::path::{Path, PathBuf};

use support::{keypair, project, run, scratch, secure_scratch, write_json, write_public_key, Run};

/// A-90's two passing tiers, plus the refutation the catalogue needs — the vocabulary this suite
/// adopts as its **output** (`req/506` §2), derived from `44` §1.2's `{ valid, checks, anchor }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Conformance {
    /// Wire form does not match the declared receipt shape: not even a receipt.
    Malformed,
    /// Wire form matches, and the document is self-consistent, but its issuer's key is not in the
    /// third party's trust set. Structurally usable, not verified — and **recoverable**.
    Compatible,
    /// Compatible, and the semantics hold under the third party's trust set. `44` §1.2's `true`.
    Verified,
    /// A declared check fails even under the issuer's own key: tamper. **Unrecoverable.**
    Refuted,
}

/// Everything a third party holds for one receipt, and the two empty directories the verify runs in.
struct Fixture {
    /// An empty working directory with no `.gx/`: the auditor is not in a project.
    cwd: PathBuf,
    /// An empty `HOME`: no `~/.gx/keys/`, which `--key` stands in for.
    home: PathBuf,
    /// The directory holding the receipt, the checkpoint and the public key.
    inputs: PathBuf,
    /// The exported `CommitReceipt`, on disk.
    receipt: PathBuf,
    /// That receipt, parsed, for probes that mutate it.
    receipt_value: serde_json::Value,
    /// The signed checkpoint the ledger owner published.
    checkpoint: PathBuf,
    /// The **issuer's** own public key, in `gx key gen`'s shape (the self-consistency anchor).
    signer_key: PathBuf,
}

/// Build the Given: a real ledger, a real inclusion proof, a signed checkpoint, the issuer's public
/// key — and three empty directories, from which the verifying half is hermetic. The signing key is
/// deleted before any verify runs (a fixture that left it lying beside the public one would be
/// measuring the owner's environment under another name).
fn mint(name: &str, seed: u8) -> Fixture {
    mint_sized(name, seed, 8)
}

/// A `mint` variant taking an explicit number of prior leaves, so a second ledger can be a different
/// tree under the same issuer key -- the Given the origin-swap probe needs.
fn mint_sized(name: &str, seed: u8, others: u64) -> Fixture {
    let (project_dir, layout) = project(&format!("{name}_project"));
    let inputs = scratch(&format!("{name}_inputs"));
    let cwd = scratch(&format!("{name}_cwd"));
    let home = scratch(&format!("{name}_home"));
    let key = keypair(seed);

    let (receipt, _index) = support::seed_ledger(&layout, &key, 444, others);
    let receipt_value = serde_json::to_value(&receipt).expect("the receipt serialises");
    let receipt_path = write_json(&inputs.join("receipt.json"), &receipt_value);

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

    let signer_key = write_public_key(&inputs, &key);
    std::fs::remove_file(&secret).expect("the verifier does not hold the signing key");

    Fixture {
        cwd,
        home,
        inputs,
        receipt: receipt_path,
        receipt_value,
        checkpoint: checkpoint_path,
        signer_key,
    }
}

impl Fixture {
    /// `gx receipt verify <receipt> --offline --checkpoint <cp> --key <pub> --checkpoint-key <pub>`,
    /// hermetic: `env_clear` leaves one variable, `HOME`, pointing at an empty directory, and the
    /// working directory is empty and holds no `.gx/`. An implementation that reached for a key store,
    /// a cache or a server would fail (nothing is there) or leave a trace (it created one).
    fn verify(&self, receipt: &Path, checkpoint: &Path, key: &Path) -> Run {
        let mut cmd = support::gx();
        cmd.env_clear();
        cmd.env("HOME", &self.home);
        cmd.current_dir(&self.cwd);
        cmd.arg("receipt")
            .arg("verify")
            .arg(receipt)
            .arg("--offline")
            .arg("--checkpoint")
            .arg(checkpoint)
            .arg("--key")
            .arg(key)
            .arg("--checkpoint-key")
            .arg(key);
        run(&mut cmd)
    }

    /// Write a public key file for an arbitrary key, in its own directory so two can coexist.
    fn foreign_key(&self, name: &str, seed: u8) -> PathBuf {
        let dir = scratch(name);
        write_public_key(&dir, &keypair(seed))
    }
}

/// The wire-form check, made **without any trust key**: does this document conform to 42 §3.10's
/// declared receipt shape? This is the whole of `compatible` — it says "this is a receipt", and a
/// buyer can run it before deciding whether they even hold a key to check it against.
fn is_wire_compatible(receipt: &serde_json::Value) -> bool {
    let envelope = &receipt["envelope"];
    let type_ok =
        envelope["payload_type"].as_str() == Some("application/vnd.glovrex.receipt+dagcbor");
    let payload_ok = envelope["payload"]
        .as_str()
        .and_then(|p| gx_core::b64::decode(p).ok())
        .is_some_and(|bytes| !bytes.is_empty());
    let signatures_ok = envelope["signatures"].as_array().is_some_and(|sigs| {
        !sigs.is_empty()
            && sigs.iter().all(|s| {
                let keyid_ok = s["keyid"].as_str().is_some_and(|k| !k.is_empty());
                let sig_ok = s["sig"]
                    .as_str()
                    .and_then(|x| gx_core::b64::decode(x).ok())
                    .is_some_and(|bytes| bytes.len() == 64);
                keyid_ok && sig_ok
            })
    });
    type_ok && payload_ok && signatures_ok
}

/// Whether one verify run said `valid: true`, taking the exit status and the object to agree (44
/// §1.3 makes them one answer, and a probe that read only one of them would miss a build that let
/// them drift).
fn is_valid(out: &Run) -> bool {
    out.code == 0 && out.json()["valid"] == serde_json::json!(true)
}

/// The classifier the whole suite turns on. It holds every key (it is the author of the fixture), so
/// it can tell the two `valid: false` shapes apart the way a third party with only one key cannot:
///
/// * `self_valid` — does the document verify under the **issuer's own** key? This is self-consistency,
///   and it is what separates an untrusted-but-genuine receipt (`true`) from a tampered one (`false`).
/// * `trust_valid` — does it verify under the **third party's** trust key? This is `44`'s `valid`.
fn classify(
    fx: &Fixture,
    receipt: &Path,
    receipt_value: &serde_json::Value,
    checkpoint: &Path,
    trust_key: &Path,
) -> Conformance {
    if !is_wire_compatible(receipt_value) {
        return Conformance::Malformed;
    }
    let self_valid = is_valid(&fx.verify(receipt, checkpoint, &fx.signer_key));
    if !self_valid {
        return Conformance::Refuted;
    }
    let trust_valid = is_valid(&fx.verify(receipt, checkpoint, trust_key));
    if trust_valid {
        Conformance::Verified
    } else {
        Conformance::Compatible
    }
}

/// Entries of a directory, sorted — the shape the "nothing was created" assertions compare.
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("the directory exists")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// 🔴 **P1 AC(1) + AC(3)** — the passing case is `verified`, hermetically.
///
/// A genuine receipt, its genuine anchor, the issuer's key in the trust set: `44` §1.2's `valid: true`
/// with `signature`, `canonical_cid` and `inclusion: "verified"` all holding and the anchor
/// authenticated. And the environment is still what it was: no `.gx/`, no key store, no cache, no
/// socket was created, so the "three files and one binary" claim is asserted as a state of the world,
/// not just an exit code.
#[test]
fn a_genuine_receipt_is_verified_hermetically() {
    let fx = mint("p1_verified", 44);
    assert!(
        !fx.cwd.join(".gx").exists(),
        "the auditor is not in a gx project"
    );
    assert!(entries(&fx.home).is_empty(), "and HOME starts empty");

    let out = fx.verify(&fx.receipt, &fx.checkpoint, &fx.signer_key);
    let json = out.json();
    println!("P1_VERIFIED exit={} {json}", out.code);

    assert_eq!(out.code, 0, "44 §1.4's `0`: {}", out.stderr);
    assert_eq!(json["valid"], serde_json::json!(true));
    assert_eq!(json["checks"]["signature"], serde_json::json!(true));
    assert_eq!(json["checks"]["canonical_cid"], serde_json::json!(true));
    assert_eq!(json["checks"]["inclusion"], serde_json::json!("verified"));
    assert_eq!(json["anchor_authenticated"], serde_json::json!(true));

    assert_eq!(
        classify(
            &fx,
            &fx.receipt,
            &fx.receipt_value,
            &fx.checkpoint,
            &fx.signer_key
        ),
        Conformance::Verified,
    );

    // The hermetic half: nothing was created anywhere the process could write to by default.
    assert!(
        entries(&fx.cwd).is_empty(),
        "the working directory gained files: {:?}",
        entries(&fx.cwd)
    );
    assert!(
        entries(&fx.home).is_empty(),
        "HOME gained files: {:?}",
        entries(&fx.home)
    );
    assert_eq!(
        entries(&fx.inputs),
        vec![
            "checkpoint.json".to_string(),
            "key.pub.json".to_string(),
            "receipt.json".to_string()
        ],
        "the three files are unchanged",
    );
}

/// 🔴 **P1 AC(2), tamper 1 of 3 — the signature.** A single flipped bit in the receipt's signature is
/// `refuted`, not `verified` and not `compatible`: the document does not verify under *any* key,
/// including its own issuer's, so the honest word is tamper.
#[test]
fn a_flipped_signature_bit_is_refuted() {
    let fx = mint("p1_sigflip", 44);

    let mut tampered = fx.receipt_value.clone();
    let sig = tampered["envelope"]["signatures"][0]["sig"]
        .as_str()
        .expect("42 §3.10 signature")
        .to_string();
    let mut bytes = gx_core::b64::decode(&sig).expect("base64");
    bytes[0] ^= 0x01;
    tampered["envelope"]["signatures"][0]["sig"] = serde_json::json!(gx_core::b64::encode(&bytes));
    let path = write_json(&fx.inputs.join("sigflip.json"), &tampered);

    let out = fx.verify(&path, &fx.checkpoint, &fx.signer_key);
    let json = out.json();
    println!("P1_SIGFLIP exit={} {json}", out.code);
    assert_eq!(
        out.code, 7,
        "44 §1.4's `7`: the file parsed, the check refused"
    );
    assert_eq!(json["valid"], serde_json::json!(false));
    assert_eq!(json["checks"]["signature"], serde_json::json!(false));

    assert!(
        is_wire_compatible(&tampered),
        "a flipped bit keeps the wire form well-formed"
    );
    assert_eq!(
        classify(&fx, &path, &tampered, &fx.checkpoint, &fx.signer_key),
        Conformance::Refuted,
        "it does not verify under the issuer's own key either: tamper, not distrust",
    );
}

/// 🔴 **P1 AC(2), tamper 2 of 3 — the anchor's `tree_size`.** The checkpoint signature covers
/// `{origin, tree_size, root_hash}` (E-M2-19), so editing `tree_size` breaks the anchor's own
/// signature; offered with `--checkpoint-key`, it does not authenticate and the run is `refuted`
/// rather than degrading to a pass on the strength of an unchecked number (the exact hole `req/232`
/// H-01 measured).
#[test]
fn a_tampered_anchor_tree_size_is_refuted() {
    let fx = mint("p1_treesize", 44);

    let mut head: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&fx.checkpoint).expect("read the checkpoint"))
            .expect("a checkpoint");
    let original = head["tree_size"].as_u64().expect("42 §3.11 tree_size");
    head["tree_size"] = serde_json::json!(original + 1);
    let forged = write_json(&fx.inputs.join("treesize-checkpoint.json"), &head);

    let out = fx.verify(&fx.receipt, &forged, &fx.signer_key);
    let json = out.json();
    println!("P1_TREESIZE exit={} {json}", out.code);
    assert_eq!(
        out.code, 7,
        "an anchor whose own signature does not hold is not an anchor"
    );
    assert_eq!(json["valid"], serde_json::json!(false));
    assert_eq!(json["anchor_authenticated"], serde_json::json!(false));

    assert_eq!(
        classify(&fx, &fx.receipt, &fx.receipt_value, &forged, &fx.signer_key),
        Conformance::Refuted,
    );
}

/// 🔴 **P1 AC(2), tamper 3 of 3 — the anchor's origin.** A receipt checked against a **different
/// ledger's** genuine, correctly-signed checkpoint: the anchor authenticates (it is a real head) but
/// the inclusion proof does not reach that head's root, so the answer is one of `refuted` / `unbridged`
/// — the two words 44 §1.2 has no spelling for — and never `verified`. This is the origin swap 42
/// §3.11 makes `origin` exist to stop.
#[test]
fn a_foreign_ledgers_checkpoint_is_not_a_pass() {
    let fx = mint("p1_origin_a", 44);
    let other = mint_sized("p1_origin_b", 44, 12);

    let out = fx.verify(&fx.receipt, &other.checkpoint, &fx.signer_key);
    let json = out.json();
    println!("P1_FOREIGN_ORIGIN exit={} {json}", out.code);
    assert_ne!(
        out.code, 0,
        "a foreign head does not turn one log's receipt into another's"
    );
    assert_eq!(json["valid"], serde_json::json!(false));
    let inclusion = json["checks"]["inclusion"].as_str().unwrap_or("");
    assert!(
        inclusion == "refuted" || inclusion == "unbridged",
        "the proof did not reach a foreign root, and the word says so honestly: {json}",
    );

    // Classified against the foreign trust key (the only key a holder of `other`'s head would have):
    // the receipt does not verify under that anchor, so it is not a pass.
    assert_ne!(
        classify(
            &fx,
            &fx.receipt,
            &fx.receipt_value,
            &other.checkpoint,
            &fx.signer_key
        ),
        Conformance::Verified,
    );
}

/// 🔴 **P1 AC(4) — the non-tamper the suite must NOT refute.** A genuine receipt, checked by a third
/// party whose trust set does **not** contain its issuer: `compatible` (the wire form is right and the
/// document is self-consistent under its own key) but not `verified` (this verifier cannot confirm the
/// issuer). This is the middle state the split exists for, and the proof that `compatible` and
/// `verified` actually branch: the **same** receipt is `verified` under the issuer's key and merely
/// `compatible` under a stranger's.
#[test]
fn an_untrusted_issuer_is_compatible_not_verified() {
    let fx = mint("p1_untrusted", 44);
    let foreign = fx.foreign_key("p1_untrusted_foreign", 99);

    // Under the issuer's own key: verified.
    assert_eq!(
        classify(
            &fx,
            &fx.receipt,
            &fx.receipt_value,
            &fx.checkpoint,
            &fx.signer_key
        ),
        Conformance::Verified,
        "the issuer's own key confirms the receipt",
    );
    // Under a stranger's key: compatible, not verified — and not refuted (nothing was tampered).
    let verdict = classify(
        &fx,
        &fx.receipt,
        &fx.receipt_value,
        &fx.checkpoint,
        &foreign,
    );
    println!("P1_UNTRUSTED verdict={verdict:?}");
    assert_eq!(
        verdict,
        Conformance::Compatible,
        "an untrusted issuer is a key the verifier lacks, not a forgery",
    );

    // And what the third party's own run reports: valid:false, but the form was fine all along.
    let out = fx.verify(&fx.receipt, &fx.checkpoint, &foreign);
    assert_eq!(out.json()["valid"], serde_json::json!(false));
    assert!(
        is_wire_compatible(&fx.receipt_value),
        "the receipt was well-formed the whole time"
    );
}

/// 🔴 **The anti-false-positive gate.** A verifier that answered one verdict for everything would
/// satisfy each probe above that expects that verdict; this test fails unless the one binary under
/// test actually produces all three of `verified`, `compatible` and `refuted`, so the split is a
/// property of the build and not of the test's wording.
#[test]
fn the_three_verdicts_are_distinct() {
    let fx = mint("p1_distinct", 44);
    let foreign = fx.foreign_key("p1_distinct_foreign", 99);

    // verified
    let verified = classify(
        &fx,
        &fx.receipt,
        &fx.receipt_value,
        &fx.checkpoint,
        &fx.signer_key,
    );
    // compatible (untrusted issuer, nothing tampered)
    let compatible = classify(
        &fx,
        &fx.receipt,
        &fx.receipt_value,
        &fx.checkpoint,
        &foreign,
    );
    // refuted (a flipped signature bit)
    let mut tampered = fx.receipt_value.clone();
    let sig = tampered["envelope"]["signatures"][0]["sig"]
        .as_str()
        .unwrap()
        .to_string();
    let mut bytes = gx_core::b64::decode(&sig).unwrap();
    bytes[0] ^= 0x01;
    tampered["envelope"]["signatures"][0]["sig"] = serde_json::json!(gx_core::b64::encode(&bytes));
    let path = write_json(&fx.inputs.join("distinct-tampered.json"), &tampered);
    let refuted = classify(&fx, &path, &tampered, &fx.checkpoint, &fx.signer_key);

    println!("P1_DISTINCT verified={verified:?} compatible={compatible:?} refuted={refuted:?}");
    assert_eq!(verified, Conformance::Verified);
    assert_eq!(compatible, Conformance::Compatible);
    assert_eq!(refuted, Conformance::Refuted);
    assert_ne!(verified, compatible);
    assert_ne!(compatible, refuted);
    assert_ne!(verified, refuted);
}
