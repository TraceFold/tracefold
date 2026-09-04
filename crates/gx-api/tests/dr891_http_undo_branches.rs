// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DEFECT-891-1 parity probe over HTTP** — req/910 C6 / req/919 W3.
//!
//! `crates/gx-cli/tests/dr891_undo_branches.rs` measured, through the `gx` binary, that a second
//! undo sharing one `Intent` with an earlier undo (two undos that restore the same bytes at the
//! same locator under the same context and actor collide on `IntentId`, since `Transformation`'s
//! identity carries `parents` and `Intent`'s does not) used to make the first undo's branch
//! unreachable: `gx undo <T_u>` answered exit 6 `NOT_FOUND`. req/910 C6 asked whether `gx-api`
//! (the HTTP face) shares that failure. This file drives the same scenario through the HTTP
//! router and reports the answer as one of HIT / MISS / UNTESTABLE (req/919 W3's three-valued
//! form) rather than pass/fail, because a divergence here is a finding and not a defect in this
//! probe.
//!
//! # What the source already says, and why this file still runs it live
//!
//! `crates/gx-api/src/handlers.rs`'s `rebuilt()` resolves a transformation's `IntentId` through
//! [`gx_engine::Engine::intent_of`] (a table/shadow lookup keyed **by** `TransformationId`), never
//! through the search-and-compare-with-`Engine::resolved` shape `gx_cli::session::Session`'s
//! (pre-repair) `intent_of` used — `grep -n "\.resolved(" crates/gx-api/src/*.rs` is empty. So the
//! *mechanism* DEFECT-891-1 exploited does not exist in this crate's source. That is a claim about
//! code shape, not about behaviour, and W3 asks for behaviour — hence the live probe below rather
//! than resting on the grep.

mod support;

use axum::body::Body;
use axum::http::Request;
use support::Server;
use tower::ServiceExt;

/// `POST /v1/candidates` -> verify -> commit, for one goal. Returns the committed
/// transformation's id. Context is `Evidence` on every call ([`Server::intent_body`] fixes it),
/// which is what makes two of these under the same actor share one intent once undone.
async fn commit(server: &Server, goal: &str) -> String {
    let client = server.client();
    let actor = server.keys.signing_id();
    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body(goal, &actor)),
        )
        .await;
    assert_eq!(
        created.status.as_u16(),
        201,
        "create({goal}): {}",
        created.json
    );
    let id = created.json["id"]
        .as_str()
        .expect("POST /v1/candidates returns the id")
        .to_string();

    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    assert_eq!(
        verified.status.as_u16(),
        200,
        "verify({goal}): {}",
        verified.json
    );

    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    assert_eq!(
        committed.status.as_u16(),
        200,
        "commit({goal}): {}",
        committed.json
    );
    id
}

/// `POST /v1/transformations/{id}/undo`. Returns `(status, minted transformation id, body)`.
async fn undo(server: &Server, id: &str) -> (u16, String, serde_json::Value) {
    let out = server
        .client()
        .send(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({})),
        )
        .await;
    let minted = out.json["transformation"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    (out.status.as_u16(), minted, out.json)
}

