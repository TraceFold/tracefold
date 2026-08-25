// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-44-9** (`req/38` §168, `req/187` §5) — the adversarial half of the HTTP read views.
//!
//! `crates/gx-api/tests/wire_census.rs` fixes **which keys** the four answers carry. This file asks
//! the harder question, the one `req/187` §5 asked of the wire in the first place: is each of those
//! keys the thing it is named after, or is it something the server made up that happens to look
//! right? The GUI session's own finding is the standard being held to here — a field named `prev`
//! that held the previous row's **id** where an ancestor shell had held the previous row's **hash**,
//! the same name with different strength, passing every check on both sides.
//!
//! Four attacks, one per addition:
//!
//! 1. **`receipt_view` is the payload and nothing else.** Every member is compared against an
//!    independent decode of `envelope.payload`, and then the world is rewritten underneath it (a
//!    second commit, a grown tree, a moved root) and the view is required not to move. A view
//!    assembled from live state would drift here and nowhere else.
//! 2. **The document survives the rider.** The answer still deserialises as a bare
//!    `gx_witness::Receipt`, because a third party's verifier does exactly that.
//! 3. **`consistent` is an answer, not a constant.** The function behind it is shown refusing a
//!    proof it does not reach, and the two `checked_*` keys are pinned to the proof's own sizes so
//!    they can never become a second account of which trees were compared.
//! 4. **`window_*_at` resolve the sequence numbers they claim to.** Compared against the `at` the
//!    verify answer carried for that very verdict, advanced across two windows, and required to be
//!    `null` — not "the nearest record" — for an empty window.

mod support;

use support::Server;

/// The sorted key set of a JSON object.
fn keys_of(value: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .expect("a JSON object")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

fn id_of(json: &serde_json::Value) -> String {
    json["id"]
        .as_str()
        .or_else(|| json["transformation"].as_str())
        .expect("an id")
        .to_string()
}

/// 44 §0's RFC 3339, spelled the way `crate::rfc3339::of` spells it — the suite's own conversion, so
/// that the assertion below compares two independent renderings of one instant rather than
/// comparing the handler with itself.
fn rfc3339_of(nanos: i64) -> String {
    chrono::DateTime::from_timestamp_nanos(nanos)
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}

/// 🔴 Attack 1 — every member of `receipt_view` is read off the signed payload, and a rewritten
/// world does not move a single one of them.
///
/// The independent decode is `gx_witness::Receipt::payload()` reached from the **wire JSON**, which
/// is the road a stranger takes: deserialise the document, decode the canonical DAG-CBOR, read the
/// fields. If the handler had built the view out of the engine's table or out of the current
/// ledger, the second half of this test would catch it — the second commit grows the tree from one
/// leaf to two, so "the current root" and "the root this receipt's own audit path reconstructs"
/// stop being the same value, and only the second is allowed to be in this object.
#[tokio::test]
async fn the_receipt_view_is_the_signed_payload_and_a_rewritten_world_does_not_move_it() {
    let server = Server::new("dr44_9_view_payload", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();

    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", &actor)),
        )
        .await;
    assert_eq!(created.status.as_u16(), 201, "{}", created.json);
    let id = id_of(&created.json);
    client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    assert_eq!(committed.status.as_u16(), 200, "{}", committed.json);

    let answer = client
        .send("GET", &format!("/v1/receipts/{id}"), None)
        .await;
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.json);
    let view = answer.json["receipt_view"].clone();
    println!("RECEIPT_VIEW={view}");

    // The independent decode: the wire document, typed, then the payload out of the signed bytes.
    let receipt: gx_witness::Receipt =
        serde_json::from_value(answer.json.clone()).expect("the answer is a Receipt");
    let payload = receipt.payload().expect("canonical DAG-CBOR");

    assert_eq!(
        view["subject"],
        serde_json::json!(payload.transformation.0.to_text()),
        "`subject` is the payload's own transformation, not the path segment that was asked for"
    );
    assert_eq!(
        view["key_id"],
        serde_json::to_value(&payload.key_id).expect("a key id"),
        "`key_id` is inside the signature's own bytes — the field that makes a signature moved onto \
         another key's receipt fail to verify"
    );
    assert_eq!(
        view["postcondition_fingerprint"],
        serde_json::to_value(payload.postcondition_fingerprint).expect("a fingerprint"),
        "`postcondition_fingerprint` is base64 of the signed bytes — the spelling M2H1-4 chose for \
         raw bytes everywhere else on this wire, and not a second, hex spelling of the same 32"
    );
    let proof = payload
        .inclusion_proof
        .as_ref()
        .expect("a CommitReceipt carries one (ASM-14)");
    assert_eq!(view["tree_size"], serde_json::json!(proof.tree_size));
    assert_eq!(view["leaf_index"], serde_json::json!(proof.leaf_index));
    assert!(
        view["root"]
            .as_str()
            .is_some_and(|root| root.starts_with("gx1:")),
        "`root` is a Cid in 42 §1.2's one readable spelling, so that it compares by string equality \
         with `GET /ledger/checkpoint`'s `root_hash`: {view}"
    );
    // The instant, twice, in two spellings — and never a third fact.
    assert_eq!(
        view["issued_at"],
        serde_json::json!(rfc3339_of(receipt.issued_at.0)),
        "`receipt_view.issued_at` is 44 §0's RFC 3339 of the same unsigned `Timestamp` the \
         top-level `issued_at` carries as nanoseconds"
    );

    // 🔴 The attack. A second commit appends a second leaf: the ledger's current root moves, the
    // engine's table gains a row, and **none of that may reach a document already signed**.
    let second = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after again\n", &actor)),
        )
        .await;
    let second_id = id_of(&second.json);
    client
        .send("POST", &format!("/v1/candidates/{second_id}/verify"), None)
        .await;
    let second_commit = client
        .send("POST", &format!("/v1/candidates/{second_id}/commit"), None)
        .await;
    assert_eq!(second_commit.status.as_u16(), 200, "{}", second_commit.json);
    let head = client.send("GET", "/v1/ledger/checkpoint", None).await;
    assert_eq!(
        head.json["tree_size"],
        serde_json::json!(2),
        "the world really did move: {}",
        head.json
    );

    let again = client
        .send("GET", &format!("/v1/receipts/{id}"), None)
        .await;
    assert_eq!(
        again.json["receipt_view"], view,
        "a receipt's view is derived from the receipt. The tree grew by a leaf and the head moved; \
         if any member of this object had come from the engine's table or from the current ledger \
         instead of from the signed payload, it would have moved with it"
    );
    assert_ne!(
        head.json["root_hash"], view["root"],
        "and the two roots are now genuinely different values, so the assertion above had \
         something to catch (a discrimination check on the check itself)"
    );
}

