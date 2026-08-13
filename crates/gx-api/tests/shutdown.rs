//! 🔴 Graceful shutdown's three stages, and the exit that may not be 0.
//!
//! req/88 §6.2 asks for 「shutdown 中の新規 request 拒否+in-flight commit 待ち+時間制限で (b) へ落ちる」
//! and, in red, for 「crash 経路を正常終了と綴らない」. Four probes, one per stage plus the exit.
//!
//! What is measured here and what is not: this file drives the **router and the state**, which is
//! where stages 1 and 2 live. The runtime — the bind, the signal, the deadline actually elapsing — is
//! `crates/gx-cli/tests/ac_056.rs`, over a real socket with a real `SIGTERM`. Splitting them keeps
//! each failure legible: a stage-1 defect here is a layer that did not refuse, and a defect there is
//! a runtime that did not stop.

mod support;

use std::sync::Arc;

use gx_api::{ServeOutcome, Shutdown};
use support::Server;

/// 🔴 **Stage 1** — once shutdown begins, a new request is refused, and `/healthz` is refused too.
///
/// `/healthz` is outside the Bearer guard (44 §2.6) and **inside** this one, deliberately: a load
/// balancer's health check is the first request that has to stop succeeding when a server is going
/// away. A surface answering 「ok」 while refusing everything else would keep traffic arriving at a
/// socket that refuses it.
#[tokio::test(flavor = "multi_thread")]
async fn stage_one_refuses_new_requests_including_the_health_check() {
    let server = Server::new("m6h6_shutdown_stage1", "before\n");
    let client = server.client();

    let before = client.send("GET", "/v1/healthz", None).await;
    assert_eq!(before.status, 200);

    server.state.shutdown().begin();

    let health = client.send("GET", "/v1/healthz", None).await;
    let work = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", "actor-key")),
        )
        .await;
    println!(
        "STAGE1_HEALTHZ={} STAGE1_POST={} code={} content_type={}",
        health.status.as_u16(),
        work.status.as_u16(),
        work.gx_code(),
        work.content_type
    );
    assert_eq!(health.status.as_u16(), 503);
    assert_eq!(work.status.as_u16(), 503);
    assert!(
        work.content_type.starts_with("application/problem+json"),
        "44 §2.3: 「全エラー応答は`Content-Type: application/problem+json`」"
    );
    assert_eq!(
        work.gx_code(),
        gx_api::gx_code::UNAVAILABLE,
        "🔴 **E-M6-22** (§53, M6H6-4 採(a)), implemented in hand 7. This assertion read `INTERNAL` \
         for the whole of hand 6 and named the fold rather than hiding it: 44 §2.3's twelve codes \
         are twelve words about a request and none is a word about a server. The ruling added the \
         row, and the code is now the thing it always meant. `crates/gx-api/tests/m6h7_api.rs` \
         holds the rest of the claim (the transcription of §2.3 is still twelve rows)"
    );
    assert!(
        work.json["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("shutting down"),
        "the detail is where the fold's lost meaning goes: {:?}",
        work.json
    );
}

/// 🔴 **Stage 2** — a request already inside a handler is counted, and finishes.
///
/// The counter is what the runtime waits on, so it has to be right in both directions: a handler that
/// never incremented would make a shutdown abandon work it should have waited for, and one that never
/// decremented would make every shutdown hit the deadline.
#[tokio::test(flavor = "multi_thread")]
async fn stage_two_counts_a_request_in_and_out() {
    let server = Server::new("m6h6_shutdown_stage2", "before\n");
    let shutdown = server.state.shutdown();
    assert_eq!(shutdown.in_flight(), 0);

    let answer = server
        .client()
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", "actor-key")),
        )
        .await;
    assert_eq!(answer.status, 201);
    println!("STAGE2_IN_FLIGHT_AFTER={}", shutdown.in_flight());
    assert_eq!(
        shutdown.in_flight(),
        0,
        "the counter returns to zero when the handler returns — including on the refusing paths, \
         because the guard that counts it in is dropped rather than decremented by hand"
    );

    let refused = server
        .client()
        .send("GET", "/v1/candidates/gx1:not-an-id", None)
        .await;
    assert_ne!(refused.status, 200);
    assert_eq!(
        shutdown.in_flight(),
        0,
        "a refusal is a request that finished"
    );
}

