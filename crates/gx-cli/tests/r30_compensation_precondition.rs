// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R30 / `req/372` M-01 (`req/38` §240 ruling 2, `req/374` item 1)** — 43 T-10c's compensation
//! is a **write**, and until this lane it was the only write this engine issued with nothing in
//! front of it.
//!
//! # The defect, as the twenty-ninth adversarial audit produced it
//!
//! The audit drove all four shipped adapters and established that every shipped delta grammar is
//! **absolute**: no payload in any of them has an effect that is a function of the state it starts
//! from. For a compensation that is a good property — an absolute inverse comes home from a world a
//! half-failed apply left half-moved. It is a dangerous property for an **unconditional**
//! compensation, because "any world" includes a world a third party legitimately created. The audit
//! measured it on a real branch:
//!
//! ```text
//! A29_GIT_THIRD_PARTY head prior=de05de3 theirs=d2d09b5 after_rollback=de05de3
//! A29_GIT_THIRD_PARTY word=Succeeded their_commit_is_still_the_tip=false
//! ```
//!
//! A colleague's commit, off the branch, with `Succeeded` in the record. The word was not lying —
//! the object *was* back at `fp0` — and that is the point: `fp0` is a statement about this
//! transformation's object and says nothing about whose work was standing on it.
//!
//! # 🔴 Why this file exists rather than a re-run of the audit's own arm
//!
//! The arm that measured the defect —
//! `crates/gx-adapter-git/tests/a29_shipped_delta_grammar.rs`, the test printing
//! `A29_GIT_THIRD_PARTY` — is a **reconstruction**. It calls `adapter.apply(&inverse)`, then
//! `snapshot`, then `precondition` **in the test body**, reproducing the engine's old three-call
//! order rather than driving the engine. It cannot observe a repair made inside
//! `crates/gx-engine/src/pipeline.rs`, so re-running it unchanged proves nothing about this fix and
//! it is preserved untouched as the evidence of what was found. This file drives the **real**
//! engine over the same situation, through `gx_api::router` and the shipped fs policy pack, so that
//! what is asserted is the product's behaviour and not a copy of it.
//!
//! # The repair, as the source states it
//!
//! The T-10c arm now reads the object **twice**. The first read is taken the instant the forward
//! `apply` answers `Err` and asks *what did our own apply leave behind*
//! (`Engine::world_the_failed_apply_left` → `CompensationVerdict`); the second is taken immediately
//! before the compensation and asks *is the world still where our apply left it*
//! (`Engine::world_is_still_at`). Four outcomes, of which exactly one sends the inverse.
//!
//! # What each arm asserts, and why they are in this order
//!
//! 1. **the bed** — an undriven road commits and undoes and the world really comes home. Without
//!    it every refusal below could be green because the harness refused on its own account.
//! 2. **the audit's shape, through the engine** — the forward apply fails having moved **nothing**
//!    and a third party writes before the compensation would have run. Their bytes survive, the
//!    wire word is `NotAttempted`, the cause is `WorldNeverMoved`, and the fixture's own apply
//!    counter says the compensating apply was never sent.
//! 3. **the window shape** — the forward apply **moves** the world and then fails, and the third
//!    party writes between the engine's two reads. `NotAttempted` + `WorldMovedBeneath`.
//! 4. **🔴 the negative control, and it is the load-bearing one** — arm 3's road with no third
//!    party. The compensation must still **run** and the object must come home, `Succeeded`. An
//!    earlier draft of this repair was a blanket refusal, and every other arm here was green over
//!    it; this is the shape that catches that.
//! 5. **the derivation** — `crates/gx-engine/src/pipeline.rs` as text, so that the behaviour above
//!    cannot be produced by anything other than reads placed **in front of** the compensating
//!    apply, with controls that an empty derivation and a reordered one both fail.
//! 6. **the vocabulary** — six causes declared, six arms in `crates/gx-cli/src/wrap.rs`, so a
//!    seventh cannot fall silently into the arm for a cause this build does not know.
//!    (🔴 The seventh has since arrived *with* a declaration and an arm — R-1001-1, `req/1001`
//!    §4, 2026-08-31: `PromisedPostStateWasWrong`. The count in item 6's test moved with that
//!    ruling and cites it; the line above is R30's own account of its window.)
//!
//! # 🔴 Where the premises come from
//!
//! "The compensating apply was never sent" is read off `ScriptedAdapter::applies` — the fixture's
//! own count of the calls it received — and never off the answer gx hands back. Asking gx whether
//! gx did the right thing is the failure mode this repository exists to close, and a probe that did
//! it would be measuring the report rather than the world.
//!
//! # What this file does **not** claim
//!
//! It does not claim the window is closed. The repair is a compare-and-set spelled as two calls, so
//! a third party who writes between the second read and the `apply` is still overwritten; the
//! engine's own doc and `docs/LIMITS.md` carry that residue. Nothing here measures its width. It
//! also does not claim attribution: this fixture's third party is a write, and the engine cannot
//! tell it from this transformation's own half-landed apply — both land on `WorldMovedBeneath`.

