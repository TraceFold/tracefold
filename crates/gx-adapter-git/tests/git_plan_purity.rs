//! 「`plan` は object store を読んでよいが **substrate への書き込み 0**」 — measured twice.
//!
//! `req/98` §3-4 予約 5 and §6-7 carry the M7 form of **E-M4-29** (§30 M4H2-3 採(b)):
//!
//! > 「(intent, pre) の対に対する決定性+substrate への**書き込み** 0…読み込みは禁じない——**M7 の git
//! > adapter が object store を読む道を閉じない**」
//!
//! and §6-7 adds the sentence this file exists to make true rather than assert: 「則 1 の M7 版…「I/O 0」
//! ではない事を doc に 1 行」.
//!
//! # Two measurements, because one of them is text
//!
//! 1. **The source names no gitoxide call.** `src/plan.rs` is scanned for `gix` and for the names of
//!    this crate's own repository boundary. A scan is a text gate and text gates have limits (M6H8-1
//!    wrote two of them down for gx-canon's scanner); its limit here is stated below.
//! 2. **The repository does not move.** Every byte under `.git` is digested before and after a
//!    `plan`, and the two digests are compared. This is the measurement text cannot make, and it is
//!    stronger than 「no write call」 — it would catch a write through a path the scanner does not know
//!    about, including one inside gitoxide.
//!
//! 🔴 The pair is the point. `gx-adapter-fs` has only the first (its `plan` builds a payload from the
//! goal and there is nothing to read), and this crate's `plan` is in the same position **by
//! construction** — L1 forces the branch tip out of the payload, so there is nothing left to read. The
//! second measurement is therefore checking a stronger property than the contract requires, and 51 §7
//! contract 2 gets its own weaker check from the harness (`precondition` before and after). Both are
//! kept: 「a strong implementation fixed by a machine」 is what §30 asked of the fs adapter's `plan`,
//! and it is cheaper here than the argument for skipping it.

mod support;

use std::path::{Path, PathBuf};

use gx_canon::cid::{self, Domain};
use gx_core::Cid;
use gx_substrate::SubstrateAdapter;
use support::{intent_for, GitFixture, BRANCH, GOAL};

/// The names a write to a git repository would have to go through, from this module's own vocabulary.
///
/// 🔴 **The limit of the scan, stated rather than discovered** (M6H8-1's form): it reads text, so an
/// alias (`use gix as g;`) or a helper in another module called from here would be invisible to it.
/// What holds the line instead is the second measurement below, which does not read source at all,
/// and the compiler-side fact that `plan.rs`'s import list is four lines long and in the diff.
const WRITE_TOKENS: [&str; 8] = [
    "gix",
    "repo::",
    "write_blob",
    "write_object",
    "move_branch",
    "commit_entry",
    "edit_reference",
    "transaction",
];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every byte under a directory, folded into one digest, ordered by path.
///
/// gx-canon's, because 41 §6 admits one hash and a test that reached for a second one would be
/// introducing the thing the workspace forbids in order to check the thing it requires.
fn digest_tree(root: &Path) -> Cid {
    let mut files: Vec<PathBuf> = Vec::new();
    walk(root, &mut files);
    files.sort();
    let mut parts: Vec<Vec<u8>> = Vec::new();
    for path in &files {
        parts.push(path.to_string_lossy().as_bytes().to_vec());
        parts.push(std::fs::read(path).unwrap_or_default());
    }
    let refs: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
    cid::mint(Domain::Leaf, &refs)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Measurement 1: the module that plans names no repository call.
#[test]
fn the_plan_module_names_no_gitoxide_call() {
    let source = std::fs::read_to_string(crate_root().join("src/plan.rs")).expect("readable");
    // Comments are where the module *argues* about gitoxide, at length and on purpose. What must not
    // appear is a call, so comment lines are dropped the way M6's 則 1 counters drop them.
    let code: String = source
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("///") || t.starts_with("//!"))
        })
        .collect::<Vec<_>>()
        .join("\n");

    let found: Vec<&str> = WRITE_TOKENS
        .iter()
        .copied()
        .filter(|token| code.contains(token))
        .collect();
    println!("PLAN_WRITE_TOKENS={} ({found:?})", found.len());
    assert!(
        found.is_empty(),
        "src/plan.rs names {found:?} in code. E-M4-29 permits a read and forbids a write, and this \
         adapter's plan does neither (L1 is why): a call appearing here is a design change and not \
         a refactor"
    );

    // The delegation is one line and it is in `adapter.rs`, so the scan above is a scan of the whole
    // implementation rather than of a function body.
    let adapter = std::fs::read_to_string(crate_root().join("src/adapter.rs")).expect("readable");
    assert!(
        adapter.contains("crate::plan::plan(intent, pre)"),
        "`SubstrateAdapter::plan` delegates to the module this file scanned"
    );
}

/// Measurement 2: the repository is byte-identical across a `plan`.
///
/// Stronger than the contract (51 §7 契約 2 compares a `precondition` before and after, which sees the
/// branch tip and nothing else) and stronger than the scan (which sees text and not behaviour). A
/// `plan` that wrote a loose object nobody referenced would pass both of those and fail this.
#[test]
fn planning_leaves_the_repository_byte_identical() {
    let fixture = GitFixture::new();
    let adapter = fixture.git();
    let sandbox = fixture.sandbox();
    let locator = sandbox.locator_on(BRANCH);
    let git_dir = sandbox.dir().join(".git");
    assert!(git_dir.is_dir(), "the sandbox is a non-bare repository");

    let pre = adapter.snapshot(&locator).expect("the entry is there");
    let before = digest_tree(&git_dir);
    let first = adapter
        .plan(&intent_for(&locator, GOAL), &pre)
        .expect("the adapter plans");
    let after = digest_tree(&git_dir);

    println!("PLAN_REPO_DIGEST before={before:?} after={after:?}");
    assert_eq!(
        before, after,
        "`plan` moved the repository. E-M4-29 reads 41 §4's 「純関数」 as 「substrate への書き込み 0」, \
         and a loose object written by a plan is a write whether or not anything points at it"
    );

    // And the same plan again, with a commit landing in between: L1's quantifier, asserted here on
    // the delta's own bytes rather than through the harness's `PartialEq`.
    sandbox.commit_over(b"somebody else pushed\n");
    let second = adapter
        .plan(&intent_for(&locator, GOAL), &pre)
        .expect("the adapter plans against the same snapshot after the branch moved");
    println!(
        "PLAN_DETERMINISM payload_len={} equal={}",
        first.payload().len(),
        first == second
    );
    assert_eq!(
        first, second,
        "the same (intent, pre) planned two different deltas once the branch moved, so the answer \
         depends on something that is not an argument (E-M4-4, E-M4-29, L1)"
    );
}
