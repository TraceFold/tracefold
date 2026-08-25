// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **P2 — the CRUD-22-capability structural conformance suite** (`req/506` §1 P2, §2 P2 AC).
//!
//! # What this suite is, and why it is defensive tooling
//!
//! Glovrex/Tracefold is an agent-safety layer, and this file is a *read-only self-check* of its own
//! HTTP surface: it drives the shipped router with `tower::ServiceExt::oneshot` (the same socketless
//! client `support::Server` gives every gx-api suite) and reads back which `(method, path)` pairs the
//! router recognises. It writes nothing, attacks nothing, and touches no third-party system — the
//! "negative control" below is a router built inside this test process. The vocabulary here
//! ("absent", "negative control") is conformance vocabulary, not an attack.
//!
//! # The claim, in one sentence
//!
//! `req/485` declares a `resource × CRUD` matrix for gx's API surface: some cells are occupied by a
//! real route, and the rest are **structurally** empty for a stated reason (a named constant such as
//! `LEDGER_IS_APPEND_ONLY`). Today that matrix lives only as prose in `req/485` §1. This suite turns
//! it into a machine fact: every occupied cell answers over the real router, and every empty cell is
//! measured to have **no** route — so the reason constants stop being a table somebody has to trust
//! and become a check a third party can run with `cargo test -p gx-api --test crud22_conformance`.
//!
//! This is `req/506`'s "conformance" sense ②: the correspondence between what gx *declares* (the
//! catalogue of capabilities) and what the binary/wire *does*. It is the third of the three
//! "keep the declaration from rotting" instruments `req/506` §0 names — `limits_sync.rs` pins
//! declaration-text against declaration-text, `truncationSurvives` pins a declared *limit's* boundary,
//! and this pins a declared *capability's* presence and, symmetrically, a declared *absence*.
//!
//! # Why the absence half is the new work (and is not `router.rs`)
//!
//! `tests/router.rs` already walks `SPECIFIED_ENDPOINTS` + `EXTENSION_ENDPOINTS` and asserts each
//! **served** path answers (a positive claim: the route exists). What no suite states is the other
//! half of the matrix — that `DELETE /transformations/{id}` is refused *because the ledger is
//! append-only*, that `POST /receipts` has no route *because a receipt is engine-issued and
//! immutable*, and so on for all thirty empty cells. A green router suite over occupied routes says
//! nothing about a route that was added by accident to a cell `req/485` says is structurally empty.
//! This suite is that missing half, plus a negative control proving the emptiness check can actually
//! fail.
//!
//! # The routing discriminator (how "present" is told from "absent")
//!
//! Three answers, distinguished by status and — for the ambiguous 404 — by the fallback's own words:
//!
//! * **`405`** → `MethodNotAllowed`: `handlers::method_not_allowed` fires only for a *known path*
//!   asked with a method it has no handler for. This is the shape a wrong CRUD verb on an existing
//!   resource path takes, e.g. `DELETE /v1/transformations/{id}`.
//! * **`404` whose `detail` contains "is not a route of this server"** → `Unrouted`: the outer
//!   `handlers::not_found` fallback, reached when the *path itself* is unregistered, e.g.
//!   `POST /v1/receipts` (no collection route) or `GET /v1/escalations/{id}` (no by-id route).
//! * **anything else** → `Routed`: the handler ran. A handler's own `404 NOT_FOUND` for a missing
//!   object is `Routed` — it carries a *different* `detail` (it names the object, not the route), and
//!   `problem.rs` is the single writer that makes the two 404s tell apart. `200`/`201`/`401`/`422`
//!   are all `Routed` too; the CRUD probes here never assert a status beyond routed-vs-not, so a
//!   probe that lands on a handler needs no valid object to prove the route exists.
//!
//! Both `MethodNotAllowed` and `Unrouted` mean "no such capability". A false *pass* would need the
//! discriminator to call a handled route "absent", which cannot happen: a handled `(method, path)`
//! never yields `405` and never yields the router fallback. A doctored route that broke the absence
//! claim would therefore show up as `Routed` — which is exactly what the negative control injects.
//!
//! # The denominator, and where it disagrees with `req/485` (ground-truth over digest)
//!
//! `req/485` §1 tables eleven resources × C/R/U/D = **44 cells** and summarises "14 real / 30 empty".
//! That summary undercounts its own §3 enumeration, which lists **17** occupied cells. Measured
//! against the crate's own declared endpoint lists (`SPECIFIED_ENDPOINTS` 14 + `EXTENSION_ENDPOINTS`
//! 4 + `VERDICT_CHECKPOINT_ENDPOINTS` 3 = 21 HTTP routes), the honest count over the HTTP surface is
//! **16 occupied CRUD cells / 28 empty CRUD cells**:
//!
//! * the escalation route (`POST /candidates/{id}/escalation`) legitimately occupies two cells,
//!   `Candidate/U` and `Escalation/C`, so 21 routes collapse to 16 distinct occupied cells;
//! * `OfflineVerification/R` (`verifyReceiptOffline`) is the 22nd capability but is a wasm/SDK **pure
//!   function with no HTTP route** — `req/485` §3 counts it occupied, but under the P2 AC's own words
//!   ("the declared verb exists as a real POST/GET **route**") it is absent from *this* surface, so
//!   its whole row is four empty cells with reason `LOCAL_PURE_FUNCTION_NO_RESOURCE`.
//!
//! `req/506` §0 authorises exactly this: "re-grep the coordinates at launch and adopt the measured
//! value if they differ". The count below is therefore declared by the matrix in this file and gated
//! by [`the_matrix_is_the_declared_surface_and_the_eleven_reasons`] — a census over a shrunk
//! denominator is not a census, so the gate pins 44 cells, 16 occupied, 28 empty, and exactly the
//! eleven reason constants.

