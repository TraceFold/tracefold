//! 🔴 `GET /stream` — **AC-056**, M6-12's map and the five records that never reach a client.
//!
//! AC-056 逐語 (34): 「購読中に別 client が commit → `event: "committed"` が 1 行配信」. Two of the
//! probes here measure it: [`ac_056_a_subscriber_sees_a_commit_another_client_made`] over the router,
//! and `crates/gx-cli/tests/ac_056.rs` over a real socket with `gx serve`. Two, because they answer
//! different questions — this one asks whether the projection and the reader task are right, and that
//! one asks whether the runtime, the bind and the graceful shutdown are.
//!
//! # 🔴 The retry rule this file runs under
//!
//! req/88 §8.2: 「**stream の test は timing 依存が本質**(51 §13-4)。リトライ許容回数を CI 設定で明示
//! し、それでも落ちるなら flaky でなく実バグ」. The rule, stated once and applied everywhere below:
//!
//! * a subscriber polls the journal every `stream::TICK` (20ms), so a line cannot arrive sooner;
//! * every wait here is a **`tokio::time::timeout` of [`WAIT`]** (2s = 100 ticks) around a single
//!   `frame()`, never a `sleep` followed by an assertion. A timeout that expires is a **failure**,
//!   not a retry;
//! * **retries allowed: 0**. There is no loop that tries again, no `#[ignore]`, and nothing in this
//!   file re-runs a step. A hundred ticks of headroom on a 20ms period is three orders of magnitude
//!   more than the operation needs, so an expiry is a real defect and is reported as one.
//!
//! Chosen this way because a suite that retries turns 「the stream stopped delivering」 into 「the
//! suite took two attempts」, which is the shape that makes a broken audit stream ship.

mod support;

use std::time::Duration;

use gx_api::stream::{self, Cursor, EVENTS, EVENT_MAP, UNPUBLISHED};
use http_body_util::BodyExt;
use support::Server;

/// The one wait constant. 100 ticks of `stream::TICK`; see the module header.
const WAIT: Duration = Duration::from_secs(2);

/// Read the next NDJSON line off a streaming body, or fail.
async fn next_line(body: &mut axum::body::Body) -> serde_json::Value {
    let frame = tokio::time::timeout(WAIT, body.frame())
        .await
        .expect(
            "a line arrived inside the wait (see the module header: 0 retries, an expiry is a bug)",
        )
        .expect("the stream is still open")
        .expect("the frame is readable");
    let bytes = frame.into_data().expect("a data frame");
    let text = String::from_utf8(bytes.to_vec()).expect("utf-8");
    assert!(
        text.ends_with('\n'),
        "44 §1.3/§2.2: NDJSON is one object per **line**; got {text:?}"
    );
    serde_json::from_str(text.trim()).expect("each line is one JSON object")
}

/// Drive a transformation to `Committed` over HTTP and hand back its id.
async fn commit_one(server: &Server, goal: &str) -> String {
    let client = server.client();
    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body(goal, "actor-key")),
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
    id
}

// ---------------------------------------------------------------------------
// AC-056
// ---------------------------------------------------------------------------

