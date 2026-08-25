// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `Lifecycle` against 43 §1 and 43 §3, read out of the spec file rather than restated.
//!
//! The **I-11** shape ("a shape-family test's claim can be anchored to the spec side" (quoted in
//! SEM-gx-engine-806), req/67 §4) applied where the
//! ruling reserved it for -- gx-gate -- and then applied here as well, because the state machine is
//! the place where a list that drifted from the canon would do the most damage. `gate_input_spec.rs`
//! is the precedent: one reader, pointed at the spec, and the number comes from the document.
//!
//! Three lists, three sources:
//!
//! | list | source | what a mismatch means |
//! |---|---|---|
//! | 43 §1's state table | the spec file, parsed | the enum drifted from the canon |
//! | [`gx_engine::LIFECYCLE_STATES`] | the crate's declaration | the declaration drifted from the enum |
//! | `Lifecycle::name` over every variant | the compiler | — (no `_` arm, so it cannot drift) |

mod support;

use gx_engine::LIFECYCLE_STATES;
use support::read_repo;

/// The state names in 43 §1's table, in the order the table gives them.
///
/// The table's first column is a fenced state name. It is bounded above by the `## 1.` heading and
/// below by the `AbortReason` paragraph that follows it -- section-scoped before anything is parsed,
/// because 43 names these words in seven other places and a whole-file scan would find the mermaid
/// diagram first.
fn states_declared_by_43_1() -> Vec<String> {
    let text = read_repo("req/spec/40-architecture/43-state-machine.md");
    let section = text
        .split("## 1. ") // (sem: SEM-gx-engine-807)
        .nth(1)
        .expect("43 still has a §1 state list")
        .split("\n## ")
        .next()
        .expect("§1 is followed by another section");

    let mut names = Vec::new();
    for line in section.lines() {
        let line = line.trim();
        if !line.starts_with("| `") {
            continue;
        }
        let name = line
            .trim_start_matches("| `")
            .split('`')
            .next()
            .expect("a fenced name in the first column");
        names.push(name.to_string());
    }
    names
}

/// The declared list is 43 §1's list, name for name and in order.
#[test]
fn the_states_are_the_ones_43_1_names() {
    let canon = states_declared_by_43_1();
    println!(
        "CANON_STATES={} IMPLEMENTED_STATES={} ({canon:?})",
        canon.len(),
        LIFECYCLE_STATES.len()
    );
    assert_eq!(canon.len(), 11, "43 §1 names eleven states");
    assert_eq!(
        canon,
        LIFECYCLE_STATES.to_vec(),
        "`LIFECYCLE_STATES` is not 43 §1's table"
    );
}

/// 43 §1's `AbortReason` block has six variants, and gx-core declares those six.
///
/// 43 §1, verbatim: "the canonical definition of `AbortReason` is gx-core (see ASM-15, 35): six
/// variants" (sem: SEM-gx-engine-808). The engine's `Lifecycle::Aborted` carries one of them,
/// so a seventh arriving -- which is what **blocker item 5** would do if the Owner rules M5-11
/// option (a) -- is a change this file reports rather than absorbs.
#[test]
fn the_abort_reasons_are_the_six_43_1_transcribes() {
    let text = read_repo("req/spec/40-architecture/43-state-machine.md");
    let block = text
        .split("pub enum AbortReason {")
        .nth(1)
        .expect("43 §1 transcribes the enum")
        .split('}')
        .next()
        .expect("the block closes");

    let mut names: Vec<String> = block
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let (name, _) = l.split_once(',')?;
            let name = name.trim();
            name.chars()
                .all(|c| c.is_ascii_alphanumeric())
                .then(|| name.to_string())
        })
        .filter(|n| !n.is_empty())
        .collect();
    names.sort();

    // gx-core publishes no `ALL` for this enum (its own unit test writes the six out), so the six
    // are written out here too and the compiler holds the list: the exhaustive `match` below stops
    // compiling if a seventh variant is added, which is the event blocker item 5 would cause
    // (sem: SEM-gx-engine-809).
    let all = [
        gx_core::AbortReason::PreconditionChanged,
        gx_core::AbortReason::ApplyFailed,
        gx_core::AbortReason::VerifierUnavailable,
        gx_core::AbortReason::Expired,
        gx_core::AbortReason::OwnerCancelled,
        gx_core::AbortReason::InternalError,
    ];
    for r in all {
        match r {
            gx_core::AbortReason::PreconditionChanged
            | gx_core::AbortReason::ApplyFailed
            | gx_core::AbortReason::VerifierUnavailable
            | gx_core::AbortReason::Expired
            | gx_core::AbortReason::OwnerCancelled
            | gx_core::AbortReason::InternalError => {}
        }
    }
    let mut declared: Vec<String> = all.iter().map(|r| format!("{r:?}")).collect();
    declared.sort();

    println!("CANON_ABORT_REASONS={names:?}");
    assert_eq!(
        names.len(),
        6,
        "43 §1: \"six variants\" (sem: SEM-gx-engine-810)"
    );
    assert_eq!(
        names, declared,
        "gx-core's AbortReason is not the one 43 §1 transcribes (blocker item 5 would change this) \
         (sem: SEM-gx-engine-811)"
    );
}

