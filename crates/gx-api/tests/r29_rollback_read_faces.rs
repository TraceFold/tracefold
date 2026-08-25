// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R29 item 5 (`req/364` §0-5, from `req/361` §3-1 and L-03)** — what became of 43 T-10c's
//! roll-back reaches **all three read faces**, and they agree with Σ and with each other.
//!
//! # The question this closes, and who could not ask it
//!
//! R28 put the roll-back facts on the **refusal** — the body returned to whoever was holding the
//! request that aborted. The twenty-eighth audit then drove the read faces on a **corrupted world**
//! and read what a client arriving afterwards actually gets:
//!
//! ```text
//! A28_READ_FACE get_members=["receipt","state","superseded_by","transformation"]
//!               get_body: "state":{"Aborted":"ApplyFailed"}
//! A28_READ_FACE list rows: {transformation, state, verdict, enforced, created_at, actor, scope,
//!                           superseded_by, inverse_status}   ← 9 keys, no rollback
//! A28_STREAM    journal_record_holds_rollback=true stream_arm_emits_rollback=false
//! ```
//!
//! So the abort **reason** was readable and the roll-back was not. An auditor reconciling a ledger,
//! a GUI reconnecting after a disconnect, a script re-fetching the row — none of them could reach
//! *is my object back where it was, or is it half undone*, which is the question this product is
//! named for. The stream was worse than the other two and cheaper than both: the journal record
//! carries `rollback` in its own shape and `stream.rs` was dropping it with `..`, so the value was
//! already in the assembling function's hand.
//!
//! # 🔴 Why all three in one lane rather than the two that were filed
//!
//! Because `req/361` filed the stream as a **denominator** finding, not a wire one: R28's hand-off
//! named `GET /transformations/{id}` and the list, and a ruling taken on two faces leaves the third
//! behind — which is precisely how the third came to be missing in the first place. `req/38` §238
//! ruling 3 took that reading. 44 §2.6 permits the addition in as many words ("a backward-compatible
//! addition (a new optional field) is allowed within `/v1`"), and DR-44-9's "no additions" row
//! predates the member's existence and is silent about it.
//!
//! # 🔴 How this bed makes a **non-null** roll-back without a hand-written adapter
//!
//! The value has to be real or this file measures a `null` and calls it a member. The shipped fs
//! adapter is used exactly as it ships, and the *world* is made hostile instead: the directory
//! holding the target is made unwritable after `verify`, so the commit's `apply` fails, T-10c fires
//! on the escrowed inverse, and that apply cannot land either. Σ records
//! `Aborted(ApplyFailed)` with `Rollback::Failed` — a real word, on the real road, with no fixture
//! adapter anywhere.
//!
//! **Declared, not hidden**: this bed depends on the process not being able to write into a
//! `0o555` directory, which is false for `root`. `a_bed_control_…` below **fails loudly** in that
//! case rather than passing with a `null`, because a bed that quietly stops being hostile is the
//! way this whole family of gates goes vacuously green (`req/334` §9-3).
//!
//! The sibling that produces `Rollback::Diverged` — the word R29 mints — needs an adapter whose
//! `apply` can fail halfway, and lives in `crates/gx-cli/tests/r29_rollback_is_verified.rs`, the
//! one crate that depends on both `gx-substrate` and `gx-api`.

mod support;

use support::Server;

fn id_of(json: &serde_json::Value) -> String {
    json["transformation"]["id"]
        .as_str()
        .or_else(|| json["transformation"].as_str())
        .or_else(|| json["id"].as_str())
        .expect("the answer names the transformation")
        .to_string()
}

/// Make `dir` unwritable, and prove it: a bed whose hostility is assumed is not a bed.
fn seal(dir: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555));
    }
    // The proof, taken rather than assumed.
    let probe = dir.join("r29_write_probe");
    let writable = std::fs::write(&probe, b"x").is_ok();
    let _ = std::fs::remove_file(&probe);
    !writable
}

fn unseal(dir: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
    }
}

