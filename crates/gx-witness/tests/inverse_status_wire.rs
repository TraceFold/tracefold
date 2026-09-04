// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-26**: C-25's three values reach the **signed bytes**, and the wire moves by exactly
//! one key.
//!
//! Spec: 42 §3.10 for the payload table (now fourteen rows), ASM-14 for the two kinds, `req/38`
//! §258 for the erratum. The defect this closes is `req/38` §198 ruling (b), verbatim:
//!
//! > **A-4 is judged half done**: `unknown` reaches the adapter's return value, the refusal
//! > sentence and the probe — **the receipt payload is still the same shape as `false`** →
//! > **DR-46-13 raised** (a seventh `InverseStatus` word, or a field added to 42 §3.10 — a change
//! > to a frozen face, Lean-side confirmation included).
//!
//! D24 took the first branch and seated the seventh word; DR-46-26 gave that word a writer, which
//! closed the escrow row, the API and the CLI. This file is the **other** branch, and the reason
//! both were needed is one sentence long: a reader who holds the receipt and nothing else saw
//! `inverse_delta: null` for "there is no undo" and for "nobody found out" alike.
//!
//! # The form, which is D24's and not a new one
//!
//! `tests/receipt_verdict_wire.rs` established the discipline this file follows: **the golden from
//! before the change is kept as a literal**, and the change is measured as a *subtraction* from the
//! bytes the encoder produces now. A regenerated golden records what the code does; a golden
//! carried across a change records what the change did **not** do. So the literal below is the
//! thirteen-key map D24 shipped, and
//! [`the_wire_moved_by_exactly_the_one_key_dr_46_26_added`] removes one key from today's encoding
//! and asserts the result is that literal, byte for byte.

mod support;

use gx_canon::cbor;
use gx_core::{Reversibility, VerdictKind};
use gx_witness::receipt::{ReceiptKind, ReceiptPayload};
use gx_witness::Error;
use support::{commit_payload, keypair, verdict_payload};

/// 🔴 **The pre-DR-46-26 form** (DR-46-24(A)'s): `ad` = a map of thirteen.
///
/// Copied verbatim from `tests/receipt_verdict_wire.rs`'s `VERDICT_PAYLOAD_HEX`, which is the
/// canonical DAG-CBOR of `verdict_payload(Admit, keypair(1), 5)` as D24 shipped it. It is
/// duplicated rather than imported because the two files pin the same bytes for two different
/// reasons — that one is "what D24 moved the wire *to*", this one is "what DR-46-26 moved it
/// *from*" — and a shared constant would let one erratum's regeneration silently rewrite the
/// other's evidence.
const VERDICT_PAYLOAD_HEX_BEFORE_DR_46_26: &str = "\
ad666b65795f6964656b65792d316776657264696374a2646b696e646541646d69746c70726f6f665f64696765737458\
20000000000007a12500000000000000000000000000000000000000000000000068656e666f72636564f46872656164\
5f736574f66c726563656970745f6b696e646e56657264696374526563656970746d63616e6f6e6963616c5f63696458\
2000000000008954450000000000000000000000000000000000000000000000006d696e76657273655f64656c7461f6\
6e7472616e73666f726d6174696f6e582000000000008954450000000000000000000000000000000000000000000000\
006f696e636c7573696f6e5f70726f6f66f67166696e6765727072696e745f73636f7065781a666978747572653a2f2f\
73636f70652f6f6e652d6f626a656374746661696c5f706f73747572655f656e6761676564f57818707265636f6e6469\
74696f6e5f66696e6765727072696e745820070707070707070707070707070707070707070707070707070707070707\
07077819706f7374636f6e646974696f6e5f66696e6765727072696e74f6";

/// `ReceiptPayload::ledger_digest` of that value, as D24 left it.
///
/// Pinned separately from the bytes because it is what the **ledger** committed to: a receipt
/// issued between D24 and this lane has a leaf keyed on this digest, and 43 ASM-43-1's key
/// idempotency is what makes the difference a migration rather than a retyping.
const VERDICT_PAYLOAD_LEDGER_DIGEST_BEFORE_DR_46_26: &str =
    "gx1:ndkump2ze7achtbardm2mnr66b4m4xnr5i7xyihulngfc7ygdcyq";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The one key DR-46-26 adds to a payload that leaves it empty: `text(13) "reversibility"`, `f6`.