mod support;

use support::{Server, TOKEN};

/// The eleven reason constants `req/485` §1 names for the structurally-empty cells, as first-class
/// values so a third party reads them in the source rather than inferring them from a prose table.
/// English, and spelled as `req/485` spells them.
mod reason {
    /// A transformation is not created directly; it is derived from an admitted candidate.
    pub const TRANSFORMATION_IS_DERIVED: &str = "TRANSFORMATION_IS_DERIVED";
    /// The ledger is append-only, so a transformation cannot be deleted.
    pub const LEDGER_IS_APPEND_ONLY: &str = "LEDGER_IS_APPEND_ONLY";
    /// A receipt is issued by the engine and is immutable; it is neither created, updated nor
    /// deleted over the API.
    pub const RECEIPT_IS_ENGINE_ISSUED_IMMUTABLE: &str = "RECEIPT_IS_ENGINE_ISSUED_IMMUTABLE";
    /// A proof is derived from the Merkle tree, not stored and mutated.
    pub const PROOF_IS_DERIVED_FROM_TREE: &str = "PROOF_IS_DERIVED_FROM_TREE";
    /// A checkpoint is issued by the engine.
    pub const CHECKPOINT_IS_ENGINE_ISSUED: &str = "CHECKPOINT_IS_ENGINE_ISSUED";
    /// An escalation is resolved through its candidate, not updated or deleted as its own resource.
    pub const ESCALATION_RESOLVES_VIA_CANDIDATE: &str = "ESCALATION_RESOLVES_VIA_CANDIDATE";
    /// There is no by-id read route for an escalation; the list is the read face.
    pub const NO_ROUTE_FOR_ESCALATION_BY_ID: &str = "NO_ROUTE_FOR_ESCALATION_BY_ID";
    /// A verdict checkpoint is immutable once issued.
    pub const VERDICT_CHECKPOINT_IS_IMMUTABLE_ONCE_ISSUED: &str =
        "VERDICT_CHECKPOINT_IS_IMMUTABLE_ONCE_ISSUED";
    /// Health is observation only.
    pub const OBSERVATION_ONLY: &str = "OBSERVATION_ONLY";
    /// The event stream is observation only.
    pub const STREAM_IS_OBSERVATION_ONLY: &str = "STREAM_IS_OBSERVATION_ONLY";
    /// Offline verification is a local pure function with no server-side resource or route.
    pub const LOCAL_PURE_FUNCTION_NO_RESOURCE: &str = "LOCAL_PURE_FUNCTION_NO_RESOURCE";

