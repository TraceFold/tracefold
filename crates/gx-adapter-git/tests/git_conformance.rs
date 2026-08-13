//! 51 §7's harness, pointed at the **second** real adapter — and at the question M4 could not ask.
//!
//! 51 §7 逐語: 「各adapterクレートはこのハーネスを`#[test]`から呼び出すのみで契約テストを継承する（cedar-
//! conformance/cedar-specの手法と同型）」. This file is that call, and the whole of what an adapter's
//! author writes is `support::GitFixture`.
//!
//! # 🔴 What is new here, and it is not the count
//!
//! M4 hand 3 built the harness against a **mock** and hand 6 ran it against `gx-adapter-fs`, and both
//! times the harness and its subject were written by the same hand in the same milestone. req/72 §2
//! **M4H3-9** said so in as many words: 「`Fixture` が 11 method(必須 5+既定 6)になった…**M7 の git/mcp が
//! 同じ形で継承できるかは未検証**(実 adapter が 0 本のため)」.
//!
//! This is the measurement that claim was waiting for. The git adapter's subjects are not the fs
//! adapter's in any respect that matters — the object is an entry and the **scope is a branch**, the
//! inverse is a reference reset rather than a body, the `Ok(None)` is an unborn branch rather than a
//! ceiling, and 「the substrate moved」 is a commit rather than a write — and **the harness needed no
//! change at all**. Not a line of `gx-substrate-conformance` moved for this crate. That is the first
//! evidence that 51 §7's 「adapter非依存の共有テストハーネス」 is adapter-independent rather than
//! fs-shaped, and it is worth more than the fifteen passes it produced.
//!
//! # 「落ちた」 と 「測っていない」 (**§31 M4H3-4 (b)**)
//!
//! > 「`is_conformant`(失敗 0)と **`is_complete`(未測定 0)を分離**。総合判定=両方 true。**M7 の git/mcp が
//! > 別 subject を供給する時**に「落ちた」と「測っていない」を区別できる語彙が要る」
//!
//! M4 hand 4 needed the split because three methods were unimplemented. This hand is the one the
//! ruling was actually written for — 「M7 の git/mcp が別 subject を供給する時」 — and it supplies every
//! subject, so both answers are true and `meets_51_7` with them. The vocabulary is not exercised in
//! its 「無い」 direction here; `gx-adapter-mcp` (hand 3) is where it will be.
//!
//! The bound belongs beside the claim: the seven contracts and the nine laws hold **against this
//! fixture**, on a tmpfs, single-threaded, over repositories the test created, with loose objects and
//! loose references. It is not a claim about every git repository, about concurrent pushes, about
//! packed references, or about a repository whose objects live in a pack.

mod support;

use gx_substrate_conformance::{run_all, run_contracts, run_laws, Origin, Outcome};
use support::GitFixture;

/// All seven of 51 §7's contracts hold, and none of them is 「無い」.
#[test]
fn the_git_adapter_meets_every_one_of_the_seven_contracts() {
    let fixture = GitFixture::new();
    let report = run_contracts(&fixture);
    report.print("git (M7 hand 1)");

    let failures: Vec<&gx_substrate_conformance::Check> = report
        .checks
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::Fail(_)))
        .collect();
    assert!(failures.is_empty(), "these contracts failed: {failures:?}");

    let unmeasured: Vec<&str> = report
        .checks
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::NotSupplied(_)))
        .map(|c| c.id.as_str())
        .collect();
    assert!(
        unmeasured.is_empty(),
        "51 §7 counts to seven and these have no subject: {unmeasured:?}"
    );
    assert_eq!(report.passed(), 7);
    assert!(report.is_conformant(), "nothing contradicted 51 §7");
    assert!(
        report.is_complete(),
        "every contract was run, which is the half of 51 §7's completion condition the word 「無い」 \
         holds open"
    );
}

/// All nine laws hold, on a substrate whose shape none of them was written against.
///
/// 🔴 **K2 is the one this adapter paid for.** req/99 §4's survivor (f') was a `precondition`
/// that named the entry where `apply` named the branch, and it walked through fifteen obligations
/// green; §58 R-4 put the cross obligation into the shared harness and this is where it now runs.
#[test]
fn the_git_adapter_obeys_every_law_the_rulings_added() {
    let fixture = GitFixture::new();
    let report = run_laws(&fixture);
    report.print("git (M7 hand 1, laws)");

    let failures: Vec<&gx_substrate_conformance::Check> = report
        .checks
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::Fail(_)))
        .collect();
    assert!(failures.is_empty(), "these laws failed: {failures:?}");

    let measured: Vec<&str> = report
        .checks
        .iter()
        .filter(|c| c.outcome == Outcome::Pass)
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(
        measured,
        vec!["L1", "L2", "L3", "L4", "L5", "L6", "L7", "K1", "K2"],
        "L1 is the one that decided this adapter's design: 「同一 (intent, snapshot) が substrate の \
         移動を挟んで同じ delta」 is why the branch tip is not in the payload"
    );
    assert!(report.is_conformant());
    assert!(report.is_complete());
}

