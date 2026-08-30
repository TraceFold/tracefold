// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R-922-F2 phase 1** — the `.gx` object file, measured end to end.
//!
//! Subject: `gx_witness::gxfile` and the two verbs over it (`gx object export`, `gx object
//! verify`). Requirement: `req/922` §3 F5/F6 and its acceptance criteria — a round trip whose
//! identity is equal, a tampered file that is refused, an unrecognised kind that is refused **by
//! name**, and a measured serialize/deserialize line (`req/922` §0 principle ②).
//!
//! # The negative control is exhaustive, and it is not the obvious claim
//!
//! "any flipped byte is caught" is **false of this format and false on purpose**: E-M2-6 took
//! `issued_at` out of the signed core, so a receipt's timestamp is unsigned metadata, and a format
//! that refused a change to it would be claiming a guarantee the signature does not give. So the
//! claim asserted below is the true one, over **every** byte of a real exported file:
//!
//! > every single-bit flip is either refused, or leaves the signed envelope byte-identical.
//!
//! That is stronger than a one-byte spot check and it is honest about what is covered. A
//! sample-of-one negative control would also have hidden which layer caught what, and each flip
//! here records whether the wrapper, the decoder, the identity recomputation or the signature was
//! the one that refused.

mod support;

use std::path::{Path, PathBuf};

use gx_core::{Cid, TransformationId, VerdictKind};
use gx_witness::gxfile::{self, GxKind, Refusal, FORMAT_VERSION, HEADER_LEN};
use gx_witness::receipt::{Checks, Receipt};
use gx_witness::KeyPair;

use support::{gx, issue, keypair, project, run, verdict_payload, write_public_key, Run};

/// The seed every fixture in this file uses, so one project holds one document.
const SEED: u64 = 922;

/// A project holding one signed `VerdictReceipt`, that receipt, and the public key it names.
///
/// A **verdict** receipt rather than a commit one: a `CommitReceipt` verified against no anchor
/// answers `inclusion: unanchored`, which `Checks::verified` refuses to call a pass (H5-9), and a
/// positive control has to be one that can actually pass. What this phase's `verify` does with an
/// anchored document is the anchoring flags' question and they are the next phase's.
struct Exported {
    dir: PathBuf,
    key: KeyPair,
    key_file: PathBuf,
    id: TransformationId,
    receipt: Receipt,
    file: PathBuf,
    bytes: Vec<u8>,
}

fn export_one(name: &str) -> Exported {
    let (dir, layout) = project(name);
    let key = keypair(9);
    let payload = verdict_payload(VerdictKind::Admit, &key, SEED);
    let id = payload.transformation;
    let receipt = issue(&payload, &key);

    let store = gx_cli::receipt::ReceiptStore::in_layout(&layout);
    store
        .put(&id, gx_cli::receipt::StoredKind::Verdict, &receipt)
        .expect("file the receipt");

    let key_file = write_public_key(&dir, &key);
    let file = dir.join("exported.gx");
    let out = run(gx()
        .arg("--project")
        .arg(&dir)
        .args(["object", "export", &id.0.to_text(), "--out"])
        .arg(&file));
    assert_eq!(out.code, 0, "gx object export: {}", why(&out));

    let bytes = std::fs::read(&file).expect("the export wrote a file");
    Exported {
        dir,
        key,
        key_file,
        id,
        receipt,
        file,
        bytes,
    }
}

fn verify(file: &Path, key: &Path) -> Run {
    run(gx()
        .args(["object", "verify"])
        .arg(file)
        .arg("--key")
        .arg(key))
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text)
        .unwrap_or_else(|e| panic!("the answer is one JSON object: {e}\n{text}"))
}

/// What one run said, for an assertion message. `support::Run` carries no `Debug` and this file
/// does not add one to a fixture module three other suites share.
fn why(out: &Run) -> String {
    format!(
        "exit {} stdout={:?} stderr={:?}",
        out.code, out.stdout, out.stderr
    )
}