    /// All eleven, for the denominator gate.
    pub const ALL: [&str; 11] = [
        TRANSFORMATION_IS_DERIVED,
        LEDGER_IS_APPEND_ONLY,
        RECEIPT_IS_ENGINE_ISSUED_IMMUTABLE,
        PROOF_IS_DERIVED_FROM_TREE,
        CHECKPOINT_IS_ENGINE_ISSUED,
        ESCALATION_RESOLVES_VIA_CANDIDATE,
        NO_ROUTE_FOR_ESCALATION_BY_ID,
        VERDICT_CHECKPOINT_IS_IMMUTABLE_ONCE_ISSUED,
        OBSERVATION_ONLY,
        STREAM_IS_OBSERVATION_ONLY,
        LOCAL_PURE_FUNCTION_NO_RESOURCE,
    ];
}

/// The four CRUD verbs — the matrix's second axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Crud {
    Create,
    Read,
    Update,
    Delete,
}

impl Crud {
    fn tag(self) -> &'static str {
        match self {
            Crud::Create => "C",
            Crud::Read => "R",
            Crud::Update => "U",
            Crud::Delete => "D",
        }
    }
}

/// How the router answered a `(method, path)` probe — the one measurement every row below makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Routing {
    /// A handler ran (any status that is not `405` and not the unrouted fallback).
    Routed,
    /// `405`: a known path asked with a method it has no handler for.
    MethodNotAllowed,
    /// `404` from `handlers::not_found`: the path itself is unregistered.
    Unrouted,
}

impl Routing {
    fn is_routed(self) -> bool {
        matches!(self, Routing::Routed)
    }
}

// ---------------------------------------------------------------------------
// The matrix. Eleven resources × C/R/U/D = 44 cells. Occupied cells carry the
// declared endpoint string (so the denominator gate can cross-check them against
// the crate's own endpoint lists); empty cells carry the reason constant and the
// canonical HTTP verb whose absence proves the reason.
// ---------------------------------------------------------------------------

/// An occupied cell: a declared route, spelled exactly as the crate's endpoint constants spell it
/// (so `{id}`/`{tid}`/`{window_end}` templates match `SPECIFIED_ENDPOINTS` et al.). One route per
/// row; a cell with several routes (`Candidate/U` has three) appears as several rows.
struct Occupied {
    resource: &'static str,
    verb: Crud,
    /// `"METHOD /path"`, as in `SPECIFIED_ENDPOINTS` — the escalation route appears twice (two cells).
    endpoint: &'static str,
}

const OCCUPIED: &[Occupied] = &[
    o("Candidate", Crud::Create, "POST /candidates"),
    o("Candidate", Crud::Read, "GET /candidates/{id}"),
    o("Candidate", Crud::Read, "GET /candidates"),
    o("Candidate", Crud::Update, "POST /candidates/{id}/verify"),
    o("Candidate", Crud::Update, "POST /candidates/{id}/commit"),
    o(
        "Candidate",
        Crud::Update,
        "POST /candidates/{id}/escalation",
    ),
    o("Candidate", Crud::Delete, "POST /candidates/{id}/cancel"),
    o("Transformation", Crud::Read, "GET /transformations/{id}"),
    o("Transformation", Crud::Read, "GET /transformations"),
    o(
        "Transformation",
        Crud::Update,
        "POST /transformations/{id}/undo",
    ),
    o(
        "Transformation",
        Crud::Update,
        "POST /transformations/{id}/replay",
    ),
    o("Receipt", Crud::Read, "GET /receipts/{tid}"),
    o("LedgerProof", Crud::Read, "GET /ledger/proof"),
    o("LedgerCheckpoint", Crud::Read, "GET /ledger/checkpoint"),
    o("LedgerConsistency", Crud::Read, "GET /ledger/consistency"),
    o("EventStream", Crud::Read, "GET /stream"),
    o("Escalation", Crud::Read, "GET /escalations"),
    // escalateCandidate is one route occupying two cells (Candidate/U above, Escalation/C here).
    o(
        "Escalation",
        Crud::Create,
        "POST /candidates/{id}/escalation",
    ),
    o(
        "VerdictCheckpoint",
        Crud::Create,
        "POST /verdict-checkpoints",
    ),
    o("VerdictCheckpoint", Crud::Read, "GET /verdict-checkpoints"),
    o(
        "VerdictCheckpoint",
        Crud::Read,
        "GET /verdict-checkpoints/{window_end}",
    ),
    o("Health", Crud::Read, "GET /healthz"),
];