///
/// `6d` is major type 3 with a length of thirteen, and `f6` is 42 §2.1's `null` (golden vector
/// G-5). Written out here as a derivation rather than as a magic string, so a reader can check the
/// arithmetic without a CBOR decoder.
fn absent_reversibility_key() -> String {
    format!("6d{}f6", hex(b"reversibility"))
}

/// 🔴 **DR-46-28** — one more layer of subtraction, and no new literal.
///
/// The golden above is unchanged and still says what it said. What changed again is that it is not
/// what the encoder produces **now**: `req/459` seats `determinism_boundary` on 42 §3.10, so
/// today's bytes are a map of fifteen. `crates/gx-witness/tests/boundary_attest.rs` is where *that*
/// key's own difference is asserted; here it is only removed, so DR-46-26's claim stays measurable.
/// This is the third erratum to become a layer over the one golden `receipt_verdict_wire.rs` minted.
fn without_dr_46_28(now: &str) -> String {
    let contribution = format!("74{}67{}", hex(b"determinism_boundary"), hex(b"Unknown"));
    assert!(
        now.contains(&contribution),
        "DR-46-28's key is not on the wire; this subtraction is measuring nothing"
    );
    now.replacen(&contribution, "", 1).replacen("af", "ae", 1)
}

/// 🔴 **S③ (`req/493` §1 AC-6)** — the fourth layer, and still no new literal.
///
/// Same shape as [`without_dr_46_28`] one erratum along: the golden is untouched and says what it
/// said, and today's bytes are a map of sixteen. `crates/gx-witness/tests/confinement_attest.rs`
/// asserts what *this* key adds; here it is only removed, so DR-46-26's claim stays measurable
/// through it. The contribution is derived from the encoder rather than spelled, because the value
/// is a nested map and a literal for it would be the one thing in this file a hand could regenerate.
fn without_s3_confinement(now: &str) -> String {
    let contribution = format!(
        "6b{}{}",
        hex(b"confinement"),
        hex(
            &cbor::encode(&Some(gx_witness::receipt::ConfinementContext::unconfined()))
                .expect("canonical")
        )
    );
    assert!(
        now.contains(&contribution),
        "S③'s key is not on the wire; this subtraction is measuring nothing"
    );
    now.replacen(&contribution, "", 1).replacen("b0", "af", 1)
}

/// 🔴 **DR-46-39 (`req/777`)** — the fifth layer, and the one the first live run had to add.
///
/// Same shape as [`without_dr_46_28`] and [`without_s3_confinement`] one erratum along: the golden
/// is untouched and says what it said, and today's bytes are a map of seventeen.
/// `crates/gx-witness/tests/dr4639_catalogue_hash_attest.rs` asserts what *this* key adds; here it
/// is only removed, so DR-46-26's claim stays measurable through it. Found red by `req/801`'s G-07
/// live re-run (2026-08-25): DR-46-39's lane taught its own attest suite and `ac_018` but not this
/// file's layers, and the two static S① audits (`req/753`/`req/767`) ran no cargo, so the first
/// nextest invocation after the field landed is what surfaced it.
fn without_dr_46_39(now: &str) -> String {
    let contribution = format!("6e{}f6", hex(b"catalogue_hash"));
    assert!(
        now.contains(&contribution),
        "DR-46-39's key is not on the wire; this subtraction is measuring nothing"
    );
    now.replacen(&contribution, "", 1).replacen("b1", "b0", 1)
}

/// 🔴 **F7 (`req/868` R-868-6, `req/919` W5, 2026-08-29)** — the sixth layer, outermost: today's
/// bytes are a map of eighteen. `crates/gx-witness/tests/boundary_attest.rs` asserts what *this*
/// key adds; here it is only removed, so DR-46-26's claim stays measurable through it.
fn without_payload_version(now: &str) -> String {
    let contribution = format!(
        "6f{}{}",
        hex(b"payload_version"),
        hex(&cbor::encode(&Some(gx_witness::receipt::CURRENT_PAYLOAD_VERSION)).expect("canonical"))
    );
    assert!(
        now.contains(&contribution),
        "F7's key is not on the wire; this subtraction is measuring nothing"
    );
    now.replacen(&contribution, "", 1).replacen("b2", "b1", 1)
}

