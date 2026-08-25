// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-044** — INV-S2: a superseded transformation is bit-equal to what it was (FR-040).
//!
//! 34 AC-044, verbatim: "Given: a Committed T_o and the T_u that realizes its inverse. When:
//! comparing the hashes of T_o's canonical record / Receipt before and after T_u reaches
//! `Committed` and T_o transitions to `Superseded`. Then: completely bit-equal (`superseded_by` is
//! append-only metadata; it does not rewrite the canonical structure or receipt body). | property"
//! (sem: SEM-gx-engine-546)
//!
//! # What "property" (sem: SEM-gx-engine-547) buys over `tests/ac_040.rs`'s single run
//!
//! AC-040 (3) compares one pair of digests. This generates the pair — different payloads, different
//! seeds, different clocks, different locators — and compares **four** things each time: the
//! canonical record, the receipt envelope, the ledger leaf, and the canonical bytes of the whole
//! payload. Four, because "does not rewrite" (sem: SEM-gx-engine-548) can fail in four places
//! and only one of them is the record AC-044 names first.
//!
//! # 🔴 The instrument has to be able to fail
//!
//! A probe that compares two digests taken from an engine which **never** supersedes anything would
//! pass forever. So each case asserts the edge was drawn (`superseded_by == Some(t_u)`), and
//! [`ac_044_the_comparison_can_tell_a_change_from_no_change`] shows the same comparison catching a
//! value that did move — the `enforced`-style control §30 keeps asking for.

mod support;

use std::sync::Arc;

use gx_canon::{cbor, cid};
use gx_core::Timestamp;
use gx_engine::{Engine, InverseStatus, Lifecycle};
use proptest::prelude::*;
use support::{gate, intent, scratch, signing_key, CommitAdapter, MaybeEvidence, PERMIT_ALL};

/// The four digests AC-044 compares before and after the supersede.
#[derive(PartialEq, Eq)]
struct Snapshot {
    record: gx_core::Cid,
    /// The signed envelope's **bytes**, not a digest of them: 41 §6 gives this crate no hash of its
    /// own ("every canonical encode goes only through gx-canon"; sem: SEM-gx-engine-549), and
    /// `DsseEnvelope` has no `IdentityView` to hand `cid::compute`. Bytes are the stronger
    /// comparison anyway — "bit-equal" is what AC-044
    /// asks for, and a digest would be a proxy for it.
    receipt_envelope: Vec<u8>,
    receipt_payload: gx_core::Cid,
    ledger_leaf: gx_core::Cid,
}

/// Printed by digest and by length, so a failure names which of the four moved without dumping a
/// signed envelope into a panic message.
impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Snapshot")
            .field("record", &cid::to_text(&self.record))
            .field("envelope_bytes", &self.receipt_envelope.len())
            .field(
                "envelope_head",
                &self
                    .receipt_envelope
                    .iter()
                    .take(8)
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
            )
            .field("receipt_payload", &cid::to_text(&self.receipt_payload))
            .field("ledger_leaf", &cid::to_text(&self.ledger_leaf))
            .finish()
    }
}

fn snapshot<E: gx_engine::EvidenceSource>(
    engine: &Engine<E>,
    id: &gx_core::TransformationId,
    leaf: u64,
) -> Snapshot {
    let receipt = engine
        .receipt(id)
        .expect("a committed transformation has one");
    Snapshot {
        record: cid::compute(engine.transformation(id).expect("the row")).expect("canonical"),
        // The **envelope**, signature included: 42 §1.3-4 keeps signatures out of a payload's
        // identity, so a comparison that used only `ledger_digest` would not notice a re-signature.
        receipt_envelope: cbor::encode(&receipt.envelope).expect("canonical"),
        receipt_payload: receipt.ledger_digest().expect("canonical"),
        ledger_leaf: engine
            .ledger()
            .log()
            .entry(leaf)
            .expect("the leaf is there")
            .receipt_digest,
    }
}