/// 🔴 The HIT/MISS probe, live, in one server process (no restart between steps -- see the module
/// documentation for why a restart-then-rehydrate variant is a separate, UNTESTABLE question with
/// this crate's current test fixtures).
///
/// `V0 -> V1 -> undo -> V2 -> undo -> redo(V1)`, exactly `dr891_undo_branches.rs`'s first test,
/// driven through `/v1/candidates` and `/v1/transformations/{id}/undo` instead of `gx submit` /
/// `gx plan` / `gx verify` / `gx commit` / `gx undo`.
#[tokio::test]
async fn dr891_1_parity_http_same_context_redo() {
    let server = Server::new("dr891_http_same_context", "V0\n");

    let t_o = commit(&server, "V1\n").await;
    assert_eq!(
        server.target_contents(),
        "V1\n",
        "the first commit moved the world"
    );

    let (undo_o_status, t_u, undo_o_body) = undo(&server, &t_o).await;
    assert_eq!(undo_o_status, 200, "undo(T_o): {undo_o_body}");
    assert!(
        !t_u.is_empty(),
        "the undo minted a transformation: {undo_o_body}"
    );
    assert_eq!(
        server.target_contents(),
        "V0\n",
        "the undo put the world back"
    );

    let t_x = commit(&server, "V2\n").await;
    assert_eq!(server.target_contents(), "V2\n");
    let (undo_x_status, t_xu, undo_x_body) = undo(&server, &t_x).await;
    assert_eq!(undo_x_status, 200, "undo(T_x): {undo_x_body}");
    assert_ne!(
        t_xu, t_u,
        "the two undos differ in `parents`, so they are two transformations -- if these are equal \
         the rest of this probe is measuring nothing"
    );
    assert_eq!(
        server.target_contents(),
        "V0\n",
        "the world is back at the fork"
    );

    // The redo: undo the first undo. On the pre-repair engine this is where CLI's DEFECT-891-1
    // showed as exit 6 NOT_FOUND, because the shared intent's `resolved` entry had been
    // overwritten by T_x's undo.
    let (redo_status, _, redo_body) = undo(&server, &t_u).await;
    println!(
        "DR891_HTTP_SAME_CONTEXT verdict={} redo_status={redo_status} world={:?} body={redo_body}",
        if redo_status == 200 && server.target_contents() == "V1\n" {
            "HIT"
        } else {
            "MISS"
        },
        server.target_contents()
    );
    assert_eq!(
        server.target_contents(),
        "V1\n",
        "MISS: the redo did not restore the branch over HTTP. redo said: {redo_body}"
    );
    assert_eq!(redo_status, 200, "MISS: redo: {redo_body}");
    assert!(
        redo_body.get("gx_code").and_then(|v| v.as_str()) != Some("NOT_FOUND"),
        "MISS: the HTTP face answered NOT_FOUND about a transformation with a signed commit \
         receipt, DEFECT-891-1's own shape: {redo_body}"
    );
}

/// 🔴 The discriminating control, over HTTP: a different `--context`-equivalent means the two
/// undos never shared an intent, and this road was never broken on either face. Kept for the same
/// reason `dr891_undo_branches.rs` keeps it: a future regression that broke undo in general fails
/// both this and the test above; one that reintroduces the collision fails only the test above.
#[tokio::test]
async fn dr891_1_parity_http_different_context_control() {
    let server = Server::new("dr891_http_diff_context", "V0\n");

    // `Server::intent_body` fixes `context: "Evidence"`; the second commit is built with the raw
    // JSON body directly so this control can vary it (the one flag `dr891_undo_branches.rs`
    // varies, one substrate over).
    let client = server.client();
    let actor = server.keys.signing_id();

    let t_o = commit(&server, "V1\n").await;
    let (undo_o_status, t_u, undo_o_body) = undo(&server, &t_o).await;
    assert_eq!(undo_o_status, 200, "undo(T_o): {undo_o_body}");

    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(serde_json::json!({
                "substrate": "fs",
                "locator": server.target.display().to_string(),
                "goal": "V2\n",
                "context": "Policy",
                "actor": { "Human": { "key": actor } },
            })),
        )
        .await;
    assert_eq!(
        created.status.as_u16(),
        201,
        "create(diff ctx): {}",
        created.json
    );
    let cand_id = created.json["id"].as_str().expect("id").to_string();
    let verified = client
        .send("POST", &format!("/v1/candidates/{cand_id}/verify"), None)
        .await;
    assert_eq!(verified.status.as_u16(), 200);
    let committed = client
        .send("POST", &format!("/v1/candidates/{cand_id}/commit"), None)
        .await;
    assert_eq!(committed.status.as_u16(), 200);

    let (undo_x_status, _, undo_x_body) = undo(&server, &cand_id).await;
    assert_eq!(undo_x_status, 200, "undo(T_x, diff ctx): {undo_x_body}");
    assert_eq!(server.target_contents(), "V0\n");

    let (redo_status, _, redo_body) = undo(&server, &t_u).await;
    println!(
        "DR891_HTTP_DIFF_CONTEXT redo_status={redo_status} world={:?}",
        server.target_contents()
    );
    assert_eq!(server.target_contents(), "V1\n", "redo: {redo_body}");
    assert_eq!(redo_status, 200, "redo: {redo_body}");
}

