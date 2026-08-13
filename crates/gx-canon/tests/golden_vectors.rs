//! T-31 — golden vectors: a logical value and the exact bytes it must encode to.
//!
//! Five raw JSON files in `tests/vectors/golden/`, each holding the value in serde's JSON
//! form and the expected DAG-CBOR bytes in hex. req/05 §8b R-8 asks for exactly this shape and
//! for it to live in the same repository as the implementation, with three counter-examples
//! named: cedar (three repositories to keep in sync), DSSE (a spec with no vectors), in-toto
//! (vectors that live in a prototype). A file that needs a generator to be read would repeat
//! the same mistake in miniature, so the pairs are literal.
//!
//! The hex was written out from RFC 8949 §3's major-type table and the six rules of 42 §2.1,
//! not copied from the encoder's output. That direction matters: a golden vector taken from the
//! implementation can only ever say that the implementation has not changed, whereas one taken
//! from the spec says that the implementation matches the spec. The `origin` field of each file
//! records which of the two it is.
//!
//! What the five cover, beyond regression: `Cid` as a 32-byte string rather than an array of
//! integers (G-1, 42 §1.1), map keys ordered by their encoded bytes rather than by declaration
//! (G-2), all ten `Transformation` fields at once (G-3), serde's externally tagged enum shapes
//! (G-4), and the three shapes that are easy to get subtly wrong -- `None`, an empty vector, and
//! a non-ASCII string whose CBOR length is in bytes and not characters (G-5).

mod support;

use gx_canon::cbor;
use gx_core::{Cid, DeltaRef, ObjectSnapshot, Transformation};
use std::path::{Path, PathBuf};
use support::{hex, unhex};

const EXPECTED_GOLDEN_COUNT: usize = 5;

fn golden_files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/golden");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("golden directory {} unreadable: {e}", dir.display()))
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    assert_eq!(
        paths.len(),
        EXPECTED_GOLDEN_COUNT,
        "golden vector count changed; update EXPECTED_GOLDEN_COUNT and the report"
    );
    paths
}

/// Deserialise the file's `value` into the type its `type` field names, encode it, and compare
/// bytes. The dispatch is a `match` rather than anything clever so that adding a type to the
/// vector set is a compile-time decision.
fn encode_declared_value(kind: &str, value: serde_json::Value) -> Vec<u8> {
    match kind {
        "Cid" => cbor::encode(&from_json::<Cid>(value)),
        "ObjectSnapshot" => cbor::encode(&from_json::<ObjectSnapshot>(value)),
        "Transformation" => cbor::encode(&from_json::<Transformation>(value)),
        "DeltaRef" => cbor::encode(&from_json::<DeltaRef>(value)),
        other => panic!("golden vector names an unknown type: {other}"),
    }
    .expect("golden values must encode")
}

fn from_json<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).expect("golden value does not match its declared type")
}

#[test]
fn golden_every_vector_encodes_to_its_declared_bytes() {
    let mut mismatches = Vec::new();
    for path in golden_files() {
        let text = std::fs::read_to_string(&path).expect("read golden vector");
        let v: serde_json::Value = serde_json::from_str(&text).expect("parse golden vector");
        let id = v["id"].as_str().expect("id");
        let kind = v["type"].as_str().expect("type");
        let expected = v["hex"].as_str().expect("hex").to_string();
        let got = hex(&encode_declared_value(kind, v["value"].clone()));
        if got != expected {
            mismatches.push(format!(
                "{id} ({kind})\n  expected {expected}\n  got      {got}"
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "golden vectors disagree:\n{}",
        mismatches.join("\n")
    );
}

/// The bytes go back to the value. A golden pair where only one direction worked would still
/// pin the encoder while saying nothing about the decoder.
#[test]
fn golden_every_vector_decodes_back_to_its_value() {
    for path in golden_files() {
        let text = std::fs::read_to_string(&path).expect("read golden vector");
        let v: serde_json::Value = serde_json::from_str(&text).expect("parse golden vector");
        let id = v["id"].as_str().expect("id");
        let kind = v["type"].as_str().expect("type");
        let bytes = unhex(v["hex"].as_str().expect("hex"));
        let value = v["value"].clone();
        let round = match kind {
            "Cid" => {
                let x: Cid = cbor::decode(&bytes).unwrap_or_else(|e| panic!("{id}: {e}"));
                serde_json::to_value(x)
            }
            "ObjectSnapshot" => {
                let x: ObjectSnapshot =
                    cbor::decode(&bytes).unwrap_or_else(|e| panic!("{id}: {e}"));
                serde_json::to_value(x)
            }
            "Transformation" => {
                let x: Transformation =
                    cbor::decode(&bytes).unwrap_or_else(|e| panic!("{id}: {e}"));
                serde_json::to_value(x)
            }
            "DeltaRef" => {
                let x: DeltaRef = cbor::decode(&bytes).unwrap_or_else(|e| panic!("{id}: {e}"));
                serde_json::to_value(x)
            }
            other => panic!("unknown type {other}"),
        }
        .expect("re-serialise");
        assert_eq!(
            round, value,
            "{id}: decoded value differs from the file's value"
        );
    }
}

/// Every golden byte string is canonical by the crate's own predicate, and passes the
/// independent scanner. If a golden vector were not canonical, the file would be pinning a bug.
#[test]
fn golden_every_vector_is_canonical() {
    for path in golden_files() {
        let text = std::fs::read_to_string(&path).expect("read golden vector");
        let v: serde_json::Value = serde_json::from_str(&text).expect("parse golden vector");
        let id = v["id"].as_str().expect("id");
        let bytes = unhex(v["hex"].as_str().expect("hex"));
        assert_eq!(v["expected"].as_str(), Some("ACCEPT"), "{id}");
        assert!(
            cbor::is_canonical(&bytes),
            "{id}: not canonical by the encoder's judgement"
        );
        assert!(
            cbor::scan_strict(&bytes).is_ok(),
            "{id}: the scanner rejected a golden vector"
        );
    }
}

/// 42 §1.1 in isolation: a `Cid` is `0x58 0x20` followed by thirty-two raw bytes. The derived
/// encoding of `[u8; 32]` would be a thirty-two element array of integers instead -- one byte
/// per element for small values, two for anything above 23, and no way for a reader to tell it
/// from a list of small numbers. G-1 covers this too; it is spelled out separately because it is
/// the one shape the rest of the identity face (step 4) is built on.
#[test]
fn golden_cid_is_a_byte_string_not_an_array() {
    let cid = Cid([0x5a; 32]);
    let bytes = cbor::encode(&cid).expect("encode");
    assert_eq!(
        bytes.len(),
        34,
        "two header bytes and thirty-two payload bytes"
    );
    assert_eq!(bytes[0], 0x58, "major type 2, one-byte length");
    assert_eq!(bytes[1], 32);
    assert_eq!(&bytes[2..], &[0x5a; 32]);
}
