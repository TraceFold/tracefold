// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/981` §6 F1/F2** — the two refusals a moved world produces on this face, and the row a
//! refused resume used to leave behind.
//!
//! `req/981` drove a built binary through 10,373 mutations and came back with three facts about
//! this road. Two were reported (F1, F2) and the third was not visible from the surface it
//! measured:
//!
//! * **F2** — a human edits the target between `gx verify` and `gx commit`. The world is protected
//!   (the file still holds what the human wrote), and the operator is told
//!   `{"gx_code":"INTERNAL","title":"the operation could not be completed"}`. 44 §2.3 keeps
//!   `INTERNAL` for what **cannot be classified**, and this is completely classified.
//! * **F1** — the same deletion submitted twice. 42 §3.3 puts the goal in the `IntentId` and a
//!   deletion's goal is fixed at zero bytes, so the second submission is the *same* intent by
//!   construction; `gx plan` answered the same `INTERNAL`, on **140 of 300** census runs.
//! * 🔴 **The third**, measured here for the first time: `Session::resume` called `Engine::plan` and
//!   compared afterwards, which is `req/222` H-03 on the CLI face — R3 repaired it in
//!   `gx_api::handlers::rebuilt` and nobody asked this binary the same question. A **refused**
//!   `gx verify` against a stale id appended a `Planned` record for a transformation the caller
//!   never named (journal **496 → 859 bytes**, one `Planned` record → **two**), and that row was
//!   left committable.
//!
//! # What this file is entitled to claim
//!
//! It drives the shipped binary through `support::Pipeline` and reads statuses, `gx_code`s and the
//! journal's bytes. It says nothing about the **HTTP** face, which was measured separately and
//! answers `409 PRECONDITION_CHANGED` (a real `Aborted` — the CAS runs, because the row never left
//! the process) for F2 and `409 INVALID_STATE` for F1. That difference is the point of `req/981`
//! §6 F2's last paragraph: the defect was the CLI surface's, not the engine's.
//!
//! # The negative controls, and why each one is here
//!
//! Every assertion below that a refusal *is* something is paired with one that a neighbouring
//! refusal is **not** it, because a repair that widened one arm until it swallowed its siblings
//! would pass the positive half alone.

mod support;

use support::{pipeline, run, Pipeline};

/// The journal `req/56` §2 declares, as bytes.
fn journal(fixture: &Pipeline) -> Vec<u8> {
    std::fs::read(fixture.project.join(".gx").join("ledger").join("journal")).unwrap_or_default()
}

/// How many `Planned` records the journal holds.
///
/// Counted over the bytes rather than parsed: the point is "did this refusal append a record",
/// and a count that needed the journal's reader to agree with it would be measuring two things.
fn planned_records(bytes: &[u8]) -> usize {
    bytes.windows(7).filter(|w| *w == b"Planned").count()
}

/// The `gx_code` on 44 §1.3's stderr object, or `""` when the run wrote no problem object.
fn gx_code(run: &support::Run) -> String {
    serde_json::from_str::<serde_json::Value>(run.stderr.trim())
        .ok()
        .and_then(|v| v["gx_code"].as_str().map(str::to_string))
        .unwrap_or_default()
}

/// submit → plan, handing back `(intent_id, transformation_id)`.
fn planned(fixture: &Pipeline, goal: &str) -> (String, String) {
    let submitted = fixture.submit(goal);
    assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
    let intent = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();
    let planned = run(fixture.gx().args(["plan", &intent]));
    assert_eq!(planned.code, 0, "plan: {}", planned.stderr);
    let tid = planned.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();
    (intent, tid)
}

// ---------------------------------------------------------------------------
// F2 — the world moves between verify and commit
// ---------------------------------------------------------------------------