// 🔴 `req/817`: this suite exercises a verb whose mechanism is `gx-mcp-wire`,
// one of the four crates `req/789` §3 holds private. The public distribution builds without
// it (`default = []` there), so the whole file compiles away rather than failing to resolve
// a crate that is not in the tree. The private build turns `mcp` on by default and runs it.
#![cfg(feature = "mcp")]

use std::path::{Path, PathBuf};

use gx_cli::wrap::apply_failed_clause;
use serde_json::json;

#[path = "support/scripted_substrate.rs"]
mod scripted_substrate;

use scripted_substrate::{block_on, record, Behaviour, Grammar, ScriptedServer};

/// The world every arm starts from.
const INITIAL: [&str; 2] = ["A", "B"];

/// The goal the commit is given: the initial world plus two more records.
const GOAL: &str = "A\nB\nC\nD\n";

/// What the third party writes. The audit's own bytes, so that the two measurements are talking
/// about the same event: `a29_shipped_delta_grammar.rs` commits
/// `b"a colleague's honest work\n"` over the branch.
const THEIRS: &str = "a colleague's honest work\n";

/// The forward apply fails having performed **none** of its operations: the world is exactly where
/// the transformation found it, and there is no effect to compensate.
///
/// Index 0 is the commit's own apply, which must land; index 1 is the undo's forward apply, which
/// is the one whose failure opens 43 T-10c.
const NOTHING_MOVED: [Behaviour; 2] = [Behaviour::Full, Behaviour::HalfThenFail(0)];

/// The forward apply performs its operation **and then** answers `Err`: the world has moved and
/// there is something to take back.
const MOVED_THEN_FAILED: [Behaviour; 2] = [Behaviour::Full, Behaviour::HalfThenFail(1)];

/// The apply the third-party hook is timed against: the undo's forward apply, the second this
/// fixture receives on every road here.
const THE_FAILING_APPLY: usize = 2;

/// The `rollback` member of a refusal, as a plain string, or `<absent>` / `<null>`.
///
/// The three cases are told apart rather than folded into an `unwrap_or_default`, for
/// `r29_rollback_is_verified.rs`'s reason: 44 §2.3's problem object carries these as **extension
/// members** (`req/334` M-01) and `null` there is an honest answer rather than a missing one.
fn member(body: &serde_json::Value, name: &str) -> String {
    match body.get(name) {
        None => "<absent>".to_string(),
        Some(serde_json::Value::Null) => "<null>".to_string(),
        Some(value) => value.as_str().unwrap_or("<not a string>").to_string(),
    }
}

fn rollback_word(body: &serde_json::Value) -> String {
    member(body, "rollback")
}

/// The cause, as `crates/gx-api/src/problem.rs` spells it on the wire.
///
/// The member is `rollback_not_attempted_because`; `not_attempted_because` is the name the same
/// fact carries on the commit object `crates/gx-cli/src/wrap.rs` reads. Both are asserted in this
/// file, at the layer each one belongs to.
fn because_word(body: &serde_json::Value) -> String {
    member(body, "rollback_not_attempted_because")
}

/// Print the adapter's own account of every `apply` and of every answered read, tagged.
///
/// Both traces are the fixture's: each line was written inside the call at the moment it ran,
/// before anything downstream could have an opinion about it.
fn traces(tag: &str, server: &ScriptedServer) {
    for line in server.adapter.trace() {
        record(&format!("R30_TRACE_{tag} {line}"));
    }
    for line in server.adapter.read_trace() {
        record(&format!("R30_READS_{tag} {line}"));
    }
}

// ---------------------------------------------------------------------------
// 1. The bed
// ---------------------------------------------------------------------------

