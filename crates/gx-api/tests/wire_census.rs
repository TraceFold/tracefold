// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **A field census of every 44 §2.2 response shape** (sem: SEM-gx-api-411) — every response shape, exactly its keys, at the HTTP
//! boundary (v0.4-i; `req/38` §115's residue: "44 §2.2's response-shape census (the commit composite shape, ProblemDetail) is
//! unfixed outside §110's carrier scope", declared by `req/177` §5).
//!
//! # Why this suite exists, and why it must drive the real router
//!
//! The Rust field censuses of `req/38` §110/§113 (`m2_types.rs`, `receipt_verdict_wire.rs`) fix the
//! **carrier types** — `Receipt`, `DsseEnvelope`, `Checkpoint`, `VerdictCheckpoint` — and the
//! handlers pass those through `serde_json::to_value`, so the type censuses cover that part of the
//! wire *transitively*. What they constitutionally cannot see is everything 44 §2.2 specifies that
//! is **not** a serde type: the composite bodies `handlers.rs` builds with `serde_json::json!`
//! (`POST /candidates`' 201, the commit answer that mounts four fields beside the receipt, the
//! verify answer, `ProblemDetail`, the stream envelope and its ten `data` shapes). A `json!` block
//! is not type-fixed — §113 ruling 4's struct-literal dispensation (sem: SEM-gx-api-412) applies only where a `#[derive]`
//! pins the field set at compile time, and here nothing does. Adding a field to any of these
//! composites compiles clean and ships silently. ∴ each census below is measured **through the real
//! handler over the real router** (51 §7's client), never by rebuilding the JSON in the test.
//!
//! # The census (denominator declared, nothing silently dropped)
//!
//! One row per response shape 44 §2.2 (+§2.3) specifies; the section is the shape's source (sem: SEM-gx-api-413):
//!
//! | shape | 44 | fixed by |
//! |---|---|---|
//! | `POST /candidates` 201 composite | §2.2 L324-340 | here |
//! | `GET /candidates/{id}` 200 composite | §2.2 L342-344 | here |
//! | `POST .../verify` 200 composite (+`proof` interior) | §2.2 L346-352 | here |
//! | `POST .../commit` 200 = Receipt + 4 riders | §2.2 L354-366 | here (the shape `req/177` named) |
//! | `POST .../escalation` 200 composite | §2.2 L368-376 | here |
//! | `POST .../cancel` 200 composite | §2.2 L378-386 | here |
//! | `POST .../undo` 200 = Receipt + 5 riders | §2.2 L388-394 | here |
//! | `GET /transformations/{id}` 200 composite | §2.2 L396-398 | here |
//! | `POST .../replay` 200 composite | §2.2 L400-408 | here |
//! | `GET /receipts/{tid}` = `Receipt` + `receipt_view` + `server_health` | §2.2 L410-412 | type census (transitive) + SDK E2E (`req/177`) + here |
//! | `GET /ledger/proof` = `InclusionProof` | §2.2 L414-418 | type round-trip only (`m2_types`) → JSON face here |
//! | `GET /ledger/checkpoint` = `Checkpoint` | §2.2 L420-422 | type census (transitive) + SDK E2E + here |
//! | `POST /verdict-checkpoints` 201 = `VerdictCheckpoint` | §2.2 L424-432 | type census (transitive) + here |
//! | `GET /verdict-checkpoints` 200 page composite | §2.2 L434-440 | here |
//! | `GET /verdict-checkpoints/{window_end}` 200 | §2.2 L442-444 | as the 201's |
//! | `GET /stream` envelope + ten event `data` shapes | §2.2 L446-467 | here (both halves) |
//! | `GET /healthz` | §2.2 L469-471 | here |
//! | `ProblemDetail` (all error answers) | §2.3 L473-500 | here |
//!
//! Where a shape is already fixed transitively, the census here is **not** a duplicate: it pins the
//! HTTP layer itself — a future handler that wraps or decorates the value on its way out keeps every
//! type census green while changing the wire (`req/177` §2's finding), and unlike the SDK census of
//! `req/177` (which lives behind `GX_BINARY` and skips without it) this suite runs on every
//! `cargo test -p gx-api`.
//!
//! # 🔴 NFR-011 note 5's carrier extension (sem: SEM-gx-api-414), carried one face further
//!
//! 33 NFR-011 (revising note + close, `req/38` §109/§110; sem: SEM-gx-api-415) permanently forbids a wire-side alg-like field
//! used for crypto dispatch, and §113 extended the census from `DsseSignature` to the carriers so
//! that nothing alg-like can ride *beside* a signature. This suite is the same extension at the
//! **last** face gx controls: the handler composites are the outermost structures a signature
//! travels in over HTTP, and an exact key set on each is the constructive form of "a silent
//! addition turns the set and goes RED in our own writer first" (sem: SEM-gx-api-416) at the boundary a client actually
//! reads.
//!
//! # Declared limits (the adversarial clause, stated rather than implied)
//!
//! * **Readers cannot be policed.** serde ignores unknown fields on decode and DSSE's norm requires
//!   readers to (envelope.md "Consumers MUST ignore unrecognized fields"; sem: SEM-gx-api-417) — every census here is a
//!   gate on gx's own writer, same as §110/§113's.
//! * **Unreached arms are declared, not censused.** `POST .../verify`'s `202` arm is not produced
//!   by this build (synchronous by M6-06, adopted (a) (sem: SEM-gx-api-418); ASM-44-2 permits it); the commit answer's
//!   restarted-server arm (`receipt` gone, M5H3-5) and `replay`'s non-empty `diffs` entries (need a
//!   corrupted Σ) are degraded arms this fixture cannot honestly reach.
//!
//!   🔴 **Narrowed by DR-43-2** (`req/38` §148, `req/190` §4-2). Half of that sentence was a
//!   statement about the fixture and half of it was a statement about the **product**, and only the
//!   first half was true. `req/182` H-02 restarted a real `gx serve` and read `404` /
//!   `state: null` back — so the restarted-server arm was not "unreachable", it was *the* answer a
//!   restarted server gave, and no census stood in front of it. The Σ-shadow makes the honest arm
//!   reachable (state without a body: `200`, `transformation: null`, `receipt: null`), and
//!   `crates/gx-cli/tests/serve_runtime_e2e.rs` reaches it **through a real restarted process**,
//!   which an in-process fixture cannot do — one `AppState` per test binary, one journal that this
//!   process wrote. What stays out of this file's denominator is therefore the *fixture's* limit
//!   and is now written as such: the arm is measured, one suite over. `replay`'s non-empty `diffs`
//!   still needs a corrupted Σ and is still unreached.
//!
//!   🔴 `BUSY` (`503` + `Retry-After: 1`, `gx_api::gx_code::BUSY`) is the same shape: it is a
//!   `ProblemDetail` like every other refusal — the key set below already pins it — and reaching it
//!   needs a **second process** holding `.gx/LOCK`. Also measured in `serve_runtime_e2e.rs`, on
//!   both faces (the CLI's exit 1 + `gx_code`, and this crate's `503`), and declared here so that
//!   the code's absence from this file is a reader's answered question rather than a gap.
//! * **Out of denominator**: ~~the four `EXTENSION_ENDPOINTS` (`GET /candidates` etc.) are M6-05's
//!   additions — 44 §2.7's addendum names them, §2.2 does not specify their shapes~~ (🔴 v0.4-k,
//!   `req/181`: 44 §2.7 now carries their shapes as its own addendum — see the EXTENSION section
//!   at the bottom of this file; the strike-through is kept, not deleted, so the denominator's
//!   history stays legible); the interiors of
//!   42's own serde-derived values (`Transformation`, `Fingerprint`, `Verdict`, ...) are 42's
//!   contract, pinned by their own crates' round-trips — censused here only where an existing
//!   NFR-011 census is being mirrored on the wire (envelope/signature/tally).
//!
//! # 🔴 v0.4-k — the four EXTENSION endpoints (44 §2.7 v0.4-k addendum, `req/181`; sem: SEM-gx-api-419)
//!
//! `req/179` §2 declared the four `EXTENSION_ENDPOINTS` out of the v0.4-i denominator because 44
//! §2.2 does not specify their shapes and a census needs a source (sem: SEM-gx-api-420) to be a census of *something*.
//! `req/38` §117 ruling 6 accepted the declaration and carried the four as residue: "paired with 44 §2.7's codification" (sem: SEM-gx-api-421).
//! v0.4-k wrote the shapes into 44 §2.7 (as an addendum **inside** §2.7 — the endpoints stay 44
//! extensions per §2.6, not §2.1 rows; M6-05's standing is unchanged) by reading the shipped wire
//! first (`list.rs`), so the census below and the article text (sem: SEM-gx-api-422) pin one and the same key set from two sides:
//!
//! | shape | 44 §2.7 v0.4-k addendum (sem: SEM-gx-api-423) | fixed by |
//! |---|---|---|
//! | list page composite `{items, next_cursor}` (all three lists, full and empty) | "the three list endpoints' shared contract" (sem: SEM-gx-api-424) | here |
//! | `GET /candidates` row (4 keys) | "`GET /candidates`" | here |
//! | `GET /escalations` row (7 keys) | "`GET /escalations`" | here |
//! | `GET /transformations` row (6 keys, every row) | "`GET /transformations`" | here |
//! | `GET /ledger/consistency` `{from, to, proof}` + `proof` interior (42 §3.11) | "`GET /ledger/consistency`" **as shipped** + 🔴 DR-44-1 | here (pinned as shipped, pending the DR) |
//!
//! Every one of these bodies is a `json!`/`Map::insert` composite in `list.rs`, so the same
//! argument as the module header's applies: nothing at compile time holds the set, only a probe
//! through the real router does. `GET /ledger/consistency` is pinned **as shipped** and flagged:
//! 44 §2.7 v0.4-k raised DR-44-1 because the wrapper `{from, to, proof}` disagrees with the CLI
//! twin (bare `ConsistencyProof`), the SDK's return type (bare) and `GET /ledger/proof` (bare
//! `InclusionProof`); the census keeps the wire from moving *silently* in either direction until
//! the ruling moves the article text, census and SDK type in one commit (sem: SEM-gx-api-425).
//!
//! # 🔴 v0.4-s — DR-44-9's four additive key sets (`req/38` §168, `req/187` §5; 44 §2.7 v0.4-s addendum)
//!
//! `req/187` §5 is a GUI session's field-by-field measurement of this wire against the faces that
//! read it, taken by reading the crates rather than a note about them. Six divergences; four of
//! them are **keys this surface does not carry**, and §168 ruled all four additive. The census
//! moves with the handlers, in the same commit as the article text and the SDK types — which is
//! the discipline the v0.4-k note above asked for and DR-44-1 then exercised:
//!
//! | shape | what DR-44-9 adds | why the census had to move |
//! |---|---|---|
//! | `GET /receipts/{tid}` | `receipt_view` (7 members, no `verified`, no `alg`) | the document is `{envelope, issued_at}`; everything a receipt *says* was inside signed CBOR |
//! | `GET /ledger/consistency` | `consistent`, `checked_from`, `checked_to` | a ledger face's one alarm read `consistent === false` against a wire with no such key — unreachable, not wrong |
//! | `POST` / `GET /verdict-checkpoints` (all three) | `window_start_at`, `window_end_at` | `window_start`/`window_end` are verdict **sequence numbers**; a reader took them for clock readings |
//! | `GET /transformations` row | *(nothing — already 9 keys since v0.4-l M-15)* | the sixth filing was **stale**; recorded here rather than re-implemented (`req/189`) |
//!
//! # 🔴 R29 — one member on three read faces (`req/38` §238 ruling 3, from `req/361` §3-1 + L-03)
//!
//! Three pins above move together in this window, and the reason is one question a reader could
//! not ask: **is my object back where it was, or is it half undone?** R28 put the roll-back facts
//! on the **refusal**, which answers only whoever was holding the request that aborted. Anyone
//! arriving afterwards — an auditor reading the list, a GUI reconnecting to the stream, a client
//! re-fetching the row — got `state: {"Aborted":"ApplyFailed"}` and no road to the rest.
//!
//! | shape | what R29 adds | why |
//! |---|---|---|
//! | `GET /transformations/{id}` | `rollback` (4 keys → 5) | the row a client re-fetches after the refusal is gone |
//! | `GET /transformations` row | `rollback` (9 keys → 10) | "which of these left my object somewhere it should not be" is a question about a **set**, exactly as `inverse_status` was |
//! | `aborted` stream event | `rollback` (1 key → 2) | the value was already in the assembling function's hand and was being dropped with `..` |
//!
//! All three in one commit **on purpose**: `req/361` filed the stream as L-03 precisely because
//! R28's hand-off named two faces and a ruling taken on two leaves the third behind. 44 §2.6
//! permits it in as many words ("a backward-compatible addition (a new optional field) is allowed
//! within `/v1`"), and DR-44-9's "no additions" row above predates the member's existence and is
//! silent about it — the addition is ruled, not slipped past. `null` where no roll-back was in
//! question, which is what keeps the refusal face's five-key census exact everywhere else.
//!
//! Two of these are deliberate refusals rather than additions, and both are censused as absences:
//! **no `alg`** on `receipt_view` (33 NFR-011's closing note, `req/38` §109/§113 — the algorithm is
//! the key's property and nothing alg-like may ride beside a signature) and **no verdict** of any
//! kind on it (`gx receipt verify` grades receipts, where the reader is). `GET /ledger/consistency`
//! is the single stated exception and carries its reason in `list.rs` and in the article.
//!
//! # 🔴 L-02 — a fourth key on `GET /receipts/{tid}` (`req/38` §369 item 1, from `req/603` §2)
//!
//! | shape | what L-02 adds | why the census had to move |
//! |---|---|---|
//! | `GET /receipts/{tid}` | `server_health` (`status`, `status_reason`) | a receipt reader was told to ask `/healthz`; the repository's own SDK did not, and the limit said in advance that this makes it a wire change |
//!
//! This one is not a filing that found a gap — it is a **falsifier that fired**. `docs/LIMITS.md`
//! declares that a served receipt and a 500 `/healthz` can be true of one server at one instant, and
//! rested that on "a consumer who wants to know can ask". `req/566` G-2 and `req/578` §5 both
//! measured `sdk/typescript/src/client.ts`'s `getReceipt` going straight to this endpoint and
//! calling `healthz()` nowhere. The paragraph named the consequence before it happened, so this key
//! is the paragraph being honoured rather than a new decision.
//!
//! 🔴 It is health and **not a verdict about the receipt**. Nothing here says `verified`, and
//! nothing here is signed: the object stands beside the document on exactly DR-44-9's terms, and
//! `crates/gx-cli/tests/r40_serving_routes.rs` is the negative control that `envelope` and
//! `issued_at` are byte-for-byte identical either side of a cut while this object is the one member
//! that moves.

