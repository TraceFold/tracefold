// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R-930-B1** — the kind-binding rule (R-939-1) and `.gx` kind 15.
//!
//! Spec: `req/939_KIND_BATCH1_C1_DESIGNTOKEN_2026-08-30.md` §2 (the C-1 remedy and what it does
//! **not** close), §3 (the body ruling) and §7 (the acceptance criteria these are named after).
//!
//! # Two halves, written in two passes
//!
//! The first two probes were written and run **before** the remedy, so that what follows repairs
//! something measured rather than something asserted. They still pass, and that is not an
//! oversight: what they measure is the residual (`req/939` §2-F-1), and a limit re-shown on every
//! run cannot quietly be believed closed.
//!
//! # What the adversary is, here
//!
//! Defect C-1 (`req/930` §4 Q3) is that `.gx` identity covers the body and not the header, so the
//! kind is a claim standing outside the number that is checked. The adversarial probes below are
//! built the way an attacker would build them: **each one carries a correct identity**. A file
//! whose identity does not recompute is refused by machinery that already existed and proves
//! nothing about this lane.

use std::time::Instant;

use gx_core::Cid;
use gx_witness::design_token::{
    Declaration, DesignToken, Document, KernelString, Layer, Unsaid, DESIGN_TOKEN_TAG,
};
use gx_witness::gxfile::{self, GxKind, KindWitness, Refusal, FORMAT_VERSION, HEADER_LEN, MAGIC};

/// A `.gx` file built by hand, so that a probe can put a body under a header the writer would
/// never pair it with. The identity is computed over the bytes actually written, which is what
/// makes these probes about the kind rather than about the digest.
fn hand_built(kind: GxKind, body: &[u8]) -> Vec<u8> {
    let cid = gxfile::body_cid(body).expect("the probe's body is canonical");
    framed(kind, &cid.0, body)
}

/// The same, for a probe whose body has **no** identity to claim because it is not canonical at
/// all. The claim is zeroed: a reader that got as far as comparing it would refuse anyway, and the
/// probes that use this are asserting that it never gets that far.
fn framed(kind: GxKind, claimed: &[u8; 32], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + body.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
    out.extend_from_slice(&kind.code().to_be_bytes());
    out.extend_from_slice(claimed);
    out.extend_from_slice(body);
    out
}

fn line(at: &str, text: &str) -> KernelString {
    KernelString {
        at: at.to_string(),
        text: text.to_string(),
    }
}

fn a_board() -> DesignToken {
    DesignToken::board(
        "req939.demo",
        vec![
            line("n1.one", "reversibility is a property"),
            line("n1.title", "undo"),
            line("n2.title", "escrow"),
        ],
    )
    .expect("the fixture is in order")
}

// ---------------------------------------------------------------------------
// Pass 1 -- the defect, measured on the build that had it (and kept as the residual)
// ---------------------------------------------------------------------------

/// 🔴 **C-1, measured** (`req/930` §4 Q3): one body, two kinds, one identity.
///
/// This passed on the build that had the defect, and it passes now, because the remedy does not
/// change what a `Cid` is. What it changes is reachability: the two files below cannot both be
/// admitted, which the next probe shows. Printed as well as asserted, so the measurement is in the
/// log rather than only in a document.
#[test]
fn one_body_under_two_kinds_carries_one_identity() {
    let body = gx_canon::cbor::encode(&"the same document").expect("canonical");
    let as_receipt = hand_built(GxKind::Receipt, &body);
    let as_checkpoint = hand_built(GxKind::Checkpoint, &body);

    assert_ne!(
        &as_receipt[4..6],
        &as_checkpoint[4..6],
        "the probe did not actually vary the kind"
    );
    assert_eq!(
        &as_receipt[6..HEADER_LEN],
        &as_checkpoint[6..HEADER_LEN],
        "C-1 no longer holds; if this fails the identity derivation changed and req/939 §2-E is \
         stale"
    );
    println!(
        "R939_C1 kinds={},{} identity_equal=true body_bytes={}",
        GxKind::Receipt.code(),
        GxKind::Checkpoint.code(),
        body.len()
    );
}