/// 🔴 **`req/981` §6 F2** — the refusal names the fact, and the file still holds what the human
/// wrote.
///
/// The two halves are asserted together on purpose. "The world was protected" was already true
/// before the repair, and reporting only that would be reporting the half that was never broken;
/// "the word changed" without the world check would be a message improvement nobody had shown was
/// still safe.
#[test]
fn a_world_that_moved_between_verify_and_commit_is_named_rather_than_called_internal() {
    let fixture = pipeline("r981_f2", "the agent plan\n");
    let (_, tid) = planned(&fixture, "agent replacement\n");
    let verified = run(fixture.gx().args(["verify", &tid]));
    assert_eq!(verified.code, 0, "verify: {}", verified.stderr);

    // The world moves. A person edits the file the agent is about to overwrite.
    std::fs::write(&fixture.target, "a human wrote this\n").expect("the human writes");

    let committed = run(fixture.gx().args(["commit", &tid]));
    let code = gx_code(&committed);
    println!(
        "F2 exit={} gx_code={code} stderr={}",
        committed.code,
        committed.stderr.trim()
    );

    assert_ne!(
        code, "INTERNAL",
        "🔴 44 §2.3 keeps `INTERNAL` for what cannot be classified. A pre-flight that refused \
         because the state machine named a state and named a transition it does not offer is \
         completely classified, and `req/981` F2 measured this exact run answering \
         \"the operation could not be completed\""
    );
    assert_eq!(
        code, "VALIDATION_ERROR",
        "the word `Error::Usage` carries: {}",
        committed.stderr
    );
    // 44 §1.4's 1, unmoved. `req/306` §1 forbids moving an exit status without a ruling and this
    // repair does not ask for one — see `pipeline::plan`'s comment for the word that would.
    assert_eq!(
        committed.code,
        1,
        "the status does not move: {committed:?}",
        committed = committed.stderr
    );

    // The sentence has to carry the facts an operator acts on: which state the row is in, and a
    // remedy that is true of *that* state.
    assert!(
        committed.stderr.contains("Admitted"),
        "the refusal names the state the row is in: {}",
        committed.stderr
    );
    assert!(
        committed.stderr.contains("left `Candidate`"),
        "the remedy branch for a row past `Candidate`: {}",
        committed.stderr
    );
    assert!(
        !committed.stderr.contains("run `gx plan` again"),
        "🔴 the old message printed one remedy for both states. `req/981` §6 F2 ran it and it \
         failed: past `Candidate` the re-plan is refused. A remedy that fails when followed is \
         worse than none: {}",
        committed.stderr
    );

    assert_eq!(
        std::fs::read_to_string(&fixture.target).expect("the target reads"),
        "a human wrote this\n",
        "the world is protected — that half was never broken and is asserted so that a message \
         repair cannot quietly cost it"
    );
}

/// 🔴 The remedy the F2 sentence names, **run**.
///
/// `r19_escalation_road.rs`'s M-07 established the rule for this repository: a remedy is measured
/// by running it, not by reading it. The sentence says a different intent is the road forward, so
/// this drives one.
#[test]
fn the_road_the_f2_refusal_names_is_one_that_works() {
    let fixture = pipeline("r981_f2_remedy", "the agent plan\n");
    let (intent, tid) = planned(&fixture, "agent replacement\n");
    assert_eq!(run(fixture.gx().args(["verify", &tid])).code, 0);
    std::fs::write(&fixture.target, "a human wrote this\n").expect("the human writes");
    assert_eq!(run(fixture.gx().args(["commit", &tid])).code, 1);

    // (a) what the old sentence told the operator to do, measured: it does not work here.
    let replanned = run(fixture.gx().args(["plan", &intent]));
    println!(
        "F2 REPLAN exit={} {}",
        replanned.code,
        replanned.stderr.trim()
    );
    assert_ne!(
        replanned.code, 0,
        "this is why the remedy branches: past `Candidate` the re-plan is refused"
    );
    assert_ne!(gx_code(&replanned), "INTERNAL", "and it is still named");

    // (b) what the new sentence names: a different intent.
    let (_, fresh) = planned(&fixture, "agent second attempt\n");
    assert_eq!(run(fixture.gx().args(["verify", &fresh])).code, 0);
    let committed = run(fixture.gx().args(["commit", &fresh]));
    assert_eq!(
        committed.code, 0,
        "the road the refusal names has to be a road: {}",
        committed.stderr
    );
}

// ---------------------------------------------------------------------------
// F1 — the same intent, submitted twice
// ---------------------------------------------------------------------------

