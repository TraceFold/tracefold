//! The bytes a checkpoint signature covers (**E-M2-19**, `req/38_ERRATA_2026-08-07.md` §9).
//!
//! No acceptance criterion is claimed here. 34 gives the checkpoint none -- AC-021..024 are the
//! log's four and none of them mention a signed tree head -- so this file exists because a ruling
//! landed, not because an AC asks.
//!
//! # The ruling
//!
//! E-M2-19 逐語: 「`Checkpoint` の**署名 core={origin, tree_size, root_hash}**とし `timestamp` は
//! 署名 core の外（unsigned advisory field）へ——E-M2-6 と完全同型（CM-5: clock-free signed
//! payload）。根拠は内部一貫性が一次（同じ原則が receipt にだけ効いて checkpoint に効かない状態は
//! 説明不能）。…field 自体は 42 §3.11 どおり残す（lane の据え置きは正しかった）。手 2/手 4 で実装」.
//!
//! Two things follow, and both are checked below. The `Checkpoint` struct keeps all five fields
//! 42 §3.11 gives it -- hand 1 was right not to remove `timestamp`, and this hand does not remove
//! it either. What changes is that there is now one function that says which of them a signature
//! covers, so 「the clock is not signed」 is a property of a byte string rather than a sentence in
//! a document.
//!
//! # What is not here
//!
//! Signing. Producing a `DsseSignature` needs a PAE encoding and an Ed25519 key, both of which are
//! `gx-witness::dsse` (hand 5, AC-018..020). This hand stops at the bytes: 「署名生成自体が手 2
//! scope 外なら『署名対象 byte 列を作る関数』までを置き doc に E-M2-19 anchor を書く」.
//! Consequently nothing in this file verifies a signature, and a caller that signs these bytes has
//! done nothing this crate can check.
//!
//! # Hand 3 adds the head itself (H2-4)
//!
//! req/38 §11 逐語: 「H2-4（Checkpoint 生成関数の不在）→ 手 3: store が木の状態を持って初めて
//! 作れる。unsigned 生成（署名 core byte 列は手 2 の checkpoint_core が既設）を手 3・
//! DsseSignature 装着は手 5」. So the second half of this file is
//! [`gx_log::proof::unsigned_checkpoint`]: the head a log can state about itself, carrying a
//! signature nothing can verify because none has been made.

use gx_canon::cbor;
use gx_core::{Checkpoint, Cid, DsseSignature, Timestamp};
use gx_log::proof::{checkpoint_signing_bytes, unsigned_checkpoint};
use gx_log::tile::TileLog;
use gx_log::Error;

fn cid(seed: u8) -> Cid {
    Cid([seed; 32])
}

fn log_of(n: u64) -> TileLog {
    let mut log = TileLog::new();
    for i in 0..n {
        let mut raw = [0u8; 32];
        raw[..8].copy_from_slice(&i.to_be_bytes());
        log.append(
            gx_core::TransformationId(Cid(raw)),
            cid(u8::try_from(i % 251).expect("in range")),
            Timestamp(i as i64),
        )
        .expect("canonical");
    }
    log
}

fn checkpoint() -> Checkpoint {
    Checkpoint {
        origin: "glovrex-ledger/v1".to_string(),
        tree_size: 100,
        root_hash: cid(7),
        timestamp: Timestamp(1_700_000_000_000_000_000),
        signature: DsseSignature {
            keyid: "ledger-key-1".to_string(),
            sig: vec![0xAB; 64],
        },
    }
}

/// The clock is not in the signed bytes (CM-5, E-M2-19).
///
/// Two checkpoints of the same tree, minted a second apart, sign the same bytes. Without this the
/// signature would bind to when the head was *written down*, and re-issuing a checkpoint for an
/// unchanged tree would produce a second signature over a different message -- which is the
/// property E-M2-6 removed from the receipt for the same reason.
#[test]
fn the_signed_bytes_do_not_move_when_the_clock_does() {
    let a = checkpoint();
    let mut b = checkpoint();
    b.timestamp = Timestamp(1_700_000_001_000_000_000);

    assert_eq!(
        checkpoint_signing_bytes(&a).expect("bytes"),
        checkpoint_signing_bytes(&b).expect("bytes"),
    );
}

/// Nor is the signature itself, which would be circular.
#[test]
fn the_signed_bytes_do_not_contain_the_signature() {
    let a = checkpoint();
    let mut b = checkpoint();
    b.signature = DsseSignature {
        keyid: "ledger-key-2".to_string(),
        sig: vec![0xCD; 64],
    };

    assert_eq!(
        checkpoint_signing_bytes(&a).expect("bytes"),
        checkpoint_signing_bytes(&b).expect("bytes"),
    );
}

