// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The read side of the terminal face: four loopback reads and the vocabulary of nothing.
//!
//! # Why this module holds a hand-written client
//!
//! `crates/gx-cli/tests/ac_057.rs` scans every workspace member's manifest for the seven names that
//! can open a socket and requires **exactly one** declarer (`gx-api`). Declaring `reqwest` or `ureq`
//! here would make this crate a second road into the offline verifier's binary, which is the one
//! property that probe exists to keep. So the client is forty lines of [`std::net`] over
//! HTTP/1.1, and it is enough: this face reads `http://127.0.0.1:<port>` and nothing else.
//!
//! The membrane declaration of `req/942` §2 ("this face performs no effect") is therefore
//! **structural rather than promised**. [`Request::send`] writes the four bytes `GET ` and there is
//! no code path in this module that writes another method; `tui/tests/r942_tui.rs`
//! asserts the same fact twice, once by scanning this source and once by recording what a fixture
//! server actually received.
//!
//! # The six words for nothing, which are now seven
//!
//! 🔴 `req/38` SS974 queue row Q4 added [`Nothing::Empty`]: the wire carrying a key as `""` was
//! being drawn with the mark for *a count of nought*, inside the very function whose job is to keep
//! those apart. The heading is kept in the shape it was written, with the correction beside it,
//! because a heading edited to read as though it had always said seven takes the reader's chance to
//! see that this face got it wrong once.
//!
//! `req/942` §12 asks for six, and asks that the two failures they exist to prevent stay visible:
//! **loading is not unknown** (not yet measured against measured-and-unknowable) and **absent is
//! not false** (the wire never carried the key against the wire carrying `false`). The wire itself
//! draws both lines for us, which is why the classification in [`cell`] is mechanical rather than a
//! judgement: a key missing from the object is `absent`, a key present and `null` is `unknown`.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// The four routes this face reads. Same denominator as the browser face (`req/942` §9), so that a
/// difference between the two faces is a difference of medium and not of scope.
///
/// 🔴 **That sentence is AXIS A, not the whole face.** `req/38` SS1106 (2026-09-02, lane
/// [T-r59-parity-truth]) named it a false claim of exhaustiveness: this face has read
/// [`RECORD_ROUTES`] below since the two-route split was written, so "the four routes this face
/// reads" undercounts by two the moment a reader opens a row. The heading above is kept in the words
/// it was written in — the same choice this module's own "six words for nothing, which are now
/// seven" heading made — because a heading silently edited to read as though it had always been
/// exhaustive takes the reader's chance to see that this face's own gate got it wrong once
/// (`tools/gates/surface_parity_gate.mjs` derived its coverage set from this array alone and
/// reported [`RECORD_ROUTES`]'s two paths MISS; see that file's doc comment for the fix).
///
/// Read the line above as answering exactly one question — **AXIS A, the browser-parity
/// denominator**: does this face still declare the same route-*count* the browser face does. That
/// is what `tui/tests/r942_tui.rs`'s `assert_eq!(wire::ROUTES.len(), 4)` pins, and it is why
/// [`RECORD_ROUTES`] stays a second array rather than growing this one (see its own doc comment —
/// the separation is deliberate, not an omission).
///
/// A different question — **AXIS B, coverage**: everything this face actually reads — has a
/// different answer, `ROUTES` ∪ [`RECORD_ROUTES`] (plus `live.rs`'s `STREAM_ROUTE` when read beyond
/// its own declaration). Splitting the two axes was always the right call; not telling
/// `surface_parity_gate.mjs` about the split was the defect, and that gate now derives the AXIS B
/// union from every path-shaped `pub const` array this file declares rather than reading `ROUTES`
/// alone, so a third array of the same shape is picked up with no further edit to either file.
pub const ROUTES: [&str; 4] = [
    "/v1/healthz",
    "/v1/transformations",
    "/v1/candidates",
    "/v1/escalations",
];

/// The two routes this face reads about **one** record, as the templates `route-table.json`
/// declares them.
///
/// 🔴 **Kept out of [`ROUTES`] rather than added to it, and the separation is a claim rather than
/// tidiness.** [`ROUTES`] is a **denominator**: `tui/tests/r942_tui.rs` measures that it is the same
/// four the browser face reads, "so that a difference between the two faces is a difference of
/// medium and not of scope". These two are not in that denominator — they are read about **one row,
/// when a reader opens it**, and a face that never opens a row reads neither. Folding them in would
/// move the number the parity test compares and quietly change what that test is about.
///
/// 🔴 **They are spelled with their holes in.** `route-table.json` writes `{id}` and `{tid}`, the
/// screen writes `{id}` and `{tid}`, and [`record_path`] is the one place either is filled. A road a
/// reader can retype is worth more than a road already resolved — the resolved one is on the row
/// above it.
pub const RECORD_ROUTES: [&str; 2] = ["/v1/transformations/{id}", "/v1/receipts/{tid}"];

/// The hole each of [`RECORD_ROUTES`] carries, in the same order.
///
/// 🔴 Two spellings for one substitution, because the engine gives them two: 44 §2.2 names the
/// transformation's own id `{id}` on one route and `{tid}` on the other. This face does not
/// normalise them — a template edited to say something the route table does not say is a second
/// contract.
pub const RECORD_HOLES: [&str; 2] = ["{id}", "{tid}"];

/// The most bytes of id this face will put into a request line.
///
/// 🔴 A bound rather than trust: `gx1:` ids are short and fixed-width, and a length that is not
/// checked is a length the wire chooses.
pub const MAX_ID_BYTES: usize = 128;

/// Whether this face will put `id` into a request line.
///
/// # 🔴 The membrane's first clause stops being structural the moment a path stops being a constant
///
/// This module's own heading says the claim *this face performs no effect* is "**structural rather
/// than promised**": one function writes the four bytes `GET ` and no code path writes another
/// method.
///
/// 🔴 **That function is [`open`], and the heading's `Request::send` names nothing** (`[T-r55]`,
/// 2026-09-02, independent audit finding M1). There is no `Request` type in this file; `send`
/// exists and delegates; the bytes are written in `open`, which says so of itself. The heading is
/// left in the words it was written in — it is the claim's own history — and this paragraph is the
/// correction, because a doc link that resolves to nothing is a road that arrives nowhere and this
/// lane copied it into new prose before checking it.
///
/// The argument holds because every path `open` has ever written was a `&'static str` declared
/// in this file. [`RECORD_ROUTES`] breaks that: the path now carries a value **the engine sent us**,
/// and an id carrying a carriage return would let a list row compose a second request line — which
/// is a method this module cannot write becoming a method this module can be *made* to write.
///
/// So the set is named, and it is named as an allow-list rather than as a list of characters to
/// refuse: a deny-list is a claim about every character that exists, and this is a claim about the
/// eight-odd this face needs. `gx1:` ids are base32 text after a scheme and a colon
/// (`crates/gx-core`'s `Cid::to_text`), and the four punctuation marks below are RFC 3986's
/// unreserved set plus the colon a scheme needs.
///
/// 🔴 **It is not the only guard, and that is deliberate.** [`open`] refuses a path with a control
/// character or a space in it as well, so a caller that reaches it by some road this function does
/// not sit on still cannot write two request lines. Two layers, and each one says out loud what it
/// is for — the same shape as `acts::grounded` being asked by the reducer *and* by the draw road.
#[must_use]
pub fn addressable(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '_' | '~' | '-'))
}

/// The address of one record on one of [`RECORD_ROUTES`], or `None` for an id this face will not
/// put on a socket.
///
/// `None` is **not** an error to be drawn as a kind of nothing: it is this face declining to ask,
/// and [`Held::refusal`] is where that is said in words. A mark would report the engine's silence
/// for a question the engine was never asked.
#[must_use]
pub fn record_path(template: &str, id: &str) -> Option<String> {
    if !addressable(id) {
        return None;
    }
    RECORD_HOLES
        .iter()
        .find(|hole| template.contains(**hole))
        .map(|hole| template.replace(hole, id))
}

/// The default the terminal face reads when neither flag nor environment names one.
///
/// 🔴 `sdk/typescript/README.md` and `glovrex_app/monitor/serve.mjs` do not agree on this value
/// (`8787` there, `8842` here). `req/942_artifacts/build_lane_report.md` raises the disagreement
/// rather than picking silently; this constant records which side the terminal face took.
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8842";

/// The environment variable naming the server, borrowed rather than invented.
pub const BASE_URL_ENV: &str = "GX_BASE_URL";
/// The environment variable naming the bearer token, borrowed rather than invented.
pub const TOKEN_ENV: &str = "GX_TOKEN";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// The most header bytes this face will hold while waiting for the blank line that ends them.
const MAX_HEAD_BYTES: usize = 64 * 1024;

/// One route read once: what was asked, what came back, and the four facts the renderer measured
/// while asking.
///
/// 🔴 The last four members are the **renderer-local facts** of `req/942` §19-4: the engine returns
/// none of them from any of its routes, so a reading that is dropped from the screen cannot be
/// fetched again — a second read produces a *new* measurement, not the lost one. That is why the
/// region carrying them is `priority.1` and why, when it cannot be drawn at all, it is folded into
/// the disclosure line instead of dropped.
#[derive(Clone, Debug)]
pub struct Reading {
    /// The address, spelled the way a reader can retype it.
    pub route: String,
    /// The status line's code, or `None` when the exchange did not reach one.
    pub status: Option<u16>,
    /// When this process performed the read, RFC 3339.
    pub read_at: String,
    /// How long the read took.
    pub elapsed_ms: u128,
    /// The parsed body, or `None` when there was no body to parse.
    pub body: Option<serde_json::Value>,
    /// Why there is no body, in the words the operating system or the parser used.
    pub error: Option<String>,
}

impl Reading {
    /// The state before any read has happened: the only producer of [`Nothing::Loading`].
    ///
    /// 🔴 **Corrected the same turn a second producer was added** (`[T-r82]`, 2026-09-02;
    /// `req/924` §TUI-101). [`null_meaning`] now resolves a `Candidate` row's missing judged fields
    /// to the same mark, so *the only producer* is no longer true and is corrected here rather than
    /// left to be inherited — a doc comment that has drifted from the implementation is a lie the
    /// next reader has no way to catch.
    ///
    /// The two producers say the **same thing to a reader** — *not measured yet; something is still
    /// coming* — which is why an existing word was re-seated instead of an eighth being minted
    /// (§TUI-48 reserves that spelling to the Owner). They differ in **what** is still coming: here
    /// it is this process's read, and there it is the engine's own next transition. If those two
    /// ever need telling apart on the screen, that is a spelling question and it goes to the Owner.
    #[must_use]
    pub fn pending(path: &str) -> Self {
        Self {
            route: format!("GET {path}"),
            status: None,
            read_at: String::new(),
            elapsed_ms: 0,
            body: None,
            error: None,
        }
    }