/// 🔴 The **bed control** for everything below (`req/372` M-01, `req/38` §240 ruling 2).
///
/// The same fixture, the same grammar and no failure scripted anywhere: the road commits a change
/// and then undoes it, and the object really is back at the bytes it started from. Every arm below
/// reads either a refusal or a homecoming, and a harness that refused for a reason of its own — a
/// gate that denied, an archive holding no receipt, a locator the pack forbids — would make the
/// refusal arms green while measuring nothing. This is the arm that fails first when that happens.
///
/// The apply count is asserted too: two applies and no more means 43 T-10c did not fire on a road
/// where nothing failed, so the counter the arms below rest their central premise on is known to be
/// reading the thing they think it reads.
#[test]
fn an_undriven_road_commits_and_undoes_and_the_world_comes_home() {
    let server = ScriptedServer::start("r30_bed", Grammar::Absolute, &INITIAL);
    let prior = server.world();

    let (status, body, after) = block_on(async {
        let id = server.commit_goal(GOAL).await;
        let committed = server.world();
        assert_eq!(
            committed, GOAL,
            "the bed is broken before the undo: the commit's own apply did not land"
        );
        let (status, body) = server.undo(&id).await;
        (status, body, server.world())
    });

    record(&format!(
        "R30_BED status={status} rollback={} prior={prior:?} after={after:?} came_home={} \
         applies={} reads={} third_party_at_read={}",
        rollback_word(&body),
        after == prior,
        server.adapter.applies(),
        server.adapter.reads(),
        server.adapter.third_party_fired_at_read(),
    ));
    traces("BED", &server);

    assert_eq!(
        status, 200,
        "the bed control's undo must succeed. The arms below read refusals, so a harness that \
         refuses on its own account would make them green while measuring nothing: {body}"
    );
    assert_eq!(
        after, prior,
        "the bed control's undo must put the object back. If this fails, the fixture cannot tell a \
         restored world from a destroyed one and nothing below is measurable"
    );
    assert_eq!(
        server.adapter.applies(),
        2,
        "two applies and no more: the commit's and the undo's. A third would mean 43 T-10c fired \
         on a road nothing failed on, and the apply counter is the instrument arms 2 and 3 rest \
         their central premise on"
    );
    assert_eq!(
        server.adapter.third_party_fired_at_read(),
        0,
        "🔴 the third-party hook is **opt-in and off** unless a probe arms it. If it fired here, \
         every other user of this fixture is driving a road it did not ask for"
    );
}

// ---------------------------------------------------------------------------
// 2. The audit's shape, driven through the real engine
// ---------------------------------------------------------------------------

/// 🔴 **The audit's finding, through the engine** — the forward apply fails having moved
/// **nothing**, a third party writes, and the third party's bytes are still there afterwards
/// (`req/372` M-01, `req/38` §240 ruling 2, `req/374` item 1).
///
/// This is `A29_GIT_THIRD_PARTY`'s situation with the reconstruction taken out of it. The undo's
/// forward apply performs **zero** of its operations and answers `Err` — the contract-conforming
/// failure `crates/gx-substrate/src/error.rs` permits — so the object is exactly where the
/// transformation found it. The third party then writes, timed by the fixture to land after the
/// engine's first read has been answered and before the compensation could run.
///
/// The old engine sent the escrowed inverse here unconditionally. It is **absolute**, so it would
/// have restored the object to `fp0` from the third party's world, erased their write, and reported
/// `Succeeded` — a transformation that did nothing erasing somebody else's work and nothing else.
///
/// Four things are asserted and each fails differently:
///
/// * the third party's bytes are on disk afterwards, read straight off the world file;
/// * the wire word is `NotAttempted`;
/// * the cause is `WorldNeverMoved` and not one of the other five;
/// * 🔴 the compensating apply was **never sent**, taken from `ScriptedAdapter::applies` — the
///   fixture's own count of calls it received — rather than from gx's account of itself.
#[test]
fn a_failed_apply_that_moved_nothing_does_not_erase_a_third_partys_write() {
    let server = ScriptedServer::start("r30_third_party", Grammar::Absolute, &INITIAL);

    let (status, body, prior, after) = block_on(async {
        let id = server.commit_goal(GOAL).await;
        let prior = server.world();
        server.adapter.script(&NOTHING_MOVED);
        server
            .adapter
            .third_party_writes_after_apply(THE_FAILING_APPLY, THEIRS);
        let (status, body) = server.undo(&id).await;
        (status, body, prior, server.world())
    });

    let applies = server.adapter.applies();
    let sent_a_compensation = applies > 2;

    record(&format!(
        "R30_THIRD_PARTY status={status} gx_code={} rollback={} because={} prior={prior:?} \
         theirs={THEIRS:?} after={after:?} their_write_survives={} \
         compensating_apply_was_sent={sent_a_compensation} applies={applies} \
         third_party_at_read={}",
        body["gx_code"].as_str().unwrap_or("<absent>"),
        rollback_word(&body),
        because_word(&body),
        after == THEIRS,
        server.adapter.third_party_fired_at_read(),
    ));
    // The same sentence the audit printed, in the audit's own shape, so the two lines can be read
    // side by side: `A29_GIT_THIRD_PARTY word=Succeeded their_commit_is_still_the_tip=false`.
    record(&format!(
        "R30_THIRD_PARTY_VS_A29 a29=word=Succeeded,their_commit_is_still_the_tip=false \
         r30=word={},their_write_survives={}",
        rollback_word(&body),
        after == THEIRS,
    ));
    traces("THIRD_PARTY", &server);

    // The premises first. A claim about the word over a road that did not happen is a claim about
    // nothing, and all three of these come from the fixture rather than from the answer.
    assert_eq!(
        status, 422,
        "the undo's own apply failed, so 44 §2.3's `APPLY_FAILED` row and its status: {body}"
    );
    assert_eq!(
        body["gx_code"].as_str(),
        Some("APPLY_FAILED"),
        "the road this arm is about is the failed apply's: {body}"
    );
    let account = server.adapter.trace();
    assert_eq!(
        account.len(),
        2,
        "the fixture received exactly the commit's apply and the undo's: {account:?}"
    );
    assert!(
        account[1].contains("performed=0"),
        "🔴 this arm's premise is that the forward apply moved **nothing**. The fixture's own \
         account of that call says otherwise: {account:?}"
    );
    assert!(
        server.adapter.third_party_fired_at_read() > 0,
        "🔴 and its other premise is that somebody else wrote. The hook never fired, so this arm \
         is measuring a road with no third party on it: {:?}",
        server.adapter.read_trace()
    );
    assert_ne!(
        prior, THEIRS,
        "the third party's bytes must differ from the world they landed on, or 'their write \
         survives' cannot be told from 'nothing happened'"
    );

    // Then the world, then the word. The world first: a word over an object that is fine is a word
    // about nothing.
    assert_eq!(
        after, THEIRS,
        "🔴 **THE FINDING, CLOSED.** The third party's bytes must still be on the object. The \
         world began at {prior:?}, the forward apply moved nothing, {THEIRS:?} was written by \
         somebody else, and the object now holds {after:?}. If it holds {prior:?} the escrowed \
         inverse was sent over the top of their work — which is `A29_GIT_THIRD_PARTY` exactly, one \
         substrate along"
    );
    assert!(
        !sent_a_compensation,
        "🔴 and the reason must be that **no compensating apply was sent**, read off the fixture's \
         own counter rather than off gx's account of itself. The adapter received {applies} \
         applies; two is the commit's and the undo's, and a third is the compensation. Trace: \
         {account:?}"
    );
    assert_eq!(
        rollback_word(&body),
        "NotAttempted",
        "🔴 the word for a compensation that was refused is `NotAttempted`. `Succeeded` is the \
         defect, and `Diverged` or `Failed` would both claim the inverse reached the adapter: \
         {body}"
    );
    assert_eq!(
        because_word(&body),
        "WorldNeverMoved",
        "🔴 and the cause has to be the one that is true. `req/324` §5(d): the value is \
         constructed on six roads and a reader told the wrong one has been handed a confident \
         account of an observation nobody made. Here the forward apply moved nothing, which is \
         `WorldNeverMoved` and not `WorldMovedBeneath` (nobody's write was observed by the read \
         that decided this) and not `WorldCouldNotBeRead` (the read answered): {body}"
    );
}

