// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **M6-05, adopted (a)** (sem: SEM-gx-api-243) — the list endpoints 44 does not have, in the shape 44 §2.7 reserved.
//!
//! # What is being added, and why it is an addition rather than a fix
//!
//! 44 §2.7 is explicit: "the endpoints this document specifies include no list-family endpoint" (sem: SEM-gx-api-244), and
//! then reserves the regulation for the day somebody adds them —
//! `?limit=<int, default 50, maximum 200>&cursor=<opaque string>` (sem: SEM-gx-api-245), answering `{items, next_cursor}`. §47
//! adopted M6-05 (a): the three are implemented in M6 as **44 extensions**, which 44 §2.6 permits
//! ("a backward-compatible addition (… **a new endpoint** …) is allowed within `/v1`"; sem: SEM-gx-api-246).
//!
//! 48's console needs them and so does an operator with a terminal: today the only way to find a
//! transformation is to already know its id, and the only way to find an **escalation** is to have
//! been watching when the ticket was created. That last one is not a convenience — 43 T-4c creates a
//! ticket and 44 §1.2's `gx escalation approve <TICKET_ID>` consumes one, with nothing in between
//! that lists them.
//!
//! # 🔴 The three, and a fourth that req/88 names in the same ruling
//!
//! The hand-6 brief names "the equivalents of candidates/escalations/transformations" (sem: SEM-gx-api-247). req/88 §2.2's row for
//! M6-05 gives the engine sources as "`transformation_ids()` / `sigma()` / `prove_consistency`" and
//! its §4 entry singles out "in particular, `GET /ledger/consistency?from=&to=` **already exists in the CLI as `gx log
//! consistency`**, so it is an asymmetry where only HTTP is missing". The two readings name four
//! endpoints between them and three in each, so this hand implements the **union** and raises the
//! discrepancy (**M6H6-5**) rather than choosing a reading and calling it the ruling:
//!
//! | endpoint | source | why |
//! |---|---|---|
//! | `GET /candidates` | `transformation_ids()` + `state()` | 43 §0's broad-sense Candidate (sem: SEM-gx-api-248) — every row that has not reached a terminal state. "what is in flight" |
//! | `GET /escalations` | `ticket()` | the queue 43 T-4c fills and `POST /candidates/{id}/escalation` empties |
//! | `GET /transformations` | `transformation_ids()` | the audit list, terminal rows included |
//! | `GET /ledger/consistency` | `prove_consistency` | not a list (no cursor); the CLI/HTTP asymmetry M6-05 names |
//!
//! # 🔴 The order is the journal's, and that is a ruling rather than a taste
//!
//! req/88 M6-05: "the cursor design requires an ordering on `Σ` … the engine's table, `BTreeMap<TransformationId, _>`, is
//! **TID order (= CID order = effectively random), not chronological order**" (sem: SEM-gx-api-249), and §47 fixed it — "**cursor = journal order**
//! (shared with M6-13; table order = CID order is arbitrary with respect to time)". So these endpoints sort by the position of a
//! transformation's **first** journal record and page on [`crate::stream::Cursor`], the same
//! coordinate `GET /stream` resumes from and `gx replay --from` counts in (E-M6-6). One coordinate,
//! three consumers: a client can list a page, note `next_cursor`, and hand that same string to
//! `/stream` to watch what happens after it.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use gx_core::TransformationId;
use gx_engine::store::EngineJournalRecord;
use gx_engine::{Engine, Lifecycle};
use serde::Deserialize;

use crate::extract::Params;
use crate::problem::ApiError;
use crate::state::{AppState, RequestEvidence};
use crate::stream::Cursor;

/// 44 §2.7: "`?limit=<int, default 50, maximum 200>`" (sem: SEM-gx-api-250).
pub const DEFAULT_LIMIT: usize = 50;
/// 44 §2.7's ceiling.
pub const MAX_LIMIT: usize = 200;

/// 44 §2.7's query, for all three lists.
#[derive(Debug, Default, Deserialize)]
pub struct Page {
    /// "default 50, maximum 200" (sem: SEM-gx-api-251).
    pub limit: Option<usize>,
    /// "`<opaque string>`" (sem: SEM-gx-api-252) — a [`Cursor`] this server published.
    pub cursor: Option<String>,
}