    /// A read that has not happened yet is not a read that failed.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.read_at.is_empty()
    }

    /// Whether the server answered with the thing that was asked for.
    ///
    /// 🔴 **The distinction this method exists to keep.** A `401` is an answer, and it has a body,
    /// and the body has no `items` — so a face that asked "is the item list empty?" would draw
    /// `zero` and tell the reader there are no records. There are records; this process was not
    /// allowed to see them. That is `unknown`, and it is the same collapse (`zero` into `unknown`)
    /// the seven words exist to prevent, committed by the face rather than by the engine.
    ///
    /// Measured rather than reasoned: after a restart on this machine the bearer token no longer
    /// matched, `/v1/healthz` answered `200` (it sits outside the guard) and the other three
    /// answered `401`, and the first build of this face drew `0` in every cell of the table while
    /// the provenance line honestly read `status 200/401/401/401`. The line was right and the cells
    /// were wrong.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self.status, Some(code) if (200..300).contains(&code))
    }

    /// The kind of nothing a whole reading amounts to, before any key is looked at.
    ///
    /// `None` means the reading carried an answer and the cells should be read from it.
    #[must_use]
    pub fn nothing(&self) -> Option<Nothing> {
        if self.is_pending() {
            Some(Nothing::Loading)
        } else if self.body.is_none() || !self.is_ok() {
            Some(Nothing::Unknown)
        } else {
            None
        }
    }

    /// The object a row-shaped route answers with, if it answered with one.
    #[must_use]
    pub fn items(&self) -> Vec<&serde_json::Value> {
        self.body
            .as_ref()
            .and_then(|b| b.get("items"))
            .and_then(serde_json::Value::as_array)
            .map(|a| a.iter().collect())
            .unwrap_or_default()
    }
}

/// The four readings one frame draws.
#[derive(Clone, Debug)]
pub struct Screen {
    /// `GET /v1/healthz` — the apparatus region's source.
    pub healthz: Reading,
    /// `GET /v1/transformations` — the subject region's source.
    pub transformations: Reading,
    /// `GET /v1/candidates` — read, and declared not drawn.
    pub candidates: Reading,
    /// `GET /v1/escalations` — read, and declared not drawn.
    pub escalations: Reading,
}

impl Screen {
    /// The frame drawn before the first read returns.
    #[must_use]
    pub fn pending() -> Self {
        Self {
            healthz: Reading::pending(ROUTES[0]),
            transformations: Reading::pending(ROUTES[1]),
            candidates: Reading::pending(ROUTES[2]),
            escalations: Reading::pending(ROUTES[3]),
        }
    }

    /// Read all four, in declaration order.
    #[must_use]
    pub fn read(base_url: &str, token: Option<&str>) -> Self {
        Self {
            healthz: read_route(base_url, ROUTES[0], token),
            transformations: read_route(base_url, ROUTES[1], token),
            candidates: read_route(base_url, ROUTES[2], token),
            escalations: read_route(base_url, ROUTES[3], token),
        }
    }

    /// All four readings, in the order the provenance region reports them.
    #[must_use]
    pub fn readings(&self) -> [&Reading; 4] {
        [
            &self.healthz,
            &self.transformations,
            &self.candidates,
            &self.escalations,
        ]
    }
}

/// Read one route.
///
/// Every failure mode lands in [`Reading::error`] rather than in a `Result`: a face that cannot draw
/// because it could not ask is the case this face most needs to draw well, and an error return
/// would hand that decision to the caller.
#[must_use]
pub fn read_route(base_url: &str, path: &str, token: Option<&str>) -> Reading {
    let started = Instant::now();
    let outcome = send(base_url, path, token);
    let elapsed_ms = started.elapsed().as_millis();
    // 🔴 The `.0` that used to be here is gone with the crate this face left (#188/#189): the read
    // returned a `gx_core::Timestamp`, a newtype over `i64`, and unwrapping it was the whole of
    // what this line ever wanted from the engine's id crate. `crate::clock` is now a nanosecond
    // count and the graph carries no `gx-*` edge at all.
    let read_at = rfc3339(crate::clock::now_nanos());
    match outcome {
        Ok((status, raw)) => {
            let (body, error) = match serde_json::from_slice::<serde_json::Value>(&raw) {
                Ok(value) => (Some(value), None),
                Err(e) => (None, Some(format!("the body is not JSON: {e}"))),
            };
            Reading {
                route: format!("GET {path}"),
                status: Some(status),
                read_at,
                elapsed_ms,
                body,
                error,
            }
        }
        Err(error) => Reading {
            route: format!("GET {path}"),
            status: None,
            read_at,
            elapsed_ms,
            body: None,
            error: Some(error),
        },
    }
}

/// The key a row names itself with.
///
/// 🔴 **The one thing that makes a record face possible**, and it is a key of the list rather than
/// a name this face chose: `crates/gx-api/src/list.rs`'s `row_json` writes `"transformation":
/// id.0.to_text()`, so every row on the ledger carries the address it can be asked about. Declared
/// here rather than typed at the one call site because [`RECORD_ROUTES`] is filled from it —
/// a second spelling would be a second answer to *which member is the address*, and gate `g80`
/// measures that this is the same key `super::layout::LEDGER_COLUMNS` draws.
pub const LEDGER_ID_KEY: &str = "transformation";

/// The key `GET /v1/receipts/{tid}` mounts its decoded half under.
///
/// 🔴 The document itself is `envelope` + `issued_at`, and everything a receipt *says* is canonical
/// DAG-CBOR inside `envelope.payload`. `crates/gx-api/src/handlers.rs`'s `get_receipt` mounts
/// `receipt_view` **beside** the document rather than inside it, expressly so that a reader does not
/// have to carry a decoder. This face is that reader.
pub const RECEIPT_VIEW_KEY: &str = "receipt_view";

/// The members of `receipt_view` this face draws, in the order it draws them.
///
/// 🔴 **`alg` is not here and never will be**, and that is the engine's ruling rather than this
/// face's economy: `handlers.rs`'s `receipt_view` records that 33 NFR-011 makes the algorithm a
/// property of the **key**, and forbids a wire-side alg-like field. A face that spelled `Ed25519`
/// beside a receipt would be naming a fact no route carries — the reader's answer to *which
/// algorithm* is [`RECEIPT_KEY_ID`], resolved against the key they pinned.
pub const RECEIPT_VIEW_KEYS: [&str; 4] = ["key_id", "leaf_index", "tree_size", "root"];

/// The members of `receipt_view` this face **does not** draw, named because they were let go of.
///
/// 🔴 `[T-r58]` named these three and did not close them; `[T-r76]` closes them the second way the
/// membrane's second obligation allows — *drawn, or dropped and named* — and this array is the
/// naming. [`help_lines`](super::renderer::help_lines) spells it on the `record` entry, on the same
/// row that already names the route these members come off, so the hatch grows no new doorway: a
/// reader reaches all three from the one key they already press.
///
/// `crates/gx-api/src/handlers.rs`'s `receipt_view` writes seven members, and its own table gives
/// the reason each of these is dropped rather than drawn:
///
/// * **`subject`** — `payload.transformation`, which is *this record's own id*. The record already
///   spells it on its head row as `transformation`, so drawing it here would put the same
///   fifty-two characters on the screen a third time. That is the defect this lane exists to
///   remove, and closing one instance of it by adding another would be absurd.
/// * **`postcondition_fingerprint`** — a digest of what the transformation left behind. This face
///   has no route that carries the *thing* it fingerprints, so the value is a number a reader can
///   neither check nor use here; `gx receipt verify` is the road, and [`help_lines`]'s `beyond`
///   entry already spells it.
/// * **`issued_at`** — when the **receipt** was issued, which the record's own doc comment already
///   warns is a different fact from `created_at`. Two instants three rows apart, one labelled and
///   one not, is how two facts become one; the row is not bought.
///
/// 🔴 Together with [`RECEIPT_VIEW_KEYS`] this is the **whole** of the engine's object. Gate `g95`
/// asserts the two arrays are disjoint and cover all seven members named in `handlers.rs`'s table,
/// so a member the engine adds tomorrow makes the gate red rather than vanishing unnamed.
pub const RECEIPT_VIEW_NOT_DRAWN: [&str; 3] = ["subject", "postcondition_fingerprint", "issued_at"];

/// How the receipt block introduces [`RECEIPT_VIEW_NOT_DRAWN`].
///
/// 🔴 **Declared beside the array it introduces, and spelled `not spelled` rather than `not drawn`,
/// because a gate found the collision the other word makes.** `[T-r76]` first wrote `not drawn:`,
/// which is the face's idiom for the **cut note** — `not drawn: 5 of 6 more rows`, a sentence about
/// rows that ran out of screen. `g82` reads that string to decide whether the screen is claiming a
/// cut, and went red at every shape: one spelling had been given a second meaning and the screen
/// was reporting an event that had not happened. Two facts, one word, inside the module whose
/// subject is that they are two.
///
/// 🔴 The two differ in kind as well, which is why the second spelling is right rather than merely
/// convenient: the cut note is about **rows that ran out of screen** and a wider terminal undoes
/// it, and this is about **members this face will not draw at any width**.
pub const RECEIPT_VIEW_DROP_PHRASE: &str = "not spelled:";

/// The member that answers *which key signed this*, which is as close as any route comes to
/// answering *which algorithm*.
pub const RECEIPT_KEY_ID: &str = "key_id";

/// The three members of `receipt_view` whose `null` is [`Nothing::Absent`] and not
/// [`Nothing::Unknown`].
///
/// 🔴 **The third carve-out on this file's general rule, on the same evidence as the first two.**
/// [`inverse_status`] and [`status_reason`] are carved out because the engine's own source says what
/// its `null` means on those keys; so does this one. `crates/gx-api/src/handlers.rs`'s `receipt_view`
/// carries a table with a `null when` column, and for these three it reads *no inclusion proof
/// (ASM-14: every `VerdictReceipt`)*. A receipt with no inclusion proof is a receipt for which these
/// coordinates were **never written**, which is `Absent`'s definition — "a line where nothing was
/// ever written". Drawing `?` would say this face asked and could not know, and it knows.
///
/// 🔴 `key_id` is deliberately **not** in this set: the same table says `null` **never** for it, so a
/// `null` there is the engine breaking its own contract, and [`cell`]'s general rule drawing `?` is
/// the honest answer to a fact this face cannot account for.
pub const RECEIPT_NEVER_WRITTEN: [&str; 3] = ["leaf_index", "tree_size", "root"];