const fn o(resource: &'static str, verb: Crud, endpoint: &'static str) -> Occupied {
    Occupied {
        resource,
        verb,
        endpoint,
    }
}

/// An empty cell: no route, for a stated reason. `method`/`probe` is the canonical HTTP form of the
/// missing verb; probing it must answer *not routed*.
struct Empty {
    resource: &'static str,
    verb: Crud,
    reason: &'static str,
    method: &'static str,
    /// A concrete path (templates already filled: `{id}`→`x`, `{window_end}`→`1`).
    probe: &'static str,
}

const fn e(
    resource: &'static str,
    verb: Crud,
    reason: &'static str,
    method: &'static str,
    probe: &'static str,
) -> Empty {
    Empty {
        resource,
        verb,
        reason,
        method,
        probe,
    }
}

/// The twenty-eight structurally-empty CRUD cells, plus one sub-route emptiness
/// (`GET /escalations/{id}`) so that all eleven reason constants appear. The `NO_ROUTE_FOR_
/// ESCALATION_BY_ID` row is not a CRUD cell of the 44 — `Escalation/R` is occupied by the list — so
/// it is flagged `sub_route: true` and excluded from the 28 count in the gate.
struct EmptyRow {
    cell: Empty,
    sub_route: bool,
}

const fn crud(cell: Empty) -> EmptyRow {
    EmptyRow {
        cell,
        sub_route: false,
    }
}

const fn sub(cell: Empty) -> EmptyRow {
    EmptyRow {
        cell,
        sub_route: true,
    }
}

