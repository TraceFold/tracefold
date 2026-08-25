// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **M4H5-4, adopted (b)** -- the forward payload has a ceiling of its own, declared once and named by the (sem: SEM-gx-adapter-fs-210)
//! contract it belongs to.
//!
//! req/38 §33, verbatim: "the forward payload is a **separate constant** (the escrow ceiling asks 'can it be carried', the forward
//! ceiling asks 'will it be accepted' -- gate/journal resource protection makes the two judgements differ). One declaration
//! site + a 1:1 contract-row probe (M4H2-8 shape); the value is decided by hand 6, with the reasoning printed". (sem: SEM-gx-adapter-fs-211)
//!
//! # Why the two ceilings are two constants and not one
//!
//! They answer different questions about different bytes. [`MAX_INVERSE_PAYLOAD_BYTES`] bounds what
//! this adapter is willing to **carry back**: the escrowed inverse holds the *old* content (42 §5:
//! "because a digest-only inverse makes an actual undo physically impossible"), and over the bound `invert` answers `Ok(None)`, which (sem: SEM-gx-adapter-fs-212)
//! **E-M3-4** turns into an escalation to a human. The forward ceiling bounds what it is willing to
//! **accept**: the payload of a plan travels through a gate and into a journal (**E-M4-8**: "
//! `PlannedDelta.payload` is stored (mandatory)"), so an unbounded one is a cost nobody declared, in a place (sem: SEM-gx-adapter-fs-213)
//! nobody chose. One number could not move without moving the other question with it.
//!
//! # The relation, in one line
//!
//! v0.1 sets them to the same number, and that is a decision rather than a coincidence:
//! **`MAX_FORWARD_PAYLOAD_BYTES <= MAX_INVERSE_PAYLOAD_BYTES`** means every change this adapter is
//! willing to make is one whose result it could also have escrowed back. Were the forward bound the
//! larger, an accepted change could be structurally unundoable for a size reason -- `Ok(None)` by
//! construction on a path the adapter itself opened.

mod support;

use gx_adapter_fs::{FsAdapter, FsDelta, MAX_INVERSE_PAYLOAD_BYTES};
use gx_substrate::SubstrateAdapter;
use support::{intent_for, snapshot_of, Sandbox, SUBJECT};

/// Hand 6's ceiling. Declared test-side in the RED commit and replaced by the crate's own `pub const`
/// here -- the practice §33 **M4H5-10, adopted (b)** fixed after hand 5 put one declaration line into a red
/// commit: "henceforth, put a private const test-side and swap it in the implementation commit as the practice (RED's purity)". (sem: SEM-gx-adapter-fs-214)
use gx_adapter_fs::MAX_FORWARD_PAYLOAD_BYTES as FORWARD_CEILING;

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).expect("a directory is readable") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-adapter-fs")
        .to_path_buf()
}

/// Plan a replacement of `size` bytes and report the payload it produced, or the refusal.
fn plan_of(size: usize) -> Result<usize, String> {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let pre = snapshot_of(&adapter, &locator);
    adapter
        .plan(&intent_for(&locator, &vec![b'g'; size]), &pre)
        .map(|delta| delta.payload().len())
        .map_err(|e| e.kind().to_string())
}

/// Either side of the bound, and the refusal is `plan`'s own word.
///
/// The refusal is [`gx_substrate::Error::NotPlannable`] because that is what 41 §4 documents for this
/// method -- "no delta plans this intent against this snapshot" -- and a goal too large to carry is (sem: SEM-gx-adapter-fs-215)
/// exactly that: a fact about the pair, not damage and not an unimplemented feature. `Ok(None)` has no
/// spelling here; `plan` is total in its answer or it refuses.
#[test]
fn the_forward_ceiling_is_the_number_the_source_declares() {
    let under = plan_of(FORWARD_CEILING - 1024);
    let over = plan_of(FORWARD_CEILING + 1);

    println!(
        "FORWARD_CEILING={FORWARD_CEILING} UNDER={}->{under:?} OVER={}->{over:?}",
        FORWARD_CEILING - 1024,
        FORWARD_CEILING + 1
    );
    let accepted = under.expect("a goal under the ceiling is plannable");
    assert!(
        accepted <= FORWARD_CEILING,
        "the payload of an accepted plan is {accepted} bytes, over the ceiling the adapter declares"
    );
    assert_eq!(
        over.expect_err("a goal over the ceiling is refused"),
        "NotPlannable",
        "an over-large goal is 'this intent cannot be planned against this snapshot' and not a \
         failure of the world (sem: SEM-gx-adapter-fs-216)"
    );
}

