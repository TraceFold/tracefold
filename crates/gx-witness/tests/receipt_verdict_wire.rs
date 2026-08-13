//! 🔴 **E-M5-11**: `ReceiptPayload.verdict` becomes an `Option`, and the wire does not move.
//!
//! Spec: 42 §3.10 for the payload table, 43 T-4e for the admission that has no verdict, ASM-14 for
//! the two kinds. The ruling is `req/38_ERRATA_2026-08-07.md` §41, M5H4-3 採(a):
//!
//! > **M5H4-3 採(a)**=**E-M5-11・手 6 発射条件**: `ReceiptPayload.verdict` を **`Option`** にする
//! > (42 §3.10 erratum)。T-4e は gate を呼ばず verdict が存在しない——**空 digest の鋳造禁
//! > (M4H4-2)を wire まで貫く**形で、journal 側 E-M5-7(`verdict_digest=Option`)と対称。実装窓=
//! > **手 6**(AC-037 がこの経路を正面から踏む)・**golden/pae vector の更新を same-turn**
//!
//! # What 「golden/pae vector の更新」 turned out to mean, measured rather than assumed
//!
//! The instruction anticipates a wire break. **There is none for any receipt that has a verdict**,
//! and this file is where that stops being an assumption. serde writes `Some(x)` as `x` — the
//! encoder's `serialize_some` forwards to the value — so a payload carrying a verdict encodes to
//! the bytes it encoded to before the type changed, and `None` writes the `0xf6` that 42 §2.1's
//! own golden vector G-5 pins for an absent value (「None は null(0xf6)」).
//!
//! So the vectors are *checked* rather than regenerated, and the check is the stronger statement:
//! the two byte strings below were taken from the tree at `53e8285` — **before** E-M5-11 — and are
//! written here as literals. `tools/verify_m5h6.sh` §2 prints them again from this suite. A
//! regenerated golden records what the code does; a golden carried across the change records what
//! the change did **not** do, which is the claim the ruling asks for.
//!
//! # Why there is no schema rule pairing `verdict: None` with `fail_posture_engaged`
//!
//! Because the ruling did not make one, and inventing one here would be this crate ruling on 42
//! §3.10 rather than implementing §41. The pairing is real — a receipt with no verdict and no
//! posture flag says a commit happened for no stated reason — and the **engine** refuses to build
//! one (`gx-engine`'s `Error::Unrepresentable`, which E-M5-11 would otherwise have left without a
//! producer). Whether `check_schema` should carry the same rule is raised in the hand's report as
//! a ticket, not decided here.

mod support;

use gx_canon::cbor;
use gx_core::VerdictKind;
use support::{degraded_payload, issue, keypair, verdict_payload};

/// The canonical DAG-CBOR of `verdict_payload(Admit, keypair(1), 5)`, taken at `53e8285`.
///
/// 358 bytes. The eleven keys of 42 §3.10 as E-M2-6 and E-M2-7 corrected them, in encoded-key
/// order, with `verdict` carrying a two-key map.
const VERDICT_PAYLOAD_HEX: &str = "\
ab666b65795f6964656b65792d316776657264696374a2646b696e646541646d69746c70726f6f665f64696765737458\
20000000000007a12500000000000000000000000000000000000000000000000068656e666f72636564f46c72656365\
6970745f6b696e646e56657264696374526563656970746d63616e6f6e6963616c5f6369645820000000000089544500\
00000000000000000000000000000000000000000000006d696e76657273655f64656c7461f66e7472616e73666f726d\
6174696f6e582000000000008954450000000000000000000000000000000000000000000000006f696e636c7573696f\
6e5f70726f6f66f6746661696c5f706f73747572655f656e6761676564f57818707265636f6e646974696f6e5f66696e\
6765727072696e74582007070707070707070707070707070707070707070707070707070707070707077819706f7374\
636f6e646974696f6e5f66696e6765727072696e74f6";

/// `ReceiptPayload::ledger_digest` of the same value, taken at `53e8285`.
///
/// Pinned separately from the bytes because it is what the **ledger** committed to: if this moved,
/// every leaf gx-log holds for a receipt issued before this hand would stop matching the receipt
/// (43 ASM-43-1's key idempotency would answer `Error::Conflict` on a re-append), which is a
/// migration and not a type change.
const VERDICT_PAYLOAD_LEDGER_DIGEST: &str =
    "gx1:4mfdpy5pmsgpvml6wl3eeexeixiqyp2cdjm36vmyvygfmpcbfv6q";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// The type
// ---------------------------------------------------------------------------

/// 🔴 The field is an `Option` (**E-M5-11**), read off this crate's own source.
///
/// Structural because that is what the erratum is: 42 §3.10's table types the field as a
/// `VerdictSummary` and §41 rules it optional. A behavioural probe can show that `None` round
/// trips; only a scan can show that the **declaration** is the one the ruling names.
#[test]
fn e_m5_11_the_verdict_field_is_optional() {
    let src = include_str!("../src/receipt.rs");
    let body = src
        .split("pub struct ReceiptPayload {")
        .nth(1)
        .expect("receipt.rs declares ReceiptPayload")
        .split("\n}")
        .next()
        .expect("split always yields one");
    let field = body
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("pub verdict"))
        .expect("the payload has a verdict field");
    println!("RECEIPT_VERDICT_FIELD={field:?}");
    assert_eq!(
        field, "pub verdict: Option<VerdictSummary>,",
        "E-M5-11: 43 T-4e admits with no verdict, and an empty digest may not be minted to fill it"
    );

    // The count 42 §3.10 is read as having does not move: the field changed type, not existence.
    let fields = body
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("pub ") && l.ends_with(','))
        .count();
    println!("RECEIPT_PAYLOAD_FIELDS={fields}");
    assert_eq!(fields, 11, "E-M5-11 retypes a field; it adds none");
}

