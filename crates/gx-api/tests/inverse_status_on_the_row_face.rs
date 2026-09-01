// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **Owner #260 (relay ①, ruled with the TUI seat) / `req/987`** — 42 §3.12's `InverseStatus`
//! reaches `GET /transformations/{id}`, in the spelling the list already publishes.
//!
//! # The question this closes, and who could not ask it
//!
//! `list.rs` has carried `inverse_status` on every `GET /transformations` row since M6H6-15, and
//! `handlers::get_transformation` has never carried it. `req/987` §3-4 measured both faces in the
//! source and stated the consequence: *is this one still undoable* was answerable about a **page**
//! and not about the **row**, so a client that already held an id — a consent screen, which by
//! construction holds exactly one — had to fetch a list to learn a fact about a transformation it
//! could name.
//!
//! `list.rs`'s own comment reads "Why on the **list** and not only on `GET /transformations/{id}`",
//! which `req/987` §4-1 (b) read as a decision to keep this face bare. It is not one: it argues
//! that the set-shaped question needs the list *as well as* the row, and presupposes the row face
//! rather than excluding it. The absence was an omission wearing a justification — the same shape
//! as R29's `rollback`, where a ruling taken on two faces left the third behind.
//!
//! 44 §2.6 permits the addition in the words it permitted `rollback`'s: "a backward-compatible
//! addition (a new optional field) is allowed within `/v1`".
//!
//! # What this file measures, and what it deliberately does not
//!
//! It measures the **agreement of two mouths of one surface**, not the presence of a key. A member
//! assembled from somewhere else that happens to look right is `req/187` §5's own finding, and it
//! is what a key-presence check cannot see. `req/496` L-02 is this endpoint's record of what the
//! disagreement costs: one row read through two mouths at one instant answering two shapes.
//!
//! It does **not** measure "what would be restored". That descriptor (`inverse: {substrate,
//! locator, goal_cid}`) is `req/987` §4-2's design and is not in this window; a status word says
//! *whether* an inverse can still be run and never *what comes back*.
//!
//! **Declared limit**: the vocabulary arm below round-trips all seven words through serde, which
//! is a fact about the spelling and not about reachability. Three of the seven (`Expired`,
//! `Undetermined`, `BodyMissing`) have no producer this bed can reach — `Expired` has no writer at
//! all in v0.1 (DR-9, `req/78` N-06) — so this file pins their **wire form** and says so, rather
//! than pretending to have driven them. `gx-engine`'s `lifecycle_transitions.rs` is where the
//! writers are counted.

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

/// What Σ itself holds for `id`, serialised the way both wire faces serialise it.
fn sigma_inverse_status(server: &Server, id: &str) -> serde_json::Value {
    let engine = server.state.engine();
    let tid = gx_core::TransformationId(gx_core::Cid::from_text(id).expect("a transformation id"));
    engine
        .inverse_status(&tid)
        .and_then(|status| serde_json::to_value(status).ok())
        .unwrap_or(serde_json::Value::Null)
}

/// Drive one transformation all the way to `Committed` on the shipped road, so T-10b has escrowed
/// an inverse and Σ holds a real word for it.
async fn a_committed_row(name: &str) -> (Server, String) {
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
    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    assert_eq!(
        committed.status.as_u16(),
        200,
        "🔴 the bed did not commit, so no inverse was escrowed and every arm below would be \
         asserting that `null` equals `null`: {}",
        committed.json
    );
    (server, id)
}

/// A candidate that is planned and nothing else: no `commit`, therefore no T-10b, therefore no
/// escrow row at all. This is `req/987` §4-3's **E1**.
async fn a_row_that_never_reached_escrow(name: &str) -> (Server, String) {
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
    assert_eq!(created.status.as_u16(), 201, "{}", created.json);
    let id = id_of(&created.json);
    (server, id)
}

// ---------------------------------------------------------------------------
// a — bed control
// ---------------------------------------------------------------------------