/// 🔴 **AC (a)** — export, read back, and the identity is the same number.
///
/// Three equalities and not one: the recomputed identity equals the body's own `Cid`, the decoded
/// document equals the one that was filed, and the signed bytes are carried through **unchanged**
/// (the round trip is an identity, not a re-derivation — `req/38` §324 ruling 3).
#[test]
fn export_then_read_back_gives_the_same_identity_and_the_same_document() {
    let e = export_one("r922_round_trip");

    let expected = gxfile::body_cid(&e.receipt.envelope.payload).expect("the fixture is canonical");
    let read = gxfile::read(&e.bytes).expect("what this build wrote, this build reads");

    assert_eq!(
        read.cid, expected,
        "the identity moved across the round trip"
    );
    assert_eq!(read.format_version, FORMAT_VERSION);
    assert_eq!(read.kind, GxKind::Receipt);
    // 🔴 `read.receipt` became `read.receipt()` when a second kind shipped (R-930-B1, `req/939`
    // §2-E): the body is a kind-tagged enum now, which is the upgrade `gxfile.rs` predicted. What
    // is asserted is unchanged.
    let body = read.receipt().expect("a receipt file holds a receipt");
    assert_eq!(body, &e.receipt, "the document moved across the round trip");
    assert_eq!(
        body.envelope.payload, e.receipt.envelope.payload,
        "the signed bytes were re-encoded rather than carried"
    );

    // The header is where the wrapper's whole claim lives, so it is read as bytes here rather
    // than through the parser that wrote it.
    assert_eq!(&e.bytes[..2], b"gx");
    assert_eq!(&e.bytes[2..4], &FORMAT_VERSION.to_be_bytes());
    assert_eq!(&e.bytes[4..6], &GxKind::Receipt.code().to_be_bytes());
    assert_eq!(
        &e.bytes[6..HEADER_LEN],
        &expected.0,
        "the stored claim is the body's identity"
    );

    // And the same equality through the surface an operator uses.
    let answer = json(
        &run(gx()
            .arg("--project")
            .arg(&e.dir)
            .args(["object", "export", &e.id.0.to_text(), "--out"])
            .arg(e.dir.join("again.gx")))
        .stdout,
    );
    assert_eq!(answer["cid"], serde_json::json!(expected.to_text()));
    assert_eq!(answer["kind"], serde_json::json!("Receipt"));
    assert_eq!(answer["stored_kind"], serde_json::json!("verdict"));
    assert_eq!(answer["format_version"], serde_json::json!(1));
}

/// 🔴 **AC (a), the positive control on the surface** — an untouched file verifies, and the
/// answer says which checks were made.
#[test]
fn an_untouched_file_verifies_and_names_its_checks() {
    let e = export_one("r922_positive");
    let out = verify(&e.file, &e.key_file);
    assert_eq!(
        out.code,
        0,
        "an untouched export must verify: {}",
        why(&out)
    );

    let answer = json(&out.stdout);
    assert_eq!(answer["valid"], serde_json::json!(true));
    assert_eq!(answer["checks"]["identity"], serde_json::json!(true));
    assert_eq!(answer["checks"]["signature"], serde_json::json!(true));
    assert_eq!(answer["kind"], serde_json::json!("Receipt"));
    assert_eq!(
        answer["cid"],
        serde_json::json!(gxfile::body_cid(&e.receipt.envelope.payload)
            .expect("canonical")
            .to_text())
    );
    // What was *not* checked is on the wire too: no ledger was consulted and no anchor was given.
    assert_eq!(answer["anchor"], serde_json::json!("none"));
    assert_eq!(answer["anchor_authenticated"], serde_json::json!(false));
}

/// 🔴 The property that decides whether this format can rot: **the wrapper does not decode the
/// body**, so a document issued by an older generation survives being wrapped and read back.
///
/// The subject is R38's frozen specimen (`gx-witness/tests/fixtures/frozen_receipts/
/// issued_2026_08_18/`), which was issued by a build whose `ReceiptPayload` had eleven members
/// where today's has more. That file is the one thing in this tree the current binary did not
/// mint, which is why it is the right subject: `req/38` §324 sent three lanes back over code that
/// asked "what would *this build's* schema have written" about a document that arrived, and a
/// wrapper that re-encoded its body would be the same mistake one layer out.
///
/// Whether this build can decode that payload into today's type is **not** asserted here — that is
/// `frozen_receipt_corpus.rs`'s declared limit and its to move. It is printed, so this run says
/// which world it ran in.
#[test]
fn a_receipt_from_an_earlier_generation_survives_the_wrapper_unread() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/gx-cli sits under crates/")
        .join("gx-witness/tests/fixtures/frozen_receipts/issued_2026_08_18/receipt.json");
    assert!(
        path.exists(),
        "the frozen specimen moved: {} — this probe is about a document this binary did not \
         mint, so a missing fixture is a red and not a skip",
        path.display()
    );
    let text = std::fs::read_to_string(&path).expect("the frozen receipt is readable");
    let old: Receipt = serde_json::from_str(&text).expect("the frozen receipt is a Receipt");
    println!(
        "R922_FROZEN_SPECIMEN payload_bytes={} decodes_into_todays_payload={}",
        old.envelope.payload.len(),
        old.payload().is_ok()
    );

    let wrapped = gxfile::write_receipt(&old).expect("the wrapper does not need to understand it");
    let back = gxfile::read(&wrapped).expect("what the wrapper wrote, the wrapper reads");
    let carried = back.receipt().expect("a receipt file holds a receipt");
    assert_eq!(
        carried.envelope.payload, old.envelope.payload,
        "the body of a document from another generation was re-encoded"
    );
    assert_eq!(carried, &old, "the document did not survive the wrapper");
    assert_eq!(
        back.cid,
        gxfile::body_cid(&old.envelope.payload).expect("the frozen body is canonical"),
        "the identity of a document this build cannot read is still recomputable from its bytes"
    );
}