/// 🔴 **`req/981` §6 F1** — the deletion an agent retries, and the sentence that tells it why.
///
/// The goal is zero bytes, which is what makes this the sharp case: 42 §3.3 puts substrate,
/// locator, goal, context and actor in the `IntentId`, and a deletion's goal has **no free bits**,
/// so a retry is the same intent whatever the agent does short of renaming its reason or its key.
#[test]
fn the_same_deletion_submitted_twice_is_named_rather_than_called_internal() {
    let fixture = pipeline("r981_f1", "alpha\n");
    let (intent, tid) = planned(&fixture, "");
    assert_eq!(run(fixture.gx().args(["verify", &tid])).code, 0);
    assert_eq!(run(fixture.gx().args(["commit", &tid])).code, 0);
    assert_eq!(
        std::fs::read_to_string(&fixture.target).expect("readable"),
        "",
        "the deletion landed"
    );

    // The agent retries. `submit` is content-addressed, so this is the same intent id.
    let again = fixture.submit("");
    assert_eq!(again.code, 0, "{}", again.stderr);
    assert_eq!(
        again.json()["intent_id"].as_str().expect("an intent id"),
        intent,
        "🔴 the premise: an identical request is the same intent by construction"
    );

    let replanned = run(fixture.gx().args(["plan", &intent]));
    let code = gx_code(&replanned);
    println!(
        "F1 exit={} gx_code={code} stderr={}",
        replanned.code,
        replanned.stderr.trim()
    );
    assert_ne!(
        code, "INTERNAL",
        "🔴 `req/981` measured this on 140 of 300 census runs: the most ordinary thing an agent \
         does — retrying a deletion — told the operator the binary had broken"
    );
    assert_eq!(code, "VALIDATION_ERROR", "{}", replanned.stderr);
    assert_eq!(replanned.code, 1, "the status does not move");
    assert!(
        replanned.stderr.contains("Committed"),
        "the refusal names the state of the transformation the intent already resolved to: {}",
        replanned.stderr
    );
    assert!(
        replanned.stderr.contains("nothing was written"),
        "and says so, because `req/222` H-03's lesson is that a refusal which writes is worse \
         than the refusal it pretends to be: {}",
        replanned.stderr
    );
    assert!(
        replanned.stderr.contains("different"),
        "the road forward is a different intent: {}",
        replanned.stderr
    );
    assert!(
        !replanned.stderr.contains("change `--context`")
            && !replanned.stderr.contains("try a different --context"),
        "🔴 `--context` and `--actor-key` do unstick this (`req/981` controls3) and the sentence \
         must not recommend them: they are the record of *why* and *who*: {}",
        replanned.stderr
    );
}

// ---------------------------------------------------------------------------
// The third fact — a refused resume that wrote
// ---------------------------------------------------------------------------

/// 🔴 **`req/222` H-03's twin on this face** — a refusal that writes nothing, measured in bytes.
///
/// The positive control is in the same test and is what makes the zero meaningful: a **successful**
/// verify on the same fixture does grow the journal, so "no growth" is a fact about this refusal
/// and not about the instrument.
#[test]
fn a_refused_resume_leaves_no_row_behind() {
    let fixture = pipeline("r981_h03_twin", "alpha\n");
    let (_, tid) = planned(&fixture, "agent bytes\n");

    let before = journal(&fixture);
    let (b0, p0) = (before.len(), planned_records(&before));

    // The world moves while the row is still a `Candidate` — the arm where `Engine::plan` would
    // have happily minted a second transformation and appended a `Planned` for it.
    std::fs::write(&fixture.target, "a human wrote this\n").expect("the human writes");
    let refused = run(fixture.gx().args(["verify", &tid]));
    let after = journal(&fixture);
    let (b1, p1) = (after.len(), planned_records(&after));
    println!(
        "H03_TWIN refused_exit={} bytes {b0}->{b1} planned {p0}->{p1}",
        refused.code
    );

    assert_ne!(
        refused.code, 0,
        "the stale id is refused: {}",
        refused.stdout
    );
    assert_eq!(
        b1,
        b0,
        "🔴 a refusal wrote {} bytes to the journal. Measured before the repair: 496 -> 859",
        b1 as i64 - b0 as i64
    );
    assert_eq!(
        p1, p0,
        "🔴 and it was a `Planned` record for a transformation the caller never named — \
         `req/222` H-03's \"a refused request that leaves behind a fresh, committable claim on \
         somebody else's substrate is worse than the refusal it was pretending to be\""
    );

    // The positive control for the instrument: a run that *is* supposed to write, does.
    let fixture2 = pipeline("r981_h03_control", "alpha\n");
    let (_, tid2) = planned(&fixture2, "agent bytes\n");
    let c0 = journal(&fixture2);
    assert_eq!(run(fixture2.gx().args(["verify", &tid2])).code, 0);
    let c1 = journal(&fixture2);
    println!("H03_CONTROL bytes {}->{}", c0.len(), c1.len());
    assert!(
        c1.len() > c0.len(),
        "the instrument can see growth — otherwise the zero above measures nothing"
    );
}

// ---------------------------------------------------------------------------
// N-37 — the neighbours the repair must not have swallowed
// ---------------------------------------------------------------------------