mod support;

use support::Server;

/// The sorted key set of a JSON object — the one measurement every census below makes.
fn keys_of(value: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .unwrap_or_else(|| panic!("a JSON object was expected, got: {value}"))
        .keys()
        .cloned()
        .collect();
    keys.sort_unstable();
    keys
}

/// One census: the object's keys are exactly `expected` (which must be given sorted).
fn assert_keys(value: &serde_json::Value, expected: &[&str], shape: &str) {
    let found = keys_of(value);
    println!("CENSUS {shape}: {found:?}");
    assert_eq!(
        found, expected,
        "{shape}: the wire carries a key set 44 §2.2 does not specify — a silent addition or \
         removal must turn a census RED before a client sees it (NFR-011 note 5's carrier extension (sem: SEM-gx-api-426) \
         at the HTTP boundary)"
    );
}

/// The id out of `POST /candidates`' answer — this suite's own copy, for the reason
/// `tests/verdict_checkpoints.rs` gives: a shared helper would be one more place reading the shape.
fn id_of(json: &serde_json::Value) -> String {
    json["id"]
        .as_str()
        .unwrap_or_else(|| panic!("`POST /candidates` returns the id: {json}"))
        .to_string()
}

/// 🔴 The Admit pipeline's ten response shapes, each exactly its keys (44 §2.2, rows 1-3 and 7-12
/// of the module table).
///
/// One flow rather than ten fixtures because the shapes chain — a commit's answer only exists after
/// a verify's, and 43 §8 would hold a second candidate behind the first anyway. The **commit
/// composite is the shape `req/177` §5 named unfixed**: `committed_answer` mounts
/// `transformation`/`state`/`enforced`/`at` beside the receipt's own two keys with
/// `Map::insert`, which no type census can see.
#[tokio::test]
async fn the_admit_pipeline_answers_exactly_the_specified_key_sets() {
    let server = Server::new("wire_census_admit", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();

    // 44 §2.2 L324-340: "Response `201 Created`: `Transformation`…+ `precondition_fingerprint`…+
    // `state`" (sem: SEM-gx-api-427). `id` and `created_at` are the handler's declared riders (handlers.rs: the id so a
    // client need not compute a CID; the RFC 3339 clock per 44 §0 "the API layer's responsibility").
    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", &actor)),
        )
        .await;
    assert_eq!(created.status.as_u16(), 201, "{}", created.json);
    assert_keys(
        &created.json,
        &[
            "created_at",
            "id",
            "precondition_fingerprint",
            "state",
            "transformation",
        ],
        "POST /candidates 201",
    );
    let id = id_of(&created.json);

    // 44 §2.2 L344: "`{ transformation: Transformation, state: <43's state name>, verdict: Verdict|null,
    // fingerprint: Fingerprint }`" (sem: SEM-gx-api-428) — the four, and only the four.
    let candidate = client
        .send("GET", &format!("/v1/candidates/{id}"), None)
        .await;
    assert_eq!(candidate.status.as_u16(), 200);
    assert_keys(
        &candidate.json,
        &["fingerprint", "state", "transformation", "verdict"],
        "GET /candidates/{id} 200",
    );

    // 44 §2.2 L350: the synchronous 200 arm (ASM-44-2) — "`200` + the final `state`/`verdict` may be returned
    // immediately" (sem: SEM-gx-api-429). The handler's composite carries M6H3-2's proof/reasons and the DR-2 axes beside the
    // two the spec names; the census fixes the whole set so none of them can drift or multiply.
    // The 202 arm is not produced by this build (M6-06, adopted (a): one lock, no second thread; sem: SEM-gx-api-430) — declared
    // in the module header rather than half-tested.
    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    assert_eq!(verified.status.as_u16(), 200);
    assert_keys(
        &verified.json,
        &[
            "at",
            "enforced",
            "fail_posture_engaged",
            "held_by",
            "proof",
            "reasons",
            "record_only",
            "state",
            "ticket",
            "transformation",
            "verdict",
        ],
        "POST /candidates/{id}/verify 200",
    );
    // The `proof` interior is `admit_proof_json` — a `json!` composite of 42 §3.8's five accessor
    // answers (M6H3-2: the type has no `Serialize` on purpose), so it too is fixed nowhere else.
    assert_keys(
        &verified.json["proof"],
        &[
            "composed_from",
            "evidence_digests",
            "invariant_results",
            "policy_decisions",
            "proof_ref",
        ],
        "verify.proof interior (42 §3.8 via accessors)",
    );

    // 🔴 44 §2.2 L358: "Response `200 OK` (Committed success): `Receipt`" (sem: SEM-gx-api-431) — and `committed_answer`
    // mounts exactly four riders beside the receipt's own {envelope, issued_at} (E-M2-6). This is
    // the commit composite shape `req/177` §5 declared unfixed: six keys, no more (sem: SEM-gx-api-432).
    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    assert_eq!(committed.status.as_u16(), 200, "{}", committed.json);
    assert_keys(
        &committed.json,
        &[
            "at",
            "enforced",
            "envelope",
            "issued_at",
            "state",
            "transformation",
        ],
        "POST /candidates/{id}/commit 200 (Receipt + riders)",
    );
    // The envelope rides unchanged inside the composite: the same three keys and {keyid, sig}
    // `receipt_verdict_wire.rs`'s census fixes at the type — mirrored here so the HTTP layer cannot
    // decorate the *interior* either (NFR-011 note 5 (sem: SEM-gx-api-433): nothing alg-like beside a signature, at any
    // face).
    assert_keys(
        &committed.json["envelope"],
        &["payload", "payload_type", "signatures"],
        "commit.envelope interior (42 §3.10)",
    );
    for signature in committed.json["envelope"]["signatures"]
        .as_array()
        .expect("signatures is an array")
    {
        assert_keys(
            signature,
            &["keyid", "sig"],
            "commit signature (NFR-011 note 5; sem: SEM-gx-api-434)",
        );
    }

    // 44 §2.2 L398: "`{ transformation, state, receipt: Receipt|null, superseded_by:
    // TransformationId|null }`" (sem: SEM-gx-api-435).
    let permanent = client
        .send("GET", &format!("/v1/transformations/{id}"), None)
        .await;
    assert_eq!(permanent.status.as_u16(), 200);
    assert_keys(
        &permanent.json,
        // 🔴 **R29 / `req/361` §3-1** — `rollback` joins the four. 44 §2.6 allows a
        // backward-compatible addition inside `/v1`, and DR-44-9's "no additions" predates the
        // member and is silent about it.
        &[
            "receipt",
            "rollback",
            "state",
            "superseded_by",
            "transformation",
        ],
        "GET /transformations/{id} 200",
    );
    assert_keys(
        &permanent.json["receipt"],
        &["envelope", "issued_at"],
        "transformation.receipt interior (bare Receipt, E-M2-6)",
    );

    // 44 §2.2 L412: "Response `200`: `Receipt`" (sem: SEM-gx-api-436) — the bare pair, nothing riding along. The SDK
    // census (`req/177`, e2e.test.mjs) fixes the same face but only where GX_BINARY exists; this
    // one runs always.
    let receipt = client
        .send("GET", &format!("/v1/receipts/{id}"), None)
        .await;
    assert_eq!(receipt.status.as_u16(), 200);
    // 🔴 **DR-44-9** (`req/38` §168 ruling 1, `req/187` §5): a third key, `receipt_view` — the
    // signed payload decoded on this side so that a reader needs no DAG-CBOR decoder. The pair
    // above is unmoved (44 §2.6's "a new optional field"), which is the half of this census that
    // matters most: `tests/dr44_9_views.rs` measures that an offline verifier still reads the same
    // two members, and `GET /transformations/{id}`'s interior above stays **two** keys, so the view
    // is on the one endpoint that was asked for and has not leaked into every composite.
    //
    // 🔴 **L-02 / `req/38` §369 item 1 (`req/603` §2, ruling (a))** — a **fourth** key,
    // `server_health`, and the same sentence applies to it: `envelope` and `issued_at` are
    // byte-for-byte what they were, and the addition rides on the HTTP wrapper rather than in the
    // signed document. It is here because `docs/LIMITS.md`'s falsifier fired: the limit's own
    // escape — "a caller who wants to know asks `/healthz`" — was broken by this repository's own
    // SDK reading `/receipts/{tid}` without asking (`req/566` G-2, `req/578` §5), and the paragraph
    // said in advance that the consequence would be **a wire change rather than a paragraph**.
    assert_keys(
        &receipt.json,
        &["envelope", "issued_at", "receipt_view", "server_health"],
        "GET /receipts/{tid} 200 (DR-44-9 + L-02)",
    );
    // 🔴 **L-02** — two members, and the same two words `/healthz` answers with, so that a reader
    // who never asks learns exactly what the asking reader learns. No third member: this object is
    // health, not a second verdict about the receipt beside it.
    assert_keys(
        &receipt.json["server_health"],
        &["status", "status_reason"],
        "GET /receipts/{tid} 200 -> server_health interior (L-02)",
    );
    println!("SERVER_HEALTH={}", receipt.json["server_health"]);
    // 🔴 **L-02, the half that makes it worth carrying** — the in-band word is the word the
    // out-of-band endpoint says, measured on one server at one instant. Two places deriving one
    // fact is the shape `req/38` §227 keeps naming, so this is the machine that stops them drifting
    // apart on the healthy road; `crates/gx-cli/tests/r40_serving_routes.rs` drives the unhealthy
    // one, where `/healthz` is a 500 and this object is the only face still answering.
    let health_beside = client.send("GET", "/v1/healthz", None).await;
    assert_eq!(health_beside.status.as_u16(), 200);
    assert_eq!(
        receipt.json["server_health"]["status"], health_beside.json["status"],
        "the receipt reader and the monitor read one word, not two"
    );
    assert_eq!(
        receipt.json["server_health"]["status_reason"], health_beside.json["status_reason"],
        "and one reason"
    );
    assert_keys(
        &receipt.json["envelope"],
        &["payload", "payload_type", "signatures"],
        "receipt.envelope interior (42 §3.10)",
    );
    // The view's own key set. **No `verified`, no `refuted`, no `inclusion`** — HTTP carries the
    // proof and does not grade it (`gx receipt verify` does, where the reader is) — and **no
    // `alg`**, permanently: 33 NFR-011's closing note (`req/38` §109 = DR-46-5 (b)) makes the
    // algorithm a property of the key and forbids an alg-like field riding beside a signature,
    // which `req/38` §113 extended from `DsseSignature` to its carriers. This object is a carrier.
    assert_keys(
        &receipt.json["receipt_view"],
        &[
            "issued_at",
            "key_id",
            "leaf_index",
            "postcondition_fingerprint",
            "root",
            "subject",
            "tree_size",
        ],
        "GET /receipts/{tid} 200 → receipt_view interior (DR-44-9)",
    );
    println!("RECEIPT_VIEW={}", receipt.json["receipt_view"]);

    // 44 §2.2 L418: "Response `200`: `InclusionProof` (42 §3.11)" (sem: SEM-gx-api-437). `m2_types.rs` fixes this type
    // by struct literal + round trip (compile-time field set) but has no JSON-key census for it —
    // this is the proof's first exact-keys measurement on any face, taken at the wire.
    let proof = client
        .send("GET", &format!("/v1/ledger/proof?leaf={id}"), None)
        .await;
    assert_eq!(proof.status.as_u16(), 200);
    assert_keys(
        &proof.json,
        &["audit_path", "leaf_index", "tree_size"],
        "GET /ledger/proof 200 (InclusionProof, 42 §3.11)",
    );

    // 44 §2.2 L422: "Response `200`: `Checkpoint` (42 §3.11, DSSE-signed)" (sem: SEM-gx-api-438) — the five keys
    // `m2_types.rs`'s census fixes, confirmed unwrapped at the boundary.
    let checkpoint = client.send("GET", "/v1/ledger/checkpoint", None).await;
    assert_eq!(checkpoint.status.as_u16(), 200);
    assert_keys(
        &checkpoint.json,
        &["origin", "root_hash", "signature", "timestamp", "tree_size"],
        "GET /ledger/checkpoint 200 (Checkpoint, 42 §3.11)",
    );
    assert_keys(
        &checkpoint.json["signature"],
        &["keyid", "sig"],
        "checkpoint.signature (NFR-011 note 5; sem: SEM-gx-api-439)",
    );

    // 44 §2.2 L406: "Response `200`: `{ matches: bool, diffs: [...] }`" (sem: SEM-gx-api-440) plus the three declared
    // riders (M6-26, adopted (a)'s `unchecked` denominator, the count, the echoed `dry_run`). The interior
    // shape of a `diffs` entry needs a corrupted Σ to exist and is a declared unreached arm.
    let replay = client
        .send("POST", &format!("/v1/transformations/{id}/replay"), None)
        .await;
    assert_eq!(replay.status.as_u16(), 200);
    assert_keys(
        &replay.json,
        &[
            "diffs",
            "dry_run",
            "matches",
            "records_replayed",
            "unchecked",
        ],
        "POST /transformations/{id}/replay 200",
    );

    // 🔴 44 §2.2 L392: "Response `200`: the new Transformation's `Receipt`" (sem: SEM-gx-api-441) — undo's composite mounts
    // ~~five~~ six riders (M6H8-14 ④'s `enforced` among them) beside the receipt pair: ~~seven~~
    // **eight** keys.
    //
    // 🔴 **R3 / `req/222` M-12** — `witness` is the sixth rider, and the census is where a rider
    // stops being an addition somebody made and becomes one somebody declared. The audit's finding:
    // "neither the receipt nor the response says whether the CAS ran", so a `200` meant "the
    // inverse was applied" and said nothing about whether anything was compared first — and a GUI
    // drawing "verified undo" on that is drawing something it does not know. The value is
    // `attested` or `unobservable:<reason>`; `missing:*` cannot appear on a `200`, because since R3
    // a missing witness is `409 PRECONDITION_CHANGED`.
    let undone = client
        .send(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({})),
        )
        .await;
    assert_eq!(undone.status.as_u16(), 200, "{}", undone.json);
    assert_keys(
        &undone.json,
        &[
            "at",
            "enforced",
            "envelope",
            "issued_at",
            "superseded_state",
            "transformation",
            "undone",
            "witness",
        ],
        "POST /transformations/{id}/undo 200 (Receipt + riders)",
    );
    assert_eq!(
        undone.json["witness"],
        serde_json::json!("attested"),
        "🔴 R3 / `req/222` M-12: an undo over a world nobody moved, with the commit receipt in \
         place, is the case where DR-43-1's compare-and-set actually ran — and the word is how a \
         caller learns that: {}",
        undone.json
    );
}

