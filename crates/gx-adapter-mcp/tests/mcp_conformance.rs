//! 51 §7's harness, pointed at the **third** real adapter — and at the obligation §58 added.
//!
//! 51 §7 逐語: 「各adapterクレートはこのハーネスを`#[test]`から呼び出すのみで契約テストを継承する（cedar-
//! conformance/cedar-specの手法と同型）」. This file is that call, and the whole of what an adapter's
//! author writes is `support::McpFixture`.
//!
//! # 🔴 What the third adapter tests that the second could not
//!
//! M7 hand 1 measured that the harness did not move for `gx-adapter-git` (req/99 §2-2), which is the
//! first evidence that 51 §7's 「adapter非依存」 is a property rather than a hope. What one more adapter
//! adds is a **different kind** of subject:
//!
//! * git's substrate is readable and its footprint is derivable; **this one's footprint is not**. The
//!   fingerprint's scope and the commutation's subject are two different sets here (the crate root's
//!   table), and no obligation in the harness noticed -- because none of them asks an adapter to make
//!   them the same, and none should.
//! * git's idempotence is a comparison; **this one's is a record**. 51 §7 contract 7 and L2 hold
//!   without knowing which, which is what 「adapter非依存」 has to mean if it means anything.
//! * git's `Ok(None)` is an edge case (an unborn branch); **this one's is the ordinary case** (most
//!   tools have no inverse). Contract 5 is the same obligation over a subject that is common rather
//!   than rare.
//!
//! # 🔴 K2, and why this hand is where it is re-checked
//!
//! `req/38` §58 R-4: 「共有 harness に precondition↔postcondition 突合 obligation を足し、監査手が
//! fs/git/mcp 3 adapter で再検」. The obligation is in `gx-substrate-conformance/src/laws.rs` and its
//! argument is there; what this file does is **run** it against a third adapter whose two fingerprints
//! come from two different code paths, so that a green K2 here is a measurement and not an inheritance.
//!
//! # The bound
//!
//! The seven contracts and the nine laws hold **against this fixture** — an MCP server that lives in
//! this process, single-threaded, whose three tools do what `tests/support/mod.rs` says. Nothing here
//! is a claim about a server on a socket, about a transport that reorders, about concurrent sessions,
//! or about a tool that reports success and does nothing.

mod support;

use gx_substrate_conformance::{run_all, run_contracts, run_laws, Fixture, Origin, Outcome};
use support::McpFixture;

/// All seven of 51 §7's contracts hold, and none of them is 「無い」.
#[test]
fn the_mcp_adapter_meets_every_one_of_the_seven_contracts() {
    let fixture = McpFixture::new();
    let report = run_contracts(&fixture);
    report.print("mcp (M7 hand 3)");

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

/// All nine laws hold, on a substrate none of them was written against.
#[test]
fn the_mcp_adapter_obeys_every_law_the_rulings_added() {
    let fixture = McpFixture::new();
    let report = run_laws(&fixture);
    report.print("mcp (M7 hand 3, laws)");

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
        "L1 is the law that decided this adapter's `plan` (no read at all), and K2 is §58 R-4's \
         cross obligation running against its third subject"
    );
    assert!(report.is_conformant());
    assert!(report.is_complete());
}

/// One run, both sections, and the completion condition 51 §7 makes M7 turn on.
#[test]
fn one_run_reports_sixteen_obligations_and_meets_the_completion_condition() {
    let fixture = McpFixture::new();
    let report = run_all(&fixture);
    report.print("mcp (M7 hand 3, contracts and laws)");

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
        "51 §7's completion condition needs both questions answered, and M7's own condition is this \
         assertion twice — once in `gx-adapter-git` and once here"
    );
    println!(
        "MCP_CONFORMANCE conformant={} complete={} meets_51_7={} passed={} 無い={} restorable_tools={}",
        report.is_conformant(),
        report.is_complete(),
        report.meets_51_7(),
        report.passed(),
        report.not_supplied(),
        fixture.mcp().catalogue().declared(),
    );
}

/// 🔴 The fixture is not vacuous: the catalogue declares something, and the server was reached.
///
/// A run against an **empty** catalogue would answer `Ok(None)` for every `invert`, which makes
/// contract 4 and L3 「無い」 rather than failed — and `is_complete` would be false, so the assertions
/// above would catch it. This probe closes the other direction: a fixture whose server was never
/// touched at all would also satisfy 「no failures」, and 「conformant」 about a run in which nothing
/// happened is §30's disease.
#[test]
fn the_run_reached_the_server_and_the_catalogue_was_not_empty() {
    let fixture = McpFixture::new();
    let report = run_all(&fixture);

    let calls = fixture.server().calls();
    let reads = fixture.server().reads();
    println!(
        "MCP_FIXTURE_NON_VACUOUS calls={calls} reads={reads} restorable={} checks={}",
        fixture.mcp().catalogue().declared(),
        report.checks.len()
    );
    assert!(
        calls > 0,
        "no tool call reached the server in sixteen obligations"
    );
    assert!(reads > 0, "no resource was read in sixteen obligations");
    assert_eq!(
        fixture.mcp().catalogue().declared(),
        1,
        "the fixture declares exactly one restorable tool, so contract 5's `Ok(None)` subject and \
         contract 4's round trip are two different tools rather than two readings of one"
    );
    assert_eq!(
        fixture.server().unmatched_admissions(),
        0,
        "a call arrived with an `Admitted` naming a different delta"
    );
}