/// Every transition id 43 §3 declares, and which of them this hand implements.
///
/// Not an assertion that the missing ones are wrong -- they belong to hands 4, 5 and 6. It is the
/// **coverage denominator** printed early, so that hand 7's `tests/state_machine_coverage.md`
/// (51 §14's named artefact) is built on a number that has been measured since hand 2 rather than
/// counted once at the end.
#[test]
fn the_transition_ids_43_3_declares_and_this_hands_share_of_them() {
    let text = read_repo("req/spec/40-architecture/43-state-machine.md");
    let section = text
        .split("## 3. ") // (sem: SEM-gx-engine-812)
        .nth(1)
        .expect("43 has a §3")
        .split("\n## ")
        .next()
        .expect("§3 is followed by §4");

    let mut ids: Vec<String> = Vec::new();
    for line in section.lines() {
        let line = line.trim();
        if !line.starts_with("| T-") {
            continue;
        }
        let id = line
            .trim_start_matches("| ")
            .split(' ')
            .next()
            .expect("an id in the first column");
        ids.push(id.to_string());
    }

    // req/78 §6.2, hand 2: "**T-1 through T-4e + T-8/T-8r**" (sem: SEM-gx-engine-813).
    let mine = [
        "T-1", "T-2", "T-3", "T-4a", "T-4b", "T-4c", "T-4d", "T-4e", "T-8", "T-8r",
    ];
    println!(
        "CANON_TRANSITIONS={} HAND2_TRANSITIONS={} ({ids:?})",
        ids.len(),
        mine.len()
    );

    // 🔴 **M5H2-7**: the denominator is **21**, and the number carried since req/78 is 19.
    //
    // req/78 §4's M5-14 row writes "the target is 19 IDs (T-1 through T-13 + T-4a/b/c/d/e, T-5b,
    // T-8r, T-10a/b/c)" (sem: SEM-gx-engine-814) and
    // §6.2 hand 7 inherits it, but the parenthetical does not add to 19: T-4, T-5, T-8 and T-10 have
    // no bare rows -- every one of them is lettered -- so 43 §3's table is nine plain ids (T-1, T-2,
    // T-3, T-6, T-7, T-9, T-11, T-12, T-13) plus T-4a..T-4e (5), T-5/T-5b (2), T-8/T-8r (2) and
    // T-10a/b/c (3) = **21**. Counted from the file rather than from the sentence, which is why this
    // probe exists at all.
    //
    // It matters because 51 §14 makes "state-machine branch coverage 100%" a completion condition
    // (sem: SEM-gx-engine-815) and
    // req/38 §37's M5-14, adopted (a), says "19/19 is achievable" (sem: SEM-gx-engine-815). A
    // hand 7 that reports 19/19 against a
    // denominator of 21 would be reporting 90% as 100%.
    assert_eq!(
        ids.len(),
        21,
        "43 §3's table is the denominator 51 §14 gates on, and it is 21 rows (M5H2-7)"
    );
    for id in mine {
        assert!(
            ids.contains(&id.to_string()),
            "{id} is in this hand's scope and 43 §3 no longer declares it"
        );
    }
}
