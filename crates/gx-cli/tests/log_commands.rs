// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx log proof` / `gx log consistency` / `gx log checkpoint` — 44 §1.2 and **M6-24 adopted (b); sem: SEM-gx-cli-1907**.
//!
//! # What each probe is for
//!
//! `proof` and `consistency` are thin: the arithmetic is gx-log's and was measured in M2 (AC-021 —
//! AC-023). What the CLI adds and what is measured here is the **resolution** 44 §1.2 asks for
//! ("`--leaf <INDEX|TRANSFORMATION_ID>`"; sem: SEM-gx-cli-1908), the refusal shape for a leaf that is not there, and the
//! fact that a read command does not create a ledger it did not find.
//!
//! `checkpoint` is not thin. It is M6-24 adopted (b; sem: SEM-gx-cli-1909), the first caller of `gx_witness::dsse::sign_checkpoint`
//! outside gx-witness, and without it AC-057 has no Given.

mod support;

use gx_log::LedgerStore;
use support::{keypair, project, run, scratch, tid};

/// Both spellings of `--leaf` resolve to the same proof, and the proof verifies against the head.
///
/// The second half is what stops this from being a test of `serde`: a command that emitted a
/// syntactically valid `InclusionProof` for the wrong leaf would pass an output-shape assertion.
#[test]
fn proof_resolves_both_spellings_of_leaf_and_the_proof_holds() {
    let (dir, layout) = project("log_proof");
    let key = keypair(5);
    let (_receipt, index) = support::seed_ledger(&layout, &key, 21, 6);

    let by_index = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("log")
        .arg("proof")
        .arg("--leaf")
        .arg(index.to_string()));
    let by_id = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("log")
        .arg("proof")
        .arg("--leaf")
        .arg(tid(21).0.to_text()));
    println!(
        "LOG_PROOF_BY_INDEX exit={} {}\nLOG_PROOF_BY_ID exit={} {}",
        by_index.code,
        by_index.json(),
        by_id.code,
        by_id.json()
    );
    assert_eq!(by_index.code, 0);
    assert_eq!(by_id.code, 0);
    assert_eq!(
        by_index.json(),
        by_id.json(),
        "44 §0's id-resolution: both spellings name one leaf"
    );
    assert_eq!(by_index.json()["leaf_index"], serde_json::json!(index));

    // The proof is a real one: gx-log walks it back to the tree's own root.
    let store = LedgerStore::open(layout.ledger_path()).expect("open");
    let proof: gx_core::InclusionProof =
        serde_json::from_value(by_index.json()).expect("an InclusionProof");
    let root = store.log().root().expect("a non-empty log has a root");
    let entry = store.log().entry(index).expect("the leaf is there");
    assert!(
        gx_log::proof::verify_inclusion(&proof, &root, entry).expect("canonical"),
        "the emitted proof reaches the root of the tree it came from"
    );
}

/// A leaf that is not in the log is 44 §1.2's `1=out-of-range/not-found` (sem: SEM-gx-cli-1910), with the tree size in the object.
///
/// 🔴 44 §1.4's common table gives "not-found" (sem: SEM-gx-cli-1911) the code **6** and 44 §1.2's `log` line gives it 1. The
/// per-command text wins here because it is the more specific statement; the divergence is M6-25's
/// material and `crates/gx-cli/src/exit.rs` carries it as a note rather than repairing it.
#[test]
fn a_leaf_that_is_not_there_is_refused_with_the_tree_size() {
    let (dir, layout) = project("log_proof_missing");
    let key = keypair(6);
    support::seed_ledger(&layout, &key, 22, 2);

    for leaf in ["9999", &tid(1234).0.to_text()] {
        let out = run(support::gx()
            .arg("--project")
            .arg(&dir)
            .arg("log")
            .arg("proof")
            .arg("--leaf")
            .arg(leaf));
        let json = out.json();
        println!("LOG_PROOF_MISSING {leaf} exit={} {json}", out.code);
        assert_eq!(
            out.code, 6,
            "🔴 **E-M6-24** (req/38 §55): 44 §1.2's `log` line writes \"1=out-of-range/not-found\" (sem: SEM-gx-cli-1912) and §1.4's \
             common table gives \"not-found\" (sem: SEM-gx-cli-1912) the code **6**. M6-25 rules that the common \
             table wins and the per-command list is an excerpt; E-M6-13/16 applied that to \
             `cancel`, `escalation` and `undo`, and this verb is the third"
        );
        assert_eq!(json["found"], serde_json::json!(false));
        assert_eq!(json["tree_size"], serde_json::json!(3));
    }

    // And a `--leaf` that is neither an index nor an id is "invalid input" (sem: SEM-gx-cli-1913) — a different failure, and
    // discipline 52 (sem: SEM-gx-cli-1913) still sends it to 1 rather than to clap's 2.
    let out = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("log")
        .arg("proof")
        .arg("--leaf")
        .arg("not-an-id"));
    println!(
        "LOG_PROOF_BAD_LEAF exit={} stderr={}",
        out.code,
        out.stderr.trim()
    );
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("VALIDATION_ERROR"));
}

