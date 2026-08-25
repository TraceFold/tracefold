// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The fs delta grammar: canonical DAG-CBOR, a free monoid, and exactly one operation in v0.1.
//!
//! # Three rulings, one payload
//!
//! * **M4-13, adopted (a)** (req/38 §28): "v0.1's fs delta is a **single whole-file replacement** -- atomicity is from `rename`...
//!   **v0.1's `apply` accepts only a sequence of `len==1`**; `len>1` is Err (unimplemented, stated explicitly -- it does not run non-atomically in silence, i.e.
//!   fail-closed)". (sem: SEM-gx-adapter-fs-196)
//! * **M4-07, adopted (c)**: the composite is a **free monoid** in the payload -- "the fs payload is a 'sequence of single-file
//!   operations', and concatenating the sequence is the witness of composition" -- so the shape is a sequence even while only one (sem: SEM-gx-adapter-fs-197)
//!   length is accepted. A grammar that admitted a single operation and nothing else would have no
//!   room for the composition the ruling named.
//! * **N-14** (req/69 §1): "the fs delta's payload is canonical DAG-CBOR (42 §3.4), so it is within existing
//!   `fuzz_dagcbor_decode`'s reach -- **it would be a different matter if the adapter wrote its own parser**". So this adapter (sem: SEM-gx-adapter-fs-198)
//!   writes no parser: it derives `Serialize`/`Deserialize` and hands the bytes to gx-canon, which is
//!   also what keeps 41 §6's "canonical encoding goes through gx-canon" true of an adapter's own grammar. (sem: SEM-gx-adapter-fs-199)
//!
//! # What "concatenation is the witness" means for a CBOR array (sem: SEM-gx-adapter-fs-200)
//!
//! The mock of hand 3 used a framed byte format, where concatenating two payloads concatenated two
//! sequences. Canonical DAG-CBOR cannot do that -- an array carries its length in the head -- so the
//! monoid operation is on the **sequences**, and the payload is what one sequence encodes to. That is
//! the honest reading of M4-07(c) under N-14, and [`concatenation_is_the_composition`] measures the
//! associativity that makes it a monoid at all.

mod support;

use gx_adapter_fs::{FsDelta, FsOp, MAX_OPS};
use gx_canon::cbor;

fn op(locator: &str, content: &[u8]) -> FsOp {
    FsOp::write(locator.to_string(), content.to_vec())
}

/// The payload is canonical DAG-CBOR, judged by the encoder rather than by a parse (ASM-01-2).
#[test]
fn the_payload_is_canonical_dag_cbor() {
    let payload = FsDelta::one(op("/tmp/x", b"after"))
        .encode()
        .expect("a one-operation sequence has a canonical form");
    println!("FS_DELTA_PAYLOAD_BYTES={}", payload.len());
    assert!(
        cbor::is_canonical(&payload),
        "the payload is not what gx-canon's encoder would have written, so two byte strings could \
         name one delta (42 §2.1)"
    );
}

/// It round-trips through gx-canon, and this adapter wrote no parser to do it (**N-14**).
#[test]
fn the_grammar_round_trips_through_gx_canon() {
    let original = FsDelta::one(op("/tmp/x", b"after"));
    let payload = original.encode().expect("an encoding");
    let back = FsDelta::decode(&payload).expect("the adapter reads its own grammar");
    assert_eq!(back, original);
    assert_eq!(back.ops()[0].locator(), "/tmp/x");
    assert_eq!(back.ops()[0].content(), Some(b"after".as_slice()));
}

/// A removal is a distinct operation and survives the round trip.
///
/// AC-049 (hand 5) asks for "creation / change / deletion, the three kinds", so the grammar has to be able to say all three (sem: SEM-gx-adapter-fs-201)
/// before hand 5 can plan them. Hand 4 plans only the replacement -- an intent carries a goal, and a
/// goal of "nothing" has no spelling in 42 §3.3 -- which is recorded here rather than left as a (sem: SEM-gx-adapter-fs-202)
/// silence.
#[test]
fn a_removal_has_a_spelling_of_its_own() {
    let removal = FsDelta::one(FsOp::remove("/tmp/x".to_string()));
    let payload = removal.encode().expect("an encoding");
    assert!(cbor::is_canonical(&payload));
    let back = FsDelta::decode(&payload).expect("a removal reads back");
    assert_eq!(back.ops()[0].content(), None);
    assert_ne!(
        payload,
        FsDelta::one(op("/tmp/x", b""))
            .encode()
            .expect("an encoding"),
        "'remove the file' and 'make the file empty' are two changes and must not be one payload (sem: SEM-gx-adapter-fs-203)"
    );
}

/// **M4-13(a)**: a sequence longer than one is refused, loudly, and not run half-way.
#[test]
fn a_longer_sequence_is_refused_rather_than_run() {
    let two = FsDelta::of(vec![op("/tmp/x", b"one"), op("/tmp/y", b"two")]);
    let payload = two.encode().expect("the grammar can say it");
    let refusal = FsDelta::decode(&payload).expect_err("v0.1 accepts one operation");

    println!("FS_DELTA_LEN2_REFUSAL={}", refusal.kind());
    assert_eq!(
        refusal.kind(),
        "Unimplemented",
        "a two-operation sequence is not malformed, it is unsupported: 'it does not run non-atomically in silence, i.e. \
         fail-closed' (M4-13(a)), and 45 §3 names a multi-file `apply` as TH-3's residual condition (sem: SEM-gx-adapter-fs-204)"
    );
    assert_eq!(MAX_OPS, 1, "the bound v0.1 declares, in one place");
}

