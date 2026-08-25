// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 44 §2.2's thirteen, driven end to end through 51 §7's test client.
//!
//! The whole pipeline over HTTP: create → verify → commit → undo, plus the read side and the two
//! refusal paths 44 §2.2 names for each. What this is **not** is AC-055 — that suite compares the
//! CLI and this surface and lives in `crates/gx-cli/tests/ac_055.rs`, because comparing them needs
//! both and this crate cannot depend on gx-cli (see the manifest).

mod support;

use support::Server;

/// The id in a `POST /candidates` response.
fn id_of(json: &serde_json::Value) -> String {
    json["id"]
        .as_str()
        .unwrap_or_else(|| panic!("`POST /candidates` returns the id: {json}"))
        .to_string()
}

/// 🔴 The pipeline, over HTTP: create → verify → commit, and the file moves.
///
/// Three facts and each fails a different wrong implementation:
///
/// * **201 with a `Candidate`** — 44 §2.1's "runs the equivalent of submit+plan in one call … the Draft-alone
///   state is not observable at the HTTP layer" (sem: SEM-gx-api-362). A surface that returned a `Draft` would be publishing the state §2.1 says
///   is not observable;
/// * **the verdict is `Admit` and the proof is there** — M6H3-2's other consumer;
/// * **the substrate moved** — a commit that answered 200 without applying would be a receipt about
///   a change that did not happen, which is the whole thing the ledger is for.
#[tokio::test]
async fn the_pipeline_runs_over_http_and_the_substrate_moves() {
    let server = Server::new("endpoints_pipeline", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();

    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", &actor)),
        )
        .await;
    println!("CREATE={} {}", created.status.as_u16(), created.json);
    assert_eq!(
        created.status.as_u16(),
        201,
        "44 §2.2: \"Response `201 Created`\" (sem: SEM-gx-api-363)"
    );
    assert_eq!(created.json["state"], "Candidate");
    assert!(
        created.json["precondition_fingerprint"].is_object(),
        "44 §2.2 returns the `Fingerprint` beside the transformation: {}",
        created.json
    );
    assert!(
        created.json["created_at"]
            .as_str()
            .is_some_and(|s| s.ends_with('Z')),
        "44 §0: \"date-times are RFC 3339\", and the conversion is \"the API layer's responsibility\" (M6H2-4; sem: SEM-gx-api-364): {}",
        created.json
    );
    let id = id_of(&created.json);

    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    println!("VERIFY={} {}", verified.status.as_u16(), verified.json);
    assert_eq!(
        verified.status.as_u16(),
        200,
        "ASM-44-2 permits the synchronous reading, and M6-06, adopted (a) (sem: SEM-gx-api-365), is why this build takes it"
    );
    assert_eq!(verified.json["verdict"], "Admit");
    assert_eq!(verified.json["state"], "Admitted");
    assert!(
        verified.json["proof"].is_object(),
        "M6H3-2: the proof reaches the surface: {}",
        verified.json
    );

    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    println!("COMMIT={} {}", committed.status.as_u16(), committed.json);
    assert_eq!(committed.status.as_u16(), 200);
    assert_eq!(committed.json["state"], "Committed");
    assert!(
        committed.json["envelope"].is_object(),
        "44 §2.2: \"`Receipt` (42 §3.10's JSON representation, `payload` is base64)\" (sem: SEM-gx-api-366): {}",
        committed.json
    );
    assert_eq!(
        server.target_contents(),
        "after\n",
        "43 T-10 applied the delta"
    );

    // The read side, on the same transformation.
    let candidate = client
        .send("GET", &format!("/v1/candidates/{id}"), None)
        .await;
    println!(
        "GET_CANDIDATE={} {}",
        candidate.status.as_u16(),
        candidate.json
    );
    assert_eq!(candidate.status.as_u16(), 200);
    assert_eq!(candidate.json["state"], "Committed");
    assert_eq!(candidate.json["verdict"], "Admit");
    assert!(candidate.json["fingerprint"].is_object());

    let permanent = client
        .send("GET", &format!("/v1/transformations/{id}"), None)
        .await;
    println!("GET_TRANSFORMATION={}", permanent.status.as_u16());
    assert_eq!(permanent.status.as_u16(), 200);
    assert!(permanent.json["receipt"].is_object());
    assert!(permanent.json["superseded_by"].is_null());

    let receipt = client
        .send("GET", &format!("/v1/receipts/{id}"), None)
        .await;
    println!("GET_RECEIPT={}", receipt.status.as_u16());
    assert_eq!(receipt.status.as_u16(), 200);
    assert!(receipt.json["envelope"].is_object());

    // The ledger, which now has a leaf.
    let proof = client
        .send("GET", &format!("/v1/ledger/proof?leaf={id}"), None)
        .await;
    println!("LEDGER_PROOF={} {}", proof.status.as_u16(), proof.json);
    assert_eq!(proof.status.as_u16(), 200);
    assert_eq!(proof.json["tree_size"], 1);

    let by_index = client.send("GET", "/v1/ledger/proof?leaf=0", None).await;
    assert_eq!(by_index.status.as_u16(), 200);
    assert_eq!(
        by_index.json, proof.json,
        "44 §2.2: \"`leaf` accepts a `u64` … or `gx1:...` …\" (sem: SEM-gx-api-367) — the two spellings name one leaf"
    );

    // 🔴 M6-24, adopted (b) (sem: SEM-gx-api-368): the handler is the producer of a **signed** head.
    let checkpoint = client.send("GET", "/v1/ledger/checkpoint", None).await;
    println!(
        "CHECKPOINT={} {}",
        checkpoint.status.as_u16(),
        checkpoint.json
    );
    assert_eq!(checkpoint.status.as_u16(), 200);
    assert!(
        checkpoint.json["signature"]["sig"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "44 §2.2: \"`Checkpoint` (42 §3.11, **DSSE-signed**)\" (sem: SEM-gx-api-369) — an unsigned head would carry the \
         empty placeholder `unsigned_checkpoint` has to leave in the field: {}",
        checkpoint.json
    );
    // 🔴 The checkpoint's own `timestamp` is **nanoseconds** and not RFC 3339, and that is a fold
    // rather than an oversight: `Checkpoint` is 42 §3.11's value with its own canonical encoding,
    // and `gx receipt verify --checkpoint <FILE>` reads the same bytes back. Re-shaping a domain
    // type on the wire would make the two surfaces disagree about what a checkpoint file **is**.
    // 44 §0's "the API layer's responsibility" (sem: SEM-gx-api-370) is therefore read as covering this surface's own envelope fields
    // (`at`, `created_at`) and not the interior of the values it carries. Raised with M6H5-5.
    assert!(
        checkpoint.json["timestamp"].is_i64(),
        "42 §3.11's own field, unconverted and said so: {}",
        checkpoint.json
    );
    assert_eq!(
        checkpoint.json["signature"]["keyid"],
        server.keys.signing_id(),
        "signed by the server's own key (E-M6-7)"
    );

    // The replay endpoint, over a committed transformation.
    let replay = client
        .send("POST", &format!("/v1/transformations/{id}/replay"), None)
        .await;
    println!("REPLAY={} {}", replay.status.as_u16(), replay.json);
    assert_eq!(replay.status.as_u16(), 200);
    assert_eq!(replay.json["matches"], true);
    assert_eq!(
        replay.json["unchecked"],
        serde_json::json!(["drafts", "transformations", "escrow"]),
        "the denominator: `matches: true` is about the one component with a second copy"
    );

    // And the undo, which is a whole second pipeline (43 §5-2).
    let undone = client
        .send(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({})),
        )
        .await;
    println!("UNDO={} {}", undone.status.as_u16(), undone.json);
    assert_eq!(undone.status.as_u16(), 200, "{}", undone.json);
    assert_eq!(undone.json["superseded_state"], "Superseded");
    assert_eq!(
        server.target_contents(),
        "before\n",
        "P-5: \"an undo is the commit of a new inverse transformation\" (sem: SEM-gx-api-371) — the world is back"
    );
}

