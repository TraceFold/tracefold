// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 `gx undo`'s settle pre-flight and `--retry` (`req/38` §98 ruling 2 — DR-2 option A + D-complement) (sem: SEM-gx-cli-1329).
//!
//! `req/154` F1 measured the gap this closes: the product's `gx undo` fired right after a forward
//! commit against a real SaaS substrate lands in the server's read-after-write window and answers
//! `Aborted(ApplyFailed)` (fail-safe), and the green E2E of `req/153` depended on a settle gate
//! that existed **only in the harness script**. §98 ruling 2 (sem: SEM-gx-cli-1330) moves the gate into the product:
//! before `Engine::undo`, the CLI polls `Engine::live_digest` (read-only) against the commit
//! receipt's own signed `postcondition_fingerprint`, bounded by `--settle <SECS>` (default 120,
//! 0 = off), and on timeout **fires once anyway** — the poll judges nothing.
//!
//! # What this suite pins, and what it deliberately cannot
//!
//! The fs substrate is in-process and read-after-write consistent, so poll 1 always matches here
//! — which is itself one of the design's claims ("mock/in-process substrate = poll 1 matches =
//! zero behavioral difference"; sem: SEM-gx-cli-1331) and is what the first test measures. The *lag* half of the design — a world that
//! catches up after N polls — cannot be built out of a consistent substrate without stubbing the
//! adapter, so what is measured instead is every exit of the pre-flight that is not "waited and
//! matched late" (sem: SEM-gx-cli-1332): disabled, skipped (no receipt), timeout (a world that never matches), and the
//! fact that none of them changes the launch's result. The late-match case is the real-server
//! E2E's (`tools/a3_settle_undo_e2e.sh`, poll distribution on stderr, `req/163`).

mod support;

use support::{deny_writable_pack, pipeline, pipeline_named, run, DENIED_FRAGMENT};

/// 🔴 The in-process substrate matches on poll 1 — "zero behavioral difference" (sem: SEM-gx-cli-1333), with the distribution line to show it.
///
/// The world right after a fs commit **is** what the receipt's postcondition attests (same
/// process, same disk), so the pre-flight's cost on every existing flow is one snapshot and no
/// sleep. The stderr line is load-bearing: §98 ruling 2 makes the poll distribution's presence a
/// condition of the design ("include the poll-distribution record riding along as a condition of validity"; sem: SEM-gx-cli-1334), so a build that stopped
/// printing it would fail here rather than silently un-measuring the default.
#[test]
fn the_preflight_matches_on_poll_one_and_the_undo_is_green() {
    let fixture = pipeline("a3_settle_match", "before\n");
    let committed = fixture.commit_one("after\n");
    assert_eq!(fixture.target_contents(), "after\n");

    let undone = run(fixture.gx().args(["undo", &committed]));
    println!(
        "SETTLE_MATCH exit={} stderr={}",
        undone.code,
        undone.stderr.trim()
    );
    assert_eq!(undone.code, 0, "stderr: {}", undone.stderr);
    assert_eq!(
        fixture.target_contents(),
        "before\n",
        "the undo still undoes"
    );
    assert!(
        undone.stderr.contains("gx undo settle:") && undone.stderr.contains("result=matched"),
        "the pre-flight reports its outcome on stderr (§98: the distribution rides along): {}",
        undone.stderr
    );
    assert!(
        undone.stderr.contains("polls=1"),
        "an in-process substrate settles on the first poll — the \"zero behavioral difference\" claim (sem: SEM-gx-cli-1335), measured: {}",
        undone.stderr
    );
}

/// `--settle 0` disables the pre-flight, and says so rather than silently not polling.
#[test]
fn settle_zero_disables_the_preflight() {
    let fixture = pipeline("a3_settle_zero", "before\n");
    let committed = fixture.commit_one("after\n");

    let undone = run(fixture.gx().args(["undo", &committed, "--settle", "0"]));
    println!(
        "SETTLE_ZERO exit={} stderr={}",
        undone.code,
        undone.stderr.trim()
    );
    assert_eq!(undone.code, 0, "stderr: {}", undone.stderr);
    assert_eq!(fixture.target_contents(), "before\n");
    assert!(
        undone.stderr.contains("gx undo settle: disabled"),
        "a disabled pre-flight is announced, not omitted: {}",
        undone.stderr
    );
    assert!(
        !undone.stderr.contains("result=matched"),
        "and nothing polled: {}",
        undone.stderr
    );
}

