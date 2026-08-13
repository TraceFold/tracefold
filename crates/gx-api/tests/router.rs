//! What the router serves, and what it deliberately does not.
//!
//! Hand 1 wrote this file to measure a **zero**: 「a crate that exists and does nothing is a crate a
//! reader has to rule out, so the zero is stated as a number the next hand has to change
//! deliberately」. This is that hand, and the number is thirteen.

mod support;

use gx_api::list::EXTENSION_ENDPOINTS;
use gx_api::{HAND6_ENDPOINTS, IMPLEMENTED_ENDPOINTS, SPECIFIED_ENDPOINTS};
use support::Server;

/// 🔴 44 §2.1's fourteen are named, **all fourteen** are served, and the four extensions are named.
///
/// Both halves, because either alone is hollow: a count of implementations means nothing without a
/// list of what is not implemented, and a list means nothing if nobody checks it against the count.
///
/// # 🔴 The count req/88 §6.2 writes is **eleven** and this is thirteen (M6H5-1)
///
/// 「44 §2 の同期面 11 endpoint(`/stream` と一覧系を除く全部)」. Fourteen minus `/stream` is thirteen,
/// and 44 §2.7 says the list endpoints do not exist in 44 at all — 「本書指定のエンドポイント群には
/// 一覧（list）系エンドポイントは含まれない」 — so M6-05's three are hand 6's **additions** and cannot
/// be subtracted from a count of 44's own rows. The parenthetical is the operative text; the numeral
/// is an enumeration error, raised under 規律49's lane-side reconciliation.
#[test]
fn all_fourteen_of_44s_rows_are_served_and_the_four_extensions_are_named() {
    println!(
        "SPECIFIED_ENDPOINTS={} IMPLEMENTED_ENDPOINTS={IMPLEMENTED_ENDPOINTS} \
         HAND6_ENDPOINTS={HAND6_ENDPOINTS:?}",
        SPECIFIED_ENDPOINTS.len()
    );
    assert_eq!(
        SPECIFIED_ENDPOINTS.len(),
        14,
        "44 §2.1 specifies fourteen: {SPECIFIED_ENDPOINTS:?}"
    );
    assert_eq!(
        IMPLEMENTED_ENDPOINTS,
        SPECIFIED_ENDPOINTS.len(),
        "hand 6 closes the list: every row of 44 §2.1 is served"
    );
    for endpoint in HAND6_ENDPOINTS {
        assert!(
            SPECIFIED_ENDPOINTS.contains(&endpoint),
            "{endpoint} was hand 6's and is not a row of 44 §2.1"
        );
    }
    // 🔴 The four M6-05 adds are **not** rows of 44, and the two lists must not blur into one: 44
    // §2.7 is explicit that 44 has no list endpoints, so an extension that crept into
    // `SPECIFIED_ENDPOINTS` would turn an addition into a citation.
    for endpoint in EXTENSION_ENDPOINTS {
        assert!(
            !SPECIFIED_ENDPOINTS.contains(&endpoint),
            "{endpoint} is an M6-05 extension (44 §2.6's 「新規エンドポイント」); 44 §2.7 says 44              specifies no list endpoints, so it may not be listed as one of 44's own"
        );
    }
    for endpoint in SPECIFIED_ENDPOINTS {
        let (method, path) = endpoint.split_once(' ').expect("METHOD /path");
        assert!(
            ["GET", "POST"].contains(&method),
            "44 §2 uses GET and POST only; {endpoint} says {method}"
        );
        assert!(path.starts_with('/'), "{endpoint} is not a path");
    }
}

