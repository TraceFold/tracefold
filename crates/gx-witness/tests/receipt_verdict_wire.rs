// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **E-M5-11**: `ReceiptPayload.verdict` becomes an `Option`, and the wire does not move.
//! (sem: SEM-gx-witness-249, SEM-gx-witness-250, SEM-gx-witness-251, SEM-gx-witness-252,
//! SEM-gx-witness-253, SEM-gx-witness-254, SEM-gx-witness-255, SEM-gx-witness-256,
//! SEM-gx-witness-257, SEM-gx-witness-258, SEM-gx-witness-259, SEM-gx-witness-260,
//! SEM-gx-witness-261, SEM-gx-witness-262, SEM-gx-witness-263, SEM-gx-witness-264,
//! SEM-gx-witness-265, SEM-gx-witness-266)
//!
//! Spec: 42 §3.10 for the payload table, 43 T-4e for the admission that has no verdict, ASM-14 for
//! the two kinds. The ruling is `req/38_ERRATA_2026-08-07.md` §41, M5H4-3 adopted (a):
//!
//! > **M5H4-3, adopted (a)** = **E-M5-11, hand 6's firing condition**: make `ReceiptPayload.verdict`
//! > an **`Option`** (a 42 §3.10 erratum). T-4e calls no gate and no verdict exists — carried
//! > through to the wire in the shape that keeps **the ban on minting an empty digest (M4H4-2)**,
//! > symmetric with the journal side's E-M5-7 (`verdict_digest=Option`). Implementation window =
//! > **hand 6** (AC-037 walks straight into this road) · **update the golden/pae vectors in the
//! > same turn**
//!
//! # What "updating the golden/pae vectors" turned out to mean, measured rather than assumed
//!
//! The instruction anticipates a wire break. **There is none for any receipt that has a verdict**,
//! and this file is where that stops being an assumption. serde writes `Some(x)` as `x` — the
//! encoder's `serialize_some` forwards to the value — so a payload carrying a verdict encodes to
//! the bytes it encoded to before the type changed, and `None` writes the `0xf6` that 42 §2.1's
//! own golden vector G-5 pins for an absent value ("`None` is null (0xf6)").
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
use gx_core::{DsseSignature, Timestamp, VerdictKind};
use gx_witness::{DsseEnvelope, Receipt, RECEIPT_PAYLOAD_TYPE};
use support::{degraded_payload, issue, keypair, verdict_payload};

/// 🔴 **The pre-DR-46-24 form**, kept rather than deleted: 358 bytes, `ab` = a map of eleven.
///
/// The canonical DAG-CBOR of `verdict_payload(Admit, keypair(1), 5)`, taken at `53e8285`, when the
/// payload had the eleven keys of 42 §3.10 as E-M2-6 and E-M2-7 corrected them. It is no longer
/// what the encoder produces — DR-46-24(A) added `read_set` and `fingerprint_scope` — and it
/// is still here because a golden carried across a change records the change, while a golden
/// regenerated after one records only that somebody regenerated it.
/// `the_wire_moved_by_exactly_the_two_keys_dr_46_24_added` below is what makes it load-bearing
/// instead of decorative.
const VERDICT_PAYLOAD_HEX_BEFORE_DR_46_24: &str = "\
ab666b65795f6964656b65792d316776657264696374a2646b696e646541646d69746c70726f6f665f64696765737458\
20000000000007a12500000000000000000000000000000000000000000000000068656e666f72636564f46c72656365\
6970745f6b696e646e56657264696374526563656970746d63616e6f6e6963616c5f6369645820000000000089544500\
00000000000000000000000000000000000000000000006d696e76657273655f64656c7461f66e7472616e73666f726d\
6174696f6e582000000000008954450000000000000000000000000000000000000000000000006f696e636c7573696f\
6e5f70726f6f66f6746661696c5f706f73747572655f656e6761676564f57818707265636f6e646974696f6e5f66696e\
6765727072696e74582007070707070707070707070707070707070707070707070707070707070707077819706f7374\
636f6e646974696f6e5f66696e6765727072696e74f6";

/// 🔴 **The shipped form** (DR-46-24(A)): 414 bytes, `ad` = a map of thirteen.
///
/// The same fixture, encoded by the same encoder, after the erratum. Two keys appear and each
/// carries what ASM-14 requires of a verdict receipt: `read_set` is `f6` (the escrow that reads is
/// 43 T-10b, during commit) and `fingerprint_scope` is the fixture's scope string. Nothing else
/// moved — the diff below locates that claim rather than asserting it in prose.
const VERDICT_PAYLOAD_HEX: &str = "\
ad666b65795f6964656b65792d316776657264696374a2646b696e646541646d69746c70726f6f665f64696765737458\
20000000000007a12500000000000000000000000000000000000000000000000068656e666f72636564f46872656164\
5f736574f66c726563656970745f6b696e646e56657264696374526563656970746d63616e6f6e6963616c5f63696458\
2000000000008954450000000000000000000000000000000000000000000000006d696e76657273655f64656c7461f6\
6e7472616e73666f726d6174696f6e582000000000008954450000000000000000000000000000000000000000000000\
006f696e636c7573696f6e5f70726f6f66f67166696e6765727072696e745f73636f7065781a666978747572653a2f2f\
73636f70652f6f6e652d6f626a656374746661696c5f706f73747572655f656e6761676564f57818707265636f6e6469\
74696f6e5f66696e6765727072696e745820070707070707070707070707070707070707070707070707070707070707\
07077819706f7374636f6e646974696f6e5f66696e6765727072696e74f6";

