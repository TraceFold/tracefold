//! AC-012 (FR-012) — canon is idempotent, and the encoder is what decides.
//!
//! AC-012 逐語: 「Given: ランダム生成されたTransformation x。When: `canon(canon(x))`と`canon(x)`を
//! それぞれ計算。Then: 両者がbit-equal（Lean T3-aに対応、12 F0 T3）。」property（proptest,
//! ≥1000ケース/PR）。
//!
//! 42 §2.3 spells `canon` out for implementers:
//! `encode_canonical(decode(encode_canonical(x))) == encode_canonical(x)`. That is the first
//! property below, applied to `Transformation` as the AC names and to `ObjectSnapshot` as well,
//! since a property of one struct is not a property of the encoding.
//!
//! The rest of the file is T-21, the half of the requirement that is easy to satisfy vacuously.
//! Idempotence holds trivially for an encoder that normalises everything it is handed; what
//! makes it mean something is that `is_canonical` refuses the spellings the encoder would not
//! have written. `ASM-01-2` names the failure being avoided: `alpha-ledger::scan_strict` was
//! built on its own parser and so returned `Ok` for a CRLF-corrupted line, because the parser
//! did. The two vectors P-1 and P-2 are the cases where that distinction is visible in DAG-CBOR
//! -- `serde_ipld_dagcbor` parses both, and `is_canonical` still says no.

mod support;

use gx_canon::cbor;
use gx_core::{ObjectSnapshot, Transformation};
use ipld_core::ipld::Ipld;
use proptest::prelude::*;
use std::path::Path;
use support::{any_object_snapshot, any_transformation, hex, unhex};

proptest! {
    /// 42 §2.3 verbatim, on the type AC-012 names.
    #[test]
    fn ac_012_canon_is_idempotent_on_transformation(x in any_transformation()) {
        let once = cbor::encode(&x).expect("encode");
        let round: Transformation = cbor::decode(&once).expect("decode");
        let twice = cbor::encode(&round).expect("re-encode");
        prop_assert_eq!(hex(&twice), hex(&once));
    }

    #[test]
    fn ac_012_canon_is_idempotent_on_object_snapshot(x in any_object_snapshot()) {
        let once = cbor::encode(&x).expect("encode");
        let round: ObjectSnapshot = cbor::decode(&once).expect("decode");
        let twice = cbor::encode(&round).expect("re-encode");
        prop_assert_eq!(hex(&twice), hex(&once));
    }

    /// Whatever the encoder writes, it agrees is canonical. A `false` here would mean the
    /// predicate and the encoder had drifted apart, which is the one thing an encoder-defined
    /// predicate cannot survive.
    #[test]
    fn ac_012_the_encoder_output_is_canonical_by_its_own_predicate(x in any_transformation()) {
        let bytes = cbor::encode(&x).expect("encode");
        prop_assert!(cbor::is_canonical(&bytes), "encoder output rejected: {}", hex(&bytes));
    }

    /// The idempotent step, expressed through the predicate rather than through equality: a
    /// value that came out of a canonical byte string goes back to the same byte string.
    #[test]
    fn ac_012_canonical_bytes_survive_a_value_round_trip(x in any_transformation()) {
        let bytes = cbor::encode(&x).expect("encode");
        let value: Ipld = cbor::decode(&bytes).expect("decode as untyped");
        prop_assert_eq!(hex(&cbor::encode(&value).expect("re-encode")), hex(&bytes));
    }
}