/// 🔴 Every row of 44 §2.1 **answers**, and so do M6-05's four extensions.
///
/// The strongest available statement about a router's shape without a socket: each path is sent a
/// request and the answer is read. What it proves is that the route exists — a 404 from a served
/// path would be a route that was declared in the list above and never wired — and what it proves
/// about `/stream` is that hand 5's placeholder 404 is gone.
///
/// Statuses are **not** asserted here beyond 「not 404」: that is what `endpoints.rs` is for, and a
/// probe asserting both would be two claims one failure could not tell apart.
#[tokio::test]
async fn every_served_path_answers_including_stream_and_the_four_extensions() {
    let server = Server::new("router_paths", "before\n");
    let client = server.client();
    // A transformation id that parses and names nothing: routing is what is under test, so the
    // handler is expected to refuse — with 404 NOT_FOUND from the **handler**, which is
    // indistinguishable from an unrouted path by status alone. `gx_code` is what tells them apart:
    // an unrouted path in axum answers with an empty body and no `gx_code` at all.
    let id = "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let probes: [(&str, String); 17] = [
        ("POST", "/v1/candidates".to_string()),
        ("GET", format!("/v1/candidates/{id}")),
        ("POST", format!("/v1/candidates/{id}/verify")),
        ("POST", format!("/v1/candidates/{id}/commit")),
        ("POST", format!("/v1/candidates/{id}/escalation")),
        ("POST", format!("/v1/candidates/{id}/cancel")),
        ("POST", format!("/v1/transformations/{id}/undo")),
        ("POST", format!("/v1/transformations/{id}/replay")),
        ("GET", format!("/v1/transformations/{id}")),
        ("GET", format!("/v1/receipts/{id}")),
        ("GET", "/v1/ledger/proof?leaf=0".to_string()),
        ("GET", "/v1/ledger/checkpoint".to_string()),
        ("GET", "/v1/healthz".to_string()),
        // Hand 6: 44 §2.2's fourteenth row is measured by `stream.rs` (a body that never ends is not
        // a body this client can drain), and the four extensions are measured here.
        ("GET", "/v1/candidates?limit=1".to_string()),
        ("GET", "/v1/escalations?limit=1".to_string()),
        ("GET", "/v1/transformations?limit=1".to_string()),
        ("GET", "/v1/ledger/consistency?from=0&to=0".to_string()),
    ];
    let mut routed = 0usize;
    for (method, path) in &probes {
        let answer = client.send(method, path, Some(serde_json::json!({}))).await;
        let has_body = !answer.json.is_null();
        println!(
            "ROUTE {method} {path} -> {} body={has_body} gx_code={:?}",
            answer.status.as_u16(),
            answer.gx_code()
        );
        assert_ne!(
            answer.status.as_u16(),
            405,
            "{method} {path} is routed to a different method"
        );
        assert!(
            has_body,
            "{method} {path} answered with no body at all, which is what an unrouted path does"
        );
        routed += 1;
    }
    println!(
        "ROUTED={routed} DECLARED={IMPLEMENTED_ENDPOINTS} EXTENSIONS={}",
        EXTENSION_ENDPOINTS.len()
    );
    assert_eq!(
        routed,
        IMPLEMENTED_ENDPOINTS - 1 + EXTENSION_ENDPOINTS.len(),
        "thirteen of 44's fourteen (the fourteenth is `/stream`, below) plus M6-05's four extensions"
    );

    // 🔴 `/stream` **is** routed now, and hand 5's claim here — 「a route that answered today would
    // be four rulings made by accident」 — is invalidated deliberately: M6-05, M6-06, M6-12 and
    // M6-13 are ruled in §47 and implemented in this hand. The assertion is on the header rather
    // than the body, because `send` drains what it reads and this body does not end (`stream.rs`
    // reads it a frame at a time).
    let stream = client
        .open("/v1/stream?since=2099-01-01T00:00:00Z", &[])
        .await;
    println!(
        "STREAM_STATUS={} CONTENT_TYPE={:?}",
        stream.status().as_u16(),
        stream.headers().get("content-type")
    );
    assert_eq!(stream.status().as_u16(), 200);
}

/// `/v1` is the base path, and a request without it is not routed (44 §2: 「Base path: `/v1`」).
#[tokio::test]
async fn the_base_path_is_v1() {
    let server = Server::new("router_base_path", "before\n");
    let client = server.client();
    let with = client.send("GET", "/v1/healthz", None).await;
    let without = client.send("GET", "/healthz", None).await;
    println!(
        "BASE_PATH v1={} bare={}",
        with.status.as_u16(),
        without.status.as_u16()
    );
    assert_eq!(with.status.as_u16(), 200);
    assert_eq!(
        without.status.as_u16(),
        404,
        "44 §2 fixes `/v1`, and 44 §2.6 makes it the thing a breaking change moves"
    );
}