/// 🔴 **Stage 3's exit** — a deadline that expired is **not** a normal termination.
///
/// 44 §1.2 gives `gx serve` two codes and glosses 1 as 「起動失敗」; E-M6-16 already settled that
/// §1.2's columns are excerpts and §1.4's table governs, and §1.4's 1 is 「エラー（入力不正・内部
/// エラー・adapterエラー）」. A shutdown that abandoned work inside 43 T-9's critical section is an
/// internal error, and answering 0 would tell an init system that a process which may have left a
/// half-applied commit stopped cleanly — M4H4-2 at the deployment layer.
#[test]
fn stage_three_never_exits_zero() {
    let clean = ServeOutcome {
        reason: "SIGTERM",
        deadline_exceeded: false,
        in_flight_at_end: 0,
    };
    let abandoned = ServeOutcome {
        reason: "SIGTERM",
        deadline_exceeded: true,
        in_flight_at_end: 2,
    };
    println!(
        "CLEAN_EXIT={} ABANDONED_EXIT={} ABANDONED_JSON={}",
        clean.exit_code(),
        abandoned.exit_code(),
        abandoned.to_json()
    );
    assert_eq!(clean.exit_code(), 0, "44 §1.2: 「0=正常終了（SIGTERM等）」");
    assert_eq!(
        abandoned.exit_code(),
        1,
        "🔴 「crash 経路を正常終了と綴らない」 (req/88 §6.2)"
    );
    // The log line carries the same three facts, so that an operator reading stdout learns what the
    // status alone cannot say: **how many** requests were abandoned.
    assert_eq!(
        abandoned.to_json()["in_flight_at_end"],
        serde_json::json!(2)
    );
    assert_eq!(
        abandoned.to_json()["deadline_exceeded"],
        serde_json::json!(true)
    );
    assert_eq!(abandoned.to_json()["exit"], serde_json::json!(1));
}

/// The flag is one-way and shared: two holders of the `Arc` see one answer.
#[test]
fn the_shutdown_flag_is_shared_and_one_way() {
    let shutdown = Arc::new(Shutdown::new());
    let reader = Arc::clone(&shutdown);
    assert!(!reader.begun());
    shutdown.begin();
    assert!(
        reader.begun(),
        "the router refuses on one clone, the reader task in `stream` ends on another, and the \
         runtime waits on a third — a flag one of them could not see would be a shutdown that hung"
    );
    shutdown.begin();
    assert!(reader.begun(), "beginning twice is beginning");
}

/// 🔴 A subscriber's reader loop ends when shutdown begins — the reason stage 1 exists at all.
///
/// A `GET /stream` subscription is an in-flight request **that never ends**. Without stage 1 telling
/// the readers to stop, `axum::serve`'s graceful shutdown would wait for a body that has no last
/// frame, every shutdown would reach the deadline, and every exit would be 1. So this is not a
/// courtesy to subscribers: it is what makes a clean shutdown possible at all.
#[tokio::test(flavor = "multi_thread")]
async fn a_subscription_ends_when_the_server_begins_shutting_down() {
    use http_body_util::BodyExt;

    let server = Server::new("m6h6_shutdown_stream", "before\n");
    let response = server
        .client()
        .open("/v1/stream?since=2099-01-01T00:00:00Z", &[])
        .await;
    let mut body = response.into_body();

    server.state.shutdown().begin();

    let ended = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
        .await
        .expect("the body ended inside the wait (0 retries — see stream.rs's module header)");
    println!("STREAM_AFTER_SHUTDOWN={:?}", ended.is_none());
    assert!(
        ended.is_none(),
        "the stream ended rather than producing another frame"
    );
}
