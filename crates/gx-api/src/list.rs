//! 🔴 **M6-05 採(a)** — the list endpoints 44 does not have, in the shape 44 §2.7 reserved.
//!
//! # What is being added, and why it is an addition rather than a fix
//!
//! 44 §2.7 is explicit: 「本書指定のエンドポイント群には一覧（list）系エンドポイントは含まれない」, and
//! then reserves the regulation for the day somebody adds them —
//! `?limit=<int, 既定50, 最大200>&cursor=<opaque string>`, answering `{items, next_cursor}`. §47
//! adopted M6-05 (a): the three are implemented in M6 as **44 extensions**, which 44 §2.6 permits
//! (「後方互換な追加（…**新規エンドポイント**…）は`/v1`内で許容する」).
//!
//! 48's console needs them and so does an operator with a terminal: today the only way to find a
//! transformation is to already know its id, and the only way to find an **escalation** is to have
//! been watching when the ticket was created. That last one is not a convenience — 43 T-4c creates a
//! ticket and 44 §1.2's `gx escalation approve <TICKET_ID>` consumes one, with nothing in between
//! that lists them.
//!
//! # 🔴 The three, and a fourth that req/88 names in the same ruling
//!
//! The hand-6 brief names 「candidates/escalations/transformations 相当」. req/88 §2.2's row for
//! M6-05 gives the engine sources as 「`transformation_ids()` / `sigma()` / `prove_consistency`」 and
//! its §4 entry singles out 「特に `GET /ledger/consistency?from=&to=` は **CLI に `gx log
//! consistency` が既在**なので、HTTP だけが欠けている非対称である」. The two readings name four
//! endpoints between them and three in each, so this hand implements the **union** and raises the
//! discrepancy (**M6H6-5**) rather than choosing a reading and calling it the ruling:
//!
//! | endpoint | source | why |
//! |---|---|---|
//! | `GET /candidates` | `transformation_ids()` + `state()` | 43 §0's 広義 Candidate — every row that has not reached a terminal state. 「what is in flight」 |
//! | `GET /escalations` | `ticket()` | the queue 43 T-4c fills and `POST /candidates/{id}/escalation` empties |
//! | `GET /transformations` | `transformation_ids()` | the audit list, terminal rows included |
//! | `GET /ledger/consistency` | `prove_consistency` | not a list (no cursor); the CLI/HTTP asymmetry M6-05 names |
//!
//! # 🔴 The order is the journal's, and that is a ruling rather than a taste
//!
//! req/88 M6-05: 「cursor 設計が `Σ` に順序を要求する…engine の表は `BTreeMap<TransformationId, _>`=
//! **TID 順(=CID 順=実質ランダム)であって時間順ではない**」, and §47 fixed it — 「**cursor=journal 順**
//! (M6-13 と共有・表順=CID 順は時間について任意)」. So these endpoints sort by the position of a
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

/// 44 §2.7: 「`?limit=<int, 既定50, 最大200>`」.
pub const DEFAULT_LIMIT: usize = 50;
/// 44 §2.7's ceiling.
pub const MAX_LIMIT: usize = 200;

/// 44 §2.7's query, for all three lists.
#[derive(Debug, Default, Deserialize)]
pub struct Page {
    /// 「既定50, 最大200」.
    pub limit: Option<usize>,
    /// 「`<opaque string>`」 — a [`Cursor`] this server published.
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

/// The four fields 44 §2.2's `GET /candidates/{id}` answers with, for a row in a list.
fn row_json(engine: &Engine<RequestEvidence>, id: &TransformationId) -> serde_json::Value {
    serde_json::json!({
        "transformation": id.0.to_text(),
        "state": engine.state(id),
        "verdict": engine.verdict(id),
        "enforced": engine.enforced(id),
    })
}

/// 🔴 `GET /candidates` (**M6-05**, a 44 extension) — the rows that have not reached a terminal state.
///
/// 43 §0 defines the broad sense this endpoint's name is in: 「`Draft, Candidate, Verifying,
/// Admitted, Denied, Escalated, Canonicalized` はすべて広義Candidate（未承認）に属し、`Committed`
/// のみが…solid」. `Denied` is terminal in 43 §1 and is nevertheless **included**, because under
/// `RecordOnly` it is a road (T-8r) and because an operator asking 「what is waiting for me」 needs
/// to see a refusal they have not dealt with.
///
/// # Errors
/// [`ApiError`] `VALIDATION_ERROR` for 44 §2.7's limit or cursor.
pub async fn candidates(State(state): State<AppState>, Params(page): Params<Page>) -> Answer {
    let (limit, after) = (page.limit()?, page.after()?);
    let engine = state.engine();
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
/// find one. 43 T-4c calls the ticket's creation 「人間へ通知」 and v0.1 has no notifier, so this list
/// **is** the notification.
///
/// # Errors
/// [`ApiError`] `VALIDATION_ERROR` for 44 §2.7's limit or cursor.
pub async fn escalations(State(state): State<AppState>, Params(page): Params<Page>) -> Answer {
    let (limit, after) = (page.limit()?, page.after()?);
    let engine = state.engine();
    let rows = journal_order(&engine)
        .into_iter()
        .filter_map(|(cursor, id)| {
            let ticket = engine.ticket(&id)?;
            Some((
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
            ))
        })
        .collect();
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
    let engine = state.engine();
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
                // 🔴 **M6H6-15** (§53, 検収者起票) — ASM-48-3's second half: 42 §3.12's
                // `EscrowedInverse.status`, exposed on the row.
                //
                // `null` for a transformation with **no escrow row at all**, and that is the whole
                // care this line takes: `InverseStatus::Unavailable` means 「`invert()` answered
                // `None`」 (42 §3.12 verbatim), so writing it for a candidate that never reached
                // T-10b would answer a question nobody asked — the skip/pass conflation req/29 §4
                // forbids, one field over.
                //
                // Why on the **list** and not only on `GET /transformations/{id}`: DR-1(a)'s wedge is
                // verified undo, and 「which of these can I still undo」 is a question about a set.
                map.insert(
                    "inverse_status".into(),
                    engine
                        .inverse_status(&id)
                        .and_then(|status| serde_json::to_value(status).ok())
                        .unwrap_or(serde_json::Value::Null),
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
/// `GET /ledger/proof`. req/88 M6-05 calls the gap 「HTTP だけが欠けている非対称」, and the value it
/// withholds is the one a third party needs most: an inclusion proof says a receipt is in *a* tree,
/// and a consistency proof says the tree it is in is an extension of the tree they saw last time.
///
/// Not a list, so no cursor and no `limit` — the shape is `GET /ledger/proof`'s.
///
/// # Errors
/// [`ApiError`] `VALIDATION_ERROR` for sizes gx-log refuses (`from > to`, or a size past the tree).
pub async fn ledger_consistency(
    State(state): State<AppState>,
    Params(query): Params<Consistency>,
) -> Answer {
    let engine = state.engine();
    let proof = gx_log::proof::prove_consistency(engine.ledger().log(), query.from, query.to)
        .map_err(|e| ApiError::from_log(&e))?;
    ok(serde_json::json!({
        "from": query.from,
        "to": query.to,
        "proof": proof,
    }))
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
/// Used by the suite rather than by the handlers: the assertion 「a row's list position is its
/// `Planned` record」 is what makes 「the cursor is stable」 mean something.
#[must_use]
pub fn is_first_mention(record: &EngineJournalRecord) -> bool {
    matches!(record, EngineJournalRecord::Planned { .. })
}
