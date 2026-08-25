// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R31 / `req/378` H-02** — the framing the engine believes it is writing is the framing on
//! the disk, on **every** road into the writer's door.
//!
//! # The defect this suite is the invariant for
//!
//! `EngineJournal::open_declared_creating` reads the file into a `bytes` buffer, and when that
//! buffer is empty it stamps a marker chosen from the caller's **declaration**. Before R31 it did
//! not put the stamped marker back into `bytes`, so `replay(&bytes)` kept answering from the empty
//! buffer — and `replay` on an empty buffer is fixed at [`JournalFormat::ChainedV2`]. The
//! thirtieth adversarial audit measured the pair that follows for a project declaring `chained`
//! (every project made between R6 and R29):
//!
//! ```text
//! A30_EMPTY_STAMP declared=Chained marker_on_disk="GXJRNL01" in_memory_format=ChainedV2 agree=false
//! A30_EMPTY_GUARD diverged_append=ACCEPTED bytes=152
//! A30_EMPTY_REOPEN open=Ok records=0 chain_intact=false format=Chained
//! ```
//!
//! Three things follow from `agree=false`, and each is asserted below rather than argued:
//!
//! 1. R30's vocabulary guard compares `rank(record) > rank(self.format)`. With `self.format`
//!    wrongly at `ChainedV2` the comparison is `2 > 2`, the guard does **not** fire, and a
//!    `Rollback::Diverged` record lands in a file whose header says v1 — the exact state R30 was
//!    built to prevent.
//! 2. That record's link is minted from the **v2** genesis under a **v1** header, so the next open
//!    walks from the v1 genesis and the chain breaks at byte 8. One record written, zero records
//!    readable.
//! 3. DR-43-9 forbids truncating at a chain break, so there is no road back.
//!
//! # What is asserted, and why it is one predicate rather than three
//!
//! The repair is not "handle the empty case"; it is that the in-memory format has exactly **one**
//! source, the bytes on the disk. So the predicate asserted on every road here is the same
//! sentence — *the marker under the first eight bytes is the marker of the format this journal
//! reports* — and the roads are enumerated so that a later change which reintroduces a second
//! source fails here whichever road it takes.
//!
//! Roads covered: an existing zero-byte file (declared `chained`, `chained-v2`, and undeclared),
//! a file this call creates (the same three), an existing v1 file, an existing v2 file, and a file
//! with no marker at all (`Legacy`, whose invariant is the negative one — it reports the format
//! whose marker is absent).
//!
//! # What this file does not measure
//!
//! Reachability from the CLI. That is `crates/gx-cli/tests/r31_e2e_empty_journal_submit.rs`, which
//! drives the shipped binary end to end — the half the thirtieth audit named as R31's first job
//! (`req/378` §4 says of its own H-02 arm that `gx submit` end to end was not driven).

mod support;

use gx_core::{AbortReason, Timestamp};
use gx_engine::{EngineJournal, EngineJournalRecord, JournalCreation, JournalFormat, Rollback};
use support::{scratch, tid};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// The first eight bytes on the disk, as a string, or `""` for a file shorter than that.
fn marker_on_disk(path: &std::path::Path) -> String {
    let bytes = std::fs::read(path).expect("the journal is readable");
    String::from_utf8_lossy(&bytes[..bytes.len().min(8)]).to_string()
}

/// 🔴 The one predicate. Asserted identically on every road, so that the roads are the only thing
/// that varies.
///
/// `JournalFormat::marker` answers `None` for `Legacy`, which is the framing with no marker at
/// all; the invariant for that arm is that the disk carries **neither** known marker.
fn the_format_is_the_marker(road: &str, path: &std::path::Path, format: JournalFormat) {
    let on_disk = marker_on_disk(path);
    let expected = format
        .marker()
        .map(|m| String::from_utf8_lossy(m).to_string());
    println!(
        "R31_FORMAT_FROM_DISK road={road} marker_on_disk={on_disk:?} in_memory_format={format:?} \
         marker_of_that_format={expected:?} agree={}",
        expected
            .as_deref()
            .map_or(on_disk != "GXJRNL01" && on_disk != "GXJRNL02", |m| on_disk
                == m)
    );
    match expected {
        Some(marker) => assert_eq!(
            on_disk, marker,
            "🔴 `req/378` H-02 ({road}): the journal reports {format:?}, whose marker is \
             {marker:?}, and the eight bytes on the disk are {on_disk:?}. The framing a guard \
             compares against must be the framing the file actually has"
        ),
        None => assert!(
            on_disk != "GXJRNL01" && on_disk != "GXJRNL02",
            "🔴 ({road}): the journal reports {format:?}, which is the framing with no marker, \
             and the disk carries {on_disk:?}"
        ),
    }
}

