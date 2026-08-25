// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 v0.4-l small repairs (`req/189`) — the HTTP half of `req/182`'s H-04 / H-10 / H-11 / M-14 /
//! M-15 and DR-44-1 (a).
//!
//! Every test is a **refusing test**: run before the repair against the coordinates `req/182` §1
//! names (RED, raw lines quoted in `req/189` §3) and after (GREEN). The exact key sets these
//! answers carry are `tests/wire_census.rs`' business; this file measures the **behaviour** the
//! audit named — a ruled row leaving the queue, a replay keeping its `escalated` line, a body
//! refused instead of dropped, four writers folded into one.

mod support;

use support::Server;

fn id_of(json: &serde_json::Value) -> String {
    json["id"]
        .as_str()
        .unwrap_or_else(|| panic!("`POST /candidates` returns the id: {json}"))
        .to_string()
}

async fn next_line(body: &mut axum::body::Body) -> serde_json::Value {
    use http_body_util::BodyExt;
    let frame = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
        .await
        .expect("a line arrived inside the wait")
        .expect("the stream is still open")
        .expect("the frame is readable");
    let bytes = frame.into_data().expect("a data frame");
    serde_json::from_slice(&bytes).expect("each line is one JSON object")
}

/// The whole backlog of `GET /stream`, with the journal as the count's denominator.
async fn backlog(server: &Server) -> Vec<serde_json::Value> {
    let expected: usize = {
        let engine = server.state.engine();
        engine
            .journal()
            .records()
            .iter()
            .map(|r| gx_api::stream::events_for(r, &engine).len())
            .sum()
    };
    let mut body = server.client().open("/v1/stream", &[]).await.into_body();
    let mut lines = Vec::new();
    for _ in 0..expected {
        lines.push(next_line(&mut body).await);
    }
    lines
}

// ---------------------------------------------------------------------------
// H-04
// ---------------------------------------------------------------------------

/// 🔴 H-04: a ruled row leaves `GET /escalations`, and the replayed `escalated` line survives it.
///
/// Before the repair the list filtered on the ticket alone and nothing ever cleared the ticket, so
/// the ruled row stayed in the queue for ever; and the stream decided `escalated` from the live
/// ticket, so a replay after the ruling would have lost the line the moment the ticket was cleared
/// (and after a restart, always). Both halves are measured on one fixture: E-M3-4's escrow ceiling
/// (an inverse over 1 MiB) makes the shipped pack escalate.
#[tokio::test(flavor = "multi_thread")]
async fn h04_a_ruled_escalation_leaves_the_queue_and_keeps_its_stream_line() {
    let server = Server::new("v04l_h04_api", &"x".repeat(1024 * 1024 + 4096));
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
    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    assert_eq!(verified.json["verdict"], "Escalate", "{}", verified.json);

    let waiting = client.send("GET", "/v1/escalations", None).await;
    let waiting_rows = waiting.json["items"].as_array().expect("items").len();
    let ticket_before = backlog(&server)
        .await
        .into_iter()
        .find(|l| l["event"] == "escalated")
        .map(|l| l["data"]["ticket_id"].clone())
        .expect("the fixture escalated, so the backlog carries an `escalated` line");

    let ruled = client
        .send(
            "POST",
            &format!("/v1/candidates/{id}/escalation"),
            Some(serde_json::json!({
                "decision": "approve",
                "reason": "v0.4-l H-04: ruled, so the queue entry is spent",
                "actor": { "Human": { "key": server.keys.ruler_id() } },
            })),
        )
        .await;
    assert_eq!(ruled.status.as_u16(), 200, "{}", ruled.json);

    let drained = client.send("GET", "/v1/escalations", None).await;
    let drained_rows = drained.json["items"].as_array().expect("items").len();
    let lines = backlog(&server).await;
    let escalated_after: Vec<&serde_json::Value> =
        lines.iter().filter(|l| l["event"] == "escalated").collect();
    let human: Vec<&serde_json::Value> = lines
        .iter()
        .filter(|l| l["event"] == "human.decision")
        .collect();
    println!(
        "H04_API waiting_rows={waiting_rows} drained_rows={drained_rows} ticket_before={ticket_before} \
         escalated_after={} human_decision={} human_ticket={}",
        escalated_after.len(),
        human.len(),
        human.first().map_or(serde_json::Value::Null, |l| l["data"]["ticket_id"].clone())
    );
    assert_eq!(waiting_rows, 1, "control: the row is queued while it waits");
    assert_eq!(
        drained_rows, 0,
        "H-04: after the ruling `GET /escalations` no longer lists the row"
    );
    assert_eq!(
        escalated_after.len(),
        1,
        "H-04 (stream): the replayed `escalated` line is decided by the record, not the live ticket"
    );
    assert_eq!(
        escalated_after[0]["data"]["ticket_id"], ticket_before,
        "and it names the same ticket T-4c raised (rebuilt from Σ, `Engine::ticket_as_raised`)"
    );
    assert!(
        ticket_before.is_string(),
        "the ticket id is a real id, not null: {ticket_before}"
    );
    assert_eq!(
        human[0]["data"]["ticket_id"], ticket_before,
        "L-02 (stream.rs:542): `human.decision` names the ticket the ruling resolved"
    );
}

