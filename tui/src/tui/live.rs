// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The subscription: what the engine pushes, and the five states of being connected to it.
//!
//! # What an event is allowed to do
//!
//! **Nothing to a row.** An event is the sentence "go and look again", and the only thing this
//! module hands the rest of the face is [`Subscription::due`] — a boolean. The bodies the engine
//! pushes are counted and then dropped, so there is no path by which a row on the screen was
//! written by anything except one of the four reads in [`super::wire`].
//!
//! That is a design ruling and not an accident, and the reason is the one this whole product is
//! built on: an event stream that edits rows makes a **second source of truth**. The list would say
//! one thing, the accumulated events another, and nothing on the machine could say which was right.
//! With `reread` the engine stays the only thing that says what is true, and a subscription that
//! dies degrades into the face this build already had — a screen that is out of date and knows it,
//! rather than a screen that is out of date and looks fresh.
//!
//! The word is a constant ([`ON_EVENT`]) so that a later edit that reaches for `apply` has to
//! change a declaration a gate is reading, rather than a line inside a loop.
//!
//! # The five states, and why `closed` is the one that matters
//!
//! | state | is it an absence? | mark |
//! |---|---|---|
//! | [`Link::Off`] | the wire was never asked | `--` |
//! | [`Link::Opening`] | asked and not yet answered | `...` |
//! | [`Link::Open`] | **no** — being connected is not a kind of nothing | `<<` |
//! | [`Link::Never`] | asked at least once, and it has never once been up | `no` |
//! | [`Link::Closed`] | it has been up, and what is arriving now is not knowable | `?` |
//!
//! 🔴 **`never` is not `closed`** (`req/38` SS988). Until this repair there were four states and the
//! first failure to open produced `closed`, whose own definition in this table said *it was
//! connected* — a sentence about a connection that had never existed. **That sentence is the thing
//! that was false and it is retracted here, not the code that drew it.** The collapse was total and
//! not partial: [`LinkReport::long`] spells a closed connection `closed after {events} events,
//! {reconnects} reconnects`, so an engine that answered once, sent nothing and dropped printed
//! `closed after 0 events, 1 reconnects` — byte for byte what an engine that was never once up
//! printed. No width and no count told them apart. What made it look as though the counts did is
//! the accident that the engine on `:8842` replays its history on connect.
//!
//! 🔴 **`never` is not `zero` either**, for the reason [`Link::nothing`] gives about `closed`: `0
//! events` is a measurement and a process that was never listening has not made it. `never` is
//! [`super::wire::Nothing::False`] — *asked, and the answer is no* — which is the only one of the
//! seven words that is a measurement with a negative answer rather than an unmeasured quantity.
//! `absent` was the first reach and it is wrong: `absent` is already `off`'s mark, so giving it to
//! `never` would **merge a pair the old map separated** (`off` drew `--`, the never-case drew `?`),
//! and a classification change that merges any pair the old one separated is a trade and not a
//! repair. That is the standard `g20` holds [`super::wire::cell`] to, and gate `g22` holds this map
//! to it. **On this face a word is chosen with the partition and not with the sentence**, because a
//! sentence can be written to justify either choice.
//!
//! 🔴 **`closed` must never wear the mark for `zero`.** A subscription that has fallen over does not
//! know whether the ledger is moving; drawing `0 events` for it would tell the reader that nothing
//! is happening, which is precisely the collapse (`unknown` into `zero`) that this face's seven
//! words exist to refuse. `open` **with** zero events is a different sentence and it is a
//! measurement: this process was listening, and nothing came.
//!
//! 🔴 Four of the five states reuse [`super::wire::Nothing`] rather than spelling marks of their
//! own ([`Link::nothing`]). A second vocabulary of absence would drift from the first the day either
//! one is edited — the symbol for `unknown` would change in one table and not the other, and the
//! subscription line would go on drawing a mark the legend no longer explains. Only `open` needs a
//! mark of its own, because it is the one state that is not an absence at all.
//!
//! # An attempt is not an accomplishment
//!
//! The counter behind `reconnects` was incremented on every pass of the retry loop, including the
//! passes where nothing was ever opened, so an engine that had never once been up reported a growing
//! number of *re*-openings of a connection that had never existed — `closed after 0 events, 30
//! reconnects` after a minute. The worker now counts two things it can actually observe, `attempts`
//! and `opens`, and both numbers the report carries are **derived** from them ([`reopenings`],
//! [`after_attempt`]) rather than accumulated beside them. A derived number cannot drift from what
//! it describes.
//!
//! # A read that returns nothing is not a disconnection
//!
//! The socket carrying the stream is given a short read timeout so the worker can notice that it has
//! been asked to stop. A timeout on that socket therefore means "the window passed and the engine
//! had nothing to say", which is the *normal* state of a healthy subscription — measured against the
//! live engine on `:8842`, which replays its history in one burst and then says nothing for as long
//! as you leave it open. Reading a timeout as a close would make this face report a broken engine
//! every half second. [`pulse`] is where that distinction is made, and it is a pure function over an
//! [`std::io::Result`] so a gate can fire every arm of it without a socket.
//!
//! # Two layers of framing, because the wire has two
//!
//! Measured rather than assumed (`GET /v1/stream`, engine 0.1.0): the response is
//! `content-type: application/x-ndjson` **and** `transfer-encoding: chunked`. So the bytes off the
//! socket are chunk headers wrapped around NDJSON lines, and neither layer's boundaries line up with
//! a `read` — a chunk header can be split across two reads and so can a line. [`Frames`] carries the
//! remainder of both, which is why it is a state machine rather than a `split('\n')`. An
//! implementation that dropped the straddling line would pass almost every test and lose events in
//! the field at a rate nobody could reproduce, which is the worst schedule a defect can have.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::wire::{self, Nothing};

