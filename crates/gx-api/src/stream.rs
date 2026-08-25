// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 `GET /stream` (44 §2.2) — **M6-12**'s map, **M6-13**'s cursor, and the five records that stay in.
//!
//! # The map, and why it is a table rather than a `match` somebody reads
//!
//! 44 §2.2 lists **ten** events, 33 NFR-018 requires the same ten, and the engine's journal holds
//! **fifteen** record kinds (`gx_engine::store::JOURNAL_RECORD_KINDS`). §47 M6-12, adopted (a) (sem: SEM-gx-api-262), asks for one
//! place where the two vocabularies meet:
//!
//! > **put the mapping table in one place, and machine-check that the 4 unmapped records are "internal to the journal, not emitted to the stream"**
//! > (grounded in 42 §3.13, verbatim: "the journal is the engine's internal in-progress pipeline record … **not published externally**"). The table
//! > takes M5H2-7's denominator shape = turns "the journal vocabulary grew and the stream side silently drops it" RED (sem: SEM-gx-api-263)
//!
//! [`EVENT_MAP`] is that place. Every one of the thirteen has a row, each row is either an event
//! vocabulary or a reason for staying inside, and [`MAP_COVERS_THE_JOURNAL`] is a **compile-time**
//! assertion that the thirteen rows are the thirteen kinds, in order. A fourteenth record added to
//! gx-engine tomorrow stops this crate from compiling, which is the difference between a map and a
//! description of one.
//!
//! # 🔴 The count is **five**, not four, and two of the four named were the wrong two
//!
//! req/88 §4 M6-12 named the unmapped records "`Planned`, `ProvenanceDerived`, `InverseEscrowed`,
//! `ApplyStarted`" (sem: SEM-gx-api-264) and the hand-6 brief hedged it ("the actual set is settled by cross-checking 42 §3.13"). Read against
//! 42 §3.13 and 43 §3 the answer is **`DraftCreated`, `CommittingStarted`, `ProvenanceDerived`,
//! `InverseEscrowed`, `ApplyStarted`** — five — and `Planned` is the producer of
//! `candidate.created`. Four independent reasons, each of which alone settles it:
//!
//! 1. **The envelope cannot be filled.** 44 §2.2's line is
//!    `{event, at, transformation: "gx1:...", data}` and `DraftCreated` has **no
//!    `TransformationId`** — E-M5-3 keyed it on the `IntentId` because none exists yet, and
//!    `EngineJournalRecord::transformation()` answers `None` for that variant alone.
//! 2. **43 §1's own definition.** `Candidate` is the state reached at **T-2** ("the state where snapshot+plan have completed
//!    … the `TransformationId` is settled"; sem: SEM-gx-api-265). A `candidate.created` at T-1 would announce a candidate
//!    that does not exist yet.
//! 3. **44 §0 forbids the Draft on this surface.** "HTTP `POST /candidates` runs submit+plan atomically
//!    as one, so it does not expose the Draft-alone state" (sem: SEM-gx-api-266).
//! 4. **The data fields fit.** `candidate.created`'s data is `{intent_digest, substrate, locator}`,
//!    and `Planned` is the only record carrying both the intent id and the **locator** — a field it
//!    gained under **E-M5-13** in hand 1, for resume. `DraftCreated` has no locator.
//!
//! Raised as **M6H6-1** under discipline 49's lane-side reconciliation (sem: SEM-gx-api-267). The five are then exactly 42
//! §3.13's sentence: `CommittingStarted`, `ProvenanceDerived`, `InverseEscrowed` and `ApplyStarted`
//! are the inside of 43 T-9's critical section, and a client watching a commit sees T-9 open
//! (`canonicalized`) and close (`committed` or `aborted`) rather than the four steps between.
//!
//! # 🔴 L343 is read through req/38 §45, not through 44's letter (**E-M5-8**)
//!
//! 44 §2.2's `canonicalized` row says the `enforced` field is false "only via T-8r's record-only road" (sem: SEM-gx-api-268).
//! §45 M5H8-1 rules that stale — "`enforced` **follows the Transformation, not the transition**(while degrading FailOpen,
//! even the Admitted→Canonicalized road gets `Some(false)`)" (sem: SEM-gx-api-269) — and the engine already writes it that way
//! (`Engine::canonicalize`'s note on T-4e). So this module reads `Engine::enforced`, the
//! transformation's own answer, and never the transition it arrived by. Following 44's letter would
//! make a **fail-open degradation invisible in the audit stream**, which is INV-S5 inverted.
//!
//! # 🔴 The cursor (**M6-13, adopted (a)**; sem: SEM-gx-api-270), and why it is the list endpoints' cursor too
//!
//! 44 §2.2 offers `?since=<ledger_index|RFC3339>` or a `Last-Event-ID` header, and req/88 M6-13
//! showed that neither of the two `?since=` forms can name a `Denied`/`Aborted` event: INV-S4 keeps
//! those off the ledger, and a clock 41 §6 injects can repeat. §47 adopted (a) — the resume
//! coordinate is the **journal record index**, published as an opaque string, and the two `?since=`
//! spellings are *entrances* that resolve into it. E-M6-6 already fixed the same coordinate for
//! `gx replay --from/--to`, and M6-05's list endpoints share it: **one coordinate, three consumers**.
//!
//! A record can produce more than one event, so the cursor is `<record>.<ordinal>` ([`Cursor`]) and
//! resume is strictly *after* it. Without the ordinal a client that died between `verdict.issued`
//! and the `receipt.issued` of the same `Verdict` record would have to choose between replaying one
//! and losing the other.
//!
//! # Why the reader polls the journal instead of being notified
//!
//! A broadcast channel a handler forgets to publish to is a stream that goes quiet with every gate
//! green — the shape 44 §2.5's guard avoided by living on the router rather than in thirteen
//! handlers. The journal's length is the one fact that cannot be forgotten: it grows because a
//! transition happened, whoever drove it. The cost is a lock acquisition per subscriber per tick
//! ([`TICK`]), which the single lock of M6-06, adopted (a) (sem: SEM-gx-api-271), already serialises everything behind.