/// Each of the three fields of the core is covered: change one, and the bytes change.
///
/// The negative half of the two tests above. A function that returned a constant would satisfy
/// them both.
#[test]
fn every_field_of_the_core_is_covered() {
    let base = checkpoint_signing_bytes(&checkpoint()).expect("bytes");

    let mut other_origin = checkpoint();
    other_origin.origin = "glovrex-ledger/v2".to_string();
    assert_ne!(
        checkpoint_signing_bytes(&other_origin).expect("bytes"),
        base,
        "the origin is what stops one log's checkpoint verifying against another's (42 §3.11)"
    );

    let mut other_size = checkpoint();
    other_size.tree_size = 101;
    assert_ne!(checkpoint_signing_bytes(&other_size).expect("bytes"), base);

    let mut other_root = checkpoint();
    other_root.root_hash = cid(8);
    assert_ne!(checkpoint_signing_bytes(&other_root).expect("bytes"), base);
}

/// The bytes are canonical DAG-CBOR, produced through gx-canon like everything else (41 §6).
///
/// A signature over bytes that have a second spelling is a signature that can be re-encoded into a
/// different message for the same value, which is the whole reason 42 §2.1 exists.
#[test]
fn the_signed_bytes_are_canonical() {
    let bytes = checkpoint_signing_bytes(&checkpoint()).expect("bytes");
    assert!(cbor::is_canonical(&bytes));
}

/// Determinism: the same head signs the same bytes, every time.
#[test]
fn the_signed_bytes_are_deterministic() {
    assert_eq!(
        checkpoint_signing_bytes(&checkpoint()).expect("bytes"),
        checkpoint_signing_bytes(&checkpoint()).expect("bytes"),
    );
}

// ---------------------------------------------------------------------------
// A-10: the arithmetic 5 = 3 + 2, held mechanically
// ---------------------------------------------------------------------------

/// The keys of a canonical DAG-CBOR map, and the count its head byte declares.
///
/// Two readings of the same bytes on purpose. The head byte is the encoder's own statement of how
/// many pairs follow -- canonical form puts a count under 24 in the low five bits of a major-type-5
/// byte (42 §2.1, RFC 8949 §3) -- and the decode is what names them. A test that only counted
/// would pass on a struct that swapped one field for another.
fn map_keys(bytes: &[u8]) -> (u8, Vec<String>) {
    let head = bytes[0];
    assert_eq!(
        head & 0b1110_0000,
        0b1010_0000,
        "the encoding of a struct is a CBOR map (major type 5); head byte was {head:#04x}"
    );
    let declared = head & 0b0001_1111;
    assert!(
        declared < 24,
        "a map of {declared} or more pairs spells its count in following bytes; this helper reads \
         the short form only, which is all any type in this workspace needs"
    );

    let decoded: std::collections::BTreeMap<String, serde::de::IgnoredAny> =
        cbor::decode(bytes).expect("a canonical map of text keys");
    (declared, decoded.into_keys().collect())
}

/// `Checkpoint` encodes exactly five map keys: the three the signature covers plus two it does not
/// (**A-10**).
///
/// A-10 逐語 (`req/38_ERRATA_2026-08-07.md` §18, adopted as a required DoD of M3's first hand):
/// 「`Checkpoint` の encode map key 数=5(=3 covered+2 declared-out)を assert する 1 本。field 追加が
/// 黙って署名対象から外れる mirror-struct 構造の機械 guard」.
///
/// # What was unguarded until this test
///
/// `checkpoint_signing_bytes` builds a private `CheckpointCore` of three fields -- a *mirror* of
/// `Checkpoint`, not a projection of it. A sixth field added to `Checkpoint` compiles, changes no
/// signature, and leaves every test above green: they all ask what the core covers, and none asks
/// what the whole is. So `every_field_of_the_core_is_covered` proves the three; this proves the
/// two, and the two are the ones a reader has to be told about. E-M2-19 declared the uncovered set
/// to be exactly `{timestamp, signature}`, and a declaration nothing counts is req/08 N-1's shape.
///
/// The set difference is asserted rather than the arithmetic alone: `5 == 3 + 2` also holds if a
/// new field displaced `timestamp` out of the struct.
#[test]
fn the_checkpoint_encodes_five_keys_three_covered_and_two_declared_out() {
    let head = checkpoint();
    let (declared, whole) = map_keys(&cbor::encode(&head).expect("canonical"));
    let (core_declared, core) = map_keys(&checkpoint_signing_bytes(&head).expect("bytes"));

    assert_eq!(
        declared, 5,
        "42 §3.11 gives `Checkpoint` five fields; the encoding declares {declared}. A sixth field \
         is invisible to every other test in this file (E-M2-19, A-10)"
    );
    assert_eq!(
        whole,
        ["origin", "root_hash", "signature", "timestamp", "tree_size"],
        "the five keys are not the five 42 §3.11 names"
    );

    assert_eq!(
        core_declared, 3,
        "the signed core is E-M2-19's three fields"
    );
    assert_eq!(core, ["origin", "root_hash", "tree_size"]);

    let out_of_core: Vec<&String> = whole.iter().filter(|k| !core.contains(k)).collect();
    assert_eq!(
        out_of_core,
        ["signature", "timestamp"],
        "E-M2-19 declares exactly `timestamp` (CM-5, the clock) and `signature` (circularity) to \
         be outside the signed core; the encoding leaves {out_of_core:?} outside"
    );
    assert_eq!(
        usize::from(declared),
        core.len() + out_of_core.len(),
        "5 = 3 covered + 2 declared-out (A-10)"
    );
}

