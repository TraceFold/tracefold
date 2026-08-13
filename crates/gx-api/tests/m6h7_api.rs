//! M6 hand 7's three surface changes, each of which a previous hand raised and a ruling scheduled
//! for 「手7 以降最初に触る手」.
//!
//! | # | ruling | what changes |
//! |---|---|---|
//! | 1 | **E-M6-22** (§53, M6H6-4 採(a)) | the shutdown `503`'s `gx_code` stops being `INTERNAL` |
//! | 2 | **M6H5-12 採(c)+(a)**, 手7 窓 (§52) | `/healthz`'s `engine_version` comes from the engine |
//! | 3 | **M6H6-15** (§53, 検収者起票) | `GET /transformations` rows carry `inverse_status` |
//!
//! # Why one file rather than three edits
//!
//! Each of the three is a **one-field** change to a surface an earlier hand built, and each one has
//! exactly one reason it is not cosmetic. Kept together they can be read as what they are: the three
//! places where hands 5 and 6 wrote down a fold, a borrowed constant and an absence, and said so
//! rather than hiding them. `req/95` §5 carries the same three with their rulings.

mod support;

use support::Server;

// ---------------------------------------------------------------------------
// 1. E-M6-22 — `UNAVAILABLE`
// ---------------------------------------------------------------------------

