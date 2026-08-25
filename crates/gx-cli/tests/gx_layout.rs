// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 req/88 §6.2 hand 1 ④ (sem: SEM-gx-cli-1340) — the paths of `.gx/`, created, read, damaged, recovered, **declared**.
//!
//! Seven when hand 1 wrote it; **eight** since hand 2 added `receipts/` (M6H2-1), which is where
//! `gx receipt show` reads from — 44 §1.2's "local store" (sem: SEM-gx-cli-1341), which had no implementation because
//! neither the journal nor the ledger keeps a receipt body.
//!
//! > create/read/recover a round-trip of `.gx/` layout's 7 paths (req/56 §2's 6 + `drafts/`) + **§5's
//! > per-subdir recovery declaration** (what was lost, what was regenerated) (sem: SEM-gx-cli-1342)
//!
//! req/56 §5's requirement is the one this file is really about: "dir absent = initialize / index damaged = regenerate /
//! ledger damaged = tail truncate…+ **must always declare what was lost and what was regenerated**" (sem: SEM-gx-cli-1343). A recovery that repaired
//! everything and said nothing would pass a round-trip test and fail the requirement, so every probe
//! below asserts the **report** and not only the tree.

use std::path::{Path, PathBuf};

use gx_cli::layout::{Layout, Nature, Recovery, Shape, GX_PATHS, LAYOUT_VERSION};

/// An empty directory under the cargo target directory.
///
/// `CARGO_TARGET_TMPDIR` rather than `std::env::temp_dir`, the reason gx-engine and gx-log both
/// write out: on this project's WSL2 setup `/tmp` is cleared while the machine sits idle, and a
/// suite whose fixtures evaporate between runs reports housekeeping as a failure. Cleared on entry,
/// not on exit, so a failing test leaves its tree to be read.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

/// Every declared path present after `create`, and the version stamped.
#[test]
fn create_makes_every_declared_path_and_stamps_the_version() {
    let project = scratch("layout_create");
    let layout = Layout::create(&project).expect("create");

    let mut present = Vec::new();
    for path in GX_PATHS {
        let full = layout.join(path.rel);
        // 🔴 **DR-43-5 (2) / DR-43-7 (1)** — two of the ten rows are declared and deliberately
        // not created (`Layout::create` skips them, and says why). They are counted separately so
        // that "eight were made" and "ten are declared" are two numbers a reader can see.
        if path.shape == Shape::Pattern || path.nature == Nature::Transient {
            continue;
        }
        let ok = match path.shape {
            Shape::Dir => full.is_dir(),
            Shape::File => full.is_file(),
            Shape::Pattern => unreachable!("skipped above"),
        };
        if ok {
            present.push(path.rel);
        }
    }
    println!(
        "GX_PATHS={} PRESENT={} ({present:?})",
        GX_PATHS.len(),
        present.len()
    );
    assert_eq!(
        GX_PATHS.len(),
        11,
        "req/56 §2's six, plus M6-01 adopted (a)'s drafts/ and M6H2-1's receipts/ (sem: SEM-gx-cli-1344), plus DR-43-5 (2)'s LOCK and DR-43-7 (1)'s ledger/*.torn.* (req/38 §156 ruling 3), plus R13's repair/ (req/244 H-01: where `gx repair --yes` files the report it printed, so that a run whose stdout died leaves the fact behind)"
    );
    assert_eq!(
        present.len(),
        9,
        "one of the nine created paths was not created"
    );

    // 🔴 **R12 / `req/242` H-01** — the declaration is **complete** on the first write.
    //
    // It used to be `LAYOUT_VERSION` alone, and the `journal_format` line arrived later, from the
    // first writer verb to touch the project (`Layout::declare_journal_format`, reached through
    // `session::anchor_accepting`). That road is what `req/242` H-01 measured rewriting operator
    // bytes and re-arming R7's digest detector, so it is gone, and the framing is declared here —
    // where it is not a guess, because `DeclarationWriter::initialise` creates the journal in the
    // same call and a journal this binary creates is chained.
    // 🔴 **R30 / `req/372` M-02 (`req/38` §240 ruling 3)** — the framing this build creates,
    // named once, with the declaration's value and the marker's bytes both derived from it.
    // M-02 versioned the record vocabulary, so the value on that line is `chained-v2` and the
    // eight bytes below are `GXJRNL02`. What did **not** change is the claim or its strength:
    // this is still an exact comparison against a pinned format, so a build that created journals
    // in some other framing still turns this probe red.
    const CREATED: gx_engine::JournalFormat = gx_engine::JournalFormat::ChainedV2;

    let version = std::fs::read_to_string(layout.join("VERSION")).expect("VERSION");
    println!("VERSION_AFTER_CREATE={version:?}");
    assert_eq!(
        version,
        format!("{LAYOUT_VERSION}\njournal_format={}\n", CREATED.kind()),
        "a project this binary creates says what it is from its first byte"
    );

    // 🔴 **R12 / `req/242` H-01 (d)** — and the journal exists, carrying the marker, before
    // any engine has opened. `GX_PATHS` has no row for it (`ledger/` is the declared row), so it is
    // asserted here rather than counted above.
    let journal = layout.join("ledger").join("journal");
    let bytes = std::fs::read(&journal).expect("the journal a new project gets");
    println!("JOURNAL_AFTER_CREATE={} bytes", bytes.len());
    assert_eq!(
        bytes,
        CREATED
            .marker()
            .expect("a created journal carries a marker")
            .to_vec(),
        "the marker and nothing else: this is the only road that creates this file"
    );
}

