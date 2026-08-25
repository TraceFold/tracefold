// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/38` §324 ruling 3** — the two things the leaf repair rests on, asserted here rather
//! than assumed one crate up.
//!
//! `gx-witness` derives a receipt's ledger leaf from the bytes that were signed. That road is only
//! as good as two claims this crate owns:
//!
//! 1. **`cid::of_canonical_bytes` answers what `cid::compute` answers.** If it did not, the repair
//!    would not be "the same number reached honestly", it would be a new number — and every leaf in
//!    every ledger would have to move, which is the opposite of the point.
//! 2. **`cbor::value_span` lands on exactly the value it names.** A span off by one byte produces a
//!    digest that is wrong in a way no reader could see: it would still be a digest, of bytes that
//!    mean nothing.
//!
//! Both are measured over values built here, so a failure names this crate rather than travelling.

use gx_canon::cbor;
use gx_canon::cid::{self, IdentityView};
use serde::{Deserialize, Serialize};

/// A map with members on both sides of the one this file clears, so a span that ran long or short
/// would swallow a neighbour and the assertions below would see it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Shaped {
    /// Sorts before the target: shorter key.
    a: u64,
    /// The member every assertion here is about. Nested, so a span that stopped at the first
    /// closing byte of an inner item would be caught.
    inclusion_proof: Option<Nested>,
    /// Sorts after the target, and carries text — so a span that ate into it would produce bytes
    /// that are still canonical CBOR and a digest that is still a digest.
    zulu: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Nested {
    depth: u64,
    path: Vec<u64>,
}

impl IdentityView for Shaped {
    type View<'a> = &'a Shaped;
    fn identity_view(&self) -> &Shaped {
        self
    }
}

fn filled() -> Shaped {
    Shaped {
        a: 7,
        inclusion_proof: Some(Nested {
            depth: 3,
            path: vec![11, 22, 33],
        }),
        zulu: "a value long enough to be a run rather than a byte".to_string(),
    }
}

fn cleared() -> Shaped {
    Shaped {
        inclusion_proof: None,
        ..filled()
    }
}

// ---------------------------------------------------------------------------
// Claim 1 — the third road is the same number
// ---------------------------------------------------------------------------

/// 🔴 `of_canonical_bytes(encode(v))` **is** `compute(v)`.
///
/// The repair replaces one call with the other on the verification road. If these two ever
/// disagreed, every receipt this build issues would carry a leaf its own verifier refuses — the
/// failure would be total and immediate, which is the only reason it is safe to make the swap at
/// all. Asserted rather than reasoned, because "obviously the same" is what a private `digest`
/// with two public wrappers invites somebody to assume after the second wrapper grows a parameter.
#[test]
fn the_bytes_road_and_the_value_road_answer_the_same_digest() {
    for value in [filled(), cleared()] {
        let bytes = cbor::encode(&value).expect("canonical");
        let by_value = cid::compute(&value).expect("canonical");
        let by_bytes = cid::of_canonical_bytes(&bytes).expect("canonical");
        println!(
            "BYTES_ROAD len={} by_value={} by_bytes={}",
            bytes.len(),
            cid::to_text(&by_value),
            cid::to_text(&by_bytes)
        );
        assert_eq!(by_value, by_bytes);
    }
    // And the two *values* do not share a digest, so the equality above is not the trivial one
    // that would hold if either road ignored its input.
    assert_ne!(
        cid::compute(&filled()).expect("canonical"),
        cid::compute(&cleared()).expect("canonical")
    );
}

/// The bytes road audits what it is handed.
///
/// A digest over bytes nobody checked would let two spellings of one value carry two leaves — the
/// property `cbor::decode`'s strictness exists to prevent. `f9` is `false` written as a
/// two-byte simple value, which RFC 8949 admits and 42 §2.1 does not.
#[test]
fn the_bytes_road_refuses_bytes_the_encoder_would_not_have_written() {
    for bad in [
        vec![],                             // empty
        vec![0xa1],                         // a map header with nothing after it
        vec![0xf8, 0x14],                   // `false` in a two-byte spelling
        vec![0x1b, 0, 0, 0, 0, 0, 0, 0, 1], // 1, written in eight bytes
    ] {
        let answer = cid::of_canonical_bytes(&bad);
        println!("BYTES_ROAD_REFUSED {bad:?} -> {}", answer.is_err());
        assert!(
            answer.is_err(),
            "a leaf may not be minted over a spelling no encoder would have written: {bad:?}"
        );
    }
    // The control: the good bytes of the same shape are accepted, so the refusals above are about
    // the spelling and not about the road being closed.
    assert!(cid::of_canonical_bytes(&cbor::encode(&filled()).expect("canonical")).is_ok());
}

