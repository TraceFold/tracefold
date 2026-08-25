// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R37 / `req/501` M-04 + L-02** — the red bed, written **before** the repair
//! (`req/38` §226).
//!
//! # M-04 — three ledger reads, one project, two answers
//!
//! Audit 36 (`req/496` §4-4) cut a journal's last frame — the `Committed` record — and left the
//! ledger whole, which is 43 §7-3b's crash window and the state a buyer's independent verification
//! meets after a power cut. On that project, in the same process, at the same moment:
//!
//! | route | healthy | after the cut, before the repair |
//! |---|---|---|
//! | `GET /ledger/checkpoint` | 200 signed | **500 `LEDGER_DISAGREES`** — the one route with a gate |
//! | `GET /ledger/proof?leaf=<id>` | 200, 46 bytes | **200, the same 46 bytes** |
//! | `GET /ledger/consistency` | 200 `consistent: true` | **200 `consistent: true`, identical** |
//!
//! `ledger_proof`'s own doc-string carries 44 §2.2 with no degradation (`SEM-gx-api-154`): `404`
//! for "an unknown leaf, **or not yet committed**". The cut row reads back `Committing`. So the two
//! routes a buyer uses to check this server **without trusting it** are byte-identical to a healthy
//! project, while the one route that signs refuses.
//!
//! What the repair owes, and what it must not do: the two ungated routes must stop answering as
//! though the project were sound, and the answers that were **already right** must not move. The
//! negative controls are therefore as load-bearing as the attack —
//!
//! * the identical requests **before** the cut still answer 200 (the repair must not refuse a
//!   healthy project);
//! * `leaf=99`, an index outside a one-leaf tree, answers **404 before and after** — the
//!   unknown-leaf question is about the ledger's own size and is answerable whatever the journal
//!   says, so a gate that swallowed it would be trading one wrong answer for another;
//! * a `gx1:` id this ledger has never held answers 404 on both sides, for the same reason.
//!
//! # L-02 — one state, two shapes on one surface
//!
//! `POST /candidates/{id}/cancel` answers 44 §2.2's flat shape (`state: "Aborted"`,
//! `reason: "OwnerCancelled"`; `SEM-gx-api-136`) and says in its own note that `Lifecycle`'s
//! serialised form "is not" the contract. `GET /candidates/{id}` publishes exactly that serialised
//! form. The bed reads the same row through both mouths and requires one shape.

mod support;

use std::path::Path;

use support::Server;

fn id_of(json: &serde_json::Value) -> String {
    json["id"]
        .as_str()
        .unwrap_or_else(|| panic!("`POST /candidates` returns the id: {json}"))
        .to_string()
}

// ---------------------------------------------------------------------------
// The cut — copied verbatim from `r36_error_road.rs` by way of audit 36's probe, because a
// re-implementation of this from memory is what `req/496` §7 item 1 confesses to.
// ---------------------------------------------------------------------------

fn frames(bytes: &[u8]) -> Vec<(usize, usize)> {
    const CEILING: u32 = 1 << 20;
    let chained = bytes.len() >= 8 && {
        let mut header = [0u8; 4];
        header.copy_from_slice(&bytes[..4]);
        u32::from_be_bytes(header) > CEILING
    };
    let link = usize::from(chained) * 32;
    let mut at = usize::from(chained) * 8;
    let mut out = Vec::new();
    while at + 4 <= bytes.len() {
        let mut header = [0u8; 4];
        header.copy_from_slice(&bytes[at..at + 4]);
        let length = u32::from_be_bytes(header) as usize;
        if length == 0 || length > CEILING as usize || at + 4 + length + link > bytes.len() {
            break;
        }
        out.push((at, 4 + length + link));
        at += 4 + length + link;
    }
    out
}

fn truncate_at(path: &Path, at: u64) {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open for truncation");
    file.set_len(at).expect("truncate");
}

/// The three routes and the two unknown-leaf controls, in one place so that "healthy" and
/// "after the cut" are asked the *same* questions.
struct ThreeAnswers {
    proof_by_id: u16,
    proof_by_index: u16,
    proof_unknown_index: u16,
    proof_unknown_id: u16,
    checkpoint: u16,
    consistency: u16,
    consistent_field: serde_json::Value,
    proof_bytes: usize,
}