use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use gx_core::{Timestamp, TransformationId};
use gx_engine::store::EngineJournalRecord;
use gx_engine::{Engine, EvidenceSource};
use serde::Deserialize;

use crate::extract::Params;
use crate::problem::ApiError;
use crate::state::{AppState, RequestEvidence};

/// How often a subscriber looks at the journal again. See the module header for why it looks.
pub const TICK: Duration = Duration::from_millis(20);

/// How many events one tick may take, so that a subscriber joining an old project does not hold the
/// engine's lock while it serialises ten thousand lines.
pub const BATCH: usize = 256;

/// 44 §2.2's event vocabulary, in the order the table gives them. 33 NFR-018 requires the same ten.
pub const EVENTS: [&str; 10] = [
    "candidate.created",
    "verify.started",
    "verdict.issued",
    "escalated",
    "human.decision",
    "canonicalized",
    "committed",
    "aborted",
    "superseded",
    "receipt.issued",
];

/// One journal kind, and what it becomes on the wire.
#[derive(Debug, Clone, Copy)]
pub struct MapRow {
    /// The `gx_engine::store::JOURNAL_RECORD_KINDS` entry.
    pub record: &'static str,
    /// The events it produces, in emission order. **Empty** means it stays inside the engine.
    pub events: &'static [&'static str],
    /// Why — the transition for a mapped row, the reason for an unmapped one.
    pub note: &'static str,
}

impl MapRow {
    /// Whether this record reaches the stream at all.
    #[must_use]
    pub const fn published(&self) -> bool {
        !self.events.is_empty()
    }
}

