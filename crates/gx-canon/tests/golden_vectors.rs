// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
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
use gx_core::{
    Checkpoint, Cid, DeltaRef, DsseSignature, ObjectSnapshot, Timestamp, Transformation,
    VerdictCheckpoint, VerdictTally,
};
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

/// 🔴 **NFR-011 close, C2 (DAG-CBOR face)** — the canonical encoding of a `DsseSignature` is a
/// **map of exactly two entries** (`a2`), keys in canonical order, and no third entry.
///
/// The hex below is written out from RFC 8949 §3's major-type table and 42 §2.1's ordering rule
/// (encoded keys compared bytewise, header included), not taken from the encoder: `a2` (map, 2
/// entries); `63 736967` ("sig" — its 4-byte encoded key sorts before "keyid"'s 6-byte one);
/// `44 deadbeef` (4 raw bytes, major type 2 — M2H1-4's byte-string face); `65 6b65796964`
/// ("keyid"); `65 6b65792d31` ("key-1"). `req/171` §1-7 measured the same 22 bytes from the live
/// encoder, so spec-derivation and measurement agree.
///
/// Why the entry *count* is the claim: `req/38` §109 (DR-46-5, option (b)) fixes the wire form (sem: SEM-gx-canon-085)
/// `{keyid, sig}` permanently — the signing algorithm is a property of the verifier's pinned key
/// (its very type, `ed25519_dalek::VerifyingKey`), never a wire field, per DSSE issue #35
/// ("property of the public key") and RFC 8725 §3.1 ("each key MUST be used with exactly one
/// algorithm") (sem: SEM-gx-canon-086). Reader-side tolerance of unknown fields is DSSE's own norm (envelope.md:
/// "Consumers MUST ignore unrecognized fields") and is not under test — this pins **our own
/// writer**, so a silently added `alg` (map header `a3`) turns this RED instead of shipping.
/// The JSON face of the same claim is `gx-core/tests/m2_types.rs`.
#[test]
fn dsse_signature_canonical_cbor_is_a_two_entry_map_and_no_alg() {
    let signature = DsseSignature {
        keyid: "key-1".to_string(),
        sig: vec![0xde, 0xad, 0xbe, 0xef],
    };
    let bytes = cbor::encode(&signature).expect("a signature encodes");
    println!("DSSE_SIGNATURE_CBOR={}", hex(&bytes));
    assert_eq!(
        bytes[0], 0xa2,
        "RFC 8949 §3: a map of exactly two entries opens with 0xa2 — a third field would be 0xa3"
    );
    assert_eq!(
        hex(&bytes),
        "a26373696744deadbeef656b65796964656b65792d31",
        "42 §3.10 / req/38 §109: {{keyid, sig}}, canonical order, and nothing else"
    );
    assert!(
        cbor::is_canonical(&bytes) && cbor::scan_strict(&bytes).is_ok(),
        "the pinned bytes must themselves be canonical"
    );
}

// ---------------------------------------------------------------------------
// v0.4-j — CBOR-face goldens for the signature carriers (req/38 §113 residue) (sem: SEM-gx-canon-093)
// ---------------------------------------------------------------------------
//
// req/172's DsseSignature golden (above) pinned the signature in isolation; the JSON-face
// census (v0.4-e, `gx-core/tests/m2_types.rs` / `gx-witness/tests/receipt_verdict_wire.rs`)
// pinned the four structures that CARRY one, but on the JSON face only. These two tests are
// the DAG-CBOR face of the same carrier claim for the two gx-core carriers. The two
// gx-witness carriers (`DsseEnvelope`/`Receipt`) live in `gx-witness/tests/
// receipt_verdict_wire.rs`, not here: gx-canon cannot name gx-witness (the dependency runs
// the other way — gx-witness names this crate for `cid::compute`), while gx-core's types are
// exactly what this suite already reaches through its normal `gx-core` edge. That is the
// same split req/172 §2-1 made under A-1, extended one crate up.
//
// The fixtures are the very values the JSON census tests hold (`m2_types.rs`,
// `checkpoint_serialises_exactly_the_five_keys...` / `verdict_checkpoint_serialises...`), so
// the two faces of each structure pin one logical value — a divergence between faces would
// have to show up as one of these four tests disagreeing with its census twin about what the
// value even is.
//
// Every hex literal below is written out from RFC 8949 §3's major-type table and 42 §2.1's
// six rules, NOT copied from the encoder — this file's own discipline (see the module doc).
// The derivation was doubled before the tests first ran: an independent second derivation
// (a from-scratch RFC 8949 §3 emitter with its own encoded-key sort, no CBOR library, no
// gx encoder) produced byte-identical strings, and that second derivation also reproduces
// the 22-byte DsseSignature golden req/171 §1-7 measured from the live encoder — so the
// doubling chain is: hand-derivation ⇄ independent derivation ⇄ (via the signature entry)
// the measured golden already double-checked in req/172 §2-2.