async fn ask_the_three(client: &support::Client, when: &str, id: &str) -> ThreeAnswers {
    let proof = client
        .send("GET", &format!("/v1/ledger/proof?leaf={id}"), None)
        .await;
    let by_index = client.send("GET", "/v1/ledger/proof?leaf=0", None).await;
    let unknown_index = client.send("GET", "/v1/ledger/proof?leaf=99", None).await;
    // A well-formed `gx1:` id this ledger has never held.
    //
    // 🔴 **R38 / `req/38` §292** — the swap is between the base32 body's **two legal tails**, and
    // the assertion below is part of the control rather than decoration.
    //
    // What was here before flipped the last character between `'a'` and `'b'`. A `gx1:` body writes
    // a 256-bit digest five bits at a time, so its 52nd character carries one meaningful bit and
    // four unused ones, and `Cid::from_text` refuses a spelling whose unused bits are set — that is
    // AC-011's "one digest, one spelling", and it is correct. Only value 0 (`'a'`) and value 16
    // (`'q'`) are legal there, so flipping to `'b'` produced a **malformed** id exactly when the
    // real one ended in `'a'`, and a malformed id takes the *validation* road (422) instead of the
    // *unresolved* road (404) this control is about.
    //
    // Which road was taken depended on the environment. The id is content-addressed over an
    // `IntentView` carrying the scratch directory's absolute path, so `CARGO_TARGET_DIR` decides
    // the tail: `req/516` ran one commit twice, changing nothing but that variable, and reproduced
    // green and red. The bed, not the product, was the coin flip.
    let unknown_id = {
        let mut spelling = id.to_string();
        let last = spelling.pop().unwrap_or('a');
        spelling.push(if last == 'a' { 'q' } else { 'a' });
        // 🔴 The construction asserts that it preserved the grammar of the thing it is a control
        // for. `req/38` §292's standing lesson: a negative control built by mutating a real value
        // measures a *different* refusal the moment the mutation leaves the value's own language,
        // and it does so silently, because both roads are refusals.
        gx_core::Cid::from_text(&spelling).unwrap_or_else(|e| {
            panic!(
                "the negative control stopped being well-formed: {spelling} does not parse ({e}). \
                 An unparseable id measures the validation road, not the unresolved one"
            )
        });
        client
            .send("GET", &format!("/v1/ledger/proof?leaf={spelling}"), None)
            .await
    };
    let checkpoint = client.send("GET", "/v1/ledger/checkpoint", None).await;
    let consistency = client
        .send("GET", "/v1/ledger/consistency?from=1&to=1", None)
        .await;

    let answers = ThreeAnswers {
        proof_by_id: proof.status.as_u16(),
        proof_by_index: by_index.status.as_u16(),
        proof_unknown_index: unknown_index.status.as_u16(),
        proof_unknown_id: unknown_id.status.as_u16(),
        checkpoint: checkpoint.status.as_u16(),
        consistency: consistency.status.as_u16(),
        consistent_field: consistency.json["consistent"].clone(),
        proof_bytes: proof.json.to_string().len(),
    };
    println!(
        "R37_M04 when={when} proof_by_id={} proof_by_index={} proof_unknown_index={} \
         proof_unknown_id={} checkpoint={} consistency={} consistent={} proof_bytes={}",
        answers.proof_by_id,
        answers.proof_by_index,
        answers.proof_unknown_index,
        answers.proof_unknown_id,
        answers.checkpoint,
        answers.consistency,
        answers.consistent_field,
        answers.proof_bytes
    );
    println!("R37_M04 when={when} proof_body={}", proof.json);
    println!("R37_M04 when={when} checkpoint_body={}", checkpoint.json);
    println!("R37_M04 when={when} consistency_body={}", consistency.json);
    answers
}