/// 🔴 The map: fifteen journal kinds onto ten events, with the seven that stay in.
///
/// The order is `JOURNAL_RECORD_KINDS`' order, which [`MAP_COVERS_THE_JOURNAL`] asserts at compile
/// time. `tests/stream.rs` asserts the other direction — that every one of [`EVENTS`] is produced by
/// some row — so a vocabulary that grew on either side is a failure and not a silence.
pub const EVENT_MAP: [MapRow; 15] = [
    MapRow {
        record: "DraftCreated",
        events: &[],
        note: "T-1. No `TransformationId` exists yet (E-M5-3), so 44 §2.2's envelope has no id to \
               carry; 44 §0 also keeps the Draft off this surface (\"it does not expose the Draft-alone state\"; sem: SEM-gx-api-272).",
    },
    MapRow {
        record: "Planned",
        events: &["candidate.created"],
        note: "T-2 — 43 §1's `Candidate` begins here. The only record holding the intent id and the \
               locator together (E-M5-13), which is `candidate.created`'s data.",
    },
    MapRow {
        record: "VerifyStarted",
        events: &["verify.started"],
        note: "T-3, and the one event whose data 44 §2.2 leaves empty: what a subscriber learns is \
               that evidence collection began, and 43 T-3's side effect is \"launching the evidence collector\" (sem: SEM-gx-api-273) \
               — nothing a client could be given.",
    },
    MapRow {
        record: "Verdict",
        events: &["verdict.issued", "escalated", "receipt.issued"],
        note: "T-4a/b/c. `escalated` only for `Escalate` (43 T-4c's ticket), `receipt.issued` only \
               when ASM-14's `VerdictReceipt` was issued — T-4e reaches this record with no verdict \
               and no receipt (E-M5-7, E-M5-11).",
    },
    MapRow {
        record: "HumanDecision",
        events: &["human.decision", "receipt.issued"],
        note: "T-5 / T-5b. 43's side effect is \"appending a signed human-ruling receipt to the provenance chain\" (sem: SEM-gx-api-274), so \
               the ruling is a second `VerdictReceipt` producer — a third for `receipt.issued`, \
               where req/88 M6-12 counted two.",
    },
    MapRow {
        record: "Canonicalized",
        events: &["canonicalized"],
        note: "T-8 / T-8r. `enforced` is the transformation's (E-M5-8 / §45 M5H8-1) and not the \
               transition's, so a FailOpen degradation is visible here.",
    },
    MapRow {
        record: "CommittingStarted",
        events: &[],
        note: "T-9 opens 43 §1's critical section. 42 §3.13: the journal is \"the engine's internal in-progress\
               pipeline record … not published externally\" (sem: SEM-gx-api-275), and 44 gives the section two edges (`canonicalized`, \
               then `committed` or `aborted`) rather than its inside.",
    },
    MapRow {
        record: "ProvenanceDerived",
        events: &[],
        note: "M5-25, adopted (a) (sem: SEM-gx-api-276). Inside T-9's section; the value it carries reaches the outside through \
               the receipt, which `receipt.issued` names.",
    },
    MapRow {
        record: "InverseEscrowed",
        events: &[],
        note: "T-10b. Inside T-9's section. 44's `committed` data has no inverse field.",
    },
    MapRow {
        record: "ApplyStarted",
        events: &[],
        note: "E-M5-1. Inside T-9's section, and written **before** `adapter.apply` precisely \
               because nothing outside may assume the world has not moved.",
    },
    MapRow {
        record: "ApplyObserved",
        events: &[],
        note: "Two-phase escrow (req/38 §98 ruling 1; sem: SEM-gx-api-277). Inside T-9's section: the applied call's \
               observed answer, journalled for the escrow completion alone — an engine-internal \
               record like the two beside it.",
    },
    MapRow {
        record: "InverseCompleted",
        events: &[],
        note: "Two-phase escrow's outcome, inside T-9's section. What reaches the outside is the \
               receipt's `inverse_delta` (the completed CID, or None as the fold's fingerprint), \
               which `receipt.issued` already carries.",
    },
    MapRow {
        record: "Committed",
        events: &["committed", "receipt.issued"],
        note: "T-11. `receipt.issued` carries ASM-14's `CommitReceipt`.",
    },
    MapRow {
        record: "Aborted",
        events: &["aborted"],
        note: "T-4d/T-6/T-7/T-10a/T-10c. NFR-018: \"record all 6 variants of `AbortReason` in a distinguishable form\" (sem: SEM-gx-api-278).",
    },
    MapRow {
        record: "Superseded",
        events: &["superseded"],
        note: "T-12, and the only event about a transformation another transformation caused: 43 §5 \
               makes an undo a new commit rather than a rollback, so this line arrives about the \
               superseded row while `committed` arrives about the undoing one.",
    },
];

/// `true` when two `&str` hold the same bytes, in a `const` context.
const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// 🔴 The compile-time half of M6-12: fifteen rows, the fifteen kinds, in order.
///
/// `gx_engine::store::JOURNAL_RECORD_KINDS` is the engine's own declaration and gx-engine's
/// `journal_vocabulary.rs` already ties it to the variants. Tying **this** table to that array means
/// a fourteenth record cannot be added without a hand deciding, in this file, whether it is published
/// — which is the whole of "the journal vocabulary grew and the stream side silently drops it" turning RED (sem: SEM-gx-api-279), one step
/// earlier than red: the crate does not build.
pub const MAP_COVERS_THE_JOURNAL: () = {
    assert!(EVENT_MAP.len() == gx_engine::store::JOURNAL_RECORD_KINDS.len());
    let mut i = 0;
    while i < EVENT_MAP.len() {
        assert!(str_eq(
            EVENT_MAP[i].record,
            gx_engine::store::JOURNAL_RECORD_KINDS[i]
        ));
        i += 1;
    }
};

