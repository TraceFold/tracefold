// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-053 (FR-048) — the independence question can be asked with no engine and no gate in the room.
//!
//! AC-053, verbatim: "Given: a test context outside the engine pipeline. When: `commutation` API
//! is called directly against any 2 PlannedDeltas. Then: a Commutation result is returned without going through the engine/gate route." Judgment method:
//! "unit". FR-048 says why it is worth having: "SHOULD, for pre-screening use" -- a caller that wants (sem: SEM-gx-adapter-fs-147)
//! to know whether two changes clash before building a pipeline around them.
//!
//! # Measuring something that is absent
//!
//! "not going through the engine/gate route" is a claim about what is **not** there, and req/69 §8.2 fixes its
//! price: "adding one route in becomes empty unless it is measured by mutation to confirm it goes RED". The precedent is AC-029,
//! which measured "no bypass route exists" on three faces and had `tools/verify_m3h3.sh` add the (sem: SEM-gx-adapter-fs-148)
//! missing path to show all three go red. The same three, one milestone later:
//!
//! | face | what it measures | what a route would do to it |
//! |---|---|---|
//! | behaviour | two hand-written deltas, one adapter, an answer -- and two different answers | a route would need a value this test never builds |
//! | the graph | `[dependencies]` names no gate and the shipped tree has no edge to one | a route would have to be declared before it could be called |
//! | source | no code line under `src/` names a gate or an engine | a call would put the word there |
//!
//! `tools/verify_m4h6.sh` §5 (k) adds a gate hop to `src/commutation.rs` and (l) declares `gx-gate` as
//! a dependency; the two faces below go red in turn, which is what stops this file from asserting
//! nothing at all.
//!
//! # Why the gate is the whole of what there is to avoid
//!
//! There is no engine crate in this workspace yet -- gx-engine is M5 (**N-01**) -- so "the engine route" (sem: SEM-gx-adapter-fs-149)
//! cannot be reached by any code here, and saying that is more honest than implying this file ruled it
//! out. What **is** reachable is `gx-gate`: hand 5 put it in `[dev-dependencies]` for AC-048's
//! integration test (**M4-18, adopted (a)**), which means a route from `src/` to a gate would compile the day (sem: SEM-gx-adapter-fs-150)
//! somebody moved one line between two sections of the manifest. That is the mutation, and that is why
//! the manifest is one of the three faces.

mod support;

use gx_adapter_fs::FsAdapter;
use gx_core::{Commutation, SubstrateKind};
use gx_substrate::{PlannedDelta, SubstrateAdapter};
use support::spelled;

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-adapter-fs")
        .to_path_buf()
}

/// The lines of a source file that are code rather than documentation or comment.
///
/// §30's erratum about `FMT_ALL_USES`: an "absence" grep that counts comments answers a different (sem: SEM-gx-adapter-fs-151)
/// question than the one it was asked. This file is allowed to discuss gates in prose precisely
/// because the scan below cannot see prose.
fn code_lines(source: &str) -> Vec<&str> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//") && !line.is_empty())
        .collect()
}

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

// ---------------------------------------------------------------------------
// Face 1: behaviour
// ---------------------------------------------------------------------------

/// "any 2 PlannedDelta are called directly against the `commutation` API", and the answer comes back. (sem: SEM-gx-adapter-fs-152)
///
/// Nothing in this probe is an engine's: the two deltas are written in the grammar rather than
/// planned from an `Intent`, there is no `Transformation`, no `GateInput`, no policy set and no
/// substrate -- the sandbox other suites need is absent because independence is decided from the
/// payloads (**M4-13, adopted (a)**). What is left is 41 §4's method and two values. (sem: SEM-gx-adapter-fs-153)
///
/// The **control** is the second half. An adapter that returned one constant would satisfy "an answer
/// comes back" while answering nothing, so the two calls differ in one input and have to differ in the (sem: SEM-gx-adapter-fs-154)
/// answer -- the shape AC-048's `Admit`/`Escalate` pair has, one criterion over.
#[test]
fn the_api_answers_outside_any_pipeline() {
    let adapter = FsAdapter::new();
    let a = spelled("/tmp/glovrex-ac053/x", b"one");
    let independent = spelled("/tmp/glovrex-ac053/y", b"two");
    let dependent = spelled("/tmp/glovrex-ac053/x", b"two");

    let commutes = adapter
        .commutation(&a, &independent)
        .expect("the question is answerable without a pipeline");
    let conflicts = adapter
        .commutation(&a, &dependent)
        .expect("the question is answerable without a pipeline");

    println!("AC_053_DIRECT_CALL COMMUTES={commutes:?} CONFLICTS={conflicts:?}");
    assert_eq!(commutes, Commutation::Commutes);
    assert!(
        matches!(conflicts, Commutation::Conflicts { .. }),
        "the two calls differ in one delta and returned the same answer, so 'an answer comes back' is being \
         satisfied by a constant (sem: SEM-gx-adapter-fs-155)"
    );
}

/// The values it is asked about are `PlannedDelta`s and nothing else (41 §4's signature).
///
/// "any 2 PlannedDelta" is a claim about the **arguments**: a method that also needed a (sem: SEM-gx-adapter-fs-156)
/// transformation, a snapshot or a verdict would not be callable for pre-screening, which is what
/// FR-048 wants it for. Measured by calling it through a `&dyn SubstrateAdapter`, which is the shape
/// an engine holds (`Box<dyn SubstrateAdapter>`, AC-046) and which carries no extra channel.
#[test]
fn the_two_arguments_are_the_whole_of_the_input() {
    let adapter = FsAdapter::new();
    let boundary: &dyn SubstrateAdapter = &adapter;
    let a: PlannedDelta = spelled("/tmp/glovrex-ac053/x", b"one");
    let b: PlannedDelta = spelled("/tmp/glovrex-ac053/x", b"two");
    assert!(matches!(
        boundary.commutation(&a, &b),
        Ok(Commutation::Conflicts { .. })
    ));
}

