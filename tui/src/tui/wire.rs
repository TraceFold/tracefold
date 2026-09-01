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
pub const ROUTES: [&str; 4] = [
    "/v1/healthz",
    "/v1/transformations",
    "/v1/candidates",
    "/v1/escalations",
];

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