/// 🔴 A world a third party moved never matches — the deadline bounds the wait, and **DR-43-1(a)
/// refuses the undo** (`req/38` §132 ruling 2).
///
/// ~~This is the "fire once as-is after timeout, result vocabulary unchanged" (sem: SEM-gx-cli-1336) half of §98 ruling 2, driven through the
/// binary: the target is overwritten behind gx's back after the commit, so the live digest never
/// equals the receipt's postcondition, `--settle 1` expires, and the launch proceeds — here to a
/// green undo (the fs apply of the escrowed inverse does not care who moved the file since;
/// T-10a's CAS takes the undo's *own* fresh precondition, 43 §5-2). A pre-flight that refused on
/// timeout, or waited forever on a world that legitimately moved on, would fail this test.~~
///
/// The struck paragraph is kept because it is the true record of what §98 ruling 2 asked for and of
/// what this test measured until 2026-08-16 (no-delete). `req/182` H-15 then measured what the last
/// sentence cost — "RC 0 · Committed · file back at `AAA`, `CCC` gone" — and `req/38` §132 ruling 2
/// adopted DR-43-1 **(a)**: the undo is compared against `T_o`'s signed postcondition and refused if
/// the world moved. So the half that is now inverted is the exit: **3** (`PRECONDITION_CHANGED`),
/// and the file keeps the third party's content.
///
/// What did **not** change is the pre-flight's own posture: it still polls, still bounds the wait
/// with `--settle`, still refuses nothing itself, and still prints its distribution. The judge is
/// `Engine::undo`, one layer down, which is where §122's "promote the settle pre-flight to a gate"
/// actually lands.
#[test]
fn a_moved_world_times_out_and_dr_43_1_refuses_the_undo() {
    let fixture = pipeline("a3_settle_timeout", "before\n");
    let committed = fixture.commit_one("after\n");
    std::fs::write(&fixture.target, "moved by a third party\n").expect("move the world");

    let undone = run(fixture.gx().args(["undo", &committed, "--settle", "1"]));
    println!(
        "SETTLE_TIMEOUT exit={} stderr={}",
        undone.code,
        undone.stderr.trim()
    );
    assert!(
        undone.stderr.contains("result=timeout"),
        "the poll never matches a moved world, and the deadline is what ends the wait: {}",
        undone.stderr
    );
    assert_eq!(
        undone.code, 3,
        "🔴 DR-43-1(a): 44 §1.4's 3 = `PRECONDITION_CHANGED`, the number 44 §1.2 already gives \
         `gx undo`. This assertion read `0` until `req/38` §132 ruling 2. stderr: {}",
        undone.stderr
    );
    assert!(
        undone.stderr.contains("PRECONDITION_CHANGED"),
        "and the refusal wears 44 §2.3's code on stderr (44 §1.3's problem object): {}",
        undone.stderr
    );
    assert!(
        undone.stderr.contains(&fixture.target.display().to_string()),
        "🔴 aider's property (`req/207` §3-2, \"do not proceed on a guess\"): the refusal names the \
         place that moved, so the operator can look at it. {}",
        undone.stderr
    );
    assert_eq!(
        fixture.target_contents(),
        "moved by a third party\n",
        "🔴 `req/182` H-15's measurement, inverted: the third party's content is still there. \
         Before the ruling this read \"before\\n\" and the third party's write was gone"
    );
}