// ---------------------------------------------------------------------------
// Claim 2 — the span is exactly the value
// ---------------------------------------------------------------------------

/// 🔴 Splicing `f6` over the span turns the filled bytes into the cleared bytes, **byte for byte**.
///
/// This is the whole of the leaf repair's arithmetic, stated as an equality between two byte
/// strings rather than as a claim about a digest. A digest comparison would pass for a span that
/// was wrong in a way that happened to produce the same hash — which is not a real risk, and is
/// also not what a reader needs to see. What they need to see is that "clear the member" and
/// "encode it as `None`" are the same operation, because that is what makes one road able to
/// answer for documents the other road wrote years earlier.
#[test]
fn splicing_null_over_the_span_gives_the_bytes_the_encoder_writes_for_none() {
    let filled_bytes = cbor::encode(&filled()).expect("canonical");
    let cleared_bytes = cbor::encode(&cleared()).expect("canonical");

    let span = cbor::value_span(&filled_bytes, "inclusion_proof")
        .expect("the bytes are canonical")
        .expect("the map has the key");
    println!(
        "SPAN key=inclusion_proof at={}..{} filled={} cleared={}",
        span.start,
        span.end,
        filled_bytes.len(),
        cleared_bytes.len()
    );

    let mut spliced = filled_bytes[..span.start].to_vec();
    spliced.push(0xf6);
    spliced.extend_from_slice(&filled_bytes[span.end..]);
    assert_eq!(
        spliced, cleared_bytes,
        "clearing the member at the byte level has to be the same thing the encoder does for a \
         `None` at that key — if these differ, the leaf repair is computing a number no build ever \
         wrote"
    );

    // The map header did not move: the key is still there, holding `null`. Removing it instead
    // would decrement the count and produce a leaf that matches nothing in any ledger.
    assert_eq!(
        filled_bytes[0], cleared_bytes[0],
        "the map keeps its member count; one value became `null`"
    );
}

/// A key the map does not hold answers `None`, and a key that is a substring of another does not
/// match it.
///
/// The second half is the one worth a test: the walk compares the **decoded** key text after the
/// header the sort order is over, so `proof` must not find `inclusion_proof`.
#[test]
fn a_key_the_map_does_not_hold_is_absent_and_a_substring_is_not_a_match() {
    let bytes = cbor::encode(&filled()).expect("canonical");
    for absent in ["proof", "inclusion", "nclusion_proof", "", "a_"] {
        let found = cbor::value_span(&bytes, absent).expect("the bytes are canonical");
        println!("SPAN_ABSENT key={absent:?} -> {found:?}");
        assert!(
            found.is_none(),
            "`{absent}` is not a key of this map, and a span for it would be a span into the \
             middle of a value"
        );
    }
    // The control: two keys that *are* present resolve, and to different spans.
    let a = cbor::value_span(&bytes, "a")
        .expect("canonical")
        .expect("present");
    let z = cbor::value_span(&bytes, "zulu")
        .expect("canonical")
        .expect("present");
    println!("SPAN_PRESENT a={a:?} zulu={z:?}");
    assert!(a.end <= z.start, "the map is walked in order");
}

/// The walk carries `scan_strict`'s strictness rather than a looser one of its own.
///
/// A span reader that accepted bytes the decoder refuses would be a second, weaker entrance to the
/// same values — which is the shape 42 §2.1-6 exists to forbid. Measured on the key-order rule,
/// because that is the clause a hand-rolled scanner is most likely to drop and the one this
/// function's correctness depends on ("the first entry with this key" is only "the only entry with
/// this key" while the order holds).
#[test]
fn the_span_walk_refuses_what_the_strict_scan_refuses() {
    // `{"b": 1, "a": 2}` — two text keys, out of order. Well-formed CBOR, not canonical.
    let unsorted = vec![0xa2, 0x61, b'b', 0x01, 0x61, b'a', 0x02];
    assert!(
        cbor::scan_strict(&unsorted).is_err(),
        "the fixture has to be something the strict scan already refuses, or this measures nothing"
    );
    let answer = cbor::value_span(&unsorted, "a");
    println!("SPAN_UNSORTED -> {}", answer.is_err());
    assert!(
        answer.is_err(),
        "an unsorted map is not a map this workspace reads, on either road"
    );

    // A top-level item that is not a map at all.
    let not_a_map = cbor::encode(&7u64).expect("canonical");
    assert!(cbor::value_span(&not_a_map, "a").is_err());

    // Trailing bytes after a complete map.
    let mut trailing = cbor::encode(&filled()).expect("canonical");
    trailing.push(0x01);
    assert!(cbor::value_span(&trailing, "a").is_err());
}