/// `create` twice is `create` once. Every command may call it, so it has to be safe to.
#[test]
fn create_is_idempotent_and_does_not_overwrite_config() {
    let project = scratch("layout_idempotent");
    let layout = Layout::create(&project).expect("first create");
    let config = layout.join("config.toml");
    std::fs::write(&config, "# the operator edited this\n").expect("write");

    Layout::create(&project).expect("second create");
    let after = std::fs::read_to_string(&config).expect("read");
    println!("CONFIG_AFTER_SECOND_CREATE={after:?}");
    assert_eq!(
        after, "# the operator edited this\n",
        "a second create must not overwrite a file the operator owns"
    );
}

/// req/56 §4: `.gx/` keeps itself out of the user's history without editing the user's `.gitignore`.
///
/// "zero interference" (sem: SEM-gx-cli-1345) is req/56 §1's whole first sentence, and a tool that rewrote the project's own
/// `.gitignore` to achieve it would be interfering in order to claim it does not.
#[test]
fn the_directory_ignores_itself_and_leaves_the_project_alone() {
    let project = scratch("layout_gitignore");
    let layout = Layout::create(&project).expect("create");
    let inner = std::fs::read_to_string(layout.join(".gitignore")).expect("the inner .gitignore");
    println!(
        "INNER_GITIGNORE_HAS_STAR={} PROJECT_GITIGNORE_EXISTS={}",
        u8::from(inner.lines().any(|l| l.trim() == "*")),
        u8::from(project.join(".gitignore").exists())
    );
    assert!(
        inner.lines().any(|l| l.trim() == "*"),
        "req/56 §4: \"default = gitignore the entirety of `.gx/`\" (sem: SEM-gx-cli-1346)"
    );
    assert!(
        !project.join(".gitignore").exists(),
        "zero interference (req/56 §1; sem: SEM-gx-cli-1347): the project's own files are not touched"
    );
}