const EMPTY: &[EmptyRow] = &[
    // Transformation: C derived, D append-only. R and U are occupied.
    crud(e(
        "Transformation",
        Crud::Create,
        reason::TRANSFORMATION_IS_DERIVED,
        "POST",
        "/v1/transformations",
    )),
    crud(e(
        "Transformation",
        Crud::Delete,
        reason::LEDGER_IS_APPEND_ONLY,
        "DELETE",
        "/v1/transformations/x",
    )),
    // Receipt: only R. C has no collection route (Unrouted); U/D are wrong methods on the item.
    crud(e(
        "Receipt",
        Crud::Create,
        reason::RECEIPT_IS_ENGINE_ISSUED_IMMUTABLE,
        "POST",
        "/v1/receipts",
    )),
    crud(e(
        "Receipt",
        Crud::Update,
        reason::RECEIPT_IS_ENGINE_ISSUED_IMMUTABLE,
        "PUT",
        "/v1/receipts/x",
    )),
    crud(e(
        "Receipt",
        Crud::Delete,
        reason::RECEIPT_IS_ENGINE_ISSUED_IMMUTABLE,
        "DELETE",
        "/v1/receipts/x",
    )),
    // LedgerProof: only R (derived from the tree).
    crud(e(
        "LedgerProof",
        Crud::Create,
        reason::PROOF_IS_DERIVED_FROM_TREE,
        "POST",
        "/v1/ledger/proof",
    )),
    crud(e(
        "LedgerProof",
        Crud::Update,
        reason::PROOF_IS_DERIVED_FROM_TREE,
        "PUT",
        "/v1/ledger/proof",
    )),
    crud(e(
        "LedgerProof",
        Crud::Delete,
        reason::PROOF_IS_DERIVED_FROM_TREE,
        "DELETE",
        "/v1/ledger/proof",
    )),
    // LedgerCheckpoint: only R (engine-issued).
    crud(e(
        "LedgerCheckpoint",
        Crud::Create,
        reason::CHECKPOINT_IS_ENGINE_ISSUED,
        "POST",
        "/v1/ledger/checkpoint",
    )),
    crud(e(
        "LedgerCheckpoint",
        Crud::Update,
        reason::CHECKPOINT_IS_ENGINE_ISSUED,
        "PUT",
        "/v1/ledger/checkpoint",
    )),
    crud(e(
        "LedgerCheckpoint",
        Crud::Delete,
        reason::CHECKPOINT_IS_ENGINE_ISSUED,
        "DELETE",
        "/v1/ledger/checkpoint",
    )),
    // LedgerConsistency: only R (derived from the tree).
    crud(e(
        "LedgerConsistency",
        Crud::Create,
        reason::PROOF_IS_DERIVED_FROM_TREE,
        "POST",
        "/v1/ledger/consistency",
    )),
    crud(e(
        "LedgerConsistency",
        Crud::Update,
        reason::PROOF_IS_DERIVED_FROM_TREE,
        "PUT",
        "/v1/ledger/consistency",
    )),
    crud(e(
        "LedgerConsistency",
        Crud::Delete,
        reason::PROOF_IS_DERIVED_FROM_TREE,
        "DELETE",
        "/v1/ledger/consistency",
    )),
    // Escalation: C (via candidate) and R (list) are occupied; U/D resolve via the candidate.
    crud(e(
        "Escalation",
        Crud::Update,
        reason::ESCALATION_RESOLVES_VIA_CANDIDATE,
        "PUT",
        "/v1/escalations",
    )),
    crud(e(
        "Escalation",
        Crud::Delete,
        reason::ESCALATION_RESOLVES_VIA_CANDIDATE,
        "DELETE",
        "/v1/escalations",
    )),
    // VerdictCheckpoint: C and R occupied; immutable once issued (no U/D).
    crud(e(
        "VerdictCheckpoint",
        Crud::Update,
        reason::VERDICT_CHECKPOINT_IS_IMMUTABLE_ONCE_ISSUED,
        "PUT",
        "/v1/verdict-checkpoints/1",
    )),
    crud(e(
        "VerdictCheckpoint",
        Crud::Delete,
        reason::VERDICT_CHECKPOINT_IS_IMMUTABLE_ONCE_ISSUED,
        "DELETE",
        "/v1/verdict-checkpoints/1",
    )),
    // Health: only R (observation only).
    crud(e(
        "Health",
        Crud::Create,
        reason::OBSERVATION_ONLY,
        "POST",
        "/v1/healthz",
    )),
    crud(e(
        "Health",
        Crud::Update,
        reason::OBSERVATION_ONLY,
        "PUT",
        "/v1/healthz",
    )),
    crud(e(
        "Health",
        Crud::Delete,
        reason::OBSERVATION_ONLY,
        "DELETE",
        "/v1/healthz",
    )),
    // EventStream: only R (observation only).
    crud(e(
        "EventStream",
        Crud::Create,
        reason::STREAM_IS_OBSERVATION_ONLY,
        "POST",
        "/v1/stream",
    )),
    crud(e(
        "EventStream",
        Crud::Update,
        reason::STREAM_IS_OBSERVATION_ONLY,
        "PUT",
        "/v1/stream",
    )),
    crud(e(
        "EventStream",
        Crud::Delete,
        reason::STREAM_IS_OBSERVATION_ONLY,
        "DELETE",
        "/v1/stream",
    )),
    // OfflineVerification: the 22nd capability, a wasm/SDK pure function with no HTTP resource at all
    // — every one of its four cells is empty on this surface.
    crud(e(
        "OfflineVerification",
        Crud::Create,
        reason::LOCAL_PURE_FUNCTION_NO_RESOURCE,
        "POST",
        "/v1/verify-receipt-offline",
    )),
    crud(e(
        "OfflineVerification",
        Crud::Read,
        reason::LOCAL_PURE_FUNCTION_NO_RESOURCE,
        "GET",
        "/v1/verify-receipt-offline",
    )),
    crud(e(
        "OfflineVerification",
        Crud::Update,
        reason::LOCAL_PURE_FUNCTION_NO_RESOURCE,
        "PUT",
        "/v1/verify-receipt-offline",
    )),
    crud(e(
        "OfflineVerification",
        Crud::Delete,
        reason::LOCAL_PURE_FUNCTION_NO_RESOURCE,
        "DELETE",
        "/v1/verify-receipt-offline",
    )),
    // Sub-route emptiness (not one of the 44 CRUD cells): an escalation has no by-id read route.
    sub(e(
        "Escalation",
        Crud::Read,
        reason::NO_ROUTE_FOR_ESCALATION_BY_ID,
        "GET",
        "/v1/escalations/x",
    )),
];

