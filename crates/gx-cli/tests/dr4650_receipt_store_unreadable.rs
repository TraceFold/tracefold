// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-50** (`req/973` §9-5, `req/1036` R11) — the receipt **store** not answering and a
//! stored receipt **document** not decoding are two different faults, and `gx undo`'s pre-flight
//! used to fold both into `WitnessMissing::Unreadable`, whose own word is "the archived commit
//! receipt would not decode" — a claim about a document a `std::fs::read` failure never got to
//! read (`crates/gx-cli/src/lifecycle.rs::settle_evidence`, the `Err(e)` arm on
//! `ReceiptStore::get`).
//!
//! `WitnessMissing::StoreUnreadable` is the new variant this file measures through the real
//! binary, and the discriminating half is the point: a store that will not open is a permissions
//! or filesystem problem, a payload that will not decode is a corrupt file, and an operator reading
//! the wrong one of the two goes to fix the wrong thing.
//!
//! # 🔴 The second defect this file caught, in the same lane, same turn
//!
//! `ReceiptStore::get` (`crates/gx-cli/src/receipt.rs`) returns `Error::Io` when `std::fs::read`
//! itself fails and `Error::Malformed` when the read succeeds but `read_receipt`'s
//! `serde_json::from_slice` does not — and the first cut of this repair matched `Err(e) => ...
//! StoreUnreadable` without looking at which shape `e` was, which folds `Malformed` into
//! `StoreUnreadable` and reproduces the exact access/decode confusion DR-46-50 was filed to close,
//! just moved to the other arm. [`a_malformed_receipt_is_still_reported_as_unreadable_not_store`]
//! is the regression test for that: it is the arm that would have failed against the first cut and
//! passes against the repair in `lifecycle.rs` that matches on `Error::Io` specifically.
//!
//! Both arms refuse (44 §1.4's **3**, `PRECONDITION_CHANGED` — `req/38` §132 ruling 2 mints no new
//! number for either), so both write nothing (`undo_cas_e2e.rs`'s residual: no journal record, no
//! receipt, no supersede edge), which is what makes it safe to run them against the same committed
//! transformation and close with a positive control that undoes it for real.

mod support;

use std::path::{Path, PathBuf};

use support::{pipeline, run};

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("set the mode");
}

/// `permissions_do_not_bind`'s shared carrier machinery lives in `support/mod.rs` (SS552 fold);
/// this wrapper is `#[track_caller]` so the printed site and the carrier line name this file's own
/// call site rather than `support/mod.rs`.
#[cfg(unix)]
#[track_caller]
fn permissions_do_not_bind(parent: &Path, child: &Path) -> bool {
    support::permissions_do_not_bind("dr4650", parent, child)
}

/// The one `.commit.json` file `commit_one` filed under `.gx/receipts/` — found by listing rather
/// than by rebuilding `ReceiptStore::path_of`'s naming from outside the crate, so this test does
/// not silently stop measuring anything the naming scheme changes.
fn commit_receipt_path(project: &Path) -> PathBuf {
    let dir = project.join(".gx").join("receipts");
    let mut hits: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the receipts directory exists after a commit")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".commit.json"))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "exactly one commit receipt after one commit_one: {hits:?}"
    );
    hits.pop().expect("the one hit")
}

#[cfg(unix)]
#[test]
fn store_and_decode_failures_are_reported_under_different_words() {
    let p = pipeline("dr4650_store_unreadable", "before\n");
    let t_o = p.commit_one("after\n");
    let receipts_dir = p.project.join(".gx").join("receipts");
    let receipt_path = commit_receipt_path(&p.project);
    let good_bytes = std::fs::read(&receipt_path).expect("read the good receipt back");

    // req/635 box L-4 (req/38 §394) — positive control taken *before* either fault: a healthy
    // pre-flight must not itself refuse under `--settle 0` review-only probing. `--dry-run` does
    // not exist on `gx undo` (44 §1.2), so the control is the final undo, held for last and run for
    // real once both faults have been measured and reverted.

    // ---------------------------------------------------------------------------------------
    // Arm 1 — decode failure: the store answers, the payload will not parse.
    // ---------------------------------------------------------------------------------------
    std::fs::write(&receipt_path, b"{ this is not a receipt }").expect("corrupt the payload");
    let decode_failure = run(p.gx().args(["undo", &t_o, "--settle", "0"]));
    std::fs::write(&receipt_path, &good_bytes).expect("restore the good receipt");
    println!(
        "DR4650 decode_failure exit={} stderr={}",
        decode_failure.code,
        decode_failure.stderr.trim()
    );
    assert_eq!(
        decode_failure.code, 3,
        "44 §1.4's 3 (`PRECONDITION_CHANGED`), req/38 §132 ruling 2's standing number: {}",
        decode_failure.stderr
    );
    assert!(
        decode_failure.stderr.contains("would not decode"),
        "a payload the store *did* hand back is `WitnessMissing::Unreadable`'s claim, unchanged \
         by DR-46-50: {}",
        decode_failure.stderr
    );
    assert!(
        !decode_failure.stderr.contains("receipt store would not answer"),
        "🔴 the regression this file exists to catch: `Error::Malformed` (the store answered, the \
         document did not decode) folded into `StoreUnreadable`'s wording would be the same \
         access/decode confusion DR-46-50 was filed to close, moved rather than fixed: {}",
        decode_failure.stderr
    );

    // ---------------------------------------------------------------------------------------
    // Arm 2 — store failure: `.gx/receipts/` itself will not open, so nothing is read at all.
    // ---------------------------------------------------------------------------------------
    if permissions_do_not_bind(&receipts_dir, &receipt_path) {
        return;
    }
    set_mode(&receipts_dir, 0o000);
    let store_failure = run(p.gx().args(["undo", &t_o, "--settle", "0"]));
    set_mode(&receipts_dir, 0o700);
    println!(
        "DR4650 store_failure exit={} stderr={}",
        store_failure.code,
        store_failure.stderr.trim()
    );
    assert_eq!(
        store_failure.code, 3,
        "same row (`witness-missing`), same code (req/38 §148: no new exit number): {}",
        store_failure.stderr
    );
    assert!(
        store_failure.stderr.contains("receipt store would not answer"),
        "`Error::Io` (nothing was read) is `WitnessMissing::StoreUnreadable`'s claim: {}",
        store_failure.stderr
    );
    assert!(
        !store_failure.stderr.contains("would not decode"),
        "🔴 the confusion DR-46-50 was filed against: an I/O failure that never read a document \
         must not be reported as a document that would not decode: {}",
        store_failure.stderr
    );
    assert!(
        store_failure
            .stderr
            .contains("\"gx_code\":\"PRECONDITION_CHANGED\""),
        "shared with the HTTP surface (`req/227` M-07's row): {}",
        store_failure.stderr
    );

    // ---------------------------------------------------------------------------------------
    // Positive control — both faults were transient and reverted; the same T_o still undoes.
    // ---------------------------------------------------------------------------------------
    let healthy = run(p.gx().args(["undo", &t_o, "--settle", "0"]));
    println!(
        "DR4650 healthy exit={} target={:?}",
        healthy.code,
        p.target_contents()
    );
    assert_eq!(
        healthy.code, 0,
        "neither refusal above left a mark: the healthy road still undoes T_o. stderr: {}",
        healthy.stderr
    );
    assert_eq!(
        p.target_contents(),
        "before\n",
        "and it actually restores the world, not only exits 0"
    );
}