/// 🔴 **DR-43-1, adopted (a)** at the HTTP face — the shape an undo takes when the world moved.
///
/// `req/38` §132 ruling 2 puts the CAS on both surfaces ("exit = the existing refusal code, HTTP =
/// 409 problem+json"), so this census exists to fix **which** shape that refusal is. The answer is
/// deliberately boring: `ProblemDetail`, the same five keys every other refusal on this router
/// carries, with 44 §2.3's own `PRECONDITION_CHANGED` in `gx_code`. No new response shape enters
/// the census — a ruling that had needed one would have been a wire change and therefore a
/// different DR.
///
/// The disturbance is a write to the target that never passed through the adapter, which is
/// `req/182` H-15's measurement A on the HTTP side. The witness comes out of this server's
/// [`MemoryArchive`], standing in for `.gx/receipts/`, which is the road
/// `gx_api::handlers::undo_witness` takes in production.
#[tokio::test]
async fn an_undo_over_a_world_that_moved_is_a_problem_detail_with_precondition_changed() {
    let server = Server::new("wire_census_undo_cas", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();
    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", &actor)),
        )
        .await;
    assert_eq!(created.status.as_u16(), 201, "{}", created.json);
    let id = id_of(&created.json);
    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    assert_eq!(verified.status.as_u16(), 200, "{}", verified.json);
    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    assert_eq!(committed.status.as_u16(), 200, "{}", committed.json);

    std::fs::write(&server.target, "a third party wrote this\n").expect("move the world");
    let refused = client
        .send(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({})),
        )
        .await;
    println!(
        "UNDO_CAS_HTTP status={} body={} target={:?}",
        refused.status.as_u16(),
        refused.json,
        std::fs::read_to_string(&server.target)
    );
    assert_eq!(
        refused.status.as_u16(),
        409,
        "44 §2.3 pairs `PRECONDITION_CHANGED` with 409, and the ruling mints nothing new: {}",
        refused.json
    );
    assert_eq!(refused.gx_code(), "PRECONDITION_CHANGED");
    assert_keys(
        &refused.json,
        &["detail", "gx_code", "status", "title", "type"],
        "POST /transformations/{id}/undo 409 (ProblemDetail, DR-43-1(a))",
    );
    assert_eq!(
        refused.content_type, "application/problem+json",
        "the single writer answers this road too"
    );
    assert_eq!(
        std::fs::read_to_string(&server.target).unwrap_or_default(),
        "a third party wrote this\n",
        "🔴 `req/182` H-15 measurement A on the HTTP face, inverted"
    );
}

