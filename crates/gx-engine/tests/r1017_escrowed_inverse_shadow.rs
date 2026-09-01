// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R1017 ruling (a)** — `Engine::escrowed_inverse` falls through to the Σ-shadow, so a
//! restarted process cannot say `Available` and `null` about one row in the same breath.
//!
//! `req/987_UNDO_INTENT_WIRE_REQDEF_2026-08-31.md` §13-3 measured the asymmetry and left it open:
//! [`gx_engine::Engine::inverse_status`] carries 43 T6 condition ①'s fall-through (`self.escrow`,
//! then `self.shadow.escrow_of`) and `escrowed_inverse` read `self.table` alone. The table is
//! empty after a restart (M5H3-5), so the two accessors disagreed about one row: "the inverse is
//! here" beside "I hold no name for it".
//!
//! # Why the fall-through and not a seventh word
//!
//! The rule this workspace already applies is *fall through exactly where the value is a component
//! of Σ*. `EscrowRow.inverse_cid` **is** one (E-M5-2: "state table + ledger root + escrow index",
//! `replay.rs`'s `Sigma`), and `SigmaShadow::escrow_of` already hands the whole row to
//! `inverse_status`, which reads its `status` and drops its `inverse_cid`. The opposite arm of the
//! same rule is [`gx_engine::Engine::rollback_not_attempted_because`], which stays table-only and
//! says so in its own doc: the cause "is not a component of Σ", so `None` there is an honest
//! answer. A new `InverseStatus` word spelling "this process holds no name" would be a claim about
//! the process rather than about the escrow, and it would be false the moment it was written —
//! the name is in Σ and the shadow is already holding it.
//!
//! # The shape of the bed
//!
//! One clean commit through the shipped road, the escrow read before and after a restart, and two
//! negative controls: an id no record names must stay `None` (the fall-through invents no rows),
//! and the body must still be retrievable under the name the reopened engine answers (a CID that
//! addresses nothing would be a worse answer than `null`).
//!
//! `tests/supersede.rs::e_m5_9_a_commit_with_no_constructible_inverse_records_the_absence` holds
//! the other half — an escrow whose `inverse_cid` is genuinely `None` — and is unmoved by this
//! change, which is what keeps `None` meaning "no CID" rather than "we lost it".

mod support;

use std::sync::Arc;

use gx_core::{Cid, Timestamp, TransformationId};
use gx_engine::{Engine, InjectedEvidence, InverseStatus, Lifecycle};
use support::{gate, intent, scratch, signing_key, tid, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);

/// One committed transformation with an inverse escrowed against it, and the journal it lives in.
///
/// The shipped road (`submit` → `plan` → `verify` → `canonicalize` → `commit`) over an adapter
/// that refuses nothing, so the commit reaches `Committed` and the escrow is left `Available`
/// rather than `Consumed`.
fn a_committed_row(name: &str) -> (std::path::PathBuf, TransformationId, Cid) {
    let dir = scratch(name);
    let path = dir.join("journal.bin");
    let mut engine = Engine::open(&path, gate(PERMIT_ALL), InjectedEvidence::none())
        .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent("/tmp/target.txt", "after");
    engine.submit(&i, 42, AT).expect("submit");
    let id = engine.plan(&i, AT).expect("plan");
    engine.verify(&id, AT, &signing_key(), None).expect("verify");
    engine.canonicalize(&id, AT, None).expect("canonicalize");
    let state = engine.commit(&id, AT, &signing_key()).expect("commit runs");

    // Bed control: the premise every assertion below rests on. If the commit did not land, or the
    // escrow is not `Available` in the process that made it, the restart measures nothing.
    let cid = engine
        .escrowed_inverse(&id)
        .expect("T-10b escrowed an inverse in this process");
    println!(
        "BED state={state:?} live_status={:?} live_cid={cid:?}",
        engine.inverse_status(&id)
    );
    assert_eq!(state, Lifecycle::Committed);
    assert_eq!(engine.inverse_status(&id), Some(InverseStatus::Available));

    drop(engine);
    (path, id, cid)
}

/// 🔴 The ruling, measured: after a restart the two accessors answer about the same escrow.
#[test]
fn a_reopened_engine_names_the_inverse_it_says_is_available() {
    let (path, id, cid) = a_committed_row("r1017_escrow_shadow");

    let reopened = Engine::open(&path, gate(PERMIT_ALL), InjectedEvidence::none())
        .expect("the journal reopens");

    println!(
        "REOPENED table_rows={} status={:?} cid={:?} expected_cid={cid:?}",
        reopened.transformation_ids().len(),
        reopened.inverse_status(&id),
        reopened.escrowed_inverse(&id)
    );

    // The premise: the live table is empty, so anything answered below came out of the Σ-shadow
    // and not out of a row this process built. Without this the test would pass on an engine that
    // never restarted.
    assert!(
        reopened.transformation_ids().is_empty(),
        "M5H3-5: `open` rebuilds no state rows, which is what makes this a restart"
    );

    // The sibling, unchanged — the negative control for the repair. `inverse_status` carried the
    // fall-through before this lane and must still carry it after.
    assert_eq!(
        reopened.inverse_status(&id),
        Some(InverseStatus::Available),
        "43 T6 condition ①: the escrow's status survives the restart through the Σ-shadow"
    );

    // The repair.
    assert_eq!(
        reopened.escrowed_inverse(&id),
        Some(cid),
        "🔴 `req/987` §13-3: `Available` and a `null` name cannot stand on one row. \
         `EscrowRow.inverse_cid` is a component of Σ (E-M5-2), so the same fall-through owes the \
         same answer"
    );

    // A name that addresses nothing would be a worse answer than `None`: the point of the CID is
    // that the body can be fetched under it (42 §5's exception to ASM-9).
    let body = reopened
        .blobs()
        .get(&cid)
        .expect("42 §5: the escrowed body outlives the process that escrowed it");
    println!("REOPENED_BODY={:?}", String::from_utf8_lossy(body.payload()));
    assert_eq!(
        body.reference().cid,
        cid,
        "content addressing: the reopened engine's name hashes to the body it names"
    );

    // Negative control: the fall-through reads the shadow, it does not invent rows in it.
    let stranger = tid(9_999);
    println!(
        "STRANGER status={:?} cid={:?}",
        reopened.inverse_status(&stranger),
        reopened.escrowed_inverse(&stranger)
    );
    assert_eq!(reopened.inverse_status(&stranger), None);
    assert_eq!(
        reopened.escrowed_inverse(&stranger),
        None,
        "an id no record has named has no escrow, before or after the repair"
    );
}
