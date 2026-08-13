//! **AC-043** — crash injection at three points, ten times each, in real processes.
//!
//! 34 AC-043 逐語:
//!
//! > Given: crash-injection環境。When: T-9（`CommittingStarted`journal直後）・T-10b
//! > （`InverseEscrowed`直後）・apply成功直後かつ`ledger.append`前、の3injection pointそれぞれで
//! > `kill -9`し、43 §7のリカバリ手順に従い再起動する（各injection point 10回試行）。Then: 全試行を
//! > 通じ同一TransformationIdについて`ledger`entryが高々1件（重複0件）。
//!
//! # The criterion counts entries; this suite also checks the sentence underneath it
//!
//! 「高々1件」 is satisfied by an engine that never commits anything at all. So every trial below
//! checks the ledger count **and** the three faces together: the journal (what was recorded), the
//! ledger (what was witnessed) and the world (what actually happened). The conjunction that must
//! never appear is 「the world moved and the ledger is empty」 — P-4, and the reason this project
//! exists. A count on its own cannot see it.
//!
//! # Why the third point is the one that matters
//!
//! req/78 §3.2 Λ4 is a three-line proof that the recovery 43 §7-3c *writes down* produces exactly
//! that conjunction at the third injection point: the crash lands after a successful `apply`, the
//! re-run recomputes `Fingerprint₁`, finds it different **because of its own write**, and aborts
//! with `PreconditionChanged` — leaving a changed world and an empty ledger. **E-M5-1** is the
//! ruling that closes it and `tests/crash_recovery.rs` runs both recoveries side by side. Here the
//! third point is simply run ten times and measured.

mod support;

use support::{copy_tree, kill_at, need, probe, scratch, value};

/// How many trials each injection point gets (34 AC-043: 「各injection point 10回試行」).
const TRIALS: usize = 10;

/// One trial: a run armed for `point`, killed there, then a restart that recovers.
///
/// Returns `(recovery stdout, world after recovery, leaves after recovery)`.
fn trial(point: &str, n: usize) -> (String, String, u64) {
    let dir = scratch(&format!("ac043_{point}_{n}"));
    let marker = kill_at(&dir, point, "after");
    assert_eq!(
        marker, point,
        "the child stopped at the point it was armed for"
    );

    // The three faces, as the crash left them. Read from the files rather than from the dead
    // process, because a process that was `SIGKILL`ed reports nothing about itself.
    let world_at_crash = std::fs::read_to_string(dir.join("world")).expect("the world survives");
    let tid = std::fs::read_to_string(dir.join("tid")).expect("the id was written before T-9");

    let out = probe(&["recover", &dir.display().to_string()]);
    let world = std::fs::read_to_string(dir.join("world")).expect("the world survives recovery");
    let leaves: u64 = need(&out, "LEDGER_LEAVES")
        .parse()
        .expect("the leaf count is a number");

    // 🔴 P-4, at every point and in both directions.
    assert!(
        !(world == "after" && leaves == 0),
        "{point} trial {n}: the world moved and the ledger is empty -- 「適用されたのに記録が無い」\n\
         crash-time world {world_at_crash:?}, id {tid}\n{out}"
    );
    assert_eq!(
        need(&out, "LEDGER_AGREES"),
        "true",
        "{point} trial {n}: the frontier and the ledger disagree after recovery\n{out}"
    );
    (out, world, leaves)
}

/// 51 §8.1's first point: `CommittingStarted` is journalled and the world has not been touched.
///
/// The recovery folds this to `Aborted(InternalError)` and re-runs nothing, because 43 §7-3c's
/// 「最初から再実行」 needs a locator the journal does not carry (see `Engine::recover`, and
/// **M5H5-2**). The criterion is met the strong way: **zero** ledger entries, and a world still
/// holding what `plan` saw.
#[test]
fn ac_043_a_crash_after_t9_leaves_no_entry_and_an_untouched_world() {
    let mut seen = Vec::new();
    for n in 0..TRIALS {
        let (out, world, leaves) = trial("t9", n);
        assert_eq!(
            leaves, 0,
            "nothing was applied, so nothing may be witnessed\n{out}"
        );
        assert_eq!(world, "before", "T-10a's fresh read never returned");
        seen.push(need(&out, "RECOVERED_ROWS"));
        let row = out
            .lines()
            .find(|l| l.starts_with("RECOVERED id="))
            .unwrap_or_default()
            .to_string();
        // The reason is named, not just the terminal. `PreconditionChanged` would claim somebody
        // moved the world and `ApplyFailed` would claim an adapter refused; neither happened, and
        // 「事実の誤記」 is what §32 M4H4-2 and §33 M4H5-5 refused twice. `InternalError` is 43
        // T-13's receptacle and the least wrong of six -- which is exactly why **M5H5-2** raises it
        // rather than settling it.
        assert!(
            row.contains("path=NothingWasApplied") && row.contains("state=Aborted(InternalError)"),
            "trial {n}: {row}"
        );
    }
    println!("AC043_T9 trials={TRIALS} rows={seen:?} LEAVES=0 WORLD=before");
}

