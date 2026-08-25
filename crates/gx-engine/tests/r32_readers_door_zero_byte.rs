// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! R32 / `req/392` M-01 — the reader's door has one source for the framing, and it is the disk.
//!
//! # The predicate
//!
//! R31 closed the writer's door on the sentence *"the marker under the first eight bytes is the
//! marker of the format this journal reports"* and asserted it on six roads, **all of them**
//! through `EngineJournal::open_declared_creating`. The thirty-first audit drove the same sentence
//! through `EngineJournal::open_read_only_declared` and measured `agree=false` on `3/3`
//! declarations over a journal of zero bytes, with `9/9` marked beds and `21/21` partial markers
//! agreeing on the same door — so the reader's door was not broken; **one point on it** was.
//!
//! The second source was not in the door. It was in `replay`, whose `bytes.is_empty()` arm
//! answered `ChainedV2` — "the format a writer is about to give it" — inside the function whose
//! contract is that the framing is sniffed from the bytes. The writer's door stopped reaching that
//! arm at R31 (it extends its buffer with the marker it just wrote); the reader cannot write, so
//! it kept reaching it.
//!
//! # What is asserted here
//!
//! The audit's own predicate, spelled the way `a31_single_source_attack.rs` spells it: the eight
//! bytes on the disk are the marker of the format the journal reports, with the empty string
//! standing for "no marker" on both sides. Plus the consequence R6 built and this bed disarmed —
//! a project that declares a chain over a file carrying no marker is `downgraded` — and the
//! symmetry the audit's §2-4 table inverted: zero bytes and four unmarked bytes are answered the
//! same way by the same door under the same declaration.
//!
//! Self-directed test of this repository's own engine. Every byte written here lives inside this
//! worktree's `CARGO_TARGET_TMPDIR`; no network is used.

mod support;

use gx_engine::{EngineJournal, JournalCreation, JournalFormat};
use support::scratch;

/// The first eight bytes on the disk as a string, or the empty string for a shorter file. The
/// audit's helper, copied so the predicate is compared the way it was measured.
fn marker_on_disk(path: &std::path::Path) -> String {
    let bytes = std::fs::read(path).expect("the journal is readable");
    String::from_utf8_lossy(&bytes[..bytes.len().min(8)]).to_string()
}

/// R31's predicate: the marker under the first eight bytes is the marker of the reported format.
fn agrees(path: &std::path::Path, format: JournalFormat) -> bool {
    let on_disk = marker_on_disk(path);
    match format.marker() {
        Some(m) => on_disk == String::from_utf8_lossy(m),
        None => on_disk != "GXJRNL01" && on_disk != "GXJRNL02",
    }
}

fn decl_name(d: Option<JournalFormat>) -> &'static str {
    match d {
        None => "none",
        Some(JournalFormat::Legacy) => "legacy",
        Some(JournalFormat::Chained) => "chained",
        Some(JournalFormat::ChainedV2) => "chained-v2",
    }
}

const DECLARATIONS: [Option<JournalFormat>; 3] = [
    None,
    Some(JournalFormat::Chained),
    Some(JournalFormat::ChainedV2),
];

/// 🔴 The bed the audit measured `agree=false` on, three times out of three.
#[test]
fn r32_the_readers_door_reports_the_framing_a_zero_byte_journal_has() {
    for declared in DECLARATIONS {
        let dir = scratch(&format!("r32_zero_{}", decl_name(declared)));
        let path = dir.join("journal");
        std::fs::write(&path, b"").expect("a zero-byte journal exists");
        assert_eq!(
            std::fs::metadata(&path).expect("stat").len(),
            0,
            "the bed is a file of zero bytes"
        );

        let j = EngineJournal::open_read_only_declared(&path, declared)
            .expect("the file exists, so the reader's door opens it");
        println!(
            "R32_RO_EMPTY declared={} format={:?} marker_on_disk={:?} downgraded={} \
             chain_intact={} agree={}",
            decl_name(declared),
            j.format(),
            marker_on_disk(&path),
            j.downgraded(),
            j.chain_intact(),
            agrees(&path, j.format()),
        );
        assert!(
            agrees(&path, j.format()),
            "🔴 `req/392` M-01: the marker under the first eight bytes is the marker of the format \
             this journal reports. The audit measured this false on this bed for all three \
             declarations, because `replay` answered `ChainedV2` about a file carrying no marker"
        );
        assert_eq!(
            j.format(),
            JournalFormat::Legacy,
            "and the framing of a file with no marker is the absence of one"
        );
        assert_eq!(
            j.format().marker(),
            None,
            "🔴 and the format it reports names no marker, so there is no chain for a report to \
             call intact: `journal_intact_basis` cannot answer `\"chain\"` about this file any more"
        );
        assert_eq!(
            std::fs::metadata(&path).expect("stat").len(),
            0,
            "and the reader wrote nothing: this door still cuts nothing and stamps nothing"
        );
    }
}

