//! 🔴 **M6-11 採(b)** — 44 §2.4's `Idempotency-Key`, and the sentence it is **not** about.
//!
//! The four claims 44 §2.4 makes, plus one it does not and M6-11 requires:
//!
//! 1. a repeated key returns the **same response**;
//! 2. a repeated key with a **different body** is `409 IDEMPOTENCY_CONFLICT`;
//! 3. the record survives a restart (which is why §47 took (b) over (a));
//! 4. a request with **no** key is not cached, so nothing accumulates by accident;
//! 5. 🔴 **deleting the cache does not make a double apply possible** — the guarantee is 43 T-11's
//!    key idempotence (ASM-43-1), and the cache protects the identity of the *response*. M6-11's
//!    ruling is that this be written down; here it is measured, which is stronger.

mod support;

use gx_api::idempotency::{replay_or_conflict, Entry, IdempotencyStore, TTL_NANOS};
use gx_core::{Timestamp, TransformationId};
use support::Server;

/// The id in a `POST /candidates` response.
fn id_of(json: &serde_json::Value) -> String {
    json["id"]
        .as_str()
        .unwrap_or_else(|| panic!("`POST /candidates` returns the id: {json}"))
        .to_string()
}

/// A committed transformation, and the server that committed it.
async fn committed(name: &str) -> (Server, String) {
    let server = Server::new(name, "before\n");
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
    (server, id)
}

/// 🔴 The same key twice returns the same bytes, and the ledger grew **once**.
///
/// The second half is the one that matters. 44 §2.4's purpose is 「ネットワーク断による『apply成功・
/// 応答喪失』時の二重適用を防ぐ」, and what a client needs after losing a response is *the same
/// receipt* — a second `Receipt` describing the same commit differently would be two documents about
/// one event, which is what a ledger exists to prevent.
#[tokio::test]
async fn the_same_key_twice_returns_the_same_response() {
    let (server, id) = committed("idem_same_key").await;
    let client = server.client();
    let key = [("idempotency-key", "client-chosen-1")];

    let first = client
        .send_with("POST", &format!("/v1/candidates/{id}/commit"), None, &key)
        .await;
    println!(
        "IDEM_FIRST={} bytes={}",
        first.status.as_u16(),
        first.json.to_string().len()
    );
    assert_eq!(first.status.as_u16(), 200, "{}", first.json);

    let second = client
        .send_with("POST", &format!("/v1/candidates/{id}/commit"), None, &key)
        .await;
    println!("IDEM_SECOND={}", second.status.as_u16());
    assert_eq!(second.status.as_u16(), 200);
    assert_eq!(
        second.json, first.json,
        "44 §2.4: 「同一キーでの再送に対して**副作用を再実行せず**同一レスポンスを返す」"
    );

    let proof = client
        .send("GET", &format!("/v1/ledger/proof?leaf={id}"), None)
        .await;
    println!("LEDGER_AFTER_RETRY tree_size={}", proof.json["tree_size"]);
    assert_eq!(
        proof.json["tree_size"], 1,
        "one commit, one leaf — and note that this is 43 T-11's own idempotence rather than the \
         cache's doing (see the module header)"
    );
}

/// 🔴 The one line M6-11 requires, **measured**: the cache is not what prevents a double apply.
///
/// > engine の commit は既に冪等なので、cache が守るのは**応答の同一性**であって副作用ではない
///
/// The same commit is retried with **no key at all**, so nothing is cached and nothing is replayed —
/// and the ledger still has one leaf and the file is still written once. If this test failed, the
/// cache would be load-bearing for correctness and 44 §2.4's paragraph would be describing the wrong
/// mechanism.
#[tokio::test]
async fn without_any_key_a_retried_commit_is_still_applied_once() {
    let (server, id) = committed("idem_no_key").await;
    let client = server.client();

    let first = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    assert_eq!(first.status.as_u16(), 200, "{}", first.json);
    let second = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    println!(
        "NO_KEY_RETRY first={} second={}",
        first.status.as_u16(),
        second.status.as_u16()
    );
    assert_eq!(
        second.status.as_u16(),
        200,
        "43 T-9's idempotency column answers a re-entrant commit with the state it reached: {}",
        second.json
    );
    assert_eq!(second.json["state"], "Committed");
    assert_eq!(server.target_contents(), "after\n");

    let proof = client
        .send("GET", &format!("/v1/ledger/proof?leaf={id}"), None)
        .await;
    assert_eq!(
        proof.json["tree_size"], 1,
        "🔴 one leaf, with no idempotency key anywhere: the exactly-once guarantee is ASM-43-1's, \
         and deleting `crate::idempotency` entirely would not change this number"
    );

    println!(
        "NOTHING_CACHED={}",
        !server.state.idempotency().dir().exists()
    );
    assert!(
        !server.state.idempotency().dir().exists(),
        "a request with no key writes no record: 44 §2.4's cache is keyed on a header the client \
         chose to send"
    );
}