/// Run one case and answer with the two snapshots and what the edge did.
fn one_case(
    tag: &str,
    seed: u64,
    goal: &str,
    at: Timestamp,
) -> (
    Snapshot,
    Snapshot,
    Option<gx_core::TransformationId>,
    Lifecycle,
) {
    let dir = scratch(tag);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        MaybeEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent(&format!("/tmp/inv-s2-{seed}.txt"), goal);
    engine.submit(&i, seed, at).expect("submit");
    let t_o = engine.plan(&i, at).expect("plan");
    engine.verify(&t_o, at, &signing_key(), None).expect("T-4a");
    engine.canonicalize(&t_o, at, None).expect("T-8");
    engine.commit(&t_o, at, &signing_key()).expect("T-11");
    let before = snapshot(&engine, &t_o, 0);

    let undo_at = Timestamp(at.0 + 3_600_000_000_000);
    let (_, t_u) = engine
        .undo(
            &t_o,
            &engine.attested_postcondition(&t_o),
            seed + 1,
            undo_at,
        )
        .expect("the candidate");
    engine
        .verify(&t_u, undo_at, &signing_key(), None)
        .expect("T-4a");
    engine.canonicalize(&t_u, undo_at, None).expect("T-8");
    engine.commit(&t_u, undo_at, &signing_key()).expect("T-11");
    let after = snapshot(&engine, &t_o, 0);

    assert_eq!(
        engine.inverse_status(&t_o),
        Some(InverseStatus::Consumed { by: t_u }),
        "the edge really was drawn, or this case measures nothing"
    );
    (
        before,
        after,
        engine.superseded_by(&t_o),
        engine.state(&t_o).expect("the row is still there"),
    )
}

/// 🔴 AC-044, once, with all four digests printed.
#[test]
fn ac_044_the_original_is_bit_equal_across_the_supersede() {
    let at = Timestamp(1_754_000_000_000_000_000);
    let (before, after, by, state) = one_case("ac044_one", 70, "after", at);
    println!("AC044_BEFORE {before:?}");
    println!("AC044_AFTER  {after:?}");
    println!("AC044 superseded_by={by:?} state={state:?}");
    assert_eq!(state, Lifecycle::Superseded, "T-12 fired");
    assert!(by.is_some(), "and the metadata was appended");
    assert_eq!(
        before, after,
        "43 §5-4 / INV-S2: \"`T_o`'s canonical record/receipt/ledger entry are never rewritten, at \
         all\" (sem: SEM-gx-engine-550)"
    );
}

/// 🔴 The property AC-044 asks for, over generated cases (**M5-15, adopted (b)** (sem:
/// SEM-gx-engine-551): plain `proptest`).
#[test]
fn ac_044_the_property_holds_for_every_generated_case() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: 16,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                1u64..50_000,
                "[a-z]{1,12}",
                1_600_000_000_000_000_000i64..1_900_000_000_000_000_000i64,
            ),
            |(seed, goal, nanos)| {
                let (before, after, by, state) =
                    one_case(&format!("ac044_prop_{seed}"), seed, &goal, Timestamp(nanos));
                prop_assert_eq!(state, Lifecycle::Superseded);
                prop_assert!(by.is_some());
                prop_assert_eq!(before, after);
                Ok(())
            },
        )
        .expect("the property holds");
    println!("AC044_PROPERTY_CASES=16");
}

/// 🔴 The control: the same four-digest comparison **does** notice a change.
///
/// An equality probe over values nothing ever changes is a probe about the fixture. Here the same
/// `Snapshot` is taken over two different transformations in one engine, and every one of the four
/// digests differs — which is what says the comparison above is capable of failing.
#[test]
fn ac_044_the_comparison_can_tell_a_change_from_no_change() {
    let at = Timestamp(1_754_000_000_000_000_000);
    let dir = scratch("ac044_control");
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        MaybeEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let mut snaps = Vec::new();
    for (n, goal) in ["one", "two"].iter().enumerate() {
        let i = intent(&format!("/tmp/control-{n}.txt"), goal);
        engine.submit(&i, 80 + n as u64, at).expect("submit");
        let id = engine.plan(&i, at).expect("plan");
        engine.verify(&id, at, &signing_key(), None).expect("T-4a");
        engine.canonicalize(&id, at, None).expect("T-8");
        engine.commit(&id, at, &signing_key()).expect("T-11");
        snaps.push(snapshot(&engine, &id, n as u64));
    }
    println!("AC044_CONTROL a={:?}", snaps[0]);
    println!("AC044_CONTROL b={:?}", snaps[1]);
    assert_ne!(snaps[0].record, snaps[1].record);
    assert_ne!(snaps[0].receipt_envelope, snaps[1].receipt_envelope);
    assert_ne!(snaps[0].receipt_payload, snaps[1].receipt_payload);
    assert_ne!(snaps[0].ledger_leaf, snaps[1].ledger_leaf);
}
