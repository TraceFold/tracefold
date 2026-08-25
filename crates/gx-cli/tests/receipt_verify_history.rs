// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **H-09** — a receipt older than the head still verifies, and a receipt of a leaf that is not
//! in the tree still does not.
//!
//! `req/222` §4 row 6 measured the false negative this suite exists to keep closed: in a project
//! with three commits, `gx receipt verify <FILE>` answered
//!
//! ```text
//! leaf0  {"valid":false, "checks":{"inclusion":"refuted"}, "anchor":"local-ledger"}
//! leaf1  {"valid":false, "checks":{"inclusion":"refuted"}, "anchor":"local-ledger"}
//! leaf2  {"valid":true,  "checks":{"inclusion":"verified"}, "anchor":"local-ledger"}
//! ```
//!
//! — two accusations of tampering produced by nothing but the log growing. The cause is arithmetic
//! and not policy: an `InclusionProof` names a `tree_size` and reaches exactly one root
//! (`gx_log::proof::prove_inclusion_at`), while the default anchor is `local_head(now)`, a
//! checkpoint of the tree **as it stands**. For every receipt but the newest those are two different
//! trees, so `verify_inclusion_of` was being asked a question whose answer is `false` for honest and
//! forged receipts alike. A verification mark drawn from that output would mark two rows in three as
//! refuted, which is the GUI premise `req/38` §160 made GO condition 6.
//!
//! # What the repair is, and what it is not
//!
//! RFC 6962 §2.1.2's consistency proof is the missing link, and `gx-log` has had both halves of it
//! since M2 (`prove_consistency` / `verify_consistency`, `gx log consistency`). What was missing was
//! the carriage. So:
//!
//! * the root at the receipt's own `tree_size` is **computed from the receipt**
//!   (`gx_log::proof::root_of_inclusion`) — it is not an input, so it cannot be forged;
//! * a consistency proof carries that root to the anchor's `root_hash`;
//! * the anchor is what the verifier already believed.
//!
//! Three links, none unchecked. That is why the answer is `verified` and not some softer word: the
//! leaf really is in the tree the anchor commits to. Nothing was widened — [`a_leaf_that_is_not_in_the_tree_is_refuted`]
//! and [`the_newest_receipt_still_refuses_a_forged_leaf`] hold the other side, and
//! [`an_unanchored_verification_does_not_exit_zero`] holds H5-9.
//!
//! What *did* need a new word is the case where nothing can bridge: a third party holding a receipt
//! and a later checkpoint and no proof between them. That was `refuted` before and is `unbridged`
//! now — **not** a pass ([`gx_witness::receipt::Checks::verified`] excludes it, exit stays 7), and
//! not a refutation either, because no evidence was offered against the receipt.
//! [`a_later_anchor_with_nothing_to_bridge_it_is_unbridged_not_refuted`] is that probe, and its
//! second half hands the proof over and watches the same pair become `verified`.

mod support;

use std::path::{Path, PathBuf};

use gx_core::Timestamp;
use gx_witness::KeyPair;
use support::{issue, keypair, run, scratch, write_json, write_public_key, Run};

/// The seeds of the three commits, in the order they are appended.
const SEEDS: [u64; 3] = [901, 902, 903];

/// A project whose ledger holds three commit receipts, each proved at the size the log had when it
/// was issued — which is how `gx commit` issues them (43 T-11, and `support::seed_ledger`'s note).
struct History {
    project: PathBuf,
    export: PathBuf,
    /// `receipts[i]` is the receipt of leaf `i`.
    receipts: Vec<PathBuf>,
    key_path: PathBuf,
    key: KeyPair,
    /// The tree size after all three appends.
    head_size: u64,
}

