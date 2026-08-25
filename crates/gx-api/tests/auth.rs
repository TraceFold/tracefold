// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **M6-10** — 44 §2.5's Bearer, on every route but one, and the absence said out loud.
//!
//! Three claims, and the third is the one a reviewer should read first:
//!
//! 1. every route but `/healthz` refuses an unauthenticated request;
//! 2. `/healthz` does not (44 §2.6: "no authentication required"; sem: SEM-gx-api-465);
//! 3. **what the check does not do** is written where an operator meets it.

mod support;

use gx_api::auth::{bind_refusal, Bearer, ABSENCE_NOTICE, DEFAULT_BIND};
use support::{Server, TOKEN};

/// 🔴 The paths a request can reach, **derived from the router** (M6H8-9, adopted (a), req/38 §55; sem: SEM-gx-api-466).
///
/// This used to be a hand-written list of twelve, and hand 8 measured what that cost: `/stream` and
/// M6-05's four list endpoints were added to the guarded block in hand 6 and never walked here, so
/// 44 §2.5's "every endpoint (except `/healthz`)" was being checked over a **subset** — "that is how
/// it is structurally arranged" (sem: SEM-gx-api-467) with no assertion behind it. A second table is also a second thing to forget: the one
/// that was forgotten is exactly the one this probe existed to cover.
///
/// So the routes are read out of `crates/gx-api/src/lib.rs`'s `guarded` block, which is where they
/// are declared once. Reading source rather than asking axum is deliberate — axum 0.8's `Router`
/// exposes no route iterator, and the alternative (a second const beside the router) is the second
/// table again.
fn paths(id: &str) -> Vec<(&'static str, String)> {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"),
    )
    .expect("the router's source is readable");
    // 🔴 The **whole** of `router`, not the `guarded` block. 44 §2.5's sentence is "every endpoint
    // (except `/healthz`)" (sem: SEM-gx-api-468), so the set this walk has to cover is every route the function declares
    // minus one — and a route **moved out** of the guard is exactly the mutation that a walk over
    // the guarded block alone could not see (it would shrink and stay green).
    let body = source
        .split("pub fn router(state: AppState) -> axum::Router {")
        .nth(1)
        .expect("`router` is still spelled this way")
        .split("\n}")
        .next()
        .expect("the function is closed");

    let mut out = Vec::new();
    for chunk in body.split(".route(").skip(1) {
        let mut quoted = chunk.split('"');
        let _before = quoted.next();
        let path = quoted.next().expect("a route's first argument is its path");
        let rest = &chunk[chunk.find(path).expect("just found") + path.len()..];
        let method = match (rest.find("get("), rest.find("post(")) {
            (Some(g), Some(p)) if g < p => "GET",
            (Some(_), None) => "GET",
            (_, Some(_)) => "POST",
            (None, None) => panic!("{path} is routed to neither get() nor post()"),
        };
        if path == "/healthz" {
            // 44 §2.6 exempts it by name, and `the_health_check_needs_no_token` measures the
            // exemption. Excluded here rather than filtered by the caller so that the exemption is
            // one line in one place.
            continue;
        }
        out.push((
            method,
            format!("/v1{}", path.replace("{id}", id).replace("{tid}", id)),
        ));
    }
    assert!(
        out.len() >= 17,
        "the derivation found {} routes needing a token; hand 6 left seventeen (twelve plus \
         /stream plus M6-05's four lists), so the parse has stopped seeing the router",
        out.len()
    );
    out
}