/// The route the engine pushes on.
///
/// 🔴 **Measured, not copied out of a document.** Specification 44 §2.2 spells it `/stream`, because
/// the routes are declared inside the `/v1` nest; on the wire it is `/v1/stream`. Asking the running
/// engine for `/stream` answers `404` with `"GET /stream is not a route of this server"`. The brief
/// this module was written from carried the specification's spelling, and the first probe against
/// the live engine is what turned it into the wire's.
pub const STREAM_ROUTE: &str = "/v1/stream";

/// What an event does to this face. One word, and a gate reads it.
///
/// 🔴 The whole of the ruling in the module documentation, in a form something can check. `apply`
/// here would mean a second source of truth; `reread` means the engine stays the only one.
pub const ON_EVENT: &str = "reread";

/// How long after an event this face waits before asking the four routes again.
///
/// The engine replays its whole history on connect — fourteen events in one burst, measured — and a
/// face that read four routes per event would perform fifty-six reads to draw one frame.
pub const DEBOUNCE: Duration = Duration::from_millis(250);

/// How long a read on the stream socket waits before returning so the worker can check whether it
/// has been asked to stop. **A read that ends this way is idle, not closed.**
pub const STREAM_TICK: Duration = Duration::from_millis(500);

/// How long the worker waits before opening the stream again after it ended.
pub const REOPEN_AFTER: Duration = Duration::from_secs(2);

/// The mark for a connection that is up.
///
/// 🔴 Two arrows pointing at the screen: the engine is pushing this way. Inside `U+0020..=U+007E`
/// like every other mark in this face, and equal to none of the seven words for nothing — being
/// connected is not one of them.
pub const OPEN_MARK: &str = "<<";

/// Whether this run is subscribed to the engine's events, and what has become of the connection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Link {
    /// This run does not subscribe. `gx tui --dump` draws one frame and leaves.
    Off,
    /// The stream was asked for and has not answered.
    Opening,
    /// The engine is pushing changes down it.
    Open,
    /// It has been asked for at least once and it has never once been up.
    ///
    /// 🔴 Defined by the history and not by the reason. An engine that answered `401` and an engine
    /// that could not be reached are both this state, because what each of them *means* is already
    /// on the screen in the four reads' status codes — the ruling `follow` was already making when
    /// it folded the two into one arm. This repair carves `Never` out of [`Link::Closed`] and leaves
    /// that ruling alone.
    Never,
    /// It has been up, and what is arriving now is not knowable — either it ended, or this face
    /// cannot read its own record of it (a poisoned lock).
    Closed,
}

