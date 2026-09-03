// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 `Engine::deadline` across a restart — the same **T6 condition ①** fall-through
//! [`Engine::state`]/[`Engine::verdict`]/[`Engine::intent_of`]/[`Engine::rollback`]/
//! [`Engine::canonical_cid`]/[`Engine::enforced`]/[`Engine::escrowed_inverse`] already carry, wired
//! into the one sibling that had a private twin (`Engine::shadow_deadline`, **R3 / `req/222`
//! H-04**) built and even used by [`Engine::expire_if_due`] -- but never called from the public,
//! `&self` read accessor itself.
//!
//! # Why this was a live defect and not a theoretical one
//!
//! `crates/gx-api/src/list.rs` puts `engine.deadline(&id)` straight into the `deadline` key of
//! every row `GET /v1/transformations` answers. A restarted server (`Engine::open` leaves the
//! table empty, M5H3-5) held a row this process had not itself planned, in `Candidate` --
//! `reap()`/`expire_if_due` would still have expired it on schedule (it already read the shadow),
//! but the wire answered `"deadline": null` for it in the meantime: the enforcement sweep and the
//! read face disagreed about the same fact, the exact shape `Engine::escrowed_inverse`'s own doc
//! names for the pre-`d3872a4d` escrow pair.
//!
//! This suite reproduces the **restart** half directly (a second `Engine::open` on the same
//! journal file, mirroring `ac_045.rs`'s single-process fixture), asserts the pre-fix answer is
//! `None` for a row that is unmistakably not `None` (the live engine's own `deadline()` in the
//! same test, at the same `id`), and asserts the post-restart answer now agrees with it.

mod support;

use std::sync::Arc;

use gx_core::Timestamp;
use gx_engine::{Engine, InjectedEvidence};
use support::{gate, intent, scratch, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const MS: i64 = 1_000_000;
const TTL: i64 = 100 * MS;

/// 🔴 A `Candidate`'s deadline survives a restart on the read face, not only on the reap face.
#[test]
fn a_shadow_only_candidate_still_answers_its_deadline() {
    let dir = scratch("deadline_shadow_restart");
    let journal = dir.join("journal.bin");

    let (id, live_deadline) = {
        let mut engine = Engine::open(&journal, gate(PERMIT_ALL), InjectedEvidence::none())
            .expect("a fresh journal opens")
            .with_ttl(TTL, TTL);
        let (adapter, _counts, _world) = CommitAdapter::new("before");
        engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

        let i = intent("/tmp/deadline_shadow_restart.txt", "after");
        engine.submit(&i, 1, AT).expect("submit");
        let id = engine.plan(&i, AT).expect("plan");
        let deadline = engine
            .deadline(&id)
            .expect("a live Candidate has a deadline");
        // Dropped here: the table this row lived in goes with `engine`, leaving only the journal
        // on disk -- the same thing a process restart does (M5H3-5).
        (id, deadline)
    };

    // 🔴 A second, independent engine over the same journal: `Engine::open` replays it into the
    // Σ-shadow and leaves `self.table` empty (M5H3-5). Same TTL, because 43 T-6's arithmetic
    // needs it to compare against `live_deadline`. `id` travels from the first engine rather than
    // through `Engine::transformation_ids` -- that accessor is deliberately table-only (its own
    // doc: "the Σ-shadow knows every row ... this accessor still answers only the rows this
    // process holds a body for"), so it is empty here by design and is not the fact under test.
    let mut restarted = Engine::open(&journal, gate(PERMIT_ALL), InjectedEvidence::none())
        .expect("the same journal reopens")
        .with_ttl(TTL, TTL);
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    restarted.register_adapter(Arc::new(adapter), "commit-adapter-1");

    assert!(
        restarted.transformation_ids().is_empty(),
        "M5H3-5: a reopened engine's table starts empty"
    );
    let shadow_deadline = restarted.deadline(&id);

    println!(
        "DEADLINE_SHADOW_RESTART live={live_deadline:?} table_empty={} shadow_answer={shadow_deadline:?}",
        restarted.state(&id).is_some() && restarted.transformation(&id).is_none()
    );

    assert_eq!(
        shadow_deadline,
        Some(live_deadline),
        "a row this process never planned still has a deadline `reap()` would honour \
         (R3 / req/222 H-04); the read accessor must agree with the enforcement sweep"
    );
}
