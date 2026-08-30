// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/868` R-868-5 / `req/919` W4 — the adapter's own directory-durability gap, made
//! machine-checked (2026-08-29).**
//!
//! `crates/gx-adapter-fs/src/apply.rs`'s `sync_parent_directory` used to be called un-`cfg`-gated
//! (`std::fs::File::open(parent)?.sync_all()` inline, at both the write and the removal call sites),
//! so on native Windows `apply`/`remove` reported failure for a change that had already landed --
//! the exact sibling of `req/859` G9's gap in `gx-engine`'s journal directory, and closed the same
//! way: the gap is now `#[cfg(unix)]`/`#[cfg(not(unix))]`-gated data (`NameDurability` /
//! `NAME_DURABILITY`), not a comment.
//!
//! This suite mirrors `crates/gx-engine/tests/g9_name_durability.rs` exactly, on this crate's own
//! copy of the type -- see the crate's doc comment on `sync_parent_directory` for why this crate
//! duplicates the type rather than depending on `gx-engine` for it (E-M4-26's zero-dependency
//! declaration; adapters sit below the engine in the layer doctrine).
//!
//! # What this crate did not run, said honestly (three-valued: this is UNTESTABLE, not a pass)
//!
//! `req/919` W4's AC allows a WSL-side-only test if `cargo check --target x86_64-pc-windows-msvc`
//! cannot run at all. It cannot: this WSL toolchain has only `wasm32-unknown-unknown`,
//! `x86_64-unknown-linux-gnu` and `x86_64-unknown-linux-musl` installed (`rustup target list
//! --installed`), and `cargo check -p gx-adapter-fs --target x86_64-pc-windows-msvc` fails with
//! `E0463: can't find crate for 'core'`/`'std'` before reaching this crate at all -- the target is
//! not installed, not that the code is wrong for it. `req/859`'s own `sync_parent_directory` has the
//! identical property: a `#[cfg(not(unix))]` item is not even parsed for type errors on a unix build,
//! only stripped, so no unix CI run -- this one included -- has ever type-checked either crate's
//! Windows arm. That is a pre-existing, accepted limitation this lane inherits rather than
//! introduces (`crates/gx-engine/tests/g9_name_durability.rs`'s own suite does not close it either).
//! What **is** checked, on this platform, is everything below: the `NameDurability` type itself is
//! unconditionally compiled (only the `NAME_DURABILITY` const's *value* is `cfg`-selected), so both
//! enum variants and both `notice()` arms run and are asserted here regardless of which one this
//! build's `NAME_DURABILITY` resolves to.

use gx_adapter_fs::{NameDurability, NAME_DURABILITY};

#[test]
fn the_declaration_and_the_implementation_answer_to_the_same_cfg() {
    println!("NAME_DURABILITY={NAME_DURABILITY:?} UNIX={}", cfg!(unix));
    assert_eq!(
        NAME_DURABILITY.is_held(),
        cfg!(unix),
        "NAME_DURABILITY is decided by `cfg(unix)` because sync_parent_directory is. If you have \
         just implemented directory-entry durability for another platform, move BOTH cfgs and \
         bring the measurement with you -- req/868 R-868-5's whole point (borrowed from req/859 \
         G9) is that an unmeasured guarantee is worse than a declared gap"
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

/// The gap is worth stating in the negative too: `apply`'s reported error no longer mentions this
/// step off unix, because there is no longer a step there to fail (R-868-5's fix, not merely its
/// declaration). This is a marker, not a guarantee about the platter -- see
/// `crates/gx-engine/tests/ledger_durability.rs`'s own caveat, which applies here identically.
#[test]
fn nothing_prints_the_notice_yet_and_that_is_the_named_residual() {
    assert!(
        !NAME_DURABILITY.notice().is_empty(),
        "the sentence exists and is available to be asked for (R-868-1's residual: an operator is \
         not yet *told*, because printing it is a CLI/wire change out of this lane's box)"
    );
}
