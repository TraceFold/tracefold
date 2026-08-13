//! 🔴 **E-M6-20** — 44 §2.2's `[DR-2感度]` paragraph, made reachable from an HTTP request.
//!
//! 44 §2.2's commit section says: 「record-onlyモードではこのケースでも`200`＋`enforced:false`の
//! Receiptを返す」. Hand 5 implemented the thirteen synchronous endpoints and found that **no HTTP
//! request could produce that answer** — the mode reached T-8r only through the posture the engine
//! was opened in — and raised it as M6H5-8. §52 ruled it:
//!
//! > **E-M6-20(M6H5-8 採(a))**: 44 §2.2 の commit body に `record_only: bool` を足して読む
//! > (E-M6-10 の HTTP 版・[DR-2感度] 段落の実行可能化)。実装窓=手6 以降の最初に触る手
//!
//! This is that hand, and this file is the paragraph as a measurement: a change the gate **denied**,
//! committed, applied to the substrate, with `enforced:false` in the receipt — P-7's 「適用は通ったが、
//! ポリシー上は拒否されていた」 through the wire rather than through a process's start-up flags.

mod support;

use support::Server;

/// The body 44 §2.2's `POST /candidates` takes, aimed at a path the fixture pack denies.
async fn denied_candidate(server: &Server) -> String {
    let created = server
        .client()
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", "actor-key")),
        )
        .await;
    assert_eq!(created.status, 201, "{:?}", created.json);
    created.json["id"].as_str().expect("an id").to_string()
}

/// 🔴 **E-M6-20** — `{"record_only": true}` on the commit body commits a denied transformation.
///
/// Four assertions, and each is a different half of DR-2:
///
/// 1. the gate **denied** it (so this is the case 44's paragraph is about, not a happy path);
/// 2. a commit with **no** body is refused `403 NOT_ADMITTED` — the enforcing answer;
/// 3. a commit with `{"record_only": true}` answers `200`;
/// 4. the substrate **moved** and the answer carries `enforced: false`. 「適用は通すが `enforced=false`」
///    is one sentence with two halves and a probe that checked only the flag would pass for an
///    implementation that recorded the exception and applied nothing.
#[tokio::test(flavor = "multi_thread")]
async fn record_only_on_the_commit_body_applies_a_denied_change_and_marks_it() {
    let server = Server::denying("m6h6_dr2_record_only", "before\n");
    let client = server.client();
    let id = denied_candidate(&server).await;

    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    println!("DR2_VERDICT={}", verified.json["verdict"]);
    assert_eq!(
        verified.json["verdict"],
        serde_json::json!("Deny"),
        "the fixture pack denies a writable path (M6H3-9): {:?}",
        verified.json
    );

    let enforcing = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    println!(
        "DR2_ENFORCING status={} code={}",
        enforcing.status,
        enforcing.gx_code()
    );
    assert_eq!(enforcing.status, 403);
    assert_eq!(enforcing.gx_code(), "NOT_ADMITTED");
    assert_eq!(
        server.target_contents(),
        "before\n",
        "a refused commit changes nothing"
    );

    let recorded = client
        .send(
            "POST",
            &format!("/v1/candidates/{id}/commit"),
            Some(serde_json::json!({ "record_only": true })),
        )
        .await;
    println!(
        "DR2_RECORD_ONLY status={} body={}",
        recorded.status, recorded.json
    );
    assert_eq!(
        recorded.status, 200,
        "44 §2.2: 「record-onlyモードではこのケースでも`200`…を返す」: {:?}",
        recorded.json
    );
    assert_eq!(recorded.json["state"], serde_json::json!("Committed"));
    assert_eq!(
        recorded.json["enforced"],
        serde_json::json!(false),
        "44 §2.2: 「`enforced:false`のReceiptを返す」 — INV-S5's visibility on the wire"
    );
    assert_eq!(
        server.target_contents(),
        "after\n",
        "🔴 「適用は通すが `enforced=false`」 — the apply is the other half of the sentence"
    );
}