/// 🔴 **DR-43-5 (3) (`req/38` §156 ruling 2(c))** — `BUSY` is the one refusal with a **sixth**
/// member, and this is where the shape is pinned.
///
/// Every other `ProblemDetail` on this router carries 44 §2.3's five keys exactly, and the probe
/// above measures that through nine roads. `BUSY` carries `retry_after_ms` as well, and the reason
/// is a measurement rather than a preference: `req/215` M-04 sampled `.gx/LOCK` every 2 ms across
/// one CLI commit and found it held about **50 ms per verb**, while RFC 9110's `Retry-After` is
/// `delay-seconds` and cannot say anything finer than `1`. R1b put the real number in `detail`,
/// where only a person can read it, and filed the shape question as a DR because 44 §2.3 fixes the
/// member set and this file pins it. The ruling is RFC 9457 §3.2's own provision — a problem type
/// may define extension members — applied to exactly one type.
///
/// **The asymmetry is the signal and is asserted from both sides**: the key's presence means "retry
/// this", and every code that is not `BUSY` is measured above as five keys, so a future refusal
/// that quietly grew a sixth turns one of the two censuses red.
///
/// Reaching it needs a writer that is not this router. `req/213` §2-3 put the exclusion on
/// `std::fs::File::try_lock`, whose lock belongs to an open file description rather than to a
/// process, so a second handle on `.gx/LOCK` — taken here, in this process — is what a second `gx`
/// would be. `crates/gx-cli/tests/serve_runtime_r2.rs` measures the same body across two real
/// processes; this measures the shape.
#[tokio::test]
async fn a_busy_refusal_carries_a_sixth_member_and_nothing_else_does() {
    let mut server = Server::new("wire_census_busy", "before\n");
    let other_writer = server.with_writer_lock();
    let client = server.client();
    let actor = server.keys.signing_id();

    let held = other_writer
        .acquire_owned("a second gx")
        .expect("the second handle takes the lock");

    let refused = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", &actor)),
        )
        .await;
    println!(
        "CENSUS BUSY: {} {} retry-after={:?}",
        refused.status.as_u16(),
        refused.json,
        refused.retry_after
    );
    assert_eq!(refused.status.as_u16(), 503);
    assert_eq!(refused.json["gx_code"], "BUSY");
    assert_keys(
        &refused.json,
        &[
            "detail",
            "gx_code",
            "retry_after_ms",
            "status",
            "title",
            "type",
        ],
        "ProblemDetail + BUSY's extension member (DR-43-5 (3))",
    );
    assert_eq!(
        refused.json["retry_after_ms"],
        serde_json::json!(gx_api::gx_code::BUSY_RETRY_AFTER_MILLIS),
        "the number is the measured hold, not the header's rounded second"
    );
    assert_eq!(refused.content_type, "application/problem+json");
    assert_eq!(
        refused.retry_after.as_deref(),
        Some("1"),
        "the header stays RFC 9110's delay-seconds; the member is what carries the measurement"
    );

    // 🔴 The control: the same endpoint, the same client, with the lock released. Without it this
    // probe would pass on a router that answered `503` to everything.
    drop(held);
    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", &actor)),
        )
        .await;
    println!("CENSUS BUSY control: {}", created.status.as_u16());
    assert_eq!(created.status.as_u16(), 201);
}