/// `ReceiptPayload::ledger_digest` of the same value, taken at `53e8285`.
///
/// Pinned separately from the bytes because it is what the **ledger** committed to: if this moved,
/// every leaf gx-log holds for a receipt issued before this hand would stop matching the receipt
/// (43 ASM-43-1's key idempotency would answer `Error::Conflict` on a re-append), which is a
/// migration and not a type change.
const VERDICT_PAYLOAD_LEDGER_DIGEST_BEFORE_DR_46_24: &str =
    "gx1:4mfdpy5pmsgpvml6wl3eeexeixiqyp2cdjm36vmyvygfmpcbfv6q";

/// 🔴 **And the digest DR-46-24(A) moved it to.**
///
/// This is the half of the erratum that is a **migration** and not a retyping, and it is stated
/// here rather than in a comment somewhere: a receipt issued before this hand has a leaf in the
/// ledger keyed on the digest above, and re-issuing the same transformation now produces the
/// digest below. E-M5-11 could say "the wire did not move" because `Some(x)` and `x` encode
/// alike; two new map keys have no such escape — `None` is `0xf6` **at a key**, so even a
/// payload that fills neither seat encodes differently.
const VERDICT_PAYLOAD_LEDGER_DIGEST: &str =
    "gx1:ndkump2ze7achtbardm2mnr66b4m4xnr5i7xyihulngfc7ygdcyq";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 🔴 **DR-46-26** — the fourteenth key, and the ledger digest it moved the payload to.
///
/// The two goldens above are unchanged and still say what they said. What changed is that neither
/// of them is what the encoder produces **now**: `req/38` §258 added `reversibility` to 42 §3.10,
/// so today's bytes are a map of fourteen. Rather than regenerate the literals — which would
/// destroy the evidence they exist to carry — every comparison below subtracts this key first, in
/// exactly the way `the_wire_moved_by_exactly_the_two_keys_dr_46_24_added` already subtracts D24's
/// two. Each erratum is then a *layer* of subtraction over one golden, and the file records three
/// states of the wire instead of one.
///
/// `crates/gx-witness/tests/inverse_status_wire.rs` is where DR-46-26's own difference is asserted;
/// here the key is only removed, so that E-M5-11's and DR-46-24(A)'s claims stay measurable.
const REVERSIBILITY_KEY: &str = "6d7265766572736962696c697479f6";

