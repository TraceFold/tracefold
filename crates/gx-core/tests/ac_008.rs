// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-008 (FR-008) — no I/O, no `unsafe`, no clippy warnings.
//!
//! AC-008, verbatim (quoted in SEM-gx-core-118): "Given: the whole source of `crates/gx-core`.
//! When: `cargo clippy -p gx-core -- -D warnings` and `cargo tree -p gx-core` are run. Then: zero
//! clippy warnings; the `#![forbid(unsafe_code)]` declaration is present; no I/O crate (`tokio`,
//! `std::net`, etc.) appears in the dependency graph."
//!
//! Two of the three clauses are commands, and they are run by `tools/verify_hand2.sh` with their
//! output recorded in `req/40_M1_HAND2_REPORT_2026-08-07.md`; a test process cannot honestly
//! measure its own clippy run. What is left for this file is the part that can be checked from
//! inside: the attribute is present, and no source file in the crate reaches for I/O.
//!
//! **This test was green on the commit that introduced it, and that is stated rather than
//! hidden.** The property it guards was established in hand 1 (commit f9fb086, which wrote
//! `#![forbid(unsafe_code)]` and an empty `[dependencies]`), so hand 2 had nothing to make it
//! true -- only the standing obligation not to break it. Producing a red would have meant adding
//! an I/O dependency in order to delete it again, which would put a failure in the history that
//! never described the code. T-27's ordering is kept for the eight ACs that had something to
//! implement; AC-008 is recorded as the documented exception (see req/40 §3).
//!
//! The scan is not vacuous: `finds_a_planted_violation` shows the detector firing.

/// Crate names and std paths that would mean this crate had started doing I/O. The `std::`
/// entries are what FR-008's "performs no I/O (zero external calls)" (sem: SEM-gx-core-119) rules
/// out from inside; the crate
/// names are what AC-008 rules out from the dependency side.
const DENY: &[&str] = &[
    "tokio",
    "async-std",
    "async_std",
    "reqwest",
    "hyper",
    "mio",
    "socket2",
    "std::net",
    "std::fs",
    "std::io",
    "std::process",
    "std::env",
];

const LIB: &str = include_str!("../src/lib.rs");

/// Every module file, listed. `module_list_is_complete` below is what keeps this list honest
/// when a file is added.
const SOURCES: &[(&str, &str)] = &[
    ("lib.rs", LIB),
    ("b64.rs", include_str!("../src/b64.rs")),
    // 🔴 **DR-46-28** — a third comes down, by the same rule and for the same reason
    // (`req/38` §255 ruling 4, `req/459` ruling 1): the declaration face is
    // `gx-adapter-mcp`'s catalogue and the attest face is `gx-witness`'s receipt, and
    // neither of those crates can name the other. `of_stages` is arithmetic on two values
    // its caller already holds; nothing here reads, hashes, signs or compares.
    ("boundary.rs", include_str!("../src/boundary.rs")),
    ("commutation.rs", include_str!("../src/commutation.rs")),
    ("context.rs", include_str!("../src/context.rs")),
    ("delta.rs", include_str!("../src/delta.rs")),
    ("dsse.rs", include_str!("../src/dsse.rs")),
    // M5 hand 1: DR-2's two axes come down here (M5-08, adopted (a); sem: SEM-gx-core-120), the
    // way `VerdictKind` did.
    ("enforcement.rs", include_str!("../src/enforcement.rs")),
    ("error.rs", include_str!("../src/error.rs")),
    ("fingerprint.rs", include_str!("../src/fingerprint.rs")),
    ("intent.rs", include_str!("../src/intent.rs")),
    ("ledger.rs", include_str!("../src/ledger.rs")),
    ("measure.rs", include_str!("../src/measure.rs")),
    ("object.rs", include_str!("../src/object.rs")),
    ("planned.rs", include_str!("../src/planned.rs")),
    ("proof.rs", include_str!("../src/proof.rs")),
    // 🔴 **DR-46-26** — two more come down here, and the rule is the one that brought every entry
    // above: the data comes down, the computation stays up. `ReadEntry` is `{Cid, String}` and
    // `Reversibility` is three words with an `as_str`; neither reads, hashes, signs or compares.
    // (`req/38` §258. `gx-substrate` cannot name `gx-adapter-mcp` and `gx-witness` cannot name
    // `gx-substrate`, so this is the one crate both parties already depend on.)
    ("reads.rs", include_str!("../src/reads.rs")),
    ("reversibility.rs", include_str!("../src/reversibility.rs")),
    (
        "transformation.rs",
        include_str!("../src/transformation.rs"),
    ),
    ("verdict.rs", include_str!("../src/verdict.rs")),
];