#[tokio::test]
async fn r37_m04_every_ledger_read_refuses_a_project_whose_two_files_disagree() {
    let server = Server::new("r37_m04_ledger", "before\n");
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
        .send(
            "POST",
            &format!("/v1/candidates/{id}/verify"),
            Some(serde_json::json!({})),
        )
        .await;
    let committed = client
        .send(
            "POST",
            &format!("/v1/candidates/{id}/commit"),
            Some(serde_json::json!({})),
        )
        .await;
    println!(
        "R37_M04 commit status={} state={} world={:?}",
        committed.status.as_u16(),
        committed.json["state"],
        server.target_contents()
    );

    // ---- Negative control 1: a healthy project, measured twice, before anything is cut ----
    let healthy = ask_the_three(&client, "healthy", &id).await;
    let healthy_again = ask_the_three(&client, "healthy_second_reading", &id).await;
    assert_eq!(
        (healthy.proof_by_id, healthy.checkpoint, healthy.consistency),
        (200, 200, 200),
        "the bed failed before the product did: a healthy project must answer all three"
    );
    assert_eq!(
        healthy.proof_bytes, healthy_again.proof_bytes,
        "instrument: two readings of a project nobody touched must agree"
    );
    assert_eq!(
        healthy.proof_unknown_index, 404,
        "the bed failed before the product did: leaf 99 of a one-leaf tree is a 404"
    );
    assert_eq!(
        healthy.proof_unknown_id, 404,
        "the bed failed before the product did: an id this ledger never held is a 404"
    );

    // ---- The cut: the journal's last frame goes, the ledger file is left whole ----
    let journal_path = server.project.join(".gx").join("ledger").join("journal");
    let bytes = std::fs::read(&journal_path).expect("read the journal");
    let kinds: Vec<&'static str> = gx_engine::replay(&bytes)
        .records()
        .iter()
        .map(gx_engine::EngineJournalRecord::kind)
        .collect();
    let spans = frames(&bytes);
    assert_eq!(spans.len(), kinds.len(), "instrument: one frame per record");
    let last_committed = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "Committed")
        .map(|(i, _)| i)
        .next_back()
        .expect("instrument: the commit wrote a Committed record");
    println!("R37_M04 kinds_before={kinds:?} cutting_at_record={last_committed}");
    truncate_at(&journal_path, spans[last_committed].0 as u64);

    // A second server over the same project: a fresh engine reading the cut journal, which is what
    // a process that comes back after a power cut is.
    let after = Server::over(server.project.clone(), server.target.clone());
    let client2 = after.client();
    let row = client2
        .send("GET", &format!("/v1/candidates/{id}"), None)
        .await;
    println!(
        "R37_M04 row_after_cut status={} body={}",
        row.status.as_u16(),
        row.json
    );

    let cut = ask_the_three(&client2, "after_cut", &id).await;

    // ---- The gate that was already there, unchanged ----
    assert_eq!(
        cut.checkpoint, 500,
        "the bed failed before the product did: `checkpoint`'s `ledger_agrees` gate is what this \
         finding compares the other two against, and it did not fire"
    );

    // ---- The two routes the repair owes ----
    assert_ne!(
        cut.proof_by_id, 200,
        "🔴 req/496 M-04: `GET /ledger/proof` answered a project whose journal witnesses no commit \
         for this leaf exactly as it answers a sound one. `SEM-gx-api-154` transcribes 44 §2.2 \
         with no degradation: `404` for an unknown leaf **or one not yet committed**, and this row \
         reads back `Committing`"
    );
    assert_ne!(
        cut.proof_by_index, 200,
        "🔴 req/496 M-04: the same leaf asked for by index, same answer"
    );
    assert_ne!(
        cut.consistency, 200,
        "🔴 req/496 M-04: `GET /ledger/consistency` answered `consistent: true` over a pair of \
         files that describe different trees. The endpoint's own doc grants it the one judgement \
         on this surface because it is `a claim about the server's own log` — and this server \
         cannot say which log it has"
    );

    // ---- Negative controls 2 and 3: the answers that were already right must not move ----
    assert_eq!(
        cut.proof_unknown_index, 404,
        "negative control: leaf 99 is outside a one-leaf tree before **and** after the cut. The \
         size of the ledger file is a fact the journal's state does not change, and a repair that \
         turned this 404 into the disagreement refusal would be trading one wrong answer for \
         another"
    );
    assert_eq!(
        cut.proof_unknown_id, 404,
        "negative control: an id this ledger has never held stays a 404"
    );
}

// ---------------------------------------------------------------------------
// L-02 — one state, one shape
// ---------------------------------------------------------------------------