/// Every state a subscription can be in. Gate g19 requires there to be five of them and requires
/// each one to be drawn differently from the other four.
pub const LINKS: [Link; 5] = [
    Link::Off,
    Link::Opening,
    Link::Open,
    Link::Never,
    Link::Closed,
];

impl Link {
    /// The spelled name, for a gate and for a report.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Link::Off => "link.off",
            Link::Opening => "link.opening",
            Link::Open => "link.open",
            Link::Never => "link.never",
            Link::Closed => "link.closed",
        }
    }

    /// Which of the wire's kinds of nothing this state **is**, or `None` when it is not an absence.
    ///
    /// 🔴 The single most load-bearing line in this module. `Closed` resolves to
    /// [`Nothing::Unknown`] and there is no arm here that can produce [`Nothing::Zero`] — a dropped
    /// subscription does not know how much has happened, and the mark that says "the count is
    /// nought" would be an answer this process is not entitled to give. `Never` resolves to
    /// [`Nothing::False`] and not to [`Nothing::Absent`], which is `Off`'s: giving `Never` the mark
    /// its neighbour already wears would merge a pair this map separates, and gate `g22` fires on
    /// exactly that.
    #[must_use]
    pub fn nothing(self) -> Option<Nothing> {
        match self {
            Link::Off => Some(Nothing::Absent),
            Link::Opening => Some(Nothing::Loading),
            // Being connected is not a kind of nothing, so it takes a mark of its own rather than
            // borrowing one that means an absence.
            Link::Open => None,
            // Asked, and the answer is no. The one word among the seven that is a measurement with
            // a negative answer rather than a quantity nobody has.
            Link::Never => Some(Nothing::False),
            Link::Closed => Some(Nothing::Unknown),
        }
    }

    /// What is drawn for this state.
    #[must_use]
    pub fn mark(self) -> &'static str {
        self.nothing().map_or(OPEN_MARK, Nothing::mark)
    }

    /// The sentence this state would say, one per state.
    ///
    /// 🔴 Five sentences and not one with a number in it. A single sentence covering five states is
    /// where three of them collapse into one, because the writer of it has to choose which
    /// distinction to spend the words on. The three that are easiest to blur come out different
    /// here on purpose: `off` says *there is no such line*, `never` says *asked, and the answer is
    /// no*, `closed` says *I cannot say what has arrived since*.
    #[must_use]
    pub fn sentence(self) -> &'static str {
        match self {
            Link::Off => "there is no connection for events to arrive on",
            Link::Opening => "the connection has not answered yet",
            Link::Open => "the engine is pushing changes down this connection",
            Link::Never => "this connection has never once been up",
            Link::Closed => "nothing can arrive while this is closed",
        }
    }
}

/// What the connection is once an attempt to open it has ended, given how many times the stream has
/// ever been up.
///
/// 🔴 A pure function rather than a `match` inside the worker loop, shaped like [`pulse`] and for
/// the same reason: a gate fires both arms of it without a socket. It is also the whole of the
/// `never`/`closed` ruling in one line — the state is decided by the **history** and not by the
/// reason the last attempt failed, so a refusal and an unreachable host land in the same place and
/// the screen distinguishes them by the four reads' status codes rather than by this line.
#[must_use]
pub const fn after_attempt(opens: u64) -> Link {
    if opens == 0 {
        Link::Never
    } else {
        Link::Closed
    }
}

/// How many times a connection that has been opened `opens` times has been opened **again**.
///
/// 🔴 Derived, not accumulated. The defect this replaces incremented a counter on every pass of the
/// retry loop, including the passes where nothing opened, so an attempt was reported as a
/// re-opening: an engine that had never been up claimed thirty reconnections of a connection that
/// had never existed. An attempt is not an accomplishment, and the cheapest way to keep it from
/// becoming one is to stop having a second number that could disagree.
#[must_use]
pub const fn reopenings(opens: u64) -> u64 {
    opens.saturating_sub(1)
}