// ---------------------------------------------------------------------------
// Face 2: the graph
// ---------------------------------------------------------------------------

/// The shipped manifest declares no gate and no engine.
///
/// A route has to be declared before it can be called, so the dependency section is where the
/// absence is cheapest to see and hardest to fake. `[dev-dependencies]` is a different matter and is
/// counted separately: `gx-gate` is there for AC-048 (**M4-18, adopted (a)**) and reaches no shipped graph, (sem: SEM-gx-adapter-fs-157)
/// which `tools/verify_m4h6.sh` prints as `SHIPPED_EDGES_TO_GATE=0` from `cargo tree -e normal`.
#[test]
fn the_shipped_dependencies_name_no_gate_and_no_engine() {
    let manifest = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
    )
    .expect("this crate's manifest is readable");

    let section = |name: &str| -> Vec<String> {
        manifest
            .split(&format!("[{name}]"))
            .nth(1)
            .map(|rest| {
                rest.split("\n[")
                    .next()
                    .expect("a section ends")
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .map(|l| {
                        l.split('=')
                            .next()
                            .expect("a dependency line names something")
                            .trim()
                            .to_string()
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let shipped = section("dependencies");
    let dev = section("dev-dependencies");
    println!("AC_053_SHIPPED_DEPS={shipped:?} DEV_DEPS={dev:?}");
    let routes: Vec<&String> = shipped
        .iter()
        .filter(|d| d.contains("gate") || d.contains("engine"))
        .collect();
    assert!(
        routes.is_empty(),
        "the shipped dependencies of an adapter name {routes:?}; FR-048 asks for a `commutation` \
         callable 'from outside the engine pipeline', and a declared route is the first step to one (sem: SEM-gx-adapter-fs-158)"
    );
    assert!(
        dev.iter().any(|d| d == "gx-gate"),
        "the dev-dependency AC-048 needs is gone, so this probe would pass for a crate that had no \
         gate anywhere and would stop measuring the distinction it is about"
    );
}

// ---------------------------------------------------------------------------
// Face 3: the source
// ---------------------------------------------------------------------------

/// No line of this adapter's shipped code names a gate, a verdict or an engine.
///
/// The face that catches a route which happens to answer the same thing today. It is the idiom
/// `ac_029`'s third face established -- "when a property is about the shape of the code rather than
/// about a value, the source is the instrument, and it is written down as such". (sem: SEM-gx-adapter-fs-159)
#[test]
fn no_shipped_line_of_this_adapter_names_a_gate_or_an_engine() {
    let src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let tokens = ["gx_gate", "gx-gate", "GateInput", "Verdict", "gx_engine"];
    let mut offenders: Vec<String> = Vec::new();
    let mut scanned = 0usize;

    for file in walk(&src) {
        let text = std::fs::read_to_string(&file).expect("a source file is readable");
        for line in code_lines(&text) {
            scanned += 1;
            for token in tokens {
                if line.contains(token) {
                    offenders.push(format!("{}: {line}", file.display()));
                }
            }
        }
    }
    println!(
        "AC_053_SOURCE_ROUTES={} SCANNED_CODE_LINES={scanned}",
        offenders.len()
    );
    assert!(
        offenders.is_empty(),
        "these shipped lines reach a gate: {offenders:?}. AC-053 is 'not going through the engine/gate route', and \
         a route in the source is a route whether or not today's inputs take it (sem: SEM-gx-adapter-fs-160)"
    );
}

/// The gate is reachable from the tests, and that is the point of measuring the absence in `src/`.
///
/// Without this line the probe above would be satisfied by a workspace with no gate at all, and
/// "there is no route" would be a fact about the repository rather than about this adapter. AC-048's suite (sem: SEM-gx-adapter-fs-161)
/// calls `gx_gate::Gate` from `tests/`, one directory away from the code that must not.
#[test]
fn the_gate_is_one_directory_away() {
    let ac_048 = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ac_048.rs"),
    )
    .expect("AC-048's suite is readable");
    assert!(
        code_lines(&ac_048).iter().any(|l| l.contains("gx_gate")),
        "no test in this crate reaches a gate, so the absence in `src/` measures nothing"
    );
    assert!(
        repo_root().join("crates/gx-gate/src/lib.rs").is_file(),
        "the gate this criterion is about does not exist in the workspace"
    );
}

/// A `SubstrateKind` is the only thing about the wider system the answer depends on.
///
/// The refusal an adapter gives a foreign delta is "this is not my grammar" and not "the gate said
/// no": the distinction is what keeps `commutation` a pre-screening call. Kept short because (sem: SEM-gx-adapter-fs-162)
/// `ac_052.rs` measures both slots; this one is here so that the AC-053 suite carries the negative
/// too.
#[test]
fn the_only_refusal_is_about_the_grammar() {
    let error = FsAdapter::new()
        .commutation(
            &spelled("/tmp/glovrex-ac053/x", b"one"),
            &PlannedDelta::new(SubstrateKind::Git, b"a git payload".to_vec())
                .expect("the projection is encodable"),
        )
        .expect_err("a git payload is not this adapter's grammar");
    assert_eq!(error.kind(), "ForeignDelta");
}