/// Classify one member of `receipt_view`.
///
/// See [`RECEIPT_NEVER_WRITTEN`] for why three keys do not read through [`cell`]'s general rule.
#[must_use]
pub fn receipt_cell(view: &serde_json::Value, key: &str) -> Cell {
    let Some(value) = view.get(key) else {
        return Cell::Nothing(Nothing::Absent);
    };
    if value.is_null() && RECEIPT_NEVER_WRITTEN.contains(&key) {
        return Cell::Nothing(Nothing::Absent);
    }
    cell(view, key)
}

/// What `GET /v1/receipts/{tid}` amounted to.
///
/// 🔴 **Five values and not two, and the fourth is the one that costs something to keep.** A face
/// that asked *is there a receipt* and answered yes-or-no would fold four different facts into two,
/// which is the collapse the seven words exist to refuse, committed one layer up from them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReceiptMark {
    /// The read has not happened yet.
    Loading,
    /// The engine answered with a document.
    Held,
    /// 🔴 The engine answered `404`, and **that answer covers two facts this face cannot tell
    /// apart**. `handlers.rs`'s `get_receipt` spells both in one refusal: *it has not been
    /// committed*, **or** *this server holds neither its row nor its archive*. The first is an
    /// absence; the second is this server's ignorance about a document that may well exist. One
    /// status code, two preimages.
    ///
    /// So this face draws neither `--` nor `?` for it — both would pick one of the two — and spells
    /// a phrase instead. The row beside it carries `state`, which is what lets a reader finish the
    /// sentence themselves: a row that is not `Committed` has no receipt to hold. **The face
    /// reports; the reader concludes.**
    NotHere,
    /// The read did not reach an answer: no body, or a status that is neither `2xx` nor `404`.
    Unknown,
    /// This face declined to ask. See [`addressable`].
    Refused,
}

impl ReceiptMark {
    /// All five, so a gate that sweeps them measures the set rather than a copy of it.
    pub const ALL: [ReceiptMark; 5] = [
        ReceiptMark::Loading,
        ReceiptMark::Held,
        ReceiptMark::NotHere,
        ReceiptMark::Unknown,
        ReceiptMark::Refused,
    ];

    /// What is drawn.
    ///
    /// 🔴 Three of the five borrow a mark from [`Nothing`] and two spell words, and the split is the
    /// ruling: a mark is available exactly when one of the seven words is the whole truth. For
    /// [`ReceiptMark::NotHere`] and [`ReceiptMark::Refused`] none of them is, so the screen spends
    /// the characters.
    #[must_use]
    pub fn mark(self) -> &'static str {
        match self {
            ReceiptMark::Loading => Nothing::Loading.mark(),
            ReceiptMark::Held => "held",
            ReceiptMark::NotHere => "not here",
            ReceiptMark::Unknown => Nothing::Unknown.mark(),
            ReceiptMark::Refused => "not asked",
        }
    }

    /// The paint role it is drawn in.
    #[must_use]
    pub fn role(self) -> super::tokens::Role {
        match self {
            ReceiptMark::Loading => Nothing::Loading.role(),
            ReceiptMark::Held => super::tokens::Role::Body,
            ReceiptMark::NotHere | ReceiptMark::Refused => Nothing::Absent.role(),
            ReceiptMark::Unknown => Nothing::Unknown.role(),
        }
    }

    /// What the mark means, for the page `?` opens.
    ///
    /// 🔴 Declared beside [`ReceiptMark::mark`] rather than written out in
    /// `super::renderer::help_lines`, for the reason [`Nothing::word`] is: a second copy of these
    /// five sentences is five sentences that stop being true one at a time. `held` and `not here`
    /// are already words rather than marks, and they still need this — **a word on the screen is
    /// not self-explaining just because it is a word**, and `not here` in particular is the one
    /// that has to say *which two facts* the engine folded into one status code.
    /// 🔴 **Declared, and no screen reads it yet.** That is the shape of a dead declaration, which
    /// this face has been caught with before (`super::renderer::help_lines`'s own heading: two
    /// declarations written, gated for internal consistency, and called by nothing that draws), so
    /// it is said out loud rather than left for an audit.
    ///
    /// The entry that would have drawn it was written and **measured off the page**: at 120x32 the
    /// hatch has a fixed number of rows, and a fourth entry of `[T-r55]`'s turned gates `g36` and
    /// `g65` red by pushing four acts' intents and the vacant-field names off it. Room was bought
    /// back out of that lane's own two entries and was still not enough. Taking it out of another
    /// entry was refused: displacing an act's intent to explain `key_id` trades one road for
    /// another, which is the reduction pass that cuts the honesty rather than the padding.
    ///
    /// So this stays, callable, as the single source the day the hatch can scroll or carry a second
    /// page — either of which is a ruling, not a lane's own decision. The full argument and the
    /// measured numbers are in `help_lines`, at the point where the entry would stand.
    ///
    /// What may **not** be shortened when that day comes is [`ReceiptMark::NotHere`]: it is the one
    /// mark standing for **two** facts, and a gloss naming only one of them would be the collapse
    /// the type exists to refuse.
    #[must_use]
    pub fn means(self) -> &'static str {
        match self {
            ReceiptMark::Loading => "not asked yet",
            ReceiptMark::Held => "a signed document",
            ReceiptMark::NotHere => "404: not committed, or this server does not hold it",
            ReceiptMark::Unknown => "no answer",
            ReceiptMark::Refused => "this face would not ask with that id",
        }
    }
}

/// The status `GET /v1/receipts/{tid}` answers when it holds no receipt.
const NO_RECEIPT: u16 = 404;

/// The key every refusal this surface issues carries (44 §2.3's table).
///
/// 🔴 Read as a **signature of the speaker**, not as a value: [`Held::receipt_mark`] uses it to tell
/// a refusal `crates/gx-api` issued from a `404` that came from somewhere else entirely.
pub const GX_CODE_KEY: &str = "gx_code";

/// What this face knows about **one** record, over and above the row the list carried.
///
/// 🔴 **Not a member of [`Screen`], and the reason is a measurement rather than a preference.**
/// `Screen` is constructed as a literal in ten places in `tui/tests/r942_tui.rs`; a fifth member
/// would edit ten tests that have nothing to do with this, and an edit to a test is a thing this
/// repository requires a dated ruling for. It is also the truer shape: `Screen` is *what one frame
/// of the ledger is*, and this is *what one row of it says about itself* — the two are read on
/// different occasions and one of them is usually not read at all.
#[derive(Clone, Debug)]
pub struct Held {
    /// The record this is about. Empty when no record is open, which is the discriminator
    /// [`Held::is_open`] reads.
    pub id: String,
    /// `GET /v1/transformations/{id}` — the row read again, about itself.
    pub transformation: Reading,
    /// `GET /v1/receipts/{tid}` — the signed document, if this server holds one.
    pub receipt: Reading,
    /// 🔴 Why this face did not ask, in words, when [`addressable`] refused the id. `None` is the
    /// ordinary case and does not mean "the reads succeeded" — the two [`Reading`]s say that.
    pub refusal: Option<String>,
}

impl Held {
    /// No record is open. Both reads are pending and neither will ever be performed.
    #[must_use]
    pub fn none() -> Self {
        Self {
            id: String::new(),
            transformation: Reading::pending(RECORD_ROUTES[0]),
            receipt: Reading::pending(RECORD_ROUTES[1]),
            refusal: None,
        }
    }

    /// A record is open and neither read has returned.
    #[must_use]
    pub fn pending(id: &str) -> Self {
        Self {
            id: id.to_string(),
            ..Self::none()
        }
    }

    /// Whether a record is open at all.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.id.is_empty()
    }

    /// Read both, in declaration order.
    ///
    /// 🔴 The refusal is decided **once**, before either socket is opened, and it refuses **both**
    /// reads rather than each separately: an id this face will not put in one request line is an id
    /// it will not put in the other, and deciding twice is two answers.
    #[must_use]
    pub fn read(base_url: &str, token: Option<&str>, id: &str) -> Self {
        let mut held = Self::pending(id);
        let Some(transformation) = record_path(RECORD_ROUTES[0], id) else {
            held.refusal = Some(format!(
                "this face will not ask about {id:?}: an id it can put in a request line is \
                 alphanumeric with `:._~-`, and no longer than {MAX_ID_BYTES} bytes"
            ));
            return held;
        };
        // `record_path` refused nothing above, so the second template resolves for the same reason.
        let Some(receipt) = record_path(RECORD_ROUTES[1], id) else {
            held.refusal = Some(format!(
                "{:?} carries neither of the two holes this face knows how to fill",
                RECORD_ROUTES[1]
            ));
            return held;
        };
        held.transformation = read_route(base_url, &transformation, token);
        held.receipt = read_route(base_url, &receipt, token);
        held
    }

    /// Both readings, in the order [`RECORD_ROUTES`] declares them.
    #[must_use]
    pub fn readings(&self) -> [&Reading; 2] {
        [&self.transformation, &self.receipt]
    }

    /// What the receipt read amounted to.
    ///
    /// 🔴 **A `404` is [`ReceiptMark::NotHere`] only when it came from this engine's own refusal**
    /// (`[T-r55]`, 2026-09-02, independent audit finding S5). The two facts `NotHere` stands for are
    /// the two `crates/gx-api/src/handlers.rs`'s `get_receipt` names, and they are facts **about a
    /// row**. A `404` that never reached that handler is a third preimage and a different kind of
    /// thing entirely: a server too old to carry the route, a proxy, or the wrong port — and this
    /// file documents a live port disagreement of its own two paragraphs above [`DEFAULT_BASE_URL`],
    /// so the wrong-server case is measured rather than imagined. Folding it in would make the face
    /// answer *there is no receipt for this row* about **every** row while talking to a server that
    /// cannot answer at all, which is the collapse this type exists to refuse, committed by the
    /// classifier that refuses it.
    ///
    /// The discriminator is the engine's own error contract: 44 §2.3 gives every refusal a
    /// [`GX_CODE_KEY`], so a body carrying one is a refusal this surface issued and a body without
    /// one is something else answering. **It is not proof** — a proxy could forge the key — and the
    /// point is not proof, it is that the face stops asserting a fact about a row on the strength of
    /// a status code alone.
    #[must_use]
    pub fn receipt_mark(&self) -> ReceiptMark {
        if self.refusal.is_some() {
            return ReceiptMark::Refused;
        }
        if self.receipt.is_pending() {
            return ReceiptMark::Loading;
        }
        match self.receipt.status {
            Some(NO_RECEIPT) if self.speaks_gx() => ReceiptMark::NotHere,
            _ if self.receipt.is_ok() && self.receipt.body.is_some() => ReceiptMark::Held,
            _ => ReceiptMark::Unknown,
        }
    }

    /// Whether the receipt read's body is a refusal **this surface** issued.
    #[must_use]
    fn speaks_gx(&self) -> bool {
        self.receipt
            .body
            .as_ref()
            .and_then(|body| body.get(GX_CODE_KEY))
            .is_some_and(serde_json::Value::is_string)
    }

    /// The decoded half of the receipt, when there is one.
    ///
    /// `None` covers two shapes and the caller may not tell them apart from here: no receipt at all,
    /// and a receipt whose payload would not decode (`handlers.rs`'s `receipt_view` answers `null`
    /// for that, and serves the document anyway, so that a stranger can establish it is malformed).
    /// [`Held::receipt_mark`] is what separates the first from the second.
    #[must_use]
    pub fn view(&self) -> Option<&serde_json::Value> {
        let view = self.receipt.body.as_ref()?.get(RECEIPT_VIEW_KEY)?;
        (!view.is_null()).then_some(view)
    }
}