impl Page {
    /// The limit, refused rather than clamped when it is out of range.
    ///
    /// 🔴 Clamping would answer a request for 1000 items with 200 and no indication that the number
    /// changed, so a client paging by 1000 would silently see one fifth of the ledger and believe it
    /// had seen all of it. 44 §2.7 states a maximum; a maximum a caller can exceed without being told
    /// is a default.
    ///
    /// # Errors
    /// [`ApiError`] `VALIDATION_ERROR` for `0` or for more than [`MAX_LIMIT`].
    pub fn limit(&self) -> Result<usize, ApiError> {
        match self.limit {
            None => Ok(DEFAULT_LIMIT),
            Some(n) if (1..=MAX_LIMIT).contains(&n) => Ok(n),
            Some(n) => Err(ApiError::validation(format!(
                "`limit={n}` is outside 44 §2.7's range (1..={MAX_LIMIT}, default {DEFAULT_LIMIT}). \
                 A clamped limit would be a page that looks complete and is not"
            ))),
        }
    }

    /// The cursor, or the beginning.
    ///
    /// # Errors
    /// [`ApiError`] `VALIDATION_ERROR` for a cursor this server did not publish.
    pub fn after(&self) -> Result<Option<Cursor>, ApiError> {
        self.cursor.as_deref().map(Cursor::parse).transpose()
    }
}

/// 🔴 Every transformation the engine holds, in the order the journal first mentioned each.
///
/// The position is the **first** record naming a transformation — `Planned`, for every row that has
/// one — so a transformation's place in the list does not move when it is verified or committed. A
/// list ordered by the *latest* record would reshuffle under a reader who was paging through it, and
/// 44 §2.7's cursor contract assumes it does not.
fn journal_order(engine: &Engine<RequestEvidence>) -> Vec<(Cursor, TransformationId)> {
    let mut seen: Vec<(Cursor, TransformationId)> = Vec::new();
    for (index, record) in engine.journal().records().iter().enumerate() {
        let Some(id) = record.transformation() else {
            // `DraftCreated` alone (E-M5-3), and 44 §0 keeps the Draft off this surface anyway.
            continue;
        };
        if seen.iter().any(|(_, known)| *known == id) {
            continue;
        }
        seen.push((
            Cursor {
                record: index,
                ordinal: 0,
            },
            id,
        ));
    }
    seen
}

/// 44 §2.7's envelope for a page of rows.
fn page_of(
    rows: Vec<(Cursor, serde_json::Value)>,
    after: Option<Cursor>,
    limit: usize,
) -> serde_json::Value {
    let mut items = Vec::new();
    let mut next = None;
    for (cursor, row) in rows {
        if let Some(after) = after {
            if cursor <= after {
                continue;
            }
        }
        if items.len() == limit {
            // There is at least one more, so the cursor is the last one **included**.
            break;
        }
        next = Some(cursor);
        items.push(row);
    }
    serde_json::json!({
        "items": items,
        // 🔴 `null` when the page was not full: 44 §2.7 gives `next_cursor` the type
        // `string|null`, and a non-null cursor on a short page would make a client ask again for
        // an answer that is always empty. A client that wants to keep watching hands the last
        // cursor to `GET /stream` instead, which is what sharing the coordinate is for.
        "next_cursor": next.filter(|_| items.len() == limit).map(Cursor::to_text),
    })
}