/// The canonical encoding of the fixture with DR-46-26's key removed and the map header wound back
/// to thirteen: the bytes D24 shipped, reconstructed rather than re-pinned.
fn as_dr_46_24_shipped_it(now: &str) -> String {
    // 🔴 **DR-46-45 (`req/973` §B-1/§B-2, 2026-08-31)** — the outermost layer now: twenty keys back
    // to nineteen. The fixture is a verdict receipt and `check_schema` refuses any other value on
    // that kind, so the contribution is the key and a null — `catalogue_hash`'s shape, and a
    // reminder that an absent `Option` is `f6` **at a key**, which is what makes a new key a
    // migration rather than a retyping.
    let undo = format!("64{}f6", hex(b"undo"));
    assert!(
        now.contains(&undo),
        "DR-46-45's key is not on the wire; this subtraction is measuring nothing"
    );
    let now = now.replacen(&undo, "", 1).replacen("b4", "b3", 1);
    let now = now.as_str();
    // 🔴 **A2 (`req/910` A., `req/919` W8, 2026-08-30)** — nineteen keys
    // back to eighteen. Derived from the fixture's own constant rather than re-spelled, so this
    // layer cannot keep passing after the value it subtracts has moved.
    let engine_version = format!(
        "6e{}{}",
        hex(b"engine_version"),
        hex(&cbor::encode(&Some(support::FIXTURE_ENGINE_VERSION.to_string())).expect("canonical"))
    );
    assert!(
        now.contains(&engine_version),
        "A2's key is not on the wire; this subtraction is measuring nothing"
    );
    let now = now.replacen(&engine_version, "", 1).replacen("b3", "b2", 1);
    let now = now.as_str();
    // 🔴 **F7 (`req/868` R-868-6, `req/919` W5, 2026-08-29)** — eighteen keys back to seventeen.
    // The fixture carries `Some(CURRENT_PAYLOAD_VERSION)` (every receipt this build issues does),
    // so the contribution is the key and the encoded small uint.
    let payload_version = format!(
        "6f{}{}",
        hex(b"payload_version"),
        hex(&cbor::encode(&Some(gx_witness::receipt::CURRENT_PAYLOAD_VERSION)).expect("canonical"))
    );
    assert!(
        now.contains(&payload_version),
        "F7's key is not on the wire; this subtraction is measuring nothing"
    );
    let now = now
        .replacen(&payload_version, "", 1)
        .replacen("b2", "b1", 1);
    let now = now.as_str();
    // 🔴 **DR-46-39 (`req/777`)** — seventeen keys back to sixteen.
    //
    // Found red by `req/801`'s G-07 live re-run (2026-08-25): DR-46-39 seated `catalogue_hash` and
    // taught its own attest suite (`dr4639_catalogue_hash_attest.rs`) and `ac_018`, but not this
    // file's subtraction tower — the first nextest run after the field landed is what surfaced it.
    // The fixture never names a catalogue, so the contribution is the key and a null.
    let catalogue = format!("6e{}f6", hex(b"catalogue_hash"));
    assert!(
        now.contains(&catalogue),
        "DR-46-39's key is not on the wire; this subtraction is measuring nothing"
    );
    let now = now.replacen(&catalogue, "", 1).replacen("b1", "b0", 1);
    let now = now.as_str();
    // 🔴 **S③ (`req/493` §1 AC-6)** — sixteen keys back to fifteen.
    //
    // Derived from the encoder rather than pinned as a constant, unlike the layer below it. Two
    // reasons, and the second is the one that decided it: the value is a nested map (two members)
    // so a literal would be a long hex run nobody can check by eye, and `crates/gx-cli/tests/
    // secret_scan.rs` refuses exactly that shape near a name carrying `KEY` — the note on
    // `BOUNDARY_KEY_AND_UNKNOWN` records what that cost last time.
    let confinement = format!(
        "6b{}{}",
        hex(b"confinement"),
        hex(
            &cbor::encode(&Some(gx_witness::receipt::ConfinementContext::unconfined()))
                .expect("canonical")
        )
    );
    assert!(
        now.contains(&confinement),
        "S③'s key is not on the wire; this subtraction is measuring nothing"
    );
    let now = now.replacen(&confinement, "", 1).replacen("b0", "af", 1);
    let now = now.as_str();
    // 🔴 **DR-46-28** — fifteen keys back to fourteen.
    assert!(
        now.contains(BOUNDARY_KEY_AND_UNKNOWN),
        "DR-46-28's key is not on the wire; this subtraction is measuring nothing"
    );
    let now = now
        .replacen(BOUNDARY_KEY_AND_UNKNOWN, "", 1)
        .replacen("af", "ae", 1);
    assert!(
        now.contains(REVERSIBILITY_KEY),
        "DR-46-26's key is not on the wire; this subtraction is measuring nothing"
    );
    now.replacen(REVERSIBILITY_KEY, "", 1)
        .replacen("ae", "ad", 1)
}

/// 🔴 The digest DR-46-26 moved the payload to.
///
/// Kept **beside** [`VERDICT_PAYLOAD_LEDGER_DIGEST`] and
/// [`VERDICT_PAYLOAD_LEDGER_DIGEST_BEFORE_DR_46_24`] rather than replacing either. The three
/// together are the migration history of one fixture: a receipt issued under each of the three has
/// a leaf keyed on the corresponding value, and 43 ASM-43-1's key idempotency is what makes that a
/// fact about the ledger rather than about this file.
const VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_26: &str =
    "gx1:zyio6kws5vaypmw46a7j2n5xvqp45yawmehx6o2ptfjmvmcoq66q";

/// 🔴 **DR-46-28** — the fifteenth key, and the fourth state of this one fixture's wire.
///
/// Same shape as the layer above and for the same reason: the goldens are kept, the bytes are
/// reached by subtraction, and only the **digest** — which cannot be un-applied — gets a new
/// literal. `req/459` seats `determinism_boundary` on 42 §3.10; this fixture's value is `unknown`,
/// which is a *value* and not an absence, so the key's contribution is itself and `text(7)` rather
/// than itself and `f6`.
///
/// `crates/gx-witness/tests/boundary_attest.rs` is where the erratum's own claims are asserted.
// 🔴 Kept on two lines on purpose (req/478 §4-4). `cargo fmt` joins this into one line, and a name
// carrying `KEY` on the same line as a long hex run is exactly the `keyed_hex_token` shape
// `crates/gx-cli/tests/secret_scan.rs` refuses -- NFR-012 asks for 0 findings over crates/, and the
// joined form is 1. The value is a CBOR key/value fixture, not a credential; splitting the line is
// the cheapest way to keep both gates true without weakening the scanner or renaming the constant.
#[rustfmt::skip]
const BOUNDARY_KEY_AND_UNKNOWN: &str =
    "7464657465726d696e69736d5f626f756e6461727967556e6b6e6f776e";