/// T-21, stated as the thing it is meant to prevent: `is_canonical` must not be a synonym for
/// "the parser accepted it". Each negative vector is fed to the predicate; two of them
/// (`P-1`, a 64-bit float, and `P-2`, a well-formed tag 42 link) parse fine and are still not
/// canonical, because 42 §2.1-4 and §2.1-5 keep floats and tags out of gx values.
#[test]
fn ac_012_is_canonical_is_decided_by_the_encoder_not_by_the_parser() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/negative");
    let mut checked = 0;
    let mut parsed_but_not_canonical = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    entries.sort();
    for path in entries {
        let text = std::fs::read_to_string(&path).expect("read vector");
        let v: serde_json::Value = serde_json::from_str(&text).expect("parse vector");
        let id = v["id"].as_str().expect("id").to_string();
        let bytes = unhex(v["vector"].as_str().expect("vector"));
        assert!(
            !cbor::is_canonical(&bytes),
            "{id}: is_canonical accepted a negative vector"
        );
        if serde_ipld_dagcbor::from_slice::<Ipld>(&bytes).is_ok() {
            parsed_but_not_canonical.push(id);
        }
        checked += 1;

        if let Some(control) = v.get("control") {
            let cb = unhex(control["vector"].as_str().expect("control.vector"));
            let expect_canonical = control["expected"].as_str() == Some("ACCEPT");
            assert_eq!(
                cbor::is_canonical(&cb),
                expect_canonical,
                "{}: control vector disagreed with its declared expectation",
                v["id"]
            );
        }
    }
    // The second literal count over the same directory (`negative_vectors.rs` holds the other).
    // Both are here on purpose: this file asks the *encoder* about every vector and that one asks
    // the *scanner*, so a vector added to one reader and not the other would leave one face
    // unchecked. 12 -> 18 with H6-4's six additional-info boundary vectors, -> 19 with F-2's D-65,
    // -> 21 with M3 hand 4's D-65K (A-9: the ceiling reached while reading a map key) and TR-1
    // (F-3: the `missing` an early-ending input reports).
    assert_eq!(checked, 21, "vector count changed");
    // Non-vacuity. If this list were empty the predicate could be nothing more than the parser
    // and every assertion above would still pass.
    assert!(
        parsed_but_not_canonical.len() >= 2,
        "no vector distinguishes the encoder from the parser; got {parsed_but_not_canonical:?}"
    );
    println!("PARSED_BUT_NOT_CANONICAL={parsed_but_not_canonical:?}");
}

/// The two layers of the decode path must agree about which byte strings are admissible.
/// `scan_strict` is gx's independent reading of 42 §2.1; `is_canonical` is the encoder's
/// judgement. They are computed by entirely different code, and a byte string one admits and the
/// other refuses would mean `decode` and the canonical predicate disagree -- which is how a
/// value ends up with two spellings and identity stops being a function of content.
#[test]
fn ac_012_the_scanner_and_the_encoder_agree() {
    // Canonical encodings, their negative counterparts, and byte-level mutations of both.
    let mut cases: Vec<Vec<u8>> = vec![
        unhex("01"),
        unhex("20"),
        unhex("f6"),
        unhex("f5"),
        unhex("f4"),
        unhex("40"),
        unhex("60"),
        unhex("80"),
        unhex("a0"),
        unhex("a2616101616202"),
        unhex("a261620162616102"),
        unhex("1bffffffffffffffff"),
        unhex("3bffffffffffffffff"),
        unhex("5820000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"),
        unhex("a2616181016162a1616301"),
    ];
    let seeds = cases.clone();
    for seed in seeds {
        for i in 0..seed.len() {
            for delta in [1u8, 0x20, 0x80] {
                let mut m = seed.clone();
                m[i] = m[i].wrapping_add(delta);
                cases.push(m);
            }
            let mut truncated = seed.clone();
            truncated.truncate(i);
            cases.push(truncated);
            let mut extended = seed.clone();
            extended.push(0x00);
            cases.push(extended);
        }
    }

    let mut disagreements = Vec::new();
    for bytes in &cases {
        let scanner_ok = cbor::scan_strict(bytes).is_ok();
        if scanner_ok != cbor::is_canonical(bytes) {
            disagreements.push(format!(
                "{}: scan_strict={scanner_ok} is_canonical={}",
                hex(bytes),
                cbor::is_canonical(bytes)
            ));
        }
    }
    assert!(
        disagreements.is_empty(),
        "the scanner and the encoder disagree on {} of {} inputs:\n{}",
        disagreements.len(),
        cases.len(),
        disagreements.join("\n")
    );
    println!("LAYER_AGREEMENT_INPUTS={}", cases.len());
}
