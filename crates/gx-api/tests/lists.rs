//! 🔴 **M6-05** — the three lists, the fourth extension, and the one coordinate they share with
//! `GET /stream`.
//!
//! 44 §2.7 reserves the regulation and specifies no list endpoints; §47 M6-05 採(a) adds them as 44
//! extensions and fixes the ordering: 「**cursor=journal 順**(M6-13 と共有・表順=CID 順は時間について
//! 任意)」. The sharpest probe in this file is [`the_list_cursor_is_the_stream_cursor`], because that
//! sentence is a claim about **two features being one thing** and is otherwise only a comment.

mod support;

use gx_api::list::{DEFAULT_LIMIT, EXTENSION_ENDPOINTS, MAX_LIMIT};
use support::Server;

/// Drive `n` transformations to `Committed`, newest last, and hand back their ids in order.
async fn commit_many(server: &Server, n: usize) -> Vec<String> {
    let client = server.client();
    let mut ids = Vec::new();
    for i in 0..n {
        let goal = format!("change {i}\n");
        let created = client
            .send(
                "POST",
                "/v1/candidates",
                Some(server.intent_body(&goal, "actor-key")),
            )
            .await;
        assert_eq!(created.status, 201, "{:?}", created.json);
        let id = created.json["id"]
            .as_str()
            .expect("44 §2.2: `POST /candidates` answers with the id it fixed")
            .to_string();
        let verified = client
            .send("POST", &format!("/v1/candidates/{id}/verify"), None)
            .await;
        assert_eq!(verified.status, 200, "{:?}", verified.json);
        let committed = client
            .send("POST", &format!("/v1/candidates/{id}/commit"), None)
            .await;
        assert_eq!(committed.status, 200, "{:?}", committed.json);
        ids.push(id);
    }
    ids
}

