// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **FR-M04** — the file the aggregate verdict checkpoints are appended to (M7 hand 6, option A). (sem: SEM-gx-log-173)
//!
//! # Why a second file and not the ledger's own
//!
//! FR-M04 asks for "append the count-tally's **commitment** to the ledger" (sem: SEM-gx-log-174), and the design material's option A
//! (`FRM04_DESIGN_MATERIAL_2026-08-11.md` §3) leaves the writer's shape open with two candidates:
//! a separate index, or a second kind of record inside the ledger's own physical log. The second
//! is not available: `store.rs`'s frame is `length:u32be || canonical_dagcbor(LedgerEntry)` and
//! replay decodes **every** record as a `LedgerEntry`, so a second record type in that file is a
//! file the current reader refuses at the first one it meets. Making the reader tolerant is option B (sem: SEM-gx-log-175)
//! by another road — `LedgerEntry` becomes a sum type and `LedgerStore::append` changes shape —
//! and `req/98` §3-3 already chose option A precisely to keep adapter work and canonical-structure work in (sem: SEM-gx-log-176)
//! different milestones.
//!
//! So the artefact is its own append-only file, beside the ledger, with the **same** framing and
//! the same recovery discipline, written by the same crate. What that buys and what it costs is
//! written in `store.rs`'s own documentation; what this file measures is that it holds.
//!
//! # RED-first
//!
//! Committed before `VerdictCheckpointStore` exists (compile-stage RED, discipline 47). (sem: SEM-gx-log-177)

mod support;

use gx_core::{Cid, DsseSignature, KeyId, Timestamp, VerdictCheckpoint, VerdictTally};
use gx_log::store::VerdictCheckpointStore;
use support::scratch;

const ORIGIN: &str = "glovrex-verdicts/v1";
const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// A checkpoint with distinguishable numbers. Not signed by a real key: this file is about the
/// **file**, and a signature is bytes to it.
fn checkpoint(window_start: u64, deny: u64) -> VerdictCheckpoint {
    VerdictCheckpoint {
        origin: ORIGIN.to_string(),
        tally: VerdictTally {
            deny,
            admit: 1,
            escalate: 0,
            unverdicted: 0,
        },
        window_start,
        window_end: window_start + deny + 1,
        ledger_root_hash: Some(Cid([9u8; 32])),
        ledger_tree_size: window_start + 1,
        timestamp: AT,
        signature: DsseSignature {
            keyid: KeyId::from("fixture-key"),
            sig: vec![7u8; 64],
        },
    }
}

/// Append, close, reopen: what the file holds is what was put in it, in order.
#[test]
fn what_is_appended_is_what_replays() {
    let dir = scratch("verdict_store_roundtrip");
    let path = dir.join("journal.bin.verdicts");
    let written: Vec<VerdictCheckpoint> = (0..4).map(|n| checkpoint(n * 3, 2)).collect();

    {
        let mut store = VerdictCheckpointStore::open(&path).expect("a fresh file opens");
        assert_eq!(store.recovery().records, 0, "a new file replays nothing");
        for (n, cp) in written.iter().enumerate() {
            let seq = store.append(cp.clone()).expect("append");
            assert_eq!(seq, n as u64, "the sequence number is the position");
        }
        assert_eq!(store.checkpoints().len(), 4);
    }

    let reopened = VerdictCheckpointStore::open(&path).expect("the file replays");
    println!(
        "VCSTORE_ROUNDTRIP written={} replayed={} torn={} path={}",
        written.len(),
        reopened.checkpoints().len(),
        reopened.recovery().torn_tail_bytes,
        reopened.path().display()
    );
    assert_eq!(reopened.checkpoints(), written.as_slice());
    assert_eq!(reopened.recovery().records, 4);
    assert_eq!(reopened.recovery().torn_tail_bytes, 0);
}

/// A half-written record at the tail is removed and **reported**, never repaired.
///
/// The same rule `LedgerStore` holds (its `Recovery` doc: "Reported rather than logged"). A store
/// that silently truncated would make "the chain ends here" indistinguishable from "the last write
/// was cut in half" (sem: SEM-gx-log-178), and the whole of AC-VC-5 is about telling those two apart.
#[test]
fn a_torn_tail_is_removed_and_reported() {
    let dir = scratch("verdict_store_torn");
    let path = dir.join("journal.bin.verdicts");
    {
        let mut store = VerdictCheckpointStore::open(&path).expect("open");
        store.append(checkpoint(0, 2)).expect("append");
        store.append(checkpoint(3, 2)).expect("append");
    }
    let whole = std::fs::metadata(&path).expect("the file is there").len();
    let bytes = std::fs::read(&path).expect("read");
    std::fs::write(&path, &bytes[..bytes.len() - 5]).expect("cut the last record in half");

    let reopened = VerdictCheckpointStore::open(&path).expect("a torn file still opens");
    println!(
        "VCSTORE_TORN whole={whole} replayed={} torn_tail_bytes={}",
        reopened.checkpoints().len(),
        reopened.recovery().torn_tail_bytes
    );
    assert_eq!(
        reopened.checkpoints().len(),
        1,
        "the prefix that replayed is kept and the cut record is not guessed at"
    );
    assert!(
        reopened.recovery().torn_tail_bytes > 0,
        "and the fact that something was removed is a value the caller can read"
    );
    let after = std::fs::metadata(&path).expect("still there").len();
    assert!(
        after < whole - 5,
        "the file is truncated to the last **good** record ({after} of {whole}), not merely to the \
         bytes that survived the cut, so the next append lands where the sequence actually reached"
    );
    let again = VerdictCheckpointStore::open(&path).expect("open");
    assert_eq!(
        again.recovery().torn_tail_bytes,
        0,
        "and a second open of the repaired file finds nothing left to remove"
    );
}

/// An append after a recovery continues the sequence rather than restarting it.
#[test]
fn an_append_after_a_recovery_continues_the_sequence() {
    let dir = scratch("verdict_store_continue");
    let path = dir.join("journal.bin.verdicts");
    {
        let mut store = VerdictCheckpointStore::open(&path).expect("open");
        store.append(checkpoint(0, 2)).expect("append");
    }
    let mut reopened = VerdictCheckpointStore::open(&path).expect("open");
    let seq = reopened.append(checkpoint(3, 1)).expect("append");
    println!(
        "VCSTORE_CONTINUE seq={seq} held={}",
        reopened.checkpoints().len()
    );
    assert_eq!(seq, 1);
    assert_eq!(reopened.checkpoints().len(), 2);
}

/// The store offers no writer but `append` — the same shape AC-021 asserts of the ledger.
///
/// Read off the source rather than off the API, because a method that mutated in place would still
/// compile against every caller in the workspace. 42's transparency argument is that an audit
/// artefact is append-only; a second writer is that argument gone.
#[test]
fn the_store_offers_no_writer_but_append() {
    let source = std::fs::read_to_string(support::source("store.rs")).expect("read store.rs");
    let writers: Vec<&str> = source
        .lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with("pub fn ") && l.contains("&mut self"))
        .collect();
    println!("VCSTORE_WRITERS {writers:?}");
    assert!(
        writers.iter().all(|l| l.contains("pub fn append")),
        "gx-log's mutable-receiver surface is `append` and nothing else: {writers:?}"
    );
}