/// 🔴 **A2 (`req/910` A., `req/919` W8, 2026-08-30)** — the seventh layer, outermost: today's bytes
/// are a map of nineteen. `crates/gx-witness/tests/r919_engine_version_attest.rs` asserts what
/// *this* key adds; here it is only removed, so DR-46-26's claim stays measurable through it.
///
/// The contribution is derived from the fixture's own constant rather than re-spelled, for the
/// reason `req/801`'s G-07 run recorded one erratum earlier: a tower that carries its own copy of a
/// value goes on passing after the value it is subtracting has changed.
fn without_engine_version(now: &str) -> String {
    let contribution = format!(
        "6e{}{}",
        hex(b"engine_version"),
        hex(&cbor::encode(&Some(support::FIXTURE_ENGINE_VERSION.to_string())).expect("canonical"))
    );
    assert!(
        now.contains(&contribution),
        "A2's key is not on the wire; this subtraction is measuring nothing"
    );
    now.replacen(&contribution, "", 1).replacen("b3", "b2", 1)
}

/// 🔴 **DR-46-45 (`req/973` §B-1/§B-2, 2026-08-31)** — the eighth layer, outermost: today's bytes
/// are a map of twenty. `crates/gx-engine/tests/r973_undo_attestation.rs` asserts what *this* key
/// adds; here it is only removed, so DR-46-26's claim stays measurable through it.
///
/// The fixture is a verdict receipt and `check_schema` refuses any other value on that kind, so the
/// contribution is the key and a null.
fn without_undo(now: &str) -> String {
    let contribution = format!("64{}f6", hex(b"undo"));
    assert!(
        now.contains(&contribution),
        "DR-46-45's key is not on the wire; this subtraction is measuring nothing"
    );
    now.replacen(&contribution, "", 1).replacen("b4", "b3", 1)
}

// ---------------------------------------------------------------------------
// The type
// ---------------------------------------------------------------------------

/// The field exists, is an `Option`, and the struct now has fourteen.
///
/// Structural, for `e_m5_11_the_verdict_field_is_optional`'s reason: a behavioural probe can show
/// that a value round-trips, and only a scan can show that the **declaration** is the one the
/// erratum names.
#[test]
fn dr_46_26_the_payload_declares_an_inverse_status_field() {
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
        .find(|l| l.starts_with("pub reversibility"))
        .expect("DR-46-26 seats C-25's answer on the payload");
    println!("RECEIPT_REVERSIBILITY_FIELD={field:?}");
    assert_eq!(
        field, "pub reversibility: Option<Reversibility>,",
        "DR-46-26: three values plus the absence of an answer, which is not a fourth value"
    );

    let fields = body
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("pub ") && l.ends_with(','))
        .count();
    println!("RECEIPT_PAYLOAD_FIELDS={fields}");
    // 🔴 **G-07 live re-run (`req/801`, 2026-08-25)** — this pin sat at sixteen while DR-46-39
    // (`req/777`, commit `49b09617`) seated `catalogue_hash` as the seventeenth field, and the two
    // static S① audits (`req/753`/`req/767`) could not see it because neither ran cargo. The first
    // live run did, on the first command. The count below is the corrected pin, and the genealogy
    // names the field that moved it.
    // 🔴 **F7 (`req/868` R-868-6, `req/919` W5, 2026-08-29)** — seventeen became eighteen when
    // `payload_version` seated.
    // 🔴 **A2 (`req/910` A., `req/919` W8, 2026-08-30)** — eighteen became nineteen when
    // `engine_version` seated.
    // 🔴 **DR-46-45 (`req/973` §B-1/§B-2, 2026-08-31)** — nineteen became twenty when `undo` seated.
    assert_eq!(
        fields, 20,
        "DR-46-24(A) took the count to thirteen, DR-46-26 the fourteenth, DR-46-28 the fifteenth,          and S③ (`req/493` §1 AC-6) the sixteenth, DR-46-39 (`req/777` catalogue_hash) the seventeenth, F7 (`req/868` R-868-6, payload_version) the eighteenth, A2 (`req/910`, engine_version) the nineteenth, and DR-46-45 (`req/973`, undo) the twentieth"
    );
}