/// 🔴 **The retry sends nothing**, which is the only thing 51 §7 contract 7 can mean here.
///
/// This probe exists because the battery found its absence. Mutation (g) of `tools/verify_m7h3.sh`
/// removes `apply`'s question to the [`gx_adapter_mcp::CallLog`] and **the sixteen obligations stayed
/// green** — because this fixture's write tool is idempotent *in effect*, so a second call leaves the
/// resource where the first did and contract 7's digest comparison sees nothing. That is the whole
/// reason `log.rs` exists: on an effect substrate, 「the state did not move」 and 「no second effect
/// happened」 are different facts, and only the second one is 43 T-10c's.
///
/// So the measurement is the **counter**. Two applications of one delta, one call.
#[test]
fn the_retry_of_one_delta_sends_one_call() {
    let fixture = McpFixture::new();
    let adapter = fixture.adapter();
    let locator = fixture.locator();
    let pre = adapter.snapshot(&locator).expect("the server answers");
    let delta = adapter.plan(&fixture.intent(), &pre).expect("plan");

    let first = adapter.apply(&delta).expect("the first apply");
    let after_one = fixture.server().calls();
    let second = adapter.apply(&delta).expect("the retry");
    let after_two = fixture.server().calls();

    println!(
        "MCP_RETRY calls_after_first={after_one} calls_after_retry={after_two} log={} \
         digest_equal={}",
        fixture.log().len(),
        first.resulting_digest() == second.resulting_digest()
    );
    assert_eq!(after_one, 1, "the first apply made {after_one} calls");
    assert_eq!(
        after_two, 1,
        "the retry sent a second effect. 43 T-10c's recovery is 「run the same delta again」, and on a \
         substrate whose deltas declare no state the only way that is safe is the record"
    );
    assert_eq!(fixture.log().len(), 1, "one delta, one record");
    assert_eq!(
        first.resulting_digest(),
        second.resulting_digest(),
        "L2: the retry reported a different observation"
    );
}

/// **M4H3-9's second number**, now with a third row: what it costs an adapter's author to inherit 51 §7.
///
/// req/72 §2 left the validity of the eleven-method `Fixture` open until a second adapter existed;
/// req/99 §2-2 printed the second. This is the third, and all three come from their sources rather than
/// from an estimate.
#[test]
fn the_fixture_implementation_is_measured_beside_the_first_two_adapters() {
    let here = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crates = here.parent().expect("crates/");
    let read = |path: &str| std::fs::read_to_string(crates.join(path)).expect("readable");

    let measure = |source: &str, needle: &str| -> (usize, usize) {
        let start = source
            .find(needle)
            .unwrap_or_else(|| panic!("{needle} is not in this source"));
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

    let (mcp_lines, mcp_methods) = measure(
        &read("gx-adapter-mcp/tests/support/mod.rs"),
        "impl Fixture for McpFixture",
    );
    let (git_lines, git_methods) = measure(
        &read("gx-adapter-git/tests/support/mod.rs"),
        "impl Fixture for GitFixture",
    );
    let (fs_lines, fs_methods) = measure(
        &read("gx-adapter-fs/tests/support/mod.rs"),
        "impl Fixture for FsFixture",
    );
    println!(
        "FIXTURE_IMPL mcp={mcp_lines} lines/{mcp_methods} methods  git={git_lines} \
         lines/{git_methods} methods  fs={fs_lines} lines/{fs_methods} methods  OF_11"
    );
    assert!(
        mcp_methods >= 5,
        "the five required methods of `Fixture` are what 51 §7 costs an adapter author"
    );
    assert!(
        mcp_methods <= 11,
        "eleven is the whole trait; an adapter cannot implement more of it than it has"
    );
}

/// 🔴 The harness still names no adapter — now including this one's vocabulary.
///
/// req/99 §2-2 measured it for git's words. A third adapter is a third chance for the harness to have
/// grown a branch to make a subject pass, and the words this one would have grown are not git's.
///
/// The scan is over **code lines**: a harness's documentation names adapters by design (「`gx-adapter-fs`
/// (hand 4) is the first real one」), and 「discusses」 and 「depends on」 are different facts.
#[test]
fn the_shared_harness_names_no_adapter_of_this_kind_either() {
    let harness = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("gx-substrate-conformance/src");
    let mut scanned = 0usize;
    for name in ["lib.rs", "contracts.rs", "laws.rs"] {
        let source = std::fs::read_to_string(harness.join(name)).expect("the harness is readable");
        scanned += 1;
        let code: String = source
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !(t.starts_with("//") || t.starts_with("///") || t.starts_with("//!"))
            })
            .collect::<Vec<_>>()
            .join("\n");
        // 🔴 The list is this adapter's **vocabulary**, not its English. `resource` and `server` are
        // in the harness's code already — inside L6's message about 「two applications of one change to
        // one resource」, which is DPO's word and predates this crate by three milestones. A scan that
        // banned them would be measuring the language rather than the dependency, and would have to be
        // weakened the first time it fired, which is how a gate becomes decorative.
        for word in [
            "mcp",
            "Mcp",
            "tool_",
            "ToolCall",
            "ToolTransport",
            "transport",
            "catalogue",
            "SubstrateKind::",
        ] {
            assert!(
                !code.contains(word),
                "gx-substrate-conformance/src/{name} names {word:?} in code: 51 §7 asks for an \
                 「adapter非依存の共有テストハーネス」 and a harness that knows its third subject is not one"
            );
        }
    }
    println!("HARNESS_FILES_SCANNED={scanned} MCP_WORDS_IN_CODE=0");
    assert_eq!(scanned, 3);
}