/// Drive one transformation into `Aborted(ApplyFailed)` and hand back the server and the id.
///
/// The road is the shipped one — `POST /candidates` → `verify` → `commit` — and only the world is
/// hostile. Returns the commit's status so an arm can assert the abort really happened.
async fn a_world_that_refuses_the_write(name: &str) -> (Server, String, u16, serde_json::Value) {
    let server = Server::new(name, "A\nB\n");
    let client = server.client();
    let actor = server.keys.signing_id();

    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("A\nB\nC\n", &actor)),
        )
        .await;
    let id = id_of(&created.json);
    client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;

    let dir = server
        .target
        .parent()
        .expect("the target has a parent")
        .to_path_buf();
    let sealed = seal(&dir);
    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    unseal(&dir);
    assert!(
        sealed,
        "🔴 the bed could not make {} unwritable, so the commit below did not fail for the reason \
         this file needs it to fail for. Running as root will do this. A `null` roll-back measured \
         here would be a member with nothing in it, reported as a passing gate",
        dir.display()
    );
    (server, id, committed.status.as_u16(), committed.json)
}

/// What Σ itself holds for `id`, as the word the wire is supposed to be carrying.
fn sigma_rollback(server: &Server, id: &str) -> Option<&'static str> {
    let engine = server.state.engine();
    let tid = gx_core::TransformationId(gx_core::Cid::from_text(id).expect("a transformation id"));
    engine.rollback(&tid).map(|r| r.kind())
}

// ---------------------------------------------------------------------------
// a — bed control
// ---------------------------------------------------------------------------

/// 🔴 **Bed control** — the commit really aborted, and Σ really holds a **non-null** roll-back word.
///
/// Every arm below reads a member off a wire answer. If the road did not abort, or if it aborted
/// before T-10c's guard opened, the member is `null` everywhere and all three arms pass while
/// measuring nothing at all. `req/334` §9-3 is the standing reason this arm exists: *the instrument
/// returned zero three times and three times the instrument was wrong.*
#[tokio::test]
async fn a_bed_control_the_road_aborts_and_sigma_holds_a_rollback_word() {
    let (server, id, status, body) = a_world_that_refuses_the_write("r29_faces_bed").await;
    let sigma = sigma_rollback(&server, &id);
    println!("R29_FACES_BED commit_status={status} sigma_rollback={sigma:?} body={body}");
    assert_ne!(
        status, 200,
        "🔴 the commit succeeded against a directory this bed sealed, so nothing below is being \
         measured on an abort: {body}"
    );
    assert!(
        sigma.is_some(),
        "🔴 the transformation aborted but Σ holds no roll-back word, so every arm below would be \
         asserting that `null` equals `null`. T-10c's guard did not open on this road and the bed \
         has to be rebuilt rather than the arms weakened"
    );
}

// ---------------------------------------------------------------------------
// b, c, d — the three faces
// ---------------------------------------------------------------------------

/// 🔴 **`req/361` §3-1** — `GET /transformations/{id}` carries the word Σ holds.
///
/// The audit measured four members here and no road to the roll-back. The member is added under
/// 44 §2.6, and this arm holds the harder half: not that a key called `rollback` exists, but that
/// it carries **the same word Σ carries**. A member assembled from somewhere else that happens to
/// look right is `req/187` §5's own finding (a `prev` that held an id where an ancestor held a
/// hash) and is the failure mode a key-presence check cannot see.
#[tokio::test]
async fn b_the_transformation_row_carries_the_rollback_word() {
    let (server, id, _, _) = a_world_that_refuses_the_write("r29_faces_get").await;
    let sigma = sigma_rollback(&server, &id).expect("the bed control holds that Σ has one");
    let got = server
        .client()
        .send("GET", &format!("/v1/transformations/{id}"), None)
        .await;
    println!(
        "R29_FACE_GET status={} body={}",
        got.status.as_u16(),
        got.json
    );
    assert_eq!(got.status.as_u16(), 200, "{}", got.json);
    assert!(
        got.json.get("rollback").is_some(),
        "🔴 `req/361` §3-1: this is the row a client re-fetches once the refusal is gone, and it \
         still cannot say whether the object is back where it was: {}",
        got.json
    );
    assert_eq!(
        got.json["rollback"], sigma,
        "🔴 the member is on the wire and does not agree with Σ, which is worse than its absence: \
         a reader can branch on it and be wrong: {}",
        got.json
    );
}