/// What this face reports when it cannot read its own record of the connection.
///
/// 🔴 **Named rather than left inline, because naming it is the only way anything can fire it.** The
/// lock behind [`Subscription`] is poisoned only by a panic inside a critical section, and every
/// critical section in this module is one assignment or one addition: [`set`]'s closures,
/// [`record`]'s tally, [`Subscription::due`], [`Subscription::report`] and the one line in
/// [`Subscription::start`]. There is no `unwrap`, no index, no allocation and no user code under a
/// guard, so **on this code the branch is unreachable** (`req/38` SS996, read rather than run).
/// Unreachable is not absent: the branch exists, this is the report it produces, and gate `g25`
/// requires it to go on existing and to go on saying `?`. An arm nothing can reach and nothing names
/// is the shape a lie takes when someone later edits the critical sections.
///
/// `Closed` and not `Never`: what has happened is unknowable, which is what `?` means, and `never`
/// would be a claim about a history this process can no longer read.
///
/// 🔴 **Named ceiling.** The counts come back nought, so [`LinkReport::long`] would spell `closed
/// after 0 events, 0 reconnects` — a count offered for a record that could not be read, which is the
/// `unknown`-into-`zero` collapse this module's own documentation refuses one paragraph up. Saying
/// *unknown* about a `u64` needs a third value on this report, and a third value is a change to what
/// the worker and the face agree on. Declared here rather than half-done, and out of reach in
/// practice for the reason above.
#[must_use]
pub const fn unreadable_record() -> LinkReport {
    let mut report = LinkReport::off();
    report.link = Link::Closed;
    report
}

/// What the subscription has measured about itself: the state, and the counts behind it.
///
/// 🔴 Every member is a **renderer-local** fact in the sense of `req/942` §19-4 — the engine returns
/// none of them from any route, so a second read produces a new measurement rather than the lost
/// one. That is why the line carrying them is the provenance line, which is `priority.1`, rather
/// than the apparatus, which is dropped first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinkReport {
    /// The state of the connection.
    pub link: Link,
    /// How many well-formed events have arrived on it.
    pub events: u64,
    /// How many complete lines arrived that were **not** well formed, plus any line the connection
    /// ended in the middle of. Counted rather than discarded: quietly dropping what arrived is the
    /// lie "nothing came".
    pub unreadable: u64,
    /// How many times the connection has been opened again after ending. **Derived** from the
    /// number of times it was up ([`reopenings`]); the worker keeps no counter of its own for it.
    pub reconnects: u64,
    /// How many times opening it has been attempted. The number that separates
    /// [`Link::Never`] from [`Link::Off`] on the screen: `off` never asked, `never` asked this many
    /// times and was never once answered with a body.
    pub attempts: u64,
}

impl Default for LinkReport {
    fn default() -> Self {
        Self::off()
    }
}

impl LinkReport {
    /// The report of a run that does not subscribe.
    #[must_use]
    pub const fn off() -> Self {
        Self {
            link: Link::Off,
            events: 0,
            unreadable: 0,
            reconnects: 0,
            attempts: 0,
        }
    }

    /// The long form: the state spelled in words, with its counts.
    #[must_use]
    pub fn long(&self) -> String {
        let mut text = match self.link {
            Link::Off => "not subscribed".to_string(),
            Link::Opening => "connecting".to_string(),
            // 🔴 `0 events` on an open connection is a **measurement** and is drawn as one. The mark
            // in front of it says the connection is up, so the nought is the answer to a question
            // that was actually asked.
            Link::Open => format!("{} events", self.events),
            // 🔴 No `events` count and no `reconnects` count, because both would be nought and both
            // would read as measurements. What this state can honestly report is how many times it
            // tried, which is the one number it did observe.
            Link::Never => format!("never connected in {} attempts", self.attempts),
            Link::Closed => format!(
                "closed after {} events, {} reconnects",
                self.events, self.reconnects
            ),
        };
        if self.unreadable > 0 {
            text.push_str(&format!(", {} unreadable", self.unreadable));
        }
        text
    }

    /// The short form. The counts only; the mark in front of the line carries the state.
    #[must_use]
    pub fn short(&self) -> String {
        let mut text = match self.link {
            Link::Off | Link::Opening => String::new(),
            Link::Open => format!("{}ev", self.events),
            Link::Never => format!("{}att", self.attempts),
            Link::Closed => format!("{}ev {}re", self.events, self.reconnects),
        };
        if self.unreadable > 0 {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&format!("{}?", self.unreadable));
        }
        text
    }
}

/// What one read on the stream socket amounted to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pulse {
    /// The window passed and the engine had nothing to say. **Not a disconnection.**
    Idle,
    /// Bytes arrived.
    Bytes(usize),
    /// The connection ended.
    Ended,
}