/// 🔴 The field's **absence** is not `false`, and `false` is not absence.
///
/// Every client written against 44 before E-M6-20 sends no body at all, and its commits have to keep
/// meaning 「whatever posture this server runs in」. `{"record_only": false}` is a different request:
/// it **overrides** a `RecordOnly` server for one call. `Option<bool>` is how 「未指定」 and 「no」 stay
/// two answers instead of one — and the shape M6H5-11 already settled for extractors, one field on.
#[tokio::test(flavor = "multi_thread")]
async fn an_absent_flag_and_an_explicit_false_are_different_requests() {
    // A server whose **process** posture is record-only, so that an explicit `false` has something
    // to override. This is DR-2's 「substrate単位または全体設定」 side, and the request-level flag is
    // M6-08's per-call axis on top of it.
    let server = Server::denying_in(
        "m6h6_dr2_absent_vs_false",
        "before\n",
        Some(gx_core::EnforcementMode::RecordOnly),
    );
    let client = server.client();
    let id = denied_candidate(&server).await;
    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    assert_eq!(verified.json["verdict"], serde_json::json!("Deny"));

    let overridden = client
        .send(
            "POST",
            &format!("/v1/candidates/{id}/commit"),
            Some(serde_json::json!({ "record_only": false })),
        )
        .await;
    println!(
        "DR2_EXPLICIT_FALSE status={} code={}",
        overridden.status,
        overridden.gx_code()
    );
    assert_eq!(
        overridden.status, 403,
        "an explicit `false` enforces, even on a record-only server: {:?}",
        overridden.json
    );

    let absent = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    println!(
        "DR2_ABSENT status={} enforced={}",
        absent.status, absent.json["enforced"]
    );
    assert_eq!(
        absent.status, 200,
        "no body means 「this server's posture」, which here is RecordOnly: {:?}",
        absent.json
    );
    assert_eq!(absent.json["enforced"], serde_json::json!(false));
}

/// 🔴 44 §2.4's conflict sees the posture: two postures are two requests.
///
/// 「同一キーで異なるリクエスト本文が来た場合は`409 IDEMPOTENCY_CONFLICT`」. A cache keyed on the
/// transformation alone would answer a **record-only** retry with the response of an **enforcing**
/// attempt, or the reverse — the two differ in whether the change was applied, which is the single
/// most consequential thing a commit can differ in.
#[tokio::test(flavor = "multi_thread")]
async fn two_postures_under_one_idempotency_key_are_a_conflict() {
    let server = Server::denying("m6h6_dr2_idempotency", "before\n");
    let client = server.client();
    let id = denied_candidate(&server).await;
    client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;

    let first = client
        .send_with(
            "POST",
            &format!("/v1/candidates/{id}/commit"),
            Some(serde_json::json!({ "record_only": true })),
            &[("idempotency-key", "one-key")],
        )
        .await;
    assert_eq!(first.status, 200, "{:?}", first.json);

    let replayed = client
        .send_with(
            "POST",
            &format!("/v1/candidates/{id}/commit"),
            Some(serde_json::json!({ "record_only": true })),
            &[("idempotency-key", "one-key")],
        )
        .await;
    assert_eq!(replayed.status, 200, "the same request is the same answer");

    let conflicting = client
        .send_with(
            "POST",
            &format!("/v1/candidates/{id}/commit"),
            Some(serde_json::json!({ "record_only": false })),
            &[("idempotency-key", "one-key")],
        )
        .await;
    println!(
        "DR2_IDEMPOTENCY_CONFLICT status={} code={}",
        conflicting.status,
        conflicting.gx_code()
    );
    assert_eq!(conflicting.status, 409);
    assert_eq!(conflicting.gx_code(), "IDEMPOTENCY_CONFLICT");
}

/// 🔴 **M6H8-14 ④** — `/undo` says which DR-2 axis it ran on, the way `/commit` does.
///
/// 44 §2.1 marks `POST /transformations/{id}/undo` with the same `[DR-2感度]` ✓ as `/commit`, and
/// until this batch the two answered differently: `committed_answer` carried `enforced` and the undo
/// body carried `transformation`/`undone`/`superseded_state`/`at` and nothing about enforcement. An
/// undo drives canonicalize→commit with `mode: None`, so on a `RecordOnly` server the undo's own
/// receipt says `enforced: false` — a fact a client could previously reach only by base64-decoding
/// the DSSE payload out of a receipt it had to fetch separately.
///
/// Both axes are measured, because a field that is always `true` is not a measurement.
#[tokio::test(flavor = "multi_thread")]
async fn the_undo_answer_carries_the_enforcement_axis() {
    let server = Server::new("m6fix_undo_enforced", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();

    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", &actor)),
        )
        .await;
    assert_eq!(created.status, 201, "{:?}", created.json);
    let id = created.json["id"].as_str().expect("an id").to_string();

    client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    assert_eq!(committed.status, 200, "{:?}", committed.json);
    assert_eq!(committed.json["enforced"], serde_json::json!(true));

    let undone = client
        .send(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({})),
        )
        .await;
    println!(
        "UNDO_ENFORCED status={} body={}",
        undone.status, undone.json
    );
    assert_eq!(undone.status, 200, "{:?}", undone.json);
    assert_eq!(
        undone.json["enforced"],
        serde_json::json!(true),
        "🔴 M6H8-14 ④: the undo's own receipt reports its own axis, on the endpoint 44 §2.1 gives \
         the same DR-2 mark as `/commit`"
    );
    assert_eq!(
        undone.json["superseded_state"],
        serde_json::json!("Superseded")
    );
}