/// An empty sequence is refused too, and as a different thing.
///
/// The unit of the free monoid is a legal *value* and not a legal *v0.1 payload*: it describes no
/// file operation, so it cannot be applied and cannot be inverted. That is a payload this adapter
/// would never have written, which is what [`gx_substrate::Error::PayloadUnreadable`] is for --
/// unlike the two-operation case, which the adapter could write once hand 5's successor supports it.
#[test]
fn the_empty_sequence_is_not_a_v0_1_payload() {
    let payload = FsDelta::of(Vec::new()).encode().expect("the unit encodes");
    let refusal = FsDelta::decode(&payload).expect_err("v0.1 needs exactly one operation");
    assert_eq!(refusal.kind(), "PayloadUnreadable");
}

/// The monoid: concatenating sequences is associative, and the empty sequence is its unit.
///
/// **M4-07, adopted (c)** is the ruling this measures -- "the shape is a **free monoid**... just the associativity law" -- and it is (sem: SEM-gx-adapter-fs-205)
/// measured on the sequences rather than on the bytes, for the reason the module documentation
/// gives. Nothing here claims the general law the crate root explicitly refuses ("the general law (a composite
/// arrow's delta = the composition of its parts) is not claimed"): this is associativity of concatenation, which is the (sem: SEM-gx-adapter-fs-206)
/// whole of what a free monoid promises.
#[test]
fn concatenation_is_the_composition() {
    let a = vec![op("/tmp/a", b"1")];
    let b = vec![op("/tmp/b", b"2")];
    let c = vec![op("/tmp/c", b"3")];

    let left = FsDelta::of([a.clone(), b.clone()].concat());
    let left = FsDelta::of([left.ops().to_vec(), c.clone()].concat());
    let right = FsDelta::of([b.clone(), c.clone()].concat());
    let right = FsDelta::of([a.clone(), right.ops().to_vec()].concat());
    assert_eq!(left, right, "(a·b)·c and a·(b·c) are the same sequence");

    let unit = FsDelta::of(Vec::new());
    assert_eq!(
        FsDelta::of([a.clone(), unit.ops().to_vec()].concat()),
        FsDelta::of(a.clone()),
        "the empty sequence is the unit"
    );
}

/// Bytes that are not this grammar are refused as a payload, not as a crash.
#[test]
fn foreign_bytes_are_refused() {
    for bytes in [b"".as_slice(), b"not cbor at all", &[0xffu8, 0xff]] {
        let refusal = FsDelta::decode(bytes).expect_err("these are not an fs delta");
        assert_eq!(refusal.kind(), "PayloadUnreadable", "for {bytes:?}");
    }
}

/// A relative locator is refused as **not a position**, and not as a failed application
/// (**M4H5-5, adopted (b)**). (sem: SEM-gx-adapter-fs-207)
///
/// req/38 §33, verbatim: "**adding the `NotAPosition` variant** (refusing a relative locator is not 'application failure' but 'the argument is not a
/// position' -- reusing ApplyFailed would misstate the fact, the same three-evils fallacy as Unimplemented)". Hand 5 spelled this (sem: SEM-gx-adapter-fs-208)
/// [`gx_substrate::Error::ApplyFailed`] and raised it against itself (req/74 §2 M4H5-5); the word
/// exists now, and this is the probe that keeps the fact and its name together. A relative locator is
/// a legal **value** of the grammar -- L7 is defined over every string -- and an illegal thing to act
/// on (**ASM-69-3**), which is why the refusal lives at [`FsOp::position`] and not in `decode`.
#[test]
fn a_relative_position_is_refused_as_not_a_position() {
    let refusal = op("relative/x", b"after")
        .position()
        .expect_err("v0.1 names positions from the root");
    println!(
        "FS_OP_RELATIVE_REFUSAL={} MESSAGE={refusal}",
        refusal.kind()
    );
    assert_eq!(
        refusal.kind(),
        "NotAPosition",
        "'the argument is not a position' and 'the delta could not be applied' are different \
         facts, and 43 T-11 turns the second into `AbortReason::ApplyFailed` (sem: SEM-gx-adapter-fs-209)"
    );
    assert!(
        refusal.to_string().contains("relative/x"),
        "the refusal does not name the spelling that was refused: {refusal}"
    );
    assert_eq!(
        op("/absolute/x", b"after")
            .position()
            .expect("an absolute locator is a position"),
        "/absolute/x",
        "the control: the same call on a position answers with the normalised spelling"
    );
}

/// The delta an adapter plans carries exactly this payload, so the grammar is not a second story.
#[test]
fn the_planned_delta_carries_the_grammar() {
    use gx_adapter_fs::FsAdapter;
    use support::{Sandbox, GOAL, SUBJECT};

    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let delta = support::planned(&adapter, &locator, GOAL);

    let decoded = FsDelta::decode(delta.payload()).expect("the adapter reads what it wrote");
    assert_eq!(decoded.ops().len(), MAX_OPS);
    assert_eq!(decoded.ops()[0].locator(), locator);
    assert_eq!(decoded.ops()[0].content(), Some(GOAL));
}