/// The four fields 44 §2.2's `GET /candidates/{id}` answers with, for a row in a list — plus, since
/// v0.4-l, the three a reader of a **list** needs and could only get by one `GET` per row.
///
/// 🔴 **M-15** (`req/182` §1-2, `req/189`; 44 §2.7 v0.4-l addendum): `created_at` / `actor` / `scope`
/// are additive (§2.6 "new optional field") and each is the engine's own answer read off the
/// row — `Transformation.created_at`, `Transformation.actor`, `Fingerprint.scope` (the locator
/// half of 42 §3.5, ASM-69-1). **`null` when the row is not on the table**, which after a
/// restart is every row (M5H3-5): the three fields say "this process holds the body" or "it does
/// not", and inventing a value from Σ (which carries none of the three, `replay.rs`) would be the
/// skip/pass conflation req/29 §4 forbids. `time / who / target` for a GUI's list, without the
/// N+1 the audit measured (`req/182` §3-2 #7).
/// 🔴 **DR-44-9 / the name trap** (`req/38` §168 ruling 1 ⑤, `req/187` §5): the "target" a person
/// reads on a row is **`scope`**, and it is `Fingerprint.scope`. `gx_core::Transformation` also has
/// a field spelled `target` — `Option<Cid>`, the **expected post-state digest** — and it is not
/// this column and must never be wired to it. `target -> target` is the most natural line anybody
/// would write here, it type-checks, it looks entirely correct, and it puts a content hash in the
/// first column a human reads. The GUI session found the trap by reading this crate rather than a
/// note about it, declined to connect the two, and made its own check refuse the rename **by
/// name** — "refusing something that looks right takes naming it". The same refusal belongs on
/// this side of the wire, which is what this paragraph is.
///
/// A graph face draws its lanes on `scope` for the same reason: a projection whose axis is a
/// digest has one row per lane and says nothing.
fn row_json(engine: &Engine<RequestEvidence>, id: &TransformationId) -> serde_json::Value {
    let transformation = engine.transformation(id);
    serde_json::json!({
        "transformation": id.0.to_text(),
        "state": engine.state(id),
        "verdict": engine.verdict(id),
        "enforced": engine.enforced(id),
        "created_at": transformation.map_or(serde_json::Value::Null, |t| {
            crate::rfc3339::of(t.created_at).into()
        }),
        "actor": transformation
            .and_then(|t| serde_json::to_value(&t.actor).ok())
            .unwrap_or(serde_json::Value::Null),
        "scope": engine
            .precondition_fingerprint(id)
            .map_or(serde_json::Value::Null, |f| f.scope().into()),
    })
}

/// 🔴 `GET /candidates` (**M6-05**, a 44 extension) — the rows that have not reached a terminal state.
///
/// 43 §0 defines the broad sense this endpoint's name is in: "`Draft, Candidate, Verifying,
/// Admitted, Denied, Escalated, Canonicalized` all belong to the broad-sense Candidate (unapproved), and only `Committed`
/// is … solid" (sem: SEM-gx-api-253). `Denied` is terminal in 43 §1 and is nevertheless **included**, because under
/// `RecordOnly` it is a road (T-8r) and because an operator asking "what is waiting for me" needs
/// to see a refusal they have not dealt with.
///
/// # Errors
/// [`ApiError`] `VALIDATION_ERROR` for 44 §2.7's limit or cursor.
pub async fn candidates(State(state): State<AppState>, Params(page): Params<Page>) -> Answer {
    let (limit, after) = (page.limit()?, page.after()?);
    // 🔴 **DR-43-6 / `req/215` H-05** — read to the end of the log first, without the project
    // lock. Before this, a list answered from whatever this process last read: `req/215` measured
    // one row against six leaves on disk, unchanged after three seconds, until the server itself
    // wrote something.
    let engine = state.engine_refreshed()?;
    let rows = journal_order(&engine)
        .into_iter()
        .filter(|(_, id)| {
            !matches!(
                engine.state(id),
                Some(Lifecycle::Committed)
                    | Some(Lifecycle::Aborted(_))
                    | Some(Lifecycle::Superseded)
                    | None
            )
        })
        .map(|(cursor, id)| (cursor, row_json(&engine, &id)))
        .collect();
    ok(page_of(rows, after, limit))
}

