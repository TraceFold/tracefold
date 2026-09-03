// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 `Engine::planned_delta` across a restart -- the one real gap the T-r90 census
//! (`crates/gx-engine/src/pipeline.rs:2942`, commit `97f288ab`/`8ac24ae5`) found unprotected on the
//! wire, at `crates/gx-api/src/stream.rs:486` (`events_for`'s `Planned` arm, read by every
//! `GET /v1/stream` backlog replay) and, this suite also checks directly, at
//! `crates/gx-cli/src/pipeline.rs:257` (`gx plan`'s JSON body).
//!
//! # Why this is not the same shape as `deadline_shadow_restart.rs` / `r1017_escrowed_inverse_shadow.rs`
//!
//! Those two return a **name** (`Option<Timestamp>`, `Option<Cid>`) that Σ's `StateRow`/`EscrowRow`
//! already carries, so the fix was `.or_else(|| self.shadow...)` on an unchanged return type.
//! `planned_delta` returns the **body** (`Option<&PlannedDelta>`), and `replay.rs`'s own doc on
//! `StateRow` states the boundary directly: "**Names and digests, never bodies** (ASM-9). A
//! `PlannedDelta` is here as its CID and the body is in the `BlobStore`" (`replay.rs:724-726`).
//! Σ-shadow alone cannot answer this accessor after a restart -- not because the information is
//! gone (the CID survives in `StateRow::delta_cid`, confirmed live in this test) but because a body
//! is architecturally the wrong thing to ask Σ for.
//!
//! What *does* survive a restart, already, is the blob itself: `Engine::plan` calls
//! `self.blobs.put(&delta)` for both the fresh-plan and the "rehydrating" road
//! (`pipeline.rs:4607`/`4701`), `BlobStore` is disk-backed (`store.rs:3073-3086`), and
//! `crash_recovery.rs`'s own `a_restart_restores_the_drafts_and_the_recovery_needs_no_table` already
//! demonstrates a restarted engine reading the delta body back through `self.blobs.get(&delta_cid)`
//! for the crash-recovery road -- while asserting, on the very same row and the very same restart,
//! that `engine.planned_delta(&id).is_none()` (`crash_recovery.rs:637`). That assertion is this
//! gap's other face: it documents the *current* accessor's blindness to a body its own engine
//! already holds on disk, one call away.
//!
//! This suite plants the gap directly, on the same accessor `stream.rs`/`gx-cli` call, mirroring
//! `deadline_shadow_restart.rs`'s two-`Engine::open` shape rather than `r1094`'s `#[ignore]`d one,
//! because -- unlike `undo_intent` (which needs `locator`/`context`/`actor` Σ does not carry) --
//! this repair is a mirror of an already-landed pattern (`d3872a4d`'s shadow fall-through) plus an
//! already-landed store (`BlobStore`), not an invented concept.

mod support;

use std::sync::Arc;

use gx_core::Timestamp;
use gx_engine::{Engine, InjectedEvidence};
use support::{gate, intent, scratch, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// 🔴 The negative control: a restarted engine's `planned_delta` disagrees with the live engine's,
/// for the same row, even though the CID that names the body (`StateRow::delta_cid`) and the body
/// itself (on disk, in `BlobStore`) both survive the restart untouched.
#[test]
fn planned_delta_survives_a_restart() {
    let dir = scratch("planned_delta_shadow_restart");
    let journal = dir.join("journal.bin");

    let (id, live_payload, live_cid) = {
        let mut engine = Engine::open(&journal, gate(PERMIT_ALL), InjectedEvidence::none())
            .expect("a fresh journal opens");
        let (adapter, _counts, _world) = CommitAdapter::new("before");
        engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

        let i = intent("/tmp/planned_delta_shadow_restart.txt", "after");
        engine.submit(&i, 7, AT).expect("submit");
        let id = engine.plan(&i, AT).expect("plan");
        let delta = engine
            .planned_delta(&id)
            .expect("a live Candidate has its delta");
        let payload = delta.payload().to_vec();
        let cid = delta.reference().cid;

        // 🔴 Plant, then print what was planted, before the process-equivalent drop below.
        println!(
            "LIVE id={id:?} delta_cid={cid:?} payload_len={} blob_present={}",
            payload.len(),
            engine.blobs().contains(&cid)
        );
        assert!(
            engine.blobs().contains(&cid),
            "plant check: Engine::plan must have already written this delta's body to the disk-backed \
             BlobStore before this line, or the restart half of this test proves nothing"
        );
        // Dropped here: the table this row lived in goes with `engine`, leaving only the journal
        // and the blob store on disk -- the same thing a process restart does (M5H3-5).
        (id, payload, cid)
    };

    // 🔴 A second, independent engine over the same journal: `Engine::open` replays it into the
    // Σ-shadow and leaves `self.table` empty (M5H3-5), exactly as `deadline_shadow_restart.rs` and
    // `crash_recovery.rs` already establish for this repo.
    let restarted =
        Engine::open(&journal, gate(PERMIT_ALL), InjectedEvidence::none()).expect("reopens");

    assert!(
        restarted.transformation_ids().is_empty(),
        "M5H3-5: a reopened engine's table starts empty"
    );
    // The name survives (Σ-shadow carries `delta_cid`) -- confirms this is a body problem, not a
    // name problem, before reading the accessor under test.
    let shadow_cid = restarted
        .shadow()
        .row(&id)
        .and_then(|r| r.delta_cid);
    // The body survives too, one layer down (this is the fix's ingredient, read directly rather than
    // through the accessor under test, so a failure below cannot be blamed on the blob store).
    let blob_present = shadow_cid.is_some_and(|c| restarted.blobs().contains(&c));

    let restarted_delta = restarted.planned_delta(&id);
    println!(
        "RESTARTED id={id:?} table_empty=true shadow_cid={shadow_cid:?} blob_present={blob_present} \
         accessor_answer_is_some={}",
        restarted_delta.is_some()
    );

    assert_eq!(
        shadow_cid,
        Some(live_cid),
        "the name survives the restart (StateRow::delta_cid) -- if this fails, the gap is upstream \
         of planned_delta and this test's premise is wrong"
    );
    assert!(
        blob_present,
        "the body survives the restart on disk (BlobStore) -- if this fails, `crash_recovery.rs`'s \
         own claim about `engine.blobs().contains(&delta_cid)` after a restart is stale and this \
         test's premise is wrong"
    );

    // 🔴 The gap, asserted directly rather than described: today `planned_delta` does not fall
    // through to the blob store the two checks above just proved is right there, so this fails
    // pre-fix and is expected to pass once `Engine::planned_delta` reads `self.blobs` on a table
    // miss (mirroring `escrowed_inverse`'s `d3872a4d` shape, one layer further down because the
    // Σ-shadow itself carries only the name -- see this file's module doc).
    assert!(
        restarted_delta.is_some(),
        "planned_delta must answer for a row whose name and body both survived the restart, the same \
         way escrowed_inverse/inverse_status/deadline already do for their own rows"
    );
    assert_eq!(
        restarted_delta.map(|d| d.payload().to_vec()),
        Some(live_payload),
        "the restarted body must be byte-identical to the live one, not merely present"
    );
}