// ---------------------------------------------------------------------------
// 3. The window shape
// ---------------------------------------------------------------------------

/// 🔴 **The window shape** — the forward apply **moved** the world and then failed, and somebody
/// wrote between the engine's two reads (`req/372` M-01, `req/38` §240 ruling 2).
///
/// This is the road the second read exists for. The first read finds the object away from `fp0` and
/// answers `CompensationVerdict::TakeBackFrom(left_at)` — there **is** an effect to compensate. The
/// second read, taken immediately before the compensation, finds the object somewhere else again,
/// and that disagreement is a third party, because our own apply had already finished when the
/// first read was taken. The information separating the two cases is *time*, not fingerprints,
/// which is why one read compared against `fp0` cannot do this and two reads can.
///
/// The write is landed in the window by the fixture rather than by a thread: `ScriptedAdapter`
/// answers the engine's read from the world as it stands and then writes over it, once, so the
/// engine's *next* read is the first one that can see it. A racing thread would make this arm
/// flaky and would measure the scheduler.
#[test]
fn a_write_between_the_two_reads_stops_the_compensation_and_survives() {
    let server = ScriptedServer::start("r30_window", Grammar::Absolute, &INITIAL);

    let (status, body, prior, after) = block_on(async {
        let id = server.commit_goal(GOAL).await;
        let prior = server.world();
        server.adapter.script(&MOVED_THEN_FAILED);
        server
            .adapter
            .third_party_writes_after_apply(THE_FAILING_APPLY, THEIRS);
        let (status, body) = server.undo(&id).await;
        (status, body, prior, server.world())
    });

    let applies = server.adapter.applies();
    let sent_a_compensation = applies > 2;

    record(&format!(
        "R30_WINDOW status={status} gx_code={} rollback={} because={} prior={prior:?} \
         theirs={THEIRS:?} after={after:?} their_write_survives={} \
         compensating_apply_was_sent={sent_a_compensation} applies={applies} \
         third_party_at_read={}",
        body["gx_code"].as_str().unwrap_or("<absent>"),
        rollback_word(&body),
        because_word(&body),
        after == THEIRS,
        server.adapter.third_party_fired_at_read(),
    ));
    traces("WINDOW", &server);

    assert_eq!(status, 422, "the undo's own apply failed: {body}");
    let account = server.adapter.trace();
    assert_eq!(
        account.len(),
        2,
        "the fixture received the commit's apply and the undo's, and nothing else: {account:?}"
    );
    assert!(
        account[1].contains("performed=1"),
        "🔴 this arm's premise is the opposite of arm 2's: the forward apply **did** move the \
         world before it failed. The fixture's account of that call: {account:?}"
    );
    assert!(
        server.adapter.third_party_fired_at_read() > 0,
        "🔴 and that somebody else wrote. The hook never fired: {:?}",
        server.adapter.read_trace()
    );

    assert_eq!(
        after, THEIRS,
        "🔴 the third party's bytes must still be on the object. It began at {prior:?}, the \
         forward apply moved it, {THEIRS:?} landed between the engine's two reads, and the object \
         now holds {after:?}. An absolute inverse sent from here would have restored {prior:?} and \
         taken their work with it"
    );
    assert!(
        !sent_a_compensation,
        "🔴 and no compensating apply may have been sent — read off the fixture's own counter, \
         which says {applies}. Trace: {account:?}"
    );
    assert_eq!(
        rollback_word(&body),
        "NotAttempted",
        "the compensation was refused rather than attempted: {body}"
    );
    assert_eq!(
        because_word(&body),
        "WorldMovedBeneath",
        "🔴 and the cause is the second read disagreeing with the first, which is what \
         `WorldMovedBeneath` names. Not `WorldNeverMoved` — the apply did move the object, and \
         saying it did not would tell an operator this call was inert when it was not — and not \
         `WorldCouldNotBeRead`, because both reads answered: {body}"
    );
}

