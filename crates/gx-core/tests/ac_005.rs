// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-005 (FR-005) — `Commutation` survives a serde round trip, residual CID included.
//!
//! AC-005, verbatim (quoted in SEM-gx-core-112): "Given: `Commutation::Conflicts{residual:
//! DeltaRef(cid)}` and `Commutation::Commutes`. When: a serde round trip. Then: every field,
//! including the CID in `residual`, is restored bit-equal."
//!
//! The interesting field is the 32 raw bytes inside `DeltaRef.cid`: an encoding that dropped or
//! reordered them would still produce a `Conflicts` that looks right at the variant level, so
//! the property generates the bytes rather than reusing a fixed value (42 §1.1, §3.4).

use gx_core::{Cid, Commutation, DeltaRef, SubstrateKind};
use proptest::prelude::*;

fn any_substrate_kind() -> impl Strategy<Value = SubstrateKind> {
    prop_oneof![
        Just(SubstrateKind::Fs),
        Just(SubstrateKind::Git),
        Just(SubstrateKind::Mcp),
        ".*".prop_map(SubstrateKind::Custom),
    ]
}

fn any_cid() -> impl Strategy<Value = Cid> {
    any::<[u8; 32]>().prop_map(Cid)
}

fn any_delta_ref() -> impl Strategy<Value = DeltaRef> {
    (any_substrate_kind(), any_cid()).prop_map(|(substrate, cid)| DeltaRef { substrate, cid })
}

fn any_commutation() -> impl Strategy<Value = Commutation> {
    prop_oneof![
        Just(Commutation::Commutes),
        any_delta_ref().prop_map(|residual| Commutation::Conflicts { residual }),
    ]
}

proptest! {
    #[test]
    fn ac_005_round_trip_is_bit_equal(c in any_commutation()) {
        let bytes = serde_json::to_vec(&c).expect("encode");
        let back: Commutation = serde_json::from_slice(&bytes).expect("decode");
        prop_assert_eq!(&back, &c);
        let again = serde_json::to_vec(&back).expect("re-encode");
        prop_assert_eq!(again, bytes);
    }

    #[test]
    fn ac_005_residual_cid_bytes_are_preserved_exactly(raw in any::<[u8; 32]>()) {
        let c = Commutation::Conflicts {
            residual: DeltaRef { substrate: SubstrateKind::Git, cid: Cid(raw) },
        };
        let back: Commutation =
            serde_json::from_slice(&serde_json::to_vec(&c).expect("encode")).expect("decode");
        match back {
            Commutation::Conflicts { residual } => {
                prop_assert_eq!(residual.cid.0, raw);
                prop_assert_eq!(residual.substrate, SubstrateKind::Git);
            }
            Commutation::Commutes => prop_assert!(false, "variant changed across the round trip"),
        }
    }
}

/// E-JCS-1 (`req/38_ERRATA_2026-08-07.md` §5): a `Cid` embedded in JSON is spelled
/// `gx1:<base32>`, not a thirty-two element array of numbers.
///
/// 42 §1.2 is verbatim on the point -- "human-readable display in CLI/API/logs and **JSON
/// embedding** all take this form as canonical" (quoted in SEM-gx-core-113) -- and hand 4 shipped
/// the array form, which req/42 §5-2 raised and the
/// erratum settled against. The AC-005 round trip alone cannot see the difference (an array
/// round trips as faithfully as a string), so the spelling is asserted here rather than left to
/// be inferred from a property that passes either way.
#[test]
fn ac_005_a_cid_is_spelled_gx1_inside_json() {
    let c = Commutation::Conflicts {
        residual: DeltaRef {
            substrate: SubstrateKind::Git,
            cid: Cid([0u8; 32]),
        },
    };
    let json = serde_json::to_string(&c).expect("encode");

    assert!(
        json.contains("\"gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\""),
        "42 §1.2 spells an embedded Cid `gx1:<base32>`; got {json}"
    );
    assert!(
        !json.contains("[0,0,0"),
        "the array spelling is the one E-JCS-1 removed; got {json}"
    );

    let back: Commutation = serde_json::from_str(&json).expect("decode");
    assert_eq!(back, c, "the string spelling must still round trip");
}

#[test]
fn ac_005_commutes_has_no_payload_to_lose() {
    let c = Commutation::Commutes;
    let back: Commutation =
        serde_json::from_slice(&serde_json::to_vec(&c).expect("encode")).expect("decode");
    assert_eq!(back, Commutation::Commutes);
}
