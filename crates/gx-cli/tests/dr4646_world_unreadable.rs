// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-46 (`req/973` §9-3, filed 2026-08-31, repaired here)** — the settle pre-flight's two
//! "the probe would not read" arms name **which** nothing they had.
//!
//! Spec: 43 §5.2 for the witness vocabulary, 42 §3.10/§3.13 for where the word ends up.
//!
//! # What was wrong, and why the prose was not the defect
//!
//! Both arms already said the *cause* on stderr in English ("the probe could not read the world",
//! "the world would not read afterwards"). What they **returned** was
//! `Unobservable::NoPostcondition`, whose sentence is "the commit receipt carries no postcondition"
//! — a different fact about a different object. Before DR-46-45 nothing published that word and the
//! mismatch cost nothing; DR-46-45 put the disposition inside the signed receipt, and from then on
//! the wrong word was a wrong word in signed bytes.
//!
//! So the repair is a variant (`Unobservable::WorldUnreadable`, one arm per cause — `req/38` §231
//! ruling 5) and not a rewording, and these probes assert the **witness**, not the prose.
//!
//! # 🔴 Why the assertion is on the word and not on a receipt
//!
//! On the fs substrate a world the pre-flight could not read is a world `Engine::undo` cannot
//! snapshot either, so this road ends in an adapter refusal before T-11 mints anything: there is no
//! receipt here to look inside. The word is therefore asserted where the caller says it out loud —
//! `UndoWitness::word()`, the one spelling DR-46-45 minted so that stdout, HTTP's `witness` field
//! and the signed payload cannot drift apart. That the same word reaches signed bytes is measured
//! one crate down, on a substrate that *can* be snapshotted, by
//! `gx-engine/tests/dr4646_world_unreadable_witness.rs`. Two probes because it is two facts:
//! **this arm produces that witness**, and **that witness reaches the signature**.
//!
//! # 🔴 What this file measured that `req/973` §9-3 did not know
//!
//! The release condition asks for "a probe that fires each" of the two sites. One of them fires here
//! end to end; the other does not, and the reason is a fact about the road rather than a gap in this
//! lane's effort — `lifecycle::undo` snapshots the same locator two lines earlier, in
//! `Session::rehydrate_committed`, so a cold `gx undo` over an unreadable world never reaches the
//! pre-flight. The first site is reachable only through a transient failure inside a window no
//! outside party can schedule. That is written down as its own arm below rather than smoothed over,
//! and it is the one part of DR-46-46's release condition this lane did **not** discharge as
//! written.

mod support;

use support::{pipeline, run};

/// The sentence the repaired arms declare, as the caller prints it.
const WORLD_UNREADABLE: &str = "unobservable:the world could not be read when the undo probed it";

/// The sentence they used to declare, about an object nobody had asked about.
const NO_POSTCONDITION: &str = "unobservable:the commit receipt carries no postcondition";

/// 🔴 **The denominator arm** — measured while writing this file, and the reason the sibling probe
/// above is the only end-to-end one.
///
/// DR-46-46 names *two* sites, and the intent here was to fire the first one (the probe taken under
/// the lock, before any waiting) by taking the world away before `gx undo` starts. Measured, that
/// never reaches the pre-flight: `Session::rehydrate_committed` (`session.rs`, calling
/// `Engine::rehydrate_committed` at `pipeline.rs`'s `let pre = adapter.snapshot(&locator)`) rebuilds
/// `T_o`'s table entry from Σ and **snapshots the same locator** first, on `lifecycle::undo`'s
/// second line. A cold `gx undo` over an unreadable world therefore answers `ADAPTER_ERROR` with no
/// settle line at all.
///
/// So on the shipped single-shot road the first site is reachable only by a **transient** read
/// failure inside the window between that snapshot and the pre-flight's probe — receipt load, DSSE
/// verify, a key-store read — which no third party can schedule from outside the process. It is not
/// dead code and it is not drivable here, and this arm records which of the two it is rather than
/// leaving a reader to assume the probe above covered both. The site carries the repair regardless,
/// because a repair that fixed one and not the other would leave the false word reachable the moment
/// the window is entered.
///
/// What is asserted is the boundary itself, so that a future build which moves the rehydrate
/// snapshot — and thereby makes the first site drivable — turns this red instead of quietly
/// changing what "arm 1 is unreachable from a cold start" means.
#[test]
fn a_cold_undo_over_an_unreadable_world_is_refused_by_the_rehydrate_before_the_preflight() {
    let fixture = pipeline("dr4646_first_probe", "before\n");
    let committed = fixture.commit_one("after\n");
    std::fs::remove_file(&fixture.target).expect("take the world away before the undo starts");

    let undone = run(fixture.gx().args(["undo", &committed, "--settle", "1"]));
    println!(
        "DR4646_DENOMINATOR exit={} stderr={}",
        undone.code,
        undone.stderr.trim()
    );

    assert!(
        undone.stderr.contains("ADAPTER_ERROR") && undone.stderr.contains("snapshot"),
        "the boundary: the rebuild of T_o's row snapshots the locator and refuses first: {}",
        undone.stderr
    );
    assert!(
        !undone.stderr.contains("gx undo settle:"),
        "and the pre-flight is never reached, which is why the first of DR-46-46's two sites cannot \
         be driven from a cold start: {}",
        undone.stderr
    );
    assert!(
        !undone.stderr.contains(NO_POSTCONDITION) && !undone.stderr.contains(WORLD_UNREADABLE),
        "no witness is declared on this road at all — the run ends before one is computed: {}",
        undone.stderr
    );
}

/// 🔴 Arm 2 — the twin, taken after the wait when the lock comes back.
///
/// Reaching it needs a first probe that **read** the world and did not match (a matching first probe
/// returns before the wait), and then a world that stops being readable while the pre-flight is
/// outside the lock. So: a third party moves the file, and takes it away five seconds into a
/// twenty-second settle budget.
///
/// 🔴 The one timing this test has, stated rather than hidden: the mutation must land after the
/// first probe. The budget is twenty seconds and the mutation is at five, so the only way to lose
/// that race is for `gx` to take longer than five seconds to reach its first probe — and if it does,
/// arm 1 fires instead and the assertion below fails by name rather than passing quietly.
#[test]
fn the_probe_after_the_wait_declares_the_same_absence() {
    let fixture = pipeline("dr4646_second_probe", "before\n");
    let committed = fixture.commit_one("after\n");
    std::fs::write(&fixture.target, "moved by a third party\n").expect("move the world");

    let target = fixture.target.clone();
    let mutator = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(5));
        std::fs::remove_file(&target).expect("take the world away during the wait");
    });

    let undone = run(fixture.gx().args(["undo", &committed, "--settle", "20"]));
    mutator.join().expect("the third party finishes");
    println!(
        "DR4646_ARM2 exit={} stderr={}",
        undone.code,
        undone.stderr.trim()
    );

    assert!(
        undone
            .stderr
            .contains("the world would not read afterwards"),
        "the fixture's premise: the twin arm is the one that ran. If this fails with \"the probe \
         could not read the world\" instead, the mutation landed before the first probe and the \
         test measured arm 1: {}",
        undone.stderr
    );
    assert!(
        undone.stderr.contains(WORLD_UNREADABLE),
        "DR-46-46: the twin declares the same absence as its sibling — a repair that fixed one site \
         and not the other would leave the false word reachable by the slower road: {}",
        undone.stderr
    );
    assert!(
        !undone.stderr.contains(NO_POSTCONDITION),
        "DR-46-46: and it does not name the receipt's postcondition either: {}",
        undone.stderr
    );
}
