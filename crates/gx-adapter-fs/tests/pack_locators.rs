// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **PACK_FORMAT F7 for the fs pack** (**R35**, `req/470` L-02 / `req/38` §264 ruling 3 item 4).
//!
//! # What was missing
//!
//! F7 says no shipped pack may carry an unreachable rule (**D-4**, `req/99` §3: "do not declare an
//! unreachable constant"): every conformance case's locator has to be a spelling the substrate's
//! own adapter can actually produce, **and a test must measure that rather than a reader believing
//! it**. `policies/PACK_FORMAT.md` credits the clause to "`ac_074.rs` for F7", and audit 34 counted
//! what that summary covers:
//!
//! | pack | F7 instrument | where |
//! |---|---|---|
//! | git | `every_case_names_a_locator_the_git_adapter_could_produce` | `crates/gx-gate/tests/ac_074.rs` |
//! | mcp | `every_case_names_a_locator_the_mcp_adapter_could_produce` | `crates/gx-gate/tests/ac_074.rs` |
//! | postgres | `every_case_names_a_locator_the_postgres_adapter_could_produce` | `crates/gx-adapter-postgres/tests/pack_locators.rs` |
//! | **fs** | **none** | — |
//!
//! Three of four, and the missing one is the pack that shipped **first** and the substrate whose
//! `/etc` example the document uses to explain the clause.
//!
//! # Why this file is here and not in `crates/gx-gate/tests/ac_074.rs`
//!
//! `req/471` §0-6 names `ac_074.rs`, and that is the wrong address for two independent reasons
//! found by reading the tree rather than the summary:
//!
//! 1. **The fs case table is not there.** `ac_074.rs` holds the git and mcp tables; fs's lives in
//!    `crates/gx-gate/tests/ac_028.rs` and uses a bespoke local `Case`/`Expect` pair rather than
//!    `PackCase`/`PackExpectation`, so the git/mcp instrument has nothing to iterate over.
//! 2. **gx-gate must not name an adapter.** That is the rule `ac_074.rs` states about itself, and
//!    it is why git's and mcp's instruments *restate* their grammars instead of calling a parser —
//!    which their own doc admits can be "wrong in the direction of being **too permissive**".
//!
//! So this takes the **stronger** of the two available shapes, the one
//! `crates/gx-adapter-postgres/tests/pack_locators.rs` uses: read the locators out of the pack's
//! own shipped scenario file and put them through the adapter's real code. A restated fs grammar
//! would be four lines and would be the third copy of a rule the adapter already owns.
//!
//! # What "a locator the fs adapter could produce" means here
//!
//! fs has no `parse()` and no `Position` type — its locator module is `is_absolute` and
//! `normalize`. `SubstrateAdapter::snapshot` normalises on the way in and reports the normalised
//! spelling, so the set of locators this adapter can hand a gate is exactly the **fixed points of
//! `normalize` that are absolute**. That is the round trip postgres's instrument asserts, spelled
//! for this substrate: a pack rule written against a spelling the adapter would rewrite is a rule
//! matching something no gate will ever see.

use std::path::Path;

use gx_adapter_fs::locator;

/// The `(name, locator)` pairs the fs pack's shipped scenario file declares for its own substrate.
///
/// Read off the disk rather than restated in Rust, for the reason the postgres instrument gives:
/// `gx policy test <PACK> --scenario <FILE>` is the operator's face of this, and the file is what
/// they point it at. A table retyped here would be a second copy, and the copy is what goes stale.
fn pack_locators() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/gx-adapter-fs sits at <root>/crates/gx-adapter-fs");
    let path = root.join("policies").join("fs").join("scenarios.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the fs pack ships a scenario file at {}: {e}",
            path.display()
        )
    });
    let cases: serde_json::Value = serde_json::from_str(&text).expect("scenarios.json is JSON");
    cases
        .as_array()
        .expect("scenarios.json is an array of cases")
        .iter()
        .filter(|case| case["substrate"].as_str() == Some("fs"))
        .map(|case| {
            (
                case["name"].as_str().unwrap_or_default().to_string(),
                case["locator"]
                    .as_str()
                    .expect("every case names a locator")
                    .to_string(),
            )
        })
        .collect()
}

/// 🔴 **F7 / D-4, fourth pack**: every fs conformance locator is one this adapter would write.
#[test]
fn every_case_names_a_locator_the_fs_adapter_could_produce() {
    let cases = pack_locators();
    assert!(
        cases.len() >= 2,
        "the pack's own substrate must appear in its scenario file more than once, or this test \
         measures a single hand-picked string: {cases:?}"
    );
    for (name, spelling) in &cases {
        assert!(
            locator::is_absolute(spelling),
            "{name}: {spelling:?} is not absolute, and ASM-69-3 names positions from the root. A \
             gate never sees a relative fs locator, so a rule about one could never fire (D-4)"
        );
        // The round trip is the real claim, exactly as the postgres instrument puts it: that
        // `normalize` accepts the string says it is readable; that it returns the string unchanged
        // says it is one the adapter would **write** - and a policy is compared against what the
        // adapter wrote. `/etc/../etc/passwd` reads fine and normalises to `/etc/passwd`, so a
        // pack rule spelled the first way would be a contest of spellings the pack loses.
        assert_eq!(
            &locator::normalize(spelling),
            spelling,
            "{name}: the adapter reads this spelling but does not write it, so a pack matching on \
             it matches something no gate will ever see (PACK_FORMAT F7 / D-4)"
        );
    }
    println!("PACK_V0_LOCATORS_FS cases={}", cases.len());
}

/// The negative control: the instrument can fail.
///
/// `INHERITED_PRINCIPLES.md` §3d refuses a gate over a property nothing can break, and this whole
/// clause exists because three packs had an instrument and the fourth's absence looked identical
/// to a pass. These are the two shapes an unreachable fs rule takes, and the assertions above must
/// reject both.
#[test]
fn the_instrument_rejects_the_two_shapes_of_an_unreachable_fs_rule() {
    let rewritten = "/etc/../etc/passwd";
    assert_ne!(
        locator::normalize(rewritten),
        rewritten,
        "a locator the adapter rewrites must not be a fixed point, or the round-trip assertion \
         above is vacuous"
    );
    assert_eq!(
        locator::normalize(rewritten),
        "/etc/passwd",
        "and it must rewrite to the spelling the pack does declare"
    );
    let relative = "etc/passwd";
    assert!(
        !locator::is_absolute(relative),
        "a relative locator must fail the absoluteness assertion above"
    );
}