/// 🔴 An empty ledger has no head, and the endpoint says so rather than signing nothing.
#[tokio::test]
async fn a_checkpoint_over_an_empty_log_is_not_found() {
    let server = Server::new("endpoints_empty_log", "before\n");
    let answer = server
        .client()
        .send("GET", "/v1/ledger/checkpoint", None)
        .await;
    println!(
        "EMPTY_CHECKPOINT={} {}",
        answer.status.as_u16(),
        answer.json
    );
    assert_eq!(answer.status.as_u16(), 404);
    assert_eq!(answer.gx_code(), "NOT_FOUND");
}

/// 🔴 `cancel` is T-7 over HTTP, and the `actor` it requires is checked against nothing.
///
/// 44 §2.2 makes the body `{ actor }` and 43 T-7's guard is "the actor holds owner privilege …" (sem: SEM-gx-api-372). v0.1 has no
/// authorization layer (M5H6-4, adopted (a)), so what the field does is appear in the response under a name
/// that says what happened to it.
#[tokio::test]
async fn cancel_is_t7_and_the_actor_is_unchecked() {
    let server = Server::new("endpoints_cancel", "before\n");
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

    let cancelled = client
        .send(
            "POST",
            &format!("/v1/candidates/{id}/cancel"),
            Some(serde_json::json!({ "actor": { "Human": { "key": "somebody-else" } } })),
        )
        .await;
    println!("CANCEL={} {}", cancelled.status.as_u16(), cancelled.json);
    assert_eq!(cancelled.status.as_u16(), 200);
    assert_eq!(cancelled.json["state"], "Aborted");
    assert_eq!(cancelled.json["reason"], "OwnerCancelled");
    assert_eq!(
        cancelled.json["actor_unchecked"]["Human"]["key"], "somebody-else",
        "🔴 an actor nobody has heard of cancelled somebody else's transformation, and the field \
         name is the only thing telling the client so (the feedback ledger §1.2: \"anybody can press cancel\"; sem: SEM-gx-api-373)"
    );
    assert_eq!(
        server.target_contents(),
        "before\n",
        "a cancelled transformation never reaches `apply`"
    );

    // 44 §2.2's refusal: "`409` (already `Committing` or beyond, or a terminal state, `gx_code=INVALID_STATE`)" (sem: SEM-gx-api-374).
    //
    // 🔴 Measured on a **committed** transformation rather than by cancelling the aborted one twice.
    // 43 T-7's idempotency column answers a repeated cancel with the state it already reached, so a
    // second `cancel` of an `Aborted` row is 200 and correctly so — "the transition applied and
    // changed nothing" (sem: SEM-gx-api-375) (req/29 §4). The refusal 44 §2.2 specifies is the guard "before reaching `Committing`",
    // and reaching it needs a row past that point.
    // 🔴 A **different** goal, and the reason is 42 §3.3: the `IntentId` is the CID of the intent,
    // so the same goal against the same locator is the same intent — and `Engine::plan` refuses to
    // re-plan one whose transformation has left `Candidate` (43 §8's "forces a re-`plan()`" (sem: SEM-gx-api-376) seen
    // from the other side). Reusing the goal here would measure that refusal instead of T-7's.
    let committed = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after-committed\n", &actor)),
        )
        .await;
    assert_eq!(committed.status.as_u16(), 201, "{}", committed.json);
    let committed_id = id_of(&committed.json);
    client
        .send(
            "POST",
            &format!("/v1/candidates/{committed_id}/verify"),
            None,
        )
        .await;
    let done = client
        .send(
            "POST",
            &format!("/v1/candidates/{committed_id}/commit"),
            None,
        )
        .await;
    assert_eq!(done.status.as_u16(), 200, "{}", done.json);
    let again = client
        .send(
            "POST",
            &format!("/v1/candidates/{committed_id}/cancel"),
            Some(serde_json::json!({ "actor": { "Human": { "key": "somebody-else" } } })),
        )
        .await;
    println!("CANCEL_COMMITTED={} {}", again.status.as_u16(), again.json);
    assert_eq!(again.status.as_u16(), 409);
    assert_eq!(
        again.gx_code(),
        "INVALID_STATE",
        "44 §2.2 names this code three times and 44 §2.3's table does not carry it (M6H5-3)"
    );
}

