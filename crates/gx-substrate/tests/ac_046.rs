//! **AC-046** — a `Box<dyn SubstrateAdapter>` satisfies `Send + Sync`, and one that would not fails
//! to compile.
//!
//! 34 逐語: 「Given: `SubstrateAdapter` trait実装。When: `Box<dyn SubstrateAdapter>`としてtrait object
//! を格納するコードをコンパイルする。Then: `Send + Sync`境界を満たしコンパイル成功する（境界を満たさない
//! 負例ではコンパイルエラーになることをtrybuildで確認）」.
//!
//! # Why there is no `trybuild`
//!
//! **M4-20 採(b)** (req/38 §28): 「AC-046 は **compile_fail doctest**(依存 0・M1 先例・51 §2 が明示
//! 許容)+`Box<dyn SubstrateAdapter>: Send+Sync` の正例 assert。trybuild 不採用=package 244 不変(表外
//! 依存を増やさない)」. 51 §2 allows either instrument -- 「`trybuild`または単純なコンパイル成功
//! アサーション」 -- and `trybuild` is not in the selection table of `req/spec/research/02`, so taking
//! it would have been a dependency chosen by an AC's example rather than by a comparison.
//!
//! The negative case therefore lives in `crates/gx-substrate/src/adapter.rs` as a `compile_fail`
//! doctest, which
//! rustdoc runs and fails if the code **compiles**. That instrument has one weakness worth naming: a
//! `compile_fail` block passes for any compile error, including a typo. What closes it is the pair --
//! the same example is written twice, once as a running doctest and once with a single field added,
//! and the two probes below assert that the pair is exactly that. If the negative example broke for
//! a reason other than the bound, the positive one would break with it.

use std::path::{Path, PathBuf};

use gx_core::{Commutation, Fingerprint, Intent, ObjectSnapshot, SubstrateKind};
use gx_substrate::{AppliedDelta, PlannedDelta, Result, SubstrateAdapter};

/// The smallest thing that is an adapter: it implements the seven methods and holds nothing.
///
/// Bodies are `unimplemented!()` on purpose. AC-046 is a statement about the **boundary**, and an
/// adapter that did something would make this file a test of that something. What is measured is
/// that a trait object built from it can be sent and shared.
struct Minimal;

impl SubstrateAdapter for Minimal {
    fn kind(&self) -> SubstrateKind {
        SubstrateKind::Fs
    }

    fn snapshot(&self, _locator: &str) -> Result<ObjectSnapshot> {
        unimplemented!("AC-046 measures the bound, not the behaviour")
    }

    fn plan(&self, _intent: &Intent, _pre: &ObjectSnapshot) -> Result<PlannedDelta> {
        unimplemented!("AC-046 measures the bound, not the behaviour")
    }

    fn precondition(&self, _snap: &ObjectSnapshot) -> Result<Fingerprint> {
        unimplemented!("AC-046 measures the bound, not the behaviour")
    }

    fn apply(&self, _delta: &PlannedDelta) -> Result<AppliedDelta> {
        unimplemented!("AC-046 measures the bound, not the behaviour")
    }

    fn invert(&self, _delta: &PlannedDelta, _pre: &ObjectSnapshot) -> Result<Option<PlannedDelta>> {
        unimplemented!("AC-046 measures the bound, not the behaviour")
    }

    fn commutation(&self, _a: &PlannedDelta, _b: &PlannedDelta) -> Result<Commutation> {
        unimplemented!("AC-046 measures the bound, not the behaviour")
    }
}

/// The bound, asked of a type by the compiler. A call that builds is the whole assertion.
fn requires_send_and_sync<T: Send + Sync + ?Sized>(_: &T) {}

// ---------------------------------------------------------------------------
// The positive case
// ---------------------------------------------------------------------------

/// 34 AC-046 逐語: 「`Box<dyn SubstrateAdapter>`としてtrait objectを格納するコードをコンパイルする…
/// `Send + Sync`境界を満たしコンパイル成功する」.
#[test]
fn ac_046_a_boxed_adapter_satisfies_send_and_sync() {
    let boxed: Box<dyn SubstrateAdapter> = Box::new(Minimal);
    requires_send_and_sync(&*boxed);
    requires_send_and_sync(&boxed);
    assert_eq!(boxed.kind(), SubstrateKind::Fs);
}