const SOME_ID: &str = "gx1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// 🔴 Twelve routes refuse an anonymous request with 44 §2.3's `UNAUTHORIZED`, and `/healthz` does not.
///
/// Walked rather than sampled. 44 §2.5's "every endpoint (except `/healthz`)" (sem: SEM-gx-api-469) is a statement about the
/// **set**, and a suite that checked two of twelve would be checking that the layer exists rather
/// than that it covers — which is the difference between a middleware and a habit.
///
/// The status is asserted **before** the handler could have run: a 401 that arrived after a commit
/// would be a refusal issued too late to be one, so the substrate is compared as well.
#[tokio::test]
async fn every_route_but_healthz_requires_a_bearer_token() {
    let server = Server::new("auth_walk", "before\n");
    let anonymous = server.anonymous();
    let before = server.target_contents();

    let mut refused = 0usize;
    for (method, path) in paths(SOME_ID) {
        let answer = anonymous
            .send(method, &path, Some(serde_json::json!({})))
            .await;
        println!(
            "ANON {method} {path} -> {} gx_code={}",
            answer.status.as_u16(),
            answer.gx_code()
        );
        assert_eq!(
            answer.status.as_u16(),
            401,
            "44 §2.5 makes the token required on {method} {path}"
        );
        assert_eq!(answer.gx_code(), "UNAUTHORIZED", "44 §2.3's code for it");
        assert_eq!(
            answer.content_type, "application/problem+json",
            "44 §2.3: \"every error response is `Content-Type: application/problem+json`\" (sem: SEM-gx-api-470)"
        );
        refused += 1;
    }
    println!("ANON_REFUSED={refused}");
    let derived = paths(SOME_ID);
    assert_eq!(
        refused,
        derived.len(),
        "every route the router puts behind the guard was walked (M6H8-9)"
    );
    // The five hand 6 added and nobody walked until this batch, named so that a derivation which
    // silently stopped finding them would be red rather than smaller.
    for late in [
        "/v1/stream",
        "/v1/candidates",
        "/v1/escalations",
        "/v1/transformations",
        "/v1/ledger/consistency",
    ] {
        assert!(
            derived.iter().any(|(_, path)| path == late),
            "{late} is served behind the guard and is not in the derived walk"
        );
    }
    assert_eq!(
        server.target_contents(),
        before,
        "a refused request touches nothing on the substrate"
    );

    let health = anonymous.send("GET", "/v1/healthz", None).await;
    println!("ANON_HEALTHZ={} {}", health.status.as_u16(), health.json);
    assert_eq!(
        health.status.as_u16(),
        200,
        "44 §2.6: \"`GET /healthz` … no authentication required\" (sem: SEM-gx-api-471)"
    );
    assert_eq!(health.json["status"], "ok");
    assert!(health.json["engine_version"].is_string());
}

/// A **wrong** token is refused the same way a missing one is.
///
/// One answer for three failures, deliberately: a message naming which part was wrong tells a prober
/// how to get closer. gx-witness's `SignatureInvalid` makes the identical argument about four.
#[tokio::test]
async fn a_wrong_token_is_refused_like_a_missing_one() {
    let server = Server::new("auth_wrong_token", "before\n");
    let client = server.client();
    let good = client.send("GET", "/v1/healthz", None).await;
    assert_eq!(good.status.as_u16(), 200);

    // The same path a valid token reaches, with a token one byte different.
    let mut wrong = TOKEN.to_string();
    wrong.push('x');
    let answer = server
        .anonymous()
        .send_with(
            "GET",
            &format!("/v1/candidates/{SOME_ID}"),
            None,
            &[("authorization", &format!("Bearer {wrong}"))],
        )
        .await;
    println!("WRONG_TOKEN={} {}", answer.status.as_u16(), answer.json);
    assert_eq!(answer.status.as_u16(), 401);
    assert_eq!(answer.gx_code(), "UNAUTHORIZED");

    // A scheme that is not `Bearer` is the third of the three.
    let scheme = server
        .anonymous()
        .send_with(
            "GET",
            &format!("/v1/candidates/{SOME_ID}"),
            None,
            &[("authorization", &format!("Basic {TOKEN}"))],
        )
        .await;
    println!("WRONG_SCHEME={}", scheme.status.as_u16());
    assert_eq!(scheme.status.as_u16(), 401);
}

/// 🔴 **AC-P2-2** — an **equal-length** wrong token is refused exactly like every other wrong one.
///
/// `a_wrong_token_is_refused_like_a_missing_one` above sends a token one byte *longer* than the
/// server's, which [`gx_api::auth::Bearer::matches`] rejects on the length check before the
/// constant-time comparison loop ever runs. This is the case that actually exercises that loop: a
/// token the **same** length as [`TOKEN`], differing in one byte, so the request this suite sends is
/// exactly the shape `the_token_comparison_is_over_the_whole_string` proves the primitive handles
/// without an early return.
#[tokio::test]
async fn an_equal_length_wrong_token_is_refused_the_same_way() {
    let server = Server::new("auth_equal_length_wrong_token", "before\n");
    assert_eq!(
        TOKEN.len(),
        15,
        "this test's premise: the fixture token is 15 bytes"
    );

    let mut wrong: Vec<u8> = TOKEN.as_bytes().to_vec();
    wrong[0] = if wrong[0] == b'X' { b'Y' } else { b'X' };
    let wrong = String::from_utf8(wrong).expect("ASCII in, ASCII out");
    assert_eq!(wrong.len(), TOKEN.len(), "equal length, one differing byte");
    assert_ne!(wrong, TOKEN);

    let answer = server
        .anonymous()
        .send_with(
            "GET",
            &format!("/v1/candidates/{SOME_ID}"),
            None,
            &[("authorization", &format!("Bearer {wrong}"))],
        )
        .await;
    println!(
        "EQUAL_LENGTH_WRONG_TOKEN={} gx_code={}",
        answer.status.as_u16(),
        answer.gx_code()
    );
    assert_eq!(
        answer.status.as_u16(),
        401,
        "44 §2.5: a token mismatch is 401 (sem: SEM-gx-api-472)"
    );
    assert_eq!(answer.gx_code(), "UNAUTHORIZED", "44 §2.3's code for it");
}

