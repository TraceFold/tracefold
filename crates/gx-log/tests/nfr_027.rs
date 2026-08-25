// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! NFR-027 (33) / ASM-33-2 (35 §F) — the 180-day retention floor is a number this crate holds,
//! not merely a sentence in canon.
//!
//! what: 🔴 v0.2.7 batch Lane B item 4 (`req/38` §81 coverage v2 finding 3: "NFR-027 = a ruling
//!       contradiction (35 ASM-33-2's 'the floor does not move' declaration coexisting with the
//!       unimplemented)"). (sem: SEM-gx-log-168) `req/137` §B1 item 4 asks the
//!       contradiction to be resolved either by implementation or by a revised ruling; this file
//!       is the implementation half (a constant + this one test).
//! why : ASM-33-2 calls the 180-day floor immovable while `grep -rln "retention" crates/**/*.rs`
//!       found zero code before this batch (no config, no constant, no test) -- a floor declared
//!       immovable with nothing enforcing it is exactly the "declared but unenacted" shape 33's
//!       NFR-011/022 rows already show elsewhere. This file closes that gap for NFR-027 without
//!       re-scanning what `ac_021.rs` already proves (gx-log offers no delete/prune/purge/truncate
//!       function at all): see `gx_log::NFR_027_MINIMUM_RETENTION_DAYS`'s own doc for why an
//!       append-only surface with no mutation function satisfies a *minimum* retention floor by
//!       construction.
//! deps: std only, plus reading two spec files as text (doc-conformance, `ac_045.rs`'s pattern).

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate must sit at <root>/crates/gx-log")
        .to_path_buf()
}

fn read_repo(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// The number itself, and its relation to `ac_021`'s structural guarantee.
#[test]
fn nfr_027_the_floor_is_a_hundred_and_eighty_days() {
    assert_eq!(gx_log::NFR_027_MINIMUM_RETENTION_DAYS, 180);
    // A `const` block for the same reason `ac_045.rs` uses one: both operands are compile-time
    // constants, so the claim is checked at build time rather than merely at test run time.
    const {
        assert!(
            gx_log::NFR_027_MINIMUM_RETENTION_DAYS >= 180,
            "EU AI Act Art.19/26's minimum"
        );
    }
}

/// 33's own text still names the number this crate now holds -- read off 33 rather than off
/// memory, the same discipline `ac_045.rs` applies to NFR-028's `verify_ttl`/`escalation_ttl`.
#[test]
fn nfr_027_and_asm_33_2_still_name_the_same_floor() {
    let nfr = read_repo("req/spec/30-requirements/33-non-functional.md");
    assert!(
        nfr.contains("NFR-027") && nfr.contains("180"),
        "33 NFR-027 no longer names the 180-day floor"
    );
    let open_questions = read_repo("req/spec/30-requirements/35-open-questions.md");
    assert!(
        open_questions.contains("ASM-33-2")
            // (sem: SEM-gx-log-169) byte-identical Unicode-escape respelling of the two literals
            // req/semantics/gx-log.ja.md SEM-gx-log-169 names in full; the check is unchanged.
            && open_questions.contains("\u{4e0b}\u{9650}")
            && open_questions.contains("\u{52d5}\u{304b}\u{306a}\u{3044}"),
        "35 ASM-33-2 no longer calls the floor immovable"
    );
}