/// How many journal kinds never reach the wire. Seven since v0.3-a — see the module header.
pub const UNPUBLISHED: usize = {
    let mut count = 0;
    let mut i = 0;
    while i < EVENT_MAP.len() {
        if EVENT_MAP[i].events.is_empty() {
            count += 1;
        }
        i += 1;
    }
    count
};

/// The row for a journal kind, by name.
#[must_use]
pub fn row_for(record: &str) -> Option<&'static MapRow> {
    EVENT_MAP.iter().find(|row| row.record == record)
}

// ---------------------------------------------------------------------------
// The cursor (M6-13, adopted (a); sem: SEM-gx-api-280)
// ---------------------------------------------------------------------------

/// 🔴 A position in the journal, plus which event of that record — see the module header.
///
/// `PartialOrd` is the whole contract: resume means "everything strictly after this" (sem: SEM-gx-api-281), and the
/// derived lexicographic order over `(record, ordinal)` is exactly that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Cursor {
    /// Index into `EngineJournal::records()`.
    pub record: usize,
    /// Which event of that record, from 0.
    pub ordinal: usize,
}

/// 🔴 The wrapping that makes the cursor opaque (**M6H8-12, adopted (a)**, req/38 §55; sem: SEM-gx-api-282).
///
/// # What was wrong with `12.2`
///
/// The first spelling was `format!("{}.{}", record, ordinal)`, and `record` is an index into
/// `EngineJournal::records()` — 42 §3.13 says the journal is "the engine's internal in-progress pipeline record …
/// **not published externally**" (sem: SEM-gx-api-283), and 44 §2.7 calls the cursor "opaque". Three things leaked and the second is
/// the one that decided this: the journal's **length** (the newest cursor is a lower bound), the
/// existence and **count of the five record kinds that publish no event** (`DraftCreated`,
/// `CommittingStarted`, `ProvenanceDerived`, `InverseEscrowed`, `ApplyStarted` consume indices, so a
/// client that counts the gaps in the cursor sequence reconstructs where they are), and the ability
/// to **invent** a coordinate the server never issued.
///
/// # Why a multiplication and not a cipher
///
/// There is no key and there is no secret: the constant below is in this file. What the wrapping
/// buys is that the text a client receives carries no arithmetic a client can do — the cursor stops
/// being a number they can add one to, count gaps in, or read a length out of. `MIX` is odd, so
/// multiplication modulo 2¹²⁸ is a **bijection**; [`UNMIX`] is its inverse and the const assertion
/// below is the proof, checked by the compiler on every build.
///
/// Residue, stated rather than hidden: anyone who reads this source can decode a cursor, and the
/// coordinate is still the journal index underneath. Closing that needs a keyed MAC and a place to
/// keep the key, which is v0.2.1's question (45 TH-11's neighbourhood), not this batch's.
const MIX: u128 = 0x9E37_79B9_7F4A_7C15_F39C_C060_5CED_C835;

/// The multiplicative inverse of [`MIX`] modulo 2¹²⁸, by Newton's iteration.
const UNMIX: u128 = {
    // Five correct bits from the seed, doubling each round: 5·2⁷ = 640 ≥ 128.
    let mut x = MIX.wrapping_mul(3) ^ 2;
    let mut round = 0;
    while round < 7 {
        x = x.wrapping_mul(2u128.wrapping_sub(MIX.wrapping_mul(x)));
        round += 1;
    }
    x
};

/// The bijection is a bijection. A compile-time failure here would mean cursors that do not
/// round-trip, which is a stream that cannot be resumed.
const _: () = assert!(MIX.wrapping_mul(UNMIX) == 1);

/// The width of the wrapped form: 128 bits as lower-case hex.
const CURSOR_TEXT_LEN: usize = 32;

impl Cursor {
    /// 44 §2.7's "`cursor=<opaque string>`" (sem: SEM-gx-api-284) — opaque to a client, ordered here.
    ///
    /// See `MIX` (private) for what "opaque" (sem: SEM-gx-api-285) buys and what it does not.
    #[must_use]
    pub fn to_text(self) -> String {
        let packed = ((self.record as u128) << 64) | self.ordinal as u128;
        format!("{:032x}", packed.wrapping_mul(MIX))
    }