/// 🔴 **AC-056** — a subscriber sees a commit another client made, as one line.
///
/// The order is the acceptance criterion's: **subscribe first**, then commit from a second client,
/// then read. A probe that committed first and subscribed afterwards would be measuring the replay of
/// a backlog, which is a different feature (and one this stream also has — see
/// [`a_late_subscriber_receives_the_backlog_and_then_follows`]).
#[tokio::test(flavor = "multi_thread")]
async fn ac_056_a_subscriber_sees_a_commit_another_client_made() {
    let server = Server::new("m6h6_ac056", "before\n");
    let subscriber = server.client();

    // Subscribe from the end: `?since=` with an instant in the future means 「what happens next」.
    let response = subscriber
        .open("/v1/stream?since=2099-01-01T00:00:00Z", &[])
        .await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/x-ndjson"),
        "44 §2.2: 「`Content-Type: application/x-ndjson`」"
    );
    let mut body = response.into_body();

    // A **second** client, as AC-056 asks (「別 client が commit」).
    let id = commit_one(&server, "after\n").await;

    // Everything the commit produced, until `committed` — the criterion names that event.
    let mut seen = Vec::new();
    let committed = loop {
        let line = next_line(&mut body).await;
        let event = line["event"].as_str().unwrap_or_default().to_string();
        seen.push(event.clone());
        if event == "committed" {
            break line;
        }
        assert!(
            seen.len() < 20,
            "no `committed` inside twenty lines: {seen:?}"
        );
    };
    println!("AC056_EVENTS={seen:?}");
    println!("AC056_COMMITTED={committed}");

    assert_eq!(committed["transformation"], serde_json::json!(id));
    assert!(
        committed["data"]["ledger_index"].is_number(),
        "44 §2.2's `committed` data is `{{canonical_cid, ledger_index, enforced}}`: {committed}"
    );
    assert!(committed["data"]["canonical_cid"].is_string());
    assert_eq!(committed["data"]["enforced"], serde_json::json!(true));
    // 44 §2.2's envelope, all four fields plus the cursor (M6H6-2).
    for field in ["event", "at", "transformation", "data", "cursor"] {
        assert!(
            committed.get(field).is_some(),
            "44 §2.2's envelope is missing {field}: {committed}"
        );
    }
    assert!(
        committed["at"].as_str().unwrap_or_default().ends_with('Z'),
        "44 §0: 「日時はRFC 3339」"
    );
}

/// A subscriber that joins late gets what it missed, then keeps following.
///
/// The journal is the backlog, so 「everything from the beginning」 is the default subscription. This
/// is also the probe that shows the ordering is the journal's rather than the table's (`BTreeMap` by
/// `TransformationId` = CID order = arbitrary in time, req/88 M6-05).
#[tokio::test(flavor = "multi_thread")]
async fn a_late_subscriber_receives_the_backlog_and_then_follows() {
    let server = Server::new("m6h6_backlog", "before\n");
    let first = commit_one(&server, "after\n").await;

    let response = server.client().open("/v1/stream", &[]).await;
    let mut body = response.into_body();

    let opening = next_line(&mut body).await;
    println!("BACKLOG_FIRST={opening}");
    assert_eq!(
        opening["event"],
        serde_json::json!("candidate.created"),
        "the first event of the first transformation, not the newest"
    );
    assert_eq!(opening["transformation"], serde_json::json!(first));
    assert_eq!(
        opening["cursor"],
        serde_json::json!(Cursor {
            record: 1,
            ordinal: 0
        }
        .to_text()),
        "🔴 the cursor sits at the **journal record index** (M6-13 採(a)) and the candidate is the \
         second record — `DraftCreated` is index 0 and publishes nothing. Since M6H8-12 the wire \
         form is the opaque wrapping of that coordinate rather than the coordinate itself, so the \
         expectation is written as the wrapping of 1.0 and not as a literal: a test that spelled the \
         text would be a second implementation of `to_text`"
    );
    assert!(
        !opening["cursor"].as_str().expect("a cursor").contains('.'),
        "M6H8-12: `<record>.<ordinal>` reached a client"
    );
}

