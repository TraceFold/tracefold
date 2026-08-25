// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! ASM-43-1 — `ledger.append` is keyed and idempotent (H2-3, ruled onto this hand by req/38 §11).
//!
//! ASM-43-1 verbatim (43 §additional-ASM proposal): "`ledger.append` writes into the contract, in
//! words, that it is an idempotent operation keyed on `TransformationId` (or a canonical CID)
//! (appended to gx-log's API contract)", and req/38 §11 states the
//! two arms this hand owes: "a repeat append under the same key is a no-op for the same CID, a
//! reject for a different CID". (sem: SEM-gx-log-144)
//!
//! # Which CID
//!
//! Not `leaf_cid`. A leaf hash covers the entry's `index`, and a second append of the same
//! transformation would be offered at a different index, so comparing leaf hashes would make every
//! repeat a conflict -- the arm ASM-43-1 exists to prevent. What a caller offers is a
//! `receipt_digest`, so that is what "the same CID" (sem: SEM-gx-log-145) is read as here, and `store.rs` says so in its
//! documentation rather than leaving the reading in a test.
//!
//! # Which type
//!
//! [`gx_log::LedgerStore`] and not [`gx_log::tile::TileLog`]. INV-S3's exactly-once is a property
//! of the ledger -- the thing that survives a restart and therefore has something to be idempotent
//! *about*. The tree is a fold over leaves with no key index, and a caller reaching for it directly
//! gets the tree's contract and not the ledger's. That split is raised as H3-2 in req/52 §4.

mod support;

use gx_core::Timestamp;
use gx_log::{AppendOutcome, Error, LedgerStore};
use std::fs;
use support::{cid, scratch, tid};

/// The first arm: the same key with the same digest changes nothing.
#[test]
fn a_repeat_of_the_same_append_is_a_no_op() {
    let path = scratch("idem_same").join("ledger.log");
    let mut store = LedgerStore::open(&path).expect("open");

    let first = store
        .append(tid(0), cid(1_000), Timestamp(7))
        .expect("the first append");
    assert!(matches!(first, AppendOutcome::Appended(_)));
    let size = fs::metadata(&path).expect("metadata").len();

    // A later clock and the same content: "the same key, the same CID" (sem: SEM-gx-log-146) is about what was recorded, not about
    // when the caller tried again.
    let second = store
        .append(tid(0), cid(1_000), Timestamp(99))
        .expect("the repeat");
    assert!(
        matches!(second, AppendOutcome::AlreadyPresent(_)),
        "a repeat must be reported as a repeat, not silently appended: {second:?}"
    );
    assert_eq!(
        second.entry(),
        first.entry(),
        "the repeat answers with the entry that is in the log, timestamp included"
    );
    assert_eq!(store.log().len(), 1, "no second leaf");
    assert_eq!(
        fs::metadata(&path).expect("metadata").len(),
        size,
        "a no-op writes no bytes"
    );
}

/// The second arm: the same key with a different digest is refused.
#[test]
fn the_same_key_with_a_different_digest_is_rejected() {
    let path = scratch("idem_conflict").join("ledger.log");
    let mut store = LedgerStore::open(&path).expect("open");
    store
        .append(tid(0), cid(1_000), Timestamp(0))
        .expect("the first append");
    let size = fs::metadata(&path).expect("metadata").len();

    let clash = store.append(tid(0), cid(2_000), Timestamp(1));
    assert_eq!(
        clash,
        Err(Error::Conflict {
            transformation: tid(0),
            recorded: cid(1_000),
            offered: cid(2_000),
        }),
        "a second answer for one transformation is a conflict, not an append"
    );
    assert_eq!(store.log().len(), 1);
    assert_eq!(
        fs::metadata(&path).expect("metadata").len(),
        size,
        "a rejected append writes no bytes"
    );
}

/// Different keys are unaffected.
#[test]
fn distinct_transformations_append_normally() {
    let path = scratch("idem_distinct").join("ledger.log");
    let mut store = LedgerStore::open(&path).expect("open");
    for i in 0..5u64 {
        let outcome = store
            .append(tid(i), cid(1_000 + i), Timestamp(i as i64))
            .expect("append");
        assert!(matches!(outcome, AppendOutcome::Appended(_)));
        assert_eq!(outcome.entry().index, i);
    }
    assert_eq!(store.log().len(), 5);
}

/// The key index is rebuilt from the file, so idempotence outlives the process.
///
/// This is the arm that makes ASM-43-1 worth anything: a retry after a crash is exactly the case
/// exactly-once has to survive, and an index held only in memory would let the retry through.
#[test]
fn idempotence_survives_a_reopen() {
    let path = scratch("idem_reopen").join("ledger.log");
    {
        let mut store = LedgerStore::open(&path).expect("open");
        for i in 0..4u64 {
            store
                .append(tid(i), cid(1_000 + i), Timestamp(i as i64))
                .expect("append");
        }
    }

    let mut reopened = LedgerStore::open(&path).expect("reopen");
    assert_eq!(reopened.recovery().records, 4);

    let repeat = reopened
        .append(tid(2), cid(1_002), Timestamp(500))
        .expect("the repeat");
    assert!(
        matches!(repeat, AppendOutcome::AlreadyPresent(_)),
        "the key index was not rebuilt from the file: {repeat:?}"
    );
    assert_eq!(repeat.entry().index, 2);

    let clash = reopened.append(tid(2), cid(9_999), Timestamp(500));
    assert!(matches!(clash, Err(Error::Conflict { .. })), "{clash:?}");
    assert_eq!(reopened.log().len(), 4);
}

/// A no-op is not a write, and the file after a run of repeats is the file before them.
#[test]
fn a_run_of_repeats_leaves_the_bytes_alone() {
    let path = scratch("idem_bytes").join("ledger.log");
    let mut store = LedgerStore::open(&path).expect("open");
    for i in 0..3u64 {
        store
            .append(tid(i), cid(1_000 + i), Timestamp(i as i64))
            .expect("append");
    }
    let before = fs::read(&path).expect("read");

    for _ in 0..10 {
        for i in 0..3u64 {
            store
                .append(tid(i), cid(1_000 + i), Timestamp(1_000))
                .expect("repeat");
        }
    }
    assert_eq!(fs::read(&path).expect("read"), before);
    assert_eq!(store.log().len(), 3);
}