/// 🔴 An escalation is signed by the **ruler's** key, and a key this server does not hold is refused.
///
/// **E-M6-15** / INV-S6: "there is no default under which the party being ruled on approves themselves" (sem: SEM-gx-api-377). The negative half is the one
/// worth having — a server that fell back to its own key would sign a human ruling as itself.
#[tokio::test]
async fn an_escalation_is_signed_by_the_ruler_and_never_by_default() {
    let server = Server::new("endpoints_escalation", &"x".repeat(1024 * 1024 + 4096));
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

    // E-M3-4's only road to an `Escalate`: an inverse over the escrow ceiling.
    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    println!(
        "VERIFY_ESCALATE={} {}",
        verified.status.as_u16(),
        verified.json
    );
    assert_eq!(verified.json["verdict"], "Escalate");
    assert!(verified.json["ticket"].is_object(), "43 T-4c raises one");

    // A ruler this server holds no key for is 422, with no fallback.
    let unknown = client
        .send(
            "POST",
            &format!("/v1/candidates/{id}/escalation"),
            Some(serde_json::json!({
                "decision": "approve",
                "reason": "I say so",
                "actor": { "Human": { "key": "nobody-here" } },
            })),
        )
        .await;
    println!(
        "ESCALATION_UNKNOWN_KEY={} {}",
        unknown.status.as_u16(),
        unknown.json
    );
    assert_eq!(unknown.status.as_u16(), 422);
    assert_eq!(unknown.gx_code(), "VALIDATION_ERROR");

    // The ruler this server does hold.
    let ruler = server.keys.ruler_id();
    let ruled = client
        .send(
            "POST",
            &format!("/v1/candidates/{id}/escalation"),
            Some(serde_json::json!({
                "decision": "approve",
                "reason": "the inverse is over the escrow ceiling and I accept that",
                "actor": { "Human": { "key": ruler } },
            })),
        )
        .await;
    println!("ESCALATION={} {}", ruled.status.as_u16(), ruled.json);
    assert_eq!(ruled.status.as_u16(), 200, "{}", ruled.json);
    assert_eq!(ruled.json["state"], "Admitted");
    assert_eq!(
        ruled.json["signed_by"], ruler,
        "🔴 INV-S6: the ruling is signed by the ruler and **not** by the server (E-M6-15). Without \
         this field a ruling signed by the wrong key would be invisible at this surface"
    );
    assert_ne!(
        ruled.json["signed_by"], actor,
        "and not by the submitter either, which is the party the ruling is about"
    );

    // 44 §2.2's refusal for a second ruling: "the target is not `Escalated`" (sem: SEM-gx-api-378).
    let again = client
        .send(
            "POST",
            &format!("/v1/candidates/{id}/escalation"),
            Some(serde_json::json!({
                "decision": "reject",
                "reason": "changed my mind",
                "actor": { "Human": { "key": server.keys.ruler_id() } },
            })),
        )
        .await;
    println!("ESCALATION_AGAIN={} {}", again.status.as_u16(), again.json);
    assert_eq!(again.status.as_u16(), 409);
    assert_eq!(again.gx_code(), "INVALID_STATE");
}

