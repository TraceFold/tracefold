// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 42 §3.12's `EscrowedInverse` and `InverseStatus`, and the one sentence pair in that table that
//! cannot both be true.
//!
//! # The contradiction, stated
//!
//! 42 §3.12 types `inverse_delta` as a `PlannedDelta` — not an option — and defines
//! `InverseStatus::Unavailable` as "when `invert()` returns None (cannot be constructed)" (sem:
//! SEM-gx-engine-734). A value cannot both
//! hold a delta and record that none could be built. This suite is where that is measured rather
//! than argued: the field table is **parsed out of the canon**, the four status values are parsed
//! out of the same row, and the constructors are exercised in both directions.
//!
//! **M5H1-3** is the raising. The implementation's answer — an `Option` field whose relationship to
//! the status is held by checked constructors — is the smallest one that keeps both sentences of the
//! table true of every constructible value, and it is an implementation choice rather than a
//! ruling: `req/spec/` is untouched (52 contract 1, sem: SEM-gx-engine-735), and an owner may
//! prefer "a row does not exist when
//! no inverse does" instead, which would delete `Unavailable`.
//!
//! # What is not here
//!
//! The transitions. §37's M5-16 adopted (a) (both instances below) (sem: SEM-gx-engine-736) puts `Consumed { by }` at
//! T-12, together with the
//! `superseded_by` index (M5-09 adopted (a)) — one place, hand 6. Nothing in this hand moves a
//! status, so
//! there is no setter to test and the absence of one is asserted below.

mod support;

use gx_core::{SubstrateKind, Timestamp};
use gx_engine::{EscrowedInverse, InverseStatus};
use gx_substrate::PlannedDelta;
use support::{read_repo, tid};

const DATA_MODEL: &str = "req/spec/40-architecture/42-data-model.md";

/// The rows of 42 §3.12's field table, as `(field, type)`.
fn canon_rows() -> Vec<(String, String)> {
    let text = read_repo(DATA_MODEL);
    let at = text.find("### 3.12").expect("42 has a §3.12");
    let section = &text[at..];
    let end = section[3..].find("\n### ").map_or(section.len(), |i| i + 3);
    section[..end]
        .lines()
        .filter(|l| l.starts_with("| `"))
        .filter_map(|l| {
            // `\|` inside a cell is an escaped pipe, not a column boundary: 42 §3.12's status row
            // writes its four values as `Available \| Consumed { .. } \| Expired \| Unavailable`,
            // and a naive split reads that one cell as four.
            let line = l.replace("\\|", "\u{1}");
            let mut cells = line.split('|').skip(1);
            let field = cells.next()?.trim().trim_matches('`').to_string();
            let ty = cells.next()?.trim().replace('\u{1}', "|");
            Some((field, ty))
        })
        .collect()
}

fn delta() -> PlannedDelta {
    PlannedDelta::new(SubstrateKind::Fs, vec![7, 8, 9]).expect("a short payload")
}

/// 42 §3.12 gives the record four fields, and the type carries four.
///
/// Parsed rather than transcribed, for `journal_vocabulary.rs`'s reason: a second copy of a table
/// is a second thing that drifts. The accessors are the field list on this side, because the fields
/// themselves are private — F-6's "one spelling per field" (sem: SEM-gx-engine-737), which is
/// also what makes the invariant
/// enforceable.
#[test]
fn the_record_carries_the_four_fields_of_42_3_12() {
    let rows = canon_rows();
    let names: Vec<&str> = rows.iter().map(|(f, _)| f.as_str()).collect();
    println!("CANON_ESCROW_FIELDS={} ({names:?})", rows.len());
    assert_eq!(
        names,
        vec![
            "transformation",
            "inverse_delta",
            "retained_until",
            "status"
        ],
        "42 §3.12's four rows"
    );

    let source = read_repo("crates/gx-engine/src/store.rs");
    for field in &names {
        assert!(
            source.contains(&format!("pub fn {field}(")),
            "no accessor for 42 §3.12's `{field}`"
        );
    }
}

/// 🔴 The canon types `inverse_delta` as non-optional, and this crate does not.
///
/// The probe that keeps **M5H1-3** honest in both directions: it fails if the canon is amended (the
/// raising has been answered and this file should follow), and it fails if the implementation
/// quietly drops the `Option` (the contradiction is back and `Unavailable` becomes unconstructible).
#[test]
fn the_optional_delta_is_a_deliberate_departure_from_42_3_12() {
    let rows = canon_rows();
    let (_, ty) = rows
        .iter()
        .find(|(f, _)| f == "inverse_delta")
        .expect("42 §3.12 has an inverse_delta row");
    println!("CANON_INVERSE_DELTA_TYPE={ty}");
    assert!(
        ty.contains("PlannedDelta") && !ty.contains("Option"),
        "42 §3.12 has stopped typing `inverse_delta` as a bare `PlannedDelta`; M5H1-3's premise moved"
    );

    let source = read_repo("crates/gx-engine/src/store.rs");
    assert!(
        source.contains("inverse_delta: Option<PlannedDelta>"),
        "the departure M5H1-3 raises is the `Option`; without it `Unavailable` cannot be built"
    );
}