/// 51 §8.1's second point: the inverse is escrowed and the world has not moved.
///
/// 🔴 The journal tail is one record longer than 51 §8.1 expects — `ApplyStarted` sits between
/// `InverseEscrowed` and the call, and **E-M5-1** put it there after 51 was written, so no adapter
/// seam can land between the two. The fact the point exists to measure is measured exactly: the
/// escrow is durable, the world is untouched. Raised as **M5H5-4**.
#[test]
fn ac_043_a_crash_after_the_escrow_reapplies_and_witnesses_once() {
    for n in 0..TRIALS {
        let (out, world, leaves) = trial("escrow", n);
        assert_eq!(leaves, 1, "exactly-once, from the announced side\n{out}");
        assert_eq!(world, "after", "the recovery completed the application");
        let row = out
            .lines()
            .find(|l| l.starts_with("RECOVERED id="))
            .unwrap_or_default()
            .to_string();
        assert!(
            row.contains("path=ApplyWasAnnounced") && row.contains("state=Committed"),
            "trial {n}: {row}"
        );
    }
    println!("AC043_ESCROW trials={TRIALS} LEAVES=1 WORLD=after PATH=ApplyWasAnnounced");
}

/// 🔴 51 §8.1's third point — **Λ4's window**: the apply succeeded and nothing has recorded it.
///
/// 51 §8.1 on this point: 「43はこの区間に個別のjournal record名を定義しない」. E-M5-1 defines one,
/// and this is the trial that says what it buys: ten crashes with a changed world and an empty
/// ledger, ten recoveries that reach `Committed` with exactly one entry each.
#[test]
fn ac_043_a_crash_between_apply_and_append_is_recovered_not_misread() {
    for n in 0..TRIALS {
        let dir_name = format!("ac043_applied_{n}");
        let (out, world, leaves) = trial("applied", n);
        assert_eq!(leaves, 1, "exactly-once, in Λ4's window\n{out}");
        assert_eq!(
            world, "after",
            "the world had already moved before the kill"
        );
        let row = out
            .lines()
            .find(|l| l.starts_with("RECOVERED id="))
            .unwrap_or_default()
            .to_string();
        assert!(
            row.contains("path=ApplyWasAnnounced") && row.contains("state=Committed"),
            "trial {n} ({dir_name}): {row}"
        );
    }
    println!("AC043_APPLIED trials={TRIALS} LEAVES=1 WORLD=after PATH=ApplyWasAnnounced");
}

/// The recovery is itself idempotent: recovering twice does not witness twice.
///
/// INV-S3 is 「各`TransformationId`について`ledger`entryは高々1件」 and a recovery that ran twice —
/// because the operator restarted twice, or because the first restart also crashed — is the case
/// that would break it if the second run treated the first run's work as absent. The copy is taken
/// **before** the first recovery so that the third run starts from the same crashed bytes as the
/// first, which makes 「recover, recover」 and 「recover」 comparable rather than sequential.
#[test]
fn recovering_twice_does_not_witness_twice() {
    let dir = scratch("ac043_twice");
    let marker = kill_at(&dir, "applied", "after");
    assert_eq!(marker, "applied");
    let copy = scratch("ac043_twice_copy");
    copy_tree(&dir, &copy);

    let first = probe(&["recover", &dir.display().to_string()]);
    let second = probe(&["recover", &dir.display().to_string()]);
    let once = probe(&["recover", &copy.display().to_string()]);

    println!(
        "TWICE first={} second={} once={}",
        need(&first, "LEDGER_LEAVES"),
        need(&second, "LEDGER_LEAVES"),
        need(&once, "LEDGER_LEAVES")
    );
    assert_eq!(need(&first, "LEDGER_LEAVES"), "1");
    assert_eq!(
        need(&second, "LEDGER_LEAVES"),
        "1",
        "a second restart witnesses nothing new"
    );
    assert_eq!(need(&once, "LEDGER_LEAVES"), "1");
    // The second pass finds the transformation terminal (43 §7-2) and re-runs nothing.
    let row = second
        .lines()
        .find(|l| l.starts_with("RECOVERED id="))
        .unwrap_or_default()
        .to_string();
    assert!(
        row.contains("path=Terminal") && row.contains("state=Committed"),
        "the second recovery must take 43 §7-2's road: {row}"
    );
    assert_eq!(
        value(&second, "RECOVER_REFUSED"),
        None,
        "and it must not refuse"
    );
}