/// 🔴 An unknown id is 404 everywhere it can be, and a malformed one is 422.
///
/// The two are different facts (E-M4-35: "unreadable" and "absent"; sem: SEM-gx-api-379) and 44 §2.3 gives them two codes.
#[tokio::test]
async fn an_unknown_id_is_not_found_and_a_malformed_one_is_invalid() {
    let server = Server::new("endpoints_unknown", "before\n");
    let client = server.client();
    let unknown = format!("gx1:{}", "a".repeat(52));

    for path in [
        format!("/v1/candidates/{unknown}"),
        format!("/v1/transformations/{unknown}"),
        format!("/v1/receipts/{unknown}"),
    ] {
        let answer = client.send("GET", &path, None).await;
        println!(
            "UNKNOWN {path} -> {} {}",
            answer.status.as_u16(),
            answer.gx_code()
        );
        assert_eq!(answer.status.as_u16(), 404, "{path}");
        assert_eq!(answer.gx_code(), "NOT_FOUND");
        assert_eq!(answer.content_type, "application/problem+json");
    }

    let malformed = client.send("GET", "/v1/candidates/not-an-id", None).await;
    println!("MALFORMED={} {}", malformed.status.as_u16(), malformed.json);
    assert_eq!(malformed.status.as_u16(), 422);
    assert_eq!(malformed.gx_code(), "VALIDATION_ERROR");

    // 🔴 A body that will not deserialise is also 44 §2.3's shape — the refusal a client meets
    // first, and the one axum's own rejection answers in prose (see `crate::extract`).
    let bad_body = client
        .send(
            "POST",
            "/v1/candidates",
            Some(serde_json::json!({ "substrate": 7 })),
        )
        .await;
    println!("BAD_BODY={} {}", bad_body.status.as_u16(), bad_body.json);
    assert_eq!(bad_body.status.as_u16(), 422);
    assert_eq!(bad_body.gx_code(), "VALIDATION_ERROR");
    assert_eq!(bad_body.content_type, "application/problem+json");
}