/// A socket that has answered its status line and its headers, handed over before the body has been
/// read.
///
/// 🔴 This type exists so that the subscription (`super::live`) and the four reads can share
/// **one** writer of a request. The alternative was a second place in this face that composes an
/// HTTP request, and the claim `super`'s documentation makes — that the only method this face can
/// put on a socket is `GET` — is worth exactly as much as the number of places that could put a
/// different one there.
#[derive(Debug)]
pub struct Opened {
    /// The status line's code.
    pub status: u16,
    /// The socket, positioned at the first body byte that has not been handed over.
    pub socket: TcpStream,
    /// Body bytes that arrived in the same read as the headers. A streaming body's first events are
    /// routinely in here, and a reader that started at the next `read` would lose them.
    pub head_body: Vec<u8>,
    /// Whether the response declared `transfer-encoding: chunked`.
    ///
    /// 🔴 Read off the headers rather than assumed. Measured against the engine on `:8842`: the four
    /// list routes answer with `content-length` and the stream answers `chunked`, so a face that
    /// hard-coded either one would be right about half of its own reads.
    pub chunked: bool,
}

/// Connect, write the request, and read as far as the end of the headers.
///
/// The read timeout is the caller's: a list read wants to give up on a server that has stopped
/// talking, and a subscription wants a short window it can wake up in without concluding anything
/// (`super::live::pulse`).
fn open(
    base_url: &str,
    path: &str,
    token: Option<&str>,
    read_timeout: Duration,
) -> Result<Opened, String> {
    let rest = base_url
        .strip_prefix("http://")
        .ok_or_else(|| format!("only http:// is supported; {base_url} is not"))?;
    let authority = rest.split('/').next().unwrap_or(rest);
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (
            host,
            port.parse::<u16>()
                .map_err(|_| format!("{authority} has no port number"))?,
        ),
        None => (authority, 80u16),
    };
    let address = (host, port)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or_else(|| format!("{authority} resolves to no address"))?;
    let mut socket =
        TcpStream::connect_timeout(&address, CONNECT_TIMEOUT).map_err(|e| e.to_string())?;
    socket.set_read_timeout(Some(read_timeout)).ok();
    socket.set_write_timeout(Some(CONNECT_TIMEOUT)).ok();

    // 🔴 **The second layer of [`addressable`], and it is here because this is the one function
    // that writes a request line.** Until `RECORD_ROUTES` every path through here was a
    // `&'static str` declared in this file, so "the only method this module can write is `GET`" was
    // a property of the source. A path built from a value the engine sent makes it a property of
    // whoever built the path — unless the writer itself refuses. A space ends the request target
    // and a control character ends the line, so either one turns one request into two.
    if !path.starts_with('/') || path.chars().any(|c| c.is_control() || c == ' ') {
        return Err(format!(
            "this face will not put {path:?} in a request line: a request target with a space or a \
             control character in it is a second request"
        ));
    }
    // 🔴 The only method this module can write. There is no branch here and no parameter for it.
    let mut request = String::from("GET ");
    request.push_str(path);
    request.push_str(" HTTP/1.1\r\nHost: ");
    request.push_str(authority);
    // 🔴 `Connection: close` is kept for the stream as well, measured rather than reasoned about:
    // asked this way the engine still answers `200`, still sends `transfer-encoding: chunked`, still
    // replays its history and still holds the connection open. So the header costs nothing and the
    // request stays one string with no branch in it.
    request.push_str("\r\nAccept: application/json\r\nUser-Agent: gx-tui\r\nConnection: close\r\n");
    if let Some(token) = token {
        request.push_str("Authorization: Bearer ");
        request.push_str(token);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    socket
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;

    // Read until the header terminator arrives. Bounded, because a server that never sends one is
    // otherwise a server that fills this process's memory.
    let mut raw: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 4096];
    let head_end = loop {
        if let Some(end) = find(&raw, b"\r\n\r\n") {
            break end;
        }
        if raw.len() > MAX_HEAD_BYTES {
            return Err("the response headers do not end".to_string());
        }
        let read = socket.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            return Err("the connection ended before the headers did".to_string());
        }
        raw.extend_from_slice(&buffer[..read]);
    };
    let head = String::from_utf8_lossy(&raw[..head_end]).to_string();
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| "no status line".to_string())?;
    let lowered = head.to_lowercase();
    let chunked = lowered
        .lines()
        .any(|line| line.starts_with("transfer-encoding:") && line.contains("chunked"));
    Ok(Opened {
        status,
        socket,
        head_body: raw[head_end + 4..].to_vec(),
        chunked,
    })
}

/// HTTP/1.1 over a loopback socket, closing the connection so the body ends at end-of-file.
fn send(base_url: &str, path: &str, token: Option<&str>) -> Result<(u16, Vec<u8>), String> {
    let mut opened = open(base_url, path, token, READ_TIMEOUT)?;
    let mut raw = std::mem::take(&mut opened.head_body);
    opened
        .socket
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    Ok((opened.status, raw))
}

/// Open a route that does not end, and hand the socket to the caller.
///
/// 🔴 The one road out of this module for a long-lived read. Everything after the headers —
/// un-chunking, finding line boundaries, deciding that a timeout is not a disconnection — belongs to
/// `super::live`, because none of it is about sockets and all of it is about what an event means.
///
/// # Errors
/// The same string-shaped failures [`read_route`] folds into a [`Reading`]: an unreachable host, a
/// refused connection, headers that never end.
pub fn open_stream(
    base_url: &str,
    path: &str,
    token: Option<&str>,
    read_timeout: Duration,
) -> Result<Opened, String> {
    open(base_url, path, token, read_timeout)
}

/// The read time, RFC 3339 in UTC to nanosecond precision, from a nanosecond epoch.
///
/// 🔴 **Why this face spells the date itself** (`req/38` SS965 convert row (a)). The first build
/// called `gx_api::rfc3339::of` here, and that one call was the whole of the difference between
/// "this face reads a server over HTTP" and "this face is linked into the engine's API crate". The
/// membrane `req/942` §2 declares is a claim about **what this directory can reach**, and a claim
/// that holds except for one date formatter is a claim with an exception in it — the shape of every
/// crack that later turns out to have carried something.
///
/// The cost is a second implementation of one format, and the honest response to that cost is to
/// measure it rather than to promise it: `tui/tests/r942_tui.rs` runs this function and
/// the API crate's over the same instants — the epoch, before it, both sides of a leap day, and the
/// ends of the range — and requires the two strings to be equal. A divergence is a red test rather
/// than a wrong timestamp on a screen.
///
/// Nine fractional digits and `Z`, which is what the other side spells. Nanoseconds are not rounded
/// away for `gx-api`'s reason: two reads inside one second are two reads.
#[must_use]
pub fn rfc3339(nanos: i64) -> String {
    const PER_SECOND: i64 = 1_000_000_000;
    const PER_DAY: i64 = 86_400;
    // Euclidean, so that an instant before 1970 lands on the day it belongs to rather than
    // truncating towards zero and reporting the day after.
    let seconds = nanos.div_euclid(PER_SECOND);
    let fraction = nanos.rem_euclid(PER_SECOND);
    let (year, month, day) = civil(seconds.div_euclid(PER_DAY));
    let clock = seconds.rem_euclid(PER_DAY);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{fraction:09}Z",
        clock / 3600,
        (clock / 60) % 60,
        clock % 60
    )
}

