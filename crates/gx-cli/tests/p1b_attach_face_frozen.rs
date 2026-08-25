// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P-1b / AC-13** (`req/544` §5, `req/38` §305-2 (d)) — the receipts an **attached** project
//! issued on 2026-08-22, frozen before this lane changed anything, and read on every run afterwards.
//!
//! # What is frozen here, and why it may never be re-minted
//!
//! `tests/fixtures/attach_face_frozen/issued_2026_08_22/` holds five files, byte for byte:
//!
//! | file | what it is |
//! |---|---|
//! | `attach.json` | the answer `gx attach` gave when it placed the project the receipts come from |
//! | `commit_receipt.json` | the `CommitReceipt` of one transformation through that project |
//! | `verdict_receipt.json` | the `VerdictReceipt` of the same transformation |
//! | `checkpoint.json` | the signed head the ledger published at that moment |
//! | `key.pub.json` | the public half of the key that signed all three |
//!
//! **Regenerating these files defeats the entire probe** — the same instruction
//! `frozen_receipt_corpus.rs` carries for the 2026-08-18 specimen, and for the same reason. If a
//! change makes this suite red, the answer is code that still reads the frozen shape, never a
//! fresher specimen. The digests of the five files are in `req/548` rather than here, because
//! NFR-012's secret scanner reads a name followed by a long hex run as a keyed token and it is right
//! to.
//!
//! # 🔴 The weakness of this specimen, declared rather than left to be discovered
//!
//! `frozen_receipt_corpus.rs` states where a frozen receipt's value comes from: the binary under
//! test **did not mint it**. That is exactly what is *not* true here. These files were minted by
//! today's binary, at the start of this lane, so they **do not close the structural blindness** —
//! a change that moves what the encoder writes and what the decoder requires in one commit is still
//! invisible to them. What they close is drift **after** this lane: from here on, a P-1b change that
//! alters how an attach-face receipt reads has to make this suite red first.
//!
//! So AC-13 is a **partial** answer to KA-2 and not a replacement for `req/38` §294-2 (b)'s corpus.
//! The same sentence is carried in `docs/LIMITS.md`, following `req/519` §7-6's pairing: a
//! declaration, and a test that shows on every run that the limit is real.
//!
//! # Why the attach face, and not just any receipt
//!
//! `req/535` §2 defines an attach as three parts and P-1a implemented the first. The specimen is
//! minted on a project whose `.gx/` came from `gx attach` and from nothing else, and `attach.json`
//! is kept beside the receipts so that the provenance is a **document** rather than a claim in this
//! comment. [`the_specimen_was_minted_on_a_project_this_binary_attached`] reads it back.

mod support;

use std::path::{Path, PathBuf};

use support::{keypair, run, scratch, Run};

/// The frozen directory, and one file in it.
fn frozen(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("attach_face_frozen")
        .join("issued_2026_08_22")
        .join(name)
}

/// What a frozen receipt measures as, in the four quantities a re-mint cannot help but move.
#[derive(Debug, PartialEq, Eq)]
struct Census {
    /// The file's length on disk.
    file_bytes: u64,
    /// The length of the bytes the signature covers (DSSE PAE's payload).
    signed_bytes: usize,
    /// The clock read that sits **outside** the signed payload (E-M2-6 took it out of the core).
    issued_at: i64,
    /// How many signatures the envelope carries.
    signatures: usize,
}

/// The census of a receipt document, wherever it is.
fn census(path: &Path) -> Census {
    let raw = std::fs::read(path).unwrap_or_else(|e| panic!("{} is here ({e})", path.display()));
    let doc: serde_json::Value = serde_json::from_slice(&raw)
        .unwrap_or_else(|e| panic!("{} is a receipt document ({e})", path.display()));
    let payload = doc["envelope"]["payload"]
        .as_str()
        .expect("a DSSE envelope carries its payload as base64");
    Census {
        file_bytes: raw.len() as u64,
        signed_bytes: gx_core::b64::decode(payload)
            .expect("the payload is base64")
            .len(),
        issued_at: doc["issued_at"]
            .as_i64()
            .expect("42 §3.10's clock read, outside the signed core"),
        signatures: doc["envelope"]["signatures"]
            .as_array()
            .expect("a signature list")
            .len(),
    }
}

/// 🔴 The predicate both arms of [`the_specimen_is_the_one_that_was_minted_and_not_a_fresh_one`]
/// run: is this the document that was frozen, or is it a document that was made later?
///
/// It is a function rather than four assertions so that the negative arm can be **the same
/// question** asked of a receipt minted by today's binary. An assertion written twice is two
/// questions that look alike.
fn is_the_frozen_specimen(actual: &Census, expected: &Census) -> Result<(), String> {
    if actual == expected {
        return Ok(());
    }
    Err(format!(
        "this is not the frozen specimen: expected {expected:?}, measured {actual:?}"
    ))
}

/// The `CommitReceipt` frozen on 2026-08-22, as it measured then.
const COMMIT: Census = Census {
    file_bytes: 1222,
    signed_bytes: 666,
    issued_at: 1_787_382_413_313_435_361,
    signatures: 1,
};

/// The `VerdictReceipt` of the same transformation.
const VERDICT: Census = Census {
    file_bytes: 1074,
    signed_bytes: 555,
    issued_at: 1_787_382_413_261_700_168,
    signatures: 1,
};