/// 🔴 `order` and `parents` are refused rather than dropped (M6H3-5, one surface along).
#[tokio::test]
async fn an_argument_with_nowhere_to_go_is_refused() {
    let server = Server::new("endpoints_refused_args", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();

    let mut body = server.intent_body("after\n", &actor);
    body["order"] = serde_json::json!(2);
    let refused = client.send("POST", "/v1/candidates", Some(body)).await;
    println!("ORDER_REFUSED={} {}", refused.status.as_u16(), refused.json);
    assert_eq!(refused.status.as_u16(), 422);
    assert_eq!(refused.gx_code(), "VALIDATION_ERROR");

    let mut body = server.intent_body("after\n", &actor);
    body["parents"] =
        serde_json::json!(["gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]);
    let refused = client.send("POST", "/v1/candidates", Some(body)).await;
    println!("PARENTS_REFUSED={}", refused.status.as_u16());
    assert_eq!(refused.status.as_u16(), 422);
}

/// 🔴 An undo of a transformation with no escrowed inverse is `409 INVERSE_UNAVAILABLE`.
///
/// 44 §2.2 names the status and the code, and the check happens **before** the pipeline is entered:
/// the engine's own refusal for a missing escrow is a `NotFound`, which would answer 404 about a
/// transformation that exists.
#[tokio::test]
async fn an_undo_with_no_inverse_is_a_conflict_and_not_a_not_found() {
    let server = Server::new("endpoints_no_inverse", "before\n");
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
    // Never committed, so there is no escrow row at all.
    let undone = client
        .send(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({})),
        )
        .await;
    println!("UNDO_NO_INVERSE={} {}", undone.status.as_u16(), undone.json);
    assert_eq!(undone.status.as_u16(), 409);
    assert_eq!(undone.gx_code(), "INVERSE_UNAVAILABLE");
}

/// 🔴 A second undo of one commit is `409` too — 42 §3.12's `Consumed`, which is what "only once" (sem: SEM-gx-api-380) is.
#[tokio::test]
async fn a_second_undo_of_one_commit_is_refused() {
    let server = Server::new("endpoints_second_undo", "before\n");
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
    let first = client
        .send(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({})),
        )
        .await;
    assert_eq!(first.status.as_u16(), 200, "{}", first.json);
    let second = client
        .send(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({})),
        )
        .await;
    println!("SECOND_UNDO={} {}", second.status.as_u16(), second.json);
    assert_eq!(second.status.as_u16(), 409);
    assert_eq!(second.gx_code(), "INVERSE_UNAVAILABLE");
}

