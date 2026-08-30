// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Placement tokens: the four-layer ladder `req/942` §11 puts under the terminal's fixed grid.
//!
//! ```text
//! intent  ->  role  ->  token  ->  value
//! ```
//!
//! A caller of this module names an **intent** or a **role** and a **priority**. It cannot name a
//! cell count, a direction or a rectangle; those live one layer down, in `super::renderer`, which is
//! the only module in this face allowed to spell them. `tui/tests/r942_tui.rs` gate g5
//! measures that, and gate g6 measures that every `region.*` name spelled anywhere in this face is
//! one of [`LAYOUT_ROLES`].
//!
//! # Why placement has to be a token and not a hand-written `if`
//!
//! A face that writes `if width < 80 { ... }` in each screen **cannot name what it dropped**. The
//! obligation a renderer carries in place of the `invert` an adapter carries (`req/942` §1-1) is to
//! say what it let go of, and that is only possible if letting go is a declared order rather than a
//! branch. So the drop set here is a **computed value** the screen is handed, not a fact the screen
//! knows about itself.
//!
//! # Recoverability decides priority, and nothing else does
//!
//! `req/942` §19-5 corrected this module's first draft before it was written: the provenance region
//! was `priority.2` because provenance *feels* like a footnote. It is not. The four facts it carries
//! — route, status, read time, elapsed — are measured **here**, and the engine returns none of them
//! from any route, so dropping them destroys them. Every region therefore declares
//! [`Recoverable`], and gate g10 refuses a `Recoverable::Nowhere` region at any priority below the
//! top one.

use std::fmt;

/// What a region is for, in the reader's terms. The top of the ladder.
///
/// The reason this layer exists at all rather than starting at [`RegionRole`]: an intent can be
/// checked against the product's own vocabulary, which is what lets a screen be gated by the same
/// words the engine uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Intent {
    /// "the records the engine produced"
    RecordsTheEngineProduced,
    /// "how this page knows what it knows"
    HowThisPageKnows,
    /// "where these numbers came from, and when"
    WhereTheNumbersCameFrom,
    /// "what is on the wire and not on the screen"
    WhatIsNotOnTheScreen,
}

impl Intent {
    /// The sentence, for the report and for a reader of the source.
    #[must_use]
    pub fn sentence(self) -> &'static str {
        match self {
            Intent::RecordsTheEngineProduced => "the records the engine produced",
            Intent::HowThisPageKnows => "how this page knows what it knows",
            Intent::WhereTheNumbersCameFrom => "where these numbers came from, and when",
            Intent::WhatIsNotOnTheScreen => "what is on the wire and not on the screen",
        }
    }
}

/// Which region. The second rung.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegionRole {
    /// The rows the engine produced.
    Subject,
    /// The engine's own account of itself.
    Apparatus,
    /// The four facts this process measured while reading.
    Provenance,
    /// What was let go of, and where to go for it.
    Disclosure,
}

impl RegionRole {
    /// The spelled name. Gate g6 requires every `region.*` literal in this face to be one of these.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            RegionRole::Subject => "region.subject",
            RegionRole::Apparatus => "region.apparatus",
            RegionRole::Provenance => "region.provenance",
            RegionRole::Disclosure => "region.disclosure",
        }
    }

    /// The short name a disclosure line uses, so that a forty-column line still says which.
    ///
    /// Spelled out rather than derived by trimming a prefix off [`Self::name`]: the prefix would be
    /// a fifth `"region."` literal in this file, and gate g6 — which collects every such literal and
    /// requires it to be a declared role — correctly reported it as an undeclared name. The gate was
    /// right and the first draft was wrong.
    #[must_use]
    pub fn short(self) -> &'static str {
        match self {
            RegionRole::Subject => "subject",
            RegionRole::Apparatus => "apparatus",
            RegionRole::Provenance => "provenance",
            RegionRole::Disclosure => "disclosure",
        }
    }
}

impl fmt::Display for RegionRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Every role name this face may spell.
pub const LAYOUT_ROLES: [&str; 4] = [
    "region.subject",
    "region.apparatus",
    "region.provenance",
    "region.disclosure",
];

