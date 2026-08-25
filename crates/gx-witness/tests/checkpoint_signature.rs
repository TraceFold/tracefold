// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! H3-7 — `Checkpoint.signature` finally holds a signature. (sem: SEM-gx-witness-187,
//! SEM-gx-witness-188, SEM-gx-witness-189, SEM-gx-witness-190, SEM-gx-witness-191, SEM-gx-witness-192)
//!
//! `req/38_ERRATA_2026-08-07.md` §12 verbatim: "**H3-6/H3-7**... H3-7 (`Checkpoint.signature` is
//! non-Option) → resolved in hand 5: attaching the signature is hand 5's scope. Loaded onto the
//! hand 5 lane as "confirm the interim unsigned-generation shape, and close H3-7 once the signature
//! is attached"." Both halves are here: `the_interim_form_is_what_
//! hand_3_left` reads the unsigned checkpoint hand 3 produces and states what is wrong with it, and
//! everything after that is the signature being attached and checked.
//!
//! # What is signed: E-M2-19's three fields
//!
//! `req/38_ERRATA_2026-08-07.md` §9: "`Checkpoint`'s **signed core = {origin, tree_size, root_hash}**,
//! and `timestamp` goes outside the signed core (an unsigned advisory field) — exactly the same
//! shape as E-M2-6 (CM-5: a clock-free signed payload)". `gx_log::proof::checkpoint_signing_bytes` is that byte string,
//! built by hand 2 and signed by nobody until now.
//!
//! The consequence is stated rather than hidden: a verified checkpoint says *this key stated this
//! root at this size for this origin*, and says **nothing** about when. `a_verified_checkpoint_
//! says_nothing_about_when` is the measurement.
//!
//! # The asymmetry hand 5 did not resolve, and the erratum that did (**E-M2-26**)
//!
//! Hand 5 left this: a receipt's signature is over a PAE (`dsse.rs`), a checkpoint's was over the
//! canonical core bytes directly, because E-M2-19 fixes those bytes and 42 §3.11 gives a checkpoint
//! no `payload_type` to put in a PAE. One key signed two byte formats with nothing between them
//! saying which is which, and `the_two_signing_roads_do_not_share_a_message` could only measure that
//! they did not collide for the values at hand -- which is not a domain separation. req/54 §4 raised
//! it as H5-4.
//!
//! `req/38_ERRATA_2026-08-07.md` §15 decided it: the checkpoint gets a `payload_type` (minted by the
//! erratum, since no canonical source has one) and its core rides inside a pre-authentication encoding like a
//! receipt's payload. The separation is now the length-prefixed type inside the signed bytes. Three
//! tests below hold the new state -- `a_signature_over_the_bare_core_is_refused` (written red, before
//! the change), `the_checkpoint_signature_is_taken_over_a_pae`, and
//! `the_two_roads_are_separated_by_payload_type` -- and the old measurement is kept beside them,
//! because what it says is still true and is now true for a reason somebody chose.
//!
//! gx claims **no wire compatibility with the C2SP checkpoint / signed-note text form**; see
//! `dsse.rs`'s crate note for the ruling and `the_checkpoint_message_is_not_a_c2sp_signed_note`
//! for the mechanical form of the non-claim.

mod support;

use gx_core::{Checkpoint, Cid, DsseSignature, Timestamp, VerdictKind};
use gx_log::{proof, TileLog};
use gx_witness::dsse::{
    checkpoint_signing_message, pae, sign_checkpoint, verify_checkpoint, CHECKPOINT_PAYLOAD_TYPE,
    RECEIPT_PAYLOAD_TYPE,
};
use gx_witness::Error;
use support::{cid, keypair, tid};

fn a_log(leaves: u64) -> TileLog {
    let mut log = TileLog::new();
    for i in 0..leaves {
        log.append(tid(700_000 + i), cid(710_000 + i), Timestamp(i as i64))
            .expect("canonical");
    }
    log
}

fn a_head(leaves: u64) -> Checkpoint {
    proof::unsigned_checkpoint(&a_log(leaves), "glovrex-ledger/v1", Timestamp(99))
        .expect("a non-empty log has a head")
}

// ---------------------------------------------------------------------------
// The interim form, and what closing H3-7 changes
// ---------------------------------------------------------------------------

/// What hand 3 leaves behind: a checkpoint whose `signature` is an empty placeholder.
///
/// 42 §3.11 makes the field non-optional, so `unsigned_checkpoint` has to put *something* there,
/// and hand 3 chose empty over a zero-filled 64 bytes on the grounds that empty is not a signature.
/// This states the claim as a check: the interim value is refused by anything that verifies one.
#[test]
fn the_interim_form_is_what_hand_3_left_and_verifies_nowhere() {
    let key = keypair(1);
    let unsigned = a_head(5);

    assert!(unsigned.signature.sig.is_empty());
    assert!(unsigned.signature.keyid.is_empty());
    assert!(matches!(
        verify_checkpoint(&unsigned, &key.verifying()),
        Err(Error::SignatureInvalid { .. })
    ));
}