/// 🔴 Attack 2 — the rider does not break the document, and the two refusals are censused as
/// absences.
///
/// 44 §2.6 calls a new optional field backward compatible and DSSE's own norm makes readers ignore
/// what they do not recognise; both are claims about somebody else's parser, so this measures the
/// one parser in reach — the crate that mints receipts, reading its own answer back. The SDK's
/// `verifyReceiptOffline` takes the same road (`sdk/typescript/test/e2e.test.mjs` hands it the whole
/// `GET /receipts/{tid}` body), which is why this is not a theoretical courtesy.
#[tokio::test]
async fn the_receipt_answer_still_reads_back_as_a_bare_receipt() {
    let server = Server::new("dr44_9_view_roundtrip", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();
    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", &actor)),
        )
        .await;
    let id = id_of(&created.json);
    client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    let answer = client
        .send("GET", &format!("/v1/receipts/{id}"), None)
        .await;
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.json);

    let receipt: gx_witness::Receipt =
        serde_json::from_value(answer.json.clone()).expect("a reader that ignores the third key");
    let re_encoded = serde_json::to_value(&receipt).expect("a Receipt");
    assert_eq!(
        keys_of(&re_encoded),
        vec!["envelope".to_string(), "issued_at".to_string()],
        "what a `Receipt`-shaped reader sees is exactly what it saw before DR-44-9"
    );
    assert_eq!(
        re_encoded["envelope"], answer.json["envelope"],
        "and the envelope is identical — the view is mounted beside the signed document, not folded \
         into it"
    );

    // 🔴 The two refusals. `verified`/`refuted` belong to `gx_witness::verify_offline`, run where
    // the reader is (`gx receipt verify`, DR-44-4); `alg` is permanently forbidden beside a
    // signature (33 NFR-011's closing note, `req/38` §109/§113 — the algorithm is a property of the
    // key, and `key_id` is how a reader reaches it).
    let view = &answer.json["receipt_view"];
    for forbidden in ["verified", "refuted", "inclusion", "checks", "alg", "valid"] {
        assert!(
            view.get(forbidden).is_none(),
            "`receipt_view.{forbidden}` would be this surface grading its own paper (or, for \
             `alg`, the field NFR-011 closed): {view}"
        );
    }
}