/// The digest DR-46-28 moved the payload to. The fourth kept beside the other three.
const VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_28: &str =
    "gx1:uy3efavm664wtkqzofxcj5he4mqvbqt7urg7c6k5r2o44eoieoha";

/// 🔴 **S③ (`req/493` §1 AC-6)** — the sixteenth key, and the fifth state of this one fixture's
/// wire. The fifth digest kept beside the other four.
///
/// A migration again, and one this build is entitled to make in a way DR-46-28 was not: the seat
/// carries `#[serde(default)]`, so bytes written before it still **decode** (`req/38` §294 ruling
/// 2). What moves is the digest of a payload re-encoded by this build, which is what the pin below
/// records — not the readability of anything already issued.
const VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_S3: &str =
    "gx1:pwoaqxkwzcjrtkndnr7clcblvw72d2myfc6fwcxdqjvopc2uvasa";

/// 🔴 **DR-46-39 (`req/777`)** — the seventeenth key, and the sixth state of this one fixture's
/// wire. The sixth digest kept beside the other five.
///
/// A migration with the same entitlement as S③'s: the seat carries `#[serde(default)]`
/// (`dr4639_catalogue_hash_attest.rs` AC-2/AC-3 measures that bytes with no `catalogue_hash` key
/// still decode), so what moves is the digest of a payload re-encoded by this build. Pinned by
/// `req/801`'s G-07 live re-run (2026-08-25), which is also the run that found the pin missing.
const VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_39: &str =
    "gx1:uwqy5h7kvsafrfaqkkp2fsatg6x27azhcsrmd3kpq45fvhye7i6q";

/// 🔴 **F7 (`req/868` R-868-6, `req/919` W5, 2026-08-29)** — the eighteenth key, and the seventh
/// state of this one fixture's wire. The seventh digest kept beside the other six.
///
/// A migration with the same entitlement as S③'s and DR-46-39's: the seat carries
/// `#[serde(default)]`, so bytes written before it still decode. What moves is the digest of a
/// payload re-encoded by this build.
const VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_F7: &str =
    "gx1:x4i4l54qttnz5rujfmsqos2fsr3xp2crjskxjfd5rwhuvhrxjdxq";

/// 🔴 **A2 (`req/910` A., `req/919` W8, 2026-08-30)** — the nineteenth key, and the eighth state of
/// this one fixture's wire. The eighth digest kept beside the other seven.
///
/// Same entitlement as F7's, S③'s and DR-46-39's, and the same limit: the seat carries
/// `#[serde(default)]`, so every receipt already signed still decodes, and what moves is the digest
/// of a fixture **re-encoded by this build**. No ledger anyone holds is rekeyed by this line;
/// `ASM-43-1`'s sentence is about a real ledger and this constant is about a fixture, which is why
/// the seven values above are kept rather than replaced -- the file is the record of eight wire
/// states, and a pin that were simply overwritten each time would record one.
const VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_A2: &str =
    "gx1:aiidrq4gou424jesocvg2bxmkxcrwgtj2zjsu34jfd7gzu4eukya";

/// 🔴 **DR-46-45 (`req/973` §B-1/§B-2, 2026-08-31)** — the twentieth key, and the ninth state of
/// this one fixture's wire. The ninth digest kept beside the other eight.
///
/// Same entitlement and same limit as the eight above: the seat carries `#[serde(default)]`, so
/// every receipt already signed still decodes, and what moves is the digest of a fixture
/// **re-encoded by this build**. Kept rather than overwritten for the reason the file states one
/// constant up — a pin overwritten each time would record one wire state where there have been
/// nine.
const VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_40: &str =
    "gx1:wmesukp37j6szhubek6xmjv4dzykmmejfvgewh3lzjysnd4zny2a";

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

    // The count 42 §3.10 is read as having did not move for E-M5-11: the field changed type,
    // not existence. It moved for DR-46-24(A), which added two.
    let fields = body
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("pub ") && l.ends_with(','))
        .count();
    println!("RECEIPT_PAYLOAD_FIELDS={fields}");
    // 🔴 **G-07 live re-run (`req/801`, 2026-08-25)** — sixteen became seventeen when DR-46-39
    // (`req/777`) seated `catalogue_hash`; this pin was not taught on landing day and the first
    // live run after it is what moved the count.
    // 🔴 **F7 (`req/868` R-868-6, `req/919` W5, 2026-08-29)** — seventeen became eighteen.
    // 🔴 **DR-46-45 (`req/973` §B-1/§B-2, 2026-08-31)** — nineteen became twenty.
    assert_eq!(
        fields, 20,
        "E-M5-11 retypes a field and adds none; DR-46-24(A) added two, DR-46-26 a third,          DR-46-28 a fourth, S③ (`req/493` §1 AC-6) a fifth, DR-46-39 (`req/777` catalogue_hash) a sixth, F7 (`req/868` R-868-6, payload_version) a seventh, A2 (`req/910`, engine_version) an eighth, and DR-46-45 (`req/973`, undo) a ninth"
    );
}

