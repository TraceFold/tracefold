// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-001 (FR-001) — `ObjectSnapshot` and representation independence.
//!
//! AC-001, verbatim (`req/spec/30-requirements/34-acceptance.md`; quoted in SEM-gx-core-103):
//! "Given: code that builds two `ObjectSnapshot`s whose `representation` is `Bytes` and `Json`.
//! When: the same core API function (e.g. a generic accessor) is applied to both. Then: it can be
//! confirmed that the function signature is identical regardless of the `representation` kind
//! (compilation succeeds in both cases and no type annotation has to change)."
//!
//! Field list from 41 §3 / 42 §3.1: `id`, `substrate`, `locator`, `digest`, `representation`.
//! P-10 is the point being tested: the core is not allowed to branch on representation, so a
//! single accessor has to serve `Bytes` and `Json` with no type annotation anywhere.

use gx_core::{Cid, ObjectId, ObjectSnapshot, ReprKind, SubstrateKind};

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

/// One signature, no generics, no `ReprKind` in sight. If the core ever needed to know the
/// representation to answer this, the parameter type would have to change and this file
/// would stop compiling -- which is exactly the compile-time check AC-001 asks for.
fn locator_of(x: &ObjectSnapshot) -> &str {
    x.locator()
}

#[test]
fn ac_001_same_accessor_serves_bytes_and_json() {
    let as_bytes = ObjectSnapshot::new(
        ObjectId(cid(1)),
        SubstrateKind::Fs,
        "/tmp/a.txt".to_string(),
        cid(2),
        ReprKind::Bytes,
    );
    let as_json = ObjectSnapshot::new(
        ObjectId(cid(3)),
        SubstrateKind::Fs,
        "/tmp/a.json".to_string(),
        cid(4),
        ReprKind::Json,
    );

    // Same call, both values, no turbofish and no annotation.
    assert_eq!(locator_of(&as_bytes), "/tmp/a.txt");
    assert_eq!(locator_of(&as_json), "/tmp/a.json");

    // The other accessors of the same shape.
    assert_eq!(as_bytes.digest(), &cid(2));
    assert_eq!(as_json.digest(), &cid(4));
    assert_eq!(as_bytes.substrate(), &SubstrateKind::Fs);
    assert_eq!(as_json.substrate(), &SubstrateKind::Fs);
}

#[test]
fn ac_001_all_four_repr_kinds_take_the_same_path() {
    // 42 §3.1 fixes the enumeration at four. `External` and `Tree` are as core-opaque as the
    // other two: the accessor below never learns which one it was handed.
    let kinds = [
        ReprKind::Bytes,
        ReprKind::Json,
        ReprKind::Tree,
        ReprKind::External,
    ];
    for (i, k) in kinds.into_iter().enumerate() {
        let snap = ObjectSnapshot::new(
            ObjectId(cid(i as u8)),
            SubstrateKind::Custom("x".to_string()),
            format!("loc-{i}"),
            cid(i as u8),
            k,
        );
        assert_eq!(locator_of(&snap), format!("loc-{i}"));
    }
}