/// 🔴 `open` refuses a directory written by a newer layout. Fail-closed.
///
/// This is what `.gx/VERSION` is **for** (req/56 §2: \"layout version (for migration)\"; sem: SEM-gx-cli-1348). 47 §4 makes
/// journal-schema compatibility an upgrade precondition, and E-M5-13 has just moved that schema —
/// so a binary that opened a newer directory "best effort" (sem: SEM-gx-cli-1349) would be reading a `Planned` record with
/// fields it does not know, which is the exact case the condition exists for.
#[test]
fn open_refuses_a_newer_layout_version() {
    let project = scratch("layout_version");
    let layout = Layout::create(&project).expect("create");
    std::fs::write(layout.join("VERSION"), format!("{}\n", LAYOUT_VERSION + 1)).expect("write");

    let err = Layout::open(&project).expect_err("a newer layout must be refused");
    println!("NEWER_LAYOUT_REFUSED={err}");
    assert!(
        matches!(err, gx_cli::Error::Layout { .. }),
        "the refusal has to name the version rather than look like an I/O failure: {err:?}"
    );

    std::fs::write(layout.join("VERSION"), "not a number\n").expect("write");
    let err = Layout::open(&project).expect_err("a malformed version must be refused");
    // 🔴 **R9 / `req/236` H-04** — ~~`Error::Malformed`~~ → `Error::Declaration`.
    //
    // "present but broken" is still a third answer and not "absent" (sem: SEM-gx-cli-1350); what
    // changed is that it is no longer folded into the variant that carries `VALIDATION_ERROR`.
    // `req/236` H-04 measured what the fold cost: five byte shapes an ordinary editor produces
    // stopped every verb — the diagnostic one included — with "the request is not one this binary
    // can attempt", over a request that was `gx repair` and a file nobody had asked about. Four of
    // those five are read correctly now; this line is the fifth kind, and it carries a `form` and a
    // `remedy` that name the two correct lines.
    assert!(
        matches!(err, gx_cli::Error::Declaration { .. }),
        "\"present but broken\" is a third answer, not \"absent\" (sem: SEM-gx-cli-1350): {err:?}"
    );
    let problem = err.problem();
    println!("UNREADABLE_DECLARATION={problem}");
    assert_eq!(
        problem["gx_code"], "DECLARATION_UNREADABLE",
        "and it has a word of its own rather than the caller's fault: {problem}"
    );
    assert_eq!(err.exit_code(), 1, "no new exit number (req/38 §148)");

    // 🔴 And the shapes that are not text at all, and the one with no version line — on all of
    // which the **reader's** door still opens, which is the half `req/236` H-04 is really about.
    for (what, bytes) in [
        ("bytes that are not text", vec![0x80u8, 0x81, 0x82]),
        (
            "a file with no version line",
            b"journal_format=chained\n".to_vec(),
        ),
        ("an empty file", Vec::new()),
    ] {
        std::fs::write(layout.join("VERSION"), &bytes).expect("write");
        let err = Layout::open(&project).expect_err("a declaration that will not read is refused");
        assert!(
            matches!(err, gx_cli::Error::Declaration { .. }),
            "{what}: {err:?}"
        );
        let (_, fault) = Layout::open_reporting(&project).expect("the directory still opens");
        assert!(
            matches!(fault, Some(gx_cli::Error::Declaration { .. })),
            "{what}: the diagnosis opens and hands the fault back as a value: {fault:?}"
        );
    }
}