/// 🔴 `GET /transformations` lists every row in **journal order**, which is not the table's order.
///
/// req/88 M6-05 is the reason this is a probe rather than a detail: 「engine の表は
/// `BTreeMap<TransformationId, _>`=**TID 順(=CID 順=実質ランダム)であって時間順ではない**」. A list
/// that came out of the table would be sorted by content address, so 「the newest change」 would be
/// wherever its digest happened to fall. Three commits, in a known order, are enough to tell the two
/// apart — and the probe says so by asserting the order **is** the order they were made in.
#[tokio::test(flavor = "multi_thread")]
async fn transformations_are_listed_in_the_order_they_happened() {
    let server = Server::new("m6h6_list_order", "before\n");
    // 🔴 **Five**, not three, and the reason is the assertion at the bottom of this probe: the two
    // orders have to be **different** for the comparison above to mean anything. Three ids fall into
    // CID order by chance one time in six (measured: this fixture's first three did), and five make
    // it one in a hundred and twenty — deterministically, since the goals are fixed.
    let ids = commit_many(&server, 5).await;

    let page = server
        .client()
        .send("GET", "/v1/transformations", None)
        .await;
    assert_eq!(page.status, 200, "{:?}", page.json);
    let listed: Vec<String> = page.json["items"]
        .as_array()
        .expect("44 §2.7: 「`{ items: [...], next_cursor: … }`」")
        .iter()
        .map(|row| {
            row["transformation"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    let mut by_cid = ids.clone();
    by_cid.sort();
    println!("LIST_ORDER={listed:?}\nHAPPENED={ids:?}\nBY_CID={by_cid:?}");

    assert_eq!(listed, ids, "M6-05: the order is the journal's");
    assert_ne!(
        by_cid, ids,
        "this fixture cannot tell the two orders apart — the ids happen to sort into the order they \
         were made in, so the assertion above would pass for a table-ordered list too. Change the \
         goals until they differ (a probe that cannot fail is not a probe)"
    );
    assert_eq!(page.json["next_cursor"], serde_json::Value::Null);
}

/// 🔴 The list cursor **is** the stream cursor — M6-05 and M6-13 are one decision.
///
/// Take one page of one item, hand its `next_cursor` to `GET /stream`, and the first line that
/// arrives is the first event after that transformation's list position. If the two coordinates were
/// different spaces this would deliver something arbitrary, and nothing else in either suite would
/// notice.
#[tokio::test(flavor = "multi_thread")]
async fn the_list_cursor_is_the_stream_cursor() {
    let server = Server::new("m6h6_one_coordinate", "before\n");
    let ids = commit_many(&server, 2).await;

    let first_page = server
        .client()
        .send("GET", "/v1/transformations?limit=1", None)
        .await;
    let cursor = first_page.json["next_cursor"]
        .as_str()
        .expect("a full page carries a cursor")
        .to_string();
    assert_eq!(
        first_page.json["items"][0]["transformation"],
        serde_json::json!(ids[0])
    );

    // The same string, in the other feature's query.
    let response = server
        .client()
        .open("/v1/stream", &[("last-event-id", &cursor)])
        .await;
    assert_eq!(response.status(), 200);
    let mut body = response.into_body();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(2), {
        use http_body_util::BodyExt;
        body.frame()
    })
    .await
    .expect("a line inside the wait")
    .expect("the stream is open")
    .expect("a readable frame");
    let line: serde_json::Value =
        serde_json::from_slice(frame.into_data().expect("data").as_ref()).expect("json");
    println!("SHARED_CURSOR={cursor} FIRST_LINE={line}");

    assert_eq!(
        line["event"],
        serde_json::json!("verify.started"),
        "🔴 one coordinate: the cursor of the **first** transformation's list position is its \
         `Planned` record, so the first event after it is the `verify.started` of that same \
         transformation. A second coordinate space would answer with something else"
    );
    assert_eq!(line["transformation"], serde_json::json!(ids[0]));

    // The second page picks up where the first left off, from the same string.
    let second_page = server
        .client()
        .send(
            "GET",
            &format!("/v1/transformations?limit=1&cursor={cursor}"),
            None,
        )
        .await;
    assert_eq!(
        second_page.json["items"][0]["transformation"],
        serde_json::json!(ids[1])
    );
}

/// 🔴 `GET /candidates` shows what is in flight and drops what is finished.
#[tokio::test(flavor = "multi_thread")]
async fn candidates_lists_the_unfinished_and_transformations_lists_everything() {
    let server = Server::new("m6h6_list_candidates", "before\n");
    let committed = commit_many(&server, 1).await;

    // A second transformation that stops at `Candidate`.
    let created = server
        .client()
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("in flight\n", "actor-key")),
        )
        .await;
    let waiting = created.json["id"]
        .as_str()
        .expect("44 §2.2: `POST /candidates` answers with the id it fixed")
        .to_string();

    let in_flight = server.client().send("GET", "/v1/candidates", None).await;
    let everything = server
        .client()
        .send("GET", "/v1/transformations", None)
        .await;
    let names = |page: &serde_json::Value| -> Vec<String> {
        page["items"]
            .as_array()
            .expect("items")
            .iter()
            .map(|r| r["transformation"].as_str().unwrap_or_default().to_string())
            .collect()
    };
    println!(
        "CANDIDATES={:?}\nTRANSFORMATIONS={:?}\nCOMMITTED={committed:?} WAITING={waiting}",
        names(&in_flight.json),
        names(&everything.json)
    );
    assert_eq!(names(&in_flight.json), vec![waiting.clone()]);
    assert_eq!(names(&everything.json).len(), 2);
    assert!(names(&everything.json).contains(&committed[0]));
    assert_eq!(
        in_flight.json["items"][0]["state"],
        serde_json::json!("Candidate")
    );
}

/// 🔴 `GET /escalations` is the only way to find a ticket, so it has to find one.
///
/// 44 §1.2's `gx escalation approve <TICKET_ID>` and `POST /candidates/{id}/escalation` both consume
/// a ticket and 43 T-4c's 「人間へ通知」 has no implementation in v0.1. The empty answer is asserted
/// too: a list that reported an escalation for a transformation nobody escalated would be worse than
/// no list at all.
#[tokio::test(flavor = "multi_thread")]
async fn escalations_is_empty_until_something_escalates() {
    let server = Server::new("m6h6_list_escalations", "before\n");
    commit_many(&server, 1).await;
    let page = server.client().send("GET", "/v1/escalations", None).await;
    println!("ESCALATIONS={}", page.json);
    assert_eq!(page.status, 200);
    assert_eq!(
        page.json["items"].as_array().map(Vec::len),
        Some(0),
        "the shipped pack admits this change, so nothing is waiting for a person"
    );
    assert_eq!(page.json["next_cursor"], serde_json::Value::Null);
}