/// 🔴 **AC (b)** — every single-bit flip, over the whole file.
///
/// Refused, or the signed envelope is untouched. The census is printed so that a run which
/// somehow examined nothing cannot pass quietly.
#[test]
fn every_single_bit_flip_is_refused_or_changes_nothing_that_is_signed() {
    let e = export_one("r922_flips");
    let verifying = e.key.public();

    let mut refused_by_wrapper = 0usize;
    let mut refused_by_decoder = 0usize;
    let mut refused_by_identity = 0usize;
    let mut refused_by_signature = 0usize;
    let mut unsigned_metadata_only = 0usize;

    for offset in 0..e.bytes.len() {
        let mut tampered = e.bytes.clone();
        tampered[offset] ^= 0x01;

        match gxfile::read(&tampered) {
            Err(Refusal::NotGxObjectFile { .. })
            | Err(Refusal::FormatVersion { .. })
            | Err(Refusal::UnknownKind { .. })
            | Err(Refusal::KindNotShipped { .. })
            | Err(Refusal::PayloadType { .. }) => refused_by_wrapper += 1,
            Err(Refusal::Body { .. }) => refused_by_decoder += 1,
            Err(Refusal::BodyNotCanonical { .. }) | Err(Refusal::IdentityMismatch { .. }) => {
                refused_by_identity += 1;
            }
            // `Refusal` is `#[non_exhaustive]`, so the language requires this arm. A variant added
            // later lands here and is loud rather than counted into somebody else's column.
            Err(other) => {
                panic!("byte {offset}: an unclassified refusal reached the census: {other}")
            }
            Ok(file) => {
                // 🔴 `.receipt` became `.receipt()` with the kind-tagged body (R-930-B1). A flip
                // that produced any other kind cannot reach here: this fixture's kind code is 1,
                // and no single bit takes 1 to 15.
                let carried = file
                    .receipt()
                    .expect("a bit flip admitted a file that is not a receipt");
                let checks = gx_witness::verify_offline(carried, &verifying.verifying(), None);
                match checks {
                    Err(_) => refused_by_signature += 1,
                    Ok(checks) if !Checks::verified(&checks) => refused_by_signature += 1,
                    Ok(_) => {
                        assert_eq!(
                            carried.envelope, e.receipt.envelope,
                            "byte {offset} was accepted and it moved something the signature \
                             covers"
                        );
                        unsigned_metadata_only += 1;
                    }
                }
            }
        }
    }

    println!(
        "R922_FLIP_CENSUS bytes={} wrapper={refused_by_wrapper} decoder={refused_by_decoder} \
         identity={refused_by_identity} signature={refused_by_signature} \
         unsigned_metadata_only={unsigned_metadata_only}",
        e.bytes.len()
    );
    assert_eq!(
        refused_by_wrapper
            + refused_by_decoder
            + refused_by_identity
            + refused_by_signature
            + unsigned_metadata_only,
        e.bytes.len(),
        "every byte is accounted for"
    );
    // Non-vacuity: each of the three layers this format adds has to have caught something, or the
    // census above is measuring one road and reporting three.
    assert!(refused_by_wrapper > 0, "the wrapper caught nothing");
    assert!(
        refused_by_identity > 0,
        "the identity recomputation caught nothing"
    );
    assert!(refused_by_signature > 0, "the signature caught nothing");
}

/// 🔴 **AC (b), on the surface** — a flipped identity claim is exit 7 with the reason, and a
/// flipped magic is exit 1: "this document failed its check" and "this is not one of our files"
/// are two answers.
#[test]
fn the_surface_separates_a_failed_check_from_a_file_that_is_not_ours() {
    let e = export_one("r922_surface_negatives");

    let mut claim = e.bytes.clone();
    claim[HEADER_LEN - 1] ^= 0x01;
    let path = e.dir.join("claim.gx");
    std::fs::write(&path, &claim).expect("write");
    let out = verify(&path, &e.key_file);
    assert_eq!(
        out.code,
        7,
        "a false identity claim is a failed check: {}",
        why(&out)
    );
    let answer = json(&out.stdout);
    assert_eq!(answer["valid"], serde_json::json!(false));
    assert_eq!(answer["checks"]["identity"], serde_json::json!(false));
    assert_eq!(
        answer["checks"]["signature"],
        serde_json::Value::Null,
        "nothing downstream ran, and `false` would claim it did"
    );
    assert!(
        answer["refusal"]
            .as_str()
            .unwrap_or_default()
            .contains("claims identity"),
        "the answer names the reason: {answer}"
    );

    let mut magic = e.bytes.clone();
    magic[0] ^= 0x01;
    let path = e.dir.join("magic.gx");
    std::fs::write(&path, &magic).expect("write");
    let out = verify(&path, &e.key_file);
    assert_eq!(
        out.code,
        1,
        "bytes that are not a gx object file are input: {}",
        why(&out)
    );
    assert!(
        out.stderr.contains("gx object file"),
        "the refusal says what it was not: {}",
        why(&out)
    );
}