/// One run, both sections, and the completion condition 51 §7 makes M7 turn on.
///
/// 🔴 **M7 fix批, 起票 A-8** (req/38 §64, req/105 §8-2 行 2). Renamed from `…fifteen…`: K2 (§58 R-4)
/// made the harness sixteen obligations, `gx-adapter-mcp` was written after that and named itself
/// `sixteen`, and these two kept a number that had stopped being true. Nothing went red, because a
/// function name is a declaration nothing compares with the artifact — the same shape as the README
/// floor before `floor_doubt`.
#[test]
fn one_run_reports_sixteen_obligations_and_meets_the_completion_condition() {
    let fixture = GitFixture::new();
    let report = run_all(&fixture);
    report.print("git (M7 hand 1, contracts and laws)");

    assert_eq!(report.checks.len(), 16);
    assert_eq!(report.of(Origin::Contract).len(), 7);
    assert_eq!(report.of(Origin::Law).len(), 9);
    assert_eq!(report.failed(), 0);
    assert_eq!(report.not_supplied(), 0);
    assert_eq!(report.passed(), 16);
    assert!(report.is_conformant());
    assert!(report.is_complete());
    assert!(
        report.meets_51_7(),
        "51 §7's completion condition needs both questions answered, and M7's own condition is \
         this assertion twice — once here and once in `gx-adapter-mcp`"
    );
    println!(
        "GIT_CONFORMANCE conformant={} complete={} meets_51_7={} passed={} 無い={}",
        report.is_conformant(),
        report.is_complete(),
        report.meets_51_7(),
        report.passed(),
        report.not_supplied()
    );
}

/// L7 runs against one pair per clause, rather than against one.
#[test]
fn l7_runs_against_the_clauses_rather_than_against_one() {
    use gx_substrate_conformance::Fixture;

    let fixture = GitFixture::new();
    let spellings = fixture.equivalent_spellings();
    println!("L7_EQUIVALENT_SPELLINGS={}", spellings.len());
    assert_eq!(
        spellings.len(),
        4,
        "one pair per clause of the crate root's `≈` that produces two spellings of one position"
    );
    for (left, right) in spellings {
        assert_eq!(
            fixture.normalise(&left),
            fixture.normalise(&right),
            "{left:?} and {right:?}"
        );
    }
}

/// 🔴 **M4H3-9's second number**: what it costs a *second* adapter's author to inherit 51 §7.
///
/// req/72 §2 recorded the first (「fs adapter が `Fixture` を実装した時の行数が、この形の妥当性の最初の
/// 数字になる」) and left the validity of the shape open until a second adapter existed. Both numbers are
/// printed here, taken from the two sources rather than estimated, so that the ratio is a measurement
/// and not an impression.
#[test]
fn the_fixture_implementation_is_measured_beside_the_first_adapters() {
    let here = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let git = std::fs::read_to_string(here.join("tests/support/mod.rs")).expect("readable");
    let fs = std::fs::read_to_string(
        here.parent()
            .expect("crates/")
            .join("gx-adapter-fs/tests/support/mod.rs"),
    )
    .expect("the fs adapter's fixture is readable");

    let measure = |source: &str, needle: &str| -> (usize, usize) {
        let start = source.find(needle).unwrap_or_else(|| {
            panic!("{needle} is not in this source");
        });
        let block = &source[start..];
        let mut depth = 0usize;
        let mut end = 0usize;
        for (i, c) in block.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let body = &block[..=end];
        (body.lines().count(), body.matches("    fn ").count())
    };

    let (git_lines, git_methods) = measure(&git, "impl Fixture for GitFixture");
    let (fs_lines, fs_methods) = measure(&fs, "impl Fixture for FsFixture");
    println!(
        "FIXTURE_IMPL git={git_lines} lines/{git_methods} methods  fs={fs_lines} lines/{fs_methods} \
         methods  OF_11"
    );
    assert!(
        git_methods >= 5,
        "the five required methods of `Fixture` are what 51 §7 costs an adapter author"
    );
    assert!(
        git_methods <= 11,
        "eleven is the whole trait; an adapter cannot implement more of it than it has"
    );
}

/// 🔴 The harness itself did not move for this adapter.
///
/// The claim of the module documentation, as a measurement: `gx-substrate-conformance` names no
/// substrate, no adapter crate and no git vocabulary. A harness that had grown a git branch to make
/// this crate pass would be a harness that is 「adapter非依存」 only until the next adapter, and 51 §7's
/// completion condition would then be measuring a suite written for its subject.
#[test]
fn the_shared_harness_names_no_adapter() {
    let harness = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("gx-substrate-conformance/src");
    let mut scanned = 0usize;
    for name in ["lib.rs", "contracts.rs", "laws.rs"] {
        let source = std::fs::read_to_string(harness.join(name)).expect("the harness is readable");
        scanned += 1;
        // Comments are where the harness is allowed to *discuss* adapters -- its documentation names
        // both by design ("`gx-adapter-fs` (hand 4) is the first real one"). What must not appear is
        // a git word in code, so the scan drops comment lines the way M6's 則 1 counters do.
        let code: String = source
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !(t.starts_with("//") || t.starts_with("///") || t.starts_with("//!"))
            })
            .collect::<Vec<_>>()
            .join("\n");
        for word in [
            "gix",
            "git",
            "branch",
            "commit",
            "refs/",
            "gx_adapter",
            "SubstrateKind::",
        ] {
            assert!(
                !code.contains(word),
                "gx-substrate-conformance/src/{name} names {word:?} in code: 51 §7 asks for an \
                 「adapter非依存の共有テストハーネス」 and a harness that knows its subject is not one"
            );
        }
    }
    println!("HARNESS_FILES_SCANNED={scanned} ADAPTER_WORDS_IN_CODE=0");
    assert_eq!(scanned, 3);
}
