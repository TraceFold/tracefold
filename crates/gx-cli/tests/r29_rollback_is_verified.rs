// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R29 / `req/361` H-01 (`req/38` §238 ruling 1)** — a roll-back that is **reported** is not a
//! roll-back that **happened**, and this file is the gate that keeps the two apart.
//!
//! # The defect, as the twenty-eighth adversarial audit produced it
//!
//! 43 T-10c sends the escrowed inverse back to the substrate when a commit's `apply` fails.
//! `crates/gx-engine/src/pipeline.rs` used to decide what became of the **world** from what the
//! **call** answered, in three lines:
//!
//! ```ignore
//! match self.apply_once(adapter.as_ref(), inverse) {
//!     Ok(_) => Rollback::Succeeded,
//!     Err(_) => Rollback::Failed,
//! }
//! ```
//!
//! The object was never read again. The audit drove a contract-conforming adapter whose `apply`
//! performs the first `k` operations of a delta and then answers `Err` — a shape
//! `crates/gx-substrate/src/error.rs` permits in as many words, its `ApplyFailed` documentation
//! saying that a non-atomic apply can fail halfway — over a **relative** delta grammar, and read
//! this off the wire:
//!
//! ```text
//! prior = "A\nB\nC\nD\n"
//! after = "A\nB\nD\nC\nD\n"          <- a record duplicated; NOT restored
//! HTTP  = 422 {"gx_code":"APPLY_FAILED","rollback":"Succeeded", ...}
//! ```
//!
//! The adapter had not lied. The forward apply stopped at `A B D`, the escrowed inverse *add C, add
//! D* was then applied completely and honestly, and the world came to rest one record further from
//! home than it started. `Succeeded` was a word about a call and it was being read as a word about
//! the world.
//!
//! # What this file asserts, and the order the six probes are in
//!
//! The order is the order a reader has to take them in for the third one to mean anything:
//!
//! 1. **the bed** — an undriven road commits and undoes cleanly and the world really comes home.
//!    Without it, everything below could be green because the harness is broken.
//! 2. **the negative control** — the same failing script over an **absolute** grammar leaves the
//!    world where it started, and the wire still says `Succeeded`. The repair did not blanket-
//!    replace a word: a true `Succeeded` is still `Succeeded`.
//! 3. **the finding** — the audit's road, verbatim. The world is genuinely destroyed and the
//!    delivered body does not say `Succeeded`.
//! 4. **the derivation** — the source itself, so that the behaviour above cannot be produced by
//!    anything other than a re-read, with a control that an empty derivation cannot pass.
//! 5. **the read that will not answer** — a compensation whose bytes landed and whose read-back
//!    died is `Failed`, which is fail-safe, and never `Succeeded`.
//! 6. **the vocabulary** — four words, each round-tripping, each with an arm on the proxy, so that
//!    a fifth word cannot fall silently into the arm for a word this build does not know.
//!
//! # What this file deliberately does **not** claim
//!
//! It does not claim the roll-back caused the divergence it observes. One fingerprint taken after
//! the call cannot tell *the compensation overshot* from *somebody else wrote in the window*, and
//! `docs/LIMITS.md` declares that residue in the form R8 used for the forward CAS. What is closed
//! is narrower and is the whole finding: the terminal state a reader is handed can no longer be
//! `Succeeded` over an object that is demonstrably not where it started.

use std::path::{Path, PathBuf};

#[path = "support/scripted_substrate.rs"]
mod scripted_substrate;

use scripted_substrate::{block_on, record, Behaviour, Grammar, ScriptedServer};

/// The world every probe starts from, and the two records the commit adds to it.
///
/// Four records because the audit used four, and the shape of the corruption — one record removed
/// by half of a two-operation inverse, then two records appended by the compensation — needs a
/// world with something in front of the part that moves. A one-record world would corrupt too, and
/// would corrupt in a way a reader could mistake for a truncation.
const INITIAL: [&str; 2] = ["A", "B"];