// ---------------------------------------------------------------------------
// 4. The negative control
// ---------------------------------------------------------------------------

/// 🔴 **The negative control, and it is the load-bearing arm of this file** (`req/372` M-01,
/// `req/38` §240 ruling 2).
///
/// Arm 3's road with the third party taken out and **nothing else changed**: the same grammar, the
/// same script, the same fixture, the same forward apply moving the world and then failing. The
/// compensation must still **run**, the object must come home, and the word must be `Succeeded`.
///
/// Without this arm the repair could be a blanket refusal — an engine that never sent a
/// compensation again would pass the bed, arm 2 and arm 3, and would have silently deleted the road
/// 43 T-10c exists for. That is not hypothetical: an earlier draft of this repair guarded on `fp0`
/// alone, could not tell *our own apply landed* from *somebody else wrote*, and stopped
/// compensating the ordinary case. The engine's own comment at the T-10c arm records it, and
/// `r29_rollback_is_verified.rs`'s negative control caught it in one run.
///
/// Three things are asserted: the compensating apply **was** sent (the fixture's counter reaches
/// three), the object is back at the bytes it started from (read off the world file), and the word
/// is `Succeeded` with no cause beside it.
#[test]
fn with_no_third_party_the_compensation_still_runs_and_the_object_comes_home() {
    let server = ScriptedServer::start("r30_control", Grammar::Absolute, &INITIAL);

    let (status, body, prior, after) = block_on(async {
        let id = server.commit_goal(GOAL).await;
        let prior = server.world();
        server.adapter.script(&MOVED_THEN_FAILED);
        // No `third_party_writes_after_apply`: this is the only difference from arm 3.
        let (status, body) = server.undo(&id).await;
        (status, body, prior, server.world())
    });

    let applies = server.adapter.applies();

    record(&format!(
        "R30_CONTROL status={status} gx_code={} rollback={} because={} prior={prior:?} \
         after={after:?} came_home={} compensating_apply_was_sent={} applies={applies} \
         third_party_at_read={}",
        body["gx_code"].as_str().unwrap_or("<absent>"),
        rollback_word(&body),
        because_word(&body),
        after == prior,
        applies > 2,
        server.adapter.third_party_fired_at_read(),
    ));
    traces("CONTROL", &server);

    assert_eq!(status, 422, "the undo's own apply failed: {body}");
    assert_eq!(
        server.adapter.third_party_fired_at_read(),
        0,
        "the premise of a control is that the thing being controlled for is absent: {:?}",
        server.adapter.read_trace()
    );
    let account = server.adapter.trace();
    assert!(
        account.len() >= 2 && account[1].contains("performed=1"),
        "the same forward apply as arm 3 — it moves the world and then fails: {account:?}"
    );

    assert_eq!(
        applies, 3,
        "🔴 **the repair must not be a blanket refusal.** Three applies: the commit's, the undo's \
         half-failing one, and 43 T-10c's compensation. The fixture received {applies}, so on the \
         ordinary road — a call that landed and then errored, with nobody else in the world — this \
         build stopped compensating. Every other arm in this file would still be green over that. \
         Trace: {account:?}"
    );
    assert_eq!(
        after, prior,
        "🔴 and the compensation has to have **worked**: the object is read straight off the world \
         file and must be back at the bytes the transformation started from. It began at {prior:?} \
         and holds {after:?}"
    );
    assert_eq!(
        rollback_word(&body),
        "Succeeded",
        "🔴 a true `Succeeded` is still `Succeeded`. The object is measurably home ({prior:?}), so \
         a build answering `NotAttempted` here has replaced a wrong word with a word that is wrong \
         somewhere else — and somewhere worse, because this road is the one the whole mechanism \
         exists for: {body}"
    );
    assert_eq!(
        because_word(&body),
        "<null>",
        "and a compensation that ran carries no cause for not having been attempted. `req/334` \
         M-01 writes both members together, so the honest answer here is an explicit null rather \
         than an absent member: {body}"
    );
}