/// Classify one read.
///
/// 🔴 A pure function over the result rather than a `match` inside the worker loop, so that gate g19
/// can fire every arm of it — including the two error kinds a socket timeout arrives as — without
/// opening a socket. The defect this guards against is the one the browser face met and paid for: a
/// timeout meant for a list read was applied to a subscription, and the subscription reported a
/// broken engine every six seconds.
#[must_use]
pub fn pulse(result: &std::io::Result<usize>) -> Pulse {
    match result {
        Ok(0) => Pulse::Ended,
        Ok(count) => Pulse::Bytes(*count),
        Err(error) => match error.kind() {
            std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::Interrupted => Pulse::Idle,
            _ => Pulse::Ended,
        },
    }
}

/// The chunk layer's position in the frame it is reading.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Chunk {
    /// Collecting the hexadecimal size line.
    Size(Vec<u8>),
    /// Copying this many more payload bytes.
    Body(usize),
    /// Swallowing this many more bytes of the CRLF that ends a chunk's payload.
    Tail(u8),
    /// The zero-sized chunk arrived; nothing after it is payload.
    Done,
}

/// Bytes off a socket, turned into whole NDJSON lines.
///
/// Two layers, because the wire has two: chunk framing outside, newline framing inside. Both
/// remainders are carried across calls, so a `push` that ends in the middle of either produces
/// nothing and the next one completes it.
#[derive(Clone, Debug)]
pub struct Frames {
    chunk: Chunk,
    line: Vec<u8>,
    chunked: bool,
}

impl Frames {
    /// A reader for a `transfer-encoding: chunked` body, which is what the engine sends.
    #[must_use]
    pub fn chunked() -> Self {
        Self {
            chunk: Chunk::Size(Vec::new()),
            line: Vec::new(),
            chunked: true,
        }
    }

    /// A reader for a body with no transfer encoding, so the bytes are the payload.
    ///
    /// Declared because the framing is read off the response headers rather than assumed: a face
    /// that hard-codes the encoding it saw once breaks silently the day a proxy sits in front of the
    /// engine, and it breaks by losing events rather than by failing.
    #[must_use]
    pub fn plain() -> Self {
        Self {
            chunk: Chunk::Body(usize::MAX),
            line: Vec::new(),
            chunked: false,
        }
    }

    /// The reader for a response with these headers.
    #[must_use]
    pub fn for_headers(chunked: bool) -> Self {
        if chunked {
            Self::chunked()
        } else {
            Self::plain()
        }
    }

    /// How many bytes are held in the middle of a line that has not ended yet.
    ///
    /// 🔴 Read when the connection ends: a stranded remainder is something that arrived and cannot
    /// be understood, and it is counted rather than dropped.
    #[must_use]
    pub fn partial(&self) -> usize {
        self.line.len()
    }

    /// Whether the body has run out. `false` for a plain body, which ends when the socket does.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.chunk == Chunk::Done
    }

    /// Feed one read's worth of bytes in; take whole lines out.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut lines: Vec<Vec<u8>> = Vec::new();
        let mut rest = bytes;
        while !rest.is_empty() {
            // The state is taken out and put back rather than borrowed, so that the line splitter
            // can be a method on `self` — the alternative is a free function that takes the line
            // remainder as an argument, which puts the same remainder in two places.
            match std::mem::replace(&mut self.chunk, Chunk::Done) {
                Chunk::Done => break,
                Chunk::Size(mut collected) => {
                    let Some(end) = rest.iter().position(|byte| *byte == b'\n') else {
                        collected.extend_from_slice(rest);
                        self.chunk = Chunk::Size(collected);
                        break;
                    };
                    collected.extend_from_slice(&rest[..end]);
                    rest = &rest[end + 1..];
                    self.chunk = match parse_size(&collected) {
                        // A chunk header this reader cannot parse ends the body rather than being
                        // guessed at: resynchronising on a stream whose framing is not understood
                        // would invent lines out of chunk headers.
                        None | Some(0) => Chunk::Done,
                        Some(count) => Chunk::Body(count),
                    };
                }
                Chunk::Body(owed) => {
                    let take = rest.len().min(owed);
                    self.split(&rest[..take], &mut lines);
                    rest = &rest[take..];
                    let left = owed - take;
                    self.chunk = if left == 0 && self.chunked {
                        Chunk::Tail(2)
                    } else {
                        Chunk::Body(left)
                    };
                }
                Chunk::Tail(owed) => {
                    // `owed` is at most two, so the conversion cannot lose a value.
                    let take = u8::try_from(rest.len().min(usize::from(owed))).unwrap_or(owed);
                    rest = &rest[usize::from(take)..];
                    let left = owed - take;
                    self.chunk = if left == 0 {
                        Chunk::Size(Vec::new())
                    } else {
                        Chunk::Tail(left)
                    };
                }
            }
        }
        lines
    }

    /// Append payload bytes and take off any lines they completed.
    fn split(&mut self, bytes: &[u8], lines: &mut Vec<Vec<u8>>) {
        for byte in bytes {
            if *byte == b'\n' {
                let mut line = std::mem::take(&mut self.line);
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if !line.is_empty() {
                    lines.push(line);
                }
            } else {
                self.line.push(*byte);
            }
        }
    }
}