/// 🔴 44 §2.7's limit is a **maximum**, and exceeding it is refused rather than clamped.
///
/// A clamp would answer a request for a thousand rows with two hundred and say nothing, so a client
/// paging by a thousand would see a fifth of the ledger and believe it had seen all of it. That is
/// the 「skip と pass を同じ顔にするな」 shape (req/29 §4) in a pagination parameter.
#[tokio::test(flavor = "multi_thread")]
async fn the_limit_is_44s_and_a_limit_outside_it_is_refused() {
    let server = Server::new("m6h6_list_limit", "before\n");
    commit_many(&server, 2).await;
    let client = server.client();

    let defaulted = client.send("GET", "/v1/transformations", None).await;
    assert_eq!(defaulted.status, 200);
    println!("DEFAULT_LIMIT={DEFAULT_LIMIT} MAX_LIMIT={MAX_LIMIT}");
    assert_eq!(DEFAULT_LIMIT, 50, "44 §2.7: 「既定50」");
    assert_eq!(MAX_LIMIT, 200, "44 §2.7: 「最大200」");

    for bad in ["0", "201", "9999"] {
        let refused = client
            .send("GET", &format!("/v1/transformations?limit={bad}"), None)
            .await;
        println!(
            "LIMIT_{bad}_STATUS={} code={}",
            refused.status,
            refused.gx_code()
        );
        assert_eq!(refused.status, 422, "limit={bad}");
        assert_eq!(refused.gx_code(), "VALIDATION_ERROR");
    }

    let bad_cursor = client
        .send("GET", "/v1/transformations?cursor=nonsense", None)
        .await;
    assert_eq!(bad_cursor.status, 422, "a cursor this server did not issue");
}

/// 🔴 `GET /ledger/consistency` — the CLI's twin, which 44 gives only to `gx log consistency`.
#[tokio::test(flavor = "multi_thread")]
async fn ledger_consistency_answers_over_http_as_it_does_on_the_command_line() {
    let server = Server::new("m6h6_consistency", "before\n");
    commit_many(&server, 2).await;
    let client = server.client();

    let proof = client
        .send("GET", "/v1/ledger/consistency?from=1&to=2", None)
        .await;
    println!("CONSISTENCY_1_2={}", proof.json);
    assert_eq!(proof.status, 200, "{:?}", proof.json);
    assert_eq!(proof.json["from"], serde_json::json!(1));
    assert_eq!(proof.json["to"], serde_json::json!(2));
    assert!(
        proof.json["proof"].is_object() || proof.json["proof"].is_array(),
        "the proof is the value gx-log produced: {}",
        proof.json["proof"]
    );

    let backwards = client
        .send("GET", "/v1/ledger/consistency?from=2&to=1", None)
        .await;
    println!(
        "CONSISTENCY_BACKWARDS status={} code={}",
        backwards.status,
        backwards.gx_code()
    );
    assert_ne!(
        backwards.status, 200,
        "a consistency proof from a larger tree to a smaller one is not a proof"
    );
}

/// The four extensions are named as extensions, and every one of them is a `GET`.
#[test]
fn the_extensions_are_declared_and_are_all_reads() {
    println!("EXTENSION_ENDPOINTS={EXTENSION_ENDPOINTS:?}");
    assert_eq!(EXTENSION_ENDPOINTS.len(), 4);
    for endpoint in EXTENSION_ENDPOINTS {
        assert!(
            endpoint.starts_with("GET /"),
            "44 §2.6 permits 「後方互換な追加」 and a list that changed state would not be one: \
             {endpoint}"
        );
    }
}