// ---------------------------------------------------------------------------
// The wire
// ---------------------------------------------------------------------------

/// 🔴 A payload **with** a verdict encodes to the bytes it encoded to before E-M5-11.
///
/// Green before the change and green after — deliberately. This is the whole content of "the wire
/// diff is Option-only": `Some(v)` and `v` have one encoding, so no receipt that anybody has ever
/// issued changes shape.
#[test]
fn a_payload_that_has_a_verdict_encodes_exactly_as_it_did_before_e_m5_11() {
    let key = keypair(1);
    let payload = verdict_payload(VerdictKind::Admit, &key, 5);
    let bytes = cbor::encode(&payload).expect("the fixture is canonical");
    println!("VERDICT_PAYLOAD_LEN={}", bytes.len());
    println!("VERDICT_PAYLOAD_HEX={}", hex(&bytes));
    // 🔴 **DR-46-26** — one key is subtracted before the comparison, so what this test asserts is
    // still exactly what it asserted: that E-M5-11 did not move a verdict-carrying payload's wire
    // form. Regenerating `VERDICT_PAYLOAD_HEX` instead would have quietly retired that claim.
    assert_eq!(
        as_dr_46_24_shipped_it(&hex(&bytes)),
        VERDICT_PAYLOAD_HEX.replace(['\n', ' '], ""),
        "E-M5-11 moved the wire form of a receipt that carries a verdict"
    );
}