/// The goal the commit is given: the initial world plus two more records.
const GOAL: &str = "A\nB\nC\nD\n";

/// The world the audit's road came to rest at, verbatim from `req/361` H-01.
///
/// Held as a constant rather than computed, so that a change to the fixture's grammar that made the
/// corruption *different* fails here instead of quietly gating a road nobody has measured.
const CORRUPTED: &str = "A\nB\nD\nC\nD\n";

/// The script the audit drove: the first apply is the commit's and lands, the second is the undo's
/// forward apply and performs one of its two operations before failing, and the third is 43
/// T-10c's compensation.
const HALF_FAILING: [Behaviour; 2] = [Behaviour::Full, Behaviour::HalfThenFail(1)];

/// Print the adapter's own account of every `apply`, tagged.
///
/// The trace is the fixture's, not this file's opinion of the fixture: each line was written inside
/// `apply` at the moment it ran, before anything downstream could have an opinion about it.
fn trace(tag: &str, server: &ScriptedServer) {
    for line in server.adapter.trace() {
        record(&format!("R29_TRACE_{tag} {line}"));
    }
}

/// The `rollback` member of a refusal, as a plain string, or `<absent>`.
///
/// 44 §2.3's problem object carries the roll-back as an **extension member** (`req/334` M-01), and
/// `null` there is an honest answer rather than a missing one — so the three cases are told apart
/// rather than folded into an `unwrap_or_default`.
fn rollback_word(body: &serde_json::Value) -> String {
    match body.get("rollback") {
        None => "<absent>".to_string(),
        Some(serde_json::Value::Null) => "<null>".to_string(),
        Some(value) => value.as_str().unwrap_or("<not a string>").to_string(),
    }
}

// ---------------------------------------------------------------------------
// 1. The bed
// ---------------------------------------------------------------------------

/// 🔴 The **bed control** for everything below (`req/361` H-01, `req/38` §238 ruling 1).
///
/// An undriven road — the same fixture, the same grammar, the same six requests, and no failure
/// scripted anywhere — commits a change and then undoes it, and the object really is back at the
/// bytes it started from. Every probe below reads a refusal, and a harness that refused for a
/// reason of its own (a gate that denied, an archive that held no receipt, a locator the pack
/// forbids) would make all of them green while measuring nothing. This is the arm that fails first
/// when that happens, and it is the reason the arms below are allowed to be short.
#[test]
fn an_undriven_road_commits_and_undoes_and_the_world_comes_home() {
    let server = ScriptedServer::start("bed", Grammar::Relative, &INITIAL);
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
        "R29_BED status={status} rollback={} prior={prior:?} after={after:?} same={} applies={}",
        rollback_word(&body),
        u8::from(after == prior),
        server.adapter.applies(),
    ));
    trace("BED", &server);

    assert_eq!(
        status, 200,
        "the bed control's undo must succeed. Every probe below reads a refusal, so a harness that \
         refuses on its own account would make all of them green while measuring nothing: {body}"
    );
    assert_eq!(
        after, prior,
        "the bed control's undo must put the object back. If this fails, the fixture cannot tell a \
         restored world from a destroyed one and the finding below is unmeasurable"
    );
    assert_eq!(
        server.adapter.applies(),
        2,
        "two applies and no more: the commit's and the undo's. A third would mean 43 T-10c fired \
         on a road nothing failed on"
    );
}

// ---------------------------------------------------------------------------
// 2. The negative control
// ---------------------------------------------------------------------------