/// The declared order of letting go. `One` is the last to be dropped.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Priority {
    /// Never dropped while anything is drawn at all.
    One,
    /// Dropped after everything below it.
    Two,
    /// Dropped early.
    Three,
    /// Dropped first.
    Four,
}

/// Where a reader goes for what this region carried, once it is not on the screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Recoverable {
    /// Every fact in it is addressable by an object id.
    Address,
    /// Every fact in it comes back by re-reading a route.
    Route(&'static str),
    /// Nothing in it can be fetched again; a second read makes a new measurement.
    Nowhere,
}

/// One declared region.
#[derive(Clone, Copy, Debug)]
pub struct Region {
    /// Why it is on the screen.
    pub intent: Intent,
    /// Which part of the screen.
    pub role: RegionRole,
    /// When it is let go of.
    pub priority: Priority,
    /// Whether letting go of it destroys anything.
    pub recoverable: Recoverable,
    /// The fewest rows in which it says anything at all.
    pub min_rows: u16,
}

/// The four regions, in draw order from the top of the screen.
///
/// 🔴 `Provenance` and `Disclosure` are both `Recoverable::Nowhere` and both `Priority::One`, and
/// gate g10 is the mechanism that keeps the pair honest. The symmetry is exact: if the disclosure
/// goes, there is no mouth left to say what went; if the provenance goes, what went cannot be said
/// **at all**, because it has no address to name.
pub const REGIONS: [Region; 4] = [
    Region {
        intent: Intent::HowThisPageKnows,
        role: RegionRole::Apparatus,
        priority: Priority::Three,
        recoverable: Recoverable::Route("GET /v1/healthz"),
        // 🔴 Three, and it was four. The fourth row was reserved against a `status_reason` long
        // enough to wrap twice, and in every state measured against a live engine it was **never
        // used**: the region drew two rows and held four at 46, 60, 80, 100 and 120 cells wide. A
        // panel that is permanently part empty is furniture, and this one was furniture in the
        // region that is dropped first — so the rows it was hoarding were the rows the screen went
        // looking for when it ran out.
        //
        // Three is safe because the head is now wrapped rather than clipped and a head that still
        // does not fit is cut **with a mark** (`super::renderer::apparatus`). Before that, cutting
        // the apparatus was silent, so the fourth row was buying silence rather than safety.
        min_rows: 3,
    },
    Region {
        intent: Intent::RecordsTheEngineProduced,
        role: RegionRole::Subject,
        priority: Priority::One,
        recoverable: Recoverable::Address,
        // A header and three rows. A ledger showing one row is not a smaller ledger, it is a
        // different claim: it reads as "this is what there is".
        min_rows: 4,
    },
    Region {
        intent: Intent::WhereTheNumbersCameFrom,
        role: RegionRole::Provenance,
        priority: Priority::One,
        recoverable: Recoverable::Nowhere,
        min_rows: 1,
    },
    Region {
        intent: Intent::WhatIsNotOnTheScreen,
        role: RegionRole::Disclosure,
        priority: Priority::One,
        recoverable: Recoverable::Nowhere,
        min_rows: 1,
    },
];

/// One column of the subject table.
#[derive(Clone, Copy, Debug)]
pub struct Column {
    /// 🔴 The wire's own key, never a synonym. `created_at` is not `when`; `scope` is not `target`.
    pub key: &'static str,
    /// The width the value is given, in cells.
    pub width: u16,
    /// When the column is let go of.
    pub priority: Priority,
}

/// The subject table's columns, already in the order they are let go of (last first).
///
/// 🔴 The key list is `crates/gx-api/src/list.rs`'s `row_json` plus the three the transformations
/// list adds, and the labels drawn on screen are these strings unchanged (`req/942` §9, gate P5).
pub const LEDGER_COLUMNS: [Column; 10] = [
    Column {
        key: "transformation",
        width: 16,
        priority: Priority::One,
    },
    Column {
        key: "verdict",
        width: 9,
        priority: Priority::One,
    },
    Column {
        key: "state",
        width: 13,
        priority: Priority::One,
    },
    Column {
        key: "created_at",
        width: 20,
        priority: Priority::Two,
    },
    Column {
        key: "scope",
        width: 18,
        priority: Priority::Two,
    },
    Column {
        key: "enforced",
        width: 8,
        priority: Priority::Two,
    },
    Column {
        key: "inverse_status",
        width: 14,
        priority: Priority::Three,
    },
    Column {
        key: "rollback",
        width: 10,
        priority: Priority::Three,
    },
    Column {
        key: "superseded_by",
        width: 16,
        priority: Priority::Four,
    },
    Column {
        key: "actor",
        width: 12,
        priority: Priority::Four,
    },
];