/// 🔴 Resume: `Last-Event-ID` delivers what came **after** the named line and nothing before it.
#[tokio::test(flavor = "multi_thread")]
async fn resume_from_a_cursor_delivers_only_what_followed_it() {
    let server = Server::new("m6h6_resume", "before\n");
    commit_one(&server, "after\n").await;

    // Read two lines, remember the second one's cursor.
    let mut body = server.client().open("/v1/stream", &[]).await.into_body();
    let _first = next_line(&mut body).await;
    let second = next_line(&mut body).await;
    let cursor = second["cursor"].as_str().expect("a cursor").to_string();
    let third = next_line(&mut body).await;
    drop(body);

    let mut resumed = server
        .client()
        .open("/v1/stream", &[("last-event-id", &cursor)])
        .await
        .into_body();
    let after = next_line(&mut resumed).await;
    println!(
        "RESUME_CURSOR={cursor} NEXT={} EXPECTED={}",
        after["event"], third["event"]
    );
    assert_eq!(
        after, third,
        "44 §2.2: 「`Last-Event-ID` ヘッダで途切れた位置から再送可能」 — the line after the cursor, \
         and the same line the first subscription saw there"
    );
}

/// 🔴 `?since=<ledger_index>` names a committed transformation, and says so when it cannot.
///
/// req/88 M6-13's finding, as two assertions: the ledger index **works** for what INV-S4 puts on the
/// ledger, and **refuses** for an index nothing carries — rather than silently starting from the
/// beginning, which would re-deliver a stream the client already has.
#[tokio::test(flavor = "multi_thread")]
async fn since_a_ledger_index_resolves_and_an_unknown_one_is_refused() {
    let server = Server::new("m6h6_since_ledger", "before\n");
    commit_one(&server, "after\n").await;

    // The index this fixture actually produced, read off the stream rather than assumed.
    let index = {
        let mut body = server.client().open("/v1/stream", &[]).await.into_body();
        loop {
            let line = next_line(&mut body).await;
            if line["event"] == serde_json::json!("committed") {
                break line["data"]["ledger_index"].as_u64().expect("an index");
            }
        }
    };

    // 🔴 `?since=<ledger_index>` resolves to **after** that commit — the whole record, its receipt
    // included, because a client naming that index has by definition seen the commit it names. So a
    // subscription taken here delivers nothing until something new happens, and then delivers that.
    let known = server
        .client()
        .open(&format!("/v1/stream?since={index}"), &[])
        .await;
    assert_eq!(known.status(), 200);
    let mut body = known.into_body();
    let second = commit_one(&server, "again\n").await;
    let line = next_line(&mut body).await;
    println!("SINCE_LEDGER_{index}_NEXT={line}");
    assert_eq!(
        line["event"],
        serde_json::json!("candidate.created"),
        "the first event of the **next** transformation, not a replay of the one the index names"
    );
    assert_eq!(line["transformation"], serde_json::json!(second));

    let unknown = server
        .client()
        .send("GET", "/v1/stream?since=99", None)
        .await;
    println!(
        "SINCE_UNKNOWN status={} code={}",
        unknown.status,
        unknown.gx_code()
    );
    assert_eq!(unknown.status, 422);
    assert!(
        unknown.json["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("INV-S4"),
        "the refusal explains what a ledger index cannot name (M6-13): {:?}",
        unknown.json
    );
}

// ---------------------------------------------------------------------------
// M6-12 — the map, and the five that stay in
// ---------------------------------------------------------------------------

/// 🔴 **M6-12** — thirteen journal kinds, ten events, five unpublished.
///
/// The compile-time half (`stream::MAP_COVERS_THE_JOURNAL`) already guarantees the rows **are** the
/// journal's kinds in order. This is the other direction and the arithmetic: every event 44 §2.2
/// lists has a producer, every row's events are inside that vocabulary, and the number that stays
/// inside is five rather than req/88's four (M6H6-1).
#[test]
fn the_map_covers_both_vocabularies_and_five_records_stay_inside() {
    let () = stream::MAP_COVERS_THE_JOURNAL;
    let published: Vec<&str> = EVENT_MAP
        .iter()
        .filter(|row| row.published())
        .map(|row| row.record)
        .collect();
    let internal: Vec<&str> = EVENT_MAP
        .iter()
        .filter(|row| !row.published())
        .map(|row| row.record)
        .collect();
    println!(
        "JOURNAL_KINDS={} EVENTS={} PUBLISHED_RECORDS={} UNPUBLISHED={} \nUNPUBLISHED_SET={internal:?}",
        gx_engine::store::JOURNAL_RECORD_KINDS.len(),
        EVENTS.len(),
        published.len(),
        UNPUBLISHED
    );

    assert_eq!(EVENT_MAP.len(), 13);
    assert_eq!(EVENTS.len(), 10);
    assert_eq!(
        internal,
        vec![
            "DraftCreated",
            "CommittingStarted",
            "ProvenanceDerived",
            "InverseEscrowed",
            "ApplyStarted",
        ],
        "🔴 **M6H6-1** — req/88 §4 M6-12 named 「`Planned`・`ProvenanceDerived`・`InverseEscrowed`・\
         `ApplyStarted`」 and the brief hedged it (「実物は 42 §3.13 と突合して確定」). `Planned` is \
         `candidate.created`'s producer (43 §1's `Candidate` begins at T-2, and it is the only record \
         carrying the locator E-M5-13 added); `DraftCreated` cannot fill 44 §2.2's envelope at all \
         (no `TransformationId`, E-M5-3) and `CommittingStarted` is the inside of T-9's section"
    );
    assert_eq!(UNPUBLISHED, 5);

    // Every event 44 §2.2 lists has at least one producer.
    for event in EVENTS {
        assert!(
            EVENT_MAP.iter().any(|row| row.events.contains(&event)),
            "44 §2.2 lists `{event}` and no journal record produces it"
        );
    }
    // And no row invents one 44 does not list.
    for row in EVENT_MAP {
        for event in row.events {
            assert!(
                EVENTS.contains(event),
                "`{}` produces `{event}`, which is not in 44 §2.2's ten",
                row.record
            );
        }
        assert!(
            row.note.len() > 20,
            "every row says why (M6-12's table is a decision per record, not a filter): {}",
            row.record
        );
    }
}

/// 🔴 The five never appear on the wire — measured on a **full pipeline**, not on the table.
///
/// The table above says they should not; this drives a transformation from `POST /candidates` to
/// `Committed`, collects every line the stream produced, and checks that the count of published
/// events equals the count 33 NFR-018 asks for — 「イベント種別ごとに対応する状態遷移の発生回数と
/// 出力行数が一致することを確認する」 — with the journal itself as the denominator.
#[tokio::test(flavor = "multi_thread")]
async fn no_line_ever_carries_a_record_the_map_keeps_inside() {
    let server = Server::new("m6h6_unpublished", "before\n");
    commit_one(&server, "after\n").await;

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

    let journal_kinds: Vec<&str> = {
        let engine = server.state.engine();
        engine
            .journal()
            .records()
            .iter()
            .map(gx_engine::store::EngineJournalRecord::kind)
            .collect()
    };
    let events: Vec<String> = lines
        .iter()
        .map(|l| l["event"].as_str().unwrap_or_default().to_string())
        .collect();
    println!("PIPELINE_JOURNAL={journal_kinds:?}");
    println!("PIPELINE_EVENTS={events:?}");

    // 🔴 **NFR-018's equality**: 「イベント種別ごとに対応する状態遷移の発生回数と出力行数が一致する
    // ことを確認する」. The denominator of an event is the number of records that produce it, which is
    // a **sum** for the one event with three producers — and that number is the second thing req/88
    // M6-12 got wrong: it counted two producers for `receipt.issued` (T-4's verdict receipt and
    // T-11's commit receipt) where 43 T-5's ruling issues a third.
    for event in EVENTS {
        let transitions: usize = EVENT_MAP
            .iter()
            .filter(|row| row.events.contains(&event))
            .map(|row| journal_kinds.iter().filter(|k| **k == row.record).count())
            .sum();
        let arrived = events.iter().filter(|e| *e == event).count();
        println!("NFR018 {event}: lines={arrived} transitions={transitions}");
        assert!(
            arrived <= transitions,
            "NFR-018: `{event}` arrived {arrived} times for {transitions} transitions"
        );
        // The unconditional ones are an **equality**. The two that are not are conditional inside
        // their own row and the map says so: `escalated` needs a `Verdict::Escalate`, and
        // `receipt.issued` needs a receipt to have been issued at all (T-4e issues none, E-M5-11).
        if !["escalated", "receipt.issued"].contains(&event) {
            assert_eq!(
                arrived, transitions,
                "NFR-018 asks for equality on `{event}`"
            );
        }
    }
    assert!(
        events.iter().all(|e| EVENTS.contains(&e.as_str())),
        "a line carried an event outside 44 §2.2's ten: {events:?}"
    );
    // The five, by name, on a run that produced all of them in the journal.
    for internal in [
        "CommittingStarted",
        "ProvenanceDerived",
        "InverseEscrowed",
        "ApplyStarted",
        "DraftCreated",
    ] {
        assert!(
            journal_kinds.contains(&internal),
            "the pipeline did not write a `{internal}`, so this run cannot show it stayed inside"
        );
        assert!(
            !events.iter().any(|e| e.eq_ignore_ascii_case(internal)),
            "42 §3.13: 「Journalはengine内部の…外部公開しない」 — `{internal}` reached a client"
        );
    }
}

/// A cursor round-trips through its text form, and orders the way resume needs.
///
/// 🔴 **M6H8-12 採(a)** (req/38 §55) made the text form opaque, so this probe now measures three
/// things rather than two: the round trip, the ordering, and the **absence of the coordinate** from
/// the wire. Until this batch `to_text()` was `"12.2"` — the journal record index in the clear,
/// against 44 §2.7's 「opaque」 and 42 §3.13's 「外部公開しない」.
#[test]
fn the_cursor_is_opaque_ordered_and_refuses_what_it_did_not_issue() {
    let cursor = Cursor {
        record: 12,
        ordinal: 2,
    };
    let text = cursor.to_text();
    println!("CURSOR_TEXT={text}");
    assert_eq!(Cursor::parse(&text).expect("round trip"), cursor);
    assert_eq!(
        text.len(),
        32,
        "128 bits of hex, one width for every cursor"
    );
    assert!(
        text.chars().all(|c| c.is_ascii_hexdigit()),
        "the wire form is hex and survives a query string and a header without escaping"
    );
    assert!(
        !text.contains("12") && !text.contains('.'),
        "the coordinate is not readable off the wire: {text}"
    );
    // Adjacent coordinates do not produce adjacent text: a client cannot walk the journal by
    // incrementing what it was given, which is the half of M6H8-12 that is about *invention* rather
    // than about *reading*.
    let neighbour = Cursor {
        record: 12,
        ordinal: 3,
    }
    .to_text();
    assert_ne!(
        text[..8],
        neighbour[..8],
        "the wrapping diffuses the low bit"
    );
    // Every distinct coordinate keeps a distinct text (the multiplication is a bijection).
    let mut seen = std::collections::BTreeSet::new();
    for record in 0..64usize {
        for ordinal in 0..8usize {
            assert!(
                seen.insert(Cursor { record, ordinal }.to_text()),
                "collision"
            );
        }
    }
    assert!(
        Cursor {
            record: 12,
            ordinal: 1
        } < cursor,
        "two events of one record are ordered by ordinal — without it a client that died between \
         `verdict.issued` and its `receipt.issued` would have to replay one or lose the other"
    );
    assert!(
        Cursor {
            record: 11,
            ordinal: 9
        } < cursor
    );
    for bad in ["12", "twelve.2", "12.x", ""] {
        assert!(
            Cursor::parse(bad).is_err(),
            "`{bad}` is not a cursor this server issued"
        );
    }
}