/// 🔴 The **negative control**: a true `Succeeded` is still `Succeeded` (`req/361` H-01, `req/38`
/// §238 ruling 1).
///
/// The same script as the finding below — the undo's forward apply performs one operation and then
/// answers `Err` — over an **absolute** delta grammar, where the escrowed inverse says *the
/// object's whole content is this* and therefore restores it from wherever the failed apply left
/// it. The object ends where it started, and the wire says `Succeeded`.
///
/// This is what stops the repair from being a blanket rename. A change that had simply replaced the
/// word `Succeeded` with `Diverged` at 43 T-10c would pass the finding below and fail here, and a
/// change that had made the roll-back always report failure would fail here too. The grammar is the
/// only thing that differs between this probe and the next one.
#[test]
fn an_absolute_grammar_restores_the_world_and_succeeded_is_still_succeeded() {
    let server = ScriptedServer::start("absolute", Grammar::Absolute, &INITIAL);

    let (status, body, prior, after) = block_on(async {
        let id = server.commit_goal(GOAL).await;
        let prior = server.world();
        server.adapter.script(&HALF_FAILING);
        let (status, body) = server.undo(&id).await;
        (status, body, prior, server.world())
    });

    record(&format!(
        "R29_ABSOLUTE status={status} gx_code={} rollback={} prior={prior:?} after={after:?} \
         same={} applies={}",
        body["gx_code"].as_str().unwrap_or("<absent>"),
        rollback_word(&body),
        u8::from(after == prior),
        server.adapter.applies(),
    ));
    trace("ABSOLUTE", &server);

    assert_eq!(
        status, 422,
        "the undo's own apply failed, so 44 §2.3's `APPLY_FAILED` row and its status: {body}"
    );
    assert_eq!(
        body["gx_code"].as_str(),
        Some("APPLY_FAILED"),
        "the same code the finding below reads, so that the two probes differ in the roll-back \
         word alone: {body}"
    );
    assert_eq!(
        after, prior,
        "the negative control's premise: an absolute inverse names the content the object holds \
         and therefore restores it from wherever the half-failed apply left it. If this is not \
         true, this probe is not a control for anything"
    );
    assert_eq!(
        rollback_word(&body),
        "Succeeded",
        "🔴 the repair must not be a blanket rename. The object is measurably back at the bytes it \
         started from ({prior:?}), so `Succeeded` is the true word here and this build still says \
         it. A build that answered anything else would have replaced a wrong word with a word that \
         is wrong somewhere else: {body}"
    );
}

// ---------------------------------------------------------------------------
// 3. The finding
// ---------------------------------------------------------------------------