/// The civil date a count of days since 1970-01-01 falls on.
///
/// The shift-the-year-to-March algorithm: with March as month zero the leap day is the last day of
/// the year, so the month lengths become a linear sequence and the whole conversion is arithmetic
/// with no table and no branch per month. Every quantity below is non-negative by construction, so
/// the plain divisions are exact.
///
/// The range that matters here is the one an `i64` nanosecond epoch can carry — roughly 1677 to
/// 2262 — and the conversion is correct far outside it.
fn civil(days: i64) -> (i64, i64, i64) {
    // 1970-01-01 measured from 0000-03-01, the start of the 400-year cycle this arithmetic uses.
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let march_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * march_month + 2) / 5 + 1;
    let month = if march_month < 10 {
        march_month + 3
    } else {
        march_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The kinds of nothing (`req/942` §12-1, plus `req/38` SS974's seventh), each with its own mark.
///
/// 🔴 The marks carry the meaning; colour never does. That is what makes the mono capability tier a
/// full-strength tier rather than a degradation, and it is also what makes the marks immune to the
/// tofu failure the browser face measured (`req/932` §9-5): every one of them is inside
/// `U+0020..=U+007E`, which no terminal font is missing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nothing {
    /// Not measured yet.
    Loading,
    /// Measured, and the answer was not knowable.
    Unknown,
    /// The wire did not carry the key at all.
    Absent,
    /// The wire carried the key, and the answer is no.
    False,
    /// Measured, and the count is nought.
    Zero,
    /// It was there, and it was struck out.
    Deleted,
    /// The wire carried the key, and what it carried has no characters in it.
    ///
    /// 🔴 **The seventh word, and the ruling that added it** (`req/38` SS974, queue row Q4). It was
    /// [`Nothing::Zero`] until then: `""` and `0` were drawn with the same mark, so a name the
    /// engine carried as an empty string read on screen as *a count of nought*. That is the same
    /// collapse the other six exist to refuse, committed inside the classifier that refuses it —
    /// an empty string is not a measurement, it is a value that says nothing, and the two are only
    /// the same fact if you are not looking.
    Empty,
}

/// The three words for nothing that mean **no answer was obtained**.
///
/// 🔴 **Three, and it is the ruling's own enumeration.** `req/924` §TUI-45 writes the rule as
/// 「列の全値が『無』の mark(`?` `--` `...`)なら、その列を落として開示に数えろ」 — [`Nothing::Loading`],
/// [`Nothing::Unknown`], [`Nothing::Absent`], and no others.
///
/// 🔴 **The other four are measured answers and dropping them would be the breach this product
/// exists to refuse.** `no` is the wire saying *false*; `0` is a count that was taken; `-x` is a
/// record that was written and struck; `''` is a value that arrived and is empty. A ledger in which
/// every record answers `enforced no` is a ledger making the most actionable statement a face like
/// this can carry, and a rule written to save ink would have deleted the column and told the reader
/// it "answered with a mark for nothing" — **folding *measured false* into *not measured*, in the
/// product whose first principle is that those are different**. An independent audit of this lane
/// found the first cut doing exactly that, over [`Nothing::ALL`].
///
/// The distinction is the same one [`Reading::nothing`] keeps: a read that has not happened is not
/// a read that failed, and neither is an answer.
pub const VACANT_MARKS: [Nothing; 3] = [Nothing::Loading, Nothing::Unknown, Nothing::Absent];

/// Whether a resolved cell's text is one of the marks that mean no answer was obtained.
///
/// 🔴 Derived from [`VACANT_MARKS`] rather than typed, so the set is edited in one place and the
/// argument for its size is written beside it rather than here.
///
/// It takes the **text** because the question is asked of a column of cells already resolved to
/// what the screen would draw (`super::renderer::vacant_columns` reads
/// `super::renderer::cell_mark`). 🔴 The audit's finding 5 stands against that choice — `cell_mark`
/// holds the discriminator and throws it away — and the repair taken is the other one it asked
/// for: the **mark is carried** rather than reduced to a bool, so what the column said is on the
/// screen. A `Cell`-typed classifier is the deeper repair and is **not** made here, because
/// `hoist`, `resolve_shared` and this function all read one column of resolved text and changing
/// that type is a change to the seam rather than to this rule.
#[must_use]
pub fn is_vacant_mark(text: &str) -> bool {
    VACANT_MARKS.iter().any(|nothing| nothing.mark() == text)
}

impl Nothing {
    /// All seven, in the order `req/942` §12-1 lists them, with `req/38` SS974's addition last.
    ///
    /// 🔴 The count is read from this array everywhere it is used, so a word added here grows every
    /// gate that sweeps the vocabulary rather than leaving one of them measuring six.
    pub const ALL: [Nothing; 7] = [
        Nothing::Loading,
        Nothing::Unknown,
        Nothing::Absent,
        Nothing::False,
        Nothing::Zero,
        Nothing::Deleted,
        Nothing::Empty,
    ];

    /// What is drawn in the cell.
    #[must_use]
    pub fn mark(self) -> &'static str {
        match self {
            // Something is still coming.
            Nothing::Loading => "...",
            // A question that was asked and not answered.
            Nothing::Unknown => "?",
            // A line where nothing was ever written.
            Nothing::Absent => "--",
            // The answer, in the word for it.
            Nothing::False => "no",
            // The count.
            Nothing::Zero => "0",
            // A line that was written and struck.
            Nothing::Deleted => "-x",
            // An empty quotation: the wire opened a value and closed it with nothing between.
            Nothing::Empty => "''",
        }
    }

    /// The paint role this mark is drawn in.
    ///
    /// 🔴 A mark had no declared appearance at all before `req/38` SS965 convert row (b): it was
    /// drawn raw, which looks exactly like "no colour, by decision" and is a different fact. Now the
    /// decision is spelled — most of these resolve to an emphasis and no hue, and none of them needs
    /// a hue to be told from another, which is what `P2` measures on `mono`.
    #[must_use]
    pub fn role(self) -> super::tokens::Role {
        use super::tokens::Role;
        match self {
            Nothing::Loading => Role::MarkLoading,
            Nothing::Unknown => Role::MarkUnknown,
            Nothing::Absent => Role::MarkAbsent,
            Nothing::False => Role::MarkFalse,
            Nothing::Zero => Role::MarkZero,
            Nothing::Deleted => Role::MarkDeleted,
            Nothing::Empty => Role::MarkEmpty,
        }
    }

    /// The word `req/942` §12-1 uses, for the legend and for the coverage grid.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Nothing::Loading => "loading",
            Nothing::Unknown => "unknown",
            Nothing::Absent => "absent",
            Nothing::False => "false",
            Nothing::Zero => "zero",
            Nothing::Deleted => "deleted",
            Nothing::Empty => "empty",
        }
    }
}

/// Whether a cell is filled, declared empty, or out of reach — the three values `req/942` §11-3
/// forbids collapsing into two.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Coverage {
    /// This face can produce the mark from the four routes it reads.
    Reachable,
    /// The mark exists in the vocabulary and this denominator cannot produce it.
    Unreachable(&'static str),
}

/// The coverage grid for the seven words, declared rather than inferred.
///
/// 🔴 `deleted` is the honest gap. Nothing on these four routes says a row was removed —
/// `superseded_by` names a replacement, which is a different fact — so the word is declared and
/// its cell is marked out of reach with the reason. `req/942` §12-1: "quietly settling for four is
/// the worst outcome". Settling for five and saying so is not the same act.
pub const NOTHING_COVERAGE: [(Nothing, Coverage); 7] = [
    (Nothing::Loading, Coverage::Reachable),
    (Nothing::Unknown, Coverage::Reachable),
    (Nothing::Absent, Coverage::Reachable),
    (Nothing::False, Coverage::Reachable),
    (Nothing::Zero, Coverage::Reachable),
    (
        Nothing::Deleted,
        Coverage::Unreachable(
            "no route among the four reports a removed row; superseded_by names a replacement",
        ),
    ),
    // 🔴 Reachable, and reachable is a claim: `crates/gx-api/src/list.rs` puts the wire's string
    // fields on these routes without a rule that forbids an empty one, so this face can meet `""`
    // on any of them. It met it as `0` until `req/38` SS974.
    (Nothing::Empty, Coverage::Reachable),
];

/// What one key of one row says: a value, or one of the seven kinds of nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cell {
    /// The wire carried something to draw.
    Value(String),
    /// The wire carried one of the seven absences.
    Nothing(Nothing),
}

impl Cell {
    /// What the cell puts in the buffer.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Cell::Value(text) => text.clone(),
            Cell::Nothing(nothing) => nothing.mark().to_string(),
        }
    }
}

/// The wire's key for the engine's decision.
pub const VERDICT_KEY: &str = "verdict";

/// The three kinds the engine spells, in its own spelling.
///
/// 🔴 Read out of `crates/gx-gate/src/verdict.rs` — "three arms and no fourth" — and repeated here
/// as **strings** rather than reached for as a type: naming the gate's enum in this directory would
/// put the engine's crates back inside the membrane for the sake of three words. That the three
/// words still match is a fact this face cannot check from the outside, and it does not pretend to:
/// a fourth word arriving on the wire is drawn as it arrived (see [`VerdictMark::Other`]).
pub const VERDICT_KINDS: [&str; 3] = ["Admit", "Deny", "Escalate"];

/// What the verdict column draws: one of three, the fourth mark, or a word this face does not know.
///
/// 🔴 **The fourth mark is not a fourth verdict** (`req/942`, and `req/38` SS965 convert row (d)).
/// It says the wire carried no verdict for this row, and it says **which kind of nothing** that was
/// — a key that was never carried and a key carried as `null` are two different facts, and rounding
/// them into one another would be the same collapse as rounding either of them into `Deny`.
///
/// This is the sentence the module documentation of `super` has made since the first build. It was
/// **false** until this type existed: the column drew whatever string arrived, so a fourth word
/// would have been drawn as a verdict and no declaration said what the three were.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerdictMark {
    /// One of [`VERDICT_KINDS`], spelled the engine's way.
    Kind(&'static str),
    /// No verdict on the wire, in the kind of nothing that was there instead.
    None(Nothing),
    /// A word this face's vocabulary does not hold. Drawn as it arrived: an engine that grows a
    /// fourth kind should make this face look out of date, not make it lie.
    Other(String),
}

impl VerdictMark {
    /// What is drawn in the cell.
    #[must_use]
    pub fn mark(&self) -> String {
        match self {
            VerdictMark::Kind(kind) => (*kind).to_string(),
            VerdictMark::None(nothing) => nothing.mark().to_string(),
            VerdictMark::Other(text) => text.clone(),
        }
    }

    /// The paint role it is drawn in.
    #[must_use]
    pub fn role(&self) -> super::tokens::Role {
        use super::tokens::Role;
        match self {
            VerdictMark::Kind("Admit") => Role::VerdictAdmit,
            VerdictMark::Kind("Deny") => Role::VerdictDeny,
            VerdictMark::Kind("Escalate") => Role::VerdictEscalate,
            // An unrecognised word is not given one of the three appearances either. Colour is
            // decoration in this face and it is still a claim.
            VerdictMark::Kind(_) | VerdictMark::Other(_) => Role::Body,
            VerdictMark::None(_) => Role::VerdictNone,
        }
    }

    /// Whether this is one of the three the engine spells.
    #[must_use]
    pub fn is_kind(&self) -> bool {
        matches!(self, VerdictMark::Kind(_))
    }
}