/// The status values of 42 §3.12 — four through v0.2, five since v0.3-a's
/// `Pending` (two-phase escrow, `req/38` §98 ruling 1, additive in the same window as the spec)
/// (sem: SEM-gx-engine-738), six since R8's `BodyMissing` (`req/38` §173 ruling 2, `req/234` B-5,
/// additive in the same window as 43 §7.9 (b)'s new row). The probe reads the canon cell rather
/// than a list of its own, so the two cannot drift: a seventh value has to be written into 42
/// §3.12 before this file will compile past it.
#[test]
fn the_status_has_the_four_values_42_3_12_lists() {
    let rows = canon_rows();
    let (_, ty) = rows
        .iter()
        .find(|(f, _)| f == "status")
        .expect("42 §3.12 has a status row");
    println!("CANON_STATUS_CELL={ty}");
    for value in InverseStatus::ALL_KINDS {
        assert!(
            ty.contains(value),
            "42 §3.12's status cell does not mention `{value}`"
        );
    }
    // 🔴 **R8 / `req/234` B-5** — six since 43 §7.9 (b) gained a row for
    // `.gx/ledger/journal.blobs/`. `BodyMissing` is additive in the same sense `Pending` was:
    // 42 §3.12's cell names it and the four original values did not move a letter.
    // 🔴 **DR-46-13 / §237-5** — seven since DR-46-24(A)'s erratum batch. `Undetermined` is
    // additive in the same sense the last two were: 42 §3.12's cell names it and the six before it
    // did not move a letter. The loop above is what holds the cell and the type together, so this
    // number is the only thing that had to be re-counted by hand.
    assert_eq!(InverseStatus::ALL_KINDS.len(), 7);
    assert_eq!(InverseStatus::Available.kind(), "Available");
    assert_eq!(
        InverseStatus::Consumed { by: tid(1) }.kind(),
        "Consumed",
        "the payload does not change the name"
    );
}

/// The two constructors put the status and the delta in step by construction.
#[test]
fn the_constructors_cannot_build_the_contradiction() {
    let held = EscrowedInverse::held(tid(1), delta(), None);
    assert_eq!(held.status(), &InverseStatus::Available);
    assert!(held.inverse_delta().is_some());
    assert_eq!(
        held.retained_until(),
        None,
        "DR-9: the OSS default is unlimited (sem: SEM-gx-engine-739)"
    );

    let none = EscrowedInverse::unavailable(tid(2));
    assert_eq!(none.status(), &InverseStatus::Unavailable);
    assert!(
        none.inverse_delta().is_none(),
        "`invert()` returned None: there is nothing to hold"
    );
    assert_eq!(none.transformation(), tid(2));
}

/// 🔴 `restore` refuses both halves of the contradiction (E-6's checked read-back).
///
/// Both directions, because they are different mistakes: a delta beside `Unavailable` is a store
/// that kept a body it said it never had, and a missing delta beside `Available` is a store that
/// promised an undo it cannot run — which is DR-1(a)'s guarantee failing silently.
#[test]
fn restore_refuses_a_status_that_disagrees_with_its_payload() {
    let with_delta =
        EscrowedInverse::restore(tid(1), Some(delta()), None, InverseStatus::Unavailable)
            .expect_err("a delta beside Unavailable");
    let without = EscrowedInverse::restore(tid(1), None, None, InverseStatus::Available)
        .expect_err("Available with nothing to apply");
    let consumed =
        EscrowedInverse::restore(tid(1), None, None, InverseStatus::Consumed { by: tid(2) })
            .expect_err("Consumed with nothing to have consumed");

    println!(
        "ESCROW_REFUSALS={} {} {}",
        with_delta.kind(),
        without.kind(),
        consumed.kind()
    );
    for e in [with_delta, without, consumed] {
        assert_eq!(e.kind(), "InconsistentEscrow");
    }

    let ok = EscrowedInverse::restore(
        tid(1),
        Some(delta()),
        Some(Timestamp(99)),
        InverseStatus::Consumed { by: tid(2) },
    )
    .expect("a consumed inverse still holds its body");
    assert_eq!(ok.retained_until(), Some(Timestamp(99)));
}

/// No setter: the status moves at T-12 and T-12 is hand 6 (M5-16 adopted (a), sem:
/// SEM-gx-engine-740).
///
/// A road to a transition that does not exist is exactly what "not one transition is
/// implemented" forbids, and
/// this is the "absence" scan (sem: SEM-gx-engine-740) that says the road was not built
/// early. `AppliedDelta` in gx-substrate
/// carries the same shape for the same reason (four accessors, no setters).
#[test]
fn nothing_in_this_hand_moves_a_status() {
    let source = read_repo("crates/gx-engine/src/store.rs");
    let offenders: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//"))
        .filter(|l| {
            l.contains("fn set_status")
                || l.contains("fn consume")
                || l.contains("fn mark_consumed")
                || l.contains("fn expire")
        })
        .collect();
    println!("ESCROW_MUTATORS={} ({offenders:?})", offenders.len());
    assert!(
        offenders.is_empty(),
        "M5-16 puts the write at T-12, in hand 6"
    );
}