/// **Exactly** the ceiling is accepted, which is the byte `>` and `>=` disagree about.
///
/// 🔴 req/76 §2.2 listed `plan.rs:72` (`>` → `>=`) as a `cargo mutants` survivor with the reason in
/// one line: "nobody has built the case where payload is exactly 1,048,576". Hand 6 measured 1,047,552 and (sem: SEM-gx-adapter-fs-217)
/// 1,048,577 -- either side, and neither of them **on** the bound. A ceiling probed only at
/// neighbouring values cannot tell "at most N" from "fewer than N", and the two differ by exactly the (sem: SEM-gx-adapter-fs-218)
/// change that mutation makes.
///
/// The size is solved for rather than guessed. `plan` bounds the **payload**, which carries the
/// locator and the CBOR framing as well as the goal, and the sandbox's path length is the machine's
/// business -- so the probe measures the overhead at a size in the same length class (a byte string
/// of 2^16 or more carries a five-byte header, so the overhead is constant across this whole region)
/// and then asks for the goal that lands the payload on the number.
#[test]
fn a_payload_of_exactly_the_ceiling_is_planned() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let pre = snapshot_of(&adapter, &locator);
    let payload_of = |goal: usize| {
        adapter
            .plan(&intent_for(&locator, &vec![b'g'; goal]), &pre)
            .map(|d| d.payload().len())
    };

    // One probe of the overhead, then one solve. Asserted rather than looped: if the two differ the
    // encoding is not affine in this region and the probe would be measuring something else.
    let sample = FORWARD_CEILING - 4096;
    let overhead = payload_of(sample).expect("a goal well under the ceiling is plannable") - sample;
    let goal = FORWARD_CEILING - overhead;

    let exact = payload_of(goal).expect(
        "a payload of exactly the ceiling was refused: `plan` reads its bound as 'fewer than'
         where M4H5-4 (b) declares 'at most' (sem: SEM-gx-adapter-fs-219)",
    );
    println!(
        "FORWARD_CEILING={FORWARD_CEILING} OVERHEAD={overhead} GOAL={goal} PAYLOAD={exact} \
         ON_THE_BOUND={}",
        exact == FORWARD_CEILING
    );
    assert_eq!(
        exact, FORWARD_CEILING,
        "the fixture did not land on the bound, so this probe is not about the boundary byte"
    );
    assert!(
        payload_of(goal + 1).is_err(),
        "the control: one byte over the ceiling is refused, so the acceptance above is the bound \
         holding rather than the bound being absent"
    );
}

/// **M4H2-8**: the `plan` contract row in `gx-substrate` and the one declaration name each other.
///
/// The form hand 5 used for the escrow ceiling, applied to the second constant: "the contract table's row and the constant declaration's
/// site are a 1:1 probe". Neither end is a contract on its own -- a row naming a constant nobody declares (sem: SEM-gx-adapter-fs-220)
/// is a promise with no mechanism, and a constant no row mentions is a mechanism with no promise --
/// and two declarations that could drift are what "one constant" is written against. (sem: SEM-gx-adapter-fs-221)
///
/// 🔴 **M7 hand 1 narrowed the scan, and the narrowing is the ruling's own words.** Until there was a
/// second adapter this walked every crate under `crates/` and asserted **one** declaration
/// workspace-wide, which read correctly while `gx-adapter-fs` was the only adapter and became false
/// the moment `gx-adapter-git` declared its own bound. The trait's contract row says which reading is
/// right, verbatim: "payload over the ceiling is refused (**one constant per adapter, each adapter declares its own**
/// `MAX_FORWARD_PAYLOAD_BYTES`; fs's value is 1 MiB)" -- "one constant" is **per adapter**, because the bound is (sem: SEM-gx-adapter-fs-222)
/// a fact about what one adapter will accept and two adapters bound different things.
///
/// So the count is per crate and the gate is not weakened: **every** adapter crate is required to
/// declare exactly one, and a crate that is not an adapter is required to declare none. The old
/// spelling would have passed a workspace where the git adapter declared a bound and the fs adapter
/// had lost its own.
#[test]
fn the_contract_row_and_the_one_declaration_name_each_other() {
    let root = repo_root();
    let mut per_crate: Vec<(String, Vec<String>)> = Vec::new();
    for crate_dir in std::fs::read_dir(root.join("crates")).expect("crates/ is readable") {
        let dir = crate_dir.expect("an entry").path();
        let src = dir.join("src");
        if !src.is_dir() {
            continue;
        }
        let name = dir
            .file_name()
            .expect("a named directory")
            .to_string_lossy()
            .into_owned();
        let mut found: Vec<String> = Vec::new();
        for file in walk(&src) {
            let text = std::fs::read_to_string(&file).expect("a source file is readable");
            for line in text.lines() {
                if line
                    .trim_start()
                    .starts_with("pub const MAX_FORWARD_PAYLOAD_BYTES")
                {
                    found.push(format!("{}: {}", file.display(), line.trim()));
                }
            }
        }
        per_crate.push((name, found));
    }
    per_crate.sort();

    let adapters: Vec<&(String, Vec<String>)> = per_crate
        .iter()
        .filter(|(name, _)| name.starts_with("gx-adapter-"))
        .collect();
    let declarations: usize = per_crate.iter().map(|(_, found)| found.len()).sum();
    println!(
        "MAX_FORWARD_PAYLOAD_BYTES_DECLARATIONS={declarations} ADAPTER_CRATES={} PER_CRATE={:?}",
        adapters.len(),
        per_crate
            .iter()
            .map(|(name, found)| (name, found.len()))
            .collect::<Vec<_>>()
    );
    assert!(
        !adapters.is_empty(),
        "the scan found no adapter crate, so it is measuring nothing (§30's disease)"
    );
    for (name, found) in &per_crate {
        let expected = usize::from(name.starts_with("gx-adapter-"));
        assert_eq!(
            found.len(),
            expected,
            "M4H5-4 (b) asks each adapter for one declaration and no other crate for any; \
             `{name}` has {found:?}"
        );
    }

    let trait_doc = std::fs::read_to_string(root.join("crates/gx-substrate/src/adapter.rs"))
        .expect("the trait is readable");
    let row = trait_doc
        .lines()
        .find(|l| l.contains("| `plan` |"))
        .expect("the contract table has a `plan` row");
    assert!(
        row.contains("MAX_FORWARD_PAYLOAD_BYTES"),
        "the `plan` contract row does not name the constant that decides its refusal: {row}"
    );
}