/// 🔴 The same key with a **different body** is `409 IDEMPOTENCY_CONFLICT` (44 §2.4).
///
/// Measured on `undo`, because it is the endpoint 44 gives a body — `{actor}` — and therefore the
/// only one of the two where 「同一キーで異なるリクエスト本文」 is expressible at all. That asymmetry
/// is worth noticing: `commit` takes no body, so its conflict case can only be reached by using one
/// key for two **transformations**, which the `(transformation_id, key)` pair already separates.
#[tokio::test]
async fn one_key_and_two_bodies_is_a_conflict() {
    let (server, id) = committed("idem_conflict").await;
    let client = server.client();
    client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    let key = [("idempotency-key", "one-key")];

    let first = client
        .send_with(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({ "actor": { "Human": { "key": "alice" } } })),
            &key,
        )
        .await;
    println!("UNDO_FIRST={} ", first.status.as_u16());
    assert_eq!(first.status.as_u16(), 200, "{}", first.json);

    let conflicting = client
        .send_with(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({ "actor": { "Human": { "key": "bob" } } })),
            &key,
        )
        .await;
    println!(
        "UNDO_CONFLICT={} {}",
        conflicting.status.as_u16(),
        conflicting.json
    );
    assert_eq!(conflicting.status.as_u16(), 409);
    assert_eq!(conflicting.gx_code(), "IDEMPOTENCY_CONFLICT");
    assert_eq!(conflicting.content_type, "application/problem+json");

    // And the *same* body under the same key is the replay, not the conflict.
    let replayed = client
        .send_with(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({ "actor": { "Human": { "key": "alice" } } })),
            &key,
        )
        .await;
    println!("UNDO_REPLAY={}", replayed.status.as_u16());
    assert_eq!(replayed.status.as_u16(), 200);
    assert_eq!(replayed.json, first.json);
}

/// 🔴 **Why §47 took (b) over (a)** — the record is on disk and a new store reads it.
///
/// > 44 の存在理由が「ネットワーク断による『apply成功・応答喪失』時の二重適用を防ぐ」であり、
/// > **その断は process 再起動を伴いうる**
///
/// A memory cache is empty exactly when the failure it exists for has happened. A **second store
/// object over the same directory** is the closest a single-process test can stand to a restart, and
/// it is the whole of what (b) buys.
#[tokio::test]
async fn the_record_outlives_the_store_that_wrote_it() {
    let (server, id) = committed("idem_durable").await;
    let client = server.client();
    let key = [("idempotency-key", "survives-a-restart")];
    let answered = client
        .send_with("POST", &format!("/v1/candidates/{id}/commit"), None, &key)
        .await;
    assert_eq!(answered.status.as_u16(), 200, "{}", answered.json);

    let index = server.project.join(".gx").join("index");
    let fresh = IdempotencyStore::at(&index);
    let tid = TransformationId(gx_core::Cid::from_text(&id).expect("a gx1 id"));
    let entry = fresh
        .get(&tid, "survives-a-restart", Timestamp(0))
        .expect("readable")
        .expect("a record a second store can read");
    println!(
        "DURABLE_RECORD dir={} status={} bytes={}",
        fresh.dir().display(),
        entry.status,
        entry.response.to_string().len()
    );
    assert_eq!(entry.status, 200);
    assert_eq!(
        entry.response, answered.json,
        "the response a restarted server would replay is the one the first one sent"
    );

    // 🔴 And the directory is `.gx/index/`, which req/56 §2 declares disposable — true only because
    // of the paragraph above: losing it costs a duplicated *response*, never a duplicated *effect*.
    assert!(
        fresh.dir().starts_with(&index),
        "44 §2.4's cache lives under `.gx/index/` (M6-11 採(b)): {}",
        fresh.dir().display()
    );
}

/// A record older than 44 §2.4's 「既定24時間」 is a miss, and an expired read deletes nothing.
#[test]
fn an_expired_record_is_a_miss_and_is_not_swept_on_read() {
    let dir = support::scratch("idem_ttl").join("index");
    let store = IdempotencyStore::at(&dir);
    let tid = TransformationId(gx_core::Cid([3u8; 32]));
    let request = serde_json::json!({ "endpoint": "commit" });
    store
        .put(
            &tid,
            "old",
            &request,
            200,
            &serde_json::json!({ "ok": true }),
            Timestamp(0),
        )
        .expect("put");

    let fresh = store.get(&tid, "old", Timestamp(TTL_NANOS)).expect("get");
    let expired = store
        .get(&tid, "old", Timestamp(TTL_NANOS + 1))
        .expect("get");
    println!(
        "TTL_NANOS={TTL_NANOS} fresh={} expired={}",
        fresh.is_some(),
        expired.is_some()
    );
    assert!(
        fresh.is_some(),
        "exactly at the boundary is still inside it"
    );
    assert!(expired.is_none(), "44 §2.4: 「一定期間（既定24時間）」");
    assert!(
        store.path_of(&tid).exists(),
        "an expired read must not delete: a read that mutates a directory is a read that cannot be \
         retried, and 「一定期間」 is about what may be **answered** from the cache"
    );
}

/// The conflict predicate, at the unit: same body replays, different body refuses.
#[test]
fn the_conflict_is_about_the_request_and_not_the_response() {
    let entry = Entry {
        request: serde_json::json!({ "actor": "alice" }),
        response: serde_json::json!({ "receipt": "…" }),
        status: 200,
        at_unix_nanos: 0,
    };
    let same = replay_or_conflict("k", &entry, &serde_json::json!({ "actor": "alice" }))
        .expect("the same request replays");
    println!("REPLAY status={} body={}", same.0, same.1);
    assert_eq!(same.0, 200);

    let different = replay_or_conflict("k", &entry, &serde_json::json!({ "actor": "bob" }))
        .expect_err("a different request conflicts");
    println!(
        "CONFLICT code={} detail_bytes={}",
        different.code,
        different.detail.len()
    );
    assert_eq!(different.code, "IDEMPOTENCY_CONFLICT");
    assert!(
        different.detail.contains("alice") && different.detail.contains("bob"),
        "🔴 both requests are named. A conflict that says only 「they differ」 leaves an operator \
         holding two clients and no way to tell which one was first: {}",
        different.detail
    );
}