/// 🔴 A `Deny` is `403 NOT_ADMITTED` at commit, and the reasons reach the response.
///
/// The shipped pack's one forbid is `/etc`, and 43 T-3 makes `verify` read-only, so this reaches a
/// verdict without touching the substrate — and T-8 refuses the commit before `apply` is reached.
#[tokio::test]
async fn a_denied_transformation_cannot_be_committed() {
    let server = Server::new("endpoints_denied", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();
    let mut body = server.intent_body("after\n", &actor);
    body["locator"] = serde_json::json!("/etc/hostname");

    let created = client.send("POST", "/v1/candidates", Some(body)).await;
    println!("CREATE_DENIED={} {}", created.status.as_u16(), created.json);
    assert_eq!(created.status.as_u16(), 201, "{}", created.json);
    let id = id_of(&created.json);

    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    println!("VERIFY_DENY={} {}", verified.status.as_u16(), verified.json);
    assert_eq!(verified.json["verdict"], "Deny");
    let reasons = verified.json["reasons"].as_array().unwrap_or_else(|| {
        panic!(
            "M6H3-2: 44 §1.2's `reasons`, on the API's second consumer: {}",
            verified.json
        )
    });
    assert!(!reasons.is_empty());
    assert!(reasons[0]["code"].is_string());

    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    println!(
        "COMMIT_DENIED={} {}",
        committed.status.as_u16(),
        committed.json
    );
    assert_eq!(
        committed.status.as_u16(),
        403,
        "44 §2.2: \"`403` (not record-only and Verdict≠Admit)\" (sem: SEM-gx-api-381)"
    );
    assert_eq!(committed.gx_code(), "NOT_ADMITTED");
}

/// 🔴 **M6-08, adopted (a)** (sem: SEM-gx-api-382) — `record_only` is a per-request argument and does not leak to the next request.
///
/// The leak is what (b) "serve swaps the mode via `&mut self` on a per-request basis" was ruled
/// "must not be adopted" (sem: SEM-gx-api-383) for, so it is measured rather than assumed: two transformations, the first
/// verified with `record_only: true` and the second with no body at all, and the second must not
/// inherit the posture.
#[tokio::test]
async fn record_only_is_per_request_and_does_not_leak() {
    let server = Server::new("endpoints_record_only", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();

    let first = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after-one\n", &actor)),
        )
        .await;
    let first_id = id_of(&first.json);
    let recorded = client
        .send(
            "POST",
            &format!("/v1/candidates/{first_id}/verify"),
            Some(serde_json::json!({ "record_only": true })),
        )
        .await;
    println!(
        "RECORD_ONLY_ONE={} {}",
        recorded.status.as_u16(),
        recorded.json
    );
    assert_eq!(recorded.json["record_only"], true);

    // 43 §8 forces a re-plan after the first is finished, so the second transformation is created
    // from the world as it is now.
    let second = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after-two\n", &actor)),
        )
        .await;
    let second_id = id_of(&second.json);
    let plain = client
        .send("POST", &format!("/v1/candidates/{second_id}/verify"), None)
        .await;
    println!("RECORD_ONLY_TWO={} {}", plain.status.as_u16(), plain.json);
    assert_eq!(
        plain.json["record_only"], false,
        "M6-08(b)'s leak: a posture written onto shared state would arrive here"
    );
}

