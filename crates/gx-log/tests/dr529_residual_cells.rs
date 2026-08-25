// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `req/529` residual cells (`req/534` §2's honest disclosure: "log×missing-field /
//! log×order-swap / canon×missing-field / cli×order-swap ... not individually fired, mechanism
//! same as an already-measured cell, census-level inference not live fire"). This file fires the
//! **log** layer's two cells live, against a real `LedgerStore`, following `ac_069.rs`'s own raw-
//! file-construction method (the precedent `req/534` §2 names).
//!
//! Cell identity, from `req/529` §2's grid: **log × missing-field = ✘**,
//! **log × order-swap = △** (the grid's own honest flag, translated: "map key order only; no
//! entry-reordering injection; the root recomputation is *supposed* to catch it, unmeasured").
//! Both fired live below.

mod support;

use gx_core::{Cid, Timestamp};
use gx_log::{LedgerEntry, LedgerStore};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use support::{cid, scratch, tid};

/// A fully-populated, hand-built `LedgerEntry` claiming `index`, with a genuinely correct
/// `leaf_cid` (`gx_log::tile::leaf_hash` over its own `leaf()` projection) -- so that, for the
/// order-swap test below, a record placed in the CORRECT position is accepted (the control), and
/// only a record in the WRONG position is refused, isolating order as the one variable.
fn good_ledger_entry(index: u64) -> LedgerEntry {
    let receipt_digest = cid(1_000 + index);
    let transformation = tid(index);
    let leaf = gx_log::LedgerLeaf {
        index,
        receipt_digest,
        transformation,
    };
    let leaf_cid = gx_log::tile::leaf_hash(&leaf).expect("a leaf always has a canonical form");
    LedgerEntry {
        appended_at: Timestamp(index as i64),
        index,
        leaf_cid,
        receipt_digest,
        transformation,
    }
}

/// `crates/gx-log/src/tile.rs`'s on-disk framing: a big-endian `u32` length header, then the
/// payload -- `ac_069.rs`'s own raw-write pattern, reused rather than re-derived.
fn write_raw_record(path: &std::path::Path, payload: &[u8]) {
    let mut raw = OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open the ledger file directly");
    raw.write_all(&(payload.len() as u32).to_be_bytes())
        .expect("length header");
    raw.write_all(payload).expect("payload");
    raw.sync_all().expect("fsync the injected record");
}

/// `LedgerEntry` minus its last field (`transformation`) -- a structurally valid canonical CBOR
/// map, four keys instead of five. Not garbage bytes (that is the torn-tail case `ac_069.rs`
/// already measures): this is what a `LedgerEntry` written by a build one field older would
/// produce, or what a corrupted-but-still-CBOR record looks like.
#[derive(Serialize)]
struct MissingTransformation {
    appended_at: Timestamp,
    index: u64,
    leaf_cid: Cid,
    receipt_digest: Cid,
}

/// Fill a fresh store with `n` entries, same as `ac_069.rs::fill`.
fn fill(path: &std::path::Path, n: u64) {
    let mut store = LedgerStore::open(path).expect("open the ledger");
    for i in 0..n {
        store
            .append(tid(i), cid(1_000 + i), Timestamp(i as i64))
            .expect("append");
    }
}

// ---------------------------------------------------------------------------
// Cell: log x missing-field
// ---------------------------------------------------------------------------

/// **Fired live.** A record whose payload is a schema-incomplete (but structurally valid) CBOR
/// map is appended raw after 3 good entries, then the store is reopened.
///
/// **Finding (H/M/L, per `req/529` §4-2's AC)**: `store.rs:1104`'s replay loop
/// (`let Ok(recorded) = cbor::decode::<LedgerEntry>(&payload) else { break; };`) treats a
/// missing-field decode failure **identically** to a torn/truncated record: the good prefix is
/// kept, the bad record and everything after it is dropped, and the file is truncated back to the
/// good prefix on next open (same as `ac_069_a_torn_tail_is_discarded_and_the_ledger_still_opens`).
/// **No crash, no silent-success, no false record accepted** -- the safe direction is taken. **L,
/// not H or M**: the one real gap is diagnostic, not correctness -- a missing-field corruption and
/// an accidental truncation are indistinguishable to a caller reading `Recovery` (both report as
/// `torn_tail_bytes`), so an operator cannot tell "the file was cut short" from "a record was
/// edited to drop a field" without a byte-level audit. Recorded as L (informational), matching
/// `req/534`'s own house style for this class of finding.
#[test]
fn dr529_log_missing_field_is_treated_as_a_torn_tail_not_silently_accepted() {
    let path = scratch("dr529_log_missing_field").join("ledger.log");
    fill(&path, 3);
    let good_len = fs::metadata(&path).expect("metadata").len();

    let partial = MissingTransformation {
        appended_at: Timestamp(999),
        index: 3,
        leaf_cid: cid(9_999),
        receipt_digest: cid(1_003),
    };
    let payload = gx_canon::cbor::encode(&partial).expect("a partial record still has a canonical form (4 of 5 keys)");
    write_raw_record(&path, &payload);

    let reopened = LedgerStore::open(&path).expect("reopen over a schema-incomplete record");
    println!(
        "DR529_LOG_MISSING_FIELD records={} torn_tail_bytes={}",
        reopened.recovery().records,
        reopened.recovery().torn_tail_bytes
    );
    assert_eq!(
        reopened.recovery().records,
        3,
        "the good prefix (3 entries) must survive; the schema-incomplete record must not count"
    );
    assert!(
        reopened.recovery().torn_tail_bytes > 0,
        "the schema-incomplete record must be reported as a torn tail, not silently absorbed \
         (H-class failure if this were 0 with the record actually dropped: silent data loss with \
         no signal)"
    );
    assert_eq!(
        fs::metadata(&path).expect("metadata").len(),
        good_len,
        "the bad record is removed from the file on open, same as a genuine torn tail -- not \
         left in place to be silently re-read as if valid on a future open"
    );
}