/// The size a chunk header declares, or `None` when this reader cannot read it.
///
/// The header may carry extensions after a semicolon, which are not part of the number.
fn parse_size(header: &[u8]) -> Option<usize> {
    let text = String::from_utf8(header.to_vec()).ok()?;
    let text = text.trim_end_matches('\r');
    let digits = text.split(';').next()?.trim();
    if digits.is_empty() {
        return None;
    }
    usize::from_str_radix(digits, 16).ok()
}

/// What the worker thread and the face share.
#[derive(Debug)]
struct Shared {
    link: Link,
    events: u64,
    unreadable: u64,
    /// How many times opening the stream has been **attempted**.
    attempts: u64,
    /// How many times it has actually been **up** — a 2xx with a body to read.
    ///
    /// 🔴 Two counters and not one, because the two numbers the report carries are answers to two
    /// different questions and the defect this replaces answered both with the first one.
    opens: u64,
    /// An event has arrived and the face has not been told to look again yet.
    pending: bool,
    /// When the most recent event arrived, which is what the debounce is measured from.
    last: Option<Instant>,
}

/// A live subscription to the engine's events.
///
/// 🔴 What this hands out is a [`LinkReport`] and a boolean. No event body crosses this boundary,
/// which is what makes "an event cannot write a row" a property of the shape of the code rather than
/// a promise about its contents.
#[derive(Debug)]
pub struct Subscription {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Subscription {
    /// Open the stream and keep it open, reopening it when it ends, until this value is dropped.
    #[must_use]
    pub fn start(base_url: &str, token: Option<&str>) -> Self {
        let shared = Arc::new(Mutex::new(Shared {
            link: Link::Opening,
            events: 0,
            unreadable: 0,
            attempts: 0,
            opens: 0,
            pending: false,
            last: None,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let base = base_url.to_string();
        let token = token.map(str::to_string);
        let worker = {
            let shared = Arc::clone(&shared);
            let stop = Arc::clone(&stop);
            thread::Builder::new()
                .name("gx-tui-stream".to_string())
                .spawn(move || follow(&base, token.as_deref(), &shared, &stop))
                .ok()
        };
        // 🔴 A thread that could not be started is a subscription that has never been up, said out
        // loud. The alternative — leaving the state at `opening` forever — is a screen that waits
        // for something nobody is waiting for. It is `never` and not `closed` because nothing was
        // ever opened here: `attempts` stays at nought too, since no attempt reached a socket.
        if worker.is_none() {
            if let Ok(mut state) = shared.lock() {
                state.link = Link::Never;
            }
        }
        Self {
            shared,
            stop,
            worker,
        }
    }

    /// What the subscription has measured about itself.
    ///
    /// A poisoned lock reports [`unreadable_record`]: `closed` and **not** `never`, which is the
    /// second half of this state's widened definition — *either it ended, or this face cannot read
    /// its own record of it*. That arm is spelled once, out of line, so that `g25` can fire the
    /// thing itself; unreachable on this code is not the same as absent.
    ///
    /// 🔴 `reconnects` is [`reopenings`] of the number of times the stream was up, computed here.
    /// There is no counter behind it that could disagree with the state beside it.
    #[must_use]
    pub fn report(&self) -> LinkReport {
        self.shared
            .lock()
            .map_or(unreadable_record(), |state| LinkReport {
                link: state.link,
                events: state.events,
                unreadable: state.unreadable,
                reconnects: reopenings(state.opens),
                attempts: state.attempts,
            })
    }

    /// Whether an event has arrived and the debounce window has passed since the last one.
    ///
    /// Clears the flag, so one burst produces one re-read.
    pub fn due(&self) -> bool {
        let Ok(mut state) = self.shared.lock() else {
            return false;
        };
        let ready = state.pending && state.last.is_some_and(|at| at.elapsed() >= DEBOUNCE);
        if ready {
            state.pending = false;
        }
        ready
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            // The worker's socket has a read timeout of [`STREAM_TICK`], so the longest this waits
            // is that window plus whatever it is copying when it is asked to stop.
            let _ = worker.join();
        }
    }
}

/// Open the stream, read it until it ends, and open it again. Runs on the worker thread.
fn follow(
    base_url: &str,
    token: Option<&str>,
    shared: &Arc<Mutex<Shared>>,
    stop: &Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Relaxed) {
        set(shared, |state| {
            state.link = Link::Opening;
            state.attempts += 1;
        });
        match wire::open_stream(base_url, STREAM_ROUTE, token, STREAM_TICK) {
            Ok(mut opened) if (200..300).contains(&opened.status) => {
                set(shared, |state| {
                    state.link = Link::Open;
                    state.opens += 1;
                });
                let mut frames = Frames::for_headers(opened.chunked);
                let carried = frames.push(&opened.head_body);
                record(shared, carried);
                let mut buffer = [0u8; 8192];
                loop {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let read = opened.socket.read(&mut buffer);
                    match pulse(&read) {
                        Pulse::Idle => {}
                        Pulse::Bytes(count) => {
                            let lines = frames.push(&buffer[..count]);
                            record(shared, lines);
                            if frames.finished() {
                                break;
                            }
                        }
                        Pulse::Ended => break,
                    }
                }
                if frames.partial() > 0 {
                    set(shared, |state| state.unreadable += 1);
                }
            }
            // An engine that answers and refuses (`401`, which `wire::open_stream` returns as an
            // `Ok` carrying the code), and an engine that cannot be reached, are the same fact for
            // this line: nothing is arriving. What each of them *means* is already on the screen, in
            // the status codes the four reads carry. **Measured**, not derived: a token-requiring
            // engine asked without one lands here, and the line it produces is `never`.
            Ok(_) | Err(_) => {}
        }
        if stop.load(Ordering::Relaxed) {
            return;
        }
        // 🔴 The state the attempt ended in is a function of whether this stream has ever been up,
        // and nothing is incremented here. Incrementing a `reconnects` counter on this line is the
        // defect the module documentation names: the passes that never opened anything were being
        // counted as re-openings.
        set(shared, |state| state.link = after_attempt(state.opens));
        // Slept in slices so that leaving the face does not wait out the whole window.
        let until = Instant::now() + REOPEN_AFTER;
        while Instant::now() < until {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(STREAM_TICK.min(REOPEN_AFTER));
        }
    }
}

/// Count what arrived, and mark that the face should look again.
///
/// 🔴 The bodies go no further than this function. A line that does not parse still counts as
/// something having happened — the engine said *something* — so it raises the flag and is counted
/// as unreadable, rather than being dropped into a silence that reads as "no change".
fn record(shared: &Arc<Mutex<Shared>>, lines: Vec<Vec<u8>>) {
    if lines.is_empty() {
        return;
    }
    let mut events = 0u64;
    let mut unreadable = 0u64;
    for line in &lines {
        if serde_json::from_slice::<serde_json::Value>(line).is_ok() {
            events += 1;
        } else {
            unreadable += 1;
        }
    }
    set(shared, |state| {
        state.events += events;
        state.unreadable += unreadable;
        state.pending = true;
        state.last = Some(Instant::now());
    });
}

/// Move the shared state, or leave it alone when the lock is poisoned.
fn set(shared: &Arc<Mutex<Shared>>, change: impl FnOnce(&mut Shared)) {
    if let Ok(mut state) = shared.lock() {
        change(&mut state);
    }
}