/// The keys the page carries around the rows. Never drawn, and therefore always disclosed.
pub const LEDGER_PAGE_KEYS: [&str; 1] = ["next_cursor"];

/// The address the disclosure line spells for anything the subject table let go of.
///
/// 🔴 `req/942` §19-3: the address is the **route**, not `gx show <id> --all`. Only eight of the
/// wire's fields come back from an id; all of them come back from the route. Gate g9 checks that
/// the address spelled here really answers with every field named as dropped.
pub const LEDGER_ADDRESS: &str = "GET /v1/transformations";

/// The two routes this face reads and does not draw, declared once so the count is never quietly
/// zero (`req/942` §2: the range this face does not cover is part of what it must say).
pub const READ_NOT_DRAWN: [&str; 2] = ["GET /v1/candidates", "GET /v1/escalations"];

/// The phrase that must appear when the provenance is folded into the disclosure.
///
/// 🔴 `req/942` §19-5-2. Without it the four measured facts would sit in the disclosure line
/// wearing the same face as the fields that do have addresses, and a reader would reasonably
/// believe they could be fetched again.
pub const NO_ADDRESS_PHRASE: &str = "no address, measured here";

/// The four facts each reading measured, summarised for the provenance region — and, since the
/// subscription landed, the fifth: whether these numbers are being kept fresh.
///
/// 🔴 The subscription's state belongs **here** rather than in the apparatus region, and the reason
/// is the one this whole module is ordered by. The apparatus is `priority.3` and is let go of first;
/// the state of the connection is a fact this process measured, which the engine returns from no
/// route, so losing it destroys it — [`Recoverable::Nowhere`], which is what puts the provenance at
/// `priority.1`. Putting the connection anywhere else would mean a screen that quietly stops saying
/// whether it is live.
#[derive(Clone, Debug)]
pub struct Measured {
    /// How many routes were read.
    pub routes: usize,
    /// When, RFC 3339 trimmed to the second.
    pub read_at: String,
    /// The slowest of them.
    pub worst_ms: u128,
    /// `all 200`, or the codes one by one when they are not all the same.
    pub statuses: String,
    /// The subscription's state and its counts (`super::live`).
    pub link: super::live::LinkReport,
}

impl Measured {
    /// The long form, for a screen wide enough to carry it.
    #[must_use]
    pub fn long(&self) -> String {
        format!(
            "{} read {} routes at {} | worst {}ms | {} | {}",
            self.link.link.mark(),
            self.routes,
            self.read_at,
            self.worst_ms,
            self.statuses,
            self.link.long()
        )
    }

    /// The short form. Still carries all four facts; only the spelling is smaller.
    #[must_use]
    pub fn short(&self) -> String {
        let clock = self.read_at.split('T').next_back().unwrap_or(&self.read_at);
        let tail = self.link.short();
        let line = format!(
            "{} {} routes {} {}ms {}",
            self.link.link.mark(),
            self.routes,
            clock,
            self.worst_ms,
            self.statuses
        );
        if tail.is_empty() {
            line
        } else {
            format!("{line} {tail}")
        }
    }

    /// The shortest form: the connection's mark and the four facts, with the connection's counts
    /// given up.
    ///
    /// 🔴 The **mark** is never given up, and it leads the line for that reason. A terminal cuts
    /// from the right, so anything at the front of the row survives every width; the five states of
    /// the subscription are therefore told apart at every size this face can be drawn at, and gate
    /// g19b measures exactly that over the range 20..=200.
    ///
    /// **Named ceiling**: when this rung is chosen the counts are gone and no line on the screen
    /// says so. The disclosure counts the *grid's* dropped columns and is composed before the
    /// provenance is spelled, which is the same cause as the two ceilings already named in
    /// `super::renderer`. Upgrade path is the same one: hand the view and the provenance rung to
    /// [`resolve`] in one pass.
    #[must_use]
    pub fn bare(&self) -> String {
        let clock = self.read_at.split('T').next_back().unwrap_or(&self.read_at);
        format!(
            "{} {} routes {} {}ms {}",
            self.link.link.mark(),
            self.routes,
            clock,
            self.worst_ms,
            self.statuses
        )
    }