fn given(name: &str) -> History {
    let (project, layout) = support::project(&format!("{name}_project"));
    let export = scratch(&format!("{name}_export"));
    let key = keypair(9);

    let path = layout.ledger_path();
    std::fs::create_dir_all(path.parent().expect("has a parent")).expect("create the ledger dir");
    let mut store = gx_log::LedgerStore::open(&path).expect("open the ledger");

    let mut receipts = Vec::new();
    for seed in SEEDS {
        // 43 T-11's order: stage the payload, append its digest, take the proof **at that size**,
        // then sign. A fixture that proved every leaf against the final tree would be measuring a
        // ledger nothing in this repository produces.
        let staged = support::commit_payload(&key, seed, support::empty_proof());
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
            "leaf {index} is proved against the tree as it stood at commit time"
        );
        let receipt = issue(&support::commit_payload(&key, seed, proof), &key);
        receipts.push(write_json(
            &export.join(format!("receipt{index}.json")),
            &serde_json::to_value(&receipt).expect("serialises"),
        ));
    }
    let head_size = store.log().len();
    drop(store);

    // 🔴 **R39 / `req/540` R-1c** — and the project is left **without a journal**, for the reason
    // `support::seed_ledger`'s note carries at length.
    //
    // This fixture casts `.gx/ledger` by hand, the same way `seed_ledger` does, so it builds the
    // same shape: a ledger holding three leaves beside a journal witnessing none of them, which is
    // `ledger_agrees() == false`. Until R39 the product carried a clause that let that shape
    // through, and audit 38 entered the clause by deleting one file. With the clause gone,
    // `History::checkpoint` is refused — correctly, because the project it is asking about is
    // describing two trees.
    //
    // Removing the journal makes this what it always meant to be: the third-party shape, a ledger
    // file and no project. Every verb this suite drives (`gx receipt verify`, `gx log checkpoint`,
    // `gx log consistency`) reads the ledger file, and none of them needs a journal. 🔴 This one was
    // **not** found by reading `seed_ledger`'s call sites — it mints its own ledger and so is
    // outside that list. The full floor found it (`req/542` §7).
    let journal = layout.journal_path();
    if journal.is_file() {
        std::fs::remove_file(&journal).expect("remove the journal this fixture does not use");
    }

    let key_path = write_public_key(&export, &key);
    History {
        project,
        export,
        receipts,
        key_path,
        key,
        head_size,
    }
}

impl History {
    /// `gx receipt verify <FILE> --key <FILE>` with the **default** anchor: the local ledger's head
    /// as it stands. This is the invocation a GUI drawing a per-row mark makes.
    fn verify_against_head(&self, receipt: &Path) -> Run {
        run(support::gx()
            .arg("--project")
            .arg(&self.project)
            .arg("receipt")
            .arg("verify")
            .arg(receipt)
            .arg("--key")
            .arg(&self.key_path))
    }

    /// A signed head of the current tree, written to `checkpoint.json` — the third party's anchor.
    fn checkpoint(&self, name: &str) -> PathBuf {
        let secret = support::secure_scratch(name).join("ledger.key");
        self.key.save(&secret).expect("save the ledger key");
        let out = self.export.join("checkpoint.json");
        let made = run(support::gx()
            .arg("--project")
            .arg(&self.project)
            .arg("log")
            .arg("checkpoint")
            .arg("--key")
            .arg(&secret)
            .arg("--out")
            .arg(&out));
        assert_eq!(
            made.code, 0,
            "the checkpoint producer runs: {}",
            made.stderr
        );
        std::fs::remove_file(&secret).expect("the verifier does not hold the signing key");
        out
    }

    /// `gx log consistency --from --to`, captured to a file: what `--consistency` reads.
    fn consistency(&self, from: u64, to: u64) -> PathBuf {
        let out = run(support::gx()
            .arg("--project")
            .arg(&self.project)
            .arg("log")
            .arg("consistency")
            .arg("--from")
            .arg(from.to_string())
            .arg("--to")
            .arg(to.to_string()));
        assert_eq!(out.code, 0, "the proof exists: {}", out.stderr);
        write_json(
            &self.export.join(format!("consistency_{from}_{to}.json")),
            &out.json(),
        )
    }
}

/// 🔴 The measurement `req/222` took, taken again: every receipt in the history, against the head
/// **now**.
#[test]
fn every_receipt_in_the_history_verifies_against_the_current_head() {
    let h = given("h09_history");
    assert_eq!(h.head_size, SEEDS.len() as u64);

    for (index, receipt) in h.receipts.iter().enumerate() {
        let out = h.verify_against_head(receipt);
        let json = out.json();
        println!(
            "H09_LEAF{index} exit={} {json} stderr={:?}",
            out.code,
            out.stderr.trim()
        );
        assert_eq!(
            json["checks"]["inclusion"],
            serde_json::json!("verified"),
            "leaf {index} is in the tree the head commits to, and the head can prove it \
             (RFC 6962 2.1.2)"
        );
        assert_eq!(json["valid"], serde_json::json!(true), "leaf {index}");
        assert_eq!(json["anchor"], serde_json::json!("local-ledger"));
        assert_eq!(out.code, 0, "leaf {index}: {}", out.stderr);
    }
}

