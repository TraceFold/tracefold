//! 51 §7's harness, pointed at the first real adapter.
//!
//! 51 §7 逐語: 「各adapterクレートはこのハーネスを`#[test]`から呼び出すのみで契約テストを継承する（cedar-
//! conformance/cedar-specの手法と同型）」. This file is that call, and the whole of what an adapter's
//! author writes is `support::FsFixture`.
//!
//! # 「落ちた」 と 「測っていない」 は別の答えである (**§31 M4H3-4 (b)**)
//!
//! > 「`is_conformant`(失敗 0)と **`is_complete`(未測定 0)を分離**。総合判定=両方 true。M7 の git/mcp が
//! > 別 subject を供給する時に「落ちた」と「測っていない」を区別できる語彙が要る」
//!
//! Hand 4 was the hand that needed it, with three unimplemented methods. Hand 5 supplied two of the
//! three and hand 6 supplies the last one, so this is the hand where the second question changes its
//! answer:
//!
//! | | hand 4 | hand 5 | hand 6 |
//! |---|---:|---:|---:|
//! | contracts passed | 3 | 6 | **7** |
//! | contracts 「無い」 | 4 | 1 (commutation) | **0** |
//! | laws passed | 4 | 7 | **8** |
//! | laws 「無い」 | 4 | 1 (L6) | **0** |
//! | `is_conformant` / `is_complete` | true / false | true / false | **true / true** |
//!
//! 🔴 **`meets_51_7` is true for the first time in this project.** 51 §7's completion condition is
//! 「各adapterは上記7契約すべてに合格しない限りM4/M7完了条件を満たさない」, and until this hand no adapter
//! had *run* all seven, let alone passed them. What the sentence means is bounded and the bound is
//! worth saying out loud: it is 「the seven contracts and the nine laws, against **this** fixture, on
//! a tmpfs, single-threaded」. It is not a statement about filesystems, about durability (`fsync` on a
//! tmpfs is effectively free) or about the git and mcp adapters, which supply their own subjects in M7.

mod support;

use gx_substrate_conformance::{run_all, run_contracts, run_laws, Origin, Outcome};
use support::FsFixture;

/// All seven of 51 §7's contracts hold, and none of them is 「無い」.
#[test]
fn the_fs_adapter_meets_every_one_of_the_seven_contracts() {
    let fixture = FsFixture::new();
    let report = run_contracts(&fixture);
    report.print("fs (M4 hand 6)");

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
        "51 §7 counts to seven and these have no subject: {unmeasured:?}. Hand 5 left one \
         (commutation); hand 6 owes it"
    );
    assert_eq!(report.passed(), 7);
    assert!(
        report.is_conformant(),
        "nothing measurable contradicted 51 §7"
    );
    assert!(
        report.is_complete(),
        "every contract was run, which is the half of 51 §7's completion condition that 「無い」 \
         used to hold open"
    );
}

/// All nine laws hold, including L6, which needed the seventh method to have a subject at all, and
/// **K2**, which §58 R-4 added after a defect walked past the other fifteen (`src/laws.rs`).
#[test]
fn the_fs_adapter_obeys_every_law_the_rulings_added() {
    let fixture = FsFixture::new();
    let report = run_laws(&fixture);
    report.print("fs (M4 hand 6, laws)");

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
        "L6 (**M4-25 採(a)**: symmetry, and `commutation(a,a)` = `Conflicts`) is the one this hand \
         supplied a subject for"
    );
    assert!(report.is_conformant());
    assert!(report.is_complete());
}

/// L7 finally has a real subject: the mock's normalisation folded `//` and nothing else.
///
/// req/72 §5.2 named this as a hole hand 3 left open -- 「L7 は mock の trivial な正規化(`//` の畳み込み
/// のみ)でしか走っていない。E-M4-12 の 5 clause…は手 4 の fs adapter が実装し、その時に L7 が初めて意味を
/// 持つ」 -- so the pairs the fixture supplies are one per clause, and this asserts the count rather
/// than trusting it.
#[test]
fn l7_runs_against_the_five_clauses_rather_than_against_one() {
    use gx_substrate_conformance::Fixture;

    let fixture = FsFixture::new();
    let spellings = fixture.equivalent_spellings();
    println!("L7_EQUIVALENT_SPELLINGS={}", spellings.len());
    assert_eq!(
        spellings.len(),
        4,
        "one pair per clause of E-M4-12 that produces two spellings of one position"
    );
    for (left, right) in spellings {
        assert_eq!(
            fixture.normalise(&left),
            fixture.normalise(&right),
            "{left:?} and {right:?}"
        );
    }
}

/// One run, both sections, and the two questions answered the same way for the first time.
///
/// 🔴 **M7 fix批, 起票 A-8** (req/38 §64, req/105 §8-2 行 2). The name said `fifteen` while the
/// assertion below said **16**: §58 R-4 added K2 and the number in the name stopped being true
/// without anything going red. A name that states a count is a declaration, and 「宣言が何とも
/// 比較されていない」 is the disease `floor_doubt` exists for — this is its smallest form, one
/// identifier wide. The rename is the fix; what keeps it from happening again is that the count is
/// asserted three lines down, so the name is now the only unchecked copy and a reader who compares
/// them has both in view.
#[test]
fn one_run_reports_sixteen_obligations_and_meets_the_completion_condition() {
    let fixture = FsFixture::new();
    let report = run_all(&fixture);
    report.print("fs (M4 hand 6, contracts and laws)");

    assert_eq!(report.checks.len(), 16);
    assert_eq!(report.of(Origin::Contract).len(), 7);
    assert_eq!(report.of(Origin::Law).len(), 9);
    assert_eq!(report.failed(), 0);
    assert_eq!(
        report.not_supplied(),
        0,
        "「無い」 went 8 (hand 4) -> 2 (hand 5) -> 0, and the last two were both `commutation`"
    );
    assert_eq!(report.passed(), 16);
    assert!(report.is_conformant());
    assert!(report.is_complete());
    assert!(
        report.meets_51_7(),
        "51 §7's completion condition needs both questions answered, and this is the assertion \
         that says so rather than a sentence somebody has to remember"
    );
    println!(
        "FS_CONFORMANCE conformant={} complete={} meets_51_7={} passed={} 無い={}",
        report.is_conformant(),
        report.is_complete(),
        report.meets_51_7(),
        report.passed(),
        report.not_supplied()
    );
}

/// **M4H3-9**'s first number: how much an adapter author writes to inherit 51 §7.
///
/// req/72 §2 M4H3-9 記録: 「`Fixture` が 11 method(必須 5+既定 6)になった…adapter 作者が書く量は 51 §7 の
/// 「`#[test]`から呼び出すのみ」より多い。M7 の git/mcp が同じ形で継承できるかは**未検証**(実 adapter が
/// 0 本のため)。記録+手 4 の実測待ち: fs adapter が `Fixture` を実装した時の行数が、この形の妥当性の最初
/// の数字になる」. This is that measurement, taken from the source rather than estimated.
#[test]
fn the_fixture_implementation_is_measured_rather_than_estimated() {
    let source = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/mod.rs"),
    )
    .expect("the fixture is readable");

    let start = source
        .find("impl Fixture for FsFixture")
        .expect("the fixture implements the harness trait");
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
    let impl_block = &block[..=end];
    let lines = impl_block.lines().count();
    let methods = impl_block.matches("    fn ").count();
    println!("FS_FIXTURE_IMPL_LINES={lines} METHODS={methods} OF_11");
    assert!(
        methods >= 5,
        "the five required methods of `Fixture` are what 51 §7 costs an adapter author"
    );
}
