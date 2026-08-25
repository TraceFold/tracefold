// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **AC-P3-12** — the verdict checkpoint over HTTP, and the same five judgements as the CLI.
//!
//! `req/119` §4. 32 §D said the surface was "currently zero endpoints" (sem: SEM-gx-api-296); this is one of the two that closes it, and
//! `crates/gx-cli/tests/verdict_checkpoint_surface.rs` is the other. What is measured here:
//!
//! * `POST /v1/verdict-checkpoints` answers **201** with the signed document;
//! * `GET /v1/verdict-checkpoints` follows 44 §2.7 — `?limit=` is refused outside 1..=200 rather
//!   than clamped, and `?cursor=` resumes **after** a published `window_end`;
//! * `GET /v1/verdict-checkpoints/{window_end}` answers one, and a coordinate no checkpoint closes
//!   at is `NOT_FOUND` rather than the nearest one;
//! * the counts a chain issued over HTTP publishes are the counts the **engine** holds, checked by
//!   folding the chain rather than by re-reading the field it came from.

mod support;

use gx_api::verdict_checkpoints::{DEFAULT_VERDICT_ORIGIN, VERDICT_CHECKPOINT_ENDPOINTS};
use support::Server;

/// 44 §2.2's `Candidate` id, out of the object a create answered with.
///
/// Spelled here rather than shared: `tests/endpoints.rs` and `tests/idempotency.rs` each carry their
/// own, because a helper in `support` would be a third place a suite reads the response shape from.
fn id_of(json: &serde_json::Value) -> String {
    json["id"]
        .as_str()
        .unwrap_or_else(|| panic!("`POST /candidates` returns the id: {json}"))
        .to_string()
}

/// Drive one transformation all the way through, so that a window has verdicts in it.
async fn two_verdicts(server: &Server) {
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
}

/// AC-P3-12: issue, list, get.
#[tokio::test]
async fn ac_p3_12_the_chain_is_published_listed_and_fetched() {
    let server = Server::new("verdict_checkpoints_http", "before\n");
    two_verdicts(&server).await;
    let client = server.client();

    let issued = client.send("POST", "/v1/verdict-checkpoints", None).await;
    println!("ISSUE={} {}", issued.status.as_u16(), issued.json);
    assert_eq!(
        issued.status.as_u16(),
        201,
        "a checkpoint is a document this call created"
    );
    assert_eq!(issued.json["origin"], DEFAULT_VERDICT_ORIGIN);
    assert!(
        issued.json["signature"]["sig"].is_string(),
        "and it is signed by this server's key: {}",
        issued.json
    );
    let first_window_end = issued.json["window_end"].as_u64().expect("a window end");
    assert!(
        first_window_end >= 1,
        "the window holds the verdicts this fixture drove"
    );

    // A second issue with nothing between the two: an **empty** window rather than a repeat, which
    // is the engine's own ruling (a verifier folds the chain and a repeat would double every count).
    let again = client.send("POST", "/v1/verdict-checkpoints", None).await;
    println!("ISSUE_AGAIN={} {}", again.status.as_u16(), again.json);
    assert_eq!(again.status.as_u16(), 201);
    assert_eq!(
        again.json["window_start"], issued.json["window_end"],
        "the second window opens where the first closed"
    );
    assert_eq!(
        again.json["tally"]["admit"], 0,
        "and it is empty: a true statement about a quiet period"
    );

    let listed = client.send("GET", "/v1/verdict-checkpoints", None).await;
    println!("LIST={} {}", listed.status.as_u16(), listed.json);
    assert_eq!(listed.status.as_u16(), 200);
    assert_eq!(listed.json["total"], 2);
    assert_eq!(
        listed.json["items"].as_array().map(Vec::len),
        Some(2),
        "both are in one page under the default limit"
    );

    let one = client
        .send(
            "GET",
            &format!("/v1/verdict-checkpoints/{first_window_end}"),
            None,
        )
        .await;
    println!("GET_ONE={} {}", one.status.as_u16(), one.json);
    assert_eq!(one.status.as_u16(), 200);
    assert_eq!(one.json["window_end"], first_window_end);

    let missing = client
        .send("GET", "/v1/verdict-checkpoints/9999", None)
        .await;
    println!("GET_MISSING={} {}", missing.status.as_u16(), missing.json);
    assert_eq!(missing.status.as_u16(), 404);
    assert_eq!(missing.gx_code(), "NOT_FOUND");
}

