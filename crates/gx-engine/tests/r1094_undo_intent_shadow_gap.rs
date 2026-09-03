// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **Known gap, deliberately left `#[ignore]`d** — `Engine::undo_intent` disagrees with
//! `Engine::inverse_status`/`Engine::escrowed_inverse` across a restart, and this suite is the
//! `req/1094` §5-0/§14-1 falsifier (F-3) reproduced directly rather than reasoned about.
//!
//! `req/1094_INVERSE_EXPOSURE_REQDEF_2026-09-02.md` §14-1 asks: does `undo_intent` need the same
//! shape [`r1017_escrowed_inverse_shadow.rs`] gave `Engine::escrowed_inverse` (commit `d3872a4d`)?
//! Investigated (this lane) and answered **no, not that shape** -- `self.shadow` (`StateRow` /
//! `EscrowRow`, `replay.rs`) carries none of `locator`/`context`/`actor`, and `undo_intent` needs
//! all three to build an `Intent` (`gx-core/src/intent.rs`'s five fields). A one-line
//! `.or_else(|| self.shadow...)` -- the shape `escrowed_inverse` and `Engine::deadline`
//! (`crates/gx-engine/tests/deadline_shadow_restart.rs`) both took -- is not implementable here
//! without either inventing new Σ fields (a frozen-Σ change, out of this lane's boundary per
//! `req/1094` §2) or reading `gx-api`'s external `DraftArchive` from inside `Engine` (a layering
//! violation `Engine` has no access to today -- `Engine::drafted` tracks only `IntentId -> rng_seed`,
//! never the `Intent` body).
//!
//! The mechanism that *does* close this gap already exists, one layer up: `crates/gx-api/src/
//! handlers.rs`'s `rebuilt()`/`with_a_body` (today wired into `get_transformation`/`get_candidate`
//! by commit `c9a4056e`, the same day this gap was found) loads the `Intent` from the draft
//! archive and calls `Engine::rehydrate_committed`, which seats a *complete* `Entry` -- `pre`
//! (locator included, via `planned_record`), `delta`, and `transformation` (context/actor
//! included, via `intent.context()`/`intent.actor()`) -- back into `self.table`. Once that runs,
//! `undo_intent`'s existing `self.table.get(original)` succeeds unmodified. `POST .../undo`
//! already calls `with_a_body` before `engine.undo_intent(&id)` (`handlers.rs:1454`/`1491`), which
//! is why this gap has never been client-visible: `undo_intent`'s result is not on any wire
//! response today (`req/1094` §3-2, confirmed independently here by leaving the assertion below
//! unguarded by any wire test). The gap only matters for `req/1094`'s Phase 1 (owner: api lane,
//! not this one) -- a future `GET /v1/transformations/{id}` handler must call the same
//! `rebuilt()`/`with_a_body` step *before* reading `undo_intent`, exactly as `get_transformation`
//! already does before reading `transformation()`/`precondition_fingerprint()`.
//!
//! This test is `#[ignore]`d rather than fixed or deleted: it is the negative control for whatever
//! lane wires Phase 1, and it should start passing the moment that lane's handler calls the
//! rebuild step first (no change to this file should be needed then -- only its `#[ignore]`
//! removed, with a pointer to the landing commit).

mod support;

use std::sync::Arc;

use gx_core::Timestamp;
use gx_engine::{Engine, InjectedEvidence, InverseStatus, Lifecycle};
use support::{gate, intent, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// 🔴 `req/1094` F-3, reproduced: a restarted engine says `Available` and `None` about one row's
/// escrowed inverse in the same breath, because `escrowed_inverse` falls through to the Σ-shadow
/// (`d3872a4d`) and `undo_intent` does not.
#[test]
#[ignore = "req/1094 T-3: known gap, engine-level fix DEFERRED (self.shadow lacks locator/context/actor); \
            the repair belongs in a future api-lane GET handler via rebuilt()/with_a_body, not here -- \
            un-ignore once that lane lands and cite the commit"]
fn undo_intent_disagrees_with_inverse_status_after_a_restart() {
    let dir = scratch("r1094_undo_intent_shadow_gap");
    let path = dir.join("journal.bin");
    let id = {
        let mut engine = Engine::open(&path, gate(PERMIT_ALL), InjectedEvidence::none())
            .expect("a fresh journal opens");
        let (adapter, _counts, _world) = CommitAdapter::new("before");
        engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

        let i = intent("/tmp/undo_intent_shadow_gap.txt", "after");
        engine.submit(&i, 42, AT).expect("submit");
        let id = engine.plan(&i, AT).expect("plan");
        engine.verify(&id, AT, &signing_key(), None).expect("verify");
        engine.canonicalize(&id, AT, None).expect("canonicalize");
        let state = engine.commit(&id, AT, &signing_key()).expect("commit runs");

        // Bed control, mirroring r1017_escrowed_inverse_shadow.rs's own.
        assert_eq!(state, Lifecycle::Committed);
        assert_eq!(engine.inverse_status(&id), Some(InverseStatus::Available));
        assert!(
            engine.undo_intent(&id).expect("live table, no error").is_some(),
            "the live process can build the undo's intent"
        );
        id
    };

    let reopened =
        Engine::open(&path, gate(PERMIT_ALL), InjectedEvidence::none()).expect("reopens");
    assert!(
        reopened.transformation_ids().is_empty(),
        "M5H3-5: a reopened engine's table starts empty"
    );

    let status = reopened.inverse_status(&id);
    let intent = reopened.undo_intent(&id).expect("no blob-store error");
    println!("REOPENED status={status:?} undo_intent_is_some={}", intent.is_some());

    assert_eq!(
        status,
        Some(InverseStatus::Available),
        "escrowed_inverse's own fallback (d3872a4d) already carries this"
    );
    // 🔴 The gap, asserted directly rather than described: today this is `None`, disagreeing with
    // `status` above on the same row. `req/1094` F-3 names this exact pair as the falsifier.
    assert!(
        intent.is_some(),
        "req/1094 F-3: `inverse_status` says Available and `undo_intent` says None about the same \
         row -- the gap this suite exists to keep visible"
    );
}