/// 🔴 The other side: a receipt whose leaf is **not** in the tree is refused, and refused as
/// `refuted` rather than as a gap.
///
/// The forgery is the sharpest one this repair could have let through: a real inclusion proof, from
/// this very log, at leaf 0's size — attached to a payload naming a different transformation. Under
/// the repair the reconstructed root at that size comes out wrong, so the consistency proof (which
/// is genuine, and about the right pair of sizes) does not land on the head. Signature and canonical
/// CID both pass, which is what makes this a test of the inclusion half and not of the DSSE half.
#[test]
fn a_leaf_that_is_not_in_the_tree_is_refuted() {
    let h = given("h09_forged");

    let stored: gx_witness::Receipt =
        serde_json::from_slice(&std::fs::read(&h.receipts[0]).expect("read leaf 0's receipt"))
            .expect("a receipt");
    let borrowed = stored
        .payload()
        .expect("decodes")
        .inclusion_proof
        .expect("a CommitReceipt carries one");

    // A payload nothing ever appended, wearing leaf 0's proof.
    let forged = issue(&support::commit_payload(&h.key, 4_242, borrowed), &h.key);
    let path = write_json(
        &h.export.join("forged.json"),
        &serde_json::to_value(&forged).expect("serialises"),
    );

    let out = h.verify_against_head(&path);
    let json = out.json();
    println!(
        "H09_FORGED exit={} {json} stderr={:?}",
        out.code,
        out.stderr.trim()
    );
    assert_eq!(
        json["checks"]["signature"],
        serde_json::json!(true),
        "the forgery is signed by the same key: the inclusion half is what must refuse"
    );
    assert_eq!(
        json["checks"]["inclusion"],
        serde_json::json!("refuted"),
        "a leaf that is not in the tree is evidence against, and keeps the word for it"
    );
    assert_eq!(json["valid"], serde_json::json!(false));
    assert_eq!(out.code, 7, "44 1.4's 7: {}", out.stderr);
}

/// 🔴 The same forgery at the **newest** size, so the direct branch (`m == n`) is measured too.
///
/// The two branches of the repair reach `refuted` by different roads — one equality against the
/// anchor's root, one walk across a consistency proof — and a probe that only exercised the bridged
/// road would leave the older, simpler one uncovered.
#[test]
fn the_newest_receipt_still_refuses_a_forged_leaf() {
    let h = given("h09_forged_head");

    let stored: gx_witness::Receipt = serde_json::from_slice(
        &std::fs::read(h.receipts.last().expect("three receipts")).expect("read the last receipt"),
    )
    .expect("a receipt");
    let borrowed = stored
        .payload()
        .expect("decodes")
        .inclusion_proof
        .expect("a CommitReceipt carries one");
    assert_eq!(
        borrowed.tree_size, h.head_size,
        "the newest receipt names the tree the head names, so no bridge is involved"
    );

    let forged = issue(&support::commit_payload(&h.key, 4_243, borrowed), &h.key);
    let path = write_json(
        &h.export.join("forged_head.json"),
        &serde_json::to_value(&forged).expect("serialises"),
    );

    let out = h.verify_against_head(&path);
    let json = out.json();
    println!("H09_FORGED_HEAD exit={} {json}", out.code);
    assert_eq!(json["checks"]["inclusion"], serde_json::json!("refuted"));
    assert_eq!(json["valid"], serde_json::json!(false));
    assert_eq!(out.code, 7, "{}", out.stderr);
}

/// 🔴 H5-9 is untouched: no anchor is still not a pass, and still not exit 0.
///
/// Kept here rather than left to `receipt_verify_hermetic.rs` because this suite is the one that
/// widened what `verified` can mean, and the floor under it has to be measured in the same file
/// that raised the ceiling.
#[test]
fn an_unanchored_verification_does_not_exit_zero() {
    let h = given("h09_unanchored");
    let out = run(support::gx()
        .current_dir(&h.export)
        .arg("receipt")
        .arg("verify")
        .arg(&h.receipts[0])
        .arg("--offline")
        .arg("--key")
        .arg(&h.key_path)
        .env("HOME", h.export.join("no-such-home")));
    let json = out.json();
    println!("H09_UNANCHORED exit={} {json}", out.code);
    assert_eq!(json["checks"]["inclusion"], serde_json::json!("unanchored"));
    assert_eq!(json["anchor"], serde_json::json!("none"));
    assert_eq!(json["valid"], serde_json::json!(false));
    assert_ne!(out.code, 0, "H5-9: an unchecked ledger claim is not a pass");
    assert_eq!(out.code, 7);
}