// ---------------------------------------------------------------------------
// 5. The derivation
// ---------------------------------------------------------------------------

/// `crates/gx-engine/src/pipeline.rs`, as text.
///
/// 🔴 **The shipped source, and not a copy of it.** The path is resolved from `CARGO_MANIFEST_DIR`
/// to the sibling crate this binary links against, so what is scanned is the same file the
/// behavioural arms above just executed. A derivation taken from a fixture, a vendored snapshot or
/// a pasted excerpt would pass while the shipped engine did anything at all — the scan would be
/// measuring the copy, and the copy cannot fail. The resolved path is printed for that reason.
fn pipeline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("gx-cli sits in crates/")
        .join("gx-engine")
        .join("src")
        .join("pipeline.rs")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The lines of `text` that are not whole-line comments, blanked in place so that indices stay line
/// numbers.
///
/// 🔴 A scan that reads doc comments is a scan a **sentence** can satisfy, and this repair is
/// explained at length in prose directly above the code it is made of — an instrument that counted
/// those explanations would report a repair that had been described and not made.
fn code_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| {
            if line.trim_start().starts_with("//") {
                String::new()
            } else {
                line.to_string()
            }
        })
        .collect()
}

const ROLLBACK_CALL: &str = "apply_once(adapter.as_ref(), inverse)";
const FORWARD_CALL: &str = "apply_once(adapter.as_ref(), &delta)";
const FIRST_READ: &str = "world_the_failed_apply_left(";
const SECOND_READ: &str = "world_is_still_at(";

/// Where the four lines of the T-10c arm this repair is made of sit, by index.
#[derive(Debug)]
struct Derivation {
    forward_at: usize,
    first_read_at: usize,
    second_read_at: usize,
    rollback_at: usize,
}

/// 🔴 The predicate: are **both** reads inside T-10c's arm and **in front of** the compensating
/// apply, in that order?
///
/// The region is bounded below by the forward apply whose failure opens the arm and above by the
/// compensating apply itself, so a read found anywhere else in the file — including the two
/// functions' own declarations, which sit a thousand lines further down — does not satisfy it. The
/// function is written over `&[String]` rather than reading the file itself precisely so that the
/// controls below can hand it a mutilated copy and watch it refuse.
fn derive(lines: &[String]) -> Option<Derivation> {
    let rollback_sites: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains(ROLLBACK_CALL))
        .map(|(i, _)| i)
        .collect();
    if rollback_sites.len() != 1 {
        return None;
    }
    let rollback_at = rollback_sites[0];
    let forward_at = lines
        .iter()
        .enumerate()
        .take(rollback_at)
        .filter(|(_, l)| l.contains(FORWARD_CALL))
        .map(|(i, _)| i)
        .next_back()?;
    let first_read_at = (forward_at..rollback_at).find(|i| lines[*i].contains(FIRST_READ))?;
    let second_read_at = (first_read_at..rollback_at).find(|i| lines[*i].contains(SECOND_READ))?;
    Some(Derivation {
        forward_at,
        first_read_at,
        second_read_at,
        rollback_at,
    })
}

/// The same lines with the two reads moved to **after** the compensating apply.
///
/// Every line the shipped file has is still present and every spelling the predicate looks for is
/// still spelled; only the order is different. A predicate that passed this would be a `contains`
/// dressed up as a derivation.
fn reads_moved_after_the_apply(lines: &[String], at: &Derivation) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if i == at.first_read_at || i == at.second_read_at {
            continue;
        }
        out.push(line.clone());
        if i == at.rollback_at {
            out.push(lines[at.first_read_at].clone());
            out.push(lines[at.second_read_at].clone());
        }
    }
    out
}