/// 🔴 `GET /healthz`, `POST .../cancel`, and `ProblemDetail` — the three shapes that need no
/// pipeline (44 §2.2 L469-471, L378-386; §2.3 L475-483).
///
/// `ProblemDetail` is measured three times through three refusals (404 / 422 / 401) on purpose:
/// `problem.rs::body` is the **single writer** for every error this crate answers, and three codes
/// through three different roads (handler refusal, id parse, the auth layer) showing one key set is
/// the measurement that the single-writer claim holds at the wire. RFC 9457 conformance rides in
/// the same assertions: the four RFC members (`type` a URI, `title`, `status` mirroring the HTTP
/// status code, `detail`) plus the one declared extension member (`gx_code`, §3.2 extension), under
/// `Content-Type: application/problem+json`.
#[tokio::test]
async fn healthz_cancel_and_every_problem_answer_carry_exactly_their_keys() {
    let server = Server::new("wire_census_small", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();

    // 44 §2.2 L471: "`{ "status": "ok", "engine_version": "<string>" }`" (sem: SEM-gx-api-442) — ~~exactly the two~~
    // 🔴 **four** since DR-44-6 (`req/38` §156 ruling 2(b)): `ledger_agrees` and `journal_rows`
    // join them, and the endpoint gained a second status. `req/219` §5(a) is the measurement that
    // forced it — a server refusing every write because its journal and ledger describe different
    // trees answered `{"status":"ok"}` to every monitor, so the one probe an orchestrator runs was
    // the one probe that could not see the fault. The census moves with the article text (44 §2.2
    // v0.4-n) rather than after it.
    let health = client.send("GET", "/v1/healthz", None).await;
    assert_eq!(health.status.as_u16(), 200);
    // 🔴 **R11 / `req/240` M-04** — **five**: `status_reason` joins them, and `status` gained a
    // second value. `req/240` M-04 deleted `.gx/VERSION` under a running server and measured this
    // object not changing by one character while every CLI verb on the same project refused
    // `DECLARATION_ABSENT` — a monitor cannot see a writer's door that is shut if the only word
    // this endpoint has is `ok`. `null` on a healthy project, which is what the census pins here.
    assert_keys(
        &health.json,
        &[
            "engine_version",
            "journal_rows",
            "ledger_agrees",
            "status",
            "status_reason",
        ],
        "GET /healthz 200",
    );
    assert_eq!(health.json["status"], "ok");
    assert_eq!(health.json["status_reason"], serde_json::Value::Null);
    assert_eq!(
        health.json["ledger_agrees"],
        serde_json::Value::Bool(true),
        "a fixture project's two files describe one tree"
    );

    // 44 §2.2 L384: "`{ transformation, state: "Aborted", reason: "OwnerCancelled" }`" (sem: SEM-gx-api-443) plus the
    // two declared riders: `actor_unchecked` (the field 44 requires and nothing checks — the name
    // is the disclosure, the feedback ledger §1.2; sem: SEM-gx-api-444) and `at`.
    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", &actor)),
        )
        .await;
    let id = id_of(&created.json);
    let cancelled = client
        .send(
            "POST",
            &format!("/v1/candidates/{id}/cancel"),
            Some(serde_json::json!({ "actor": { "Human": { "key": actor } } })),
        )
        .await;
    assert_eq!(cancelled.status.as_u16(), 200);
    assert_keys(
        &cancelled.json,
        &["actor_unchecked", "at", "reason", "state", "transformation"],
        "POST /candidates/{id}/cancel 200",
    );

    // §2.3's five, through ~~three~~ **nine** roads (v0.4-l, `req/189`: the four writers `req/182`
    // H-11 found outside 44 §2.3 — unrouted 404, wrong-method 405, path-segment parse, the
    // verdict-checkpoint body — plus H-10's headerless body (415), L-04's wrong media type
    // (415) and M-14's oversize body (413), each now answered by the one writer).
    let unknown = format!("gx1:{}", "a".repeat(52));
    let problem_shapes = [
        (
            client
                .send("GET", &format!("/v1/candidates/{unknown}"), None)
                .await,
            404,
            "NOT_FOUND",
            "ProblemDetail via a handler refusal",
        ),
        (
            client.send("GET", "/v1/candidates/not-an-id", None).await,
            422,
            "VALIDATION_ERROR",
            "ProblemDetail via the id parse",
        ),
        (
            server
                .anonymous()
                .send("GET", "/v1/candidates/x", None)
                .await,
            401,
            "UNAUTHORIZED",
            "ProblemDetail via the Bearer layer (44 §2.5)",
        ),
        (
            client.send("GET", "/v1/no-such-route", None).await,
            404,
            "NOT_FOUND",
            "ProblemDetail via the router fallback (H-11 ①, unrouted path)",
        ),
        (
            client
                .send(
                    "POST",
                    &format!("/v1/candidates/{unknown}"),
                    Some(serde_json::json!({})),
                )
                .await,
            405,
            "VALIDATION_ERROR",
            "ProblemDetail via the method-not-allowed fallback (H-11 ①)",
        ),
        (
            client
                .send("GET", "/v1/verdict-checkpoints/abc", None)
                .await,
            422,
            "VALIDATION_ERROR",
            "ProblemDetail via the path segment parse (H-11 ②, `Segment<u64>`)",
        ),
        (
            client
                .send(
                    "POST",
                    "/v1/verdict-checkpoints",
                    Some(serde_json::json!({ "origin": 5 })),
                )
                .await,
            422,
            "VALIDATION_ERROR",
            "ProblemDetail via the verdict-checkpoint body (H-11 ③, `Payload<IssueBody>`)",
        ),
        (
            client
                .send_raw(
                    "POST",
                    &format!("/v1/transformations/{unknown}/replay"),
                    &[],
                    br#"{"from":0,"to":1}"#.to_vec(),
                )
                .await,
            415,
            "UNSUPPORTED_MEDIA_TYPE",
            "ProblemDetail via a body with no Content-Type (H-10 / L-04)",
        ),
        (
            client
                .send_raw(
                    "POST",
                    "/v1/candidates",
                    &[("content-type", "application/json")],
                    vec![b' '; gx_api::MAX_BODY_BYTES + 1],
                )
                .await,
            413,
            "PAYLOAD_TOO_LARGE",
            "ProblemDetail via a body over MAX_BODY_BYTES (M-14)",
        ),
    ];
    for (answer, status, code, road) in problem_shapes {
        println!("PROBLEM {road}: {} {}", answer.status.as_u16(), answer.json);
        assert_eq!(answer.status.as_u16(), status, "{road}");
        assert_keys(
            &answer.json,
            &["detail", "gx_code", "status", "title", "type"],
            road,
        );
        assert_eq!(
            answer.content_type, "application/problem+json",
            "44 §2.3: \"every error response is `Content-Type: application/problem+json`\" (sem: SEM-gx-api-445) ({road})"
        );
        // RFC 9457 §3.1: `status` "duplicated storage" (sem: SEM-gx-api-446) must agree with the response's own status line.
        assert_eq!(
            answer.json["status"],
            serde_json::json!(status),
            "RFC 9457: the duplicated `status` member mirrors the HTTP status ({road})"
        );
        assert!(
            answer.json["type"]
                .as_str()
                .is_some_and(|t| t.starts_with("https://")),
            "44 §2.3: `type` is \"a URI naming the error kind\" (sem: SEM-gx-api-447) ({road}): {}",
            answer.json
        );
        assert_eq!(answer.gx_code(), code, "{road}");
    }
}