/// Road 1 — an existing file of zero bytes, opened against each of the three declarations.
///
/// This is the audit's exact bed. `JournalCreation::Refused` does not refuse it, because the file
/// is there; what is decided here is only the marker.
#[test]
fn r31_a_zero_byte_journal_reports_the_framing_it_was_stamped_with() {
    for (label, declared) in [
        ("empty/declared=chained", Some(JournalFormat::Chained)),
        ("empty/declared=chained-v2", Some(JournalFormat::ChainedV2)),
        ("empty/declared=none", None),
    ] {
        let dir = scratch(&format!("r31_empty_{}", label.replace(['/', '='], "_")));
        let path = dir.join("journal");
        std::fs::write(&path, b"").expect("a zero-byte journal exists");
        assert_eq!(
            std::fs::metadata(&path).expect("stat").len(),
            0,
            "the bed is a file of zero bytes"
        );

        let journal =
            EngineJournal::open_declared_creating(&path, declared, JournalCreation::Refused)
                .expect("the file exists, so `Refused` does not refuse it");
        the_format_is_the_marker(label, &path, journal.format());
    }
}

/// Road 2 — a file this call brings into existence, against each of the three declarations.
///
/// The audit's `A30_EMPTY_CONTROL` established that the divergence is **not** specific to a
/// zero-byte file: `JournalCreation::Permitted` over a path that does not exist takes the same
/// branch. It is a road of its own here, with the assertion the control did not carry.
#[test]
fn r31_a_created_journal_reports_the_framing_it_was_stamped_with() {
    for (label, declared) in [
        ("created/declared=chained", Some(JournalFormat::Chained)),
        (
            "created/declared=chained-v2",
            Some(JournalFormat::ChainedV2),
        ),
        ("created/declared=none", None),
    ] {
        let dir = scratch(&format!("r31_created_{}", label.replace(['/', '='], "_")));
        let path = dir.join("journal");
        assert!(!path.exists(), "the bed is a path with no file");

        let journal =
            EngineJournal::open_declared_creating(&path, declared, JournalCreation::Permitted)
                .expect("the journal is created");
        the_format_is_the_marker(label, &path, journal.format());
    }
}

/// Road 3 — files that already carry a marker, and one that carries none.
///
/// The regression control for the repair: a journal that arrived with its own framing keeps it,
/// whatever the declaration says. If the repair had been written as "derive the format from the
/// declaration" instead of "derive it from the disk", these arms are where that shows.
#[test]
fn r31_an_existing_journal_keeps_the_framing_its_bytes_carry() {
    for (label, seed, declared) in [
        (
            "existing-v1/declared=chained-v2",
            &b"GXJRNL01"[..],
            Some(JournalFormat::ChainedV2),
        ),
        (
            "existing-v2/declared=chained",
            &b"GXJRNL02"[..],
            Some(JournalFormat::Chained),
        ),
        ("existing-v1/declared=none", &b"GXJRNL01"[..], None),
        ("existing-v2/declared=none", &b"GXJRNL02"[..], None),
    ] {
        let dir = scratch(&format!("r31_existing_{}", label.replace(['/', '='], "_")));
        let path = dir.join("journal");
        std::fs::write(&path, seed).expect("a journal with these bytes exists");

        let journal =
            EngineJournal::open_declared_creating(&path, declared, JournalCreation::Refused)
                .expect("the file exists");
        the_format_is_the_marker(label, &path, journal.format());
        assert_eq!(
            marker_on_disk(&path).as_bytes(),
            &seed[..seed.len().min(8)],
            "🔴 an existing journal's marker is not rewritten by opening it ({label})"
        );
    }
}

/// The unmarked arm, kept apart from the four above because its bytes are **allowed to move**.
///
/// 🔴 This suite's own first draft asserted byte-preservation here too, and was red for a reason
/// that had nothing to do with `req/378`: four bytes carrying no marker are a legacy frame header
/// with no record behind it, which is a torn tail, and DR-43-7 says the writer's door removes one.
/// The file came back at zero bytes and the assertion caught the instrument rather than the
/// product. The invariant that does belong here is the negative half of the predicate — a journal
/// reporting `Legacy` is a journal whose disk carries neither known marker — and the truncation is
/// asserted as the documented behaviour it is.
#[test]
fn r31_an_unmarked_journal_reports_legacy_and_its_torn_tail_is_removed() {
    let dir = scratch("r31_existing_unmarked");
    let path = dir.join("journal");
    std::fs::write(&path, b"\x00\x00\x00\x00").expect("four bytes carrying no marker");

    let journal = EngineJournal::open_declared_creating(&path, None, JournalCreation::Refused)
        .expect("the file exists");
    println!(
        "R31_UNMARKED format={:?} bytes_after={} torn_tail={}",
        journal.format(),
        std::fs::metadata(&path).expect("stat").len(),
        journal.recovery().torn_tail_bytes,
    );
    the_format_is_the_marker("existing-unmarked/declared=none", &path, journal.format());
    assert_eq!(
        journal.format(),
        JournalFormat::Legacy,
        "bytes carrying no marker are the framing with no marker"
    );
}