/// H3-7 closed: signed, and verifying.
#[test]
fn a_signed_checkpoint_verifies() {
    let key = keypair(2);
    let signed = sign_checkpoint(&a_head(5), key.signing_key(), key.key_id()).expect("signable");

    assert_eq!(signed.signature.keyid, *key.key_id());
    assert_eq!(signed.signature.sig.len(), 64);
    verify_checkpoint(&signed, &key.verifying()).expect("a signed checkpoint verifies");
}

/// Signing changes nothing but the signature. The three signed fields and the advisory timestamp
/// come back exactly as they went in, so a caller cannot be handed a head that says something else.
#[test]
fn signing_changes_only_the_signature() {
    let key = keypair(3);
    let unsigned = a_head(9);
    let signed = sign_checkpoint(&unsigned, key.signing_key(), key.key_id()).expect("signable");

    assert_eq!(signed.origin, unsigned.origin);
    assert_eq!(signed.tree_size, unsigned.tree_size);
    assert_eq!(signed.root_hash, unsigned.root_hash);
    assert_eq!(signed.timestamp, unsigned.timestamp);
    assert_ne!(signed.signature, unsigned.signature);
}

// ---------------------------------------------------------------------------
// E-M2-19: what the signature covers, and what it does not
// ---------------------------------------------------------------------------

/// Each of the three signed fields, changed one at a time, breaks the signature.
#[test]
fn each_of_the_three_signed_fields_is_covered() {
    let key = keypair(4);
    let signed = sign_checkpoint(&a_head(6), key.signing_key(), key.key_id()).expect("signable");

    let tampered = [
        (
            "origin",
            Checkpoint {
                origin: "another-ledger/v1".to_string(),
                ..signed.clone()
            },
        ),
        (
            "tree_size",
            Checkpoint {
                tree_size: signed.tree_size + 1,
                ..signed.clone()
            },
        ),
        (
            "root_hash",
            Checkpoint {
                root_hash: Cid([0x5a; 32]),
                ..signed.clone()
            },
        ),
    ];
    for (field, head) in tampered {
        assert!(
            matches!(
                verify_checkpoint(&head, &key.verifying()),
                Err(Error::SignatureInvalid { .. })
            ),
            "{field} is not covered by the checkpoint signature"
        );
    }
}

/// The timestamp is **not** covered (E-M2-19, CM-5), and this is where that stops being a claim in
/// a doc comment. A checkpoint whose clock was rewritten still verifies -- so a verifier that
/// treated a verified head as evidence of *when* the log said it would be wrong, and the type
/// system cannot say so on its own.
#[test]
fn a_verified_checkpoint_says_nothing_about_when() {
    let key = keypair(5);
    let signed = sign_checkpoint(&a_head(6), key.signing_key(), key.key_id()).expect("signable");
    let rewound = Checkpoint {
        timestamp: Timestamp(0),
        ..signed.clone()
    };

    verify_checkpoint(&rewound, &key.verifying())
        .expect("the clock is outside the signed core (E-M2-19)");
    println!("CHECKPOINT_SIGNED_FIELDS=origin,tree_size,root_hash CLOCK_COVERED=no");
}

/// Two heads of the same tree at different clocks share their signed bytes exactly. The same
/// property `ac_070`'s digest test states for receipts, one layer down.
#[test]
fn the_signing_bytes_do_not_depend_on_the_clock() {
    let log = a_log(7);
    let early = proof::unsigned_checkpoint(&log, "glovrex-ledger/v1", Timestamp(1)).expect("head");
    let late = proof::unsigned_checkpoint(&log, "glovrex-ledger/v1", Timestamp(2)).expect("head");

    assert_eq!(
        proof::checkpoint_signing_bytes(&early).expect("canonical"),
        proof::checkpoint_signing_bytes(&late).expect("canonical")
    );
}

/// A head of a different tree has different signed bytes, so the previous test is not the trivial
/// observation that the function ignores its argument.
#[test]
fn a_different_tree_has_different_signing_bytes() {
    let a = a_head(4);
    let b = a_head(5);
    assert_ne!(
        proof::checkpoint_signing_bytes(&a).expect("canonical"),
        proof::checkpoint_signing_bytes(&b).expect("canonical")
    );
}