// ---------------------------------------------------------------------------
// The probe.
// ---------------------------------------------------------------------------

/// Fill an endpoint template's path parameters with concrete values and prefix the base path, so a
/// declared `"POST /candidates/{id}/verify"` becomes `"/v1/candidates/x/verify"`.
fn probe_path(endpoint: &str) -> (String, String) {
    let (method, path) = endpoint.split_once(' ').expect("METHOD /path");
    let filled = path
        .replace("{id}", "x")
        .replace("{tid}", "x")
        .replace("{window_end}", "1");
    (method.to_string(), format!("{}{filled}", gx_api::BASE_PATH))
}

/// Drive one request through a router (with an optional bearer token) and classify the answer.
///
/// The same function is used against the shipped router (with `TOKEN`) and against the doctored
/// router of the negative control (no token, no auth layer) — one discriminator, two subjects, which
/// is what makes the negative control a control.
async fn classify(
    router: &axum::Router,
    token: Option<&str>,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> Routing {
    use tower::ServiceExt;

    let mut builder = axum::http::Request::builder().method(method).uri(path);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let request = match body {
        Some(value) => builder
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::to_vec(&value).expect("serialises"),
            ))
            .expect("a request"),
        None => builder.body(axum::body::Body::empty()).expect("a request"),
    };
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("the router answers");
    let status = response.status().as_u16();

    // 🔴 The body is read **only** for a `404`, where `detail` tells the router fallback apart from a
    // handler's own NOT_FOUND. Everything else classifies on status alone — which is not just an
    // optimisation: `GET /v1/stream` answers `200` with an NDJSON body that never ends, and draining
    // it (as `router.rs` notes for `Client::open`) would hang forever. `405` and every routed
    // status are decided before the body is touched.
    if status == 405 {
        return Routing::MethodNotAllowed;
    }
    if status != 404 {
        return Routing::Routed;
    }
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("a readable body");
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    let detail = json.get("detail").and_then(|d| d.as_str()).unwrap_or("");
    if detail.contains("is not a route of this server") {
        Routing::Unrouted
    } else {
        Routing::Routed
    }
}

/// The absence assertion, factored out so the **same** predicate the real suite runs is the one the
/// negative control feeds a doctored input to. Panics (turns the suite red) when a cell `req/485`
/// declares empty is in fact routed.
fn assert_cell_empty(routing: Routing, resource: &str, verb: Crud, reason: &str) {
    assert!(
        !routing.is_routed(),
        "{resource}/{} is declared structurally empty ({reason}) but the router answered it as \
         {routing:?} — a route was added to a cell req/485 says has none, which must turn P2 red",
        verb.tag()
    );
}

// ---------------------------------------------------------------------------
// The suite.
// ---------------------------------------------------------------------------

