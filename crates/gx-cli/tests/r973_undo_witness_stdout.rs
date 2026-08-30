// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **DR-46-45 (`req/973` §B-1)** — surface parity for the undo's compare-and-swap answer, through
//! the real `gx` binary.
//!
//! # What was missing, measured rather than described
//!
//! HTTP's `/undo` has carried a `witness` word since R3. CLI stdout carried six keys and none of
//! them was it (`req/973` §B-1 read the insertion site and listed them: `transformation`, `undone`,
//! `state`, `superseded_state`, `idempotency_key`, `stored_at`). So an operator piping `gx undo`
//! into a script could not tell "the world was checked against `T_o`'s own signed observation and
//! then restored" from "there was nothing to check against, so the inverse was fired" — the two
//! produced the same six keys with the same values.
//!
//! The word printed here is minted by `gx_engine::UndoWitness::word` and by nothing in `gx-cli`, so
//! this line cannot drift away from the HTTP field or from the `undo.witness` inside the signed
//! payload. That is the parity: one function, three readers.

mod support;

use support::{pipeline, run};

/// 🔴 The word is on stdout, and it is the one the engine mints.
#[test]
fn the_undo_stdout_names_the_witness_it_fired_under() {
    let fixture = pipeline("r973_witness_stdout", "before\n");
    let committed = fixture.commit_one("after\n");

    let undone = run(fixture.gx().args(["undo", &committed]));
    println!(
        "R973_STDOUT exit={} stdout={} stderr={}",
        undone.code,
        undone.stdout.trim(),
        undone.stderr.trim()
    );
    assert_eq!(undone.code, 0, "stderr: {}", undone.stderr);

    let json = undone.json();
    assert_eq!(
        json["witness"], "attested",
        "🔴 `req/973` §B-1: an undo that ran DR-43-1's compare-and-swap says so on stdout. Before \
         this erratum the key did not exist and the two dispositions wore one face"
    );
    // The keys `req/973` §B-1 measured are all still there: this erratum adds one and moves none.
    for key in [
        "transformation",
        "undone",
        "state",
        "superseded_state",
        "idempotency_key",
        "stored_at",
    ] {
        assert!(
            json.get(key).is_some(),
            "the additive change dropped `{key}`: {json}"
        );
    }
}

/// 🔴 And the same word is inside the bytes the key signed, reachable with no live engine.
///
/// `gx receipt show` reads the stored receipt off disk in a fresh process and prints the decoded
/// payload — which is the position `req/973` §B-1 says the third party is in. The assertion is that
/// the attestation is *there*, naming the original, so the DAG §B-2 asks for is reconstructible
/// from receipts alone rather than from a journal the third party does not have.
#[test]
fn the_stored_receipt_carries_the_attestation_a_third_party_can_read() {
    let fixture = pipeline("r973_witness_receipt", "before\n");
    let committed = fixture.commit_one("after\n");

    let undone = run(fixture.gx().args(["undo", &committed]));
    assert_eq!(undone.code, 0, "stderr: {}", undone.stderr);
    let undo_id = undone.json()["transformation"]
        .as_str()
        .expect("the undo names its own id")
        .to_string();

    let shown = run(fixture
        .gx()
        .args(["receipt", "show", &undo_id, "--level", "3"]));
    println!(
        "R973_RECEIPT_SHOW exit={} stdout={} stderr={}",
        shown.code,
        shown.stdout.trim(),
        shown.stderr.trim()
    );
    assert_eq!(shown.code, 0, "stderr: {}", shown.stderr);
    assert!(
        shown.stdout.contains(&committed),
        "🔴 `req/973` §B-2: the undo's receipt names what it undid, so a reader holding receipts \
         and nothing else can build the edge. `req/973` §1-2 measured that this was impossible \
         before — `inverse_delta` collides and the fingerprint chain runs parallel: {}",
        shown.stdout
    );
    assert!(
        shown.stdout.contains("Attested") || shown.stdout.contains("attested"),
        "and it says the comparison ran: {}",
        shown.stdout
    );
}