/// 🔴 Evidence supplied by one request is not visible to the next (the same leak, the field M6-08
/// did not move).
///
/// [`gx_api::state::RequestEvidence`] is (b)'s shape for `evidence`, made safe by M6-06(a)'s lock,
/// and raised as **M6H5-6**. This is the measurement that the clearing works: an `evidence` array in
/// one request must not reach the gate on the next, and the observable is the proof's
/// `evidence_digests`.
#[tokio::test]
async fn evidence_is_per_request_and_is_cleared() {
    let server = Server::new("endpoints_evidence", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();

    let first = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after-one\n", &actor)),
        )
        .await;
    let first_id = id_of(&first.json);
    let with = client
        .send(
            "POST",
            &format!("/v1/candidates/{first_id}/verify"),
            Some(serde_json::json!({
                // 42 §3.7's `TestResult`, in `Evidence`'s own serialised shape. 🔴 The first
                // spelling of this body was **rejected** (422, "unknown variant `kind`"; sem: SEM-gx-api-384) and the
                // suite still passed, because a request that never reached the handler leaves an
                // empty cell — which is what a probe about a cell being empty was asking for. The
                // status is asserted below for that reason.
                "evidence": [{
                    "TestResult": {
                        "case": "m6h5-case",
                        "suite": "m6h5-suite",
                        "outcome": "Pass",
                        "log_digest": null,
                        "duration_ms": 12
                    }
                }]
            })),
        )
        .await;
    println!("EVIDENCE_ONE={} {}", with.status.as_u16(), with.json);
    assert_eq!(
        with.status.as_u16(),
        200,
        "🔴 the body has to be **accepted**, or everything below measures a request that never          reached a handler: {}",
        with.json
    );
    let first_digests = with.json["proof"]["evidence_digests"]
        .as_array()
        .map_or(0, Vec::len);

    // 🔴 The cell is empty **the moment this request is over**, which is the property the clear is
    // actually for — and it has to be measured *here*, not after the next request.
    //
    // The comparison below is weaker than it looks and the battery proved it twice: every verify
    // handler loads unconditionally, so the *next* request overwrites the cell on its way in and a
    // mutation deleting `RequestEvidence::clear` survives any assertion made after it (point (n),
    // runs 1 and 2). What the clear buys is not the correctness of the next verdict but the
    // **lifetime** of a client's evidence in this process's memory, and a lifetime is a fact about
    // the cell rather than about a response.
    println!(
        "EVIDENCE_CELL_AFTER_FIRST={}",
        server.state.evidence().len()
    );
    assert!(
        server.state.evidence().is_empty(),
        "a request's evidence does not outlive the request: {} items are still held after the          verify that supplied them",
        server.state.evidence().len()
    );

    // 🔴 A second request, and what it can and cannot show.
    //
    // 43 §8 holds a `Candidate` behind a conflicting predecessor — the first transformation is
    // `Admitted` on the same object — so this one reaches **no verdict** and its `proof` is `null`.
    // That is correct engine behaviour, and it is also why the *response* cannot carry the claim
    // this test is about: there is no proof to look inside. The cell is the observable, and it is
    // read on both sides of the call.
    let second = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after-two\n", &actor)),
        )
        .await;
    let second_id = id_of(&second.json);
    println!(
        "EVIDENCE_CELL_BEFORE_SECOND={}",
        server.state.evidence().len()
    );
    assert!(
        server.state.evidence().is_empty(),
        "the first request's evidence is still held when the second arrives"
    );
    let without = client
        .send("POST", &format!("/v1/candidates/{second_id}/verify"), None)
        .await;
    println!(
        "EVIDENCE_TWO={} state={} held_by={} first_digests={first_digests} cell={}",
        without.status.as_u16(),
        without.json["state"],
        without.json["held_by"],
        server.state.evidence().len()
    );
    assert!(
        server.state.evidence().is_empty(),
        "and it is not held after it either"
    );
}