/// 🔴 **AC-P2-2** — a **valid** token reaches a guarded route and answers `200`.
///
/// The suites above all exercise refusal; this is the positive control §30's ledger asks every
/// absence probe to carry. `GET /v1/escalations` is `lists.rs`'s own minimal guarded route (200
/// with an empty page on a server nothing has been submitted to yet), so this asserts the Bearer
/// layer's success path on a route that is not `/healthz` — the one route 44 §2.6 exempts and that
/// therefore proves nothing about the guard itself.
#[tokio::test]
async fn a_valid_token_reaches_a_guarded_route_and_answers_200() {
    let server = Server::new("auth_valid_token_200", "before\n");
    let answer = server.client().send("GET", "/v1/escalations", None).await;
    println!(
        "VALID_TOKEN={} body={}",
        answer.status.as_u16(),
        answer.json
    );
    assert_eq!(
        answer.status.as_u16(),
        200,
        "44 §2.5: the correct token → 200 (sem: SEM-gx-api-473)"
    );
    assert_eq!(answer.json["items"].as_array().map(Vec::len), Some(0));
}

/// 🔴 The comparison does not return early on the first differing byte.
///
/// v0.1's whole authentication is [`Bearer::matches`], so the one place being careful obviously pays
/// for itself is this one. A behavioural timing assertion would be flaky; what is asserted instead is
/// the property the implementation exists for — a shared prefix is not accepted and does not change
/// the answer — plus the length guard, which is the leak every fixed-token scheme keeps.
#[test]
fn the_token_comparison_is_over_the_whole_string() {
    let bearer = Bearer::new("abcdefgh");
    assert!(bearer.matches("abcdefgh"));
    assert!(!bearer.matches("abcdefgX"), "a differing last byte");
    assert!(!bearer.matches("Xbcdefgh"), "a differing first byte");
    assert!(!bearer.matches("abcdefg"), "a prefix is not the token");
    assert!(!bearer.matches("abcdefghi"), "nor is an extension");
    assert!(!bearer.matches(""), "nor is nothing");
    println!("BEARER_DEBUG={bearer:?}");
    assert!(
        !format!("{bearer:?}").contains("abcdefgh"),
        "the token must not be printable by accident: a secret that is `{{:?}}`-able ends up in a \
         trace the day somebody adds one"
    );
}

/// 🔴 A server started with **no** token answers `INTERNAL`, not `UNAUTHORIZED`.
///
/// 44 §2.5 makes a token required, so an empty one is not a configuration — it is a server that
/// cannot satisfy the specification. Answering 401 would tell an operator "your token is wrong"
/// about a server that has none, which is M4H4-2's "don't give not-implemented and failure the same face" (sem: SEM-gx-api-474) at the deployment
/// layer; and accepting the empty string would be authentication switched off while reporting that
/// it passed.
#[test]
fn an_unset_token_is_a_deployment_fault_and_not_a_refusal() {
    let unset = Bearer::new("");
    println!("BEARER_UNSET={}", unset.is_unset());
    assert!(unset.is_unset());
    assert!(
        !unset.matches(""),
        "and it does not match the empty presented token either, so no request can slip through \
         even if the guard were removed"
    );
}