/// 🔴 `GET /escalations` (**M6-05**, a 44 extension) — the tickets waiting for a person.
///
/// The gap this closes is 44's own: `gx escalation approve <TICKET_ID>` and
/// `POST /candidates/{id}/escalation` both consume a ticket, and until now nothing produced a way to
/// find one. 43 T-4c calls the ticket's creation "notify a human" (sem: SEM-gx-api-254) and v0.1 has no notifier, so this list
/// **is** the notification.
///
/// # Errors
/// [`ApiError`] `VALIDATION_ERROR` for 44 §2.7's limit or cursor.
pub async fn escalations(State(state): State<AppState>, Params(page): Params<Page>) -> Answer {
    let (limit, after) = (page.limit()?, page.after()?);
    // 🔴 **DR-43-6 / `req/215` H-05** — read to the end of the log first, without the project
    // lock. Before this, a list answered from whatever this process last read: `req/215` measured
    // one row against six leaves on disk, unchanged after three seconds, until the server itself
    // wrote something.
    let engine = state.engine_refreshed()?;
    let mut rows = Vec::new();
    for (cursor, id) in journal_order(&engine) {
        // 🔴 **H-04** (`req/182` §1-1, `req/189`): the queue is "rows whose **state** is
        // `Escalated`", and the ticket is read off such a row — not the other way round.
        // Until v0.4-l this filtered on `ticket()` alone, and the ticket had one writer (T-4c)
        // and no eraser, so a ruled, cancelled or expired row stayed in this list for ever
        // (a GUI's "waiting for you" badge only ever went up). The engine now clears the ticket
        // when the row leaves `Escalated`; the state filter here is the same invariant read
        // from the outside, so the two cannot disagree silently.
        // `matches!` rather than `!=`: gx-canon's authority_boundary Rule 1(iii) scanner reads a
        // `!= Some(Lifecycle::…)` comparison as a second state table (req/38 §154; RED left by ca4ee27).
        if !matches!(engine.state(&id), Some(Lifecycle::Escalated)) {
            continue;
        }
        // 🔴 **R19 / `req/279` M-05** — from Σ, not from the live table.
        //
        // `Engine::ticket` answers out of `self.table`, and T-4c's ticket is **not journalled**
        // (`Verdict{kind: Escalate}` is). So an escalation raised by a `gx wrap` process ceased to
        // exist for this list the instant that process ended: the audit started a `gx serve`
        // afterwards and got `{"items":[],"next_cursor":null}` from here while `/v1/candidates` and
        // `/v1/transformations` both returned the same row with `"state": "Escalated"`. 43 T-4c
        // calls the ticket's creation "notify a human" and this list **is** the notification, so
        // the one surface a person looks at was the one surface that could not see the queue.
        //
        // `Engine::ticket_as_raised` is the engine's own answer to the other question — "what did
        // the `Verdict{Escalate}` record at this position name?" — live ticket first, M6H3-10's
        // rebuild from Σ otherwise. `GET /stream` has read it since v0.4-l (`stream.rs`), and rule
        // 1 keeps the construction there rather than here.
        //
        // `req/182` H-04 fixed this list's badge never going **down**. This is the same list broken
        // the other way, and the two repairs meet in one predicate: the state says who is in the
        // queue, the engine says what their ticket is.
        let ticket = match engine.ticket_as_raised(&id) {
            Ok(Some(ticket)) => ticket,
            // A row that is `Escalated` and whose ticket the engine will not rebuild is an
            // inconsistency, not an empty queue entry — and it is reported as the engine's own
            // error rather than dropped, because a row silently missing from this list is exactly
            // the defect being repaired.
            Ok(None) => continue,
            Err(e) => return Err(ApiError::from_engine(&e)),
        };
        rows.push((
            cursor,
            serde_json::json!({
                "transformation": id.0.to_text(),
                "ticket_id": ticket.id.0.to_text(),
                "state": engine.state(&id),
                "reasons": ticket.reasons,
                "required_approval": ticket.required_approval,
                "created_at": crate::rfc3339::of(ticket.created_at),
                "deadline": crate::rfc3339::maybe(engine.deadline(&id)),
            }),
        ));
    }
    ok(page_of(rows, after, limit))
}