// ---------------------------------------------------------------------------
// The keys, and the two roads
// ---------------------------------------------------------------------------

/// Another key's signature does not verify, and neither does a signature offered under another id.
///
/// 45 ASM-45-1 keeps v0.1 to a single tier, so the ledger key and the receipt key may well be the
/// same one; the type does not decide it and this makes sure the check does not assume it.
#[test]
fn a_checkpoint_signed_by_another_key_is_refused() {
    let ledger = keypair(6);
    let stranger = keypair(7);
    let signed =
        sign_checkpoint(&a_head(3), ledger.signing_key(), ledger.key_id()).expect("signable");

    assert!(matches!(
        verify_checkpoint(&signed, &stranger.verifying()),
        Err(Error::SignatureInvalid { .. })
    ));

    let relabelled = Checkpoint {
        signature: DsseSignature {
            keyid: stranger.key_id().clone(),
            ..signed.signature.clone()
        },
        ..signed
    };
    assert!(matches!(
        verify_checkpoint(&relabelled, &stranger.verifying()),
        Err(Error::SignatureInvalid { .. })
    ));
}

/// Every bit of a checkpoint signature, swept.
#[test]
fn every_bit_of_a_checkpoint_signature_is_caught() {
    let key = keypair(8);
    let signed = sign_checkpoint(&a_head(4), key.signing_key(), key.key_id()).expect("signable");
    let original = signed.signature.sig.clone();

    for position in 0..original.len() * 8 {
        let mut sig = original.clone();
        sig[position / 8] ^= 1 << (position % 8);
        let tampered = Checkpoint {
            signature: DsseSignature {
                sig,
                ..signed.signature.clone()
            },
            ..signed.clone()
        };
        assert!(
            matches!(
                verify_checkpoint(&tampered, &key.verifying()),
                Err(Error::SignatureInvalid { .. })
            ),
            "checkpoint signature bit {position} was accepted"
        );
    }
    println!("CHECKPOINT_SIGNATURE_BITS={}", original.len() * 8);
}

/// The checkpoint **core** is not a pre-authentication encoding, and a receipt's signed bytes are.
///
/// Kept from hand 5, where it was the whole of the separation between the two roads: a PAE opens
/// with `DSSEv1` and a canonical CBOR map opens with a major-type-5 header, so the two byte strings
/// could not collide -- a fact about two encodings rather than a domain separation anybody chose
/// (H5-4, req/54 §4). E-M2-26 made the separation structural and this became the smaller statement
/// it always was: the core is a three-entry CBOR map, and it is what goes *inside* the encoding that
/// `the_checkpoint_signature_is_taken_over_a_pae` pins.
#[test]
fn the_two_signing_roads_do_not_share_a_message() {
    let key = keypair(9);
    let head = a_head(3);
    let core = proof::checkpoint_signing_bytes(&head).expect("canonical");
    let receipt = support::issue(&support::verdict_payload(VerdictKind::Admit, &key, 1), &key);
    let envelope = receipt.envelope.signing_bytes();

    assert_ne!(core, envelope);
    assert!(envelope.starts_with(b"DSSEv1"));
    assert!(!core.starts_with(b"DSSEv1"));
    // The checkpoint core is a CBOR map: major type 5, and a three-entry one at that.
    assert_eq!(core[0] >> 5, 5, "the checkpoint core is not a CBOR map");
    assert_eq!(core[0] & 0x1f, 3, "the signed core is not three fields");
}

// ---------------------------------------------------------------------------
// E-M2-26: the checkpoint road is a DSSE road too
// ---------------------------------------------------------------------------

/// A signature over the **bare** core -- the road hand 5 took -- does not verify.
///
/// `req/38_ERRATA_2026-08-07.md` §15 verbatim: "**E-M2-26** (H5-4's resolution, to the fix batch): a
/// checkpoint signature **is given a payload_type and carried in a PAE** (a 42 §3.11 erratum).
/// Grounds: ① unify signing discipline onto DSSE alone (the current state, where the same key
/// signs two byte formats directly, is "accidental non-collision", not a design = H5-4)".
///
/// This is the measurement of that ruling, and it is written before the change it describes: until
/// `sign_checkpoint` wraps the core in a pre-authentication encoding, the signature produced here by
/// hand -- Ed25519 over `checkpoint_signing_bytes` and nothing else -- is exactly what hand 5's
/// `sign_checkpoint` produced, and `verify_checkpoint` accepts it. Afterwards the two roads sign two
/// different messages and this one is refused, which is what "accidental non-collision" becoming a design means.
#[test]
fn a_signature_over_the_bare_core_is_refused() {
    use ed25519_dalek::{Signature, Signer};

    let key = keypair(10);
    let head = a_head(5);
    let core = proof::checkpoint_signing_bytes(&head).expect("canonical");
    let raw: Signature = key.signing_key().sign(&core);

    let signed_the_old_way = Checkpoint {
        signature: DsseSignature {
            keyid: key.key_id().clone(),
            sig: raw.to_bytes().to_vec(),
        },
        ..head
    };
    assert!(
        matches!(
            verify_checkpoint(&signed_the_old_way, &key.verifying()),
            Err(Error::SignatureInvalid { .. })
        ),
        "a signature taken over the unwrapped core verifies; the checkpoint road is not on DSSE \
         (E-M2-26, req/38 §15)"
    );
}