    /// The form used once the region is gone and the facts live in the disclosure line.
    #[must_use]
    pub fn folded(&self) -> String {
        format!("{} | {NO_ADDRESS_PHRASE}", self.short())
    }
}

/// What the screen is told to draw. Every member is computed here; none is decided by a screen.
#[derive(Clone, Debug)]
pub struct Plan {
    /// Region and row count, top to bottom, for the regions that survived.
    pub rows: Vec<(RegionRole, u16)>,
    /// The regions let go of, in the order they were let go of.
    pub dropped: Vec<RegionRole>,
    /// Whether the provenance was folded into the disclosure rather than dropped.
    pub provenance_folded: bool,
    /// The columns the subject table draws.
    pub columns: Vec<Column>,
    /// The wire keys the subject table does not draw, page keys included.
    pub dropped_fields: Vec<&'static str>,
    /// How many wire keys the route offers in total.
    pub total_fields: usize,
    /// The provenance region's text, when it has a region.
    pub provenance: String,
    /// Which rung of the provenance ladder that text is, so the disclosure and a gate can both read
    /// the decision rather than infer it from the string.
    pub provenance_rung: Rung,
    /// The disclosure region's text.
    pub disclosure: String,
    /// The screen was too small for even the floor, and says so.
    pub truncated: bool,
}

impl Plan {
    /// Rows given to one role, or zero when it was let go of.
    #[must_use]
    pub fn rows_for(&self, role: RegionRole) -> u16 {
        self.rows
            .iter()
            .find(|(r, _)| *r == role)
            .map_or(0, |(_, n)| *n)
    }
}

/// What the subject region is going to draw.
///
/// 🔴 **One classifier, read twice.** [`resolve`] reads it to compose the disclosure and
/// `super::renderer::subject` reads it to draw, so the line that says what is not on the screen and
/// the region it describes cannot disagree about which shape was drawn. Before this existed the
/// disclosure counted the **grid's** dropped columns unconditionally, so while a record was open it
/// reported fields as *not drawn* that the record was drawing one per row — the named ceiling in
/// `Subject`'s comment and in [`Measured::bare`], `req/964` §16.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Subject {
    /// A header and the grid's columns: the list, and the one-row kind-of-nothing that an empty or
    /// refused read draws. Width is what decides which fields are dropped.
    Grid,
    /// One record and every member it carries, with no grid over it. **No field is dropped by
    /// width here** — the members that do not fit are dropped by *height*, and the record's own
    /// line is the one that counts them.
    Record,
}

/// Which shape the subject region will take for this reading and this view.
///
/// The empty list keeps its grid, and that is not an inconsistency: an empty grid is still a grid,
/// and the header is what says which columns found nothing.
#[must_use]
pub fn subject_shape(reading: &super::wire::Reading, view: &super::acts::View) -> Subject {
    if view.open && !reading.items().is_empty() {
        Subject::Record
    } else {
        Subject::Grid
    }
}

/// Which rung of the provenance ladder the width bought.
///
/// 🔴 Declared rather than left as the shape of a string, because the bottom rung **drops the
/// connection's counts** and the disclosure is the line that has to say so. Chosen inside
/// [`resolve`]'s loop and before the disclosure is composed, which is the second half of
/// `req/964` §16: one pass, not two.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rung {
    /// Every fact, spelled out.
    Long,
    /// The same facts, smaller.
    Short,
    /// The four measured facts and the connection's **mark**, with its counts given up.
    Bare,
}