/// 44 §2.2 L374: "Response `200`: `{ transformation: TransformationId, state:
/// "Admitted"|"Denied" }`" (sem: SEM-gx-api-448) — plus the handler's five declared riders, `signed_by` above all
/// (INV-S6: without it a ruling signed by the wrong key is invisible at this surface).
///
/// The fixture is `tests/endpoints.rs`' road to an `Escalate`: an inverse over the escrow ceiling
/// (E-M3-4), then the ruler this server holds a key for. The verify answer on the Escalate arm is
/// also censused here — same eleven keys as the Admit arm — so the composite is measured
/// verdict-independent rather than assumed to be.
#[tokio::test]
async fn the_escalation_answer_carries_exactly_its_keys() {
    let server = Server::new("wire_census_escalation", &"x".repeat(1024 * 1024 + 4096));
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

    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    assert_eq!(verified.json["verdict"], "Escalate", "{}", verified.json);
    assert_keys(
        &verified.json,
        &[
            "at",
            "enforced",
            "fail_posture_engaged",
            "held_by",
            "proof",
            "reasons",
            "record_only",
            "state",
            "ticket",
            "transformation",
            "verdict",
        ],
        "POST /candidates/{id}/verify 200 (Escalate arm — same set as Admit's)",
    );

    let ruler = server.keys.ruler_id();
    let ruled = client
        .send(
            "POST",
            &format!("/v1/candidates/{id}/escalation"),
            Some(serde_json::json!({
                "decision": "approve",
                "reason": "the inverse is over the escrow ceiling and I accept that",
                "actor": { "Human": { "key": ruler } },
            })),
        )
        .await;
    assert_eq!(ruled.status.as_u16(), 200, "{}", ruled.json);
    assert_keys(
        &ruled.json,
        &[
            "at",
            "decision",
            "reason",
            "ruled_by",
            "signed_by",
            "state",
            "transformation",
        ],
        "POST /candidates/{id}/escalation 200",
    );
}

/// 44 §2.2 L424-444: the three verdict-checkpoint answers — the signed document's eight FR-M04
/// keys (as `m2_types.rs` fixes the type, confirmed unwrapped at the boundary, tally and signature
/// interiors included), and the page composite `{ items, next_cursor, total }` that exists only
/// here (`verdict_checkpoints::list` builds it with `json!` — L438's shape plus the declared
/// `total`).
#[tokio::test]
async fn the_verdict_checkpoint_answers_carry_exactly_their_keys() {
    let server = Server::new("wire_census_vc", "before\n");
    let client = server.client();
    // 🔴 **DR-44-9** (`req/38` §168 ruling 1 ④): eight signed members plus `window_start_at` /
    // `window_end_at` — the API layer resolving 42 §3.14's **verdict sequence numbers** against the
    // journal, because `req/187` §5 measured a reader taking `window_end` for a clock reading and
    // deriving a confidently wrong range from it. The indices stay (they are what the signature
    // covers); the two resolutions are outside it, and `tests/dr44_9_views.rs` holds them against
    // the engine's own tally.
    const VC_KEYS: [&str; 10] = [
        "ledger_root_hash",
        "ledger_tree_size",
        "origin",
        "signature",
        "tally",
        "timestamp",
        "window_end",
        "window_end_at",
        "window_start",
        "window_start_at",
    ];

    // 44 §2.2 L430: "Response `201 Created`: `VerdictCheckpoint` (42 §3.14)" (sem: SEM-gx-api-449) — an empty window is
    // a legitimate document ("issued as an empty window"), so no pipeline is needed for the shape.
    let issued = client.send("POST", "/v1/verdict-checkpoints", None).await;
    assert_eq!(issued.status.as_u16(), 201, "{}", issued.json);
    assert_keys(&issued.json, &VC_KEYS, "POST /verdict-checkpoints 201");
    assert_keys(
        &issued.json["tally"],
        &["admit", "deny", "escalate", "unverdicted"],
        "verdict-checkpoint.tally interior (FR-M04's four buckets)",
    );
    assert_keys(
        &issued.json["signature"],
        &["keyid", "sig"],
        "verdict-checkpoint.signature (NFR-011 note 5; sem: SEM-gx-api-450)",
    );

    // 44 §2.2 L438: "Response `200`: `{ items: VerdictCheckpoint[], next_cursor: int|null,
    // total: int }`" (sem: SEM-gx-api-451).
    let listed = client.send("GET", "/v1/verdict-checkpoints", None).await;
    assert_eq!(listed.status.as_u16(), 200);
    assert_keys(
        &listed.json,
        &["items", "next_cursor", "total"],
        "GET /verdict-checkpoints 200 (page composite)",
    );
    assert_keys(
        &listed.json["items"][0],
        &VC_KEYS,
        "verdict-checkpoint page item interior",
    );

    // 44 §2.2 L444: one checkpoint by the coordinate it closes at — the same document shape.
    let window_end = issued.json["window_end"].as_u64().expect("a window end");
    let one = client
        .send(
            "GET",
            &format!("/v1/verdict-checkpoints/{window_end}"),
            None,
        )
        .await;
    assert_eq!(one.status.as_u16(), 200);
    assert_keys(
        &one.json,
        &VC_KEYS,
        "GET /verdict-checkpoints/{window_end} 200",
    );
}

// ---------------------------------------------------------------------------
// `GET /stream` — the envelope and the ten event data shapes (44 §2.2 L446-467)
// ---------------------------------------------------------------------------

/// 44 §2.2's per-event `data` key sets, one row per event of the table at L454-465, each sorted.
///
/// A **table in the test** rather than assertions inline, so that
/// [`the_data_census_table_covers_exactly_the_ten_events`] can compare its domain against
/// `stream::EVENTS` — a census that silently lost an event would otherwise be a green suite over a
/// shrunk denominator (the exact failure `req/176` ruling 2 documented for detectors; sem: SEM-gx-api-452).
const EVENT_DATA_CENSUS: [(&str, &[&str]); 10] = [
    (
        "candidate.created",
        &["intent_digest", "locator", "substrate"],
    ),
    ("verify.started", &[]),
    ("verdict.issued", &["kind", "proof_digest"]),
    ("escalated", &["reasons", "ticket_id"]),
    ("human.decision", &["decision", "reason", "ticket_id"]),
    ("canonicalized", &["canonical_cid", "enforced"]),
    ("committed", &["canonical_cid", "enforced", "ledger_index"]),
    // 🔴 **R29 / `req/361` L-03** — the journal record has always held `rollback`; this arm used
    // to drop it with `..`. `null` where no roll-back was in question.
    ("aborted", &["reason", "rollback"]),
    ("superseded", &["superseded_by"]),
    (
        "receipt.issued",
        &["key_id", "receipt_digest", "receipt_kind"],
    ),
];

/// The census table's domain is exactly 44 §2.2's ten events — the denominator gate for the two
/// stream probes below.
#[test]
fn the_data_census_table_covers_exactly_the_ten_events() {
    let mut table: Vec<&str> = EVENT_DATA_CENSUS.iter().map(|(kind, _)| *kind).collect();
    table.sort_unstable();
    let mut events: Vec<&str> = gx_api::stream::EVENTS.to_vec();
    events.sort_unstable();
    println!("EVENT_DATA_CENSUS_DOMAIN={table:?}");
    assert_eq!(
        table, events,
        "the data census names a different event set than 44 §2.2's ten — a census over the wrong \
         denominator is the shrunk-detector failure, not a census"
    );
}

/// Read one NDJSON line — `tests/stream.rs`' reader, under its retry rule (0 retries, an expiry is
/// a failure).
async fn next_line(body: &mut axum::body::Body) -> serde_json::Value {
    use http_body_util::BodyExt;
    let frame = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
        .await
        .expect("a line arrived inside the wait (tests/stream.rs' rule: 0 retries)")
        .expect("the stream is still open")
        .expect("the frame is readable");
    let bytes = frame.into_data().expect("a data frame");
    let text = String::from_utf8(bytes.to_vec()).expect("utf-8");
    serde_json::from_str(text.trim()).expect("each line is one JSON object")
}