/// 🔴 Attack 3 — `consistent` is `verify_consistency`'s answer, and that function says `false`.
///
/// The `true` arm is measured over HTTP by `wire_census.rs`. Reaching `false` *through* HTTP needs a
/// tile log whose stored leaves and whose cached subtree roots disagree, which this fixture cannot
/// produce honestly — so the discrimination is measured one call in, on the function the handler
/// hands the answer straight out of. **Declared limit**: what is unmeasured here is the wiring
/// between a corrupt log and this endpoint, not whether the value can ever be `false`.
///
/// The second half is the one that would go RED on a refactor: `checked_from`/`checked_to` are the
/// **proof's** sizes, so no reader can ever be told that one pair of trees was compared while
/// another pair was proved.
#[tokio::test]
async fn the_consistency_judgement_discriminates_and_names_the_trees_the_proof_names() {
    let server = Server::new("dr44_9_consistency", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();
    for goal in ["one\n", "two\n"] {
        let created = client
            .send(
                "POST",
                "/v1/candidates",
                Some(server.intent_body(goal, &actor)),
            )
            .await;
        let id = id_of(&created.json);
        client
            .send("POST", &format!("/v1/candidates/{id}/verify"), None)
            .await;
        let committed = client
            .send("POST", &format!("/v1/candidates/{id}/commit"), None)
            .await;
        assert_eq!(committed.status.as_u16(), 200, "{}", committed.json);
    }

    let answer = client
        .send("GET", "/v1/ledger/consistency?from=1&to=2", None)
        .await;
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.json);
    println!("CONSISTENCY_DR44_9={}", answer.json);
    assert_eq!(answer.json["consistent"], serde_json::json!(true));
    assert_eq!(answer.json["checked_from"], answer.json["old_size"]);
    assert_eq!(answer.json["checked_to"], answer.json["new_size"]);
    assert_eq!(answer.json["checked_from"], serde_json::json!(1));
    assert_eq!(answer.json["checked_to"], serde_json::json!(2));

    // The proof the server just handed out, read back as the type gx-log produced, then held
    // against two roots it does not reach.
    let honest: gx_log::proof::ConsistencyProof =
        serde_json::from_value(answer.json.clone()).expect("the answer is a ConsistencyProof");
    let old_root = gx_core::Cid([7u8; 32]);
    let new_root = gx_core::Cid([9u8; 32]);
    assert_eq!(
        gx_log::proof::verify_consistency(&honest, &old_root, &new_root).ok(),
        Some(false),
        "the function behind `consistent` refuses a proof against roots it does not reach — so the \
         `true` above is an answer and not a literal"
    );
}