// ---------------------------------------------------------------------------
// The wire
// ---------------------------------------------------------------------------

/// 🔴 **The subtraction.** Today's encoding, minus one key, is D24's bytes.
///
/// This is the whole claim the erratum makes about the wire, in the one form that cannot be
/// satisfied by regenerating anything: the literal above was written before this lane existed, and
/// the only way for the assertion below to pass is for the encoder to have added `reversibility`
/// and moved nothing else — not a field's order, not a value, not the map header beyond its count.
#[test]
fn the_wire_moved_by_exactly_the_one_key_dr_46_26_added() {
    let key = keypair(1);
    let encoded =
        hex(&cbor::encode(&verdict_payload(VerdictKind::Admit, &key, 5)).expect("canonical"));
    let now = without_dr_46_28(&without_s3_confinement(&without_dr_46_39(
        &without_payload_version(&without_engine_version(&without_undo(&encoded))),
    )));
    let before = VERDICT_PAYLOAD_HEX_BEFORE_DR_46_26.replace(['\n', ' '], "");
    let added = absent_reversibility_key();

    println!("WIRE_ADDED_REVERSIBILITY={added}");
    println!("WIRE_NOW_BYTES={}", now.len() / 2);
    println!("WIRE_BEFORE_BYTES={}", before.len() / 2);
    assert!(
        now.contains(&added),
        "the inverse-status key is not on the wire"
    );

    let stripped = now
        .replacen(&added, "", 1)
        // a map of fourteen becomes the map of thirteen it was
        .replacen("ae", "ad", 1);
    assert_eq!(
        stripped, before,
        "DR-46-26 moved more than the one key it declares"
    );
}

/// 🔴 And the ledger digest moved, which is the half of the erratum that is a migration.
///
/// `None` is `0xf6` **at a key**, so even a payload that fills the new seat with nothing encodes
/// differently — E-M5-11's "the wire did not move" escape (`Some(x)` and `x` encode alike) is not
/// available to a field that did not exist. Said plainly so that nobody reads the subtraction above
/// as "nothing changed": what did not change is *the other thirteen keys*.
#[test]
fn the_ledger_digest_moved_and_that_is_the_migration() {
    let key = keypair(1);
    let payload = verdict_payload(VerdictKind::Admit, &key, 5);
    let text = gx_canon::cid::to_text(&payload.ledger_digest().expect("the fixture digests"));
    // 🔴 **DR-46-28** moved it again, so this line is "after DR-46-26 *and* DR-46-28". The
    // assertion below is `assert_ne!` against the value from before DR-46-26 and is unaffected: a
    // key added twice cannot put the digest back where it started.
    println!("VERDICT_PAYLOAD_LEDGER_DIGEST_AFTER_DR_46_26_AND_28={text}");
    println!("VERDICT_PAYLOAD_LEDGER_DIGEST_BEFORE_DR_46_26={VERDICT_PAYLOAD_LEDGER_DIGEST_BEFORE_DR_46_26}");
    assert_ne!(
        text, VERDICT_PAYLOAD_LEDGER_DIGEST_BEFORE_DR_46_26,
        "a new map key cannot leave the ledger digest where it was"
    );
}

/// The three values are distinguishable **on the wire**, which is the point of the field.
///
/// `req/350` §7-7's rule, applied one erratum later: "run both values of the granularity tag
/// through a path that actually reaches them, once each (**introducing is not functioning**)".
/// Three values here, and the assertion that binds them is that the encodings are pairwise
/// different — a field whose three values encoded alike would be the defect with more ceremony.
#[test]
fn the_three_values_of_c_25_encode_differently_from_each_other_and_from_absence() {
    let key = keypair(1);
    let proof = gx_core::InclusionProof {
        leaf_index: 0,
        tree_size: 1,
        audit_path: Vec::new(),
    };
    let base = commit_payload(&key, 11, proof);

    let mut seen: Vec<(String, String)> = Vec::new();
    for (name, value) in [
        ("true", Some(Reversibility::True)),
        ("false", Some(Reversibility::False)),
        ("unknown", Some(Reversibility::Unknown)),
        ("absent", None),
    ] {
        let payload = ReceiptPayload {
            reversibility: value,
            ..base.clone()
        };
        payload
            .check_schema()
            .expect("a commit receipt may carry any of the three, or none");
        let bytes = hex(&cbor::encode(&payload).expect("canonical"));
        println!("C25_WIRE {name}={} bytes", bytes.len() / 2);
        for (other, prior) in &seen {
            assert_ne!(
                &bytes, prior,
                "`{name}` and `{other}` encode to the same bytes, which is the defect DR-46-26 exists to close"
            );
        }
        seen.push((name.to_string(), bytes));
    }
    assert_eq!(seen.len(), 4, "four shapes were compared");
}