/// 🔴 `GET /transformations` (**M6-05**, a 44 extension) — everything, terminal rows included.
///
/// The audit list, and the one an auditor reconciling a ledger against a journal reads. Ordered by
/// first mention, so a page taken today names the same rows in the same order tomorrow.
///
/// # Errors
/// [`ApiError`] `VALIDATION_ERROR` for 44 §2.7's limit or cursor.
pub async fn transformations(State(state): State<AppState>, Params(page): Params<Page>) -> Answer {
    let (limit, after) = (page.limit()?, page.after()?);
    // 🔴 **DR-43-6 / `req/215` H-05** — read to the end of the log first, without the project
    // lock. Before this, a list answered from whatever this process last read: `req/215` measured
    // one row against six leaves on disk, unchanged after three seconds, until the server itself
    // wrote something.
    let engine = state.engine_refreshed()?;
    let rows = journal_order(&engine)
        .into_iter()
        .map(|(cursor, id)| {
            let mut row = row_json(&engine, &id);
            if let Some(map) = row.as_object_mut() {
                map.insert(
                    "superseded_by".into(),
                    engine
                        .superseded_by(&id)
                        .map_or(serde_json::Value::Null, |by| by.0.to_text().into()),
                );
                // 🔴 **M6H6-15** (§53, filed by the acceptance reviewer; sem: SEM-gx-api-255) — ASM-48-3's second half: 42 §3.12's
                // `EscrowedInverse.status`, exposed on the row.
                //
                // `null` for a transformation with **no escrow row at all**, and that is the whole
                // care this line takes: `InverseStatus::Unavailable` means "`invert()` answered
                // `None`" (sem: SEM-gx-api-256) (42 §3.12 verbatim), so writing it for a candidate that never reached
                // T-10b would answer a question nobody asked — the skip/pass conflation req/29 §4
                // forbids, one field over.
                //
                // Why on the **list** and not only on `GET /transformations/{id}`: DR-1(a)'s wedge is
                // verified undo, and "which of these can I still undo" (sem: SEM-gx-api-257) is a question about a set.
                map.insert(
                    "inverse_status".into(),
                    engine
                        .inverse_status(&id)
                        .and_then(|status| serde_json::to_value(status).ok())
                        .unwrap_or(serde_json::Value::Null),
                );
                // 🔴 **R29 / `req/361` §3-1 + H-01** — the third of the three faces this lane
                // widened together, and the reason it is not left for later is the reason
                // `inverse_status` is on the row above: "which of these can I still undo" is a
                // question about a **set**, and so is "which of these aborted with my object left
                // somewhere it should not be". An auditor reconciling a ledger reads this list, not
                // the refusal a caller saw once and discarded.
                //
                // Taking the ruling on `GET /transformations/{id}` and the stream alone would leave
                // the third face behind, which is exactly the denominator failure the twenty-eighth
                // audit filed L-03 as. `null` where no roll-back was in question.
                map.insert(
                    "rollback".into(),
                    engine
                        .rollback(&id)
                        .map_or(serde_json::Value::Null, |r| r.kind().into()),
                );
            }
            (cursor, row)
        })
        .collect();
    ok(page_of(rows, after, limit))
}