// ---------------------------------------------------------------------------
// Cell: log x order-swap
// ---------------------------------------------------------------------------

/// **Fired live** (`req/529` §2's own honest flag -- "the root recomputation is supposed to catch
/// it, unmeasured" -- turned into a measurement). Two adjacent entries are written to the raw file
/// in swapped order (entry 1's
/// bytes where entry 0's belong, and vice versa), then the store is reopened.
///
/// **Finding**: `store.rs:1103`'s `if recorded.index != tree.len() { break; }` catches the swap at
/// the FIRST swapped record (index 1's bytes decode fine but `tree.len()` is still 0, so the
/// check refuses it) -- the entire swapped pair and everything after it is dropped, keeping only
/// the genuinely-in-order prefix. **L, not H**: fail-closed, no silent wrong root, matching the
/// same safe direction the missing-field cell takes. The mechanism this measures
/// (index-monotonicity, not a content hash) is a **different** guard than merkle-root
/// recomputation -- `req/529` §2's "the root recomputation catches it" framing was imprecise about
/// *which* mechanism actually fires; this test corrects that by naming the real one.
#[test]
fn dr529_log_order_swap_is_caught_by_index_monotonicity_not_silently_accepted() {
    // The swap: two well-formed, fully-populated records, index=1 and index=2, hand-built and
    // written to a fresh 1-entry (index=0) prefix in SWAPPED order -- the record claiming index=2
    // is appended BEFORE the record claiming index=1, so a reader walking the file in disk order
    // sees {index 0 (good), index 2 (out of order), index 1 (would-be-good, never reached)}.
    let swapped_path = scratch("dr529_log_order_swap").join("ledger.log");
    fill(&swapped_path, 1); // one genuine good entry, index=0
    let payload_2 = gx_canon::cbor::encode(&good_ledger_entry(2)).expect("canonical");
    let payload_1 = gx_canon::cbor::encode(&good_ledger_entry(1)).expect("canonical");
    write_raw_record(&swapped_path, &payload_2); // written first: WRONG position
    write_raw_record(&swapped_path, &payload_1); // written second: would have been right

    let reopened = LedgerStore::open(&swapped_path).expect("reopen over a swapped pair");
    println!(
        "DR529_LOG_ORDER_SWAP records={} torn_tail_bytes={}",
        reopened.recovery().records,
        reopened.recovery().torn_tail_bytes
    );
    assert_eq!(
        reopened.recovery().records,
        1,
        "only the genuine index=0 prefix must survive -- the swapped pair (2 then 1) must be \
         rejected in its entirety, including the index=1 record that would have been valid in \
         the right position"
    );
    assert!(
        reopened.recovery().torn_tail_bytes > 0,
        "the swapped pair must be reported as a torn tail, not silently dropped with no signal"
    );

    // Control: the SAME two records, written in the CORRECT order, both survive -- proving the
    // rejection above is specifically about order, not "any two raw-appended records are
    // refused".
    let control_path = scratch("dr529_log_order_swap_control").join("ledger.log");
    fill(&control_path, 1);
    write_raw_record(&control_path, &payload_1); // correct position this time
    write_raw_record(&control_path, &payload_2); // correct position this time
    let control = LedgerStore::open(&control_path).expect("reopen over a genuinely ordered pair");
    assert_eq!(
        control.recovery().records,
        3,
        "the control (index 0,1,2 in the correct order) must accept all three -- proving the \
         swap test above is measuring order, not merely refusing raw-appended bytes"
    );
    assert_eq!(
        control.recovery().torn_tail_bytes,
        0,
        "the correctly-ordered control must show no torn tail at all"
    );
}
