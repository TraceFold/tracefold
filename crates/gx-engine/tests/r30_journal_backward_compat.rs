// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! R30 / `req/372` M-02 acceptance — **producer half, on the repaired build**.
//!
//! The twenty-ninth audit measured a pre-R29 binary meeting a post-R29 journal: it returned `Ok`,
//! read 1 record of 3, called the other 270 bytes a torn tail, quarantined them, cut a 415-byte
//! file to 145, and after one ordinary append the journal *looked healthy*. R30's repair versions
//! the record **vocabulary** (`JOURNAL_MAGIC_V2` / `JournalFormat::ChainedV2`), and the ruling's
//! acceptance condition is that the old binary now meets the file with a **refusal** rather than
//! with a silent truncation.
//!
//! This half writes the file the consumer half will be handed, in the same three-record shape the
//! audit used — the unknown word (`Rollback::Diverged`) in the **middle** record — so that "read
//! everything", "refused the file" and "stopped short" stay three distinguishable outcomes. It
//! asserts *this* build reads all three back with `torn_tail_bytes = 0`, which is the control:
//! anything the consumer reports is then a fact about the consumer and not about a damaged file.
//!
//! The bytes are copied to `R30_JOURNAL_OUT` when that variable is set, so the consumer reads a
//! file this half produced rather than a re-derivation of it.
//!
//! Helpers are declared locally rather than pulled from `tests/support`: this worktree is shared
//! with other lanes that have that module open, and an acceptance measurement should not fail for
//! a reason that belongs to somebody else's edit.

use std::path::PathBuf;

use gx_core::{AbortReason, Cid, Timestamp, TransformationId};
use gx_engine::{EngineJournal, EngineJournalRecord, Rollback};

/// The three roll-back words a v0.1 abort can carry, in the order they are appended.
///
/// The middle one is the word R29 minted and a pre-R29 binary has never been taught.
const APPENDED: [(&str, Rollback); 3] = [
    ("Succeeded", Rollback::Succeeded),
    ("Diverged", Rollback::Diverged),
    ("Failed", Rollback::Failed),
];

/// A distinguishable transformation id, in the range `tests/support` reserves for them.
fn tid(seed: u64) -> TransformationId {
    let mut raw = [0u8; 32];
    raw[..8].copy_from_slice(&(9_000_000_u64 + seed).to_be_bytes());
    TransformationId(Cid(raw))
}

/// An empty directory under the cargo target directory, cleared on entry rather than on exit.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// Bytes as ASCII, with anything unprintable shown as `.` so one line can carry the marker.
fn ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| {
            if (0x20..0x7f).contains(b) {
                *b as char
            } else {
                '.'
            }
        })
        .collect()
}

/// Write the journal, and assert **this** build reads back everything it wrote.
#[test]
fn a_journal_carrying_the_new_word_is_written_and_read_whole_by_this_build() {
    let dir = scratch("r30_journal_backward_compat");
    let path = dir.join("journal.bin");

    {
        let mut journal = EngineJournal::open(&path).expect("a fresh journal opens");
        for (index, (name, rollback)) in APPENDED.iter().enumerate() {
            let seq = journal
                .append(EngineJournalRecord::Aborted {
                    transformation: tid(index as u64 + 1),
                    reason: AbortReason::ApplyFailed,
                    rollback: Some(*rollback),
                    at: Timestamp(index as i64 + 1),
                })
                .expect("the repaired build appends every word of its own vocabulary");
            println!("R30_JOURNAL_NEW_WROTE seq={seq} rollback={name}");
        }
    }

    let raw = std::fs::read(&path).expect("the journal is on disk");
    let first_eight = ascii(&raw[..raw.len().min(8)]);
    let hex: String = raw
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("");

    let reopened = EngineJournal::open(&path).expect("the journal reopens");
    let bytes = raw.len();
    let records = reopened.records().len();
    let torn = reopened.recovery().torn_tail_bytes;
    let format = reopened.format().kind();

    println!("R30_JOURNAL_NEW_FIRST8 ascii={first_eight} hex={hex}");
    println!(
        "R30_JOURNAL_NEW_READS records={records} torn_tail_bytes={torn} file_bytes={bytes} format={format} path={}",
        path.display()
    );

    // The control. If this build could not read its own journal whole, the consumer half would be
    // measuring a damaged file rather than an older decoder.
    assert_eq!(
        records,
        APPENDED.len(),
        "this build did not read back every record it wrote"
    );
    assert_eq!(torn, 0, "the journal this half produces is intact");

    // Hand the bytes to the consumer half, byte for byte.
    if let Ok(out) = std::env::var("R30_JOURNAL_OUT") {
        let out = PathBuf::from(out);
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::copy(&path, &out).expect("the produced journal is copied out");
        println!(
            "R30_JOURNAL_NEW_OUT_WRITTEN path={} bytes={bytes}",
            out.display()
        );
    } else {
        println!("R30_JOURNAL_NEW_OUT_WRITTEN path=<unset> bytes={bytes}");
    }
}