/// 🔴 Road 1's consequence — R30's vocabulary guard fires on the journal it is pointed at.
///
/// The audit's `A30_EMPTY_GUARD` measured `diverged_append=ACCEPTED` on a v1-framed file. This is
/// the arm that has to reverse: on a journal framed v1, a record carrying a v2 word is refused,
/// and the file does not grow.
#[test]
fn r31_the_vocabulary_guard_fires_on_a_v1_journal_that_was_stamped_from_a_v1_declaration() {
    let dir = scratch("r31_guard_v1");
    let path = dir.join("journal");
    std::fs::write(&path, b"").expect("a zero-byte journal exists");

    let mut journal = EngineJournal::open_declared_creating(
        &path,
        Some(JournalFormat::Chained),
        JournalCreation::Refused,
    )
    .expect("the file exists");
    the_format_is_the_marker("guard-bed", &path, journal.format());

    let before = std::fs::metadata(&path).expect("stat").len();
    let appended = journal.append(EngineJournalRecord::Aborted {
        transformation: tid(1),
        reason: AbortReason::ApplyFailed,
        rollback: Some(Rollback::Diverged),
        at: AT,
    });
    let after = std::fs::metadata(&path).expect("stat").len();
    println!(
        "R31_GUARD_V1 diverged_append={} bytes_before={before} bytes_after={after}",
        match appended.as_ref() {
            Ok(_) => "ACCEPTED".to_string(),
            Err(e) => format!("refused({e})"),
        }
    );
    assert!(
        appended.is_err(),
        "🔴 `req/378` H-02: a `Diverged` record carries a word a v1 framing does not cover, and \
         this journal is framed v1. Before R31 the guard compared 2 > 2 against a format the file \
         did not have, and this append was ACCEPTED"
    );
    assert_eq!(before, after, "🔴 a refused record does not grow the file");
}

/// 🔴 Road 1's other consequence — a record that *is* appendable comes back when the file is
/// reopened.
///
/// The audit's `A30_EMPTY_REOPEN` measured `records=0 chain_intact=false` after one record had
/// been written: the link was minted over the v2 genesis and the reopen walked from the v1
/// genesis. This is the arm that has to reverse, and it is the one that matters most, because
/// DR-43-9 forbids truncating at a chain break — a journal that reaches that state has no road
/// back.
#[test]
fn r31_a_record_written_to_a_stamped_journal_is_readable_after_a_reopen() {
    let dir = scratch("r31_reopen_v1");
    let path = dir.join("journal");
    std::fs::write(&path, b"").expect("a zero-byte journal exists");

    let mut journal = EngineJournal::open_declared_creating(
        &path,
        Some(JournalFormat::Chained),
        JournalCreation::Refused,
    )
    .expect("the file exists");
    // A record every framing covers, so that this arm is about the chain and not the vocabulary.
    journal
        .append(EngineJournalRecord::Aborted {
            transformation: tid(2),
            reason: AbortReason::ApplyFailed,
            rollback: None,
            at: AT,
        })
        .expect("a record every binary that ever wrote this format can read");
    drop(journal);

    let reopened = EngineJournal::open_declared(&path, Some(JournalFormat::Chained))
        .expect("the journal reopens");
    println!(
        "R31_REOPEN records={} chain_intact={} format={:?} marker={:?}",
        reopened.records().len(),
        reopened.chain_intact(),
        reopened.format(),
        marker_on_disk(&path)
    );
    the_format_is_the_marker("reopen", &path, reopened.format());
    assert_eq!(
        reopened.records().len(),
        1,
        "🔴 `req/378` H-02: one record was written. Before R31 this read back as 0 — the link was \
         minted over the v2 genesis under a v1 header"
    );
    assert!(
        reopened.chain_intact(),
        "🔴 `req/378` H-02: and the chain broke at byte 8, which DR-43-9 forbids repairing by \
         truncation, so the journal could not be appended to again"
    );
}