/// The two ceilings are separate declarations, and the relation between them is stated as one.
///
/// Both halves are the ruling: **separate** because "will it be accepted" and "can it be carried" are different judgements (sem: SEM-gx-adapter-fs-223)
/// (M4H5-4 (b)), and **related** because an adapter whose forward bound exceeded its escrow bound
/// would be one that accepts changes it has already decided it cannot carry back.
///
/// The value is measured against this repository, the same population hand 5 used for the escrow
/// ceiling (2026-08-09, `git ls-files`): the largest file under `crates/`, `tools/` and `policies/` is
/// 38,292 bytes -- **27 times** under this number -- and the only two tracked files over it are raw
/// research captures under `req/15_math_rigor/raw/`. So every source file gx is made of is plannable
/// with room to spare, and what a v0.1 fs adapter declines to accept in one delta is a bulk capture.
#[test]
fn the_two_ceilings_are_separate_declarations_with_one_relation() {
    println!(
        "FORWARD_CEILING={FORWARD_CEILING} INVERSE_CEILING={MAX_INVERSE_PAYLOAD_BYTES} \
         FORWARD_LE_INVERSE={}",
        FORWARD_CEILING <= MAX_INVERSE_PAYLOAD_BYTES
    );
    // Through `min` rather than as `assert!(a <= b)`: both sides are constants, and an assertion over
    // two constants is one clippy refuses to let a suite carry (`assertions_on_constants`). The
    // comparison is the claim either way, and this spelling says which of the two is meant to be the
    // smaller when they stop being equal.
    assert_eq!(
        FORWARD_CEILING.min(MAX_INVERSE_PAYLOAD_BYTES),
        FORWARD_CEILING,
        "the forward ceiling is the larger, so this adapter accepts changes whose inverse it has \
         already declared it will not escrow"
    );

    let delta_rs = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/delta.rs"),
    )
    .expect("the grammar module is readable");
    for name in ["MAX_FORWARD_PAYLOAD_BYTES", "MAX_INVERSE_PAYLOAD_BYTES"] {
        assert_eq!(
            delta_rs
                .lines()
                .filter(|l| l.trim_start().starts_with(&format!("pub const {name}")))
                .count(),
            1,
            "{name} is not declared exactly once in the module that owns the grammar"
        );
    }
}

/// A payload already over the ceiling is not re-checked by `apply`, and that is written down.
///
/// The bound is on **what this adapter will plan**, not on what its grammar can express -- the same
/// split [`gx_adapter_fs::MAX_OPS`] has between a legal *value* and a legal *v0.1 payload*, except
/// that `MAX_OPS` is enforced in `decode` and this is enforced in `plan`. So a hand-written payload
/// over the ceiling still applies, which is a real hole in the resource bound and is raised as **filed**
/// in `req/75` §2 rather than quietly closed by a hand whose ruling said "on the plan side". (sem: SEM-gx-adapter-fs-224)
#[test]
fn a_hand_written_payload_over_the_ceiling_is_not_refused_by_the_grammar() {
    let sandbox = Sandbox::new();
    let locator = sandbox.locator(SUBJECT);
    let oversize = support::spelled(&locator, &vec![b'g'; FORWARD_CEILING + 1]);
    assert!(
        oversize.payload().len() > FORWARD_CEILING,
        "the fixture did not build the payload this probe is about"
    );
    let decoded = FsDelta::decode(oversize.payload()).expect("the grammar reads it back");
    println!(
        "FORWARD_CEILING_ENFORCED_IN=plan GRAMMAR_ACCEPTS_OVERSIZE={} OPS={}",
        decoded.ops().len() == 1,
        decoded.ops().len()
    );
    assert_eq!(
        decoded.ops().len(),
        1,
        "the grammar refused an over-large payload, so the bound moved to `decode` without the \
         ruling that would put it there"
    );
}