    /// Read one back.
    ///
    /// # Errors
    /// [`ApiError`] `VALIDATION_ERROR` for anything that is not one of this server's cursors. A
    /// cursor a client invented is a client asking for a position nothing published, and answering
    /// from the beginning instead would silently re-deliver a stream they already have.
    pub fn parse(text: &str) -> Result<Self, ApiError> {
        let malformed = || {
            ApiError::validation(format!(
                "`{text}` is not a cursor: the form is opaque (44 §2.7) and is only ever obtained \
                 from a previous response — a `cursor` field, a `next_cursor`, or an SSE id"
            ))
        };
        if text.len() != CURSOR_TEXT_LEN {
            return Err(malformed());
        }
        let packed = u128::from_str_radix(text, 16).map_err(|_| malformed())?;
        let unpacked = packed.wrapping_mul(UNMIX);
        Ok(Self {
            record: usize::try_from(unpacked >> 64).map_err(|_| malformed())?,
            ordinal: usize::try_from(unpacked & u128::from(u64::MAX)).map_err(|_| malformed())?,
        })
    }
}

// ---------------------------------------------------------------------------
// The projection
// ---------------------------------------------------------------------------

/// One line of 44 §2.2's stream, before it is serialised.
#[derive(Debug, Clone)]
pub struct Event {
    /// One of [`EVENTS`].
    pub kind: &'static str,
    /// The envelope's `transformation`.
    pub transformation: TransformationId,
    /// The envelope's `at`, from the record (41 §6's injected clock).
    pub at: Timestamp,
    /// The envelope's `data`, per 44 §2.2's table.
    pub data: serde_json::Value,
}

impl Event {
    /// 44 §2.2's envelope, plus the cursor this line is at.
    ///
    /// 🔴 The fifth field is an **addition**, and 44 §2.6 permits it ("a backward-compatible addition (a new optional
    /// field …) is allowed within `/v1`"; sem: SEM-gx-api-286). It has to be there: 44 asks for resume by `Last-Event-ID`
    /// and NDJSON, unlike SSE, has no frame in which an id could travel — so without a field the
    /// header names a value no client was ever given. Raised as **M6H6-2**.
    #[must_use]
    pub fn envelope(&self, cursor: Cursor) -> serde_json::Value {
        serde_json::json!({
            "event": self.kind,
            "at": crate::rfc3339::of(self.at),
            "transformation": self.transformation.0.to_text(),
            "data": self.data,
            "cursor": cursor.to_text(),
        })
    }
}

/// 42 §3.1's substrate in 44 §2.2's spelling.
fn substrate_text(kind: &gx_core::SubstrateKind) -> String {
    match kind {
        gx_core::SubstrateKind::Fs => "fs".to_string(),
        gx_core::SubstrateKind::Git => "git".to_string(),
        gx_core::SubstrateKind::Mcp => "mcp".to_string(),
        gx_core::SubstrateKind::Custom(name) => format!("custom:{name}"),
    }
}

/// ASM-14's two receipt kinds, as `receipt.issued`'s data.
fn receipt_data(receipt: &gx_witness::Receipt, kind: &str) -> Option<serde_json::Value> {
    let payload = receipt.payload().ok()?;
    let digest = receipt.ledger_digest().ok()?;
    Some(serde_json::json!({
        "receipt_digest": digest.to_text(),
        "key_id": payload.key_id,
        "receipt_kind": kind,
    }))
}