/// 🔴 Adversarial control 1 — 44 §1.2's "retrying the same execution is naturally idempotent".
///
/// The repair puts a read-only `planned_id` in front of every resume. If that comparison were
/// wrong, the first road to break would be the one where **nothing moved**, and it would break
/// silently at exit 0 → exit 1.
#[test]
fn the_idempotent_commit_road_did_not_move() {
    let fixture = pipeline("r981_ctl_idempotent", "alpha\n");
    let (_, tid) = planned(&fixture, "beta\n");
    assert_eq!(run(fixture.gx().args(["verify", &tid])).code, 0);
    let first = run(fixture.gx().args(["commit", &tid]));
    assert_eq!(first.code, 0, "commit: {}", first.stderr);
    let second = run(fixture.gx().args(["commit", &tid]));
    println!(
        "IDEMPOTENT second_exit={} {}",
        second.code,
        second.stderr.trim()
    );
    assert_eq!(
        second.code, 0,
        "44 §1.2: retrying the same execution is naturally idempotent: {}",
        second.stderr
    );
    assert_eq!(
        std::fs::read_to_string(&fixture.target).expect("readable"),
        "beta\n"
    );
}

/// 🔴 Adversarial control 2 — a state-machine refusal that is **not** a stale plan keeps its own
/// road.
///
/// `gx cancel` on a `Committed` row is E-M6-13's refusal: 44 §1.4's **2**, on stdout, with
/// `refused: "OutsideCancelWindow"`. If the repair had reached for every `InvalidState` instead of
/// the one condition it measured, this is the road it would have taken over.
#[test]
fn a_state_machine_refusal_that_is_not_a_stale_plan_is_unchanged() {
    let fixture = pipeline("r981_ctl_cancel", "alpha\n");
    let (_, tid) = planned(&fixture, "beta\n");
    assert_eq!(run(fixture.gx().args(["verify", &tid])).code, 0);
    assert_eq!(run(fixture.gx().args(["commit", &tid])).code, 0);

    let cancelled = run(fixture.gx().args(["cancel", &tid]));
    println!(
        "CANCEL exit={} stdout={}",
        cancelled.code,
        cancelled.stdout.trim()
    );
    assert_eq!(cancelled.code, 2, "E-M6-13's 2: {}", cancelled.stderr);
    assert_eq!(
        cancelled.json()["refused"].as_str(),
        Some("OutsideCancelWindow"),
        "its own word, on stdout, as an `Outcome` and not an `Err`"
    );
}

/// 🔴 Adversarial control 3 — an id nothing here has seen is still 44 §1.4's **6**.
///
/// R23 moved that road onto `NOT_FOUND` and this repair sits one arm away from it in
/// `pipeline::plan`. A guard written on the wrong discriminator would take it.
#[test]
fn an_unknown_id_is_still_not_found() {
    let fixture = pipeline("r981_ctl_unknown", "alpha\n");
    let stranger = "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let missed = run(fixture.gx().args(["plan", stranger]));
    println!("UNKNOWN exit={} {}", missed.code, missed.stderr.trim());
    assert_eq!(missed.code, 6, "{}", missed.stderr);
    assert_eq!(gx_code(&missed), "NOT_FOUND", "{}", missed.stderr);
}

/// 🔴 Adversarial control 4 — the `Candidate` arm keeps the remedy that is true of it, and the
/// remedy is **run**.
///
/// The two arms of `stale_remedy` are a claim about the world, so both are measured. This is the
/// one where "run `gx plan` again" is correct, and the test runs it rather than reading it.
#[test]
fn a_stale_candidate_keeps_the_remedy_that_works_and_it_works() {
    let fixture = pipeline("r981_ctl_candidate", "alpha\n");
    let (intent, tid) = planned(&fixture, "beta\n");
    // The world moves while the row is a `Candidate`: nothing has verified it yet.
    std::fs::write(&fixture.target, "a human wrote this\n").expect("the human writes");

    let refused = run(fixture.gx().args(["verify", &tid]));
    println!(
        "CANDIDATE_STALE exit={} {}",
        refused.code,
        refused.stderr.trim()
    );
    assert_eq!(gx_code(&refused), "VALIDATION_ERROR", "{}", refused.stderr);
    assert!(
        refused.stderr.contains("run `gx plan` again"),
        "🔴 from `Candidate` the re-plan is 43 T-2's legal one, so this arm keeps the old \
         remedy: {}",
        refused.stderr
    );
    assert!(
        !refused.stderr.contains("left `Candidate`"),
        "and does not print the other arm's sentence: {}",
        refused.stderr
    );

    // Run what it says.
    let replanned = run(fixture.gx().args(["plan", &intent]));
    println!(
        "CANDIDATE_REPLAN exit={} {}",
        replanned.code,
        replanned.stderr.trim()
    );
    assert_eq!(
        replanned.code, 0,
        "the remedy this arm prints, run verbatim: {}",
        replanned.stderr
    );
    let fresh = replanned.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();
    assert_ne!(
        fresh, tid,
        "and it names the transformation the moved world produces"
    );
    assert_eq!(run(fixture.gx().args(["verify", &fresh])).code, 0);
}