/// 🔴 Attack 4 — the two resolved boundaries are the verdicts they claim, across two windows, and
/// `null` when there is no verdict to point at.
///
/// The independent value is the `at` the verify answer carried for that verdict: the handler passes
/// one `state.now()` into `Engine::verify`, which is both what the composite reports and what the
/// journal record stores, so the two roads meet at the record. The advance across a second window is
/// what catches an over-counting predicate — a resolution that counted `Planned` records as verdicts
/// would land the second window's start on the wrong record — and the `null` arm is what catches a
/// resolution that nominates the nearest record instead of admitting it has none.
#[tokio::test]
async fn the_verdict_windows_resolve_to_the_verdicts_they_count() {
    let server = Server::new("dr44_9_windows", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();

    let first = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", &actor)),
        )
        .await;
    let first_id = id_of(&first.json);
    let first_verdict = client
        .send("POST", &format!("/v1/candidates/{first_id}/verify"), None)
        .await;
    assert_eq!(first_verdict.status.as_u16(), 200, "{}", first_verdict.json);
    let first_at = first_verdict.json["at"].clone();

    let one = client.send("POST", "/v1/verdict-checkpoints", None).await;
    assert_eq!(one.status.as_u16(), 201, "{}", one.json);
    println!("VC_WINDOW_1={}", one.json);
    assert_eq!(one.json["window_start"], serde_json::json!(0));
    assert_eq!(one.json["window_end"], serde_json::json!(1));
    assert_eq!(
        one.json["window_start_at"], first_at,
        "the window opens at the verdict it counts (`window_start`, inclusive)"
    );
    assert_eq!(
        one.json["window_end_at"], first_at,
        "and closes at the last one it counts — `window_end` is exclusive (42 §3.14), so the \
         resolved time is the record at `window_end - 1` and not one past the end"
    );
    let counted = one.json["tally"]["admit"].as_u64().unwrap_or(0)
        + one.json["tally"]["deny"].as_u64().unwrap_or(0)
        + one.json["tally"]["escalate"].as_u64().unwrap_or(0)
        + one.json["tally"]["unverdicted"].as_u64().unwrap_or(0);
    assert_eq!(
        counted, 1,
        "the counted window and the resolved window are the same window: {}",
        one.json
    );

    // A second verdict, then a second checkpoint: the window is `[1, 2)` and both ends land on the
    // **new** record. An off-by-one in either direction shows up here and nowhere in window one.
    //
    // The first row is committed first because a scope holds one in-flight transformation at a
    // time: a second candidate over the same target while the first is still `Admitted` answers
    // `verdict: null` and `held_by: <the first>`, and no verdict record is written at all. A commit
    // writes no verdict of its own, so the window boundary below is still the verdict's.
    let commit_first = client
        .send("POST", &format!("/v1/candidates/{first_id}/commit"), None)
        .await;
    assert_eq!(commit_first.status.as_u16(), 200, "{}", commit_first.json);
    let second = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after again\n", &actor)),
        )
        .await;
    let second_id = id_of(&second.json);
    let second_verdict = client
        .send("POST", &format!("/v1/candidates/{second_id}/verify"), None)
        .await;
    println!("VC_SECOND_VERDICT={}", second_verdict.json);
    assert_eq!(
        second_verdict.status.as_u16(),
        200,
        "{}",
        second_verdict.json
    );
    let second_at = second_verdict.json["at"].clone();
    let two = client.send("POST", "/v1/verdict-checkpoints", None).await;
    println!("VC_WINDOW_2={}", two.json);
    assert_eq!(two.json["window_start"], serde_json::json!(1));
    assert_eq!(two.json["window_end"], serde_json::json!(2));
    assert_eq!(two.json["window_start_at"], second_at);
    assert_eq!(two.json["window_end_at"], second_at);
    assert_ne!(
        second_at, first_at,
        "the two verdicts really are at different instants, so the assertions above had something \
         to catch"
    );

    // 🔴 The empty window. `window_start == window_end` names no verdict at all, and the honest
    // answer is `null` on both — not the nearest record's clock, which is the confidently wrong
    // derivation `req/187` §5 filed this question about.
    let empty = client.send("POST", "/v1/verdict-checkpoints", None).await;
    println!("VC_WINDOW_EMPTY={}", empty.json);
    assert_eq!(empty.json["window_start"], empty.json["window_end"]);
    assert!(
        empty.json["window_start_at"].is_null() && empty.json["window_end_at"].is_null(),
        "an empty window has no first and no last verdict: {}",
        empty.json
    );

    // The same two keys on the other two roads (the list and the by-coordinate `GET`), because a
    // resolution that existed only on the `201` would be a shape a reader met three times and read
    // once.
    let listed = client.send("GET", "/v1/verdict-checkpoints", None).await;
    let items = listed.json["items"].as_array().expect("items").clone();
    assert_eq!(items.len(), 3, "{}", listed.json);
    assert_eq!(items[0]["window_start_at"], first_at);
    assert_eq!(items[1]["window_end_at"], second_at);
    let by_end = client.send("GET", "/v1/verdict-checkpoints/1", None).await;
    assert_eq!(by_end.status.as_u16(), 200, "{}", by_end.json);
    assert_eq!(by_end.json["window_end_at"], first_at);
}
