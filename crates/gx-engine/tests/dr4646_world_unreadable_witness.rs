// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-46 (`req/973` §9-3)** — the other half of the repair: the new absence reaches the
//! **signature**, and is distinguishable there from the one it used to be spelled as.
//!
//! Spec: 42 §3.10 for the `undo` seat on `ReceiptPayload`, 42 §3.13 for `Planned.undo_witness`,
//! 43 §5.2 for the witness vocabulary.
//!
//! # Why this file exists beside the CLI one
//!
//! `crates/gx-cli/tests/dr4646_world_unreadable.rs` measures that the pre-flight's arms *produce*
//! `Unobservable::WorldUnreadable`. It cannot measure what a receipt carries, because on the fs
//! substrate a world the pre-flight could not read is a world `Engine::undo` cannot snapshot either:
//! that road ends in an adapter refusal and no receipt is minted. So the two facts are measured
//! where each is measurable — **the arm produces the witness** up there, **the witness reaches the
//! signed bytes** here — and neither file is asked to imply the other.
//!
//! The bed is `Engine::undo`'s witness argument, which is the same public seam both shipped surfaces
//! feed (`gx-cli`'s `settle_preflight`, `gx-api`'s `undo_witness`), and the adapter under it is the
//! in-memory `CommitAdapter`, whose position is readable — which is exactly the substrate on which a
//! transient read failure would be survivable and the lie therefore reachable.

mod support;

use std::sync::Arc;

use gx_core::{Timestamp, TransformationId};
use gx_engine::{Engine, InjectedEvidence, Lifecycle, UndoWitness, Unobservable};
use gx_witness::receipt::{UndoAttestation, UndoDisposition};
use support::{gate, intent, scratch, signing_key, CommitAdapter, PERMIT_ALL};

const AT: Timestamp = Timestamp(1_754_000_000_000_000_000);
const LATER: Timestamp = Timestamp(1_754_000_120_000_000_000);

/// A committed `T_o` and a committed undo of it, driven with `witness`.
fn undone(
    name: &str,
    witness: UndoWitness,
) -> (Engine<InjectedEvidence>, TransformationId, TransformationId) {
    let dir = scratch(name);
    let mut engine = Engine::open(
        dir.join("journal.bin"),
        gate(PERMIT_ALL),
        InjectedEvidence::none(),
    )
    .expect("a fresh journal opens");
    let (adapter, _counts, _world) = CommitAdapter::new("before");
    engine.register_adapter(Arc::new(adapter), "commit-adapter-1");

    let i = intent(&format!("/tmp/{name}.txt"), "after");
    engine.submit(&i, 460, AT).expect("submit");
    let t_o = engine.plan(&i, AT).expect("plan");
    engine.verify(&t_o, AT, &signing_key(), None).expect("T-4a");
    engine.canonicalize(&t_o, AT, None).expect("T-8");
    assert_eq!(
        engine.commit(&t_o, AT, &signing_key()).expect("T-11"),
        Lifecycle::Committed
    );

    let (_, t_u) = engine
        .undo(&t_o, &witness, 461, LATER)
        .expect("the candidate");
    engine
        .verify(&t_u, LATER, &signing_key(), None)
        .expect("T-4b");
    engine.canonicalize(&t_u, LATER, None).expect("T-8");
    assert_eq!(
        engine.commit(&t_u, LATER, &signing_key()).expect("T-11"),
        Lifecycle::Committed
    );
    (engine, t_o, t_u)
}

/// The `undo` seat of a transformation's commit receipt.
fn attestation(
    engine: &Engine<InjectedEvidence>,
    id: &TransformationId,
) -> Option<UndoAttestation> {
    engine
        .receipt(id)
        .expect("the transformation committed, so T-11 issued a receipt")
        .payload()
        .expect("the payload this build signed decodes")
        .undo
}

/// 🔴 An undo that fired because the **world** would not read says that, in the bytes its key
/// signed.
#[test]
fn a_world_that_would_not_read_is_named_as_such_in_the_signed_receipt() {
    let (engine, t_o, t_u) = undone(
        "dr4646_world_unreadable",
        UndoWitness::Unobservable(Unobservable::WorldUnreadable),
    );
    let seat = attestation(&engine, &t_u);
    println!("DR4646_SIGNED seat={seat:?}");
    assert_eq!(
        seat,
        Some(UndoAttestation {
            undoes: t_o,
            witness: UndoDisposition::Unobservable {
                reason: Unobservable::WorldUnreadable.reason().to_string(),
            },
        }),
        "DR-46-46: the absence the pre-flight actually had is the one a third party reads back"
    );
}

/// 🔴 The discrimination the repair is **for**: the world not reading and the receipt carrying no
/// postcondition are two facts, and a reader holding the receipt alone can now tell them apart.
///
/// Before the repair both sites answered with the second sentence, so two different situations wore
/// one face inside the signature — the same defect DR-46-45 exists to close one level up, at the
/// same seam, one variant deeper.
#[test]
fn it_is_distinguishable_from_a_receipt_that_carried_no_postcondition() {
    let (world, _, world_t_u) = undone(
        "dr4646_discrimination_world",
        UndoWitness::Unobservable(Unobservable::WorldUnreadable),
    );
    let (receipt, _, receipt_t_u) = undone(
        "dr4646_discrimination_receipt",
        UndoWitness::Unobservable(Unobservable::NoPostcondition),
    );

    let a = attestation(&world, &world_t_u).expect("an undo");
    let b = attestation(&receipt, &receipt_t_u).expect("an undo");
    println!("DR4646_WORDS a={} b={}", a.witness.word(), b.witness.word());

    assert_ne!(
        a.witness, b.witness,
        "DR-46-46: if these are equal the variant bought nothing and the two sites may as well have \
         kept the old word"
    );
    assert!(
        a.witness.word().starts_with("unobservable:")
            && b.witness.word().starts_with("unobservable:"),
        "both are still declarations rather than refusals (DR-46-7, `req/38` §123 ruling 1): the \
         repair names which nothing it was, it does not turn an unobservable face into a wall"
    );
}
