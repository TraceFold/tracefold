//! AC-004 (FR-004) — `ChangeContext` survives a serde round trip on every variant.
//!
//! AC-004 逐語: 「Given: `ChangeContext`の全variant（`Time, Evidence, Policy, Model,
//! Representation, Substrate, Custom("x")`）をランダム生成するproptest strategy。When: 各値を
//! serdeでencode→decode。Then: 得られた値が元の値とbit-equal（`Custom`の任意文字列を含む）。」
//!
//! The AC names serde but no format, and gx-core may not name gx-canon (A-1), so the concrete
//! format here is `serde_json` from dev-dependencies. That is deliberately *not* the canonical
//! encoding: canonical DAG-CBOR round trips are AC-009/AC-012 on the gx-canon side (A-4 面1),
//! and this file only asserts that the derive is present and total over the variants.
//!
//! "bit-equal" is read at both ends -- the decoded value equals the original, and re-encoding
//! the decoded value reproduces the same bytes. Value equality alone would let a lossy encoding
//! that happens to normalize pass.

use gx_core::ChangeContext;
use proptest::prelude::*;

fn any_change_context() -> impl Strategy<Value = ChangeContext> {
    prop_oneof![
        Just(ChangeContext::Time),
        Just(ChangeContext::Evidence),
        Just(ChangeContext::Policy),
        Just(ChangeContext::Model),
        Just(ChangeContext::Representation),
        Just(ChangeContext::Substrate),
        // Arbitrary string, not `"x"`: the AC says `Custom`の任意文字列を含む. `.*` covers the
        // empty string, quotes, and non-ASCII, which is where a hand-rolled encoder would break.
        ".*".prop_map(ChangeContext::Custom),
    ]
}

proptest! {
    #[test]
    fn ac_004_round_trip_is_bit_equal(cc in any_change_context()) {
        let bytes = serde_json::to_vec(&cc).expect("encode");
        let back: ChangeContext = serde_json::from_slice(&bytes).expect("decode");
        prop_assert_eq!(&back, &cc);
        let again = serde_json::to_vec(&back).expect("re-encode");
        prop_assert_eq!(again, bytes);
    }
}

#[test]
fn ac_004_every_named_variant_is_reachable() {
    // The proptest above samples; this pins the enumeration itself so a dropped variant is a
    // compile error rather than a strategy that quietly stops generating it (42 §3.2).
    let all = [
        ChangeContext::Time,
        ChangeContext::Evidence,
        ChangeContext::Policy,
        ChangeContext::Model,
        ChangeContext::Representation,
        ChangeContext::Substrate,
        ChangeContext::Custom("x".to_string()),
    ];
    assert_eq!(all.len(), 7);
    for cc in all {
        let bytes = serde_json::to_vec(&cc).expect("encode");
        let back: ChangeContext = serde_json::from_slice(&bytes).expect("decode");
        assert_eq!(back, cc);
        // Exhaustive match: adding an eighth variant without touching this file fails to build.
        match cc {
            ChangeContext::Time
            | ChangeContext::Evidence
            | ChangeContext::Policy
            | ChangeContext::Model
            | ChangeContext::Representation
            | ChangeContext::Substrate
            | ChangeContext::Custom(_) => {}
        }
    }
}