/// Every line the backlog holds, read off a real subscription, with the journal as the count's
/// denominator (`tests/stream.rs::no_line_ever_carries_a_record_the_map_keeps_inside`'s method).
async fn backlog(server: &Server) -> Vec<serde_json::Value> {
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
    lines
}

/// Census one stream line: the envelope's exact keys, and the exact `data` keys for its event.
///
/// The envelope is 44 §2.2 L451's four fields **plus `cursor`**, M6H6-2's declared addition (44
/// §2.6 permits it; NDJSON has no frame for `Last-Event-ID` to name otherwise) — five keys, so a
/// sixth rider needs a ruling before it ships.
fn assert_stream_line(line: &serde_json::Value) -> String {
    assert_keys(
        line,
        &["at", "cursor", "data", "event", "transformation"],
        "GET /stream envelope (44 §2.2 L451 + M6H6-2's cursor)",
    );
    let kind = line["event"].as_str().expect("an event name").to_string();
    let expected = EVENT_DATA_CENSUS
        .iter()
        .find(|(name, _)| *name == kind)
        .unwrap_or_else(|| panic!("`{kind}` is not in the data census table"))
        .1;
    assert_keys(&line["data"], expected, &format!("stream data of `{kind}`"));
    kind
}

/// 🔴 The stream's envelope and eight of the ten data shapes, measured on every line of a full
/// pipeline (44 §2.2 L446-467).
///
/// The fixture drives commit → undo (superseded's producer) and a second candidate's cancel
/// (aborted's) so that the backlog holds all eight non-escalation events; **every** line is
/// censused, not the first of each kind, because the composite is rebuilt per record and a shape
/// correct once is not a shape correct always. `escalated`/`human.decision` need a different
/// fixture and are the next probe's — the two probes' union is gated against the table by
/// [`the_data_census_table_covers_exactly_the_ten_events`].
#[tokio::test(flavor = "multi_thread")]
async fn every_stream_line_of_the_pipeline_carries_exactly_the_censused_keys() {
    let server = Server::new("wire_census_stream", "before\n");
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
    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    assert_eq!(committed.status.as_u16(), 200, "{}", committed.json);
    let undone = client
        .send(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({})),
        )
        .await;
    assert_eq!(undone.status.as_u16(), 200, "{}", undone.json);
    let second = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after-second\n", &actor)),
        )
        .await;
    let second_id = id_of(&second.json);
    let cancelled = client
        .send(
            "POST",
            &format!("/v1/candidates/{second_id}/cancel"),
            Some(serde_json::json!({ "actor": { "Human": { "key": actor } } })),
        )
        .await;
    assert_eq!(cancelled.status.as_u16(), 200, "{}", cancelled.json);

    let lines = backlog(&server).await;
    let mut seen: Vec<String> = lines.iter().map(assert_stream_line).collect();
    seen.sort_unstable();
    seen.dedup();
    println!("STREAM_CENSUS_LINES={} KINDS={seen:?}", lines.len());
    for kind in [
        "candidate.created",
        "verify.started",
        "verdict.issued",
        "receipt.issued",
        "canonicalized",
        "committed",
        "superseded",
        "aborted",
    ] {
        assert!(
            seen.contains(&kind.to_string()),
            "the fixture was built to produce `{kind}` and the backlog holds none — the census \
             ran over a smaller denominator than declared"
        );
    }
}

/// The two escalation events' data shapes, on the fixture that produces them (E-M3-4's ceiling,
/// then T-5's approve) — the other half of the ten, same census per line.
#[tokio::test(flavor = "multi_thread")]
async fn the_escalation_stream_lines_carry_exactly_the_censused_keys() {
    let server = Server::new("wire_census_stream_esc", &"x".repeat(1024 * 1024 + 4096));
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
    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    assert_eq!(verified.json["verdict"], "Escalate", "{}", verified.json);
    let ruled = client
        .send(
            "POST",
            &format!("/v1/candidates/{id}/escalation"),
            Some(serde_json::json!({
                "decision": "approve",
                "reason": "over the ceiling and accepted",
                "actor": { "Human": { "key": server.keys.ruler_id() } },
            })),
        )
        .await;
    assert_eq!(ruled.status.as_u16(), 200, "{}", ruled.json);

    let lines = backlog(&server).await;
    let mut seen: Vec<String> = lines.iter().map(assert_stream_line).collect();
    seen.sort_unstable();
    seen.dedup();
    println!("STREAM_ESC_CENSUS_LINES={} KINDS={seen:?}", lines.len());
    for kind in ["escalated", "human.decision"] {
        assert!(
            seen.contains(&kind.to_string()),
            "the fixture was built to produce `{kind}` and the backlog holds none"
        );
    }
}

// ---------------------------------------------------------------------------
// 🔴 v0.4-k — the four EXTENSION endpoints (44 §2.7 v0.4-k addendum, `req/181`; M6-05, adopted (a); sem: SEM-gx-api-453)
// ---------------------------------------------------------------------------

/// 44 §2.7 v0.4-k addendum, "the three list endpoints' shared contract" (sem: SEM-gx-api-454): `{ items: Row[], next_cursor: string|null }` — two
/// keys, **no `total`** (that key is `GET /verdict-checkpoints`' own §2.2 declaration and not part
/// of the §2.7 regulation), measured on a page whether it is full or empty.
fn assert_page<'a>(page: &'a serde_json::Value, shape: &str) -> &'a serde_json::Value {
    assert_keys(page, &["items", "next_cursor"], shape);
    assert!(
        page["items"].is_array(),
        "{shape}: 44 §2.7 \"`items: [...]`\" (sem: SEM-gx-api-455): {}",
        page["items"]
    );
    assert!(
        page["next_cursor"].is_null() || page["next_cursor"].is_string(),
        "{shape}: 44 §2.7 \"`next_cursor: string|null`\" (sem: SEM-gx-api-456): {}",
        page["next_cursor"]
    );
    &page["items"]
}

