//! The wire form of a `Tile`, pinned to bytes — the mechanical half of **E-M2-27(b)**.
//!
//! `req/38_ERRATA_2026-08-07.md` §15 逐語: 「(b)gx の `subtree_span(level)=2^level` の「level」は
//! **木の高さ**であり、一次の tile level（1 level=高さ 8 段=256 倍）と 8 倍 scale で別物——
//! **doc 化+field 名 `height` への rename を検討**（wire 不変なら rename 採用）」.
//!
//! # What this file decides
//!
//! The ruling makes the rename conditional on the wire being unchanged, so somebody has to be able
//! to say what the wire *is*. A struct's field names are its DAG-CBOR map keys: renaming
//! `Tile.level` to `Tile.height` produces a different byte string for the same tile, a different
//! `Cid` for anything containing one, and a reader of a published tile that no longer finds the key
//! it is looking for. So the condition fails for the field, the rename is taken only inside the
//! private `subtree_span`, and this file is the evidence rather than an assurance.
//!
//! # The vectors are hand-written, not captured
//!
//! Every expected byte string below was written out from 42 §2.1's rules — canonical DAG-CBOR, map
//! keys sorted by their **encoded** form (length first, then bytewise: `index`, `level`, `width`,
//! then `hashes`), integers in shortest form, a `Cid` as a 32-byte byte string (42 §1.1: 「32byte
//! byte-string（major type 2）として直接格納」, so `0x58 0x20` and the digest). None was produced by
//! running the encoder. `pae_golden.rs` gives the reason at length: a golden captured from the code
//! it tests records what the code does, and can never disagree with it.
//!
//! # What this file does not say
//!
//! Nothing about interoperability. A C2SP `tlog-tiles` reader does not parse DAG-CBOR at all, and
//! `ac_024.rs` is where that limit is stated. This is about gx's own bytes staying the bytes gx
//! published.

use gx_canon::cbor;
use gx_core::{Cid, Timestamp, TransformationId};
use gx_log::tile::{Tile, TileLog};

/// The map key `level`, as it appears in a canonical encoding: a 5-byte text string.
const LEVEL_KEY: &[u8] = &[0x65, b'l', b'e', b'v', b'e', b'l'];

/// The key it was proposed to become. Six bytes, so even its header differs.
const HEIGHT_KEY: &[u8] = &[0x66, b'h', b'e', b'i', b'g', b'h', b't'];

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2), "hex string of odd length");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `Tile { hashes: [0x11; 32], index: 0, level: 0, width: 1 }`.
///
/// `a4` opens a four-entry map. Then `65 696e646578` (`index`) `00`, `65 6c6576656c` (`level`) `00`,
/// `65 7769647468` (`width`) `01`, and `66 686173686573` (`hashes`) `81` — a one-element array —
/// `5820` and thirty-two `11` bytes.
/// The layout is the point, so the pairing of key and value is kept on one line each
/// (`rustfmt` would put every fragment on a line of its own and the reader would lose it).
#[rustfmt::skip]
const ONE_LEAF_TILE: &str = concat!(
    "a4",
    "65696e646578", "00",
    "656c6576656c", "00",
    "657769647468", "01",
    "66686173686573", "81",
    "5820", "1111111111111111111111111111111111111111111111111111111111111111",
);

/// The same shape with every number non-zero, so the vector above is not passing on zeros.
///
/// `index` 1, `level` 3, `width` 2, two digests.
#[rustfmt::skip]
const TWO_NODE_TILE: &str = concat!(
    "a4",
    "65696e646578", "01",
    "656c6576656c", "03",
    "657769647468", "02",
    "66686173686573", "82",
    "5820", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "5820", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
);

fn tile_of(hashes: Vec<Cid>, index: u64, level: u8) -> Tile {
    let width = u16::try_from(hashes.len()).expect("small");
    Tile {
        hashes,
        index,
        level,
        width,
    }
}