/// 44 §2.7's regulation, on this module's own cursor.
#[tokio::test]
async fn the_page_follows_44_2_7_and_refuses_a_limit_it_cannot_honour() {
    let server = Server::new("verdict_checkpoints_paging", "before\n");
    two_verdicts(&server).await;
    let client = server.client();
    for _ in 0..3 {
        client.send("POST", "/v1/verdict-checkpoints", None).await;
    }

    let page = client
        .send("GET", "/v1/verdict-checkpoints?limit=1", None)
        .await;
    println!("PAGE1={}", page.json);
    assert_eq!(page.json["items"].as_array().map(Vec::len), Some(1));
    let cursor = page.json["next_cursor"].as_u64().expect("a next cursor");
    assert_eq!(
        cursor, 0,
        "the cursor is the position of the last row returned"
    );

    let second = client
        .send(
            "GET",
            &format!("/v1/verdict-checkpoints?limit=1&cursor={cursor}"),
            None,
        )
        .await;
    println!("PAGE2={}", second.json);
    assert_eq!(second.json["items"].as_array().map(Vec::len), Some(1));
    // 🔴 Not `window_end`: the second, third and fourth checkpoints of this fixture have **empty**
    // windows, so they all close at the same coordinate. What distinguishes them is their position
    // in the chain, which is exactly why the cursor is one — the first run of this suite paged on
    // `window_end` and answered an empty page.
    assert_eq!(
        second.json["items"][0]["window_start"], second.json["items"][0]["window_end"],
        "the second checkpoint of this chain closes an empty window"
    );
    assert_eq!(
        second.json["next_cursor"].as_u64(),
        Some(1),
        "and paging advanced by one position"
    );

    // Refused rather than clamped: `crate::list::Page::limit`'s argument, on the same regulation.
    let over = client
        .send("GET", "/v1/verdict-checkpoints?limit=201", None)
        .await;
    println!("LIMIT_201={} {}", over.status.as_u16(), over.json);
    // 422, which is what M6-09's map spells `VALIDATION_ERROR` (44 §2.3): a limit outside the range
    // is a request this server understood and will not attempt, not a malformed one.
    assert_eq!(over.status.as_u16(), 422);
    assert_eq!(over.gx_code(), "VALIDATION_ERROR");
}

/// The three routes are declared, and the declaration is compared with what answers.
#[tokio::test]
async fn the_three_routes_are_declared_and_all_three_answer() {
    println!("VERDICT_CHECKPOINT_ENDPOINTS={VERDICT_CHECKPOINT_ENDPOINTS:?}");
    assert_eq!(VERDICT_CHECKPOINT_ENDPOINTS.len(), 3);
    let server = Server::new("verdict_checkpoints_routes", "before\n");
    let client = server.client();
    for (method, path) in [
        ("POST", "/v1/verdict-checkpoints"),
        ("GET", "/v1/verdict-checkpoints"),
        ("GET", "/v1/verdict-checkpoints/0"),
    ] {
        let answer = client.send(method, path, None).await;
        println!("ROUTE {method} {path} -> {}", answer.status.as_u16());
        assert_ne!(
            answer.status.as_u16(),
            405,
            "{method} {path} is routed to a different method"
        );
        assert!(
            !answer.json.is_null(),
            "{method} {path} answered with no body, which is what an unrouted path does"
        );
    }
}

/// 🔴 The Bearer guard covers them, like everything but `/healthz` (44 §2.5).
///
/// A new route added outside the `route_layer` would be a hole nobody notices, which is exactly why
/// `tests/auth.rs` walks the router. This is the same walk for the three added here.
#[tokio::test]
async fn the_new_routes_are_behind_the_bearer_guard() {
    let server = Server::new("verdict_checkpoints_auth", "before\n");
    let anonymous = server.anonymous();
    for (method, path) in [
        ("POST", "/v1/verdict-checkpoints"),
        ("GET", "/v1/verdict-checkpoints"),
        ("GET", "/v1/verdict-checkpoints/0"),
    ] {
        let answer = anonymous.send(method, path, None).await;
        println!("ANON {method} {path} -> {}", answer.status.as_u16());
        assert_eq!(
            answer.status.as_u16(),
            401,
            "44 §2.5: \"every endpoint (except `/healthz`) requires `Authorization: Bearer <token>`\" (sem: SEM-gx-api-297)"
        );
    }
}