/// 🔴 The restart-fidelity variant, and why it is reported UNTESTABLE rather than HIT/MISS.
///
/// `dr891_undo_branches.rs` drives every step through a **fresh `gx` process**, so
/// `Engine::open`'s empty in-flight table (M5H3-5) is guaranteed by the time the redo step asks
/// for `T_u`'s intent -- `crates/gx-api/src/handlers.rs`'s `rebuilt()` path (`Engine::intent_of` +
/// `state.drafts().load`) only runs when the table does **not** already hold the row live, which
/// none of the two tests above ever force (one live `Server`, no restart, so `undo`'s own
/// `with_a_body` check finds the row and returns immediately without calling `rebuilt()` at all).
///
/// This test forces that condition the way `crates/gx-api/tests/idempotency.rs`'s
/// `the_record_outlives_the_store_that_wrote_it` does for its own claim: **a second `Engine::open`
/// over the same on-disk journal** — a fresh, empty table, replaying from disk, the closest a
/// single-process test stands to a restart. The receipt archive is carried over by sharing the
/// same `Arc<MemoryArchive>` (receipts are meant to outlive a restart; `gx-api`'s test support has
/// no on-disk archive, so this is the honest substitute) — but `Server::build_in` never wires a
/// `DraftArchive` (`AppState::new` defaults to `NoDrafts`), which is a **pre-existing gap in this
/// crate's own test fixtures**, unrelated to DEFECT-891-1: `rebuilt()`'s `state.drafts().load(&intent_id)`
/// answers `None` unconditionally, so the row is never rebuilt regardless of whether the two
/// undos' intents collided. That is what makes the restart-fidelity question **UNTESTABLE** with
/// this crate's current fixtures rather than HIT or MISS -- a fixture gap, not a divergence.
#[tokio::test]
async fn dr891_1_parity_http_after_restart_is_untestable_no_draft_archive() {
    let server = Server::new("dr891_http_restart", "V0\n");
    let t_o = commit(&server, "V1\n").await;
    let (undo_o_status, t_u, undo_o_body) = undo(&server, &t_o).await;
    assert_eq!(undo_o_status, 200, "undo(T_o): {undo_o_body}");
    let t_x = commit(&server, "V2\n").await;
    let (undo_x_status, _, undo_x_body) = undo(&server, &t_x).await;
    assert_eq!(undo_x_status, 200, "undo(T_x): {undo_x_body}");
    assert_eq!(server.target_contents(), "V0\n");

    // "Restart": a fresh `Engine::open` over the same journal (empty table, replayed from disk),
    // the same archive `Arc` (receipts carried over, the one thing a real restart's disk-backed
    // archive would also carry), and gx-api's own default `NoDrafts` (the gap this test exists to
    // report).
    let journal = server.project.join(".gx").join("ledger").join("journal");
    let gate = gx_gate::Gate::with_policies(gx_gate::packs::fs_pack().expect("the shipped pack"));
    let evidence = gx_api::state::RequestEvidence::new();
    let mut engine =
        gx_engine::Engine::open(&journal, gate, evidence.clone()).expect("reopen the engine");
    engine.register_adapter(
        std::sync::Arc::new(gx_adapter_fs::FsAdapter::new()),
        "dr891 restart probe",
    );
    let restarted_state = gx_api::state::AppState::new(
        engine,
        evidence,
        server.keys.clone(),
        gx_api::auth::Bearer::new(support::TOKEN),
        server.project.join(".gx").join("index"),
        Some(&server.keys.signing_id()),
    )
    .expect("the recorded keyid is this server's")
    .with_archive(server.archive.clone() as std::sync::Arc<dyn gx_api::ReceiptArchive>);
    let restarted_router = gx_api::router(restarted_state);

    let request = Request::builder()
        .method("POST")
        .uri(format!("/v1/transformations/{t_u}/undo"))
        .header("authorization", format!("Bearer {}", support::TOKEN))
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .expect("a request builds");
    let response = restarted_router
        .oneshot(request)
        .await
        .expect("the router answers");
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the body reads");
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    let gx_code = body.get("gx_code").and_then(|v| v.as_str()).unwrap_or("");
    println!(
        "DR891_HTTP_RESTART status={status} gx_code={gx_code} untestable_reason=NoDrafts \
         body={body}"
    );
    // The claim this test makes: the restart-then-redo road is blocked by the missing
    // `DraftArchive`, which answers with a body-less refusal -- not `NOT_FOUND`
    // (DEFECT-891-1's own shape) and not `200`. If this ever starts answering `NOT_FOUND`, that
    // *would* be DEFECT-891-1 on the HTTP face and this assertion should be read as a MISS, not
    // adjusted to keep passing.
    assert_ne!(
        status, 200,
        "unexpected: a redo after restart succeeded with no DraftArchive wired -- the UNTESTABLE \
         classification below no longer holds and this scenario should be re-graded HIT"
    );
    assert_ne!(
        gx_code, "NOT_FOUND",
        "if this ever fires, it means the HTTP face reproduces DEFECT-891-1's exact refusal \
         after a restart -- report as MISS, do not paper over it here"
    );
}