/// 🔴 **FR-M7-1** — an undo whose **inverse** the gate denies, on a `RecordOnly` server: it answers
/// promptly, it answers `200`, and the refusal is recorded rather than enforced.
///
/// Two facts, and the expectation on the second one **reverses here**.
///
/// **It answers at all.** `undo_transformation` built its refusal by calling `halted_undo`, which
/// reads the state back through a lock of its own, **while still holding the engine guard** — and
/// `std::sync::Mutex` is not re-entrant. Under M6-06 採(a) there is exactly one
/// `Arc<Mutex<Engine>>`, so that deadlock does not fail a request: it stops the server. No suite had
/// ever driven an undo through a **denying** gate, so the branch had never been taken. This probe
/// takes it, with a timeout so that a regression is a failure rather than a hung run. That half is
/// unchanged and the timeout stays: M6FIX-1 is still the thing it guards.
///
/// 🔴 **The reversal.** Until M7 this probe asserted `403 NOT_ADMITTED` and `enforced: null`, and
/// said in the same breath that it was measuring a **gap**: 44 §2.1 marks `/undo` `[DR-2感度]` ✓ like
/// `/commit`, `/commit` in this posture answers `200`+`enforced:false`, and `/undo` had no
/// record-only road at all. The fix batch refused to patch it because changing it gives `undo` a new
/// semantics, 「which is a ruling」 (req/97 §6 M6FIX-2). The ruling arrived — **裁定 #4**, carried into
/// `req/98` §3-2 as **FR-M7-1** and confirmed by `req/38` §57:
///
/// > record-only posture の `/undo` は、undo 要求を record し `enforced:false` で応答する(commit と
/// > 対称・44 §2.1 の DR-2 ✓ の実装)
///
/// — so the old assertion is now the wrong answer and this probe is the one that says so, exactly as
/// its previous version predicted (「the day somebody implements it, this probe is the one that says
/// so」). The reversal is committed **red first**: this file changed with no line of `src/` beside it.
///
/// Four assertions, because 「record し `enforced:false` で応答する」 is one sentence with four halves:
/// the status is the one `/commit` gives (`200`, not `403`), the axis is on the wire (`enforced:
/// false`, the same field name and the same shape `committed_answer` writes), the substrate **moved
/// back** (a record-only undo applies — it is recorded, not skipped), and the original is
/// `Superseded`, which is the record the request left behind.
#[tokio::test(flavor = "multi_thread")]
async fn an_undo_of_a_denied_inverse_is_recorded_rather_than_refused_under_record_only() {
    let server = Server::denying_in(
        "m6fix_undo_record_only",
        "before\n",
        Some(gx_core::EnforcementMode::RecordOnly),
    );
    let client = server.client();
    let id = denied_candidate(&server).await;
    client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    assert_eq!(committed.status, 200, "{:?}", committed.json);
    assert_eq!(committed.json["enforced"], serde_json::json!(false));
    assert_eq!(
        server.target_contents(),
        "after\n",
        "the record-only commit applied, which is what gives the undo something to undo"
    );

    let undone = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        client.send(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({})),
        ),
    )
    .await
    .expect(
        "🔴 the undo did not answer in 20 s. `halted_undo` takes the engine lock and was being \
         called with the handler's own guard still alive; with one `Arc<Mutex<Engine>>` (M6-06 \
         採(a)) that deadlock stops every later request too",
    );
    println!(
        "UNDO_DENIED_INVERSE status={} code={} enforced={} body={}",
        undone.status,
        undone.gx_code(),
        undone.json["enforced"],
        undone.json
    );
    assert_eq!(
        undone.status, 200,
        "🔴 FR-M7-1: 「record-only posture の `/undo` は…`enforced:false` で応答する」 — the same \
         status `/commit` answers in this posture, on the row 44 §2.1 gives the same DR-2 mark: {:?}",
        undone.json
    );
    assert_eq!(
        undone.json["enforced"],
        serde_json::json!(false),
        "the axis is on the wire and it is `false`: the gate denied the inverse and this server \
         records rather than enforces (INV-S5's visibility, the field `committed_answer` writes)"
    );
    assert_eq!(
        server.target_contents(),
        "before\n",
        "🔴 「適用は通すが `enforced=false`」 — a record-only undo **applies**. A probe that checked \
         only the flag would pass for an implementation that recorded the exception and undid nothing"
    );
    assert_eq!(
        undone.json["superseded_state"],
        serde_json::json!("Superseded"),
        "the undo request is recorded: the original carries the state its inverse put it in"
    );
}