/// The 64-byte `0xab` signature both census fixtures carry, as canonical CBOR:
/// `a2` (map, 2 entries); `63 736967` ("sig" — its 4-byte encoded key sorts before "keyid"'s
/// 6-byte one, header included: `63` < `65`); `58 40` (byte string, one-byte length, 64) then
/// sixty-four `ab`; `65 6b65796964` ("keyid"); `65 6b65792d31` ("key-1"). Shared by both
/// carrier goldens below, spelled once.
const AB64_SIGNATURE_CBOR: &str = "\
a2637369675840\
abababababababababababababababab\
abababababababababababababababab\
abababababababababababababababab\
abababababababababababababababab\
656b65796964656b65792d31"; // four lines of sixteen `ab` pairs each = the sixty-four bytes `58 40` announces

/// `Timestamp(1_754_000_000_000_000_000)` — a serde newtype over `i64`, so it encodes as the
/// bare integer: major type 0, 8-byte argument (`1b`), big-endian `0x185775b4f8090000`
/// (RFC 8949 §3 preferred serialization: the shortest length that fits).
const TIMESTAMP_CBOR: &str = "1b185775b4f8090000";

/// 🔴 **v0.4-j — CBOR-face golden, `Checkpoint`** (`req/38` §113 residue "4-structure CBOR golden") (sem: SEM-gx-canon-087) —
/// a signed tree head's canonical form is a **map of exactly five entries** (`a5`), keys in
/// encoded-byte order, its signature the two-entry map req/172 pinned, and no sixth entry.
///
/// # Derivation (RFC 8949 §3 + 42 §2.1, not the encoder)
///
/// Map of five → `a5`. The five encoded keys, compared bytewise **header included** (42 §2.1
/// rule 2), order themselves: `66 6f726967696e` ("origin", 6 bytes, header `66` < every `69`);
/// then the four 9-byte keys under header `69` by first content byte — `726f6f745f68617368`
/// ("root_hash", `72`), `7369676e6174757265` ("signature", `73`), `74696d657374616d70`
/// ("timestamp", `74`), `747265655f73697a65` ("tree_size", also `74`, second byte `72` > `69`).
/// Values: "glovrex-ledger/v1" is 17 UTF-8 bytes → `71` + bytes; `tree_size: 100` → `18 64`
/// (24 ≤ n < 256 takes the one-byte argument); `root_hash` is `Cid`'s 42 §1.1 byte-string face
/// → `58 20` + thirty-two `07`; timestamp and signature are the shared constants above.
///
/// # Why this face matters beside the JSON census
///
/// 33 NFR-011 note 5 forbids a wire-side alg-like field permanently; the v0.4-e census pinned
/// this carrier's JSON key set, arguing "the two faces share one derive, so a field added to
/// the struct moves both" (sem: SEM-gx-canon-088). That argument leaves one theoretical door: a serde attribute
/// (`skip_serializing_if` etc.) can move ONE face only (req/175 §5-2's declared residue). This
/// golden closes that door for the CBOR face: a sixth entry — or a five-entry map whose bytes
/// drifted — goes RED here even if the JSON face was held still. Reader-side tolerance stays
/// out of scope (DSSE envelope.md "Consumers MUST ignore unrecognized fields") (sem: SEM-gx-canon-089); this pins
/// **our own writer**.
#[test]
fn checkpoint_canonical_cbor_is_a_five_entry_map_spec_derived() {
    let checkpoint = Checkpoint {
        origin: "glovrex-ledger/v1".to_string(),
        tree_size: 100,
        root_hash: Cid([0x07; 32]),
        timestamp: Timestamp(1_754_000_000_000_000_000),
        signature: DsseSignature {
            keyid: "key-1".to_string(),
            sig: vec![0xab; 64],
        },
    };
    let expected = format!(
        "a5\
         666f726967696e71676c6f767265782d6c65646765722f7631\
         69726f6f745f686173685820\
         0707070707070707070707070707070707070707070707070707070707070707\
         697369676e6174757265{AB64_SIGNATURE_CBOR}\
         6974696d657374616d70{TIMESTAMP_CBOR}\
         69747265655f73697a651864"
    )
    .replace([' '], "");
    let bytes = cbor::encode(&checkpoint).expect("a checkpoint encodes");
    println!("CHECKPOINT_CBOR={}", hex(&bytes));
    assert_eq!(
        bytes[0], 0xa5,
        "RFC 8949 §3: five entries open with 0xa5 — a sixth field would be 0xa6"
    );
    assert_eq!(
        hex(&bytes),
        expected,
        "42 §3.11 / 33 NFR-011 note 5: five fields, canonical order, and nothing else (sem: SEM-gx-canon-090)"
    );
    assert!(
        cbor::is_canonical(&bytes) && cbor::scan_strict(&bytes).is_ok(),
        "the pinned bytes must themselves be canonical"
    );
    let round: Checkpoint = cbor::decode(&bytes).expect("the golden decodes");
    assert_eq!(
        round, checkpoint,
        "both directions, as the file's pairs have"
    );
}