/// 🔴 `GET /ledger/consistency?from=&to=` (**M6-05**, a 44 extension) — the CLI's twin.
///
/// `gx log consistency --from --to` has existed since hand 2 and 44 gives the HTTP surface only
/// `GET /ledger/proof`. req/88 M6-05 calls the gap "an asymmetry where only HTTP is missing" (sem: SEM-gx-api-258), and the value it
/// withholds is the one a third party needs most: an inclusion proof says a receipt is in *a* tree,
/// and a consistency proof says the tree it is in is an extension of the tree they saw last time.
///
/// Not a list, so no cursor and no `limit` — the shape is `GET /ledger/proof`'s.
///
/// 🔴 **DR-44-1** (44 §2.7 v0.4-k addendum, `req/181`; sem: SEM-gx-api-259): the sentence above is not what the body below
/// ~~does~~ **did** — `GET /ledger/proof` answers a bare `InclusionProof`, this handler answered
/// `{from, to, proof}` around a bare `ConsistencyProof`, and the CLI twin (`gx log consistency`)
/// and the SDK's `ledgerConsistency()` return type are both the bare proof. The wrapper was pinned
/// as shipped by `tests/wire_census.rs` and 44 §2.7 recorded it as the actual artefact (sem: SEM-gx-api-260) pending the ruling; the
/// old sentence is kept (no-delete) so the discrepancy stays visible from here too.
///
/// 🔴 **DR-44-1 ruled = option (a) bare** (`req/38` §119 ruling 2, implemented in v0.4-l, `req/189`): the
/// body below is now the bare `ConsistencyProof` — 42 §3.11's `{old_size, new_size, path}` — and
/// the first sentence of this doc is true again. `from`/`to` were `old_size`/`new_size` said
/// twice; the CLI twin, the SDK type, `GET /ledger/proof` and this handler now agree, and
/// `sdk/typescript/test/e2e.test.mjs` holds the SDK type against this wire by field census. The
/// wrapper shipped in v0.2.x–v0.4-k as an *unspecified* extension (44 §2.7 said so itself); a
/// client that read `.proof` reads `.old_size` now — declared as the breaking change §119 weighed.
///
/// 🔴 **DR-44-9** (`req/38` §168 ruling 1, `req/187` §5 "the health of the chain has no shape
/// on the wire") — `consistent` / `checked_from` / `checked_to`, and the **stated exception** they are.
///
/// # The rule they are an exception to
///
/// This surface carries proofs and does not grade them. `verdict_checkpoints.rs` writes the reason
/// in full — "a `/verify` endpoint served by the party being audited would be a check marking its
/// own paper" — and `GET /receipts/{tid}` keeps to it (DR-44-9's `receipt_view` carries no
/// `verified`). A window that renders this endpoint kept to it too, and `req/187` §5 measured what
/// that cost: the ledger face raises exactly one alarm, "the chain is broken", off
/// `consistent === false`, and the wire had no such member — so `undefined === false` was `false`
/// and **a genuinely broken chain raised nothing**. Not a wrong answer; an unreachable one, which
/// is quieter and worse.
///
/// # Why the exception is granted here and nowhere else
///
/// The value is one this server has **already computed** in order to answer at all. Every other
/// judgement on this surface would be a new claim about a document somebody else holds; this one is
/// a claim about the server's own log, of exactly the kind the server is the natural author of
/// ("my tree of `to` leaves extends my tree of `from` leaves"). Three limits, stated on the wire's
/// behalf rather than left for a reader to discover:
///
/// 1. **It is not third-party verification.** `consistent: true` means this deployment recomputed
///    both roots from the path it just produced and they matched the roots it holds. A reader who
///    does not trust this deployment runs `gx log consistency` (44 §1.2) or feeds `path` to their
///    own verifier — which is why the proof is still the body and the judgement is a rider, not a
///    replacement (DR-44-1 (a) stands: `old_size`/`new_size`/`path` are untouched at the top level).
/// 2. **A GUI must not spell it `verified` / `refuted`.** Those two words belong to
///    `gx_witness::InclusionCheck` and to a reader's own check. This one is `consistent`, and
///    "the operator's own server says its log grew rather than being rewritten" is the whole of
///    what it licenses a face to say.
/// 3. **`false` is reachable.** The roots are rebuilt from the path (`verify_consistency` walks to
///    both, deliberately, "a check of the new root alone would accept a proof that reached it from
///    some other old tree"), so a stored tree whose leaves and whose cached subtree roots disagree
///    answers `false` here. `tests/dr44_9_views.rs` reaches the arm on a doctored proof rather than
///    declaring it unreachable, because an alarm nobody has ever seen fire is the bug `req/187` §5
///    filed in the first place.
///
/// # Errors
/// [`ApiError`] `VALIDATION_ERROR` for sizes gx-log refuses (`from > to`, or a size past the tree).
pub async fn ledger_consistency(
    State(state): State<AppState>,
    Params(query): Params<Consistency>,
) -> Answer {
    // 🔴 **DR-43-6 / `req/215` H-05** — read to the end of the log first, without the project
    // lock. Before this, a list answered from whatever this process last read: `req/215` measured
    // one row against six leaves on disk, unchanged after three seconds, until the server itself
    // wrote something.
    let engine = state.engine_refreshed()?;
    let log = engine.ledger().log();
    let proof = gx_log::proof::prove_consistency(log, query.from, query.to)
        .map_err(|e| ApiError::from_log(&e))?;
    // 🔴 **R37 / `req/496` M-04** — and the same gate `GET /ledger/checkpoint` has had since
    // `req/215` H-01.
    //
    // The doc above grants this endpoint the one judgement on this surface, and the grant is
    // conditional in its own words: the value "is one this server has **already computed** in order
    // to answer at all", and "a claim about the server's own log, of exactly the kind the server is
    // the natural author of". A server whose journal and ledger describe different trees is not the
    // natural author of anything about *its* log, because there is no fact of the matter about
    // which log is its. Audit 36 measured this endpoint answering `consistent: true`, identical to
    // the healthy reading, on a project the same process was refusing to sign for.
    //
    // `consistent: false` is **not** the answer. Limit 3 above spends a paragraph on `false` being
    // reachable and on what a GUI is licensed to say off it — "the operator's own server says its
    // log grew rather than being rewritten" — and answering it here would say the tree *was*
    // rewritten, which is a different and unmeasured claim. "This server could not state which tree
    // it has" and "the tree is not an extension" are the two sentences the `root_at` arm above
    // already refuses to conflate; this is the same refusal one step out.
    //
    // **The position is load-bearing**, for `ledger_proof`'s reason inverted: the two sizes are the
    // caller's argument, `prove_consistency` is what refuses a bad pair with `422`, and a gate above
    // it would answer `500` for a request that was malformed on its own terms.
    if !engine.ledger_agrees() {
        return Err(crate::ledger_disagrees_refusal(&engine));
    }
    // 🔴 **DR-44-9** — the judgement, beside the proof. See the doc comment above for why
    // this endpoint is the one place on this surface that states one.
    let (Some(old_root), Some(new_root)) =
        (log.root_at(proof.old_size), log.root_at(proof.new_size))
    else {
        // Unreached by construction: `prove_consistency` has already refused `old_size == 0`,
        // `old_size > new_size` and a `new_size` past the tree, which are the three answers
        // `root_at` has `None` for. Declared as an arm rather than folded into `consistent: false`
        // — "this server could not compute a root" and "the tree is not an extension of the older
        // tree" are different sentences, and saying the second when the first happened would be
        // the skip/pass conflation `req/29` §4 forbids, on the field a GUI raises an alarm from.
        return Err(ApiError::internal(format!(
            "the log proved consistency between {} and {} and then had no root for one of those              sizes; the proof is not being graded against a root this server cannot state",
            proof.old_size, proof.new_size
        )));
    };
    let consistent =
        gx_log::proof::verify_consistency(&proof, &old_root, &new_root).map_err(|e| {
            ApiError::internal(format!(
                "this server produced a consistency proof it cannot read back: {e}"
            ))
        })?;
    let mut body = serde_json::to_value(&proof).unwrap_or(serde_json::Value::Null);
    if let Some(map) = body.as_object_mut() {
        map.insert("consistent".to_string(), consistent.into());
        // The subject of the sentence `consistent` states, spelled out. Equal to `old_size` /
        // `new_size` **by construction** — the roots were taken at the proof's own sizes, not at
        // the query's — and `tests/dr44_9_views.rs` pins the identity so that the pair can never
        // become a second source of truth about which trees were compared. DR-44-1 removed a
        // wrapper that hid the proof; these two name what was checked and hide nothing.
        map.insert("checked_from".to_string(), proof.old_size.into());
        map.insert("checked_to".to_string(), proof.new_size.into());
    }
    ok(body)
}