impl Rung {
    /// Whether the connection's counts reach the screen on this rung.
    ///
    /// A folded provenance is **not** a dropped one: [`Measured::folded`] is built on
    /// [`Measured::short`], so the counts travel into the disclosure with the phrase that says they
    /// have no address. Only [`Rung::Bare`] actually gives them up.
    #[must_use]
    pub fn carries_counts(self) -> bool {
        !matches!(self, Rung::Bare)
    }
}

/// Which columns fit, and which wire keys are therefore not drawn.
#[must_use]
pub fn columns_for(width: u16) -> (Vec<Column>, Vec<&'static str>) {
    let mut drawn = Vec::new();
    let mut dropped: Vec<&'static str> = Vec::new();
    let mut used = 0u16;
    for column in LEDGER_COLUMNS {
        let cost = if drawn.is_empty() {
            column.width
        } else {
            column.width + 1
        };
        if !dropped.is_empty() || used + cost > width {
            dropped.push(column.key);
        } else {
            used += cost;
            drawn.push(column);
        }
    }
    dropped.extend(LEDGER_PAGE_KEYS);
    (drawn, dropped)
}

/// How many rows a line of text needs at this width, word-wrapped.
#[must_use]
pub fn rows_needed(text: &str, width: u16) -> u16 {
    if width == 0 {
        return u16::MAX;
    }
    wrap(text, width).len() as u16
}