/// 🔴 **The finding, repaired** — `req/361` H-01's road, verbatim (`req/38` §238 ruling 1).
///
/// A **relative** grammar and the same script. The undo's forward apply *remove C, remove D*
/// performs the first of its two operations and answers `Err`, leaving `A B D`; 43 T-10c then sends
/// the escrowed inverse *add C, add D*, which the adapter performs **completely and honestly**; and
/// the object comes to rest at `A B D C D` — a record duplicated, further from home than when the
/// compensation began.
///
/// Two things are asserted and both are load-bearing. First, that the corruption is **real**: the
/// bed still reproduces the broken world, byte for byte, so this probe cannot pass by having
/// stopped reproducing anything. Second, that the delivered body does **not** say `Succeeded` and
/// **does** say `Diverged` — the word `req/361` §9-2 asked for, because the three worlds a reader
/// has to tell apart after a failed apply are three (back where it was, moved because the roll-back
/// was refused, moved because the roll-back itself moved it) and the vocabulary had two.
#[test]
fn a_relative_grammar_destroys_the_world_and_the_answer_no_longer_says_succeeded() {
    let server = ScriptedServer::start("relative", Grammar::Relative, &INITIAL);

    let (status, body, prior, after) = block_on(async {
        let id = server.commit_goal(GOAL).await;
        let prior = server.world();
        server.adapter.script(&HALF_FAILING);
        let (status, body) = server.undo(&id).await;
        (status, body, prior, server.world())
    });

    record(&format!(
        "R29_RELATIVE status={status} gx_code={} rollback={} prior={prior:?} after={after:?} \
         corrupted={} applies={}",
        body["gx_code"].as_str().unwrap_or("<absent>"),
        rollback_word(&body),
        u8::from(after != prior),
        server.adapter.applies(),
    ));
    trace("RELATIVE", &server);

    assert_eq!(
        status, 422,
        "the undo's own apply failed, so 44 §2.3's `APPLY_FAILED` row and its status: {body}"
    );
    assert_eq!(
        body["gx_code"].as_str(),
        Some("APPLY_FAILED"),
        "the same code the negative control reads: {body}"
    );

    // The corruption first, because a claim about the *word* over a world that is fine is a claim
    // about nothing. The bed has to still be able to break before the answer is worth reading.
    assert_ne!(
        after, prior,
        "🔴 this probe cannot pass by having stopped reproducing the defect. The object must be \
         measurably NOT back where it started; it is at {after:?} and it began at {prior:?}"
    );
    assert_eq!(
        after, CORRUPTED,
        "🔴 and it must be broken the way `req/361` H-01 measured it: prior {prior:?}, the forward \
         apply half-done, the escrowed inverse applied in full, and a record duplicated. Measured \
         here as {after:?}. A different corruption is still a corruption, but it is no longer the \
         road the ruling was made on and this file would be gating something else"
    );
    assert_eq!(
        server.adapter.applies(),
        3,
        "three applies: the commit's, the undo's half-failing one, and 43 T-10c's compensation. \
         Fewer would mean the roll-back never fired and the word below would be about nothing"
    );

    // Then the word.
    assert_ne!(
        rollback_word(&body),
        "Succeeded",
        "🔴 THE FINDING. The object was destroyed and this answer called it a success. The world \
         began at {prior:?} and is now at {after:?} — a record duplicated, further from home than \
         when the compensation started — and the body handed to the caller reports the roll-back \
         as `Succeeded`. An agent reading this body is told the change was taken back, and it was \
         not: the compensation ran, and running is not working. Body: {body}"
    );
    assert_eq!(
        rollback_word(&body),
        "Diverged",
        "🔴 and the word for this world is `Diverged` — the adapter accepted the inverse and the \
         object is not back where it started. Not `Failed`, which would tell an operator the \
         compensation did not run when it ran completely; not `Succeeded`, which is the defect. \
         `req/361` §9-2: the world can be three ways and the vocabulary had two words for it. \
         Body: {body}"
    );
}

// ---------------------------------------------------------------------------
// 4. The derivation
// ---------------------------------------------------------------------------

/// `crates/gx-engine/src/pipeline.rs`, as text.
fn pipeline_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("gx-cli sits in crates/")
        .join("gx-engine")
        .join("src")
        .join("pipeline.rs");
    read(&path)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The lines of `text` that are not whole-line comments.
///
/// 🔴 A scan that reads doc comments is a scan a **sentence** can satisfy. Both files this probe
/// and the vocabulary probe below read explain the very spellings being looked for, at length, in
/// prose directly above the code — so an instrument that counted those explanations would report a
/// repair that had been described and not made. Stripping whole-line comments is the smallest thing
/// that closes it; a trailing comment on a line of code leaves the code on that line, which is what
/// is being looked for anyway.
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

/// The body of the function whose signature opens at `from`, by brace depth.
fn body_from(lines: &[String], from: usize) -> Vec<String> {
    let mut depth = 0i32;
    let mut opened = false;
    let mut out = Vec::new();
    for line in lines.iter().skip(from) {
        out.push(line.clone());
        for ch in line.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        if opened && depth == 0 {
            break;
        }
    }
    out
}