/// `GET /ledger/consistency`'s two sizes.
#[derive(Debug, Deserialize)]
pub struct Consistency {
    /// The tree size the client already trusts.
    pub from: u64,
    /// The tree size it is being asked to accept.
    pub to: u64,
}

/// What every handler in this module returns.
type Answer = Result<Response, ApiError>;

/// A `200` with a JSON body.
fn ok(body: serde_json::Value) -> Answer {
    Ok(axum::response::IntoResponse::into_response((
        StatusCode::OK,
        axum::Json(body),
    )))
}

/// The endpoints this module adds to 44's fourteen, named so that the count is a declaration.
///
/// `crates/gx-api/tests/router.rs` walks these the way it walks [`crate::SPECIFIED_ENDPOINTS`]: an
/// extension that stopped being routed would otherwise be discoverable only by a client.
pub const EXTENSION_ENDPOINTS: [&str; 4] = [
    "GET /candidates",
    "GET /escalations",
    "GET /transformations",
    "GET /ledger/consistency",
];

/// One journal position per transformation, exposed for the suite that compares the coordinate the
/// lists page on with the one `GET /stream` resumes from (M6-13 = M6-05, one coordinate).
#[must_use]
pub fn first_mentions(engine: &Engine<RequestEvidence>) -> Vec<(Cursor, TransformationId)> {
    journal_order(engine)
}

/// Whether a journal record is the one that puts a transformation in the lists.
///
/// Used by the suite rather than by the handlers: the assertion "a row's list position is its
/// `Planned` record" (sem: SEM-gx-api-261) is what makes "the cursor is stable" mean something.
#[must_use]
pub fn is_first_mention(record: &EngineJournalRecord) -> bool {
    matches!(record, EngineJournalRecord::Planned { .. })
}