const MANIFEST: &str = include_str!("../Cargo.toml");

/// Strip `//` line comments so a denied name mentioned in prose does not read as a use of it.
/// This file's own doc comments are the reason that matters: they name `tokio` twice.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Same idea for TOML, whose comment marker is `#`. Rust cannot share the function: `#` opens an
/// attribute there, and stripping from it would delete every `#[derive(..)]` line.
fn toml_code_only(src: &str) -> String {
    src.lines()
        .map(|l| match l.find('#') {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn ac_008_forbid_unsafe_code_is_declared() {
    assert!(
        LIB.contains("#![forbid(unsafe_code)]"),
        "41 §6 requires the crate root to forbid unsafe"
    );
}

#[test]
fn ac_008_no_source_file_reaches_for_io() {
    for (name, src) in SOURCES {
        let code = code_only(src);
        for bad in DENY {
            assert!(
                !code.contains(bad),
                "{name} names `{bad}`; gx-core does no I/O (FR-008, 41 §6)"
            );
        }
        assert!(
            !code.contains("unsafe "),
            "{name} contains `unsafe` despite the crate-level forbid"
        );
    }
}

#[test]
fn ac_008_manifest_dependencies_stay_inside_the_allowlist() {
    // 41 §2 allows "serde, thiserror and not much more" (sem: SEM-gx-core-121) and nothing wider.
    // The dev-dependencies section is a
    // separate matter: `cargo tree -p gx-core -e normal` is the graph a consumer gets, and
    // proptest/serde_json are absent from it. tools/verify_hand2.sh records both trees.
    let deps = MANIFEST
        .split("[dependencies]")
        .nth(1)
        .expect("manifest has a [dependencies] section")
        .split("[dev-dependencies]")
        .next()
        .expect("split always yields one");
    for bad in DENY {
        assert!(
            !toml_code_only(deps).contains(bad),
            "[dependencies] names `{bad}`"
        );
    }
    for line in toml_code_only(deps).lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let name = line.split(['=', ' ']).next().unwrap_or_default();
        assert!(
            matches!(name, "serde" | "thiserror"),
            "unexpected runtime dependency `{name}` (41 §2: serde, thiserror and not much more; \
             sem: SEM-gx-core-122)"
        );
    }
}

#[test]
fn ac_008_module_list_is_complete() {
    // If a ninth module appears, the scan above must see it. Counting `pub mod` in lib.rs against
    // the table is what makes that automatic rather than remembered.
    let declared = code_only(LIB)
        .lines()
        .filter(|l| l.trim_start().starts_with("pub mod "))
        .count();
    assert_eq!(
        declared,
        SOURCES.len() - 1,
        "SOURCES is missing a module declared in lib.rs (or vice versa)"
    );
}

#[test]
fn ac_008_finds_a_planted_violation() {
    // The detector fires, so a green run above means "clean", not "looked at nothing".
    let planted = "use std::net::TcpStream;\nfn f() {}\n";
    assert!(DENY.iter().any(|bad| code_only(planted).contains(bad)));
    let commented = "// std::net is forbidden here\nfn f() {}\n";
    assert!(!DENY.iter().any(|bad| code_only(commented).contains(bad)));
}