#[tokio::test]
async fn r37_l02_one_row_read_through_two_mouths_has_one_state_shape() {
    let server = Server::new("r37_l02_shape", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();
    let body = server.intent_body("after\n", &actor);

    let created = client.send("POST", "/v1/candidates", Some(body)).await;
    let id = id_of(&created.json);

    // ---- A non-terminal state first: the shapes must agree for every `Lifecycle`, not only for
    //      the one 44 §2.2 spells out ----
    let planned = client
        .send("GET", &format!("/v1/candidates/{id}"), None)
        .await;
    println!(
        "R37_L02 when=planned get_state={} get_reason={} body={}",
        planned.json["state"], planned.json["reason"], planned.json
    );
    assert!(
        planned.json["state"].is_string(),
        "🔴 req/496 L-02: `GET /candidates/{{id}}` publishes `Lifecycle`'s serde form. 44 §2.2's \
         shape is the **name** of the state with the reason beside it, which is what this same \
         surface's `cancel` answers and what its own note says a wire contract is"
    );

    let cancelled = client
        .send(
            "POST",
            &format!("/v1/candidates/{id}/cancel"),
            Some(serde_json::json!({ "actor": { "Human": { "key": actor } } })),
        )
        .await;
    println!(
        "R37_L02 cancel status={} state={} reason={}",
        cancelled.status.as_u16(),
        cancelled.json["state"],
        cancelled.json["reason"]
    );
    assert_eq!(
        cancelled.status.as_u16(),
        200,
        "the bed failed before the product did: the cancel did not take"
    );

    let read = client
        .send("GET", &format!("/v1/candidates/{id}"), None)
        .await;
    println!(
        "R37_L02 when=aborted get_state={} get_reason={} body={}",
        read.json["state"], read.json["reason"], read.json
    );

    assert_eq!(
        read.json["state"], cancelled.json["state"],
        "🔴 req/496 L-02: the same row, the same instant, two shapes for `state` — `cancel` \
         answers 44 §2.2's flat name and the `GET` beside it answers `Lifecycle`'s serialised \
         form. `cancel`'s own note rejects the second as a wire contract"
    );

    // 🔴 **What this repair did not do, asserted rather than left to a reader.**
    //
    // `req/501` §0 proposed carrying `reason` on this face as well, so that the two answers were
    // the same object. 44 §2.2 L344 specifies **four** keys here and `wire_census.rs` turns red on
    // a fifth, so the repair took the shape of `state` and stopped. The consequence is a real loss:
    // the serde form used to carry the reason as its payload and the flat name does not, so
    // `GET /candidates/{id}` no longer says **why** an aborted row is aborted. `cancel`'s answer
    // still does, and the write that aborted the row is where the reason is published.
    //
    // This arm is here so that the loss cannot be discovered later as a surprise, and so that a DR
    // that adds `reason` to 44 §2.2 has a test to rewrite rather than a silence to fill.
    println!(
        "R37_L02 read_face_reason={} (44 §2.2 L344 specifies four keys; see req/502)",
        read.json.get("reason").is_some()
    );
    assert!(
        read.json.get("reason").is_none(),
        "req/496 L-02: `reason` on this face is a fifth key 44 §2.2 does not specify. If a DR adds \
         it, this assertion is what the DR rewrites — it is not a silent addition"
    );

    // ---- Negative control: the rest of the object is untouched by the shape change ----
    assert!(
        !read.json["transformation"].is_null(),
        "negative control: `transformation` is still there"
    );
    assert!(
        read.json.get("verdict").is_some(),
        "negative control: `verdict` is still a key of this answer"
    );
    assert!(
        read.json.get("fingerprint").is_some(),
        "negative control: `fingerprint` is still a key of this answer"
    );

    // 🔴 **The residue, measured rather than described.**
    //
    // 44 §2.1 divides the two faces — "`/candidates` is the workflow-control face …
    // `/transformations` is the permanent-record read face" — and only the first of them has
    // `state: <43's state name>` written into 44 §2.2 L344. So this repair moved the
    // workflow-control face onto the specified shape and left the permanent-record face on
    // `Lifecycle`'s serde form, which is where the reason still lives and where
    // `serve_runtime_r2` / `serve_runtime_r3` now read it.
    //
    // That is a smaller divergence than the one `req/496` L-02 found — two **faces** with two
    // contracts rather than one face contradicting itself — but it is a divergence, and printing
    // it is how the next round finds it without re-deriving it. It is deliberately **not**
    // asserted either way: which shape the permanent-record face owes is a question for 44 §2.2,
    // not for a repair lane.
    let permanent = client
        .send("GET", &format!("/v1/transformations/{id}"), None)
        .await;
    println!(
        "R37_L02_RESIDUE candidates_state={} transformations_state={} same_shape={}",
        read.json["state"],
        permanent.json["state"],
        read.json["state"] == permanent.json["state"]
    );
}