/// 🔴 **M6-10, adopted (b)** (sem: SEM-gx-api-475) — the default bind is loopback and a public address is refused by name.
///
/// 44 §1.2 gives `gx serve --bind` **no** default, and req/88 M6-10 named the consequence: "implemented
/// while still undefined, `0.0.0.0` can become the default (going onto a public network with no authorization)" (sem: SEM-gx-api-476). The runtime is
/// hand 6's and the policy is this hand's, so that the hand which writes the flag is not also the
/// hand that decides what it defaults to.
#[test]
fn the_default_bind_is_loopback_and_anything_else_is_refused() {
    println!("DEFAULT_BIND={DEFAULT_BIND}");
    assert!(
        DEFAULT_BIND.starts_with("127.0.0.1:"),
        "M6-10, adopted (b): \"default bind = 127.0.0.1, fixed\" (sem: SEM-gx-api-477)"
    );
    assert!(
        bind_refusal(DEFAULT_BIND).is_none(),
        "the default must not refuse itself"
    );
    for loopback in [
        "127.0.0.1:9000",
        "localhost:1",
        "[::1]:8787",
        "127.5.5.5:80",
    ] {
        assert!(
            bind_refusal(loopback).is_none(),
            "{loopback} is loopback and needs no flag"
        );
    }
    for public in [
        "0.0.0.0:8787",
        "10.1.2.3:8787",
        "example.com:443",
        "[::]:80",
    ] {
        let refusal = bind_refusal(public)
            .unwrap_or_else(|| panic!("{public} is not loopback and must be refused"));
        println!("BIND_REFUSED {public}: {} chars", refusal.len());
        assert!(
            refusal.contains("loopback"),
            "the refusal says why, because \"don't hide the check's absence\" (sem: SEM-gx-api-478) means saying it where it bites"
        );
    }
}

/// 🔴 The absence notice exists, says the three things it has to, and is 45 §4-safe.
///
/// req/38 §47's brief: "spell out in `--help` and the docs that 'the only authorization check is one Bearer'" (sem: SEM-gx-api-479). A constant rather
/// than prose in a report, so that `gx serve --help` (hand 6, the hand with the flag) renders the
/// same words this crate's documentation carries.
///
/// 🔴 The `--help` half is therefore **not paid by this hand**, and saying so is the point of this
/// probe: the string exists and is asserted here; the rendering is hand 6's and req/93 §6 records it
/// as an open half rather than as done.
#[test]
fn the_absence_of_authorization_is_written_down() {
    println!("ABSENCE_NOTICE_BYTES={}", ABSENCE_NOTICE.len());
    for phrase in ["Bearer", "authorization layer", "loopback"] {
        assert!(
            ABSENCE_NOTICE.contains(phrase),
            "the notice has to name {phrase:?}: {ABSENCE_NOTICE}"
        );
    }
    // 45 §4's rule applied to the one wire-visible piece of prose this crate ships (M6-30).
    for overclaim in ["guarantee", "secure", "prevents", "impossible"] {
        assert!(
            !ABSENCE_NOTICE.to_lowercase().contains(overclaim),
            "45 §4: a notice about an absent check must not claim a present one ({overclaim:?})"
        );
    }
}

/// 🔴 **E-M6-7** — a server whose key disagrees with the project's recorded keyid does not start.
///
/// req/38 §50 M6H3-4, adopted (a)+(b) = E-M6-7: "`.gx/config.toml` carries a reference to the engine's signing keyid (within req/56 §2's
/// 'a reference to the public keyid only' frame)" (sem: SEM-gx-api-480). The **reader** of that file is `gx serve` (hand 6, the hand with the
/// flag); the **check** is here, in [`gx_api::state::AppState::new`], where a mismatch can stop the
/// surface from existing rather than be noticed in a log.
///
/// A refusal to start rather than a warning, and the reason is what the recorded value is for: a
/// project records which key its receipts are signed by so that a third party can verify them, and a
/// server that started anyway would make every receipt it issued unverifiable against the id the
/// project publishes — while looking healthy on `/healthz`.
#[test]
fn a_server_whose_key_the_project_does_not_record_refuses_to_start() {
    let good = support::Server::new("auth_keyid_match", "before\n");
    println!("KEYID_MATCH_STARTED=1 signing={}", good.keys.signing_id());

    // The same construction with a keyid this server does not hold.
    let refused =
        support::Server::try_with_expected_keyid("auth_keyid_mismatch", "somebody-elses-key");
    println!(
        "KEYID_MISMATCH_REFUSED={} code={:?}",
        u8::from(refused.is_err()),
        refused.as_ref().err().map(|e| e.code)
    );
    let error = refused.expect_err(
        "E-M6-7: a recorded keyid that names a different key is a server that must not start",
    );
    assert_eq!(error.code, "VALIDATION_ERROR");
    assert!(
        error.detail.contains("somebody-elses-key"),
        "the refusal names the recorded id, because an operator reading it has to know which of the \
         two values to change: {}",
        error.detail
    );
}
