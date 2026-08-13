//! 51 §8.1's E2E, in real binaries and real processes.
//!
//! 51 §8.1 逐語: 「integration層でも同等テストを持つが、E2Eでは**実バイナリ・実プロセスで検証する点
//! が異なる**」. `tests/ac_043.rs` runs the criterion's thirty trials; this suite is the narrative —
//! the whole of a commit's life across **three processes**, and the one AC hand 4 could only measure
//! from inside one.
//!
//! # What hand 4 could not do, and why it matters
//!
//! AC-034 asks for 「別プロセスで対象ファイルへ書き込みを行い」 and hand 4's report says plainly what
//! it did instead: 「🔴 **注入は「別プロセス」ではない**——本 crate は adapter を 1 本も持たない
//! (N-13)ので file も process も無く、in-memory の世界を engine が知らない handle 経由で書き換える
//! thread にした…**失われているのは process 境界**(51 §8.1 の E2E=手 5)」. The substrate here is a
//! **file**, so a second process can reach it, and the injection is the sentence as written.
//!
//! An in-memory world plus a thread is the same *claim* with a weaker *witness*: a thread shares the
//! address space, so a CAS that agreed for a reason having nothing to do with durability would still
//! pass. Across a process boundary the only channel is the filesystem, which is the channel 42 §3.5
//! says a fingerprint is about.

mod support;

use support::{kill_at, need, probe, scratch, value};

/// 🔴 **AC-034's process boundary**, recovered.
///
/// The pipeline runs in one process; at T-10a's fresh read a **second** process rewrites the world
/// and exits; the first process then compares `Fingerprint₁` with `Fingerprint₀`, finds them
/// different and aborts. Nothing is applied — the world still holds what the second process wrote,
/// not what the transformation intended — and nothing is witnessed.
#[test]
fn ac_034_across_a_real_process_boundary_aborts_without_applying() {
    let dir = scratch("e2e_race");
    let out = probe(&["run", &dir.display().to_string(), "race", "after"]);
    println!("E2E_RACE {}", need(&out, "COMMITTED"));
    println!(
        "E2E_RACE INJECTED_BY_PROCESS={} WORLD={} LEDGER_LEAVES={} TAIL={}",
        need(&out, "INJECTED_BY_PROCESS"),
        need(&out, "WORLD"),
        need(&out, "LEDGER_LEAVES"),
        need(&out, "JOURNAL_TAIL")
    );

    assert_eq!(need(&out, "INJECTED_BY_PROCESS"), "true");
    assert_eq!(
        need(&out, "COMMITTED"),
        "Aborted(PreconditionChanged)",
        "T-10a saw a world it did not move"
    );
    assert_eq!(
        need(&out, "WORLD"),
        "\"elsewhere\"",
        "🔴 INV-S7: the goal never reached the substrate, so the other process's bytes are still there"
    );
    assert_eq!(need(&out, "LEDGER_LEAVES"), "0", "INV-S4");
    let tail = need(&out, "JOURNAL_TAIL");
    assert!(
        !tail.contains("ApplyStarted"),
        "E-M5-1's record is the proof that no apply was even announced: {tail}"
    );
    assert!(tail.contains("Aborted"), "{tail}");
}

/// The whole of 51 §8.1, end to end: a killed process, a restart that recovers, and a third process
/// that reads the result.
///
/// 51 §8.1's four steps, each in its own process:
///
/// 1. 「`gx submit`→`gx verify`→`gx commit --async`でCommitting区間に入る」 — `crash_probe run`, armed
///    at the third injection point.
/// 2. 「`kill -9`を実プロセスに送信する」 — the parent's `SIGKILL`, after the child says it is there.
/// 3. 「プロセスを再起動し、43 §7リカバリ手順が自動実行されることを確認する」 — `crash_probe recover`,
///    which runs 43 §7 as the first thing it does.
/// 4. 「`ledger`entry重複0件・`gx log`出力の整合性・receiptの再構成成功を確認」 — one leaf, a journal
///    that agrees with it, and a **re-issued receipt that verifies offline** against the ledger's
///    own root.
///
/// The fourth process is the audit: a `recover` that finds nothing left to do (43 §7-2) and reports
/// the same ledger. 「recovery が冪等である」 is what makes a restart safe to run twice, which is the
/// only way an operator can use it.
#[test]
fn the_51_8_1_scenario_runs_across_four_processes() {
    let dir = scratch("e2e_51_8_1");
    // 1 + 2.
    let marker = kill_at(&dir, "applied", "after");
    assert_eq!(marker, "applied");
    let crashed_world = std::fs::read_to_string(dir.join("world")).expect("the world survives");
    assert_eq!(crashed_world, "after", "the apply landed before the signal");

    // 3.
    let recovered = probe(&["recover", &dir.display().to_string()]);
    // 4.
    let audit = probe(&["recover", &dir.display().to_string()]);

    println!(
        "E2E_811 RECOVERED={} AUDIT={} LEAVES={}/{} AGREES={}/{}",
        recovered
            .lines()
            .find(|l| l.starts_with("RECOVERED id="))
            .unwrap_or(""),
        audit
            .lines()
            .find(|l| l.starts_with("RECOVERED id="))
            .unwrap_or(""),
        need(&recovered, "LEDGER_LEAVES"),
        need(&audit, "LEDGER_LEAVES"),
        need(&recovered, "LEDGER_AGREES"),
        need(&audit, "LEDGER_AGREES"),
    );
    println!("E2E_811 CHECKS={:?}", value(&recovered, "REISSUED_CHECKS"));

    assert!(recovered.contains("path=ApplyWasAnnounced"));
    assert!(recovered.contains("state=Committed"));
    assert_eq!(need(&recovered, "LEDGER_LEAVES"), "1");
    assert_eq!(need(&recovered, "LEDGER_AGREES"), "true");
    // 「receiptの再構成成功」 -- and not merely that a receipt exists.
    let checks = need(&recovered, "REISSUED_CHECKS");
    assert!(
        checks.contains("inclusion: Verified") && checks.contains("canonical_cid: true"),
        "the re-issued receipt has to verify against the ledger it names: {checks}"
    );
    assert_eq!(value(&recovered, "REISSUED_CHECKS_REFUSED"), None);

    // The audit process: nothing left to do, same ledger.
    assert!(
        audit.contains("path=Terminal"),
        "43 §7-2 on the second restart"
    );
    assert_eq!(
        need(&audit, "LEDGER_LEAVES"),
        "1",
        "「ledger entry重複0件」"
    );
    assert_eq!(need(&audit, "LEDGER_AGREES"), "true");
    assert_eq!(
        std::fs::read_to_string(dir.join("world")).expect("the world survives"),
        "after"
    );
}