/// 🔴 **The derivation** — the behaviour above is produced by a re-read of the object, and the
/// source says so (`req/361` H-01, `req/38` §238 ruling 1).
///
/// A behavioural probe can be satisfied by a coincidence: an engine that answered `Diverged` for
/// every failed apply would pass the finding, and the negative control is what rules that out. What
/// neither of them can see is *how* the answer is reached, and the whole ruling is about a
/// structural asymmetry — the **forward** apply has had a second read of the same fingerprint in
/// front of it since R8 / `req/234` H-04, and the roll-back's apply had nothing at all. So the
/// source is measured directly: the window at 43 T-10c's `apply_once` reaches
/// `Engine::rollback_landed`, and that function's own body reads the object (`.precondition(`) and
/// compares it (`cas_eq(`).
///
/// **The control is the forward apply's CAS.** Without it an empty derivation passes: a scan that
/// found nothing anywhere would satisfy no assertion and a scan whose predicate was broken would
/// satisfy none either, and the two are indistinguishable. The forward CAS has been in this file
/// since R8, so it must be found; if it is not, the instrument is broken rather than the repair.
#[test]
fn the_roll_backs_apply_is_guarded_by_a_re_read_and_the_forward_cas_is_still_there() {
    let source = pipeline_source();
    let lines = code_lines(&source);

    const ROLLBACK_CALL: &str = "apply_once(adapter.as_ref(), inverse)";
    const FORWARD_CALL: &str = "apply_once(adapter.as_ref(), &delta)";

    let rollback_sites: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains(ROLLBACK_CALL))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        rollback_sites.len(),
        1,
        "43 T-10c hands the escrowed inverse to the one apply road at exactly one place \
         (req/78 §3.3 Rule 2). Found {} sites for {ROLLBACK_CALL:?}, so this probe no longer knows \
         which one it is measuring",
        rollback_sites.len()
    );
    let rollback_at = rollback_sites[0];

    // The forward apply this arm controls for is the one whose failure opens T-10c — the last
    // forward call site above it. (`Engine::resume` has a second one further down the file; it is a
    // different transition and not what T-10c's asymmetry was about.)
    let forward_at = lines
        .iter()
        .enumerate()
        .take(rollback_at)
        .filter(|(_, l)| l.contains(FORWARD_CALL))
        .map(|(i, _)| i)
        .next_back()
        .expect("the forward apply of `Engine::commit` sits above 43 T-10c's arm");

    // (a) the roll-back's apply reaches a re-read.
    let window: Vec<String> = lines.iter().skip(rollback_at).take(6).cloned().collect();
    let reread_within = window
        .iter()
        .position(|l| l.contains("rollback_landed("))
        .map(|at| at as i64)
        .unwrap_or(-1);

    // (b) the control: the forward apply's CAS is still in front of it.
    let before: Vec<String> = lines[forward_at.saturating_sub(60)..forward_at].to_vec();
    let cas_gap = before
        .iter()
        .rposition(|l| l.contains("cas_eq("))
        .map_or(-1, |at| (before.len() - at) as i64);
    let precondition_gap = before
        .iter()
        .rposition(|l| l.contains(".precondition("))
        .map_or(-1, |at| (before.len() - at) as i64);

    // (c) the re-read's own body. Absence is measured rather than panicked on: a build without the
    // repair has no such function, and this probe is more use to the person reading that build's
    // output if it prints its numbers and then fails an assertion than if it dies at an `expect`
    // before the measurement line is written.
    let landed_at = lines.iter().position(|l| l.contains("fn rollback_landed("));
    let body = landed_at
        .map(|at| body_from(&lines, at))
        .unwrap_or_default();
    let body_text = body.join("\n");

    record(&format!(
        "R29_DERIVE rollback_call_line={} reread_within={reread_within} forward_call_line={} \
         forward_cas_gap={cas_gap} forward_precondition_gap={precondition_gap} \
         rollback_landed_line={} rollback_landed_body_lines={} body_precondition={} body_cas_eq={}",
        rollback_at + 1,
        forward_at + 1,
        landed_at.map_or(-1, |at| at as i64 + 1),
        body.len(),
        u8::from(body_text.contains(".precondition(")),
        u8::from(body_text.contains("cas_eq(")),
    ));
    for line in &window {
        record(&format!("R29_DERIVE_WINDOW {}", line.trim_end()));
    }

    assert!(
        reread_within >= 0,
        "🔴 43 T-10c's roll-back must not decide what became of the world from what the call \
         answered. The window at the escrowed inverse's `apply_once` has to reach a re-read of the \
         object, and `rollback_landed(` is not in it. This is the three-line shape `req/361` H-01 \
         was measured on:\n{}",
        window.join("\n")
    );
    assert!(
        cas_gap >= 0 && precondition_gap >= 0,
        "🔴 the control failed, which means this instrument is broken rather than the repair \
         missing: the forward apply's own CAS (R8 / `req/234` H-04) has been in front of \
         {FORWARD_CALL:?} since R8, and the scan cannot find it. An empty derivation must not be \
         able to pass this probe. cas_gap={cas_gap} precondition_gap={precondition_gap}"
    );
    assert!(
        landed_at.is_some(),
        "🔴 the re-read the window above reaches has to exist: `fn rollback_landed(` is not \
         declared in this file at all, so 43 T-10c's roll-back has nothing in front of it and the \
         word it reports is a word about a call"
    );
    assert!(
        body_text.contains(".precondition("),
        "🔴 `rollback_landed` has to read the object, not consult a value the engine already \
         holds. Its body:\n{body_text}"
    );
    assert!(
        body_text.contains("cas_eq("),
        "🔴 and it has to compare that read against the fingerprint the transformation started \
         from, through 42 §3.5's own comparison rather than through `==` — `cas_eq`'s third answer \
         (the two are not comparable) is a distinct world and folding it into equality would lose \
         it. Its body:\n{body_text}"
    );
}