/// 🔴 R6's guard, which this bed had disarmed: `1 > 2` never fired.
#[test]
fn r32_a_declared_chain_over_a_zero_byte_journal_is_a_downgrade() {
    for declared in [Some(JournalFormat::Chained), Some(JournalFormat::ChainedV2)] {
        let dir = scratch(&format!("r32_down_{}", decl_name(declared)));
        let path = dir.join("journal");
        std::fs::write(&path, b"").expect("a zero-byte journal exists");
        let j = EngineJournal::open_read_only_declared(&path, declared).expect("opens");
        println!(
            "R32_RO_DOWNGRADE declared={} downgraded={}",
            decl_name(declared),
            j.downgraded()
        );
        assert!(
            j.downgraded(),
            "🔴 `req/229` H-02 asks whether this project declared a chained journal and got a file \
             without one. Zero bytes is such a file. The audit measured `downgraded=false` here \
             while `gx repair` printed `journal_format: \"chained-v2\"` over a `chained` \
             declaration — the exact shape R6 exists to refuse"
        );
    }

    // And the declaration that claims nothing is not accused of anything.
    let dir = scratch("r32_down_none");
    let path = dir.join("journal");
    std::fs::write(&path, b"").expect("a zero-byte journal exists");
    let j = EngineJournal::open_read_only_declared(&path, None).expect("opens");
    assert!(
        !j.downgraded(),
        "a project that declares no framing cannot be downgraded from one"
    );
}

/// 🔴 The audit's §2-4 asymmetry, inverted: *"the one with less information on it is reported as
/// the healthier of the two"*.
#[test]
fn r32_zero_bytes_and_four_unmarked_bytes_are_answered_the_same_way() {
    for declared in DECLARATIONS {
        let dir = scratch(&format!("r32_sym_{}", decl_name(declared)));
        let zero = dir.join("zero");
        let four = dir.join("four");
        std::fs::write(&zero, b"").expect("the zero-byte bed");
        std::fs::write(&four, [0u8; 4]).expect("the four-unmarked-byte bed");

        let z = EngineJournal::open_read_only_declared(&zero, declared).expect("opens");
        let f = EngineJournal::open_read_only_declared(&four, declared).expect("opens");
        println!(
            "R32_SYMMETRY declared={} zero=({:?},{}) four=({:?},{})",
            decl_name(declared),
            z.format(),
            z.downgraded(),
            f.format(),
            f.downgraded(),
        );
        assert_eq!(
            (z.format(), z.downgraded()),
            (f.format(), f.downgraded()),
            "🔴 `req/392` M-01: neither of these files carries a marker, so the door has the same \
             thing to say about both. Before this lane the four-byte file was refused \
             (`journal_intact: false`, `downgraded` firing, exit 1) and the zero-byte file — which \
             holds strictly less — was called healthy"
        );
    }
}

/// The negative control the audit ran: files that **do** carry each marker are unmoved.
#[test]
fn r32_negative_control_marked_journals_are_unchanged_on_this_door() {
    for (seed, label, expected) in [
        (&b"GXJRNL01"[..], "v1", JournalFormat::Chained),
        (&b"GXJRNL02"[..], "v2", JournalFormat::ChainedV2),
        (&b"\x00\x00\x00\x00"[..], "unmarked", JournalFormat::Legacy),
    ] {
        for declared in DECLARATIONS {
            let dir = scratch(&format!("r32_neg_{label}_{}", decl_name(declared)));
            let path = dir.join("journal");
            std::fs::write(&path, seed).expect("the bed exists");
            let j = EngineJournal::open_read_only_declared(&path, declared).expect("opens");
            println!(
                "R32_RO_CONTROL bed={label} declared={} format={:?} agree={}",
                decl_name(declared),
                j.format(),
                agrees(&path, j.format())
            );
            assert_eq!(
                j.format(),
                expected,
                "the nine roads the audit measured `agree=true` on are unmoved by this lane"
            );
            assert!(agrees(&path, j.format()), "and the predicate still holds");
        }
    }
}

/// 🔴 R31's road is unmoved: the **writer's** door still stamps the declared marker and reports it.
///
/// This is the regression that would matter most, because the repair is one arm inside the
/// function both doors call.
#[test]
fn r32_the_writers_door_still_stamps_and_reports_the_declared_marker() {
    for (declared, expected_marker, expected_format) in [
        (
            Some(JournalFormat::Chained),
            &b"GXJRNL01"[..],
            JournalFormat::Chained,
        ),
        (
            Some(JournalFormat::ChainedV2),
            &b"GXJRNL02"[..],
            JournalFormat::ChainedV2,
        ),
        (None, &b"GXJRNL02"[..], JournalFormat::ChainedV2),
    ] {
        let dir = scratch(&format!("r32_writer_{}", decl_name(declared)));
        let path = dir.join("journal");
        std::fs::write(&path, b"").expect("a zero-byte journal exists");
        let j = EngineJournal::open_declared_creating(&path, declared, JournalCreation::Refused)
            .expect("the file exists, so `Refused` does not refuse it");
        println!(
            "R32_WRITER declared={} format={:?} marker_on_disk={:?} bytes={}",
            decl_name(declared),
            j.format(),
            marker_on_disk(&path),
            std::fs::metadata(&path).expect("stat").len()
        );
        assert_eq!(j.format(), expected_format, "🔴 `req/378` H-02 is unmoved");
        assert_eq!(
            std::fs::read(&path).expect("read"),
            expected_marker,
            "and the eight bytes it stamped are the eight bytes on the disk"
        );
        assert!(
            agrees(&path, j.format()),
            "R31's predicate on the writer's door, re-measured here because this lane edited the \
             function both doors read their framing out of"
        );
    }
}