/// Classify one row's verdict.
///
/// Built on [`cell`] rather than beside it, so the seven words keep their one classifier: the fourth
/// mark **is** whichever kind of nothing the row carried.
#[must_use]
pub fn verdict(object: &serde_json::Value) -> VerdictMark {
    match cell(object, VERDICT_KEY) {
        Cell::Nothing(nothing) => VerdictMark::None(nothing),
        Cell::Value(text) => VERDICT_KINDS
            .into_iter()
            .find(|kind| *kind == text)
            .map_or(VerdictMark::Other(text), VerdictMark::Kind),
    }
}

/// The wire's key for what became of the escrowed inverse.
pub const INVERSE_STATUS_KEY: &str = "inverse_status";

/// The six words the engine spells as bare strings, in the order `crates/gx-engine/src/store.rs`
/// declares them, minus the one that is not a bare string.
///
/// 🔴 Read out of `InverseStatus` and repeated here as **strings**, for the reason
/// [`VERDICT_KINDS`] is: naming the engine's enum in this directory would put its crates back
/// inside the membrane for the sake of six words. `Consumed` is deliberately absent from this
/// array — it is the one variant carrying a member, so it arrives as an object and never as one of
/// these ([`InverseMark::Consumed`]).
///
/// 🔴 `Expired` is in the array and **has no writer**: `store.rs` says so in its own words
/// ("DR-9 puts enforcement of the deadline in the commercial tier"), and `lifecycle_transitions.rs`
/// asserts the absence. A word this face can draw and this engine does not yet send is not the same
/// fact as a word that will never come, so it stays in the vocabulary and the disclosure carries
/// the difference rather than the array dropping it.
pub const INVERSE_KINDS: [&str; 6] = [
    "Available",
    "Expired",
    "Unavailable",
    "Pending",
    "BodyMissing",
    "Undetermined",
];

/// The tag the one variant with a member arrives under.
pub const CONSUMED_KIND: &str = "Consumed";
/// The member it carries: which transformation used the inverse up.
pub const CONSUMED_BY_KEY: &str = "by";

/// What the `inverse_status` column draws: one of the six words, the seventh with its member, or
/// one of the three kinds of nothing that can stand in for all of them.
///
/// 🔴 **Ten states, and the reason they are ten** (`req/924` §TUI-13 追記). Two of them belong to
/// the reading rather than to the row — not measured yet, and measured and not knowable — and eight
/// are shapes the wire can carry: `null` and the seven variants. Collapsing any pair of the ten is
/// the same breach in the small that this product refuses in the large, and one pair in particular
/// was collapsed here until this type existed (see [`inverse_status`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InverseMark {
    /// No word arrived, in the kind of nothing that was there instead.
    Nothing(Nothing),
    /// One of [`INVERSE_KINDS`], spelled the engine's way.
    Kind(&'static str),
    /// The inverse was used up, and by which transformation.
    Consumed {
        /// The `by` member, whole. The reason this variant exists rather than falling through to
        /// [`InverseMark::Other`]: an object drawn by [`cell`] becomes the JSON text of itself, and
        /// a cell fourteen wide cuts that to `{"Consumed":{~` — which spends the whole column on
        /// punctuation and loses the only fact the object was carrying.
        by: String,
    },
    /// A word this face's vocabulary does not hold. Drawn as it arrived, for the reason
    /// [`VerdictMark::Other`] is: an engine that grows an eighth word should make this face look
    /// out of date, not make it lie.
    Other(String),
}

impl InverseMark {
    /// What is drawn in the cell.
    ///
    /// 🔴 `Consumed` is spelled as **two words** rather than as its serialisation. A cell narrower
    /// than the whole of it is cut by the same rule and marked with the same character as the id
    /// column, which is already cut at every width this face draws; what a reader must never see is
    /// a column that says `Consumed` and hides that a transformation is named behind it.
    #[must_use]
    pub fn mark(&self) -> String {
        match self {
            InverseMark::Nothing(nothing) => nothing.mark().to_string(),
            InverseMark::Kind(kind) => (*kind).to_string(),
            InverseMark::Consumed { by } => format!("{CONSUMED_KIND} {by}"),
            InverseMark::Other(text) => text.clone(),
        }
    }

    /// The paint role it is drawn in.
    ///
    /// 🔴 The seven words for a value all resolve to [`super::tokens::Role::Body`], and that is a
    /// decision rather than an omission: `Unavailable`, `BodyMissing` and `Undetermined` read as
    /// bad news and a hue would say so, but the ten states are told apart by their spelling on
    /// `mono`, where there is no hue at all. A meaning carried by colour is a meaning one tier
    /// loses.
    #[must_use]
    pub fn role(&self) -> super::tokens::Role {
        match self {
            InverseMark::Nothing(nothing) => nothing.role(),
            InverseMark::Kind(_) | InverseMark::Consumed { .. } | InverseMark::Other(_) => {
                super::tokens::Role::Body
            }
        }
    }
}

/// Classify one row's `inverse_status`.
///
/// 🔴 **The one key on these four routes where `null` is not [`Nothing::Unknown`].** [`cell`]'s
/// general rule is right for every other key it is asked about and wrong for this one, and the
/// engine says so in its own source: `crates/gx-api/src/list.rs` writes `null` here for "a
/// transformation with **no escrow row at all**", and names the confusion it is avoiding —
/// `InverseStatus::Unavailable` already means "`invert()` answered `None`". So the wire carries
/// three different facts that a careless face draws as one:
///
/// * `null` — **there is no escrow row**. Nobody to ask. [`Nothing::Absent`], whose mark is
///   documented as "a line where nothing was ever written".
/// * `"Unavailable"` — asked, and the adapter built no inverse. A **value**, spelled as the word.
/// * a reading that failed — [`Nothing::Unknown`], and it is [`Reading::nothing`] that produces it,
///   one layer above this function.
///
/// Before this function, the first of those three was drawn `?` — the same mark as the third. A
/// reader could not tell "this transformation never escrowed anything" from "this face could not
/// read the list", which is the collapse the seven words exist to refuse, committed on the one
/// column whose subject is whether an undo is still possible.
///
/// 🔴 **Declared and not distinguished**: a key the object does not carry at all also draws
/// [`Nothing::Absent`] here. A server older than `M6H6-15` sends no `inverse_status` member, and
/// this face spells that the same way it spells `null`. The two are different facts — *this server
/// does not have the field* and *this row has no escrow* — and nothing on this screen tells them
/// apart. It is written down rather than hidden, and the road to telling them apart is
/// `GET /v1/healthz`'s `engine_version`, which this face already draws.
///
/// 🔴 **Declared and not distinguished, the second** (`req/924` §TUI-97, SS1136, 2026-09-02;
/// measured on the Owner's running engine by `[T-r76]`): the `null` of a `Candidate` and the `null`
/// of an `{"Aborted":"Expired"}` are **not the same nothing** — one is *not yet* and one is
/// *never* — and this face draws both `--`. See [`INVERSE_NULL_STATES`] for the ruling and for why
/// no eighth mark is minted here.
///
/// 🔴 The road for this one is **better than the first's and it is on the same row**: `state`
/// separates the two and is drawn beside this cell. That is what makes this a collapse a reader can
/// undo rather than a fact the screen destroys — and it is why `g97` measures whether `state` is
/// still on the row at every width, rather than this comment asserting that it is.
#[must_use]
pub fn inverse_status(object: &serde_json::Value) -> InverseMark {
    let Some(value) = object.get(INVERSE_STATUS_KEY) else {
        return InverseMark::Nothing(Nothing::Absent);
    };
    if value.is_null() {
        return InverseMark::Nothing(Nothing::Absent);
    }
    if let serde_json::Value::Object(map) = value {
        if let Some(consumed) = map.get(CONSUMED_KIND) {
            return match consumed.get(CONSUMED_BY_KEY).and_then(|by| by.as_str()) {
                Some(by) if !by.is_empty() => InverseMark::Consumed { by: by.to_string() },
                // The tag arrived and the member did not. Drawn as it arrived rather than as a bare
                // `Consumed`: a face that quietly spells the tag alone would be reporting the shape
                // it expected instead of the shape it was sent.
                _ => InverseMark::Other(value.to_string()),
            };
        }
    }
    match cell(object, INVERSE_STATUS_KEY) {
        Cell::Nothing(nothing) => InverseMark::Nothing(nothing),
        Cell::Value(text) => INVERSE_KINDS
            .into_iter()
            .find(|kind| *kind == text)
            .map_or(InverseMark::Other(text), InverseMark::Kind),
    }
}

/// The wire's key for where the transformation stands now.
pub const STATE_KEY: &str = "state";

/// The two states whose `inverse_status` is `null` for **different reasons**.
///
/// 🔴 `req/924` §TUI-97 (SS1136, 2026-09-02), measured on the Owner's running engine: of thirty-two
/// rows, thirty carry `inverse_status "Available"` and two carry `null` — and those two are exactly
/// one `Candidate` and one `{"Aborted":"Expired"}`. The wire flattens both to `null`; the ruling is
/// that they are **two kinds of nothing**:
///
/// * `Candidate` — **not yet**. Nothing has been escrowed because nothing has been planned. Time
///   will change this.
/// * `Aborted` — **never**. The transformation did not commit, so there is nothing to invert and
///   nothing ever will be.
///
/// 🔴 **This face draws both as `--` and that is a collapse, declared here rather than hidden.**
/// [`inverse_status`]'s "Declared and not distinguished" paragraph carries the whole argument; what
/// this array adds is that the collapse is **recoverable on the same row**: `state` is 32/32
/// non-null on that engine and separates the two, so a reader looking at the row can tell them
/// apart even though the cell cannot. `g97` is what holds the road open — a width at which the row
/// is drawn and `state` is not, without the disclosure counting `state`, is the collapse becoming
/// silent.
///
/// 🔴 Spelling a new mark for *not yet* is deliberately **not** done here: `req/924` §TUI-48 ruled
/// that the vocabulary of nothing is closed at seven and that any eighth mark's spelling goes
/// through the Owner (見本 → Owner 裁定 → 量産). A lane inventing one is the failure that ruling was
/// written about.
/// 🔴 **Derived, not typed a second time** (`[T-r82]`, 2026-09-02, closing the audit's D4-③).
/// `[T-r76]` hand-wrote `["Candidate", "Aborted"]` here **and attached no freshness gate to it** —
/// in the same lane whose own doc comment warned that a hand-made list of lifecycle states goes
/// stale in silence. The names now come off [`NULL_MEANING_BY_STATE`], which `g98` checks against
/// the engine's own `LIFECYCLE_STATES` at test time, so there is one list and it is the checked one.
pub const INVERSE_NULL_STATES: [&str; 2] = [NULL_MEANING_BY_STATE[0].0, NULL_MEANING_BY_STATE[1].0];

