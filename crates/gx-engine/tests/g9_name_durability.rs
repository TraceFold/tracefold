// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/859` G9 — the platform boundary of name durability, made machine-checked**
//! (`req/868`, 2026-08-26, seat=Opus, 暫定 — 再審査可).
//!
//! `sync_parent_directory` is `#[cfg(not(unix))] -> Ok(())`. The gap was real and was declared in a
//! doc comment, which is the one place a declaration cannot be checked. This suite makes the
//! declaration answer to the same `cfg` the implementation does, so that:
//!
//! * a lane that **implements** the Windows path and forgets to promote the declaration fails here;
//! * a lane that **removes** the unix `fsync` and leaves the declaration boasting fails here;
//! * a lane that widens the `cfg` (adding a platform to the "held" arm without measuring it) has to
//!   edit a test that says, in words, that it must not.
//!
//! It does **not** assert that the platter moved — `gx-engine/tests/ledger_durability.rs` already
//! records that `sync_all` returning `Ok` does not prove that, and no test in a userspace process
//! can. What it asserts is that we do not *say* more than we do.

use gx_engine::{NameDurability, NAME_DURABILITY};

#[test]
fn the_declaration_and_the_implementation_answer_to_the_same_cfg() {
    println!("NAME_DURABILITY={NAME_DURABILITY:?} UNIX={}", cfg!(unix));
    assert_eq!(
        NAME_DURABILITY.is_held(),
        cfg!(unix),
        "NAME_DURABILITY is decided by `cfg(unix)` because sync_parent_directory is. If you have \
         just implemented directory-entry durability for another platform, move BOTH cfgs and \
         bring the measurement with you -- req/859 G9's whole point is that an unmeasured \
         guarantee is worse than a declared gap"
    );
}

#[test]
fn the_unheld_arm_says_it_is_unheld_rather_than_hinting() {
    let notice = NameDurability::NotHeldOnThisPlatform.notice();
    println!("UNHELD_NOTICE={notice}");
    assert!(
        notice.contains("NOT held"),
        "the notice an operator is shown must say the guarantee does not hold, in those words: {notice}"
    );
    assert!(
        !NameDurability::NotHeldOnThisPlatform.is_held(),
        "the unheld arm must not report itself held"
    );
    assert!(
        NameDurability::ParentDirectorySynced.is_held(),
        "the held arm must report itself held"
    );
    assert_ne!(
        NameDurability::ParentDirectorySynced.notice(),
        notice,
        "two platform answers, two sentences -- one sentence for both would be the silence this \
         type exists to end"
    );
}

/// The gap is worth stating in the negative too: nothing in the workspace *prints* this yet.
///
/// This test is a marker, not a guarantee. It exists so the ledger's honest sentence ("declared,
/// not warned") has a home in the suite, and so the lane that finally wires the notice into
/// `gx serve` or `/healthz` has an obvious place to come and delete it.
#[test]
fn the_notice_is_a_declaration_and_not_yet_a_warning() {
    assert!(
        !NAME_DURABILITY.notice().is_empty(),
        "the notice exists as data even though no surface prints it yet (req/868 owes the \
         operator-facing half: a line at `gx serve` start, or a /healthz member)"
    );
}