/// 🔴 **C-1's reach before the remedy** — what actually refused the mislabelled file.
///
/// Not a binding: `is_shipped`. The file is turned away because this build has no codec for
/// `Checkpoint`, not because anything noticed that its body does not belong to that kind. That is
/// the whole shape of the defect — the day a second kind shipped, that refusal would have gone away
/// with nothing to take its place. `req/939` §2-C is what took its place.
#[test]
fn the_shipped_list_alone_is_what_turns_away_a_kind_with_no_codec() {
    let body = gx_canon::cbor::encode(&"the same document").expect("canonical");
    let as_checkpoint = hand_built(GxKind::Checkpoint, &body);

    assert!(matches!(
        gxfile::read(&as_checkpoint),
        Err(Refusal::KindNotShipped {
            kind: GxKind::Checkpoint
        })
    ));
}

// ---------------------------------------------------------------------------
// Pass 2 -- the rule, and the kind that is the first to satisfy it
// ---------------------------------------------------------------------------

/// 🔴 **AC-1** — the rule itself: a kind may ship only if the bytes its identity covers name it.
///
/// The whole C-1 remedy in one predicate, asserted over the registry in both directions. A later
/// hand that ships a kind without a witness re-opens the defect, and this fails rather than letting
/// it through.
#[test]
fn shipping_and_naming_itself_in_its_body_are_the_same_set() {
    for kind in GxKind::REGISTRY {
        assert_eq!(
            kind.is_shipped(),
            kind.body_witness().is_some(),
            "{kind}: R-939-1 broken"
        );
    }
    let shipped: Vec<GxKind> = GxKind::REGISTRY
        .into_iter()
        .filter(|k| k.is_shipped())
        .collect();
    assert_eq!(shipped, vec![GxKind::Receipt, GxKind::DesignToken]);
    assert_eq!(
        GxKind::DesignToken.body_witness(),
        Some(KindWitness::InBandTag(DESIGN_TOKEN_TAG))
    );
    assert_eq!(GxKind::DesignToken.code(), 15);
}

/// 🔴 **AC-2, probe M1 — the discriminating one.**
///
/// The tag inside the body is changed and the identity is recomputed over the changed bytes, so the
/// file is internally consistent and the identity check passes. Only the witness comparison can
/// refuse it. Delete that comparison from `gxfile` and this test goes red: it is the mutation the
/// remedy is measured by, and the reason the remedy is not decoration.
#[test]
fn a_body_whose_in_band_tag_was_changed_is_refused_although_its_identity_matches() {
    let mut forged = a_board();
    forged.gx_kind = "gx.receipt.v1".to_string();
    let body = gx_canon::cbor::encode(&forged).expect("the forgery is still canonical");
    let bytes = hand_built(GxKind::DesignToken, &body);

    // The identity is honest: this is not a corrupted file, and nothing else would notice.
    let claimed = gxfile::body_cid(&body).expect("canonical");
    assert_eq!(&bytes[6..HEADER_LEN], &claimed.0);

    match gxfile::read(&bytes) {
        Err(Refusal::KindTag { expected, found }) => {
            assert_eq!(expected, DESIGN_TOKEN_TAG);
            assert_eq!(found, "gx.receipt.v1");
        }
        other => panic!("a forged kind tag was not refused by name: {other:?}"),
    }
}

/// 🔴 **AC-2, probe M2** — a body under the number of the wrong kind, in both directions.
#[test]
fn a_body_under_the_wrong_kind_number_is_refused_in_both_directions() {
    let body = gx_canon::cbor::encode(&a_board()).expect("canonical");
    let mut as_receipt = hand_built(GxKind::DesignToken, &body);
    as_receipt[4..6].copy_from_slice(&GxKind::Receipt.code().to_be_bytes());
    assert!(
        gxfile::read(&as_receipt).is_err(),
        "a design token read as a receipt was admitted"
    );

    let envelope_shaped = br#"{"payloadType":"application/vnd.glovrex.receipt+dagcbor","payload":"","signatures":[]}"#;
    let as_design_token = framed(GxKind::DesignToken, &[0u8; 32], envelope_shaped);
    assert!(
        matches!(gxfile::read(&as_design_token), Err(Refusal::Body { .. })),
        "a JSON envelope under kind 15 was not refused as an undecodable body"
    );
}