/// 🔴 **req/56 §5, per subdirectory, with the declaration.**
///
/// Every one of the seven is deleted, one at a time, from a fresh tree, and what `recover` *says*
/// about it is asserted alongside what it did. The five outcomes are not interchangeable:
/// `Regenerated` claims nothing was lost, `Lost` claims something was, `Delegated` says another
/// layer owns the repair, and `Initialised` says the file is back at its default. Collapsing them
/// into "fixed" is req/29 §4's "don't give skip and pass the same face" (sem: SEM-gx-cli-1351) at directory scale.
#[test]
fn recovery_declares_per_subdirectory_what_was_lost_and_what_was_rebuilt() {
    let expected = [
        // 🔴 The two DR-43-5 (2) / DR-43-7 (1) rows are exercised by
        // `the_two_declared_but_untouched_paths_are_reported_and_not_repaired` below: they cannot
        // be deleted from a fresh tree because `create` never makes them.
        ("ledger", Recovery::Delegated),
        ("checkpoints", Recovery::Lost),
        ("evidence", Recovery::Lost),
        ("index", Recovery::Regenerated),
        ("drafts", Recovery::Lost),
        ("receipts", Recovery::Lost),
        // 🔴 **R13 / `req/244` H-01** — `.gx/repair/` is `Nature::Source` and answers `Lost`, for
        // `receipts/`'s reason exactly: nothing re-derives a record of a run that has already
        // happened. What it is **not** is project state — see `GX_PATHS`' own note on why it is
        // absent from `Layout::logged` and `Layout::established`.
        ("repair", Recovery::Lost),
        ("config.toml", Recovery::Lost),
        // 🔴 **R12 / `req/242` H-01** — the two `Nature::Meta` rows are `Lost` and not
        // `Initialised`, because this walk no longer writes them.
        //
        // `Layout::recover`'s `Nature::Meta` arm was the workspace's **fourth** road into
        // `.gx/VERSION` and `.gx/config.toml`, and the one no audit had found because no verb
        // reaches it. R7 binds the declaration's digest into the signed head, so re-creating the
        // file is taking a detector off; the road that may take it off is `gx repair --yes`, which
        // says that it did. `Lost` is the honest cell and `victim.exists()` below is asserted
        // against it — see the loop.
        ("VERSION", Recovery::Lost),
    ];
    assert_eq!(
        expected.len() + 2,
        GX_PATHS.len(),
        "every declared path needs a declared recovery outcome; the two `Untouched` rows have their own probe because `create` does not make them"
    );

    for (rel, want) in expected {
        let project = scratch(&format!("layout_recover_{}", rel.replace('.', "_")));
        let layout = Layout::create(&project).expect("create");
        let victim = layout.join(rel);
        if victim.is_dir() {
            std::fs::remove_dir_all(&victim).expect("remove the directory");
        } else {
            std::fs::remove_file(&victim).expect("remove the file");
        }

        let report = layout.recover().expect("recover");
        let got = report.of(rel).expect("every path has a row");
        let changed = report.changed();
        println!(
            "RECOVER {rel}: {got:?}  CHANGED_ROWS={} {changed:?}",
            changed.len()
        );

        assert_eq!(got, want, "{rel}'s recovery outcome");
        // 🔴 **R12 / `req/242` H-01** — a `Nature::Meta` file this walk found missing is
        // still missing afterwards, and that is the assertion. `Recovery::Lost` that left the file
        // behind would be the silent write under another name.
        if want == Recovery::Lost && !victim.is_dir() {
            assert!(
                !victim.exists(),
                "{rel} was reported Lost and this walk must not have written it back"
            );
        } else {
            assert!(victim.exists(), "{rel} is usable again after recovery");
        }
        assert_eq!(
            changed.len(),
            1,
            "exactly one row changed, and the report says which: {changed:?}"
        );
        assert_eq!(
            report.rows().len(),
            GX_PATHS.len(),
            "one row per declared path"
        );
    }
}

/// A complete tree recovers to seven `Intact` rows and an **empty** changed list.
///
/// The negative control for the probe above: if `recover` reported repairs it did not make, the
/// declarations there would be meaningless. An empty `changed()` is "nothing was missing" and it has
/// to be distinguishable from "nothing was checked" (sem: SEM-gx-cli-1352), which is why `rows()` is still seven.
#[test]
fn a_complete_directory_recovers_to_nothing_changed() {
    let project = scratch("layout_recover_intact");
    let layout = Layout::create(&project).expect("create");
    let report = layout.recover().expect("recover");
    println!(
        "INTACT_ROWS={} CHANGED={}",
        report.rows().len(),
        report.changed().len()
    );
    assert_eq!(report.rows().len(), 11);
    // 🔴 `Untouched` is neither "lost" nor "regenerated", so it is **not** in `changed()` — see
    // `RecoveryReport::changed`. It is still a row, and `of` still answers about it, which is the
    // difference between a fact that is reported and a fact that is shouted.
    assert!(report.changed().is_empty());
    assert!(report
        .rows()
        .iter()
        .filter(|(rel, _)| *rel != "LOCK" && *rel != "ledger/*.torn.*")
        .all(|(_, k)| *k == Recovery::Intact));
    assert_eq!(report.of("LOCK"), Some(Recovery::Untouched));
    assert_eq!(report.of("ledger/*.torn.*"), Some(Recovery::Untouched));
}