/// 🔴 The three list pages and the consistency answer, each exactly its keys (44 §2.7 v0.4-k addendum
/// "`GET /candidates`"/"`GET /transformations`"/"`GET /ledger/consistency`" (sem: SEM-gx-api-457) and the shared
/// page contract; `GET /escalations` is the next probe's, on the fixture that produces a ticket).
///
/// One flow because the rows chain: a candidate is a `GET /candidates` row until it commits, a
/// `GET /transformations` row forever, and two commits (commit + undo) are what a consistency proof
/// between tree sizes 1 and 2 needs. **Every** row of every page is censused, not the first — a
/// row is rebuilt per transformation (`row_json` + `Map::insert` for the two extra keys) and
/// `superseded_by`/`inverse_status` take their non-null values only after the undo, so the second
/// pass is the one that would catch a key that appears conditionally.
///
/// `GET /ledger/consistency` is pinned **as shipped** (`{from, to, proof}` + 42 §3.11's three-key
/// proof interior) under 🔴 DR-44-1 (44 §2.7 v0.4-k addendum; sem: SEM-gx-api-458): the census does not decide the ruling,
/// it keeps the wire from drifting silently while the ruling is open.
#[tokio::test]
async fn the_extension_lists_and_the_consistency_answer_carry_exactly_their_keys() {
    let server = Server::new("wire_census_ext", "before\n");
    let client = server.client();
    let actor = server.keys.signing_id();

    // An empty page is a page: same two keys, `items: []`, `next_cursor: null` (44 §2.7 v0.4-k
    // "when not full, `null`" (sem: SEM-gx-api-459)).
    let empty = client.send("GET", "/v1/candidates", None).await;
    assert_eq!(empty.status.as_u16(), 200, "{}", empty.json);
    let items = assert_page(&empty.json, "GET /candidates 200 (empty page)");
    assert_eq!(items.as_array().map(Vec::len), Some(0));
    assert!(empty.json["next_cursor"].is_null());

    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", &actor)),
        )
        .await;
    assert_eq!(created.status.as_u16(), 201, "{}", created.json);
    let id = id_of(&created.json);

    // 44 §2.7 v0.4-k "`GET /candidates`" (sem: SEM-gx-api-460): an in-flight row is exactly
    // `{transformation, state, verdict, enforced}` — four keys, the id as text (not §2.2's
    // `Transformation` body).
    let in_flight = client.send("GET", "/v1/candidates", None).await;
    assert_eq!(in_flight.status.as_u16(), 200);
    let items = assert_page(&in_flight.json, "GET /candidates 200 (one in-flight row)");
    let rows = items.as_array().expect("items");
    assert_eq!(rows.len(), 1, "{}", in_flight.json);
    // 🔴 v0.4-l M-15 (`req/189`, 44 §2.7 v0.4-l addendum): the four keys plus `actor` / `created_at`
    // / `scope` — additive, each the engine's own answer off the live row.
    for row in rows {
        assert_keys(
            row,
            &[
                "actor",
                "created_at",
                "enforced",
                "scope",
                "state",
                "transformation",
                "verdict",
            ],
            "GET /candidates row",
        );
    }
    assert_eq!(rows[0]["transformation"], serde_json::json!(id));

    // Commit, then undo: two ledger entries (a consistency proof needs a tree that grew), one
    // superseded row and one escrow row (`superseded_by`/`inverse_status` non-null on the way out).
    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    assert_eq!(verified.status.as_u16(), 200, "{}", verified.json);
    let committed = client
        .send("POST", &format!("/v1/candidates/{id}/commit"), None)
        .await;
    assert_eq!(committed.status.as_u16(), 200, "{}", committed.json);
    let undone = client
        .send(
            "POST",
            &format!("/v1/transformations/{id}/undo"),
            Some(serde_json::json!({})),
        )
        .await;
    assert_eq!(undone.status.as_u16(), 200, "{}", undone.json);

    // 44 §2.7 v0.4-k "`GET /transformations`" (sem: SEM-gx-api-461): every row is exactly the six keys — the
    // `GET /candidates` four plus `superseded_by`/`inverse_status` — terminal rows included, and
    // measured on **all** rows (the superseded one carries a non-null `superseded_by`; both arms
    // of that key on one page).
    let all = client.send("GET", "/v1/transformations", None).await;
    assert_eq!(all.status.as_u16(), 200);
    let items = assert_page(&all.json, "GET /transformations 200 (full history)");
    let rows = items.as_array().expect("items");
    assert_eq!(rows.len(), 2, "commit + undo = two rows: {}", all.json);
    // 🔴 v0.4-l M-15: six + the same three additive keys = nine, on every row.
    for row in rows {
        assert_keys(
            row,
            &[
                "actor",
                "created_at",
                "enforced",
                "inverse_status",
                // 🔴 **R29 / `req/361` §3-1** — nine + one = ten, on every row.
                "rollback",
                "scope",
                "state",
                "superseded_by",
                "transformation",
                "verdict",
            ],
            "GET /transformations row",
        );
    }
    println!("TRANSFORMATIONS_ROWS={}", all.json["items"]);
    assert!(
        rows.iter().any(|r| r["superseded_by"].is_string()),
        "the undo supersedes the original, so one row names its successor: {}",
        all.json
    );

    // 44 §2.7 v0.4-k "`next_cursor`: when a page is filled to `limit` items, it is the last item's cursor" (sem: SEM-gx-api-462): with two
    // rows and `limit=1` the first page is full, so `next_cursor` is a string this server minted —
    // and the page composite is still exactly the two keys.
    let first = client
        .send("GET", "/v1/transformations?limit=1", None)
        .await;
    assert_eq!(first.status.as_u16(), 200);
    let items = assert_page(&first.json, "GET /transformations?limit=1 200 (full page)");
    assert_eq!(items.as_array().map(Vec::len), Some(1));
    assert!(
        first.json["next_cursor"].is_string(),
        "a full page hands out the last included cursor: {}",
        first.json
    );

    // After the undo the original is `Superseded` and the undo is `Committed`: `GET /candidates`
    // is empty again — the same page composite on the way back down.
    let drained = client.send("GET", "/v1/candidates", None).await;
    let items = assert_page(&drained.json, "GET /candidates 200 (drained)");
    assert_eq!(items.as_array().map(Vec::len), Some(0), "{}", drained.json);

    // 🔴 44 §2.7 v0.4-k "`GET /ledger/consistency`" (sem: SEM-gx-api-463) **as shipped, on the wire**: `{from, to, proof}`, and the
    // proof interior is 42 §3.11's `{old_size, new_size, path}` (the `ConsistencyProof` derive,
    // confirmed at the boundary as `m2_types` cannot — it lives in gx-log). DR-44-1 is open on the
    // wrapper; this pins what ships so that neither a silent unwrapping nor a fourth key can land
    // without turning this RED first.
    // 🔴 DR-44-1 **ruled = option (a) bare** (`req/38` §119 ruling 2, landed v0.4-l `req/189`): the answer
    // is the bare `ConsistencyProof` — 42 §3.11's three keys at the top level, no wrapper. The
    // three-key wrapper census above (v0.4-k) is superseded by this one in the same commit as
    // the handler and 44 §2.7, which is what the v0.4-k note asked for.
    let consistency = client
        .send("GET", "/v1/ledger/consistency?from=1&to=2", None)
        .await;
    assert_eq!(consistency.status.as_u16(), 200, "{}", consistency.json);
    // 🔴 **DR-44-9** (`req/38` §168 ruling 1 ③, `req/187` §5's loudest filing): three riders on
    // the bare proof — `consistent` (the judgement this server already computed in order to answer
    // at all) and `checked_from`/`checked_to` (its subject). DR-44-1 (a) is **not** reversed: the
    // proof's own three keys are still at the top level and a client that reads `old_size` reads
    // the same thing it read yesterday. The declared exception to "carry the proof, do not grade
    // it" is written in `list.rs`'s doc comment and in 44 §2.7's v0.4-s addendum, and the reason it
    // is granted is that a ledger face's only alarm was **unreachable** without it.
    assert_keys(
        &consistency.json,
        &[
            "checked_from",
            "checked_to",
            "consistent",
            "new_size",
            "old_size",
            "path",
        ],
        "GET /ledger/consistency 200 (bare ConsistencyProof + DR-44-9's judgement)",
    );
    assert_eq!(consistency.json["consistent"], serde_json::json!(true));
    assert_eq!(
        consistency.json["checked_from"],
        consistency.json["old_size"]
    );
    assert_eq!(consistency.json["checked_to"], consistency.json["new_size"]);
    assert_eq!(consistency.json["old_size"], serde_json::json!(1));
    assert_eq!(consistency.json["new_size"], serde_json::json!(2));
}

/// 🔴 44 §2.7 v0.4-k addendum, "`GET /escalations`" (sem: SEM-gx-api-464): the ticket row is exactly its seven keys, on the
/// fixture that produces a ticket (E-M3-4's escrow ceiling, as `tests/endpoints.rs` and the
/// escalation probe above use), and the page is 44 §2.7's two-key composite before and after —
/// empty first (the queue is empty until something escalates) so the shape is measured on both
/// arms of `next_cursor` for this endpoint too.
#[tokio::test]
async fn the_escalations_page_carries_exactly_its_row_keys() {
    let server = Server::new("wire_census_ext_esc", &"x".repeat(1024 * 1024 + 4096));
    let client = server.client();
    let actor = server.keys.signing_id();

    let empty = client.send("GET", "/v1/escalations", None).await;
    assert_eq!(empty.status.as_u16(), 200, "{}", empty.json);
    let items = assert_page(&empty.json, "GET /escalations 200 (empty queue)");
    assert_eq!(items.as_array().map(Vec::len), Some(0));

    let created = client
        .send(
            "POST",
            "/v1/candidates",
            Some(server.intent_body("after\n", &actor)),
        )
        .await;
    let id = id_of(&created.json);
    let verified = client
        .send("POST", &format!("/v1/candidates/{id}/verify"), None)
        .await;
    assert_eq!(verified.json["verdict"], "Escalate", "{}", verified.json);

    let queue = client.send("GET", "/v1/escalations", None).await;
    assert_eq!(queue.status.as_u16(), 200);
    let items = assert_page(&queue.json, "GET /escalations 200 (one ticket)");
    let rows = items.as_array().expect("items");
    assert_eq!(rows.len(), 1, "{}", queue.json);
    for row in rows {
        assert_keys(
            row,
            &[
                "created_at",
                "deadline",
                "reasons",
                "required_approval",
                "state",
                "ticket_id",
                "transformation",
            ],
            "GET /escalations row",
        );
    }
    println!("ESCALATIONS_ROW={}", rows[0]);
    assert_eq!(rows[0]["transformation"], serde_json::json!(id));
    assert!(
        rows[0]["reasons"].as_array().is_some_and(|r| !r.is_empty()),
        "a ticket carries the reasons it was raised for (43 T-4c): {}",
        rows[0]
    );
}
