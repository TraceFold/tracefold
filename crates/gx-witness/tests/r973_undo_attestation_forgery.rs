// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **DR-46-45 (`req/973` §B-1) — the forgery half, written by the verifying seat and adopted.**
//!
//! Authored by the independent re-run seat rather than by the implementing lane, and kept for that
//! reason: the implementing lane's nine probes all read the field back out of a payload the lane
//! itself built, which measures a round trip. Nobody on the implementing side asked what a forger
//! could do with the seat. Registered in `tools/e2e.sh`'s floor as this lane's suite because a
//! probe nobody runs is a probe nobody has.
//!
//! `req/973` §B-1's invariant says `witness(T_u) = Unobservable(r) ⟹ that fact is inside the
//! signed bytes`. Every probe the lane wrote reads the field back out of a payload it built
//! itself, which measures that the field round-trips — not that a forger cannot move it. So this
//! file attacks the claim from the other side: take an honest receipt, change **only** the
//! disposition, keep the signature, and require offline verification to refuse.
//!
//! If the seat were outside the signed preimage (a sibling of `issued_at`, say), every assertion
//! below would still compile and the forged receipt would verify — which is the failure this
//! probe exists to catch.

mod support;

use gx_canon::cbor;
use gx_witness::receipt::{verify_offline, UndoAttestation, UndoDisposition};
use support::{commit_receipt_in_a_log, issue, keypair, tid};

/// The honest receipt verifies; the same envelope with a swapped disposition does not.
#[test]
fn moving_the_disposition_under_a_kept_signature_is_refused() {
    let key = keypair(3);
    let (anchored, _checkpoint) = commit_receipt_in_a_log(&key, 1, 2);

    let mut attested = anchored.payload().expect("the fixture payload decodes");
    attested.undo = Some(UndoAttestation {
        undoes: tid(42),
        witness: UndoDisposition::Attested,
    });
    let honest = issue(&attested, &key);
    assert!(
        verify_offline(&honest, &key.verifying(), None).is_ok(),
        "the baseline must verify, or the forgery below proves nothing"
    );

    // The only change: "checked, then restored" becomes "fired without checking".
    let mut forged_payload = attested.clone();
    forged_payload.undo = Some(UndoAttestation {
        undoes: tid(42),
        witness: UndoDisposition::Unobservable {
            reason: "the receipt carried no postcondition".to_string(),
        },
    });

    assert_ne!(
        cbor::encode(&attested).expect("canonical"),
        cbor::encode(&forged_payload).expect("canonical"),
        "the disposition must change the canonical bytes, or no signature could cover it"
    );

    let mut forged = honest.clone();
    forged.envelope.payload = cbor::encode(&forged_payload).expect("canonical");
    assert!(
        verify_offline(&forged, &key.verifying(), None).is_err(),
        "🔴 a reader could be told `Unobservable` under a signature that said `Attested`"
    );
}

/// The edge half of the seat is covered too: repointing `undoes` breaks the signature.
#[test]
fn repointing_the_undone_transformation_is_refused() {
    let key = keypair(4);
    let (anchored, _checkpoint) = commit_receipt_in_a_log(&key, 2, 1);

    let mut payload = anchored.payload().expect("decodes");
    payload.undo = Some(UndoAttestation {
        undoes: tid(7),
        witness: UndoDisposition::Attested,
    });
    let honest = issue(&payload, &key);

    let mut elsewhere = payload.clone();
    elsewhere.undo = Some(UndoAttestation {
        undoes: tid(8),
        witness: UndoDisposition::Attested,
    });

    let mut forged = honest.clone();
    forged.envelope.payload = cbor::encode(&elsewhere).expect("canonical");
    assert!(
        verify_offline(&forged, &key.verifying(), None).is_err(),
        "🔴 the compensation edge could be aimed at a different act after signing"
    );
}