/// 🔴 **AC-2, probe M3** — canonical bytes that decode as something, but not as this kind.
#[test]
fn canonical_bytes_that_are_not_a_design_token_are_refused_rather_than_read_as_an_empty_one() {
    let body = gx_canon::cbor::encode(&vec![1u8, 2, 3]).expect("canonical");
    let bytes = hand_built(GxKind::DesignToken, &body);
    assert!(matches!(gxfile::read(&bytes), Err(Refusal::Body { .. })));
}

/// 🔴 **§2-F-1** — the residual, pinned rather than described.
///
/// The remedy makes the collision unreachable **for shipped kinds** (equal bodies carry an equal
/// tag, so they are the same kind). It does not change what a `Cid` is. Here the same bytes are
/// framed twice, the identities are equal, and the file layer is what refuses the second one.
#[test]
fn the_identity_still_does_not_name_the_kind_and_the_file_layer_is_what_closes_it() {
    let body = gxfile::write_design_token(&a_board()).expect("writable");
    let document = &body[HEADER_LEN..];
    let as_ledger_leaf = hand_built(GxKind::LedgerLeaf, document);

    assert_eq!(
        &body[6..HEADER_LEN],
        &as_ledger_leaf[6..HEADER_LEN],
        "the identity is over the body alone; if this differs the remedy changed shape"
    );
    assert!(gxfile::read(&body).is_ok());
    assert!(matches!(
        gxfile::read(&as_ledger_leaf),
        Err(Refusal::KindNotShipped { .. })
    ));
}

/// 🔴 **AC-7** — a character moves the identity, and writing the same board twice does not.
///
/// The compiler's two-island claim, lifted to `.gx`: a redesign must not move the text digest. The
/// shell has no representation in this body at all, so that half holds structurally and cannot be
/// measured here; what is measured is the other half, that a character does move it.
#[test]
fn one_character_moves_the_identity_and_the_same_characters_do_not() {
    let first = gxfile::write_design_token(&a_board()).expect("writable");
    let again = gxfile::write_design_token(&a_board()).expect("writable");
    assert_eq!(first, again, "two writes of one board differ");

    let reworded = DesignToken::board(
        "req939.demo",
        vec![
            line("n1.one", "reversibility is a property."),
            line("n1.title", "undo"),
            line("n2.title", "escrow"),
        ],
    )
    .expect("in order");
    let moved = gxfile::write_design_token(&reworded).expect("writable");
    assert_ne!(
        &first[6..HEADER_LEN],
        &moved[6..HEADER_LEN],
        "one character changed and the identity did not"
    );
}

/// 🔴 **AC-4** — `token` and `value` are not spellable.
#[test]
fn the_layer_enumeration_admits_only_intent_and_role() {
    // An exhaustive match: a third arm stops the build here rather than reaching a file.
    let named = |layer: Layer| match layer {
        Layer::Intent => "intent",
        Layer::Role => "role",
    };
    assert_eq!(named(Layer::Intent), "intent");
    assert_eq!(named(Layer::Role), "role");

    let source = include_str!("../src/design_token.rs");
    let arms: Vec<&str> = source
        .lines()
        .skip_while(|l| !l.contains("pub enum Layer"))
        .take_while(|l| !l.starts_with('}'))
        .map(str::trim)
        .filter(|l| *l == "Intent," || *l == "Role," || *l == "Token," || *l == "Value,")
        .collect();
    assert_eq!(
        arms,
        vec!["Intent,", "Role,"],
        "the layer enumeration moved"
    );
}