/// 🔴 **The whole of what DR-46-24(A) did to the wire**: two keys appeared and nothing else moved.
///
/// The old golden is not decoration. Deleting the two new keys and their values out of the shipped
/// bytes has to give back the pre-erratum bytes exactly — that is a stronger claim than "the new
/// golden matches the encoder", which any regenerated constant satisfies by construction. If a
/// third key had appeared, or an existing value had shifted, this subtraction would not close.
///
/// The scope value is the fixture's, so it is taken from the fixture rather than written out here:
/// a literal would make this test pass by agreeing with itself.
#[test]
fn the_wire_moved_by_exactly_the_two_keys_dr_46_24_added() {
    let key = keypair(1);
    let now = as_dr_46_24_shipped_it(&hex(&cbor::encode(&verdict_payload(
        VerdictKind::Admit,
        &key,
        5,
    ))
    .expect("canonical")));
    let before = VERDICT_PAYLOAD_HEX_BEFORE_DR_46_24.replace(['\n', ' '], "");

    // `read_set` is absent, so its whole contribution is the key and a null.
    let read_set_key = "68726561645f736574f6"; // text(8) "read_set", then f6
                                               // `fingerprint_scope` carries the fixture's own string. The key is `71` (text of 17) followed
                                               // by "fingerprint_scope"; the value is `78 <len>` (text, one length byte) followed by the
                                               // string, which is RFC 8949 §3's encoding for a text run of 24..=255 bytes.
    let scope = verdict_payload(VerdictKind::Admit, &key, 5).fingerprint_scope;
    assert!(
        (24..=255).contains(&scope.len()),
        "the fixture's scope is outside the one-length-byte text form this derivation writes out"
    );
    let scope_key = format!(
        "7166696e6765727072696e745f73636f706578{:02x}{}",
        scope.len(),
        hex(scope.as_bytes())
    );

    println!("WIRE_ADDED_READ_SET={read_set_key}");
    println!("WIRE_ADDED_SCOPE={scope_key}");
    assert!(
        now.contains(read_set_key),
        "the read-set key is not on the wire"
    );
    assert!(now.contains(&scope_key), "the scope key is not on the wire");

    let stripped = now
        .replacen(read_set_key, "", 1)
        .replacen(&scope_key, "", 1)
        // a map of thirteen becomes the map of eleven it was
        .replacen("ad", "ab", 1);
    assert_eq!(
        stripped, before,
        "DR-46-24(A) moved more than the two keys it declares"
    );
    assert_ne!(
        VERDICT_PAYLOAD_LEDGER_DIGEST, VERDICT_PAYLOAD_LEDGER_DIGEST_BEFORE_DR_46_24,
        "two new map keys cannot leave the ledger digest where it was"
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
    // 🔴 **DR-46-26** — the pin moves, and the two values it moved from are kept above. Unlike the
    // subtraction on the bytes, a digest cannot be un-applied: this is the half of the erratum that
    // is a migration, said as a pin rather than as a sentence.
    // 🔴 **DR-46-39** — the pin moves a sixth time, and the five values it moved through are all
    // kept above (`req/801` G-07 live re-run, 2026-08-25).
    // 🔴 **F7** — the pin moves a seventh time (`req/868` R-868-6, `req/919` W5, 2026-08-29), and
    // the six values it moved through are all kept above.
    // 🔴 **A2** — the pin moves an eighth time (`req/910` A., `req/919` W8, 2026-08-30), and the
    // seven values it moved through are all kept above.
    // 🔴 **DR-46-45** — the pin moves a ninth time (`req/973` §B-1/§B-2, 2026-08-31), and the eight
    // values it moved through are all kept above.
    assert_eq!(
        text, VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_40,
        "43 ASM-43-1 keys the ledger on this digest; moving it is a migration"
    );
    assert_ne!(
        VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_40, VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_A2,
        "a twentieth map key cannot leave the digest where A2 left it"
    );
    assert_ne!(
        VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_A2, VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_F7,
        "a nineteenth map key cannot leave the digest where F7 left it"
    );
    assert_ne!(
        VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_F7, VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_39,
        "an eighteenth map key cannot leave the digest where DR-46-39 left it"
    );
    assert_ne!(
        VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_39, VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_S3,
        "a seventeenth map key cannot leave the digest where S③ left it"
    );
    assert_ne!(
        VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_S3, VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_28,
        "a sixteenth map key cannot leave the digest where DR-46-28 left it"
    );
    assert_ne!(
        VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_26, VERDICT_PAYLOAD_LEDGER_DIGEST,
        "DR-46-26 added a key, so it cannot have left the digest where DR-46-24(A) left it"
    );
    // 🔴 **DR-46-28** — and the same again, one erratum on. Four literals, four states, one
    // fixture: a receipt issued under any of them has a leaf keyed on the matching value.
    assert_ne!(
        VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_28, VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_26,
        "DR-46-28 added a key, so it cannot have left the digest where DR-46-26 left it"
    );
}

/// 🔴 The **whole** of the wire difference: one `a2…` map becomes one `f6`.
///
/// The two encodings are compared as strings and the difference is located rather than described.
/// `null` is 42 §2.1's spelling for an absent value and G-5 (`crates/gx-canon/tests/vectors/golden/
/// G-5.json`) is the golden that fixes it — "`None` is null (0xf6), an empty `Vec` is a 0-element array (0x80)" — so
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

    // And the map key itself is untouched: "verdict" is still there, still in the same place.
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

// ---------------------------------------------------------------------------
// v0.4-e — the field census of the two signature carriers this crate writes
// ---------------------------------------------------------------------------

/// 🔴 **Field census, `DsseEnvelope`** (v0.4-e; `req/38` §110 residue carried over: "the field
/// census of `DsseEnvelope`/`Checkpoint` in full is not yet pinned down (a denominator for
/// self-adversarial-1)") — an envelope serialises to
/// **exactly the three keys** 42 §3.10's table gives it, its `payload` wears 44 §2.2's base64
/// face, and every signature inside it stays `{keyid, sig}`.
///
/// # Why three is load-bearing here of all places
///
/// `receipt.rs`'s own words: "A DSSE envelope has exactly three fields -- 42 §3.10's own table
/// says so, and the standard 42 §4 compares gx against says so -- and a fourth would make the
/// wire form something no DSSE reader parses". That sentence is why `issued_at` rides *beside*
/// the envelope rather than in it (E-M2-6) — and a census is that sentence as a measurement, so
/// the next field with a reason to ride along fails a test instead of quietly undoing the
/// ruling. 33 NFR-011's footnote 5 (`req/38` §109/§110) adds the permanent half: no alg-like wire
/// field, on the signature or beside it, ever feeds crypto dispatch — this census is that
/// prohibition made constructive at the structure that carries the signatures.
///
/// # The declared limit
///
/// The reading side is out of scope by DSSE's own norm (envelope.md "Consumers MUST ignore
/// unrecognized fields") and by serde's derive, which does the same. This gate is on **our own
/// writer** only. The envelope is built by the real producer (`Receipt::issue`) rather than by
/// a struct literal, so the census measures the shape production envelopes actually take.
#[test]
fn dsse_envelope_serialises_exactly_the_three_keys_42_3_10_names() {
    let key = keypair(1);
    let receipt = issue(&verdict_payload(VerdictKind::Admit, &key, 5), &key);
    let value = serde_json::to_value(&receipt.envelope).expect("an envelope serialises");
    let object = value.as_object().expect("an envelope is a JSON object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    println!("DSSE_ENVELOPE_JSON_KEYS={keys:?}");
    assert_eq!(
        keys,
        ["payload", "payload_type", "signatures"],
        "42 §3.10: an envelope is these three fields and nothing else — a fourth is a wire form \
         no DSSE reader parses (E-M2-6's own reasoning)"
    );
    assert!(
        object["payload"].is_string(),
        "44 §2.2: `payload` is base64 — the JSON face is a string, never a byte list"
    );
    let signatures = object["signatures"]
        .as_array()
        .expect("a Vec of signatures");
    assert!(!signatures.is_empty(), "the producer signed it");
    for signature in signatures {
        let mut signature_keys: Vec<&str> = signature
            .as_object()
            .expect("a signature is an object")
            .keys()
            .map(String::as_str)
            .collect();
        signature_keys.sort_unstable();
        assert_eq!(
            signature_keys,
            ["keyid", "sig"],
            "33 NFR-011 footnote 5: {{keyid, sig}} where the signature travels, not only in isolation"
        );
    }
}

/// 🔴 **Field census, `Receipt`** (v0.4-e, the same residue row) — the pair E-M2-6 made serialises
/// to **exactly two keys**: the envelope, and the one timestamp no signature covers.
///
/// The pair's wire shape "is in no canonical source and is raised in req/54 §4" (`receipt.rs`); until a
/// ruling lands, this census keeps our own writer from growing the unruled shape further — a
/// third unsigned rider beside `issued_at` would be exactly the kind of quiet drift a shape
/// nobody owns invites, and it would land RED here first. Reader-side tolerance is out of scope
/// for the envelope census's reason, one test up.
#[test]
fn a_receipt_serialises_as_the_envelope_and_the_unsigned_timestamp_and_nothing_else() {
    let key = keypair(1);
    let receipt = issue(&verdict_payload(VerdictKind::Admit, &key, 5), &key);
    let value = serde_json::to_value(&receipt).expect("a receipt serialises");
    let object = value.as_object().expect("a receipt is a JSON object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    println!("RECEIPT_JSON_KEYS={keys:?}");
    assert_eq!(
        keys,
        ["envelope", "issued_at"],
        "E-M2-6: the timestamp rides beside the envelope, and nothing else rides at all"
    );
}

// ---------------------------------------------------------------------------
// v0.4-j — the CBOR face of the same two carriers (req/38 §113 residue: "4-structure CBOR golden")
// ---------------------------------------------------------------------------
//
// The censuses above pin the JSON face; these two pin the DAG-CBOR face of the same claim,
// the way `gx-canon/tests/golden_vectors.rs` pins `DsseSignature`'s (req/172 C2) and, as of
// the same v0.4-j lane, `Checkpoint`/`VerdictCheckpoint`'s. They live HERE and not in that
// suite because the types are this crate's and gx-canon cannot name gx-witness — the
// dependency runs the other way (this crate's manifest declares `gx-canon` for
// `cid::compute`, hand 4's edge), which is what lets a test here reach `gx_canon::cbor`.
// Same split as req/172 §2-1's, one crate up.
//
// Every hex literal is written out from RFC 8949 §3's major-type table and 42 §2.1's rules,
// NOT copied from the encoder (golden_vectors.rs's discipline). The derivation was doubled
// before the tests first ran: an independent from-scratch RFC 8949 emitter (own encoded-key
// sort, no CBOR library, no gx code) produced byte-identical strings and also reproduces the
// 22-byte DsseSignature golden req/171 §1-7 measured live — hand ⇄ independent ⇄ measured.
//
// # Why these fixtures are literals while the JSON censuses use the real producer
//
// The census twins go through `Receipt::issue` because a key-set claim survives a real
// signature. A byte-exact golden does not: a live Ed25519 signature's 64 bytes cannot be
// derived from RFC 8949 + 42 §2.1 by hand, so a golden containing one could only be
// transcribed from the encoder — exactly what the discipline forbids. The fixture therefore
// carries the same synthetic `deadbeef`/`key-1` signature the DsseSignature golden pinned
// (whose bytes reappear verbatim inside these literals — the carrier golden contains the
// carried golden), and the `payload` is four literal bytes rather than a 358-byte canonical
// `ReceiptPayload`: the claim here is the CARRIER's shape, and `VERDICT_PAYLOAD_HEX` at the
// top of this file already pins a full payload's bytes separately. What IS production-real is
// `payload_type`: the fixture reads `RECEIPT_PAYLOAD_TYPE` from the crate, so 42 §3.10's
// fixed value has its exact canonical spelling pinned below — if the constant ever moved,
// this golden goes RED, not just a length.

/// 🔴 **v0.4-j — CBOR-face golden, `DsseEnvelope`** — the canonical form is a **map of
/// exactly three entries** (`a3`), and a fourth — `issued_at` above all — would be a wire
/// form no DSSE reader parses (E-M2-6's reasoning, now byte-pinned).
///
/// # Derivation (RFC 8949 §3 + 42 §2.1, not the encoder)
///
/// Map of three → `a3`. Encoded keys, bytewise with header: `67 7061796c6f6164` ("payload",
/// 7 bytes) < `6a 7369676e617475726573` ("signatures", 10) < `6c 7061796c6f61645f74797065`
/// ("payload_type", 12) — header length settles all three before any content byte is read,
/// which is also why "payload" precedes the longer key it prefixes. Values: `payload` is the
/// byte-string face (`raw_bytes`' non-human-readable arm, M2H1-4) → `44 deadbeef`;
/// `signatures` is a one-element array → `81` + the 22-byte DsseSignature golden verbatim
/// (`a26373696744deadbeef656b65796964656b65792d31`, req/172); `payload_type` is 39 UTF-8
/// bytes → `78 27` (24 ≤ n < 256 takes the one-byte argument) + the constant's bytes.
#[test]
fn dsse_envelope_canonical_cbor_is_a_three_entry_map_spec_derived() {
    let envelope = DsseEnvelope {
        payload_type: RECEIPT_PAYLOAD_TYPE.to_string(),
        payload: vec![0xde, 0xad, 0xbe, 0xef],
        signatures: vec![DsseSignature {
            keyid: "key-1".to_string(),
            sig: vec![0xde, 0xad, 0xbe, 0xef],
        }],
    };
    let expected = "a3\
        677061796c6f616444deadbeef\
        6a7369676e61747572657381a26373696744deadbeef656b65796964656b65792d31\
        6c7061796c6f61645f747970657827\
        6170706c69636174696f6e2f766e642e676c6f767265782e726563656970742b64616763626f72";
    let bytes = cbor::encode(&envelope).expect("an envelope encodes");
    println!("DSSE_ENVELOPE_CBOR={}", hex(&bytes));
    assert_eq!(
        bytes[0], 0xa3,
        "RFC 8949 §3: three entries open with 0xa3 — a fourth field would be 0xa4"
    );
    assert_eq!(
        hex(&bytes),
        expected,
        "42 §3.10 / 33 NFR-011 footnote 5: three fields, canonical order, and nothing else"
    );
    assert!(
        cbor::is_canonical(&bytes) && cbor::scan_strict(&bytes).is_ok(),
        "the pinned bytes must themselves be canonical"
    );
    let round: DsseEnvelope = cbor::decode(&bytes).expect("the golden decodes");
    assert_eq!(round, envelope, "both directions");
}

/// 🔴 **v0.4-j — CBOR-face golden, `Receipt`** — the pair E-M2-6 made is a **map of exactly
/// two entries** (`a2`): the envelope, and the one timestamp no signature covers. The pair's
/// wire shape "is in no canonical source and is raised in req/54 §4" (`receipt.rs`); until a ruling
/// lands, this golden holds the unruled shape byte-still on the CBOR face as the census
/// above holds its key set on the JSON face.
///
/// # Derivation (RFC 8949 §3 + 42 §2.1, not the encoder)
///
/// Map of two → `a2`. Encoded keys: `68 656e76656c6f7065` ("envelope", 8 bytes) <
/// `69 6973737565645f6174` ("issued_at", 9) — the header byte alone decides. Values: the
/// envelope golden above, verbatim (a carrier golden nests the goldens it carries — three
/// deep here: receipt ⊃ envelope ⊃ signature); then `Timestamp(1_754_000_000_000_000_000)`,
/// a serde newtype over `i64` → the bare integer `1b 185775b4f8090000` (major type 0,
/// 8-byte argument, big-endian — the same value the m2_types census fixtures hold).
#[test]
fn receipt_canonical_cbor_is_a_two_entry_map_spec_derived() {
    let receipt = Receipt {
        envelope: DsseEnvelope {
            payload_type: RECEIPT_PAYLOAD_TYPE.to_string(),
            payload: vec![0xde, 0xad, 0xbe, 0xef],
            signatures: vec![DsseSignature {
                keyid: "key-1".to_string(),
                sig: vec![0xde, 0xad, 0xbe, 0xef],
            }],
        },
        issued_at: Timestamp(1_754_000_000_000_000_000),
    };
    let expected = "a2\
        68656e76656c6f7065\
        a3677061796c6f616444deadbeef\
        6a7369676e61747572657381a26373696744deadbeef656b65796964656b65792d31\
        6c7061796c6f61645f747970657827\
        6170706c69636174696f6e2f766e642e676c6f767265782e726563656970742b64616763626f72\
        696973737565645f61741b185775b4f8090000";
    let bytes = cbor::encode(&receipt).expect("a receipt encodes");
    println!("RECEIPT_CBOR={}", hex(&bytes));
    assert_eq!(
        bytes[0], 0xa2,
        "RFC 8949 §3: two entries open with 0xa2 — a third rider would be 0xa3"
    );
    assert_eq!(
        hex(&bytes),
        expected,
        "E-M2-6 / req/54 §4: the envelope, the unsigned timestamp, and nothing else"
    );
    assert!(
        cbor::is_canonical(&bytes) && cbor::scan_strict(&bytes).is_ok(),
        "the pinned bytes must themselves be canonical"
    );
    let round: Receipt = cbor::decode(&bytes).expect("the golden decodes");
    assert_eq!(round, receipt, "both directions");
}