/// 🔴 **The derivation** — the compare-and-set is in the source, in front of the compensating
/// apply, and a scan that could not tell it apart from its absence is caught by two controls
/// (`req/372` M-01, `req/38` §240 ruling 2).
///
/// A behavioural probe can be satisfied by a coincidence: an engine that refused every compensation
/// would pass arms 2 and 3, and arm 4 is what rules that out. What none of them can see is *how*
/// the answer is reached, and the ruling is about a structural asymmetry — the forward apply has
/// had a second read of the same fingerprint in front of it since R8 / `req/234` H-04, and the
/// roll-back's apply had nothing. So the source is measured directly.
///
/// Three controls, because one is not enough:
///
/// * **the forward CAS is still there** (r29's control): if the scan cannot find a thing that has
///   been in this file since R8, the instrument is broken rather than the repair missing;
/// * **an empty derivation fails**: a predicate satisfied by nothing is satisfied by everything;
/// * 🔴 **a reordered derivation fails**: the same file with the two reads moved to after the
///   compensating apply, which contains every spelling the predicate looks for and is exactly the
///   defect. Order is the whole repair, and a scan that cannot see order is not measuring it.
#[test]
fn the_two_reads_are_in_the_source_in_front_of_the_compensating_apply() {
    let path = pipeline_path();
    let source = read(&path);
    let lines = code_lines(&source);

    let shipped = derive(&lines);
    let empty = derive(&[]);
    let reordered = shipped
        .as_ref()
        .map(|at| reads_moved_after_the_apply(&lines, at))
        .map(|copy| derive(&copy).is_some());

    // r29's control, kept: the forward apply's own CAS (R8 / `req/234` H-04) in the sixty lines
    // above it. It has been in this file since R8, so a scan that cannot find it is broken.
    let forward_cas = shipped.as_ref().map(|at| {
        let before = &lines[at.forward_at.saturating_sub(60)..at.forward_at];
        (
            before.iter().any(|l| l.contains("cas_eq(")),
            before.iter().any(|l| l.contains(".precondition(")),
        )
    });

    record(&format!(
        "R30_DERIVE source={} lines={} shipped={:?} empty_derivation_passes={} \
         reordered_derivation_passes={:?} forward_cas={:?}",
        path.display(),
        lines.len(),
        shipped.as_ref().map(|at| (
            at.forward_at + 1,
            at.first_read_at + 1,
            at.second_read_at + 1,
            at.rollback_at + 1
        )),
        empty.is_some(),
        reordered,
        forward_cas,
    ));
    if let Some(at) = &shipped {
        for i in [at.first_read_at, at.second_read_at, at.rollback_at] {
            record(&format!(
                "R30_DERIVE_LINE {} {}",
                i + 1,
                lines[i].trim_end()
            ));
        }
    }

    assert!(
        path.ends_with("gx-engine/src/pipeline.rs")
            || path.ends_with("gx-engine\\src\\pipeline.rs"),
        "🔴 the derivation must be taken from the shipped engine and not from a copy. Resolved to \
         {}",
        path.display()
    );
    let at = shipped.expect(
        "🔴 43 T-10c's compensating apply must have both of R30's reads in front of it, inside the \
         arm the failed forward apply opens. The scan did not find that shape, which is the \
         unguarded write `req/372` M-01 is about — an absolute inverse handed to the adapter with \
         no compare-and-set between the failure and the send",
    );
    assert!(
        at.forward_at < at.first_read_at
            && at.first_read_at < at.second_read_at
            && at.second_read_at < at.rollback_at,
        "🔴 and the order is the repair: the forward apply, then the read that asks what it left \
         behind, then the read that asks whether the world is still there, then the compensation. \
         Measured: {at:?}"
    );
    assert!(
        empty.is_none(),
        "🔴 the control failed: an empty derivation satisfied the predicate, so the predicate is \
         satisfied by anything and the assertion above measures nothing"
    );
    assert_eq!(
        reordered,
        Some(false),
        "🔴 the load-bearing control. The same file with the two reads moved to **after** the \
         compensating apply still contains every spelling this scan looks for, and it is exactly \
         the defect. A predicate that passes it is a `contains` wearing a derivation's clothes"
    );
    assert_eq!(
        forward_cas,
        Some((true, true)),
        "🔴 r29's control, kept: the forward apply's own CAS (R8 / `req/234` H-04) has been in \
         front of {FORWARD_CALL:?} since R8. If this scan cannot find it, the instrument is broken \
         rather than the repair missing"
    );
}

// ---------------------------------------------------------------------------
// 6. The vocabulary
// ---------------------------------------------------------------------------