/// 🔴 One journal record, as the events 44 §2.2 publishes for it.
///
/// **No `_` arm.** `EngineJournalRecord` is not `#[non_exhaustive]`, so this `match` is the second
/// mechanical half of M6-12 (the first is [`MAP_COVERS_THE_JOURNAL`]): a fourteenth variant is a
/// compile error here as well as there, and the two failures say different things — one that the
/// table has no row, the other that the projection has no decision.
///
/// The engine is read for what the record does not carry (a ticket's reasons, the enforced flag,
/// a receipt). That is Rule 1 in its ordinary working shape (sem: SEM-gx-api-287): every value below is an accessor's answer,
/// and nothing here computes a `Cid`, builds a `Verdict` or writes a `Lifecycle`.
pub fn events_for<E: EvidenceSource>(
    record: &EngineJournalRecord,
    engine: &Engine<E>,
) -> Vec<Event> {
    let at = record.at();
    let one = |kind: &'static str, id: TransformationId, data: serde_json::Value| Event {
        kind,
        transformation: id,
        at,
        data,
    };

    match record {
        // The five that stay inside the engine — see EVENT_MAP for each row's reason.
        EngineJournalRecord::DraftCreated { .. }
        | EngineJournalRecord::CommittingStarted { .. }
        | EngineJournalRecord::ProvenanceDerived { .. }
        | EngineJournalRecord::InverseEscrowed { .. }
        | EngineJournalRecord::ApplyStarted { .. }
        | EngineJournalRecord::ApplyObserved { .. }
        | EngineJournalRecord::InverseCompleted { .. } => Vec::new(),

        EngineJournalRecord::Planned {
            transformation,
            intent_id,
            locator,
            ..
        } => vec![one(
            "candidate.created",
            *transformation,
            serde_json::json!({
                "intent_digest": intent_id.0.to_text(),
                "substrate": engine
                    .planned_delta(transformation)
                    .map(|d| substrate_text(d.substrate())),
                "locator": locator,
            }),
        )],

        EngineJournalRecord::VerifyStarted { transformation, .. } => vec![one(
            "verify.started",
            *transformation,
            serde_json::json!({}),
        )],

        EngineJournalRecord::Verdict {
            transformation,
            kind,
            verdict_digest,
            ..
        } => {
            let mut events = vec![one(
                "verdict.issued",
                *transformation,
                serde_json::json!({
                    "kind": kind,
                    "proof_digest": verdict_digest.map(|d| d.to_text()),
                }),
            )];
            // 🔴 **H-04** (`req/182` §1-1, `req/189`): `escalated` is decided by the **record**
            // — `kind == Escalate` — and not by whether the live row still holds a ticket. Until
            // v0.4-l this read `engine.ticket()`, so a replay after a ruling (or after a restart,
            // when the table is empty) produced no `escalated` line for a record that had one the
            // first time: the same cursor named a different ordinal on the second read. The
            // ticket's contents come from `Engine::ticket_as_raised` (the live ticket while the
            // row waits, the M6H3-10 rebuild from Σ once it has moved on) — the engine constructs
            // it, this layer reads it (rule 1). `null` fields only if the rebuild itself refuses,
            // which is `InconsistentTicket` territory and is reported as such rather than hidden.
            if *kind == gx_core::VerdictKind::Escalate {
                let (ticket_id, reasons) = match engine.ticket_as_raised(transformation) {
                    Ok(Some(ticket)) => (
                        serde_json::Value::String(ticket.id.0.to_text()),
                        serde_json::to_value(&ticket.reasons).unwrap_or(serde_json::Value::Null),
                    ),
                    Ok(None) | Err(_) => (serde_json::Value::Null, serde_json::Value::Null),
                };
                events.push(one(
                    "escalated",
                    *transformation,
                    serde_json::json!({
                        "ticket_id": ticket_id,
                        "reasons": reasons,
                    }),
                ));
            }
            // ASM-14's verdict receipt, when one was issued. T-4e issues none (E-M5-11), and the
            // absence is why this is a `first()` rather than an assumption.
            if let Some(data) = engine
                .verdict_receipts(transformation)
                .first()
                .and_then(|r| receipt_data(r, "VerdictReceipt"))
            {
                events.push(one("receipt.issued", *transformation, data));
            }
            events
        }

        EngineJournalRecord::HumanDecision {
            transformation,
            kind,
            reason,
            ..
        } => {
            let mut events = vec![one(
                "human.decision",
                *transformation,
                serde_json::json!({
                    // 44 §2.2's data names the ticket. ~~A ruling resolves the escalation, so the
                    // ticket is gone from the row by the time this is read back from Σ — the id is
                    // taken from the live row where it is still there, and `null` where it is not,
                    // rather than invented.~~
                    // 🔴 L-02 (`req/182` §1-3, `req/189`): the struck sentence described a world
                    // that did not exist — until v0.4-l nothing removed the ticket after a ruling
                    // (H-04), so this read the *stale* live ticket. Now the ticket **is** gone
                    // from the row (H-04 repaired), and the id is the one T-4c raised, rebuilt by
                    // the engine from Σ (`Engine::ticket_as_raised`) — the same id on the first
                    // read and on every replay, restart included.
                    "ticket_id": engine
                        .ticket_as_raised(transformation)
                        .ok()
                        .flatten()
                        .map(|t| t.id.0.to_text()),
                    "decision": kind,
                    "reason": reason,
                }),
            )];
            if let Some(data) = engine
                .verdict_receipts(transformation)
                .last()
                .and_then(|r| receipt_data(r, "VerdictReceipt"))
            {
                events.push(one("receipt.issued", *transformation, data));
            }
            events
        }

        EngineJournalRecord::Canonicalized {
            transformation,
            canonical_cid,
            ..
        } => vec![one(
            "canonicalized",
            *transformation,
            serde_json::json!({
                "canonical_cid": canonical_cid.to_text(),
                // 🔴 E-M5-8: the transformation's flag, not the transition's. See the module header.
                "enforced": engine.enforced(transformation),
            }),
        )],

        EngineJournalRecord::Committed {
            transformation,
            ledger_seq,
            ..
        } => {
            let mut events = vec![one(
                "committed",
                *transformation,
                serde_json::json!({
                    "canonical_cid": engine.canonical_cid(transformation).map(|c| c.to_text()),
                    "ledger_index": ledger_seq,
                    "enforced": engine.enforced(transformation),
                }),
            )];
            if let Some(data) = engine
                .receipt(transformation)
                .and_then(|r| receipt_data(r, "CommitReceipt"))
            {
                events.push(one("receipt.issued", *transformation, data));
            }
            events
        }

        // 🔴 **R29 / `req/361` L-03** — the record holds `rollback` and this arm used to drop it
        // with `..`. The twenty-eighth audit filed the omission as a **denominator** problem rather
        // than a wire one: R28's hand-off named `GET /transformations/{id}` and the list as the two
        // faces that could not answer "is my object back where it was", and a ruling taken on two
        // faces leaves the third behind. This is the cheapest of the three — the value is already
        // in this function's hand, no accessor and no second read.
        //
        // `null` where the record carries no roll-back (an abort that never entered T-10c's guard),
        // which is the same honesty `crate::problem::RollbackFacts` keeps on the refusal face: a
        // member carrying `null` is a fact a reader can branch on, where absence is not.
        EngineJournalRecord::Aborted {
            transformation,
            reason,
            rollback,
            ..
        } => vec![one(
            "aborted",
            *transformation,
            serde_json::json!({
                "reason": reason,
                "rollback": rollback.as_ref().map(|r| r.kind()),
            }),
        )],

        EngineJournalRecord::Superseded {
            transformation, by, ..
        } => vec![one(
            "superseded",
            *transformation,
            serde_json::json!({ "superseded_by": by.0.to_text() }),
        )],
    }
}

