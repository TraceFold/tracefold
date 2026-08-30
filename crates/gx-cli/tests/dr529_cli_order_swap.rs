// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `req/529` residual cell — **cli × order-swap**, fired live.
//!
//! `req/529` §2's grid marks this cell `✘` (empty). This file builds a real 2-leaf ledger (same
//! shape `receipt_verify_history.rs` uses), then issues each leaf's receipt with the OTHER leaf's
//! inclusion proof swapped in before signing -- the shape a buggy or malicious issuer produces: a
//! validly-signed receipt whose `canonical_cid` names one transformation while its
//! `inclusion_proof` proves a different leaf's position. `gx receipt verify` (the CLI surface this
//! cell names) is then run against both swapped files.

mod support;

use gx_core::Timestamp;
use support::{commit_payload, issue, keypair, project, run, write_json, write_public_key};

/// **Fired live.** Two commits are appended to a real ledger; each one's receipt is issued with
/// the SIBLING leaf's inclusion proof (proof for leaf 1 attached to leaf 0's payload, and vice
/// versa) rather than its own -- signed normally, so the DSSE signature itself is genuine over the
/// swapped content (this is not a forgery test; `req/529` §2's signature-forgery cell already
/// covers that).
///
/// **Finding (H/M/L)**: `gx receipt verify` must refuse both swapped receipts -- accepting either
/// would be the H-class failure `req/529` §4-2's AC names directly: a destructive/inconsistent
/// input (a proof that does not prove the position its own `canonical_cid` claims) answered as
/// `valid`. The mechanism doing the refusing is the same inclusion-proof arithmetic
/// `receipt_verify_history.rs` already exercises (`gx_log::proof::verify_inclusion`/
/// `root_of_inclusion` disagreeing with what the receipt's own `canonical_cid` implies), applied
/// to a swap rather than a truncation or a bit-flip -- so this cell is the same mechanism as an
/// already-measured one, now actually fired rather than inferred, per `req/534` §1's own honest
/// framing of what "not individually fired" meant.
#[test]
fn dr529_cli_order_swap_of_two_receipts_inclusion_proofs_is_refused() {
    let (proj, layout) = project("dr529_cli_order_swap");
    let key = keypair(42);

    let path = layout.ledger_path();
    std::fs::create_dir_all(path.parent().expect("has a parent")).expect("create the ledger dir");
    let mut store = gx_log::LedgerStore::open(&path).expect("open the ledger");

    // Leaf 0 and leaf 1, two real commits.
    let seeds = [701u64, 702u64];
    let mut canonical_payloads = Vec::new();
    let mut proofs = Vec::new();
    for seed in seeds {
        let staged = commit_payload(&key, seed, support::empty_proof());
        let index = store.log().len();
        store
            .append(
                support::tid(seed),
                staged.ledger_digest().expect("canonical"),
                Timestamp(index as i64),
            )
            .expect("append");
        let proof = gx_log::proof::prove_inclusion(store.log(), index).expect("in the log");
        assert_eq!(
            proof.tree_size,
            index + 1,
            "proved at commit-time tree size"
        );
        canonical_payloads.push(commit_payload(&key, seed, support::empty_proof()));
        proofs.push(proof);
    }
    drop(store);

    let journal = layout.journal_path();
    if journal.is_file() {
        std::fs::remove_file(&journal).expect("remove the journal this fixture does not use");
    }

    let export = support::scratch("dr529_cli_order_swap_export");
    let key_path = write_public_key(&export, &key);

    // The swap: payload 0 gets proof 1, payload 1 gets proof 0.
    let swapped0 = commit_payload(&key, seeds[0], proofs[1].clone());
    let swapped1 = commit_payload(&key, seeds[1], proofs[0].clone());
    let receipt0 = issue(&swapped0, &key);
    let receipt1 = issue(&swapped1, &key);

    let file0 = write_json(
        &export.join("receipt0_swapped.json"),
        &serde_json::to_value(&receipt0).expect("serialises"),
    );
    let file1 = write_json(
        &export.join("receipt1_swapped.json"),
        &serde_json::to_value(&receipt1).expect("serialises"),
    );

    for (label, file) in [
        ("leaf0-with-leaf1-proof", &file0),
        ("leaf1-with-leaf0-proof", &file1),
    ] {
        let out = run(support::gx()
            .arg("--project")
            .arg(&proj)
            .arg("receipt")
            .arg("verify")
            .arg(file)
            .arg("--key")
            .arg(&key_path));
        println!(
            "DR529_CLI_ORDER_SWAP label={label} code={} stdout={}",
            out.code, out.stdout
        );
        assert_ne!(
            out.code, 0,
            "DR529 cli order-swap: {label} must NOT verify as exit 0 -- a swapped inclusion \
             proof accepted as valid is the H-class silent-wrong-answer this cell tests for. \
             stdout={} stderr={}",
            out.stdout, out.stderr
        );
        assert!(
            !out.stdout.contains("\"valid\":true"),
            "DR529 cli order-swap: {label} must not report valid:true. stdout={}",
            out.stdout
        );
    }
}

/// Control: the SAME two payloads, each with its OWN (correct) proof, both verify. Proves the
/// refusal above is about the swap specifically, not about this fixture being broken in general.
#[test]
fn dr529_cli_order_swap_control_unswapped_receipts_verify() {
    let (proj, layout) = project("dr529_cli_order_swap_control");
    let key = keypair(43);

    let path = layout.ledger_path();
    std::fs::create_dir_all(path.parent().expect("has a parent")).expect("create the ledger dir");
    let mut store = gx_log::LedgerStore::open(&path).expect("open the ledger");

    let seeds = [801u64, 802u64];
    let mut proofs = Vec::new();
    for seed in seeds {
        let staged = commit_payload(&key, seed, support::empty_proof());
        let index = store.log().len();
        store
            .append(
                support::tid(seed),
                staged.ledger_digest().expect("canonical"),
                Timestamp(index as i64),
            )
            .expect("append");
        let proof = gx_log::proof::prove_inclusion(store.log(), index).expect("in the log");
        proofs.push(proof);
    }
    drop(store);

    let journal = layout.journal_path();
    if journal.is_file() {
        std::fs::remove_file(&journal).expect("remove the journal this fixture does not use");
    }

    let export = support::scratch("dr529_cli_order_swap_control_export");
    let key_path = write_public_key(&export, &key);

    for (i, seed) in seeds.iter().enumerate() {
        let payload = commit_payload(&key, *seed, proofs[i].clone());
        let receipt = issue(&payload, &key);
        let file = write_json(
            &export.join(format!("receipt{i}_control.json")),
            &serde_json::to_value(&receipt).expect("serialises"),
        );
        let out = run(support::gx()
            .arg("--project")
            .arg(&proj)
            .arg("receipt")
            .arg("verify")
            .arg(&file)
            .arg("--key")
            .arg(&key_path));
        assert_eq!(
            out.code, 0,
            "DR529 control: an un-swapped, correctly-proved receipt must verify. stdout={} stderr={}",
            out.stdout, out.stderr
        );
    }
}