/// What is signed is `PAE(CHECKPOINT_PAYLOAD_TYPE, core)`, byte for byte.
///
/// The formula is `dsse.rs`'s and `pae_golden.rs` pins it against hand-written vectors; what is
/// checked here is that the checkpoint road uses it and on which two arguments.
#[test]
fn the_checkpoint_signature_is_taken_over_a_pae() {
    let head = a_head(5);
    let core = proof::checkpoint_signing_bytes(&head).expect("canonical");
    let message = checkpoint_signing_message(&head).expect("canonical");

    assert_eq!(message, pae(CHECKPOINT_PAYLOAD_TYPE, &core));
    assert!(message.starts_with(b"DSSEv1 "));
    assert!(
        message.ends_with(&core),
        "the core is what the encoding carries, and it is carried unchanged"
    );
    println!(
        "CHECKPOINT_PAYLOAD_TYPE={CHECKPOINT_PAYLOAD_TYPE} CORE_BYTES={} MESSAGE_BYTES={}",
        core.len(),
        message.len()
    );
}

/// The separation between the two roads is the payload type, and it is inside the signed bytes.
///
/// Two halves. The types differ, and -- the half that matters -- the *same body* under the two types
/// is two different messages, so a signature made on one road cannot be replayed as one made on the
/// other even for a body that both roads could hold.
#[test]
fn the_two_roads_are_separated_by_payload_type() {
    assert_ne!(CHECKPOINT_PAYLOAD_TYPE, RECEIPT_PAYLOAD_TYPE);

    let body: &[u8] = b"\x01\x02\x03";
    assert_ne!(
        pae(CHECKPOINT_PAYLOAD_TYPE, body),
        pae(RECEIPT_PAYLOAD_TYPE, body),
        "one body under two types must not produce one message"
    );

    let head = a_head(4);
    let core = proof::checkpoint_signing_bytes(&head).expect("canonical");
    assert_ne!(
        checkpoint_signing_message(&head).expect("canonical"),
        pae(RECEIPT_PAYLOAD_TYPE, &core),
        "a checkpoint core offered as a receipt payload must not sign the same bytes"
    );
}

/// gx does **not** claim wire compatibility with the C2SP checkpoint / signed-note form.
///
/// `req/38_ERRATA_2026-08-07.md` §15 verbatim: "② the C2SP text form (newline+signed-note) is
/// confirmed structurally unrelated by primary comparison — gx does not claim wire compatibility
/// with a C2SP checkpoint (a conversion layer, if any, is future work)". A non-claim
/// cannot be proved, but its opposite can be refused: the C2SP form is text that opens with the
/// origin line and separates fields with newlines, and gx's message opens with `DSSEv1` and carries
/// binary CBOR. This asserts the difference in the shape `ac_024` uses for the hash algorithm -- a
/// declared difference, so that a change making the two look alike fails here instead of quietly
/// implying an interoperability nobody built.
#[test]
fn the_checkpoint_message_is_not_a_c2sp_signed_note() {
    let head = a_head(6);
    let message = checkpoint_signing_message(&head).expect("canonical");

    // The C2SP note's body, written here from its shape: an origin line, the tree size in ASCII
    // decimal, and the root in base64, each on its own line. Written to be compared against, not
    // to be produced -- nothing in gx emits this.
    let note = format!(
        "{}\n{}\n{}\n",
        head.origin,
        head.tree_size,
        gx_core::b64::encode(&head.root_hash.0)
    );

    assert_ne!(message, note.as_bytes());
    assert!(
        !message.starts_with(head.origin.as_bytes()),
        "the C2SP note opens with the origin line; gx's message opens with the DSSE prefix"
    );
    assert!(
        !note.as_bytes().starts_with(b"DSSEv1 "),
        "the comparison is vacuous unless the two forms really differ at byte 0"
    );
    println!(
        "CHECKPOINT_C2SP_WIRE_COMPATIBLE=no (E-M2-26, req/38 §15; a conversion layer is future work)"
    );
}
