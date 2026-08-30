// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **B-audit M-1 / `req/752`, N-47** — `gx_log::classify_extension` (fork detection) reached
//! through the CLI, not just a library test.
//!
//! `req/682` §2-2 names two branches for the offline witness: same-size disagreement
//! (`detect_equivocation`, wired to `gx checkpoint audit` since Phase B) and different-size
//! divergence (`classify_extension`, given a consistency proof). Only the first ever got a caller —
//! `crates/gx-log/tests/phase_b_witness.rs` and `witness_offline.rs` call `classify_extension`
//! directly, but no CLI verb, HTTP route, or other production path reaches it (confirmed by a
//! cross-crate grep: the audit found **zero** non-test call sites). `req/682` §6's own rule —
//! "a function split off needs a caller an E2E walks through" — is what `ledger.rs:879` cites as the
//! reason `detect_equivocation` got a verb; `classify_extension` did not meet its own bar.
//!
//! red-first: this suite is red against the tree `gx checkpoint audit` had before this lane —
//! `--proof` is not a flag `clap` knows, so `fork_is_named_...` fails at the process boundary
//! (a `2` usage exit, not a `7`), and `real_extension_across_two_sizes_is_not_a_false_fork` never
//! reaches the classifier at all. The wiring in `ledger::audit` (`--proof`, `req/682` §2-2's second
//! branch) is what turns both green.
//!
//! Fixture shape mirrors `crates/gx-log/tests/phase_b_witness.rs`'s `ac_b2`/`ac_b3` at the CLI/file
//! layer instead of `TileLog` in memory: two ledgers seeded from different bases share no leaf, so a
//! checkpoint of one at a given size has a different root than a checkpoint of the other at that same
//! size — the same asymmetry `phase_b_witness_cli.rs`'s `signed_checkpoint` uses for equivocation,
//! applied across sizes instead of within one.

mod support;

use std::path::{Path, PathBuf};

use gx_core::{Cid, Timestamp, TransformationId};
use gx_log::LedgerStore;

use support::{keypair, run, scratch};

/// A deterministic leaf, so two different `base`s produce ledgers sharing no leaf at any index.
fn leaf(base: u64, i: u64) -> (TransformationId, Cid) {
    (
        TransformationId(Cid([u8::try_from((base + i) % 251).expect("in range"); 32])),
        Cid([u8::try_from((base + i + 1) % 251).expect("in range"); 32]),
    )
}

/// Build (or extend) a ledger file at `project_dir/.gx/ledger` to exactly `to_size` leaves, seeded
/// from `base`. Extends in place, so calling this twice with the same `base` and a growing `to_size`
/// on the same project produces one continuous history — the append-only shape AC-B3 needs.
fn grow_ledger(layout: &gx_cli::layout::Layout, base: u64, to_size: u64) {
    let path = layout.ledger_path();
    std::fs::create_dir_all(path.parent().expect("has a parent")).expect("create ledger dir");
    let mut store = LedgerStore::open(&path).expect("open the ledger");
    let mut at = store.log().len();
    while at < to_size {
        let (tid, cid) = leaf(base, at);
        store
            .append(tid, cid, Timestamp(at as i64))
            .expect("append");
        at += 1;
    }
    let journal = layout.journal_path();
    if journal.is_file() {
        std::fs::remove_file(&journal).expect("remove the journal this fixture does not use");
    }
}

/// Sign the project's current ledger head to a file, and return the path.
fn checkpoint_to(project_dir: &Path, secret: &Path, out: &Path) -> PathBuf {
    let out_status = run(support::gx()
        .arg("--project")
        .arg(project_dir)
        .arg("log")
        .arg("checkpoint")
        .arg("--key")
        .arg(secret)
        .arg("--out")
        .arg(out));
    assert_eq!(
        out_status.code, 0,
        "gx log checkpoint signs the current tree: {}",
        out_status.stderr
    );
    out.to_path_buf()
}

/// A real consistency proof for `project_dir`'s own ledger, `from -> to`, written to `out`.
fn consistency_to(project_dir: &Path, from: u64, to: u64, out: &Path) -> PathBuf {
    let printed = run(support::gx()
        .arg("--project")
        .arg(project_dir)
        .arg("log")
        .arg("consistency")
        .arg("--from")
        .arg(from.to_string())
        .arg("--to")
        .arg(to.to_string()));
    assert_eq!(
        printed.code, 0,
        "gx log consistency over this project's own tree: {}",
        printed.stderr
    );
    std::fs::write(out, &printed.stdout).expect("write the proof");
    out.to_path_buf()
}