// ---------------------------------------------------------------------------
// 5. The read that will not answer
// ---------------------------------------------------------------------------

/// 🔴 A read-back that **cannot be taken** is `Failed`, and never `Succeeded` (`req/361` H-01,
/// `req/38` §238 ruling 1).
///
/// The third world the repair has to answer for. The roll-back's `apply` is accepted — its bytes
/// land, in full — and the read that would confirm the object is home refuses. An engine that
/// treated an unreadable object as a homecoming would have reintroduced the whole defect one layer
/// down, because *the call succeeded and I could not check* would once again be printed as *the
/// object is back*.
///
/// The answer is `Failed` rather than a fifth word, and the sentence `crates/gx-cli/src/wrap.rs`
/// carries for `Failed` has been true of this case since R25: an adapter's `apply` is the call
/// together with the read-back of it, so a compensation whose bytes landed and whose read-back died
/// belongs there. What this probe forbids is the other answer.
#[test]
fn a_read_back_that_cannot_be_taken_is_failed_and_never_succeeded() {
    let server = ScriptedServer::start("unreadable", Grammar::Relative, &INITIAL);

    let (status, body, prior, after) = block_on(async {
        let id = server.commit_goal(GOAL).await;
        let prior = server.world();
        server.adapter.script(&[
            Behaviour::Full,
            Behaviour::HalfThenFail(1),
            // The compensation lands completely, and then the object stops answering reads.
            Behaviour::FullThenUnreadable,
        ]);
        let (status, body) = server.undo(&id).await;
        (status, body, prior, server.world())
    });

    record(&format!(
        "R29_UNREADABLE status={status} gx_code={} rollback={} prior={prior:?} after={after:?} \
         reads_refusing={} applies={}",
        body["gx_code"].as_str().unwrap_or("<absent>"),
        rollback_word(&body),
        u8::from(server.adapter.is_unreadable()),
        server.adapter.applies(),
    ));
    trace("UNREADABLE", &server);

    assert_eq!(status, 422, "the undo's own apply failed: {body}");
    assert!(
        server.adapter.is_unreadable(),
        "the premise of this probe: the compensation's bytes landed and the object then stopped \
         answering reads. If the fixture is not refusing, nothing below is being measured"
    );
    assert_eq!(
        server.adapter.applies(),
        3,
        "three applies, the third being 43 T-10c's compensation — the one whose read-back dies"
    );
    assert_ne!(
        rollback_word(&body),
        "Succeeded",
        "🔴 an object this engine cannot read is not an object this engine has seen come home. \
         Answering `Succeeded` here would be the same defect one layer down: the call was accepted \
         and the check could not be made, and the caller would be told the change was taken back. \
         Body: {body}"
    );
    assert_eq!(
        rollback_word(&body),
        "Failed",
        "🔴 and the fail-safe word is `Failed`, which `crates/gx-cli/src/wrap.rs` has read since \
         R25 as covering a compensation whose bytes landed and whose read-back died — this \
         adapter's apply is the call together with the read-back of it, and a snapshot that will \
         not answer is that same fact one layer up. Body: {body}"
    );
}