/// 🔴 **AC-5** — the shell is absent from the schema, checked against the compiler's own list.
///
/// The vocabulary is what `Glovrex_HTML/tools/diagram_design.py`'s `split()` puts in the shell
/// island. **Field declarations** are scanned and not prose: the module header names several of
/// these words on purpose, and a grep over the whole file would be measuring the documentation
/// rather than the schema.
#[test]
fn no_shell_member_is_declared_in_the_body_schema() {
    let source = include_str!("../src/design_token.rs");
    let declared: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix("pub "))
        .filter(|l| l.contains(':'))
        .filter_map(|l| l.split(':').next())
        .collect();
    for member in [
        "size",
        "shape",
        "state",
        "glyph",
        "coord",
        "views",
        "axes",
        "projections",
        "edges",
        "theme",
        "effects",
    ] {
        assert!(
            !declared.contains(&member),
            "{member} is a shell member and it is declared in the body schema"
        );
    }
    // The probe is only meaningful if it can see the members that *are* declared.
    assert!(declared.contains(&"strings"), "the scan found no members");
    assert!(declared.contains(&"parents"));
}

/// 🔴 **AC-6** — an order the writer chose is an order the reader is told.
#[test]
fn a_kernel_out_of_order_is_refused_at_the_file_layer_and_not_silently_sorted() {
    let mut unordered = a_board();
    if let Document::Board(board) = &mut unordered.document {
        board.strings.reverse();
    }
    assert!(
        gxfile::write_design_token(&unordered).is_err(),
        "the writer produced a file its own reader refuses"
    );

    let body = gx_canon::cbor::encode(&unordered).expect("still canonical CBOR");
    let bytes = hand_built(GxKind::DesignToken, &body);
    assert!(
        gxfile::read(&bytes).is_err(),
        "an out-of-order kernel was admitted on the way in"
    );
}

/// Containment is a set, so two spellings of one set are refused.
#[test]
fn parents_out_of_order_are_refused() {
    let declaration = Declaration {
        name: "rail".to_string(),
        layer: Layer::Role,
        meaning: "the persistent left column that carries state".to_string(),
        parents: vec![Cid([2u8; 32]), Cid([1u8; 32])],
        namespace: "layout".to_string(),
    };
    assert!(DesignToken::declaration(declaration).is_err());
}

/// 🔴 The round trip is an identity for this kind too, and a design token is not a receipt.
#[test]
fn a_design_token_survives_the_round_trip_unchanged() {
    let token = a_board();
    let bytes = gxfile::write_design_token(&token).expect("writable");
    let read = gxfile::read(&bytes).expect("what this build wrote, this build reads");

    assert_eq!(read.kind, GxKind::DesignToken);
    assert_eq!(read.format_version, FORMAT_VERSION);
    assert_eq!(read.design_token(), Some(&token));
    assert_eq!(read.receipt(), None, "a design token is not a receipt");
    assert_eq!(
        read.body.kind(),
        read.kind,
        "the body and the header parted"
    );
    assert_eq!(
        read.cid,
        gxfile::body_cid(&bytes[HEADER_LEN..]).expect("canonical")
    );
}

/// 🔴 **AC-8** — the disclosure is exhaustive, and none of it is a verdict about quality.
#[test]
fn the_object_says_what_it_does_not_say() {
    assert_eq!(Unsaid::ALL.len(), 4);
    assert!(Unsaid::ALL.contains(&Unsaid::VisualQuality));
    assert!(Unsaid::ALL.contains(&Unsaid::Join));
    for silence in Unsaid::ALL {
        assert!(!silence.because().is_empty(), "{silence:?} has no reason");
    }
}

/// 🔴 **AC-11** — the measured line (`req/922` §0 principle ②). A measurement, not a gate.
#[test]
fn the_cost_of_one_design_token_round_trip_is_measured_and_printed() {
    let token = a_board();
    let rounds = 1000;

    let write_start = Instant::now();
    let mut bytes = Vec::new();
    for _ in 0..rounds {
        bytes = gxfile::write_design_token(&token).expect("writable");
    }
    let write = write_start.elapsed() / rounds;

    let read_start = Instant::now();
    for _ in 0..rounds {
        gxfile::read(&bytes).expect("readable");
    }
    let read = read_start.elapsed() / rounds;

    println!(
        "R939_DESIGN_TOKEN write_us={:.1} read_us={:.1} bytes={}",
        write.as_secs_f64() * 1e6,
        read.as_secs_f64() * 1e6,
        bytes.len()
    );
}