/// Word-wrap, breaking inside a word only when the word is wider than the screen.
#[must_use]
pub fn wrap(text: &str, width: u16) -> Vec<String> {
    let width = width.max(1) as usize;
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let mut word = word;
        loop {
            // 🔴 The cost of adding this word is the line **plus** the word, not the word alone.
            // The first draft measured the word by itself, so nothing ever exceeded the width and
            // every text came back one row long — which made the layout believe the disclosure fit
            // in one row at any size and, through that, made it drop nothing at forty by ten. The
            // defect was invisible in the plan and visible in the buffer: the bottom line was cut
            // mid-word. Found by the probes, not by reading.
            let need = if line.is_empty() {
                word.chars().count()
            } else {
                line.chars().count() + 1 + word.chars().count()
            };
            if line.is_empty() && word.chars().count() > width {
                let head: String = word.chars().take(width).collect();
                lines.push(head);
                word = &word[word
                    .char_indices()
                    .nth(width)
                    .map_or(word.len(), |(index, _)| index)..];
                continue;
            }
            if need > width {
                lines.push(std::mem::take(&mut line));
                continue;
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
            break;
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// What the screen is going to be, for the one line whose job is to describe it.
///
/// 🔴 Both members are things the **plan** decided and the disclosure would otherwise have to guess
/// at: which shape the subject region takes, and whether the provenance's rung gave up the
/// connection's counts. `req/964` §16.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Shape {
    subject: Subject,
    counts_dropped: bool,
}

/// The disclosure line, in whichever of its two forms fits.
///
/// The long form names each fact. The short form names the counts and gives the address of the long
/// form, which is `gx tui --wide` — the same shape `req/942` §11-3 wrote by hand. Either way the
/// counts are there, so no number is quietly zero.
///
/// The two facts about the screen it is describing travel as one [`Shape`] rather than as two more
/// parameters: a third thing to disclose should become a member of a description, not an eighth
/// argument nobody can read at the call site.
#[must_use]
fn compose_disclosure(
    dropped_fields: &[&'static str],
    total_fields: usize,
    dropped_regions: &[RegionRole],
    fold: Option<&Measured>,
    width: u16,
    wide: bool,
    shape: Shape,
) -> String {
    let Shape {
        subject,
        counts_dropped,
    } = shape;
    let mut long: Vec<String> = Vec::new();
    // 🔴 The field count belongs to the **grid**. While a record is open there is no grid, every
    // member the wire carried is a row of its own, and the record's own line is what says how many
    // of them the height allowed. Saying `4 of 11 fields not drawn` over a record that is drawing
    // all eleven is the disclosure describing a screen that is not there.
    match subject {
        Subject::Grid => {
            if !dropped_fields.is_empty() {
                long.push(format!(
                    "{} of {total_fields} fields not drawn | {LEDGER_ADDRESS}",
                    dropped_fields.len()
                ));
            }
        }
        Subject::Record => long.push(format!(
            "a record is open: its own line counts what it drew | {LEDGER_ADDRESS}"
        )),
    }
    // 🔴 The bottom rung of the provenance ladder gives up the connection's counts, and those are
    // measured **here** — no route returns them, so a second read makes a new measurement rather
    // than the lost one. A region that drops a `Recoverable::Nowhere` fact without a line saying so
    // is the one drop this face is not allowed to make quietly.
    if counts_dropped {
        long.push(format!(
            "the connection's counts are not drawn at this width | {NO_ADDRESS_PHRASE}"
        ));
    }
    if !dropped_regions.is_empty() {
        let names: Vec<&str> = dropped_regions.iter().map(|r| r.short()).collect();
        let addresses: Vec<&str> = dropped_regions
            .iter()
            .filter_map(|role| {
                REGIONS
                    .iter()
                    .find(|region| region.role == *role)
                    .and_then(|region| match region.recoverable {
                        Recoverable::Route(route) => Some(route),
                        _ => None,
                    })
            })
            .collect();
        long.push(format!(
            "{} regions not drawn: {} | {}",
            dropped_regions.len(),
            names.join(", "),
            addresses.join(", ")
        ));
    }
    long.push(format!(
        "{} routes read and not drawn: {}",
        READ_NOT_DRAWN.len(),
        READ_NOT_DRAWN.join(", ")
    ));
    if let Some(measured) = fold {
        long.push(measured.folded());
    }
    let long = long.join(" | ");

    let cap = disclosure_cap(fold.is_some());
    if wide || rows_needed(&long, width) <= cap {
        return long;
    }
    // 🔴 The short form still **names** the regions it let go of. A count on its own would satisfy
    // "say how many" and fail the thing the disclosure is for, which is saying *which*.
    let named = if dropped_regions.is_empty() {
        String::new()
    } else {
        format!(
            ": {}",
            dropped_regions
                .iter()
                .map(|role| role.short())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    // The short form spends its words on the same facts, including the two above: an open record
    // says so instead of quoting a field count that belongs to a grid nobody is looking at, and a
    // dropped set of counts is named rather than being the one thing only the long form admits.
    let head = match subject {
        Subject::Grid => format!("{}/{total_fields} fields", dropped_fields.len()),
        Subject::Record => "record open".to_string(),
    };
    let counts = if counts_dropped {
        format!(" | counts cut, {NO_ADDRESS_PHRASE}")
    } else {
        String::new()
    };
    let mut short = format!(
        "{head} | {} routes | {} regions not drawn{named}{counts} | gx tui --wide",
        READ_NOT_DRAWN.len(),
        dropped_regions.len()
    );
    if let Some(measured) = fold {
        short.push_str(" | ");
        short.push_str(&measured.folded());
    }
    short
}

/// The disclosure may take three rows, or four once the provenance has folded into it — the row the
/// provenance gave up is the row the fold is allowed to spend.
const fn disclosure_cap(folded: bool) -> u16 {
    if folded {
        4
    } else {
        3
    }
}

/// Resolve the grid.
///
/// The loop below is bounded at three passes because each pass takes one irreversible step
/// (drop the apparatus, then fold the provenance, then stop). Recomposing the disclosure after each
/// step is the point: a region that is let go of changes what the disclosure has to say, and a
/// disclosure written before the drop would be describing a different screen.
///
/// 🔴 `req/964` §16. `subject` is the shape the subject region will actually take, and the
/// provenance's rung is chosen **inside** the loop, before the disclosure is composed. Both were
/// named ceilings until this: the disclosure counted the grid's columns while a record was open,
/// and the rung was picked after the disclosure had already been written, so the rung that gives up
/// the connection's counts gave them up with nothing on the screen saying so. One pass, and the
/// line that says what is missing is composed from what the screen is going to be.
#[must_use]
pub fn resolve(width: u16, height: u16, measured: &Measured, wide: bool, subject: Subject) -> Plan {
    let (columns, grid_dropped_fields) = columns_for(width);
    // While a record is open no field is dropped by **width**: the record draws every member the
    // wire carried, one per row. So the plan's dropped set is empty, and it is empty as a computed
    // fact rather than as a special case in whoever reads it.
    let dropped_fields = match subject {
        Subject::Grid => grid_dropped_fields,
        Subject::Record => Vec::new(),
    };
    let total_fields = LEDGER_COLUMNS.len() + LEDGER_PAGE_KEYS.len();
    let subject_floor = region(RegionRole::Subject).min_rows;
    let apparatus_rows = region(RegionRole::Apparatus).min_rows;

    let mut dropped: Vec<RegionRole> = Vec::new();
    let mut folded = false;
    let mut disclosure;
    let mut disclosure_rows;
    let mut truncated = false;
    let mut rung;
    loop {
        let fold = folded.then(|| measured.clone());
        // 🔴 A ladder rather than one threshold, for the reason `compose_disclosure` and
        // `super::renderer::fold_note` are ladders: this region gets exactly one row and a terminal
        // cuts what does not fit **without saying so**. Adding the subscription to the line made
        // the long form longer than sixty cells in some states, so the rung is chosen by measuring
        // it instead of by a width the line used to fit at. It is chosen **here**, at the top of
        // the pass, so the disclosure below can name what the rung gave up.
        rung = if width >= 60 && rows_needed(&measured.long(), width) <= 1 {
            Rung::Long
        } else if rows_needed(&measured.short(), width) <= 1 {
            Rung::Short
        } else {
            Rung::Bare
        };
        disclosure = compose_disclosure(
            &dropped_fields,
            total_fields,
            &dropped,
            fold.as_ref(),
            width,
            wide,
            Shape {
                subject,
                // A folded provenance carries its counts into the disclosure; only the bottom rung
                // of a provenance that still has a region of its own gives them up.
                counts_dropped: !folded && !rung.carries_counts(),
            },
        );
        disclosure_rows = rows_needed(&disclosure, width).min(disclosure_cap(folded));
        let apparatus = if dropped.contains(&RegionRole::Apparatus) {
            0
        } else {
            apparatus_rows
        };
        let provenance = u16::from(!folded);
        let need = subject_floor + apparatus + provenance + disclosure_rows;
        if need <= height {
            break;
        }
        if !dropped.contains(&RegionRole::Apparatus) {
            dropped.push(RegionRole::Apparatus);
            continue;
        }
        if !folded {
            folded = true;
            continue;
        }
        truncated = true;
        break;
    }

    let mut rows: Vec<(RegionRole, u16)> = Vec::new();
    let apparatus = if dropped.contains(&RegionRole::Apparatus) {
        0
    } else {
        apparatus_rows
    };
    let provenance = u16::from(!folded);
    let spent = apparatus + provenance + disclosure_rows;
    let subject = height.saturating_sub(spent);
    if subject < subject_floor {
        truncated = true;
    }
    // 🔴 The disclosure being cut is the worst cut available, because the disclosure is the line
    // that says what was cut. If the cap bit, the screen has to admit it.
    if rows_needed(&disclosure, width) > disclosure_rows {
        truncated = true;
    }
    if apparatus > 0 {
        rows.push((RegionRole::Apparatus, apparatus));
    }
    if subject > 0 {
        rows.push((RegionRole::Subject, subject));
    }
    if provenance > 0 {
        rows.push((RegionRole::Provenance, provenance));
    }
    if disclosure_rows > 0 {
        rows.push((RegionRole::Disclosure, disclosure_rows));
    }

    Plan {
        rows,
        dropped,
        provenance_folded: folded,
        columns,
        dropped_fields,
        total_fields,
        // The rung was decided at the top of the last pass, beside the disclosure that describes
        // it. Spelling it here would be a second decision, and the two could differ.
        provenance: match rung {
            Rung::Long => measured.long(),
            Rung::Short => measured.short(),
            Rung::Bare => measured.bare(),
        },
        provenance_rung: rung,
        disclosure,
        truncated,
    }
}

/// The declaration for one role.
///
/// # Panics
/// Never: [`REGIONS`] declares all four variants and gate g4 measures that it still does.
#[must_use]
pub fn region(role: RegionRole) -> &'static Region {
    REGIONS
        .iter()
        .find(|region| region.role == role)
        .expect("REGIONS declares every RegionRole; gate g4 measures it")
}