// ---------------------------------------------------------------------------
// H-10 / L-04 / M-14
// ---------------------------------------------------------------------------

/// 🔴 H-10: a body that arrives with no `Content-Type` is refused, not read as "no body".
///
/// `req/182` probe2 P7′: `replay -H 'Content-Type:' -d '{"from":0,"to":1}'` answered
/// `records_replayed: 3` (the whole journal) where the same request with the header answered 1.
/// Same probe here, through the real router: the headerless body is 415 problem+json, and the
/// same bytes with the header replay exactly one record — so the first answer cannot be mistaken
/// for the second any more.
#[tokio::test(flavor = "multi_thread")]
async fn h10_a_body_without_content_type_is_refused_rather_than_read_as_absent() {
    let server = Server::new("v04l_h10", "before\n");
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

    let with_header = client
        .send_raw(
            "POST",
            &format!("/v1/transformations/{id}/replay"),
            &[("content-type", "application/json")],
            br#"{"from":0,"to":1}"#.to_vec(),
        )
        .await;
    let without_header = client
        .send_raw(
            "POST",
            &format!("/v1/transformations/{id}/replay"),
            &[],
            br#"{"from":0,"to":1}"#.to_vec(),
        )
        .await;
    let truly_absent = client
        .send_raw(
            "POST",
            &format!("/v1/transformations/{id}/replay"),
            &[],
            Vec::new(),
        )
        .await;
    println!(
        "H10 with_header={} {} | without_header={} {} | absent={} {}",
        with_header.status,
        with_header.json,
        without_header.status,
        without_header.json,
        truly_absent.status,
        truly_absent.json["records_replayed"]
    );
    assert_eq!(with_header.status.as_u16(), 200);
    assert_eq!(with_header.json["records_replayed"], serde_json::json!(1));
    assert_eq!(
        without_header.status.as_u16(),
        415,
        "H-10: the headerless body is refused (415 UNSUPPORTED_MEDIA_TYPE, req/189 L-04's status)"
    );
    assert_eq!(without_header.gx_code(), "UNSUPPORTED_MEDIA_TYPE");
    assert_eq!(without_header.content_type, "application/problem+json");
    assert_eq!(
        truly_absent.status.as_u16(),
        200,
        "and a genuinely absent body is still 44 §2.2's optional-body road: {}",
        truly_absent.json
    );
    assert!(
        truly_absent.json["records_replayed"].as_u64().unwrap_or(0) > 1,
        "no body = the whole of this transformation's records"
    );
}

/// 🔴 M-14 / L-04: an oversize body is 413, a non-JSON media type is 415 — both problem+json.
#[tokio::test(flavor = "multi_thread")]
async fn m14_l04_oversize_is_413_and_wrong_media_type_is_415() {
    let server = Server::new("v04l_m14", "before\n");
    let client = server.client();
    let too_big = client
        .send_raw(
            "POST",
            "/v1/candidates",
            &[("content-type", "application/json")],
            vec![b' '; gx_api::MAX_BODY_BYTES + 1],
        )
        .await;
    let just_fits = client
        .send_raw(
            "POST",
            "/v1/candidates",
            &[("content-type", "application/json")],
            vec![b' '; gx_api::MAX_BODY_BYTES],
        )
        .await;
    let wrong_type = client
        .send_raw(
            "POST",
            "/v1/candidates",
            &[("content-type", "text/plain")],
            b"{}".to_vec(),
        )
        .await;
    println!(
        "M14 too_big={} {} | just_fits={} {} | wrong_type={} {}",
        too_big.status,
        too_big.gx_code(),
        just_fits.status,
        just_fits.gx_code(),
        wrong_type.status,
        wrong_type.gx_code()
    );
    assert_eq!(too_big.status.as_u16(), 413);
    assert_eq!(too_big.gx_code(), "PAYLOAD_TOO_LARGE");
    assert_eq!(too_big.content_type, "application/problem+json");
    assert_eq!(
        just_fits.status.as_u16(),
        422,
        "exactly MAX_BODY_BYTES is read (and refused as not-JSON, which is the body's own fault)"
    );
    assert_eq!(just_fits.gx_code(), "VALIDATION_ERROR");
    assert_eq!(wrong_type.status.as_u16(), 415);
    assert_eq!(wrong_type.gx_code(), "UNSUPPORTED_MEDIA_TYPE");
    assert_eq!(
        gx_api::MAX_BODY_BYTES,
        2 * 1024 * 1024,
        "44 §2.2: 2 MiB, declared"
    );
}