/// 🔴 The honest new state, and the road out of it.
///
/// A third party holds leaf 0's receipt and a signed head of a tree that has grown past it, and
/// nothing else. Before the repair that pair answered `refuted`; the offered evidence contained no
/// accusation, so the word was wrong. Now it answers `unbridged`, exit **7** — not a pass, because
/// nothing checked the ledger claim.
///
/// Then the same pair is handed the one document that closes it, and the answer becomes `verified`
/// with the anchor still coming from a **file** (`.gx/` is never opened on this road).
#[test]
fn a_later_anchor_with_nothing_to_bridge_it_is_unbridged_not_refuted() {
    let h = given("h09_unbridged");
    let checkpoint = h.checkpoint("h09_unbridged_key");
    let no_home = h.export.join("no-such-home");

    let bare = run(support::gx()
        .current_dir(&h.export)
        .arg("receipt")
        .arg("verify")
        .arg(&h.receipts[0])
        .arg("--offline")
        .arg("--checkpoint")
        .arg(&checkpoint)
        .arg("--key")
        .arg(&h.key_path)
        .env("HOME", &no_home));
    let json = bare.json();
    println!(
        "H09_UNBRIDGED exit={} {json} stderr={:?}",
        bare.code,
        bare.stderr.trim()
    );
    assert_eq!(
        json["checks"]["inclusion"],
        serde_json::json!("unbridged"),
        "two statements about two trees, and nothing between them"
    );
    assert_eq!(json["valid"], serde_json::json!(false));
    assert_eq!(bare.code, 7, "not a pass");
    assert!(
        bare.stderr.contains("unbridged") && bare.stderr.contains("gx log consistency"),
        "the note names the way out: {:?}",
        bare.stderr
    );

    // The way out, taken.
    let bridge = h.consistency(1, h.head_size);
    let bridged = run(support::gx()
        .current_dir(&h.export)
        .arg("receipt")
        .arg("verify")
        .arg(&h.receipts[0])
        .arg("--offline")
        .arg("--checkpoint")
        .arg(&checkpoint)
        .arg("--consistency")
        .arg(&bridge)
        .arg("--key")
        .arg(&h.key_path)
        .env("HOME", &no_home));
    let json = bridged.json();
    println!("H09_BRIDGED exit={} {json}", bridged.code);
    assert_eq!(json["checks"]["inclusion"], serde_json::json!("verified"));
    assert_eq!(json["anchor"], serde_json::json!("checkpoint-file"));
    assert_eq!(json["valid"], serde_json::json!(true));
    assert_eq!(bridged.code, 0, "{}", bridged.stderr);
    assert!(
        !h.export.join(".gx").exists(),
        "the bridged road opened no project"
    );
}

/// 🔴 A bridge about **other** sizes is not partial evidence.
///
/// The proof handed in is genuine — this log really did grow from 2 leaves to 3 — but the receipt
/// names 1. Accepting it would let a holder of any consistent pair launder a receipt from a third
/// tree, so the answer stays `unbridged`: the verifier was given nothing about the question it
/// asked.
#[test]
fn a_consistency_proof_between_the_wrong_sizes_bridges_nothing() {
    let h = given("h09_wrong_bridge");
    let checkpoint = h.checkpoint("h09_wrong_bridge_key");
    let wrong = h.consistency(2, h.head_size);

    let out = run(support::gx()
        .current_dir(&h.export)
        .arg("receipt")
        .arg("verify")
        .arg(&h.receipts[0])
        .arg("--offline")
        .arg("--checkpoint")
        .arg(&checkpoint)
        .arg("--consistency")
        .arg(&wrong)
        .arg("--key")
        .arg(&h.key_path)
        .env("HOME", h.export.join("no-such-home")));
    let json = out.json();
    println!("H09_WRONG_BRIDGE exit={} {json}", out.code);
    assert_eq!(json["checks"]["inclusion"], serde_json::json!("unbridged"));
    assert_eq!(json["valid"], serde_json::json!(false));
    assert_eq!(out.code, 7);
}