/// 🔴 **`req/361` §3-1** — every `GET /transformations` row carries it too.
///
/// On the list for the same reason `inverse_status` is on the list (M6H6-15): *which of these can I
/// still undo* is a question about a **set**, and so is *which of these aborted with my object left
/// somewhere it should not be*. The auditor reconciling a ledger against a journal reads this page,
/// not a refusal somebody else received once.
#[tokio::test]
async fn c_every_list_row_carries_the_rollback_word() {
    let (server, id, _, _) = a_world_that_refuses_the_write("r29_faces_list").await;
    let sigma = sigma_rollback(&server, &id).expect("the bed control holds that Σ has one");
    let all = server
        .client()
        .send("GET", "/v1/transformations", None)
        .await;
    let rows = all.json["items"]
        .as_array()
        .expect("a page of rows")
        .clone();
    println!("R29_FACE_LIST rows={} body={}", rows.len(), all.json);
    assert!(
        !rows.is_empty(),
        "🔴 the list is empty, so nothing is measured: {}",
        all.json
    );
    let missing: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|r| r.get("rollback").is_none())
        .collect();
    assert!(
        missing.is_empty(),
        "🔴 `req/361` §3-1: a row without the member is a row an auditor cannot read the answer \
         off: {missing:?}"
    );
    let ours = rows
        .iter()
        .find(|r| r["transformation"] == serde_json::Value::String(id.clone()))
        .expect("the aborted row is listed");
    assert_eq!(
        ours["rollback"], sigma,
        "🔴 the list's word and Σ's word are not the same word: {ours}"
    );
}

/// 🔴 **`req/361` L-03** — the `aborted` event on a **real subscription** carries it.
///
/// This is the arm the audit could not write. Its own §4 declares it: *"`GET /stream` is not driven.
/// L-03 is source-level"* — gx-cli holds no dev-dependency that reads a streaming body one frame at
/// a time, so the finding was filed by reading `stream.rs` and seeing the `..`. This crate does hold
/// one (`http_body_util`, already used by `wire_census.rs`), so the claim is driven here rather than
/// argued: a subscription is opened against the shipped router and the event is read off the socket.
#[tokio::test]
async fn d_the_aborted_stream_event_carries_the_rollback_word() {
    use http_body_util::BodyExt;
    let (server, id, _, _) = a_world_that_refuses_the_write("r29_faces_stream").await;
    let sigma = sigma_rollback(&server, &id).expect("the bed control holds that Σ has one");

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
    let mut aborted: Vec<serde_json::Value> = Vec::new();
    for _ in 0..expected {
        let frame = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
            .await
            .expect("a line arrived inside the wait")
            .expect("the stream is still open")
            .expect("the frame is readable");
        let bytes = frame.into_data().expect("a data frame");
        let line: serde_json::Value =
            serde_json::from_str(String::from_utf8(bytes.to_vec()).expect("utf-8").trim())
                .expect("each line is one JSON object");
        if line["event"] == "aborted" {
            aborted.push(line);
        }
    }
    println!("R29_FACE_STREAM lines={expected} aborted={aborted:?} sigma={sigma:?}");
    assert!(
        !aborted.is_empty(),
        "🔴 no `aborted` event arrived on the subscription, so this arm measures nothing — the bed \
         aborted a transformation and the stream has to carry it"
    );
    let ours = aborted
        .iter()
        .find(|l| l["transformation"] == serde_json::Value::String(id.clone()))
        .expect("the aborted row's own event");
    assert!(
        ours["data"].get("rollback").is_some(),
        "🔴 `req/361` L-03: the journal record holds `rollback` and this event still drops it. It \
         is the cheapest of the three faces — the value is in the assembling function's hand: \
         {ours}"
    );
    assert_eq!(
        ours["data"]["rollback"], sigma,
        "🔴 the stream's word and Σ's word are not the same word: {ours}"
    );
}