/// The bound is not decoration: the box crosses a thread and is read from two at once.
///
/// `Send` is exercised by moving the box into a spawned thread and `Sync` by handing a shared
/// reference to a second one. A compile-time assertion alone would be satisfied by a bound nobody
/// relies on; this is the use 41 §4 put the bound there for -- an engine working on several
/// transformations at once (51 §7's harness will hold adapters the same way).
#[test]
fn ac_046_a_boxed_adapter_crosses_a_thread_boundary() {
    let owned: Box<dyn SubstrateAdapter> = Box::new(Minimal);
    let moved = std::thread::spawn(move || owned.kind());
    assert_eq!(
        moved.join().expect("the adapter moved to a thread"),
        SubstrateKind::Fs
    );

    let shared: Box<dyn SubstrateAdapter> = Box::new(Minimal);
    let borrowed: &dyn SubstrateAdapter = &*shared;
    std::thread::scope(|scope| {
        let left = scope.spawn(|| borrowed.kind());
        let right = scope.spawn(|| borrowed.kind());
        assert_eq!(left.join().expect("left"), right.join().expect("right"));
    });
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the repository root")
        .to_path_buf()
}

fn adapter_rs() -> String {
    std::fs::read_to_string(repo_root().join("crates/gx-substrate/src/adapter.rs"))
        .expect("crates/gx-substrate/src/adapter.rs is readable")
}

/// The fenced blocks of the documentation, as `(info string, body)`.
fn doc_examples(source: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut open: Option<(String, String)> = None;
    for line in source.lines() {
        let trimmed = line.trim_start();
        let Some(body) = trimmed
            .strip_prefix("//!")
            .or_else(|| trimmed.strip_prefix("///"))
        else {
            continue;
        };
        let body = body.strip_prefix(' ').unwrap_or(body);
        if let Some(info) = body.trim_end().strip_prefix("```") {
            match open.take() {
                Some(block) => out.push(block),
                None => open = Some((info.trim().to_string(), String::new())),
            }
            continue;
        }
        if let Some((_, ref mut text)) = open {
            text.push_str(body);
            text.push('\n');
        }
    }
    assert!(
        open.is_none(),
        "a fenced block in the documentation is not closed"
    );
    out
}

// ---------------------------------------------------------------------------
// The negative example
// ---------------------------------------------------------------------------

/// The documentation holds exactly one `compile_fail` example and it implements the trait.
#[test]
fn ac_046_the_negative_example_is_a_compile_fail_doctest() {
    let examples = doc_examples(&adapter_rs());
    let failing: Vec<&(String, String)> = examples
        .iter()
        .filter(|(info, _)| info.starts_with("compile_fail"))
        .collect();

    println!(
        "AC_046_DOC_EXAMPLES={} COMPILE_FAIL={}",
        examples.len(),
        failing.len()
    );
    assert_eq!(
        failing.len(),
        1,
        "AC-046's negative case is one `compile_fail` doctest (M4-20 (b)); found {}",
        failing.len()
    );
    assert!(
        failing[0].1.contains("impl SubstrateAdapter for"),
        "the negative example does not implement the trait, so it is not AC-046's negative case"
    );
}

/// The negative example is the positive one plus the thing that breaks the bound.
///
/// This is what stops 「it failed to compile」 from meaning 「something in it was wrong」. The two
/// blocks implement the same trait for the same seven methods; the failing one holds an `Rc`, whose
/// only relevant property is that it is neither `Send` nor `Sync`, and the passing one does not.
#[test]
fn ac_046_the_negative_example_differs_by_the_bound_and_nothing_else() {
    let examples = doc_examples(&adapter_rs());
    let passing: Vec<&(String, String)> = examples
        .iter()
        .filter(|(info, body)| {
            !info.starts_with("compile_fail")
                && !info.starts_with("text")
                && body.contains("impl SubstrateAdapter for")
        })
        .collect();
    let failing: Vec<&(String, String)> = examples
        .iter()
        .filter(|(info, _)| info.starts_with("compile_fail"))
        .collect();

    assert_eq!(
        passing.len(),
        1,
        "AC-046 needs one running example beside the failing one, as the control"
    );
    assert_eq!(failing.len(), 1);

    assert!(
        failing[0].1.contains("Rc<()>"),
        "the failing example does not hold the value that breaks `Send + Sync`"
    );
    assert!(
        !passing[0].1.contains("Rc<()>"),
        "the control holds the same value, so the two examples do not differ by the bound"
    );

    // The bodies agree on every line that is not the `Rc`: same trait, same seven methods, same
    // boxing. A drift on either side would make the pair stop being a controlled comparison. Three
    // substitutions and no more -- the type's name, the field it gains and the value that fills it
    // -- so that anything else added to either example fails here.
    let strip = |text: &str| -> Vec<String> {
        text.lines()
            .map(|l| {
                l.trim()
                    .replace("NotShared", "Shared")
                    .replace("(Rc<()>)", "")
                    .replace("Shared(Rc::new(()))", "Shared")
            })
            .filter(|l| !l.is_empty() && !l.contains("use std::rc::Rc"))
            .collect()
    };
    assert_eq!(
        strip(&failing[0].1),
        strip(&passing[0].1),
        "the two examples differ by more than the `Rc` field"
    );
}