/// `consistency` proves between two sizes, and answers an impossible pair rather than panicking.
#[test]
fn consistency_proves_between_sizes_and_reports_an_impossible_pair() {
    let (dir, layout) = project("log_consistency");
    let key = keypair(7);
    support::seed_ledger(&layout, &key, 23, 5);

    let out = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("log")
        .arg("consistency")
        .arg("--from")
        .arg("2")
        .arg("--to")
        .arg("6"));
    println!("LOG_CONSISTENCY exit={} {}", out.code, out.json());
    assert_eq!(out.code, 0);

    let store = LedgerStore::open(layout.ledger_path()).expect("open");
    let proof: gx_log::ConsistencyProof =
        serde_json::from_value(out.json()).expect("a ConsistencyProof");
    let old_root = store.log().root_at(2).expect("a root at 2");
    let new_root = store.log().root_at(6).expect("a root at 6");
    assert!(
        gx_log::proof::verify_consistency(&proof, &old_root, &new_root).expect("canonical"),
        "the emitted proof links the two roots of the tree it came from"
    );

    let out = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("log")
        .arg("consistency")
        .arg("--from")
        .arg("6")
        .arg("--to")
        .arg("2"));
    let json = out.json();
    println!("LOG_CONSISTENCY_BACKWARDS exit={} {json}", out.code);
    assert_eq!(
        out.code, 6,
        "**E-M6-24**: \"out-of-range\" is \"not-found\"'s other face here and both take §1.4's 6 (sem: SEM-gx-cli-1914)"
    );
    assert_eq!(json["tree_size"], serde_json::json!(6));
    assert!(
        json["refusal"].as_str().is_some(),
        "gx-log's own reason is carried rather than re-decided"
    );
}

/// 🔴 **M6-24 adopted (b); sem: SEM-gx-cli-1915**: the checkpoint producer, and the fact that its signature checks out.
///
/// Before this hand `sign_checkpoint` had no caller outside gx-witness, so no signed head existed in
/// the shipping code at all. The probe verifies the signature with `gx_witness::dsse::verify_checkpoint`
/// rather than only asserting that a `signature` field is non-empty: an empty signature is a value
/// too, and E-M2-26 fixed *which* bytes are signed (a pre-authentication encoding, not the bare
/// core), so a producer that signed the wrong message would still produce sixty-four bytes.
#[test]
fn checkpoint_publishes_a_signed_head_whose_signature_verifies() {
    let (dir, layout) = project("log_checkpoint");
    let key = keypair(8);
    support::seed_ledger(&layout, &key, 24, 4);
    let scratch_dir = scratch("log_checkpoint_out");
    let key_dir = support::secure_scratch("log_checkpoint_key");
    let secret = key_dir.join("ledger.key");
    key.save(&secret).expect("save");
    let out_path = scratch_dir.join("heads").join("head.json");

    let out = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("log")
        .arg("checkpoint")
        .arg("--key")
        .arg(&secret)
        .arg("--out")
        .arg(&out_path));
    let json = out.json();
    println!("LOG_CHECKPOINT exit={} {json}", out.code);
    assert_eq!(out.code, 0);

    let checkpoint: gx_core::Checkpoint =
        serde_json::from_value(json.clone()).expect("a Checkpoint");
    assert_eq!(checkpoint.tree_size, 5, "four others plus the one");
    assert_eq!(
        checkpoint.origin, "glovrex-ledger/v1",
        "42 §3.11's namespace"
    );
    assert_eq!(checkpoint.signature.keyid, *key.key_id());
    assert_eq!(checkpoint.signature.sig.len(), 64);
    gx_witness::dsse::verify_checkpoint(&checkpoint, &key.verifying())
        .expect("E-M2-26's message is what was signed");

    // `--out` wrote the same document stdout carried, byte for byte.
    let stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&out_path).expect("read")).expect("json");
    assert_eq!(stored, json, "one serialisation for the pipe and the file");

    // 🔴 §47 M6-24: "only the ledger's owner can create it" (sem: SEM-gx-cli-1916). Without a key there is nothing to sign with, and
    // the refusal says so rather than inventing one.
    let out = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("log")
        .arg("checkpoint"));
    println!(
        "LOG_CHECKPOINT_NO_KEY exit={} stderr={}",
        out.code,
        out.stderr.trim()
    );
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("VALIDATION_ERROR"));
}

/// 🔴 A read command does not create the ledger it failed to find.
///
/// `LedgerStore::open` creates the file when it is absent, which is right for an engine starting up
/// and wrong for four commands that read: a `gx log proof` that left an empty ledger behind would
/// make "this project has no ledger" (sem: SEM-gx-cli-1917) unobservable after the first attempt, and the second run would
/// report an empty tree instead of an absent one.
#[test]
fn reading_a_project_with_no_ledger_leaves_no_ledger_behind() {
    let (dir, layout) = project("log_no_ledger");
    assert!(!layout.ledger_path().exists());

    let out = run(support::gx()
        .arg("--project")
        .arg(&dir)
        .arg("log")
        .arg("proof")
        .arg("--leaf")
        .arg("0"));
    println!(
        "LOG_NO_LEDGER exit={} stderr={}",
        out.code,
        out.stderr.trim()
    );
    assert_eq!(
        out.code, 6,
        "44 §1.4's \"not-found\" (sem: SEM-gx-cli-1918) — there is no log to be out of range of"
    );
    assert!(out.stderr.contains("NOT_FOUND"));
    assert!(
        !layout.ledger_path().exists(),
        "the read created a ledger: \"absent\" has become \"empty\" (sem: SEM-gx-cli-1919) and the difference is gone"
    );
}