// ---------------------------------------------------------------------------
// The golden vectors
// ---------------------------------------------------------------------------

/// A tile encodes to the declared bytes, key names included.
#[test]
fn a_tile_encodes_to_the_declared_bytes() {
    let cases = [
        (
            "one leaf",
            tile_of(vec![Cid([0x11; 32])], 0, 0),
            ONE_LEAF_TILE,
        ),
        (
            "two nodes at height 3",
            tile_of(vec![Cid([0xaa; 32]), Cid([0xbb; 32])], 1, 3),
            TWO_NODE_TILE,
        ),
    ];
    for (name, tile, expected) in cases {
        let bytes = cbor::encode(&tile).expect("a tile has a canonical form");
        assert_eq!(
            hex(&bytes),
            expected,
            "{name}: the wire form of a Tile changed"
        );
        assert_eq!(
            cbor::decode::<Tile>(&unhex(expected)).expect("the golden decodes"),
            tile,
            "{name}: the golden bytes do not decode back to the tile they were written for"
        );
    }
    println!("TILE_WIRE_VECTORS=2 TILE_WIRE_KEY_LEVEL=present");
}

/// The wire says `level`, and it does not say `height` (**E-M2-27(b)**).
///
/// The reason the field was left alone. `subtree_span`'s argument is renamed because nothing outside
/// the file can see it; this key is 42 §3.11's name and is inside the bytes, so renaming it is a
/// format change and not a spelling change.
#[test]
fn the_map_key_is_level_and_renaming_it_would_change_the_wire() {
    let tile = tile_of(vec![Cid([0x11; 32])], 0, 0);
    let bytes = cbor::encode(&tile).expect("canonical");

    assert!(
        bytes.windows(LEVEL_KEY.len()).any(|w| w == LEVEL_KEY),
        "the encoded tile does not carry the map key `level` (42 §3.11)"
    );
    assert!(
        !bytes.windows(HEIGHT_KEY.len()).any(|w| w == HEIGHT_KEY),
        "the encoded tile carries `height`; 42 §3.11 names the field `level` and the name is on \
         the wire"
    );

    // And the two keys are not the same length, so the change would move every byte after it.
    assert_ne!(LEVEL_KEY.len(), HEIGHT_KEY.len());
    println!(
        "TILE_WIRE_RENAME_IS_A_FORMAT_CHANGE=yes level_key_bytes={} height_key_bytes={}",
        LEVEL_KEY.len(),
        HEIGHT_KEY.len()
    );
}

/// A tile the log really produced encodes the same way — the goldens are not a private dialect.
#[test]
fn a_tile_from_a_log_encodes_under_the_same_keys() {
    let mut log = TileLog::new();
    for i in 0..8u64 {
        let mut raw = [0u8; 32];
        raw[..8].copy_from_slice(&i.to_be_bytes());
        log.append(
            TransformationId(Cid(raw)),
            Cid([u8::try_from(i).expect("small"); 32]),
            Timestamp(i as i64),
        )
        .expect("canonical");
    }

    let leaves = log.tile(0, 0).expect("the leaf tile");
    let bytes = cbor::encode(&leaves).expect("canonical");
    assert_eq!(bytes[0], 0xa4, "a Tile is a four-entry map");
    assert!(bytes.windows(LEVEL_KEY.len()).any(|w| w == LEVEL_KEY));
    assert_eq!(
        cbor::decode::<Tile>(&bytes).expect("round trip"),
        leaves,
        "a tile from a log does not survive its own encoding"
    );

    // Level 3 exists because eight leaves make one complete subtree of 2^3 -- gx's level is a
    // height, and this is the arithmetic that makes it one (E-M2-27(b)).
    let top = log.tile(3, 0).expect("one complete octet");
    assert_eq!(top.level, 3);
    assert_eq!(top.width, 1);
    println!("TILE_FROM_LOG_LEAVES=8 TILE_FROM_LOG_TOP_LEVEL=3 (gx level = tree height)");
}