// ---------------------------------------------------------------------------
// H-11
// ---------------------------------------------------------------------------

/// 🔴 H-11: the four writers outside 44 §2.3 now answer problem+json — measured as **content
/// type + `gx_code` + status agreement**, on the four roads `req/182` probe1 P2/P3/P4/P4b walked.
#[tokio::test(flavor = "multi_thread")]
async fn h11_the_four_non_handler_writers_answer_problem_json() {
    let server = Server::new("v04l_h11", "before\n");
    let client = server.client();
    let unknown = format!("gx1:{}", "a".repeat(52));
    let roads = [
        (
            "① unrouted path",
            client.send("GET", "/v1/nope", None).await,
            404,
            "NOT_FOUND",
        ),
        (
            "① unrouted path outside the base path",
            client.send("GET", "/nope", None).await,
            404,
            "NOT_FOUND",
        ),
        (
            "① wrong method on a known path",
            client
                .send("DELETE", &format!("/v1/candidates/{unknown}"), None)
                .await,
            405,
            "VALIDATION_ERROR",
        ),
        (
            "② Path<u64> that does not parse",
            client
                .send("GET", "/v1/verdict-checkpoints/abc", None)
                .await,
            422,
            "VALIDATION_ERROR",
        ),
        (
            "③ verdict-checkpoint body, malformed",
            client
                .send_raw(
                    "POST",
                    "/v1/verdict-checkpoints",
                    &[("content-type", "application/json")],
                    b"{not json".to_vec(),
                )
                .await,
            422,
            "VALIDATION_ERROR",
        ),
        (
            "③ verdict-checkpoint body, non-JSON content type",
            client
                .send_raw(
                    "POST",
                    "/v1/verdict-checkpoints",
                    &[("content-type", "text/plain")],
                    b"origin=x".to_vec(),
                )
                .await,
            415,
            "UNSUPPORTED_MEDIA_TYPE",
        ),
        (
            "④ percent-decoding that is not UTF-8",
            client.send("GET", "/v1/candidates/%ff", None).await,
            422,
            "VALIDATION_ERROR",
        ),
    ];
    for (road, answer, status, code) in roads {
        println!(
            "H11 {road}: {} {} {}",
            answer.status, answer.content_type, answer.json
        );
        assert_eq!(answer.status.as_u16(), status, "{road}");
        assert_eq!(
            answer.content_type, "application/problem+json",
            "{road}: 44 §2.3 reaches this writer now"
        );
        assert_eq!(answer.gx_code(), code, "{road}");
        assert_eq!(
            answer.json["status"],
            serde_json::json!(status),
            "{road}: RFC 9457 duplicated status agrees"
        );
    }
}

// ---------------------------------------------------------------------------
// M-15 / DR-44-1
// ---------------------------------------------------------------------------

/// 🔴 M-15: the list rows carry `created_at` / `actor` / `scope`, live, and DR-44-1 (a): the
/// consistency answer is the bare proof.
#[tokio::test(flavor = "multi_thread")]
async fn m15_rows_carry_time_who_target_and_dr44_1_is_bare() {
    let server = Server::new("v04l_m15", "before\n");
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
    let page = client.send("GET", "/v1/candidates", None).await;
    let row = &page.json["items"][0];
    println!("M15_ROW={row}");
    assert_eq!(row["transformation"], serde_json::json!(id));
    assert!(row["created_at"].is_string(), "RFC 3339, live row: {row}");
    assert_eq!(
        row["actor"],
        serde_json::json!({ "Human": { "key": actor } }),
        "42 §3.2's Actor, as the row's `actor`"
    );
    assert_eq!(
        row["scope"],
        serde_json::json!(server.target.display().to_string()),
        "the fingerprint scope is the locator (fs adapter): {row}"
    );

    client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    client
        .send(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({})),
        )
        .await;
    let proof = client
        .send("GET", "/v1/ledger/consistency?from=1&to=2", None)
        .await;
    println!("DR44_1={}", proof.json);
    assert_eq!(proof.status.as_u16(), 200, "{}", proof.json);
    assert_eq!(proof.json["old_size"], serde_json::json!(1));
    assert_eq!(proof.json["new_size"], serde_json::json!(2));
    assert!(proof.json["path"].is_array());
    assert!(
        proof.json.get("proof").is_none(),
        "DR-44-1 (a): the wrapper is gone: {}",
        proof.json
    );
}
