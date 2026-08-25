// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The canonical map order, measured rather than assumed (H3-1).
//!
//! Every field table in this crate is declared in the order 42 §2.1-2 wants the encoded map to
//! carry, and hand 2 recorded a safety net for it (req/51 §3.6: "even with the wrong order,
//! `encode` fails with `NotCanonicalizable` (= it does not stay silent)" (sem: SEM-gx-log-166)). That net does not exist. `serde_ipld_dagcbor`
//! sorts a struct's keys itself, so declaration order changes nothing about the bytes and a
//! wrongly ordered declaration fails nothing.
//!
//! Which does not make the convention wrong -- writing the fields in encoded order is still what
//! makes the canonical form the obvious form -- but a stated mechanical guarantee that is not
//! mechanical is exactly what req/08 N-1 is about, so it is measured here and raised as H3-1 in
//! req/52 §4 rather than left in a doc comment.
//!
//! The second assertion is the one hand 2's prose got wrong in the other direction: E-42-3
//! (req/38 §4) settles that the order is over the **encoded** key -- length first, letters second
//! -- and not over the bare UTF-8 bytes, which several doc comments in this crate still describe
//! as "bytewise" (sem: SEM-gx-log-167). `index` before `appended_at` is the case that tells the two readings apart.

use gx_canon::cbor;
use serde::Serialize;

/// The field set of `LedgerEntry`, declared in encoded-key order.
#[derive(Serialize)]
struct EncodedOrder {
    index: u64,
    leaf_cid: u64,
    appended_at: i64,
    receipt_digest: u64,
    transformation: u64,
}

/// The same set, declared in the order a reader who took 42 §2.1-2's prose literally would write.
#[derive(Serialize)]
struct BytewiseOrder {
    appended_at: i64,
    index: u64,
    leaf_cid: u64,
    receipt_digest: u64,
    transformation: u64,
}

/// The same set again, in no order at all.
#[derive(Serialize)]
struct NoOrder {
    transformation: u64,
    appended_at: i64,
    receipt_digest: u64,
    index: u64,
    leaf_cid: u64,
}

fn encoded_order() -> Vec<u8> {
    cbor::encode(&EncodedOrder {
        index: 2,
        leaf_cid: 3,
        appended_at: 1,
        receipt_digest: 4,
        transformation: 5,
    })
    .expect("canonical")
}

/// Declaration order does not reach the bytes: the encoder sorts.
#[test]
fn declaration_order_does_not_change_the_encoding() {
    let bytewise = cbor::encode(&BytewiseOrder {
        appended_at: 1,
        index: 2,
        leaf_cid: 3,
        receipt_digest: 4,
        transformation: 5,
    })
    .expect("canonical");
    let none = cbor::encode(&NoOrder {
        transformation: 5,
        appended_at: 1,
        receipt_digest: 4,
        index: 2,
        leaf_cid: 3,
    })
    .expect("canonical");

    assert_eq!(encoded_order(), bytewise);
    assert_eq!(encoded_order(), none);
}

/// A declaration in any order still produces canonical bytes, so the ordering convention in this
/// crate's field tables is a readability rule and not a checked one (H3-1).
#[test]
fn a_misordered_declaration_is_not_refused() {
    assert!(
        cbor::encode(&NoOrder {
            transformation: 5,
            appended_at: 1,
            receipt_digest: 4,
            index: 2,
            leaf_cid: 3,
        })
        .is_ok(),
        "if this ever fails, the safety net req/51 §3.6 claimed has appeared and H3-1 can close"
    );
}

/// The order the encoder actually writes is length first (E-42-3), not bare bytewise.
#[test]
fn the_encoded_order_is_length_first() {
    let bytes = encoded_order();
    let at = |key: &str| {
        bytes
            .windows(key.len())
            .position(|w| w == key.as_bytes())
            .unwrap_or_else(|| panic!("{key} is not in the encoding"))
    };

    assert!(
        at("index") < at("leaf_cid"),
        "a 5-byte key precedes an 8-byte one"
    );
    assert!(
        at("leaf_cid") < at("appended_at"),
        "an 8-byte key precedes an 11-byte one"
    );
    assert!(
        at("appended_at") < at("receipt_digest"),
        "an 11-byte key precedes a 14-byte one -- and under bare bytewise order `appended_at` \
         would come first of all, which is the case that separates the two readings"
    );
    assert!(
        at("receipt_digest") < at("transformation"),
        "equal lengths fall back to the letters"
    );
}