/// 🔴 **Bed control** — the two beds really are the two cases, measured on Σ before any wire is
/// read.
///
/// Every arm below reads a member off a wire answer. If the committed bed did not escrow, its
/// status is `null` and the agreement arms pass while comparing nothing; if the planned bed did
/// escrow, the `null` arm is asserting the wrong fact about the wrong road. `req/334` §9-3 is the
/// standing reason this arm exists: *the instrument returned zero three times and three times the
/// instrument was wrong.*
#[tokio::test]
async fn a_bed_control_one_bed_holds_a_word_and_the_other_holds_none() {
    let (committed, committed_id) = a_committed_row("inv_status_bed_committed").await;
    let held = sigma_inverse_status(&committed, &committed_id);
    let (planned, planned_id) = a_row_that_never_reached_escrow("inv_status_bed_planned").await;
    let none = sigma_inverse_status(&planned, &planned_id);
    println!("INV_STATUS_BED committed={held} planned={none}");
    assert!(
        !held.is_null(),
        "🔴 the committed bed reached `Committed` and Σ holds no escrow status for it, so the \
         agreement arms below would be comparing `null` with `null` and passing"
    );
    assert!(
        none.is_null(),
        "🔴 a candidate that never committed already has an escrow row ({none}), so the `null` arm \
         below is not measuring E1 and this bed has to be rebuilt rather than the arm weakened"
    );
}

// ---------------------------------------------------------------------------
// b, c — the member, and the agreement that makes it worth having
// ---------------------------------------------------------------------------

/// 🔴 **Owner #260 / `req/987` §3-4** — the row face carries the word Σ holds.
///
/// Not that a key called `inverse_status` exists: that it carries **the same value Σ carries**,
/// in the same spelling.
#[tokio::test]
async fn b_the_row_face_carries_the_word_sigma_holds() {
    let (server, id) = a_committed_row("inv_status_row_face").await;
    let sigma = sigma_inverse_status(&server, &id);
    let got = server
        .client()
        .send("GET", &format!("/v1/transformations/{id}"), None)
        .await;
    println!(
        "INV_STATUS_ROW status={} body={}",
        got.status.as_u16(),
        got.json
    );
    assert_eq!(got.status.as_u16(), 200, "{}", got.json);
    assert!(
        got.json.get("inverse_status").is_some(),
        "🔴 `req/987` §3-4: a client holding this id still has to fetch a page to learn whether \
         this one row can be undone: {}",
        got.json
    );
    assert_eq!(
        got.json["inverse_status"], sigma,
        "🔴 the member is on the wire and does not agree with Σ, which is worse than its absence: \
         a reader can branch on it and be wrong: {}",
        got.json
    );
}

/// 🔴 **`req/496` L-02, one member along** — the row face and the list face spell the same value
/// for the same row at the same instant.
///
/// This is the arm the addition exists for. L-02 is this endpoint's own record of the failure:
/// `state` was `Lifecycle`'s serde derive on one mouth and the state's *name* on the other, so a
/// client branching on `state === "Aborted"` got `false` from the read face for a row the write
/// face had just called `Aborted`. A second key with two spellings would be the same defect with a
/// different name, which is why this handler serialises through `serde_json::to_value` exactly as
/// `list.rs` does rather than through `InverseStatus::kind()`.
#[tokio::test]
async fn c_the_row_face_and_the_list_face_spell_the_same_value() {
    let (server, id) = a_committed_row("inv_status_two_mouths").await;
    let client = server.client();
    let row = client
        .send("GET", &format!("/v1/transformations/{id}"), None)
        .await;
    let page = client.send("GET", "/v1/transformations", None).await;
    let rows = page.json["items"].as_array().expect("a page of rows");
    assert!(
        !rows.is_empty(),
        "🔴 the list is empty, so this arm compares one value with nothing: {}",
        page.json
    );
    let listed = rows
        .iter()
        .find(|r| r["transformation"] == serde_json::Value::String(id.clone()))
        .expect("the committed row is listed");
    println!(
        "INV_STATUS_TWO_MOUTHS row={} list={}",
        row.json["inverse_status"], listed["inverse_status"]
    );
    assert_eq!(
        row.json["inverse_status"], listed["inverse_status"],
        "🔴 `req/496` L-02: one row read through two mouths of one surface at one instant answers \
         two shapes. row={} list={listed}",
        row.json
    );
}

// ---------------------------------------------------------------------------
// d — the optional half: `null` is E1 and nothing else
// ---------------------------------------------------------------------------