/// 🔴 **v0.4-j — CBOR-face golden, `VerdictCheckpoint`** (same residue row) (sem: SEM-gx-canon-091) — the signed count's
/// canonical form is a **map of exactly eight entries** (`a8`), its tally exactly four, its
/// signature exactly two — and the all-refusals window (`ledger_root_hash: None`) moves two
/// values (`f6`, `00`) while the entry count stays `a8`.
///
/// # Derivation (RFC 8949 §3 + 42 §2.1, not the encoder)
///
/// Map of eight → `a8`. Encoded keys, bytewise with header: `65 74616c6c79` ("tally", header
/// `65` shortest); `66 6f726967696e` ("origin"); under `69` — `7369676e6174757265`
/// ("signature", `73`) before `74696d657374616d70` ("timestamp", `74`); `6a 77696e646f775f656e64`
/// ("window_end", 10 bytes); `6c 77696e646f775f7374617274` ("window_start", 12); under `70`
/// (16 bytes each, common prefix `6c65646765725f` "ledger_") — `…726f6f745f68617368`
/// ("ledger_root_hash", `72`) before `…747265655f73697a65` ("ledger_tree_size", `74`).
/// The tally is its own four-entry map `a4`, keys `64 64656e79` ("deny") < `65 61646d6974`
/// ("admit") < `68 657363616c617465` ("escalate") < `6b 756e766572646963746564`
/// ("unverdicted") — header length wins before any content byte is read. Small integers
/// (3, 5, 1, 1, 10, 0, 5) are single bytes (`03`…`0a`…); `Some(cid)` encodes as the `Cid`
/// itself (`58 20` + thirty-two `09`), `None` as `f6` — G-5's rule, "None is null (0xf6)" (sem: SEM-gx-canon-092).
///
/// # Why this structure earns the longest derivation in the file
///
/// The census twin's words: the signer is the party the count is evidence *against*, so this
/// wire face is the one a deployment has the most interest in quietly growing. req/180 took
/// the brief's licence to defer the heavy derivations and spent it the other way — this is
/// the structure the licence named, derived in full. The second literal pins the FR-M04
/// window the type exists for: all-refusals, empty ledger, and the shape does not move.
#[test]
fn verdict_checkpoint_canonical_cbor_is_an_eight_entry_map_spec_derived() {
    let tally = VerdictTally {
        deny: 3,
        admit: 5,
        escalate: 1,
        unverdicted: 1,
    };
    let vc = VerdictCheckpoint {
        origin: "glovrex-ledger/v1".to_string(),
        tally,
        window_start: 0,
        window_end: 10,
        ledger_root_hash: Some(Cid([0x09; 32])),
        ledger_tree_size: 5,
        timestamp: Timestamp(1_754_000_000_000_000_000),
        signature: DsseSignature {
            keyid: "key-1".to_string(),
            sig: vec![0xab; 64],
        },
    };
    let head = format!(
        "a8\
         6574616c6c79a46464656e79036561646d69740568657363616c617465016b756e76657264696374656401\
         666f726967696e71676c6f767265782d6c65646765722f7631\
         697369676e6174757265{AB64_SIGNATURE_CBOR}\
         6974696d657374616d70{TIMESTAMP_CBOR}\
         6a77696e646f775f656e640a\
         6c77696e646f775f737461727400"
    );
    let expected = format!(
        "{head}\
         706c65646765725f726f6f745f686173685820\
         0909090909090909090909090909090909090909090909090909090909090909\
         706c65646765725f747265655f73697a6505"
    )
    .replace([' '], "");
    let bytes = cbor::encode(&vc).expect("a verdict checkpoint encodes");
    println!("VERDICT_CHECKPOINT_CBOR={}", hex(&bytes));
    assert_eq!(
        bytes[0], 0xa8,
        "RFC 8949 §3: eight entries open with 0xa8 — a ninth field would be 0xa9"
    );
    assert_eq!(
        hex(&bytes),
        expected,
        "FR-M04 / 33 NFR-011 note 5: eight fields, four buckets, canonical order, nothing else (sem: SEM-gx-canon-093)"
    );
    assert!(
        cbor::is_canonical(&bytes) && cbor::scan_strict(&bytes).is_ok(),
        "the pinned bytes must themselves be canonical"
    );
    let round: VerdictCheckpoint = cbor::decode(&bytes).expect("the golden decodes");
    assert_eq!(round, vc, "both directions, as the file's pairs have");

    // The all-refusals window: `None` → `f6`, size → `00`, and the map header stays `a8`.
    let empty_window = VerdictCheckpoint {
        ledger_root_hash: None,
        ledger_tree_size: 0,
        ..vc
    };
    let expected_empty = format!(
        "{head}\
         706c65646765725f726f6f745f68617368f6\
         706c65646765725f747265655f73697a6500"
    )
    .replace([' '], "");
    let bytes = cbor::encode(&empty_window).expect("the FR-M04 window encodes");
    println!("VERDICT_CHECKPOINT_EMPTY_WINDOW_CBOR={}", hex(&bytes));
    assert_eq!(
        hex(&bytes),
        expected_empty,
        "the interesting window differs in exactly two values and in no key"
    );
    assert!(
        cbor::is_canonical(&bytes) && cbor::scan_strict(&bytes).is_ok(),
        "the empty-window bytes must themselves be canonical"
    );
}