// ---------------------------------------------------------------------------
// 6. The vocabulary
// ---------------------------------------------------------------------------

/// 🔴 **The vocabulary is whole** (`req/361` H-01, `req/38` §238 ruling 1).
///
/// A fourth word is a journal-schema fact before it is a wire fact: `Rollback` is serialised into
/// the `Aborted` record, so the four spellings, the value each one round-trips to, and the arm the
/// proxy writes for each have to agree or a reader is handed a sentence about a different world.
/// Three things are asked here and each fails differently:
///
/// * `Rollback::ALL_KINDS` declares **four**, including `Diverged`. A count is the cheapest way for
///   a fifth word added without a declaration to be caught.
/// * every declared word **round-trips** — deserialised into the value and back through `kind()` —
///   so a word in the list that names no variant, or a variant whose `kind()` spells it differently,
///   is a failure rather than a discrepancy nobody reads. This is asked through the serialised form
///   deliberately: that form is what reaches the journal and the wire, and it is the form a reader
///   of `gx log` actually meets.
/// * `crates/gx-cli/src/wrap.rs` carries an explicit `Some("<word>")` arm for each of the four. That
///   file's own comment says why the arm-per-value shape exists: a value added in gx-engine would
///   otherwise fall into whichever arm was written last, which is the failure this repair is about.
#[test]
fn the_roll_back_vocabulary_is_whole_and_the_proxy_has_an_arm_for_every_word() {
    let words = gx_engine::Rollback::ALL_KINDS;

    let round_tripped = words
        .iter()
        .filter(|word| {
            let value = serde_json::Value::String((*word).to_string());
            serde_json::from_value::<gx_engine::Rollback>(value)
                .map(|parsed| parsed.kind() == **word)
                .unwrap_or(false)
        })
        .count();

    let wrap = read(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("wrap.rs"),
    );
    let wrap_code = code_lines(&wrap).join("\n");
    let armed: Vec<&str> = words
        .iter()
        .copied()
        .filter(|word| wrap_code.contains(&format!("Some(\"{word}\")")))
        .collect();

    record(&format!(
        "R29_VOCAB kinds={} words={words:?} round_tripped={round_tripped} armed={} missing={:?}",
        words.len(),
        armed.len(),
        words
            .iter()
            .filter(|w| !armed.contains(w))
            .collect::<Vec<_>>(),
    ));

    assert_eq!(
        words.len(),
        4,
        "🔴 `req/361` §9-2: the world can be three ways after a failed apply — back where it was, \
         moved because the compensation was refused, and moved because the compensation itself \
         moved it — and until this lane the vocabulary had two words for it. Declared: {words:?}"
    );
    assert!(
        words.contains(&"Diverged"),
        "🔴 and the fourth word is `Diverged`. Without it the third world has no spelling and is \
         reported as one of the other two, which is the defect. Declared: {words:?}"
    );
    assert_eq!(
        round_tripped,
        words.len(),
        "every declared word must name a value and that value's `kind()` must spell it back. This \
         is asked through the serialised form because that form is what reaches the `Aborted` \
         journal record and the wire: {words:?}"
    );
    assert_eq!(
        armed.len(),
        words.len(),
        "🔴 `crates/gx-cli/src/wrap.rs` writes one sentence per roll-back word, plus one for a word \
         this build does not know. A word the engine declares and the proxy has no arm for falls \
         into the unknown-word arm and an agent is told this build has not been taught the value it \
         is carrying — false, and about the very word this lane added. Armed: {armed:?}, declared: \
         {words:?}"
    );
}