/// 🔴 **`req/987` §4-3, E1** — a row that never reached T-10b carries the key with `null` in it,
/// and **not** the word `Unavailable`.
///
/// `list.rs`'s care, on this face for the same reason: 42 §3.12 defines `Unavailable` as
/// "`invert()` returned `None`", so writing it for a candidate that never asked would answer a
/// question nobody put — the skip/pass conflation `req/29` §4 forbids.
///
/// This is also the shape of the member's **optionality**, and it is the only shape an
/// exactly-N census permits: the key is always present, and its absence of content is `null`.
/// A reader written against the five-member wire is unaffected because it never looks here, which
/// is 44 §2.6's third bullet (clients must ignore unknown fields) and is a property of readers
/// this suite cannot police — `wire_census.rs`'s own declared limit, stated rather than assumed.
#[tokio::test]
async fn d_a_row_that_never_reached_escrow_carries_null_and_not_a_word() {
    let (server, id) = a_row_that_never_reached_escrow("inv_status_e1").await;
    let got = server
        .client()
        .send("GET", &format!("/v1/transformations/{id}"), None)
        .await;
    println!("INV_STATUS_E1 body={}", got.json);
    assert_eq!(got.status.as_u16(), 200, "{}", got.json);
    assert!(
        got.json.get("inverse_status").is_some(),
        "🔴 the key is missing rather than `null`, which is a third spelling of nothing on a face \
         whose census requires exactly N members: {}",
        got.json
    );
    assert!(
        got.json["inverse_status"].is_null(),
        "🔴 `req/987` §4-3 E1: this candidate never reached T-10b, so the escrow question was \
         never asked. A word here answers a question nobody put: {}",
        got.json
    );
}

// ---------------------------------------------------------------------------
// e — the vocabulary, in the spelling the wire uses
// ---------------------------------------------------------------------------

/// 🔴 **The seven words, and the one that is not a bare string.**
///
/// `InverseStatus::ALL_KINDS` is seven, not four: 42 §3.12's `Available`/`Consumed`/`Expired`/
/// `Unavailable`, plus `Pending` (`req/38` §98 ruling 1), `BodyMissing` (R8, `req/234` B-5) and
/// `Undetermined` (DR-46-13 / §237-5). Every one of them can now arrive on this face, so this arm
/// pins the spelling of all seven and the round trip back.
///
/// The arm that earns its place is `Consumed`: it carries data, so its serde form is the object
/// `{"Consumed":{"by":…}}` and **not** the bare word `InverseStatus::kind()` prints. `undo`'s
/// `409 INVERSE_UNAVAILABLE` `detail` uses `kind()`, which is right — that is a sentence for a
/// human. A wire contract is a contract about the shape, and this face is on the `to_value` side
/// of that line together with `list.rs`.
///
/// **Declared**: this is a fact about serde, not about reachability. No bed here produces
/// `Expired` (it has no writer in v0.1 at all), `Undetermined` or `BodyMissing`; the counting of
/// writers lives in `gx-engine`'s `tests/lifecycle_transitions.rs`.
#[test]
fn e_every_word_of_the_vocabulary_round_trips_in_the_spelling_the_wire_uses() {
    use gx_engine::store::InverseStatus;

    // 42 §1.2's `Cid` is a public 32-byte array, so this needs no bed and no spelling to parse.
    let by = gx_core::TransformationId(gx_core::Cid([7u8; 32]));
    let all = [
        InverseStatus::Available,
        InverseStatus::Consumed { by },
        InverseStatus::Expired,
        InverseStatus::Unavailable,
        InverseStatus::Pending,
        InverseStatus::BodyMissing,
        InverseStatus::Undetermined,
    ];
    assert_eq!(
        all.len(),
        InverseStatus::ALL_KINDS.len(),
        "🔴 the vocabulary grew and this arm was not widened, so the new word reaches \
         `GET /transformations/{{id}}` with nothing pinning its spelling"
    );

    for status in all {
        let wire = serde_json::to_value(status).expect("every word serialises");
        println!("INV_STATUS_WORD kind={} wire={wire}", status.kind());
        let back: InverseStatus =
            serde_json::from_value(wire.clone()).expect("and reads back as itself");
        assert_eq!(back, status, "🔴 {wire} does not round trip");
        match status {
            InverseStatus::Consumed { .. } => assert!(
                wire.is_object() && wire.get("Consumed").is_some(),
                "🔴 the one word that carries data must arrive as an object on both faces; a bare \
                 string here would be `kind()`'s sentence used as a wire contract: {wire}"
            ),
            other => assert_eq!(
                wire,
                serde_json::Value::String(other.kind().to_string()),
                "🔴 a unit word must be the bare string the SDK's union declares"
            ),
        }
    }
}