/// The lifecycle states from which a `null` on a **judged** field can be read, and what it means.
///
/// # 🔴 The ruling this table is (`req/924` §TUI-101, SS1145, 2026-09-02)
///
/// Measured on the Owner's running engine and read off the seat's own inspection of the captures,
/// two rows carry `verdict: null` **and** `inverse_status: null`, and the wire's `state` tells them
/// apart 32/32. `[T-r76]` repaired the one cell it had measured and left the other three:
///
/// | row | field | what the wire means | drawn before |
/// |---|---|---|---|
/// | `Aborted Expired` | `inverse_status` | never committed, so nothing was ever escrowed | `--` ✓ |
/// | `Aborted Expired` | `verdict` | terminal, so no verdict was written and none ever will be | `?` ✗ |
/// | `Candidate` | `inverse_status` | **not yet**: T-3 has not run | `--` ✗ |
/// | `Candidate` | `verdict` | **not yet**: T-3 has not run | `?` ✗ |
///
/// Three of four were wrong, and two of them in the direction `INHERITED_PRINCIPLES`' *nothing
/// vertical* puts first: 🔴 **a "not yet" that comes from time was being drawn as a semantic
/// "never".** The other pair — a `Candidate` and a terminal `Aborted` — were receiving the
/// **identical symbol pair** `? / --`, which is the collapse §TUI-97 ruled must be visible.
///
/// # 🔴 The symbol is read from `(state, field)`, not from `(field, wire value)`
///
/// This is what makes the defect closeable **on this side of the membrane**: the wire flattens both
/// rows to `null`, but `state` is non-null on every row of that bed and separates them, so the face
/// has the fact and was throwing it away. §TUI-97's own ruling is that the collapse is recoverable
/// on the same row; this table is that recovery taken rather than described.
///
/// # 🔴 No eighth mark is minted, and that is a hard boundary
///
/// `req/924` §TUI-48 (SS1079) reserves the spelling of an eighth kind of nothing to the Owner
/// (見本 → Owner 裁定 → 量産). Both words used here are **already in [`Nothing::ALL`]**:
///
/// * [`Nothing::Loading`] (`...`) — *not measured yet; something is still coming*. This is the
///   existing word for the *not yet* family, and re-seating an existing word is what §TUI-48 asks
///   for in place of minting.
/// * [`Nothing::Absent`] (`--`) — *a line where nothing was ever written*, which is what a terminal
///   abort leaves behind and what [`inverse_status`] already resolves a `null` to.
///
/// 🔴 **And the existing ruling that a `null` and an `Unavailable` may not share a spelling is
/// untouched**: `Unavailable` is a word the wire carried and is drawn as [`InverseMark::Kind`],
/// never as a mark. This table only ever replaces one mark for nothing with another.
///
/// # 🔴 Two states and no more, and the rest is declared rather than assumed
///
/// The eleven states are not all rulings waiting to be written. A `null` is read through this table
/// only where the lifecycle **entails** the reading:
///
/// * `Candidate` — 43 T-2 has run and T-3 has not. Whatever T-3 and T-4 will write is *not yet*
///   written, and time changes it.
/// * `Aborted` — terminal, carrying its `AbortReason`. Nothing further is written, ever.
///
/// Every other state is left to the classifier that was already there, because a `null` there is a
/// fact about the **engine** and not about the lifecycle — a `Committed` row with no `verdict` is a
/// gap, and drawing it as *not yet* would be this face inventing a reason. `Draft` is in the
/// vocabulary and never in the table (`pipeline.rs` says so of itself), so it cannot reach a row.
/// `Verifying` is arguably *not yet* as well and is **deliberately left out**: no row of the
/// measured bed was in it, so a ruling here would be one this lane could not fire.
///
/// The remainder is not silence — `g98` prints every state it found no ruling for.
pub const NULL_MEANING_BY_STATE: [(&str, Nothing); 2] = [
    ("Candidate", Nothing::Loading),
    ("Aborted", Nothing::Absent),
];

/// How many states `crates/gx-engine/src/pipeline.rs` declared when [`NULL_MEANING_BY_STATE`] was
/// ruled on.
///
/// 🔴 **A tripwire, not a source of truth.** The list itself is read out of the engine's source by
/// `g98` at test time and never copied here; this one number is what makes a *twelfth* state red
/// instead of silently unruled. `[T-r76]`'s array had no such wire at all, which is the whole of
/// the audit's D4-③.
pub const LIFECYCLE_STATE_COUNT_AT_RULING: usize = 11;

/// The fields whose `null` [`NULL_MEANING_BY_STATE`] is allowed to speak for.
///
/// 🔴 **Two, and the boundary is the point.** On the measured bed `created_at`, `scope`, `actor`,
/// `rollback` and `superseded_by` are `null` on **all thirty-two rows regardless of state** — that
/// is an engine gap, not a lifecycle fact, and reading it through this table would have the face
/// telling a reader that a `Committed` record's `created_at` is *still coming*. These two are the
/// judged fields: they are the ones 43's transitions write, so they are the ones the state entails.
pub const NULL_MEANING_FIELDS: [&str; 2] = [VERDICT_KEY, INVERSE_STATUS_KEY];

/// What kind of nothing this row's `state` says a missing judged field is, if it says anything.
///
/// `None` means *this face has no ruling for this state* and the general classifier stands.
///
/// 🔴 Keyed on [`StateMark`]'s **tag**, so `{"Aborted":"Expired"}` and a bare `"Aborted"` are the
/// same row to this function. The member is what the state column draws; the reason an abort
/// happened does not change that nothing further will be written.
#[must_use]
pub fn null_meaning(object: &serde_json::Value) -> Option<Nothing> {
    let tag = match state(object) {
        StateMark::Kind(text) => text,
        StateMark::Compound { kind, .. } => kind,
        StateMark::Nothing(_) | StateMark::Other(_) => return None,
    };
    NULL_MEANING_BY_STATE
        .iter()
        .find(|(name, _)| *name == tag)
        .map(|(_, nothing)| *nothing)
}

/// What the `state` column draws: the engine's word, a tagged state with its member, or one of the
/// kinds of nothing.
///
/// 🔴 **`state` carries two wire types in one column** (`[T-r76]`, 2026-09-02, measured on the
/// Owner's engine): thirty-one rows spell it as a **string** (`"Committed"`, `"Candidate"`) and one
/// as a **single-key object** (`{"Aborted":"Expired"}`). [`cell`]'s general rule turns a non-empty
/// object into `value.to_string()`, so at a column thirteen cells wide the screen read
/// `{"Aborted":"~` — **the whole column spent on punctuation, and `Expired`, the only fact the
/// object was carrying, lost.**
///
/// That is verbatim the defect [`InverseMark::Consumed`] exists to prevent one column over, and it
/// is repaired the same way and with the same words: the tag and its member, spelled as **two
/// words**, so a cut takes the member and never hides that there was one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateMark {
    /// No word arrived, in the kind of nothing that was there instead.
    Nothing(Nothing),
    /// The engine spelled it as a bare string, and it is drawn as it arrived. The face declares no
    /// enumeration of the lifecycle: `wire.rs` does not name the engine's enum (see
    /// [`VERDICT_KINDS`] for the reason), so a hand-made list here would go stale in silence.
    ///
    /// # 🔴 RETRACTED, in the place the mistake is inherited from (`[T-r82]`, 2026-09-02)
    ///
    /// The sentence above is kept because it is what was written, and the half of it that is true
    /// is still true: **this crate** may not name the engine's enum, for the membrane reason
    /// [`VERDICT_KINDS`] gives. What `[T-r76]` said next, and raised as a finding, is **false**:
    ///
    /// > *`state`'s vocabulary has no declared enumeration, so a hand-written list goes stale in
    /// > silence.*
    ///
    /// It is declared, in three places, and a seat confirmed all three from the source rather than
    /// from a report:
    ///
    /// * `crates/gx-engine/src/pipeline.rs` — `pub enum Lifecycle`, eleven variants.
    /// * the same file — **`pub const LIFECYCLE_STATES: [&str; 11]`**, whose own doc says it is
    ///   declared once in 43 §1's order and that *a twelfth state added without a row is a compile
    ///   error at `Lifecycle::name` and a failing probe at the table*.
    /// * `crates/gx-engine/tests/lifecycle_states.rs`, which reads 43 §1's table out of the spec
    ///   file and compares it against both.
    ///
    /// `crates/gx-core/src/error.rs`'s `pub enum AbortReason` does the same for the member an
    /// `Aborted` carries, and it is closed on purpose (ASM-15).
    ///
    /// 🔴 **And the technique for reaching them was already in this lane's own suite.** `g95` reads
    /// `crates/gx-api/src/handlers.rs` **at test time** to derive its denominator, and prints
    /// `UNTESTABLE` where the file is not in the checkout. The membrane forbids a *dependency*, not
    /// a *test-time read*: one column over, the same lane had already shown the way and did not
    /// take it. What went stale in silence was not a list — it was the claim that no list existed.
    ///
    /// The freshness the retracted sentence said was impossible is now held by `g98`, over
    /// [`NULL_MEANING_BY_STATE`].
    Kind(String),
    /// A single-key object — the tag and what it carries.
    Compound {
        /// The one key of the object.
        kind: String,
        /// What it held, as a string.
        member: String,
    },
    /// A shape this face has no reading for, drawn as it arrived rather than guessed at.
    Other(String),
}

impl StateMark {
    /// What is drawn in the cell.
    #[must_use]
    pub fn mark(&self) -> String {
        match self {
            StateMark::Nothing(nothing) => nothing.mark().to_string(),
            StateMark::Kind(text) | StateMark::Other(text) => text.clone(),
            StateMark::Compound { kind, member } => format!("{kind} {member}"),
        }
    }

    /// The paint role it is drawn in — [`super::tokens::Role::Body`] for everything the wire
    /// carried, for the reason [`InverseMark::role`] gives: the states are told apart by their
    /// spelling on `mono`, where there is no hue at all.
    #[must_use]
    pub fn role(&self) -> super::tokens::Role {
        match self {
            StateMark::Nothing(nothing) => nothing.role(),
            _ => super::tokens::Role::Body,
        }
    }
}