/// 🔴 **DR-43-5 (2) / DR-43-7 (1)** — the two rows that are declared and are not project state.
///
/// `create` makes neither, `recover` repairs neither, and both are reported. The third assertion is
/// the one that matters: a `LOCK` an operator finds in a tree is **not** created by `gx init`, so a
/// file being there means a process is (or was) writing, which is the only reading that makes the
/// file useful for diagnosis at all.
#[test]
fn the_two_declared_but_untouched_paths_are_reported_and_not_repaired() {
    let project = scratch("layout_untouched");
    let layout = Layout::create(&project).expect("create");
    assert!(!layout.join("LOCK").exists(), "create makes no lock file");
    assert!(
        !layout.join("ledger/*.torn.*").exists(),
        "and no file whose name is a rule"
    );

    let report = layout.recover().expect("recover");
    println!(
        "UNTOUCHED lock={:?} torn={:?}",
        report.of("LOCK"),
        report.of("ledger/*.torn.*")
    );
    assert_eq!(report.of("LOCK"), Some(Recovery::Untouched));
    assert_eq!(report.of("ledger/*.torn.*"), Some(Recovery::Untouched));
    assert!(!layout.join("LOCK").exists(), "recover makes no lock file");

    // A quarantine copy that is already there is left exactly where it is: it is evidence, and
    // this is the assertion that says no `gx` verb tidies it away (DR-43-7 (1)'s open question is
    // *who owns the lifetime*, and the answer today is "nobody removes one").
    let torn = layout.join("ledger").join("journal.ledger.torn.174-193");
    std::fs::write(&torn, b"bytes that would not replay").expect("write the quarantine copy");
    layout.recover().expect("recover again");
    assert!(torn.exists(), "a quarantine copy is evidence, not litter");
}

/// 🔴 The **whole** directory being absent is "initialization" (sem: SEM-gx-cli-1353) and not seven losses.
///
/// req/56 §5's first rule is "dir absent = initialize" (sem: SEM-gx-cli-1354). `create` is the operation for it, and this probe is
/// what says the two rules do not overlap: a missing `.gx/` is a project that has not used gx yet,
/// and reporting it as data loss would teach an operator to distrust the word.
#[test]
fn an_absent_directory_is_initialisation_rather_than_loss() {
    let project = scratch("layout_absent");
    assert!(!Layout::path_for(&project).exists());
    let layout = Layout::create(&project).expect("create");
    let report = layout.recover().expect("recover");
    println!("FRESH_CHANGED={}", report.changed().len());
    assert!(
        report.changed().is_empty(),
        "a directory that was just created is intact, not recovered"
    );
}

/// The natures in the table are req/56 §2's third column, and the counts are stated so that a row
/// silently re-classified is visible.
///
/// Losing a `Source` path loses data and losing a `Derived` one does not; that difference is what
/// decides every outcome in the recovery probe above, so it is asserted rather than assumed.
#[test]
fn the_natures_are_the_ones_req56_assigns() {
    let mut source = 0;
    let mut derived = 0;
    let mut countersigned = 0;
    let mut meta = 0;
    let mut transient = 0;
    for p in GX_PATHS {
        match p.nature {
            Nature::Source => source += 1,
            Nature::Derived => derived += 1,
            Nature::Countersigned => countersigned += 1,
            Nature::Meta => meta += 1,
            Nature::Transient => transient += 1,
        }
    }
    println!(
        "NATURE source={source} derived={derived} countersigned={countersigned} meta={meta}          transient={transient}"
    );
    assert_eq!(
        source, 6,
        "ledger/, evidence/ (req/56 §2), drafts/ (M6-01 adopted (a); sem: SEM-gx-cli-1355), receipts/ (M6H2-1), ledger/*.torn.* — a quarantine copy is bytes nothing can re-derive (DR-43-7 (1)) — and repair/ (R13, req/244 H-01: a record of a run that already happened is not re-derivable either)"
    );
    assert_eq!(
        derived, 1,
        "index/ — \"declared as OK to delete\" (sem: SEM-gx-cli-1356)"
    );
    assert_eq!(
        countersigned, 1,
        "checkpoints/ — \"re-signature required\" (sem: SEM-gx-cli-1357)"
    );
    assert_eq!(meta, 2, "config.toml and VERSION");
    assert_eq!(
        transient, 1,
        "LOCK — a running process's exclusion, which is not data (DR-43-5 (2))"
    );
}
