//! 🔴 req/88 §6.2 手 1 ④ — the paths of `.gx/`, created, read, damaged, recovered, **declared**.
//!
//! Seven when hand 1 wrote it; **eight** since hand 2 added `receipts/` (M6H2-1), which is where
//! `gx receipt show` reads from — 44 §1.2's 「ローカルストア」, which had no implementation because
//! neither the journal nor the ledger keeps a receipt body.
//!
//! > `.gx/` layout の 7 path(req/56 §2 の 6+`drafts/`)を作る/読む/復旧する round-trip+**§5 の
//! > per-subdir 復旧申告**(何が失われ何が再生成されたか)
//!
//! req/56 §5's requirement is the one this file is really about: 「dir 不在=初期化/index 破損=再生成/
//! ledger 破損=tail truncate…+**何が失われ何が再生成されたかを必ず申告**」. A recovery that repaired
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
        let ok = match path.shape {
            Shape::Dir => full.is_dir(),
            Shape::File => full.is_file(),
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
        8,
        "req/56 §2's six, plus M6-01 採(a)'s drafts/ and M6H2-1's receipts/"
    );
    assert_eq!(present.len(), 8, "one of the eight was not created");

    let version = std::fs::read_to_string(layout.join("VERSION")).expect("VERSION");
    assert_eq!(version.trim(), LAYOUT_VERSION.to_string());
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
/// 「干渉ゼロ」 is req/56 §1's whole first sentence, and a tool that rewrote the project's own
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
        "req/56 §4: 「既定=`.gx/` 全体を gitignore」"
    );
    assert!(
        !project.join(".gitignore").exists(),
        "干渉ゼロ (req/56 §1): the project's own files are not touched"
    );
}

/// 🔴 `open` refuses a directory written by a newer layout. Fail-closed.
///
/// This is what `.gx/VERSION` is **for** (req/56 §2: 「layout version(migration 用)」). 47 §4 makes
/// journal-schema compatibility an upgrade precondition, and E-M5-13 has just moved that schema —
/// so a binary that opened a newer directory 「best effort」 would be reading a `Planned` record with
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
    assert!(
        matches!(err, gx_cli::Error::Malformed { .. }),
        "「在るが壊れている」 is a third answer, not 「無い」: {err:?}"
    );
}

/// 🔴 **req/56 §5, per subdirectory, with the declaration.**
///
/// Every one of the seven is deleted, one at a time, from a fresh tree, and what `recover` *says*
/// about it is asserted alongside what it did. The five outcomes are not interchangeable:
/// `Regenerated` claims nothing was lost, `Lost` claims something was, `Delegated` says another
/// layer owns the repair, and `Initialised` says the file is back at its default. Collapsing them
/// into 「fixed」 is req/29 §4's 「skip と pass を同じ顔にしない」 at directory scale.
#[test]
fn recovery_declares_per_subdirectory_what_was_lost_and_what_was_rebuilt() {
    let expected = [
        ("ledger", Recovery::Delegated),
        ("checkpoints", Recovery::Lost),
        ("evidence", Recovery::Lost),
        ("index", Recovery::Regenerated),
        ("drafts", Recovery::Lost),
        ("receipts", Recovery::Lost),
        ("config.toml", Recovery::Initialised),
        ("VERSION", Recovery::Initialised),
    ];
    assert_eq!(
        expected.len(),
        GX_PATHS.len(),
        "every declared path needs a declared recovery outcome"
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
        assert!(victim.exists(), "{rel} is usable again after recovery");
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
/// declarations there would be meaningless. An empty `changed()` is 「nothing was missing」 and it has
/// to be distinguishable from 「nothing was checked」, which is why `rows()` is still seven.
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
    assert_eq!(report.rows().len(), 8);
    assert!(report.changed().is_empty());
    assert!(report.rows().iter().all(|(_, k)| *k == Recovery::Intact));
}

/// 🔴 The **whole** directory being absent is 「初期化」 and not seven losses.
///
/// req/56 §5's first rule is 「dir 不在=初期化」. `create` is the operation for it, and this probe is
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
    for p in GX_PATHS {
        match p.nature {
            Nature::Source => source += 1,
            Nature::Derived => derived += 1,
            Nature::Countersigned => countersigned += 1,
            Nature::Meta => meta += 1,
        }
    }
    println!("NATURE source={source} derived={derived} countersigned={countersigned} meta={meta}");
    assert_eq!(
        source, 4,
        "ledger/, evidence/ (req/56 §2), drafts/ (M6-01 採(a)) and receipts/ (M6H2-1)"
    );
    assert_eq!(derived, 1, "index/ — 「消して良いと宣言」");
    assert_eq!(countersigned, 1, "checkpoints/ — 「再署名要」");
    assert_eq!(meta, 2, "config.toml and VERSION");
}