/// Classify one row's `state`.
///
/// 🔴 **A single-key object is the only object shape read.** Two keys, or a key holding something
/// that is not a non-empty string, falls to [`StateMark::Other`] and is drawn as it arrived — a
/// face that folded an unread shape into `kind member` would be reporting the shape it expected
/// instead of the shape it was sent, which is the sentence [`inverse_status`] already makes about
/// a `Consumed` whose member did not come.
#[must_use]
pub fn state(object: &serde_json::Value) -> StateMark {
    let Some(value) = object.get(STATE_KEY) else {
        return StateMark::Nothing(Nothing::Absent);
    };
    if let serde_json::Value::Object(map) = value {
        if map.len() == 1 {
            if let Some((kind, held)) = map.iter().next() {
                if let Some(member) = held.as_str() {
                    if !member.is_empty() {
                        return StateMark::Compound {
                            kind: kind.clone(),
                            member: member.to_string(),
                        };
                    }
                }
            }
        }
        if !map.is_empty() {
            return StateMark::Other(value.to_string());
        }
    }
    match cell(object, STATE_KEY) {
        Cell::Nothing(nothing) => StateMark::Nothing(nothing),
        Cell::Value(text) => StateMark::Kind(text),
    }
}

/// The wire's key for the transformation that replaced this one.
///
/// 🔴 **Declared here and spelled in two other places, and that is measured rather than tidied.**
/// `super::layout::LEDGER_COLUMNS` draws a column under this name and
/// `super::renderer::AGREEMENT_KEYS` compares it; both are frozen tables this lane does not touch.
/// Gate `g94` asserts all three are the same string, so a rename becomes a red test instead of a
/// column that quietly stops agreeing with the row.
pub const SUPERSEDED_BY_KEY: &str = "superseded_by";

/// What the record draws on `superseded_by` when [`supersede_agrees`] holds.
///
/// 🔴 **Words, and not a symbol.** `INHERITED_PRINCIPLES` §3c-③'' lets a TUI face spend a symbol
/// only when a *word* disappears in exchange; nothing disappears here except a repeated address, so
/// no symbol is minted. Neither key loses its own word either: `inverse_status` keeps `Consumed`
/// one row above and `superseded_by` keeps its label on this one.
pub const SUPERSEDE_AGREEMENT: &str = "same as inverse_status";

/// Whether this row's `superseded_by` names the same transformation `inverse_status` already
/// named, so that drawing the address a second time would spell one fact twice.
///
/// # 🔴 The two are the same value by construction, and this is where that was checked
///
/// `[T-r76]`, 2026-09-02. The question asked first was not *are these equal on this bed* but *can
/// the engine ever make them differ* — a face that folded two coincidentally-equal facts into one
/// would be lying about the engine rather than economising on a row. Read out of the engine's live
/// source, not out of a capture:
///
/// * `crates/gx-engine/src/pipeline.rs`'s `supersede_after_commit` is 43 T-12, and its own comment
///   says "Three facts move together (**M5-16 adopted (a)**: one place) ... and this is the only
///   place any of them is written". The block writes, from **one binding** `*t_u`:
///   `self.supersedes.record(t_o, *t_u)`, `row.status = InverseStatus::Consumed { by: *t_u }` and
///   `entry.superseded_by = Some(*t_u)`.
/// * `crates/gx-engine/src/replay.rs`'s `EngineJournalRecord::Superseded` arm rebuilds both from
///   **one record field** `*by`: `row.superseded_by = Some(*by)` and
///   `held.status = InverseStatus::Consumed { by: *by }`.
/// * Those are the **only** writers. Grepping the engine's `src` for `superseded_by =`,
///   `.status = InverseStatus::` and `status: InverseStatus::` returns those four assignments plus
///   two constructions of `Available` / `Unavailable`, and no site writes `Consumed` alone.
/// * The two arrive on the wire through one spelling: `crates/gx-api/src/list.rs` asks
///   `engine.superseded_by(&id)` and `engine.inverse_status(&id)` about the **same** `id`, and
///   `gx_core::Cid`'s `Serialize` is `serialize_str(&self.to_text())` on a human-readable format,
///   which `gx-canon/tests/cid_text.rs` pins as the one implementation of `gx1:<base32>` in the
///   workspace.
///
/// So `inverse_status == Consumed { by: X }` **entails** `superseded_by == X`.
///
/// # 🔴 The converse does not hold, and the fold does not claim it
///
/// Both writers guard the escrow half (`if let Some(row) = …get_mut(…)`) and only the pipeline's
/// loop has already proved the row exists; a replay over a journal whose `Escrowed` record is
/// missing sets `superseded_by` and leaves `inverse_status` at `null`. That row draws its address
/// and its own mark, untouched — this function answers `false` there. The fold is one-directional
/// because the fact is.
///
/// # 🔴 Equality is asked of the drawn text, not of the parsed value
///
/// The comparison is `by` against [`cell`]'s `Value`, which is what the two rows would put in the
/// buffer. A row where the wire sent `superseded_by` as something other than a string, or as one of
/// the seven kinds of nothing, is not folded: this face may only remove a repetition it can see.
#[must_use]
pub fn supersede_agrees(object: &serde_json::Value) -> bool {
    let InverseMark::Consumed { by } = inverse_status(object) else {
        return false;
    };
    matches!(cell(object, SUPERSEDED_BY_KEY), Cell::Value(text) if text == by)
}

/// The wire's key for why `status` is not `ok`, on `/v1/healthz` and on `server_health`
/// (`GET /receipts/{tid}` -- 44 §2.2 `L-02`, the same two words either endpoint answers with).
pub const STATUS_REASON_KEY: &str = "status_reason";

/// Classify `status_reason`.
///
/// 🔴 **The second key on these routes where `null` is [`Nothing::Absent`], not
/// [`Nothing::Unknown`]** (`req/924` §TUI-23, SS1051 -- the ruling -- and §TUI-39, SS1069 -- the
/// repair). `crates/gx-api/src/handlers.rs`'s `healthz` and `server_health` write `null` for
/// exactly one fact: the engine is `ok` and has no reason to give. That is *never-written*, the
/// same shape [`inverse_status`] carves [`INVERSE_STATUS_KEY`] out of [`cell`]'s general rule for
/// -- and this key is carved out the same way, for the same reason: [`cell`]'s rule is right for
/// every route where `null` means *asked, and this process could not say* (`list.rs`, in
/// `gx-api`), and wrong for the one route that writes `null` to mean *asked, and there is nothing
/// to say*.
///
/// 🔴 **`tui/tests/r942_tui.rs`'s P9 found this and did not fix it.** It read this key through
/// [`cell`] directly, printed `null_reads_as=Unknown` beside `req/924_§TUI-23_says=Absent`, and left
/// the two sentences disagreeing on purpose — `req/38` SS856 is the reason given: repairing it
/// **inside** `cell` would change what `null` means on every route through it, not mirror existing
/// code. This function is that mirror: [`INVERSE_STATUS_KEY`] already proved the shape, so carrying
/// it to a second key is the cheap repair SS856 asked for, not the expensive one it refused.
///
/// 🔴 **Declared and not distinguished**, for the reason [`inverse_status`] gives for its own key: a
/// server built before `L-02`/R11 carries no `status_reason` member at all, and this face spells
/// that the same way it spells `null`. Both read [`Nothing::Absent`].
#[must_use]
pub fn status_reason(object: &serde_json::Value) -> Cell {
    let Some(value) = object.get(STATUS_REASON_KEY) else {
        return Cell::Nothing(Nothing::Absent);
    };
    if value.is_null() {
        return Cell::Nothing(Nothing::Absent);
    }
    cell(object, STATUS_REASON_KEY)
}

/// Classify one key of one JSON object.
///
/// 🔴 The two lines this function exists to hold:
/// * a key the object does not have is [`Nothing::Absent`]; a key it has with `null` is
///   [`Nothing::Unknown`]. On these routes `null` means "this process does not hold the body"
///   (`crates/gx-api/src/list.rs`), which is measured-and-unknowable and not never-written.
///   🔴 **Two exceptions**, each with a classifier of its own that does not reach this arm:
///   [`INVERSE_STATUS_KEY`] (see [`inverse_status`]) and, since `req/924` §TUI-39,
///   [`STATUS_REASON_KEY`] (see [`status_reason`]).
/// * `false` is [`Nothing::False`] and never [`Nothing::Unknown`]. Collapsing a three-valued answer
///   into two is the first-principle breach this product exists to refuse.
#[must_use]
pub fn cell(object: &serde_json::Value, key: &str) -> Cell {
    let Some(value) = object.get(key) else {
        return Cell::Nothing(Nothing::Absent);
    };
    match value {
        serde_json::Value::Null => Cell::Nothing(Nothing::Unknown),
        serde_json::Value::Bool(false) => Cell::Nothing(Nothing::False),
        serde_json::Value::Bool(true) => Cell::Value("yes".to_string()),
        serde_json::Value::Number(number) => {
            if number.as_f64() == Some(0.0) {
                Cell::Nothing(Nothing::Zero)
            } else {
                Cell::Value(number.to_string())
            }
        }
        // 🔴 **`req/38` SS974 queue row Q4.** This arm answered [`Nothing::Zero`] until that ruling,
        // so a `state` or an `actor` the engine carried as `""` was drawn as `0` and read as a count
        // of nought. An empty string is not a count of anything; it is a value that arrived and says
        // nothing, and it now has a word of its own.
        //
        // 🔴 **The range of the repair, and why it stops here.** The two arms below keep
        // [`Nothing::Zero`] for an empty array and an empty object, and that is a decision rather
        // than an oversight: `[]` **is** a count — nought items — and `{}` is nought members, so
        // for those two the mark is the right answer to the question the reader is asking. The
        // string is the one case where the container has no items to count. Saying where a repair
        // ends is part of making it.
        serde_json::Value::String(text) => {
            if text.is_empty() {
                Cell::Nothing(Nothing::Empty)
            } else {
                Cell::Value(text.clone())
            }
        }
        serde_json::Value::Array(items) => {
            if items.is_empty() {
                Cell::Nothing(Nothing::Zero)
            } else {
                Cell::Value(
                    items
                        .iter()
                        .map(|item| match item {
                            serde_json::Value::String(text) => text.clone(),
                            other => other.to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(","),
                )
            }
        }
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                Cell::Nothing(Nothing::Zero)
            } else {
                Cell::Value(value.to_string())
            }
        }
    }
}