/// 🔴 ~~No stored commit receipt → no expected value → the pre-flight **skips** (fail-safe side)
/// and the undo behaves exactly as every version before it did.~~
///
/// ~~The receipt store is `.gx/receipts/` (M6H2-1) and gotcha95 asks operators to keep the
/// checkpoint *pair*; a project where the receipt is gone must not become a project where undo
/// blocks for the full budget or, worse, refuses. "no receipt = skip the pre-flight = legacy behavior" (sem: SEM-gx-cli-1337) is the
/// posture `req/160` §2 was asked to design and this pins.~~
///
/// # 🔴 **Reversed by R3** (`req/38` §160 ruling 2, `req/222` H-01)
///
/// The struck words are kept because they are the true record of what was decided in `req/160` §2
/// and why, and because the sentence "or, worse, refuses" is what this probe now asserts. What
/// changed is not the reasoning about *cost* — a skip is still immediate and still says so — but
/// the discovery of what the skip was worth: `req/222` measured, 3/3, that removing this exact
/// file turned a `409 PRECONDITION_CHANGED` into a `200` that destroyed a third party's write,
/// and that `archive_commit_receipt`'s `let _ =` reached the same state with no attacker at all.
/// "Fail-safe" named the wrong side. The safe side of "I cannot check whether this change is still
/// the last thing that happened here" is to not put the old bytes back.
///
/// The two properties this probe still owns are kept: the answer is **immediate** (a refusal that
/// sat out a 120 s budget would be a worse bug than the one being fixed), and the reason is said
/// out loud on stderr.
#[test]
fn a_missing_commit_receipt_refuses_the_undo_rather_than_firing_it() {
    let fixture = pipeline("a3_settle_no_receipt", "before\n");
    let committed = fixture.commit_one("after\n");
    let stored = fixture
        .project
        .join(".gx")
        .join("receipts")
        .join(format!("{}.commit.json", committed.replace(':', "_")));
    assert!(stored.is_file(), "the fixture's commit stored a receipt");
    std::fs::remove_file(&stored).expect("take the receipt away");

    let started = std::time::Instant::now();
    let undone = run(fixture.gx().args(["undo", &committed]));
    let elapsed = started.elapsed();
    println!(
        "SETTLE_SKIP exit={} elapsed={elapsed:?} stderr={}",
        undone.code,
        undone.stderr.trim()
    );
    assert_eq!(
        undone.code, 3,
        "🔴 R3: this read **0**, and the line below read \"before\n\" — the undo fired with \
         nothing to compare against. 44 §1.4's 3 is the number `gx undo` already had for a CAS \
         that did not pass, and §160 ruling 2 mints no new one. stderr: {}",
        undone.stderr
    );
    assert_eq!(
        fixture.target_contents(),
        "after\n",
        "🔴 the fact the number is about: the committed change is still there, because nothing \
         could say whether it was still the last thing that happened at this locator"
    );
    assert!(
        undone.stderr.contains("gx undo refused")
            && undone.stderr.contains("no stored commit receipt"),
        "the refusal is said out loud, with its reason and with the road back: {}",
        undone.stderr
    );
    assert!(
        undone.stderr.contains("Restore the receipt"),
        "and it names what an operator can do about it — a fail-closed gate with no remedy is the \
         shape `req/222` H-06 called a trap: {}",
        undone.stderr
    );
    assert!(
        elapsed < std::time::Duration::from_secs(60),
        "a refusal is immediate — it must not sit out the 120s default budget: {elapsed:?}"
    );
}

/// 🔴 A second undo of one commit is refused **immediately**, not after the settle budget.
///
/// After a green undo the original is `Superseded` and its world is back at "before" (sem: SEM-gx-cli-1338) — so the
/// live digest can never equal T_o's postcondition again, and a pre-flight that polled anyway
/// would sit out the full 120s in front of a `Consumed` refusal that was fixed before the first
/// poll. Measured into existence: the second-undo case of `undo_cmd.rs` did exactly that against
/// the first spelling of the pre-flight. The guard skips the poll when the launch's answer
/// cannot be changed by waiting (original not `Committed`, or inverse not `Available`).
#[test]
fn a_second_undo_is_refused_immediately_rather_than_after_the_budget() {
    let fixture = pipeline("a3_settle_second_undo", "before\n");
    let committed = fixture.commit_one("after\n");
    let first = run(fixture.gx().args(["undo", &committed]));
    assert_eq!(first.code, 0, "stderr: {}", first.stderr);

    let started = std::time::Instant::now();
    let again = run(fixture.gx().args(["undo", &committed]));
    let elapsed = started.elapsed();
    println!(
        "SECOND_UNDO exit={} elapsed={elapsed:?} stderr={}",
        again.code,
        again.stderr.trim()
    );
    assert_ne!(again.code, 0, "42 §3.12's Consumed still refuses");
    assert!(
        again.stderr.contains("gx undo settle: skipped"),
        "the pre-flight names its skip instead of polling a question with a fixed answer: {}",
        again.stderr
    );
    assert!(
        elapsed < std::time::Duration::from_secs(60),
        "and the refusal is prompt — not the settle budget's length: {elapsed:?}"
    );
    assert_eq!(fixture.target_contents(), "before\n", "nothing moved");
}

