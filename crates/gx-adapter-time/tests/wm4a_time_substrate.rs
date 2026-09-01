// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! What this substrate is for, measured: the three verdicts, the invariants, and the window that
//! closes by itself.
//!
//! Ticket `req/1007` §4 item 3 (**WM-4a**), rulings `req/1038`. The conformance obligations are in
//! `tests/conformance.rs`; this file holds the claims that are this substrate's own and that no
//! shared harness asks for -- `req/1038` §6a measured that the harness reads `invert`'s `Option`
//! projection and never its verdict, so **the harness cannot tell `False` from `Unknown`** and the
//! separation has to be asserted here or nowhere.

mod support;

use gx_adapter_time::adapter::content_digest;
use gx_adapter_time::{Entry, TimeAdapter};
use gx_core::Reversibility;
use gx_substrate::SubstrateAdapter;
use support::{bytes, goal, heavy_entry, intent_for, silent, Sandbox, HEAVY, SILENT, SUBJECT};

/// The `True` row: a schedule that records firedness gets an inverse with a body.
#[test]
fn a_schedule_that_records_firedness_yields_an_inverse_with_a_body() {
    let sandbox = Sandbox::new();
    let adapter = TimeAdapter::new();
    let position = sandbox.locator(SUBJECT);

    let pre = adapter.snapshot(&position).expect("the entry is there");
    let delta = adapter
        .plan(&intent_for(&position, &bytes(&goal())), &pre)
        .expect("the intent is plannable");
    let outcome = adapter.invert(&delta, &pre).expect("the position answers");

    assert_eq!(outcome.verdict(), Reversibility::True);
    assert!(
        outcome.inverse().is_some(),
        "`True` claims an inverse was constructed, so the body has to be here"
    );
    assert_eq!(
        outcome.reads().len(),
        1,
        "the escrow read the position and the read-set has to say so (DEFECT-892-1)"
    );
    println!("WM4A_VERDICT_RECORDED={}", outcome.verdict().as_str());
}

/// The `Unknown` row: a schedule that keeps no record of firedness cannot be judged, now or later.
///
/// 🔴 This is the assertion the whole crate turns on. Collapsing this case into `False` would be
/// reporting a measurement nobody took, and collapsing it into `True` would be claiming an undo
/// restores a world nothing can observe. The read still happened and is still attested -- "gx read
/// the entry and could not establish whether an undo would un-run something" is a different fact
/// from "gx never looked".
#[test]
fn a_schedule_that_keeps_no_record_of_firedness_answers_unknown_and_carries_no_body() {
    let sandbox = Sandbox::new();
    let adapter = TimeAdapter::new();
    sandbox.put(SILENT, &silent());
    let position = sandbox.locator(SILENT);

    let pre = adapter.snapshot(&position).expect("the entry is there");
    let delta = adapter
        .plan(&intent_for(&position, &bytes(&goal())), &pre)
        .expect("the intent is plannable");
    let outcome = adapter.invert(&delta, &pre).expect("the position answers");

    assert_eq!(
        outcome.verdict(),
        Reversibility::Unknown,
        "a schedule with no firedness record leaves the question unestablished"
    );
    assert!(outcome.inverse().is_none());
    assert_eq!(
        outcome.reads().len(),
        1,
        "unknown is not the same as unlooked-at"
    );
    println!("WM4A_VERDICT_SILENT={}", outcome.verdict().as_str());
}

/// The `False` row, and the negative control that keeps it apart from the row above.
///
/// Two entries, two `None` bodies, **two different verdicts**. The shared harness sees the same
/// `None` for both (`req/1038` §9-1); this is the assertion that the adapter does not.
#[test]
fn an_inverse_over_the_escrow_ceiling_answers_false_which_is_not_the_same_as_unknown() {
    let sandbox = Sandbox::new();
    let adapter = TimeAdapter::new();
    sandbox.put(HEAVY, &heavy_entry());
    sandbox.put(SILENT, &silent());

    let heavy_position = sandbox.locator(HEAVY);
    let heavy_pre = adapter
        .snapshot(&heavy_position)
        .expect("the entry is there");
    let heavy_delta = adapter
        .plan(&intent_for(&heavy_position, &bytes(&goal())), &heavy_pre)
        .expect("the intent is plannable");
    let over_ceiling = adapter
        .invert(&heavy_delta, &heavy_pre)
        .expect("the position answers");

    let silent_position = sandbox.locator(SILENT);
    let silent_pre = adapter
        .snapshot(&silent_position)
        .expect("the entry is there");
    let silent_delta = adapter
        .plan(&intent_for(&silent_position, &bytes(&goal())), &silent_pre)
        .expect("the intent is plannable");
    let unrecorded = adapter
        .invert(&silent_delta, &silent_pre)
        .expect("the position answers");

    assert_eq!(over_ceiling.verdict(), Reversibility::False);
    assert_eq!(unrecorded.verdict(), Reversibility::Unknown);
    assert!(over_ceiling.inverse().is_none());
    assert!(unrecorded.inverse().is_none());
    assert_ne!(
        over_ceiling.verdict(),
        unrecorded.verdict(),
        "both answer `None` and they are not the same answer"
    );
    println!(
        "WM4A_THREE_VALUES true={} false={} unknown={}",
        Reversibility::True.as_str(),
        over_ceiling.verdict().as_str(),
        unrecorded.verdict().as_str()
    );
}