// ---------------------------------------------------------------------------
// H2-4: the head a log states about itself, before anybody signs it
// ---------------------------------------------------------------------------

/// The three signed fields come from the tree, and the two unsigned ones from the caller.
#[test]
fn an_unsigned_checkpoint_states_the_tree_it_was_taken_from() {
    let log = log_of(37);
    let head = unsigned_checkpoint(&log, "glovrex-ledger/v1", Timestamp(42)).expect("head");

    assert_eq!(head.tree_size, 37);
    assert_eq!(head.root_hash, log.root().expect("root"));
    assert_eq!(head.origin, "glovrex-ledger/v1");
    assert_eq!(head.timestamp, Timestamp(42));
}

/// It is unsigned, and unmistakably so.
///
/// An empty `sig` is not a signature anybody could mistake for one: Ed25519 makes 64 bytes, and
/// AC-019 asks a verifier to reject a malformed envelope. Leaving the field empty is the honest
/// spelling of 「not signed yet」 -- the alternative, making `Checkpoint.signature` an `Option`,
/// would be a change to 42 §3.11's field table, which this hand does not have the standing to make.
#[test]
fn an_unsigned_checkpoint_carries_no_signature() {
    let head = unsigned_checkpoint(&log_of(4), "glovrex-ledger/v1", Timestamp(0)).expect("head");
    assert!(head.signature.sig.is_empty());
    assert!(head.signature.keyid.is_empty());
}

/// The head's signed core is the tree's, so hand 5 can sign what this hand produced.
#[test]
fn the_head_signs_the_bytes_the_core_function_produces() {
    let log = log_of(9);
    let head = unsigned_checkpoint(&log, "glovrex-ledger/v1", Timestamp(5)).expect("head");
    let mut later = unsigned_checkpoint(&log, "glovrex-ledger/v1", Timestamp(6)).expect("head");
    later.signature = DsseSignature {
        keyid: "ledger-key-1".to_string(),
        sig: vec![0xAB; 64],
    };

    let bytes = checkpoint_signing_bytes(&head).expect("bytes");
    assert!(cbor::is_canonical(&bytes));
    assert_eq!(
        bytes,
        checkpoint_signing_bytes(&later).expect("bytes"),
        "a later clock and an attached signature do not move the signed core (CM-5, E-M2-19)"
    );
}

/// A head of a longer log is a different head.
#[test]
fn a_head_of_a_grown_tree_signs_different_bytes() {
    let short = unsigned_checkpoint(&log_of(9), "glovrex-ledger/v1", Timestamp(0)).expect("head");
    let long = unsigned_checkpoint(&log_of(10), "glovrex-ledger/v1", Timestamp(0)).expect("head");
    assert_ne!(
        checkpoint_signing_bytes(&short).expect("bytes"),
        checkpoint_signing_bytes(&long).expect("bytes"),
    );
}

/// An empty log has no head, and says so rather than publishing one.
///
/// `TileLog::root` answers `None` for the empty tree (req/51 §3.5: a value nothing consumes gets
/// no spelling), so there is nothing to put in `root_hash` and no head to sign.
#[test]
fn an_empty_log_has_no_head_to_publish() {
    let empty = TileLog::new();
    assert!(matches!(
        unsigned_checkpoint(&empty, "glovrex-ledger/v1", Timestamp(0)),
        Err(Error::Malformed { .. })
    ));
}