// ---------------------------------------------------------------------------
// ASM-14: the kind-dependent rule, which is the third field to carry it
// ---------------------------------------------------------------------------

/// A `VerdictReceipt` may not carry an answer, and `check_schema` is what says so.
///
/// The same rule `inverse_delta` and `read_set` carry, for the same one-line reason: the escrow
/// answers C-25, the escrow is 43 T-10b, and T-10b is inside commit. A verdict receipt claiming an
/// answer would be reporting on a question nothing had asked when it was signed.
#[test]
fn asm_14_refuses_an_inverse_status_on_a_verdict_receipt() {
    let key = keypair(1);
    let clean = verdict_payload(VerdictKind::Admit, &key, 5);
    assert_eq!(clean.receipt_kind, ReceiptKind::VerdictReceipt);
    clean
        .check_schema()
        .expect("the fixture is what ASM-14 allows");

    for value in [
        Reversibility::True,
        Reversibility::False,
        Reversibility::Unknown,
    ] {
        let payload = ReceiptPayload {
            reversibility: Some(value),
            ..clean.clone()
        };
        let refusal = payload
            .check_schema()
            .expect_err("ASM-14: a verdict receipt answers no question the escrow asks");
        let text = match &refusal {
            Error::Schema { detail } => detail.clone(),
            other => panic!("the schema refuses; it answered {other:?}"),
        };
        println!("ASM14_REFUSAL {}={text}", value.as_str());
        assert!(
            text.contains("inverse-status"),
            "the refusal names the field it is about"
        );
    }
}

/// The relocation, asserted where a reader would look for it.
///
/// `Reversibility` was `gx_adapter_mcp::catalogue`'s until this lane. It is `gx_core`'s now, and the
/// move was forced rather than preferred: this crate seats the value on a receipt and does not
/// depend on `gx-substrate` (where the trait that produces it lives), while `gx-adapter-mcp`
/// depends on `gx-substrate` and so could not hold a type the trait names. `gx-core` is the one
/// crate every party already names. The three words and their order are C-25's and did not move.
#[test]
fn the_three_words_are_c_25s_and_the_type_came_down_to_gx_core() {
    println!("C25_ALL={:?}", Reversibility::ALL);
    assert_eq!(Reversibility::ALL, ["true", "false", "unknown"]);
    assert_eq!(Reversibility::True.as_str(), "true");
    assert_eq!(Reversibility::False.as_str(), "false");
    assert_eq!(Reversibility::Unknown.as_str(), "unknown");

    let catalogue = include_str!("../../gx-adapter-mcp/src/catalogue.rs");
    assert!(
        catalogue.contains("pub use gx_core::Reversibility;"),
        "gx-adapter-mcp re-exports the relocated type, so `gx_adapter_mcp::Reversibility` still         names it"
    );
    // The declaration itself, not a mention of it: the no-delete note left in `catalogue.rs`
    // quotes the old `pub enum Reversibility` inside a comment, and a scan that could not tell a
    // comment from a declaration would fail on the very record that says the move happened.
    let declared: Vec<&str> = catalogue
        .lines()
        .filter(|l| l.trim_start().starts_with("pub enum Reversibility"))
        .collect();
    println!("MCP_REVERSIBILITY_DECLARATIONS={declared:?}");
    assert!(
        declared.is_empty(),
        "the type is declared in gx-core and nowhere else"
    );
    let core = include_str!("../../gx-core/src/reversibility.rs");
    assert!(
        core.contains("pub enum Reversibility {"),
        "gx-core is where the relocated type now lives"
    );
}