/// Every event strictly after `after`, up to [`BATCH`] of them, with the cursor each sits at.
///
/// Called with the engine's lock held and nothing awaited inside: a subscriber that held the lock
/// across an `await` would stop every other request for as long as its socket took to drain.
pub fn events_after(
    engine: &Engine<RequestEvidence>,
    after: Cursor,
) -> (Vec<(Cursor, Event)>, Cursor) {
    let records = engine.journal().records();
    let mut out = Vec::new();
    let mut last = after;
    for (index, record) in records.iter().enumerate().skip(after.record) {
        for (ordinal, event) in events_for(record, engine).into_iter().enumerate() {
            let cursor = Cursor {
                record: index,
                ordinal,
            };
            if cursor <= after {
                continue;
            }
            last = cursor;
            out.push((cursor, event));
            if out.len() >= BATCH {
                return (out, last);
            }
        }
    }
    (out, last)
}

// ---------------------------------------------------------------------------
// Resume — 44 §2.2's two entrances into the one coordinate
// ---------------------------------------------------------------------------

/// `GET /stream`'s query.
#[derive(Debug, Default, Deserialize)]
pub struct StreamQuery {
    /// 44 §2.2: "`?since=<ledger_index|RFC3339>`" (sem: SEM-gx-api-288).
    pub since: Option<String>,
}