/// One clause from the proxy, for the `NotAttempted` value and a given cause.
fn clause(because: &str) -> String {
    apply_failed_clause(&json!({
        "reason": "ApplyFailed",
        "detail": "NotAttempted",
        "not_attempted_because": because,
    }))
}

/// 🔴 **The vocabulary is whole** (`req/372` M-01, `req/324` §5(d), `req/38` §240 ruling 2).
///
/// This lane took `NotAttemptedBecause` from three causes to six, and a cause without an arm in
/// `crates/gx-cli/src/wrap.rs` falls into the arm written for *a cause this build does not know* —
/// where the agent is told this build has not been taught the value it is carrying, which would be
/// false and would be about the very words this lane added. Four things are asked and each fails
/// differently:
///
/// * `NotAttemptedBecause::ALL_CAUSES` declares **six**, and the three this lane added are among
///   them by name. A count is the cheapest way for a seventh added without a declaration to be
///   caught.
///
/// 🔴 **The count moved, 6 → 7, because a ruling moved it** — **R-1001-1** (`req/1001` §4, the
/// else-arm of D-999-F2, 2026-08-31) added `PromisedPostStateWasWrong`, *with* a declaration and
/// an arm, which is exactly the addition the count exists to police rather than to forbid. The
/// name of this test is a historical record and stays (the same discipline
/// `crates/gx-core/src/error.rs`'s six-named-seven test records for `AbortReason`); the paragraph
/// above is kept as R30's own account of its window.
/// * every declared cause has an explicit `Some("<cause>")` arm in `wrap.rs`.
/// * every declared cause produces a **distinct** sentence. Two causes sharing a sentence is the
///   `req/324` §5(d) defect with the arms present and the words wrong.
/// * a cause this build does not know produces neither of the six, so the fallback arm is reachable
///   and is not one of the six by accident.
#[test]
fn the_six_causes_each_have_an_arm_and_a_sentence_of_their_own() {
    let causes = gx_engine::NotAttemptedBecause::ALL_CAUSES;

    let wrap = read(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("wrap.rs"),
    );
    let wrap_code = code_lines(&wrap).join("\n");
    let armed: Vec<&str> = causes
        .iter()
        .copied()
        .filter(|cause| wrap_code.contains(&format!("Some(\"{cause}\")")))
        .collect();

    let said: Vec<String> = causes.iter().map(|cause| clause(cause)).collect();
    let unknown = clause("SomethingR30HasNotTaughtThisBuild");
    let mut distinct = said.clone();
    distinct.sort();
    distinct.dedup();

    record(&format!(
        "R30_VOCAB causes={} declared={causes:?} armed={} distinct_sentences={} \
         unknown_matches_a_known={}",
        causes.len(),
        armed.len(),
        distinct.len(),
        said.contains(&unknown),
    ));
    for (cause, sentence) in causes.iter().zip(said.iter()) {
        record(&format!(
            "R30_VOCAB_ARM {cause} armed={} chars={}",
            armed.contains(cause),
            sentence.len()
        ));
    }

    assert_eq!(
        causes.len(),
        // 🔴 6 until R-1001-1 (`req/1001` §4, D-999-F2's else-arm, 2026-08-31): a count in a test
        // may move only because a ruling moved it, and that ruling did.
        7,
        "🔴 three causes until this lane, three more on which a compensation is **refused rather \
         than skipped**, and a seventh — `PromisedPostStateWasWrong`, R-1001-1 — on which the \
         inverse exists and the model it stands on is distrusted. Declared: {causes:?}"
    );
    for expected in [
        "WorldNeverMoved",
        "WorldMovedBeneath",
        "WorldCouldNotBeRead",
    ] {
        assert!(
            causes.contains(&expected),
            "🔴 `{expected}` is one of the three roads arms 2 and 3 drive, and it has no \
             declaration. Declared: {causes:?}"
        );
    }
    assert_eq!(
        armed.len(),
        causes.len(),
        "🔴 one arm per cause. A cause the engine declares and the proxy has no arm for falls into \
         the arm for a cause this build does not know, and the agent is told this build has not \
         been taught the value it is carrying — false, and about the words this lane added. Armed: \
         {armed:?}, declared: {causes:?}"
    );
    assert_eq!(
        distinct.len(),
        causes.len(),
        "🔴 and the sentences must be as many as the causes (six when R30 wrote this; seven since \
         R-1001-1). Two causes sharing a sentence is `req/324` §5(d) \
         with the arms present and the words wrong: a reader on one road is handed a confident \
         account of another"
    );
    assert!(
        !said.contains(&unknown),
        "🔴 the fallback arm has to be reachable and separate. A cause this build does not know \
         produced the same sentence as a cause it does, so one of the six is being served by the \
         arm that declines to guess — or the arm that declines to guess is being served by one of \
         the six"
    );
}