/// 🔴 **E-M6-22** — the refusal a shutting-down server sends names itself.
///
/// Hand 6 folded it to `INTERNAL` and raised **M6H6-4** because 44 §2.3's twelve codes are all words
/// about a *request* and none is a word about a *server*. §53 ruled 採(a): `UNAVAILABLE` (503, CLI
/// exit 1) as 44 §2.6's backward-compatible addition.
///
/// What the fold cost, said as a test rather than as a sentence: a client's retry policy reads
/// `gx_code`, and `INTERNAL` means 「this server made a mistake」 while `UNAVAILABLE` means 「this
/// server is going away and the replacement will answer」. Those are different instructions to a
/// caller, and one of them was wrong.
#[tokio::test]
async fn a_shutting_down_server_answers_unavailable_rather_than_internal() {
    let server = Server::new("m6h7_unavailable", "before");
    let client = server.client();

    let before = client.send("GET", "/v1/healthz", None).await;
    assert_eq!(before.status.as_u16(), 200, "the server answers first");

    server.state.shutdown().begin();
    let after = client.send("GET", "/v1/healthz", None).await;
    assert_eq!(
        after.status.as_u16(),
        503,
        "the status was already honest and stays so"
    );
    assert_eq!(
        after.gx_code(),
        gx_api::gx_code::UNAVAILABLE,
        "E-M6-22: the shutdown refusal is no longer folded into INTERNAL"
    );
    assert_eq!(
        gx_api::gx_code::UNAVAILABLE_STATUS,
        503,
        "44 §2.6's addition carries the status 44 §2.3's third column would give it"
    );
    assert_eq!(
        gx_api::gx_code::UNAVAILABLE_CLI_EXIT,
        1,
        "44 §1.4's 1 (内部エラー) is the exit a CLI gets: there is no 「server went away」 exit"
    );
    assert!(
        after.json["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("shutting down"),
        "the detail still carries what the code cannot: {}",
        after.json
    );
}

/// 🔴 `UNAVAILABLE` is **not** in `GX_CODES`, for the reason `INVALID_STATE` is not.
///
/// `GX_CODES` is a transcription of 44 §2.3, and `probes/doubt/tests/m6_gx_code.rs` compares it
/// against the markdown. A transcription that quietly gained a thirteenth row would stop being one —
/// the same shape hand 5 chose for `INVALID_STATE` (M6H5-3) and the same reason.
#[test]
fn the_ruled_addition_is_named_beside_the_transcription_and_not_inside_it() {
    assert_eq!(
        gx_api::gx_code::GX_CODES.len(),
        12,
        "44 §2.3 still has twelve rows and this table still transcribes them"
    );
    assert!(
        !gx_api::gx_code::GX_CODES
            .iter()
            .any(|row| row.code == gx_api::gx_code::UNAVAILABLE),
        "the ruled addition may not be smuggled into the transcription"
    );
    assert!(
        gx_api::gx_code::RULED_ADDITIONS
            .iter()
            .any(|row| row.code == gx_api::gx_code::UNAVAILABLE),
        "E-M6-22 belongs on the list of codes 44 does not have and this repository does"
    );
    assert!(
        gx_api::gx_code::RULED_ADDITIONS
            .iter()
            .any(|row| row.code == gx_api::gx_code::INVALID_STATE),
        "E-M6-18's INVALID_STATE is the first entry on that list"
    );
}

// ---------------------------------------------------------------------------
// 2. M6H5-12 — the engine's version, from the engine
// ---------------------------------------------------------------------------

/// 🔴 **M6H5-12 採(c)+(a)** — `/healthz`'s `engine_version` is the **engine's**, not this crate's.
///
/// Hand 5 wrote `env!("CARGO_PKG_VERSION")` and raised the borrow: it is gx-api's version, and 44
/// §2.2's field is called `engine_version`. 41 §2 puts both crates in one workspace at one version,
/// so the two are the same string **today** — which is precisely why the borrow was invisible and
/// why §52 put the accessor in the hand that builds the distributables. The day the two crates
/// version separately, a `/healthz` reporting the API's version would be reporting the wrong thing
/// to an operator diagnosing a rollout.
#[tokio::test]
async fn healthz_reports_the_engines_version_and_not_this_crates() {
    let server = Server::new("m6h7_version", "before");
    let answer = server.client().send("GET", "/v1/healthz", None).await;
    assert_eq!(answer.status.as_u16(), 200);
    assert_eq!(answer.json["status"].as_str(), Some("ok"));
    assert_eq!(
        answer.json["engine_version"].as_str(),
        Some(gx_engine::Engine::<gx_api::state::RequestEvidence>::version()),
        "the field named engine_version is answered by the engine"
    );
    assert_eq!(
        gx_engine::VERSION,
        env!("CARGO_PKG_VERSION"),
        "41 §2: one workspace, one version -- the same value today, from two declarations"
    );
}

// ---------------------------------------------------------------------------
// 3. M6H6-15 — `inverse_status` on the list rows
// ---------------------------------------------------------------------------

/// 🔴 **M6H6-15** — ASM-48-3's second half: a `GET /transformations` row says whether its undo is
/// still there.
///
/// 42 §3.12's `InverseStatus` has **four** values (`Available | Consumed { by } | Expired |
/// Unavailable`); §53's ticket says three, and the discrepancy is raised in `req/95` rather than
/// resolved by picking one. What the row carries is the engine's own answer, and `null` for a row
/// that has no escrow entry at all — a transformation that never reached T-10b has no undo, and
/// saying `"Unavailable"` there would answer 「invert() refused」 about a transformation nobody ever
/// asked to invert.
///
/// Why it matters on the **list** and not only on `GET /transformations/{id}`: DR-1(a)'s wedge is
/// verified undo, and the question an operator asks a fleet is 「which of these can I still undo」.
/// One row at a time is not that question.
#[tokio::test]
async fn a_transformation_row_carries_its_inverse_status() {
    let server = Server::new("m6h7_inverse_status", "before");
    let client = server.client();

    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after", "human-1")),
        )
        .await;
    assert_eq!(created.status.as_u16(), 201);
    let id = created.json["id"].as_str().expect("an id").to_string();

    // Before the commit there is no escrow row at all.
    let listed = client.send("GET", "/v1/transformations", None).await;
    assert_eq!(listed.status.as_u16(), 200);
    let row = &listed.json["items"][0];
    assert!(
        row.get("inverse_status").is_some(),
        "the field is present even when the answer is 「none yet」: {row}"
    );
    assert!(
        row["inverse_status"].is_null(),
        "a transformation that never reached T-10b has no escrow row, and null says that \
         rather than claiming invert() refused: {row}"
    );

    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    assert_eq!(verified.status.as_u16(), 200);
    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    assert_eq!(committed.status.as_u16(), 200);

    let listed = client.send("GET", "/v1/transformations", None).await;
    let row = &listed.json["items"][0];
    assert_eq!(
        row["inverse_status"].as_str(),
        Some("Available"),
        "42 §3.12: T-10b escrowed an inverse and it has not been consumed: {row}"
    );
}