/// `gx receipt verify <file> --offline …` run with **nothing** in its environment but `HOME`, from
/// an empty working directory — `receipt_verify_hermetic.rs`'s posture, reused rather than
/// re-argued.
fn verify_offline(receipt: &Path, cwd: &Path, home: &Path) -> Run {
    let mut cmd = support::gx();
    cmd.env_clear();
    cmd.env("HOME", home);
    cmd.current_dir(cwd);
    run(cmd
        .arg("receipt")
        .arg("verify")
        .arg(receipt)
        .arg("--offline")
        .arg("--checkpoint")
        .arg(frozen("checkpoint.json"))
        .arg("--checkpoint-key")
        .arg(frozen("key.pub.json"))
        .arg("--key")
        .arg(frozen("key.pub.json")))
}

/// 🔴 **AC-13, the standing claim** — both frozen receipts still verify offline, exit `0`.
///
/// This is the probe that has to survive every later change in this lane. It runs the **binary**
/// rather than the library, because `req/544` §5's AC is written as an exit status and because a
/// third party holds a process and four files, not a crate.
#[test]
fn the_attach_face_specimens_of_2026_08_22_still_verify_offline() {
    let cwd = scratch("ac13_cwd");
    let home = scratch("ac13_home");

    for (name, inclusion) in [
        ("commit_receipt.json", "verified"),
        ("verdict_receipt.json", "not_applicable"),
    ] {
        let out = verify_offline(&frozen(name), &cwd, &home);
        let json = out.json();
        println!("AC13_OFFLINE file={name} exit={} {json}", out.code);
        assert_eq!(
            out.code, 0,
            "🔴 the frozen {name} no longer verifies offline. This is AC-13's alarm: a P-1b change \
             moved what an attach-face receipt reads as. The answer is a decoder that still reads \
             the frozen shape, not a fresher specimen ({})",
            out.stderr
        );
        assert_eq!(json["valid"], serde_json::json!(true));
        assert_eq!(json["checks"]["signature"], serde_json::json!(true));
        assert_eq!(json["checks"]["canonical_cid"], serde_json::json!(true));
        assert_eq!(
            json["checks"]["inclusion"],
            serde_json::json!(inclusion),
            "the two kinds answer the ledger question differently, and both are a pass"
        );
        assert_eq!(json["anchor_authenticated"], serde_json::json!(true));
    }
}

/// 🔴 **AC-13's negative control** — the same question asked of a receipt minted **now** is refused.
///
/// Without this, the probe above is satisfied by a suite that re-mints the specimen on every run and
/// then verifies what it just made, which is the failure mode `frozen_receipt_corpus.rs` names in
/// its header. The control mints one through this binary's own library road and hands it to the
/// predicate the positive arm uses.
#[test]
fn the_specimen_is_the_one_that_was_minted_and_not_a_fresh_one() {
    for (name, expected) in [
        ("commit_receipt.json", COMMIT),
        ("verdict_receipt.json", VERDICT),
    ] {
        let measured = census(&frozen(name));
        println!("AC13_CENSUS file={name} {measured:?}");
        is_the_frozen_specimen(&measured, &expected).unwrap_or_else(|why| {
            panic!(
                "🔴 {name} is not the file that was frozen on 2026-08-22. {why}. Re-minting it is \
                 the one repair this suite forbids: the specimen's whole value is that this lane \
                 did not make it after the fact"
            )
        });
    }

    // The control: a receipt this binary mints right now, put to the same predicate.
    let key = keypair(13);
    let fresh = support::issue(
        &support::commit_payload(&key, 1_313, support::empty_proof()),
        &key,
    );
    let fresh_path = scratch("ac13_control").join("fresh.json");
    support::write_json(
        &fresh_path,
        &serde_json::to_value(&fresh).expect("serialises"),
    );
    let fresh_census = census(&fresh_path);
    println!("AC13_CONTROL_FRESH {fresh_census:?}");
    let refused = is_the_frozen_specimen(&fresh_census, &COMMIT);
    assert!(
        refused.is_err(),
        "🔴 a receipt minted by today's binary passed the frozen-specimen predicate, so the \
         predicate is not measuring freshness at all and the positive arm above proves nothing"
    );
    println!("AC13_CONTROL_REFUSAL={}", refused.unwrap_err());
}

/// 🔴 The provenance, read off a document rather than asserted in a comment.
///
/// The specimen is an **attach-face** specimen because the project it came from was placed by
/// `gx attach` and by nothing else, and `attach.json` is that operation's own answer. Freezing it
/// has a second use that P-1b needs: it pins the three sentences of
/// `NOT_CARRIED_BY_THIS_FACE` **as they stood before this lane**, so R-3g's requirement (that P-1b
/// replaces the second of the three and leaves the first and third verbatim) has a fixed point to
/// be measured against instead of a memory of one.
#[test]
fn the_specimen_was_minted_on_a_project_this_binary_attached() {
    let raw = std::fs::read(frozen("attach.json")).expect("the frozen attach answer is here");
    let doc: serde_json::Value = serde_json::from_slice(&raw).expect("it is `gx attach`'s answer");
    assert_eq!(doc["gx"], serde_json::json!("attach"));
    assert_eq!(
        doc["counts"]["total"].as_u64(),
        Some(11),
        "the enumeration P-1a froze is eleven rows"
    );
    assert_eq!(doc["counts"]["already_present"].as_u64(), Some(0));
    assert_eq!(
        doc["network"],
        serde_json::json!("none"),
        "and the placement opened no socket"
    );
    let unanswered = doc["not_carried_by_this_face"]
        .as_array()
        .expect("P-1a's three sentences");
    println!(
        "AC13_FROZEN_NOT_CARRIED n={} first={:?}",
        unanswered.len(),
        unanswered.first()
    );
    assert_eq!(
        unanswered.len(),
        3,
        "the face named three things it does not answer at freeze time; R-3g lets P-1b replace \
         exactly one of them"
    );
}