/// 🔴 **AC (c)** — an unrecognised kind is refused **by name**, and a registered kind this build
/// does not ship is refused by a **different** name.
///
/// The two are kept apart deliberately: "wait for a later build" and "this is not a gx object"
/// send an operator to two different places, and folding them would be the shape this project's
/// own canon keeps closing.
#[test]
fn an_unknown_kind_and_an_unshipped_kind_are_refused_differently() {
    let e = export_one("r922_kinds");

    let mut unknown = e.bytes.clone();
    unknown[4..6].copy_from_slice(&(GxKind::REGISTRY.len() as u16 + 1).to_be_bytes());
    assert!(matches!(
        gxfile::read(&unknown),
        Err(Refusal::UnknownKind { .. })
    ));
    let path = e.dir.join("unknown.gx");
    std::fs::write(&path, &unknown).expect("write");
    let out = verify(&path, &e.key_file);
    assert_eq!(out.code, 1, "an unknown kind is refused: {}", why(&out));
    assert!(
        out.stderr.contains("in no entry"),
        "the refusal names the registry: {}",
        why(&out)
    );

    let mut unshipped = e.bytes.clone();
    unshipped[4..6].copy_from_slice(&GxKind::Checkpoint.code().to_be_bytes());
    assert!(matches!(
        gxfile::read(&unshipped),
        Err(Refusal::KindNotShipped {
            kind: GxKind::Checkpoint
        })
    ));
    let path = e.dir.join("unshipped.gx");
    std::fs::write(&path, &unshipped).expect("write");
    let out = verify(&path, &e.key_file);
    assert_eq!(
        out.code,
        1,
        "a kind with no codec is refused: {}",
        why(&out)
    );
    assert!(
        out.stderr.contains("Checkpoint") && out.stderr.contains("no codec"),
        "the refusal names the kind and what is missing: {}",
        why(&out)
    );

    // A wrapper version this build does not read is the third refusal of the same family.
    let mut newer = e.bytes.clone();
    newer[2..4].copy_from_slice(&(FORMAT_VERSION + 1).to_be_bytes());
    assert!(matches!(
        gxfile::read(&newer),
        Err(Refusal::FormatVersion { found }) if found == FORMAT_VERSION + 1
    ));
}

/// 🔴 **AC (e)** — the measured line `req/922` §0 principle ② requires.
///
/// A measurement and **not** a gate: a threshold here would be a bench in a deterministic suite,
/// which `probes/doubt/tests/bench_gate_doubt.rs` records as the wrong shape (benches are noisy
/// and live in stage 10). What is asserted is only that the work happened — the numbers are
/// printed for a reader, and a run on a different machine says so by being a different number.
#[test]
fn serialize_and_deserialize_are_measured() {
    let e = export_one("r922_bench");
    const ROUNDS: u32 = 1_000;

    let start = std::time::Instant::now();
    let mut written = 0usize;
    for _ in 0..ROUNDS {
        written += gxfile::write_receipt(&e.receipt).expect("canonical").len();
    }
    let encode = start.elapsed();

    let start = std::time::Instant::now();
    let mut identities = 0usize;
    for _ in 0..ROUNDS {
        let file = gxfile::read(&e.bytes).expect("round trip");
        identities += usize::from(file.cid != Cid([0u8; 32]));
    }
    let decode = start.elapsed();

    println!(
        "R922_BENCH rounds={ROUNDS} file_bytes={} encode_total_us={} encode_per_op_us={:.2} \
         decode_total_us={} decode_per_op_us={:.2}",
        e.bytes.len(),
        encode.as_micros(),
        encode.as_secs_f64() * 1e6 / f64::from(ROUNDS),
        decode.as_micros(),
        decode.as_secs_f64() * 1e6 / f64::from(ROUNDS),
    );
    assert_eq!(
        written,
        e.bytes.len() * ROUNDS as usize,
        "every round wrote"
    );
    assert_eq!(identities, ROUNDS as usize, "every round read");
}