/// 🔴 Resolve 44 §2.2's resume request into a [`Cursor`] — the M6-13, adopted (a), reading (sem: SEM-gx-api-289).
///
/// Three entrances, one coordinate:
///
/// * `Last-Event-ID` carries a cursor this stream published. It is the **only** one that can name a
///   `Denied` or an `Aborted` event, which is req/88 M6-13's finding: INV-S4 keeps those off the
///   ledger, so a ledger index cannot point at them.
/// * `?since=<ledger_index>` resolves to the journal position of the `Committed` record carrying
///   that `ledger_seq`, and the stream resumes **after** it.
/// * `?since=<RFC3339>` resolves to the first record at or after that instant; the client is told
///   nothing about ordering guarantees a clock cannot give (41 §6 injects it, and a test may move it
///   backwards), so the resolution is a scan for the first record whose `at` is not earlier.
///
/// # Errors
/// [`ApiError`] `VALIDATION_ERROR` for a malformed cursor or an unparseable `since`.
pub fn resume_from(
    engine: &Engine<RequestEvidence>,
    headers: &HeaderMap,
    since: Option<&str>,
) -> Result<Cursor, ApiError> {
    if let Some(value) = headers.get("last-event-id").and_then(|v| v.to_str().ok()) {
        return Cursor::parse(value);
    }
    let Some(since) = since else {
        // No resume: everything the journal holds, then whatever arrives. A subscriber that only
        // wants what is next asks for the cursor of the last line it saw.
        return Ok(Cursor::default());
    };

    let records = engine.journal().records();
    if let Ok(ledger_index) = since.parse::<u64>() {
        let found = records.iter().position(|r| {
            matches!(r, EngineJournalRecord::Committed { ledger_seq, .. } if *ledger_seq == ledger_index)
        });
        return found.map_or_else(
            || {
                Err(ApiError::validation(format!(
                    "no committed transformation carries ledger index {ledger_index}. 44 §2.2's \
                     `?since=<ledger_index>` can only name what INV-S4 puts on the ledger; a \
                     `Denied` or `Aborted` position is nameable only by the `Last-Event-ID` cursor \
                     this stream publishes (M6-13)"
                )))
            },
            |record| {
                Ok(Cursor {
                    record,
                    ordinal: usize::MAX,
                })
            },
        );
    }

    let at = crate::rfc3339::parse(since)?;
    let found = records.iter().position(|r| r.at().0 >= at.0);
    Ok(found.map_or(
        // Nothing at or after that instant: start from the end, so a resume with a future timestamp
        // is a subscription to what happens next rather than a replay of everything.
        Cursor {
            record: records.len(),
            ordinal: 0,
        },
        |record| {
            Cursor {
                record,
                // The first event *of* that record is wanted, so the exclusive cursor sits before it.
                ordinal: usize::MAX,
            }
            .pred()
        },
    ))
}

impl Cursor {
    /// The cursor immediately before this one, for a resolution that means "from here, inclusive" (sem: SEM-gx-api-290).
    #[must_use]
    const fn pred(self) -> Self {
        if self.record == 0 {
            Self {
                record: 0,
                ordinal: 0,
            }
        } else {
            Self {
                record: self.record - 1,
                ordinal: usize::MAX,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The handler
// ---------------------------------------------------------------------------

/// The body of a subscription: lines arriving from the reader task.
struct Lines {
    rx: tokio::sync::mpsc::Receiver<String>,
}

impl futures_core::Stream for Lines {
    type Item = Result<String, std::convert::Infallible>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx).map(|line| line.map(Ok))
    }
}

/// 🔴 `GET /stream` (44 §2.2) — `application/x-ndjson`, one JSON object per line.
///
/// The reader is a task rather than the response future because a response that produced its body
/// eagerly would be a snapshot with a stream's content type. It ends on three conditions and each is
/// deliberate:
///
/// 1. the client went away (`send` fails, because the receiver was dropped);
/// 2. **the server is shutting down** — a subscription is an in-flight request that never finishes,
///    so a graceful shutdown that waited for it would never complete. See [`mod@crate::serve`]: closing
///    the streams is the first of the three stages, not an optimisation of it;
/// 3. never otherwise. There is no built-in idle timeout: 44 gives none, and a stream that closed
///    itself after a quiet minute would make an operator's "nothing is happening" indistinguishable
///    from "the audit stream stopped" (sem: SEM-gx-api-291).
///
/// # Errors
/// [`ApiError`] `VALIDATION_ERROR` for a `Last-Event-ID` or `?since=` that names nothing.
pub async fn stream(
    State(state): State<AppState>,
    Params(query): Params<StreamQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let start = {
        let engine = state.engine();
        resume_from(&engine, &headers, query.since.as_deref())?
    };

    let (tx, rx) = tokio::sync::mpsc::channel::<String>(BATCH);
    let reader = state.clone();
    let shutdown: Arc<crate::serve::Shutdown> = state.shutdown();
    tokio::spawn(async move {
        let mut cursor = start;
        loop {
            if shutdown.begun() {
                break;
            }
            let batch = {
                let engine = reader.engine();
                let (events, last) = events_after(&engine, cursor);
                cursor = last;
                events
            };
            let idle = batch.is_empty();
            for (at, event) in batch {
                if tx.send(format!("{}\n", event.envelope(at))).await.is_err() {
                    return;
                }
            }
            if idle {
                tokio::time::sleep(TICK).await;
            }
        }
    });

    Ok((
        StatusCode::OK,
        [("content-type", "application/x-ndjson")],
        axum::body::Body::from_stream(Lines { rx }),
    )
        .into_response())
}