/// 🔴 The denominator gate (no server): the matrix is exactly the 44-cell surface `req/485` declares,
/// the occupied rows are exactly the crate's own declared endpoints, and exactly the eleven reason
/// constants appear. A census over a shrunk denominator is not a census (`req/176` ruling 2), so this
/// runs first and needs no router.
#[test]
fn the_matrix_is_the_declared_surface_and_the_eleven_reasons() {
    use std::collections::BTreeSet;

    // Distinct occupied (resource, verb) cells.
    let occupied_cells: BTreeSet<(&str, &str)> = OCCUPIED
        .iter()
        .map(|o| (o.resource, o.verb.tag()))
        .collect();
    // Empty CRUD cells (the sub-route row excluded).
    let empty_cells: BTreeSet<(&str, &str)> = EMPTY
        .iter()
        .filter(|r| !r.sub_route)
        .map(|r| (r.cell.resource, r.cell.verb.tag()))
        .collect();

    println!(
        "P2_MATRIX occupied_cells={} empty_cells={} total={}",
        occupied_cells.len(),
        empty_cells.len(),
        occupied_cells.len() + empty_cells.len()
    );

    // Occupied and empty cells are disjoint and cover 44 cells with no overlap.
    for cell in &occupied_cells {
        assert!(
            !empty_cells.contains(cell),
            "{cell:?} is listed both occupied and empty"
        );
    }
    assert_eq!(occupied_cells.len(), 16, "16 distinct occupied CRUD cells");
    assert_eq!(empty_cells.len(), 28, "28 structurally-empty CRUD cells");
    assert_eq!(
        occupied_cells.len() + empty_cells.len(),
        44,
        "eleven resources × C/R/U/D = 44 cells, each classified exactly once"
    );

    // Every resource contributes exactly four cells (occupied ∪ empty), so no resource silently
    // dropped a verb.
    let mut per_resource: std::collections::BTreeMap<&str, usize> =
        std::collections::BTreeMap::new();
    for (resource, _) in occupied_cells.iter().chain(empty_cells.iter()) {
        *per_resource.entry(resource).or_default() += 1;
    }
    println!("P2_PER_RESOURCE {per_resource:?}");
    assert_eq!(per_resource.len(), 11, "eleven resources");
    for (resource, count) in &per_resource {
        assert_eq!(
            *count, 4,
            "{resource} must have four CRUD cells, has {count}"
        );
    }

    // The occupied endpoints are exactly the crate's own declared HTTP surface — not a hand-typed
    // list this test could drift from. 21 declared routes; the escalation route occupies two cells,
    // so the set of *distinct* endpoints is 21.
    let declared: BTreeSet<&str> = gx_api::SPECIFIED_ENDPOINTS
        .iter()
        .copied()
        .chain(gx_api::list::EXTENSION_ENDPOINTS.iter().copied())
        .chain(
            gx_api::verdict_checkpoints::VERDICT_CHECKPOINT_ENDPOINTS
                .iter()
                .copied(),
        )
        .collect();
    let matrix_endpoints: BTreeSet<&str> = OCCUPIED.iter().map(|o| o.endpoint).collect();
    println!(
        "P2_ENDPOINTS declared={} matrix={}",
        declared.len(),
        matrix_endpoints.len()
    );
    assert_eq!(
        declared.len(),
        21,
        "SPECIFIED(14)+EXTENSION(4)+VERDICT_CHECKPOINT(3) = 21 declared routes"
    );
    assert_eq!(
        matrix_endpoints, declared,
        "P2's occupied cells must name exactly the crate's declared endpoints — a route added or \
         removed in lib.rs must move this matrix in the same commit"
    );

    // Exactly the eleven reason constants appear across the empty rows (CRUD + sub-route).
    let reasons_used: BTreeSet<&str> = EMPTY.iter().map(|r| r.cell.reason).collect();
    let reasons_declared: BTreeSet<&str> = reason::ALL.iter().copied().collect();
    println!(
        "P2_REASONS used={} declared={}",
        reasons_used.len(),
        reason::ALL.len()
    );
    assert_eq!(reason::ALL.len(), 11, "eleven reason constants");
    assert_eq!(
        reasons_used, reasons_declared,
        "every empty cell's reason is one of the eleven declared constants, and every declared \
         constant is used by at least one cell"
    );
}

/// 🔴 P2 AC (a): every occupied cell's declared route answers over the shipped router.
///
/// The probe asserts routed-vs-not only: a bogus id (`x`) makes each handler refuse without a valid
/// object, so no probe mutates the fixture and none needs a pipeline. If a route were removed from
/// `lib.rs`, its probe would fall to `405` (path still known for another method) or `Unrouted` (path
/// gone), and this turns red — the positive half of the matrix.
#[tokio::test]
async fn every_occupied_crud_cell_is_reachable_over_the_real_router() {
    let server = Server::new("crud22_occupied", "before\n");
    let router = gx_api::router(server.state.clone());

    for occ in OCCUPIED {
        let (method, path) = probe_path(occ.endpoint);
        let body = if method == "GET" {
            None
        } else {
            Some(serde_json::json!({}))
        };
        let routing = classify(&router, Some(TOKEN), &method, &path, body).await;
        println!(
            "OCCUPIED {}/{} {method} {path} -> {routing:?}",
            occ.resource,
            occ.verb.tag()
        );
        assert!(
            routing.is_routed(),
            "{}/{} is an occupied cell ({}) but the router did not route {method} {path}: \
             {routing:?}",
            occ.resource,
            occ.verb.tag(),
            occ.endpoint
        );
    }
}