/// 🔴 `--retry` re-fires on **`Aborted(ApplyFailed)` alone** — a denial is an answer, not a
/// transient, and retrying it would hammer a gate that already said no.
#[test]
fn retry_does_not_refire_a_denied_undo() {
    let fixture = pipeline_named(
        "a3_retry_denied",
        "before\n",
        &format!("{DENIED_FRAGMENT}.txt"),
    );
    let committed = fixture.commit_one("after\n");

    let pack = deny_writable_pack();
    let undone = run(fixture
        .gx()
        .args(["undo", &committed, "--retry", "3"])
        .arg("--policy")
        .arg(&pack));
    println!(
        "RETRY_DENIED exit={} stderr={}",
        undone.code,
        undone.stderr.trim()
    );
    assert_eq!(
        undone.code, 2,
        "M6-25: a denied undo is 44 §1.4's 2, and stays one under --retry. stdout: {} stderr: {}",
        undone.stdout, undone.stderr
    );
    assert!(
        !undone.stderr.contains("gx undo --retry"),
        "no re-fire on a denial — only exit 5 is a transient: {}",
        undone.stderr
    );
    assert_eq!(fixture.target_contents(), "after\n", "and nothing moved");
}

/// 🔴 `--retry <n>` on a genuine `Aborted(ApplyFailed)`: every attempt is its own journalled T_u,
/// the final answer is still exit 5, and the world is untouched (fail-safe throughout).
///
/// The Given is a directory made read-only after the commit, so the fs adapter's temp-file step
/// refuses (`Error::ApplyFailed`) on every attempt — the deterministic local stand-in for the
/// SaaS lag §98 ruling 2's D-complement exists for (sem: SEM-gx-cli-1339). Unix-only because a read-only *directory* is a
/// POSIX notion; the floor runs under WSL2 (standing row 84).
#[cfg(unix)]
#[test]
fn retry_refires_on_apply_failed_and_each_attempt_is_its_own_transformation() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = pipeline("a3_retry_applyfailed", "unused\n");
    let locked = fixture.project.join("locked");
    std::fs::create_dir_all(&locked).expect("a directory to lock");
    let target = locked.join("target.txt");
    std::fs::write(&target, "before\n").expect("write the target");

    // submit → plan → verify → commit against the nested target (the fixture's own helper is
    // bound to its default target, so the four steps are driven here with the same statuses).
    let goal_file = fixture.project.join("goal-retry.txt");
    std::fs::write(&goal_file, "after\n").expect("write the goal");
    let submitted = run(fixture
        .gx()
        .arg("submit")
        .args(["--substrate", "fs"])
        .arg("--locator")
        .arg(&target)
        .arg("--intent")
        .arg(&goal_file)
        .args(["--context", "Evidence"])
        .args(["--actor-key", &fixture.key_id]));
    assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
    let intent_id = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();
    let planned = run(fixture.gx().args(["plan", &intent_id]));
    assert_eq!(planned.code, 0, "plan: {}", planned.stderr);
    let committed = planned.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();
    let verified = run(fixture.gx().args(["verify", &committed]));
    assert_eq!(verified.code, 0, "verify: {}", verified.stderr);
    let done = run(fixture.gx().args(["commit", &committed]));
    assert_eq!(done.code, 0, "commit: {}", done.stderr);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "after\n");

    // Lock the directory: reads stay open (snapshot, settle probe), writes refuse (apply).
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).expect("lock");

    let undone = run(fixture
        .gx()
        .args(["undo", &committed, "--retry", "2", "--settle", "0"]));
    println!(
        "RETRY_APPLYFAILED exit={} stderr={}",
        undone.code,
        undone.stderr.trim()
    );

    // Unlock before asserting, so a failure here does not leave an undeletable scratch tree.
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("unlock");

    assert_eq!(
        undone.code, 5,
        "three attempts, one answer: the world would not take the write, and 44 §1.4's 5 is \
         unchanged by --retry. stdout: {} stderr: {}",
        undone.stdout, undone.stderr
    );
    assert_eq!(
        undone.stderr.matches("gx undo --retry: attempt").count(),
        2,
        "--retry 2 re-fires exactly twice after the first failure: {}",
        undone.stderr
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "after\n",
        "fail-safe throughout: no attempt moved the world"
    );

    // 🔴 Each attempt is an ordinary T_u of its own, honestly in the journal — the D-complement's
    // stated cost, measured: three `Planned` records whose parents name the original.
    let attempts = fixture
        .journal()
        .iter()
        .filter(|record| {
            matches!(
                record,
                gx_engine::EngineJournalRecord::Planned { parents, .. }
                    if parents.iter().any(|p| p.0.to_text() == committed)
            )
        })
        .count();
    assert_eq!(
        attempts, 3,
        "1 + --retry 2 = three journalled undo attempts (each with its own timestamp and id)"
    );
}
