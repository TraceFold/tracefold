// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The two "absence" scans this hand owes: `plan` touches no filesystem (**E-M4-29**) and nothing in (sem: SEM-gx-adapter-fs-168)
//! this crate reads a clock (**M4-17**).
//!
//! # Why a scan and not an assertion about behaviour
//!
//! §30 M4H2-3, adopted (b) read the trait's "pure function" as "determinism over the (intent, pre) pair + zero
//! **writes** to the substrate" and deliberately left reading open, "reading is not forbidden -- it does not close the road for M7's git adapter to read an object
//! store". Then it added a stronger clause for this adapter only: (sem: SEM-gx-adapter-fs-169)
//!
//! > "**however, for the fs adapter v0.1, `plan` achieving zero I/O holds** (for a single whole-file replacement, the target digest is
//! > derivable from the goal bytes), so **hand 4's DoD carries a machine check that 'fs's plan does not call std::fs'**
//! > (a machine-fixed implementation stronger than the contract)" (sem: SEM-gx-adapter-fs-170)
//!
//! A behavioural test cannot see the difference between "did not read" and "read and ignored": both (sem: SEM-gx-adapter-fs-171)
//! answer the same delta. So the claim is about the source, and the source is arranged to make the
//! claim cheap to check -- `src/plan.rs` is a module that never names the filesystem, and the trait
//! method delegates to it in one line.
//!
//! # The greps read invocation lines only
//!
//! §30's own erratum about `verify_m4h2.sh`: `FMT_ALL_USES=1` counted the **comment** in `ci.sh` that
//! explains why `--all` is not used. "henceforth, a verify script's 'absence' grep excludes comments or
//! targets only invocation lines". [`code_lines`] drops `//` and `//!` lines, which is why this file (sem: SEM-gx-adapter-fs-172)
//! can describe `std::fs` in prose without failing itself.

use std::path::PathBuf;

fn source(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is unreadable: {e}", path.display()))
}

/// The lines of a source file that are code rather than documentation or comment.
fn code_lines(source: &str) -> Vec<&str> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//") && !line.is_empty())
        .collect()
}

/// The tokens that would mean a filesystem call.
const IO_TOKENS: [&str; 8] = [
    "std::fs",
    "fs::",
    "File::",
    "OpenOptions",
    "read_to_string",
    "canonicalize",
    "read_link",
    "symlink_metadata",
];

/// **E-M4-29**, first half: the module that plans names no filesystem operation.
#[test]
fn the_planning_module_calls_no_filesystem_operation() {
    let src = source("src/plan.rs");
    let offenders: Vec<(&str, &str)> = code_lines(&src)
        .into_iter()
        .flat_map(|line| {
            IO_TOKENS
                .iter()
                .filter(move |token| line.contains(**token))
                .map(move |token| (*token, line))
        })
        .collect();
    println!(
        "PLAN_IO_INVOCATIONS={} SCANNED_CODE_LINES={}",
        offenders.len(),
        code_lines(&src).len()
    );
    assert!(
        offenders.is_empty(),
        "E-M4-29 fixes 'plan does not call std::fs' as a machine check, and these lines call \
         one: {offenders:?} (sem: SEM-gx-adapter-fs-173)"
    );
}

/// **E-M4-29**, second half: the trait method is the delegation and nothing else.
///
/// The scan above is only worth what this one adds. `src/adapter.rs` does open files -- `snapshot`
/// and `precondition` are supposed to -- so the guarantee has to be that `plan`'s body reaches none
/// of it, and the cheapest form of that is a body with one call in it.
#[test]
fn the_trait_method_only_delegates_to_that_module() {
    let src = source("src/adapter.rs");
    let start = src
        .find("fn plan(")
        .expect("`impl SubstrateAdapter for FsAdapter` has a `plan`");
    let open = src[start..].find('{').expect("a body") + start;
    let close = src[open..].find('}').expect("a body ends") + open;
    let body: Vec<&str> = code_lines(&src[open + 1..close]);

    println!("PLAN_BODY_LINES={} BODY={:?}", body.len(), body);
    assert_eq!(
        body.len(),
        1,
        "`plan` in the trait impl is more than a delegation, so the scan of `src/plan.rs` no longer \
         covers what runs"
    );
    assert!(
        body[0].contains("plan::plan("),
        "`plan` delegates to something other than the pure module: {:?}",
        body[0]
    );
}

/// **M4-17**: nothing in this crate reads a clock.
///
/// The module list is written out rather than walked, so that a module added without being scanned
/// is a decision somebody made in this file. Hand 5 added `src/apply.rs` and `src/invert.rs` to it in
/// the same commit that created them, and hand 6 `src/commutation.rs` -- an instrument that quietly
/// stops covering the code is the failure §29 M4H1-8 and §30's `FMT_ALL_USES` are two earlier
/// instances of.
///
/// 41 §6, verbatim: "randomness and time are injected at the engine boundary (for deterministic replay)", and §31 **E-M4-31** settled what
/// an adapter does instead: "`applied_at` is a convention **overwritten by the engine at commit time**... the adapter [writes a]
/// `Timestamp(0)` placeholder". The workspace-wide count is in `tools/verify_m4h4.sh`; this is the (sem: SEM-gx-adapter-fs-174)
/// one crate that would have had a reason to reach for a clock, because a file has an mtime.
///
/// It has no reason after all: ASM-69-1 keeps mtime out of the fingerprint digest, so the adapter
/// never asks the filesystem what time it is either.
#[test]
fn this_crate_reads_no_clock() {
    let mut found: Vec<String> = Vec::new();
    for module in [
        "src/lib.rs",
        "src/adapter.rs",
        "src/apply.rs",
        "src/commutation.rs",
        "src/delta.rs",
        "src/invert.rs",
        "src/locator.rs",
        "src/plan.rs",
    ] {
        let src = source(module);
        for line in code_lines(&src) {
            if line.contains("SystemTime") || line.contains("Instant::now") {
                found.push(format!("{module}: {line}"));
            }
        }
    }
    println!("FS_ADAPTER_CLOCK_READS={}", found.len());
    assert!(found.is_empty(), "M4-17 forbids these: {found:?}");
}

/// **M4-17** / **E-M4-31**, from the other side: the placeholder an engine overwrites is named.
#[test]
fn the_applied_at_placeholder_convention_is_written_down() {
    let src = source("src/lib.rs");
    for token in ["E-M4-31", "Timestamp(0)"] {
        assert!(
            src.contains(token),
            "the crate root does not name {token:?}, so hand 5's `apply` has no written convention \
             to follow"
        );
    }
}