/// 🔴 P2 AC (b): every structurally-empty cell has no route, and the emptiness carries its reason.
///
/// This is the half no existing suite states. Each row's canonical HTTP verb is probed and must
/// answer *not routed* — `405` for a wrong method on a known resource path, `Unrouted` for a path
/// that does not exist (`POST /receipts`, `GET /escalations/{id}`, the offline-verify pseudo-path).
#[tokio::test]
async fn every_structurally_empty_cell_has_no_route_for_its_stated_reason() {
    let server = Server::new("crud22_empty", "before\n");
    let router = gx_api::router(server.state.clone());

    for row in EMPTY {
        let cell = &row.cell;
        let body = if cell.method == "GET" {
            None
        } else {
            Some(serde_json::json!({}))
        };
        let routing = classify(&router, Some(TOKEN), cell.method, cell.probe, body).await;
        println!(
            "EMPTY {}/{} {} {} -> {routing:?} reason={} sub_route={}",
            cell.resource,
            cell.verb.tag(),
            cell.method,
            cell.probe,
            cell.reason,
            row.sub_route
        );
        assert_cell_empty(routing, cell.resource, cell.verb, cell.reason);
    }
}

/// 🔴 P2 negative control (`req/506` §2 P2): a dummy route in a cell `req/485` calls empty turns the
/// emptiness check red — the machine really detects presence, it does not merely always pass.
///
/// A router built **inside this test process** carries a `DELETE /v1/transformations/{id}` handler
/// (the AC's own example). The same discriminator that measures the shipped router measures this one,
/// and the same [`assert_cell_empty`] the real suite runs is fed the doctored result and must panic.
/// No shipped code is edited; the injection is a router this function owns and drops.
#[tokio::test]
async fn negative_control_a_present_delete_route_turns_the_emptiness_check_red() {
    async fn ok() -> &'static str {
        "ok"
    }

    // The doctored surface: the real Transformation item path, plus the DELETE `req/485` says is
    // absent because the ledger is append-only.
    let doctored = axum::Router::new().nest(
        gx_api::BASE_PATH,
        axum::Router::new().route("/transformations/{id}", axum::routing::get(ok).delete(ok)),
    );

    // On the doctored router the "absent" verb is now routed …
    let doctored_routing = classify(&doctored, None, "DELETE", "/v1/transformations/x", None).await;
    println!("NEGCTRL doctored DELETE /v1/transformations/x -> {doctored_routing:?}");
    assert_eq!(
        doctored_routing,
        Routing::Routed,
        "the injected DELETE route must be reachable, or the control proves nothing"
    );

    // … and feeding that result to the real emptiness predicate must panic (detection power). The
    // panic here is expected and caught, so its hook output is silenced — otherwise a green run
    // prints a "panicked at" line that reads like a failure to a person or a log-scraping gate.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let fired = std::panic::catch_unwind(|| {
        assert_cell_empty(
            doctored_routing,
            "Transformation",
            Crud::Delete,
            reason::LEDGER_IS_APPEND_ONLY,
        );
    })
    .is_err();
    std::panic::set_hook(previous_hook);
    println!("NEGCTRL assertion_fired={fired}");
    assert!(
        fired,
        "assert_cell_empty must reject a Routed answer — otherwise the P2 emptiness check is a \
         green-only decoration with no detection power"
    );

    // The control's other side: on the *shipped* router the same cell is genuinely empty, so the
    // check that just fired on the doctored input passes on the real one.
    let server = Server::new("crud22_negctrl_real", "before\n");
    let real = gx_api::router(server.state.clone());
    let real_routing = classify(&real, Some(TOKEN), "DELETE", "/v1/transformations/x", None).await;
    println!("NEGCTRL real DELETE /v1/transformations/x -> {real_routing:?}");
    assert!(
        !real_routing.is_routed(),
        "on the shipped router Transformation/D is append-only-empty: {real_routing:?}"
    );
}