/// 🔴 **The positive case** — an "old" checkpoint from a **divergent** history, paired with a "new"
/// checkpoint and consistency proof from a different history that shares no leaf with it, is named a
/// `fork` and the audit exits `VERIFY_FAILED` (7). This is the CLI-reachable form of
/// `phase_b_witness.rs`'s `ac_b2_a_non_prefix_extension_is_a_fork`.
#[test]
fn fork_is_named_when_the_old_checkpoint_is_not_a_real_prefix_of_the_new_one() {
    let key = keypair(170);
    let out = scratch("b_audit_m1_fork");
    let secret = out.join("ledger.key");
    key.save(&secret).expect("save the ledger key");

    // "A": a 6-leaf history seeded from base 0.
    let (dir_a, layout_a) = support::project("b_audit_m1_hist_a");
    grow_ledger(&layout_a, 0, 6);
    let old = checkpoint_to(&dir_a, &secret, &out.join("old.json"));

    // "B": a divergent 12-leaf history seeded from base 50_000 -- shares no leaf with A, including
    // at size 6, so A@6's root is not B's root at size 6.
    let (dir_b, layout_b) = support::project("b_audit_m1_hist_b");
    grow_ledger(&layout_b, 50_000, 12);
    let new = checkpoint_to(&dir_b, &secret, &out.join("new.json"));
    // B's own, internally-valid proof bridging its own 6 -> 12 -- offered (wrongly) to bridge A@6.
    let proof = consistency_to(&dir_b, 6, 12, &out.join("proof.json"));

    std::fs::remove_file(&secret).expect("drop the secret; the audit needs no key");

    let audited = run(support::gx()
        .arg("checkpoint")
        .arg("audit")
        .arg(&old)
        .arg(&new)
        .arg("--proof")
        .arg(&proof));

    assert_eq!(
        audited.code, 7,
        "a non-prefix extension is VERIFY_FAILED (7): stdout={} stderr={}",
        audited.stdout, audited.stderr
    );
    let json = audited.json();
    assert_eq!(
        json["proof_checked"], true,
        "the proof was given and consulted: {json}"
    );
    let contradictions = json["contradictions"]
        .as_array()
        .expect("a contradictions array");
    assert_eq!(
        contradictions.len(),
        1,
        "exactly the fork classify_extension names, no more: {json}"
    );
    assert_eq!(contradictions[0]["kind"], "fork");
    assert_eq!(contradictions[0]["old_size"], 6);
    assert_eq!(contradictions[0]["new_size"], 12);
}

/// 🔴 **The negative control** — a genuine, continuous extension of one history, checkpointed at two
/// sizes with its own real consistency proof between them, is **not** a false fork. Mirrors
/// `phase_b_witness.rs`'s `ac_b3_a_real_extension_is_not_a_contradiction`: without this control, a
/// classifier that called every differently-sized pair a fork would also make this suite green.
#[test]
fn real_extension_across_two_sizes_is_not_a_false_fork() {
    let key = keypair(171);
    let out = scratch("b_audit_m1_no_fork");
    let secret = out.join("ledger.key");
    key.save(&secret).expect("save the ledger key");

    let (dir, layout) = support::project("b_audit_m1_real_ext");
    grow_ledger(&layout, 900, 6);
    let old = checkpoint_to(&dir, &secret, &out.join("old.json"));
    // Same project, same base: the tree is genuinely extended, not replaced.
    grow_ledger(&layout, 900, 12);
    let new = checkpoint_to(&dir, &secret, &out.join("new.json"));
    let proof = consistency_to(&dir, 6, 12, &out.join("proof.json"));

    std::fs::remove_file(&secret).expect("drop the secret");

    let audited = run(support::gx()
        .arg("checkpoint")
        .arg("audit")
        .arg(&old)
        .arg(&new)
        .arg("--proof")
        .arg(&proof));

    assert_eq!(
        audited.code, 0,
        "a real extension is self-consistent: stdout={} stderr={}",
        audited.stdout, audited.stderr
    );
    let json = audited.json();
    assert_eq!(json["proof_checked"], true);
    assert!(
        json["contradictions"].as_array().expect("array").is_empty(),
        "no false fork across a genuine extension: {json}"
    );
}

/// 🔴 Without `--proof`, `audit` still answers exactly as it always has: two differently-sized
/// checkpoints are silently outside the same-size arithmetic, `proof_checked` is `false`, and nothing
/// about the default road changed by adding the flag.
#[test]
fn omitting_proof_leaves_the_default_road_unchanged() {
    let key = keypair(172);
    let out = scratch("b_audit_m1_omitted");
    let secret = out.join("ledger.key");
    key.save(&secret).expect("save the ledger key");

    let (dir, layout) = support::project("b_audit_m1_omitted_proj");
    grow_ledger(&layout, 1900, 6);
    let old = checkpoint_to(&dir, &secret, &out.join("old.json"));
    grow_ledger(&layout, 1900, 12);
    let new = checkpoint_to(&dir, &secret, &out.join("new.json"));
    std::fs::remove_file(&secret).expect("drop the secret");

    let audited = run(support::gx()
        .arg("checkpoint")
        .arg("audit")
        .arg(&old)
        .arg(&new));

    assert_eq!(
        audited.code, 0,
        "no proof, no fork check, and two different sizes are not an equivocation either: {} {}",
        audited.stdout, audited.stderr
    );
    let json = audited.json();
    assert_eq!(json["proof_checked"], false);
    assert!(json["contradictions"].as_array().expect("array").is_empty());
}

/// 🔴 `--proof` with anything other than exactly two `--files` is a usage refusal, not a silent
/// best-effort guess at which pair the proof bridges.
#[test]
fn proof_with_one_file_is_a_usage_refusal() {
    let key = keypair(173);
    let out = scratch("b_audit_m1_onefile");
    let secret = out.join("ledger.key");
    key.save(&secret).expect("save the ledger key");

    let (dir, layout) = support::project("b_audit_m1_onefile_proj");
    grow_ledger(&layout, 2900, 6);
    let old = checkpoint_to(&dir, &secret, &out.join("old.json"));
    // A well-formed proof file's presence must not matter here; the file-count refusal fires first.
    let bogus_proof = out.join("bogus_proof.json");
    std::fs::write(&bogus_proof, b"{}").expect("write a placeholder");
    std::fs::remove_file(&secret).expect("drop the secret");

    let audited = run(support::gx()
        .arg("checkpoint")
        .arg("audit")
        .arg(&old)
        .arg("--proof")
        .arg(&bogus_proof));

    assert_ne!(
        audited.code, 0,
        "one file cannot be the pair a proof bridges: {} {}",
        audited.stdout, audited.stderr
    );
}