/// **INV-WM4a-1**: gx never writes the assertion that an entry has already run.
///
/// The positive control is the same call with `fired: Some(false)`, which plans: the refusal is
/// about the *claim*, not about the field being present, and a test with only the negative half
/// would pass just as well against an adapter that refused every entry.
#[test]
fn gx_never_writes_the_assertion_that_an_entry_has_already_run() {
    let sandbox = Sandbox::new();
    let adapter = TimeAdapter::new();
    let position = sandbox.locator(SUBJECT);
    let pre = adapter.snapshot(&position).expect("the entry is there");

    let already_ran = Entry {
        fired: Some(true),
        ..goal()
    };
    let refused = adapter.plan(&intent_for(&position, &bytes(&already_ran)), &pre);
    assert!(
        refused.is_err(),
        "gx planned an entry claiming it had already run"
    );

    let not_yet = adapter.plan(&intent_for(&position, &bytes(&goal())), &pre);
    assert!(
        not_yet.is_ok(),
        "the positive control: an entry that has not run is ordinary and plans"
    );
    println!("WM4A_INV1_REFUSED_FIRED_TRUE=1 PLANNED_FIRED_FALSE=1");
}

/// 🔴 The window closes by itself: firedness is inside the digest, so the CAS stops matching.
///
/// This is the mechanism `req/1038` §6b settled on after the first design -- a verdict that expired
/// -- was withdrawn for being unaskable at escrow time (43 T-10b builds the inverse *before*
/// `apply`, when the effect has not happened and cannot have fired).
///
/// The negative control is the second half: rewriting the *same* entry leaves the fingerprint
/// comparing equal, so what moved the first comparison was the firedness and not the act of
/// writing.
#[test]
fn the_undo_window_closes_because_firedness_is_inside_the_fingerprint() {
    let sandbox = Sandbox::new();
    let adapter = TimeAdapter::new();
    let position = sandbox.locator(SUBJECT);

    let escrowed = adapter
        .precondition(&adapter.snapshot(&position).expect("the entry is there"))
        .expect("the position answers");

    // What the runner does, and only the runner (INV-WM4a-1): the job ran, so the entry says so.
    sandbox.mark_fired(SUBJECT);
    let after_firing = adapter
        .precondition(&adapter.snapshot(&position).expect("the entry is there"))
        .expect("the position answers");

    assert!(
        !escrowed
            .cas_eq(&after_firing)
            .expect("one substrate, one scope"),
        "an undo attempted after the entry fired has to be refused by the compare-and-set, and \
         this is the comparison the engine makes"
    );

    // Negative control: the same bytes written again move nothing.
    let standing = sandbox.read(SUBJECT);
    sandbox.put_raw(SUBJECT, &standing);
    let rewritten = adapter
        .precondition(&adapter.snapshot(&position).expect("the entry is there"))
        .expect("the position answers");
    assert!(
        after_firing
            .cas_eq(&rewritten)
            .expect("one substrate, one scope"),
        "writing the same entry again is not a change, and a fingerprint that moved here would \
         make every undo fail for the wrong reason"
    );
    println!("WM4A_CAS_CLOSES_WINDOW=1 CAS_STABLE_ON_REWRITE=1");
}

/// **INV-WM4a-2**: this crate names no clock.
///
/// Comment lines are stripped before the scan, because the crate root and two modules *discuss*
/// clocks at length and a scanner that read prose would be measuring the documentation. The
/// positive control is on the same stripped text: `std::fs::read` is in `adapter.rs` and the scan
/// finds it, so a scan that found nothing would have been caught rather than believed.
#[test]
fn this_crate_names_no_clock() {
    let src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut scanned = 0usize;
    let mut offenders: Vec<String> = Vec::new();
    let mut control = 0usize;

    for entry in std::fs::read_dir(&src).expect("the source directory is readable") {
        let path = entry.expect("a readable entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("the module is readable");
        let code: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        scanned += 1;
        for token in ["SystemTime", "Instant", "UNIX_EPOCH", "elapsed()"] {
            if code.contains(token) {
                offenders.push(format!("{}: {token}", path.display()));
            }
        }
        control += usize::from(code.contains("std::fs::read"));
    }

    println!(
        "WM4A_CLOCK_SCAN_FILES={scanned} OFFENDERS={} CONTROL_HITS={control}",
        offenders.len()
    );
    assert!(scanned >= 6, "every module of this crate is scanned");
    assert!(
        control >= 1,
        "the positive control: the scanner has to be able to find a token that is there"
    );
    assert!(
        offenders.is_empty(),
        "41 §6 injects time at the engine boundary; these name a clock: {offenders:?}"
    );
}

/// L5's two roads, for this adapter: the promise, and what `apply` observed.
///
/// Road 1 digests the goal bytes with no position read on this line or above it. Road 2 plans,
/// applies, and reads the position back. What the comparison cannot catch is that both end in the
/// one digest function 41 §6 admits -- a defect *inside* the mint is invisible to it, which is the
/// same bound `gx-adapter-fs/tests/ac_049.rs` states.
#[test]
fn the_promise_and_the_observation_are_reached_by_different_roads() {
    let entry = goal();
    let road_one = content_digest(&bytes(&entry));

    let sandbox = Sandbox::new();
    let adapter = TimeAdapter::new();
    let position = sandbox.locator(SUBJECT);
    let pre = adapter.snapshot(&position).expect("the entry is there");
    let delta = adapter
        .plan(&intent_for(&position, &bytes(&entry)), &pre)
        .expect("the intent is plannable");

    assert_eq!(
        delta.promised_target(),
        Some(road_one),
        "this adapter fills the prophecy seat at plan time (req/1020's road, third adapter on it)"
    );

    let applied = adapter
        .apply(&delta)
        .expect("the schedule accepts the entry");
    let road_two = *applied.resulting_digest();

    println!(
        "WM4A_L5_ROUTE_GOAL={} ROUTE_OBSERVED={} AGREE={}",
        gx_canon::cid::to_text(&road_one),
        gx_canon::cid::to_text(&road_two),
        road_one == road_two
    );
    assert_eq!(road_one, road_two);
}