// ---------------------------------------------------------------------------
// The wire
// ---------------------------------------------------------------------------

/// 🔴 A payload **with** a verdict encodes to the bytes it encoded to before E-M5-11.
///
/// Green before the change and green after — deliberately. This is the whole content of 「wire diff
/// が Option 化のみ」: `Some(v)` and `v` have one encoding, so no receipt that anybody has ever
/// issued changes shape.
#[test]
fn a_payload_that_has_a_verdict_encodes_exactly_as_it_did_before_e_m5_11() {
    let key = keypair(1);
    let payload = verdict_payload(VerdictKind::Admit, &key, 5);
    let bytes = cbor::encode(&payload).expect("the fixture is canonical");
    println!("VERDICT_PAYLOAD_LEN={}", bytes.len());
    println!("VERDICT_PAYLOAD_HEX={}", hex(&bytes));
    assert_eq!(
        hex(&bytes),
        VERDICT_PAYLOAD_HEX.replace(['\n', ' '], ""),
        "E-M5-11 moved the wire form of a receipt that carries a verdict"
    );
}

/// 🔴 And the digest the ledger committed to is the same digest.
#[test]
fn the_ledger_digest_of_that_payload_is_the_one_the_ledger_already_holds() {
    let key = keypair(1);
    let payload = verdict_payload(VerdictKind::Admit, &key, 5);
    let digest = payload.ledger_digest().expect("the fixture digests");
    let text = gx_canon::cid::to_text(&digest);
    println!("VERDICT_PAYLOAD_LEDGER_DIGEST={text}");
    assert_eq!(
        text, VERDICT_PAYLOAD_LEDGER_DIGEST,
        "43 ASM-43-1 keys the ledger on this digest; moving it is a migration"
    );
}

/// 🔴 The **whole** of the wire difference: one `a2…` map becomes one `f6`.
///
/// The two encodings are compared as strings and the difference is located rather than described.
/// `null` is 42 §2.1's spelling for an absent value and G-5 (`crates/gx-canon/tests/vectors/golden/
/// G-5.json`) is the golden that fixes it — 「None は null(0xf6)、空 Vec は 0 要素 array(0x80)」 — so
/// the degraded receipt is not carrying a new spelling of anything.
#[test]
fn the_only_wire_difference_is_the_null_that_stands_where_the_verdict_was() {
    let key = keypair(1);
    let with = cbor::encode(&verdict_payload(VerdictKind::Admit, &key, 5)).expect("canonical");
    let without = cbor::encode(&degraded_payload(&key, 5)).expect("canonical");
    let with = hex(&with);
    let without = hex(&without);

    // The common prefix ends where the verdict's value begins, and the common suffix begins right
    // after it. Everything between is what changed.
    let prefix = with
        .char_indices()
        .zip(without.chars())
        .take_while(|((_, a), b)| a == b)
        .count();
    let suffix = with
        .chars()
        .rev()
        .zip(without.chars().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let changed_with = &with[prefix..with.len() - suffix];
    let changed_without = &without[prefix..without.len() - suffix];
    println!("WIRE_DIFF_WITH={changed_with}");
    println!("WIRE_DIFF_WITHOUT={changed_without}");
    println!(
        "WIRE_DIFF_PREFIX_NIBBLES={prefix}  SUFFIX_NIBBLES={suffix}  \
         LEN_WITH={}  LEN_WITHOUT={}",
        with.len() / 2,
        without.len() / 2
    );

    assert_eq!(
        changed_without, "f6",
        "the absent verdict must be exactly 42 §2.1's null and nothing else"
    );
    assert!(
        with.starts_with(&without[..prefix]),
        "the prefix is shared by construction"
    );
    assert_eq!(
        changed_with,
        "a2646b696e646541646d69746c70726f6f665f64696765737458\
20000000000007a125000000000000000000000000000000000000000000000000",
        "the removed bytes are the two-key verdict map and nothing beside it"
    );

    // And the map key itself is untouched: 「verdict」 is still there, still in the same place.
    let key_bytes = "6776657264696374"; // text(7) "verdict"
    assert!(with.contains(key_bytes) && without.contains(key_bytes));
}

/// A degraded receipt is a legal `VerdictReceipt` and verifies offline (ASM-14 unchanged).
///
/// The point of the seat being optional is that the receipt can be **issued**, not merely typed:
/// `Receipt::issue` runs `check_schema` before signing, so a shape ASM-14 refuses never reaches a
/// signature. This is the probe that says E-M5-11 did not quietly make an unissuable value.
#[test]
fn a_receipt_with_no_verdict_is_still_a_receipt_a_stranger_can_check() {
    let key = keypair(2);
    let payload = degraded_payload(&key, 11);
    let receipt = issue(&payload, &key);
    let checks = gx_witness::receipt::verify_offline(&receipt, &key.verifying(), None)
        .expect("a degraded verdict receipt verifies");
    println!(
        "DEGRADED_RECEIPT verdict={:?} enforced={} fpe={} checks={checks:?}",
        payload.verdict, payload.enforced, payload.fail_posture_engaged
    );
    assert!(payload.verdict.is_none());
    assert!(!payload.enforced && payload.fail_posture_engaged, "43 T-4e");
    assert!(checks.verified(), "{checks:?}");
    assert_eq!(
        receipt.payload().expect("decodes").verdict,
        None,
        "the absence survives the envelope"
    );
}
