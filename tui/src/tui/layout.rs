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
    /// Which records of the list the subject region draws.
    ///
    /// 🔴 Decided **here** and not by the region that draws it, for the same reason
    /// [`Plan::dropped_fields`] is: letting go of rows is a declared order, and a screen that chose
    /// its own window would be hiding that order inside a branch (`req/942` §11-3). The property it
    /// carries is that the attended record is one of the ones drawn, and gate g28 is that sentence
    /// fired at [`window`] directly rather than inferred from a picture.
    pub window: Window,
    /// How many rows the subject region's note is given.
    ///
    /// 🔴 **One integer, and deliberately only one** (`req/988` §3-2). The budget is
    /// `super::renderer::note_rows`, which stays where it is, and the ladder that chooses which head
    /// and how many keys fit stays in the region as well — moving those here would be a second
    /// binding table beside the one `req/38` SS999's r6 landed, and two tables disagree the day one
    /// of them is edited. What crosses is the **number**, because the disclosure is composed here
    /// and it is the disclosure that has to say the legend went.
    ///
    /// The region **reads** this rather than recomputing it, so the count the plan disclosed against
    /// and the count the screen drew against are the same number by construction.
    ///
    /// Nought for an opened record, which carries its own closing line and no legend.
    pub note_rows: usize,
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
    /// What this face can do, in the words `super::acts` declares it with.
    ///
    /// A third value of the one classifier and **not a fifth region**: the four regions are
    /// declared, gated (g3, g4, g10) and given rows by priority, and a region that exists only
    /// while a key is held would be a fifth declaration whose priority nothing has ruled on --
    /// the reason already written down in `super::renderer::subject` for the opened record.
    Help,
}

/// Which shape the subject region will take for this reading and this view.
///
/// The empty list keeps its grid, and that is not an inconsistency: an empty grid is still a grid,
/// and the header is what says which columns found nothing.
#[must_use]
pub fn subject_shape(reading: &super::wire::Reading, view: &super::acts::View) -> Subject {
    if view.help {
        Subject::Help
    } else if view.open && !reading.items().is_empty() {
        Subject::Record
    } else {
        Subject::Grid
    }
}

/// The slice of a list the subject region draws.
///
/// 🔴 `first` is an index into the records the **read** carried rather than into the rows the
/// screen has. The region is handed the window, so the region and the line that reports where the
/// reader is standing cannot disagree about which record a row holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Window {
    /// The first record drawn.
    pub first: usize,
    /// How many records are drawn.
    pub rows: usize,
}

/// The list a plan is being resolved for: how many records it holds, and which one is attended.
///
/// 🔴 Two numbers rather than a `super::acts::View`, and the second one is the reason: a plan is
/// resolved for a *reading* as much as for a reader, and a view carries no record count. The pair
/// keeps [`resolve_attended`] a function of exactly what it needs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Attention {
    /// Which record the reader is attending to, as an index into the records the read carried.
    pub selected: usize,
    /// How many records the read carried.
    pub items: usize,
}

/// Which records fit, and which one the window starts at.
///
/// 🔴 **The attended record is inside the window whenever a window exists**, and that is the whole
/// of this function. `super::acts::View::selected` is clamped against the records the list *holds*
/// — all a reducer with no screen in front of it can know — so before this existed the attention
/// could be moved on to a record the region never drew, and the mark then appeared nowhere at all
/// (`req/38` SS999, T-r4-B). Measured at 80x24 against a twenty-eight row ledger in
/// `req/942_artifacts/visual_r5_2026-08-31/`: `G` moved the attention to record 28 and the frame
/// came back identical to the entry frame, character for character.
///
/// The window is a **function of the state**, not a scroll position that is remembered: it sits at
/// the top of the list until the attention passes the bottom edge, then follows it by the least it
/// can. There is nothing to get out of step with, which is why no act had to learn about it and why
/// `super::acts::apply` is untouched.
///
/// 🔴 A capacity of nought is **not** a window with the attention outside it. It is a region with
/// no room for a record at all, and gate g28 holds that apart as the third value rather than
/// folding it into the failing side; the position line is what speaks for the reader there.
#[must_use]
pub fn window(selected: usize, items: usize, capacity: usize) -> Window {
    let rows = items.min(capacity);
    if rows == 0 {
        return Window { first: 0, rows: 0 };
    }
    let selected = selected.min(items - 1);
    let first = if selected < rows {
        0
    } else {
        selected + 1 - rows
    };
    Window { first, rows }
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

/// The order the regions are let go of, as [`REGIONS`] declares it.
///
/// `Priority::Four` first and `Priority::One` last, ties in the order they are declared. Not every
/// role in it can be let go of — the subject is what the screen is *for* and the disclosure is the
/// line that says what went — so a caller walks this and takes the first role it has a step for.
///
/// 🔴 It exists because [`resolve_attended`]'s loop used to name `RegionRole::Apparatus` and then
/// the fold, in that order, by hand. The hand-written order agreed with the declaration, which is
/// the worst version of the defect: `Region::priority` was declared on all four regions, checked by
/// gate g10 for being *internally* honest, and **read by nothing**. Nothing on the screen, in a
/// test, or in a gate would have said a word on the day the two stopped agreeing. Gate g30 is now
/// the thing that would.
#[must_use]
pub fn letting_go_order() -> Vec<RegionRole> {
    let mut regions = REGIONS;
    // Stable, so regions of one priority keep the order they are declared in. `REGIONS` is in draw
    // order rather than in priority order, so that tie is an accident rather than a ruling — it is
    // harmless only because exactly one of the three `Priority::One` regions has a step at all.
    regions.sort_by_key(|region| std::cmp::Reverse(region.priority));
    regions.iter().map(|region| region.role).collect()
}

/// Which columns fit, and which wire keys are therefore not drawn.
#[must_use]
pub fn columns_for(width: u16) -> (Vec<Column>, Vec<&'static str>) {
    let mut drawn = Vec::new();
    let mut dropped: Vec<&'static str> = Vec::new();
    let mut used = 0u16;
    // 🔴 The declaration decides which column goes first, and this **reads** it. The fold below
    // walks the array and gives up whatever is past the budget, so before this line the order was
    // the order the array happened to be typed in; `LEDGER_COLUMNS` was typed in priority order, so
    // the two agreed, and `Column::priority` was a member no code read. Reordering the array — for
    // readability, for a new column, for anything — would have silently made it a lie.
    //
    // Ascending and stable: `Priority::One` is kept first, and among equals the column declared
    // first is kept first, which is the "last first" order the array says of itself it is in.
    let mut ordered = LEDGER_COLUMNS;
    ordered.sort_by_key(|column| column.priority);
    for column in ordered {
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
    /// How many of the acts this state offers the note is not going to spell — because it was given
    /// nought rows and is not drawn at all.
    ///
    /// 🔴 A member of the description rather than an eighth argument, for the reason the two above
    /// it are. It is the second half of a partition: `super::renderer::note_line` already discloses
    /// the keys it folded when it *is* drawn, and this is what discloses **all** of them when it is
    /// not (`req/988` §3-2).
    keys_not_drawn: usize,
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
        keys_not_drawn,
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
        // The help face draws the declaration, not the wire, so a count of wire fields would be
        // describing a screen nobody is looking at -- the error the record arm exists to avoid.
        Subject::Help => long.push(format!(
            "what this face can do is on the screen; the records are not | {LEDGER_ADDRESS}"
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
    // 🔴 **The other half of the note's disclosure** (`req/988` §3-2). `super::renderer::note_line`
    // says `{n} more keys: gx tui --help` when it spells some of them and runs out of room; there
    // was no mouth at all for the case where it is given **nought rows** and spells none — the
    // shapes `super::renderer::note_rows` names as its own bounded defect and gate g26 pins to
    // exactly the diagonal where the records fill the body. Seven declared acts left the screen and
    // nothing said a word.
    //
    // With this clause the face keeps a partition rather than a count: **declared = spelled +
    // disclosed**, at every width, height and row count, with no third bucket for the ones that
    // quietly went. Gate g34 is that sentence. The address is `HELP_ADDRESS`, which the consumer's
    // gate g12c holds to naming every declared act — so it is an address that answers rather than a
    // wave at a cut.
    if keys_not_drawn > 0 {
        long.push(format!(
            "{keys_not_drawn} keys not drawn: {}",
            super::renderer::HELP_ADDRESS
        ));
    }
    // 🔴 **The region clause, with its sign inverted** (`req/988` §3-1). It said what was let go of
    // and said nothing at all when nothing was, so a screen with all four of its regions drawn was
    // the one screen that never named a single one of them — four roles declared, gated by g3/g4/g6
    // and g10, and invisible to the reader they were declared for. `gitui` draws `Status | Log |
    // Files` at every size; this face had the vocabulary and never spelled it.
    //
    // It is **not a fifth region and not a new row**: it is this clause, made total. The words are
    // `RegionRole::short()`, the same declared function the dropped half already spells, so there
    // is no second vocabulary and no hand-written abbreviation for gate g6 to catch.
    //
    // 🔴 **In the long form only, and that is a measured retreat rather than a preference.** The
    // first build of this put it in the short form too. At forty by ten the short form is what is
    // chosen, the clause pushed it from two rows to three, and the extra row came out of the
    // ladder: the apparatus region was **dropped from a screen that had been holding all four** —
    // caught by P4, which exists to hold exactly that. `req/988` §5 wrote the falsifier before the
    // measurement ("if the rail costs a region at forty by ten, the rail comes out"), and this is
    // it being honoured. The long form is only ever chosen when it fits whole, so the rail costs
    // nothing where it is drawn, and below that width the screen still names what it **dropped**.
    //
    // **Named ceiling**: `kept` is read from the ladder's own decisions — what was dropped, and
    // whether the provenance folded — and not from the row counts, because those are settled after
    // this line runs and the disclosure's height is one of the inputs to settling them. On a screen
    // too small for even the floor a region can therefore be named here and given nought rows
    // below; that screen sets `truncated`, so it is a marked cut rather than a silent one.
    let kept: Vec<&str> = REGIONS
        .iter()
        .filter(|region| {
            !dropped_regions.contains(&region.role)
                && !(region.role == RegionRole::Provenance && fold.is_some())
        })
        .map(|region| region.role.short())
        .collect();
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
    // 🔴 **The rail is offered last and only if it is free.** Two candidate long forms are built,
    // the one with the rail is preferred, and it is taken **only when it fits in the same rows the
    // form without it would have taken**. So the clause this lane adds can never displace a clause
    // that was already there.
    //
    // Measured, not assumed. The first build of this put the rail in unconditionally: at
    // forty-six cells the longer line stopped fitting the cap, the whole disclosure fell to its
    // short form, and `LEDGER_ADDRESS` — which only the long form spells — left the screen
    // entirely. A rail that says which parts the screen is made of, bought by deleting the address
    // of the page, is a bad trade at any width. Gate g35 counts the widths where the address is on
    // no row at all, which is how this was caught.
    let mut with_rail = long.clone();
    with_rail.push(format!("screen: {}", kept.join(" ")));
    let with_rail = with_rail.join(" | ");
    let long = long.join(" | ");

    let cap = disclosure_cap(fold.is_some());
    if wide || rows_needed(&with_rail, width) <= cap {
        return with_rail;
    }
    if rows_needed(&long, width) <= cap {
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
        Subject::Help => "help open".to_string(),
    };
    let counts = if counts_dropped {
        format!(" | counts cut, {NO_ADDRESS_PHRASE}")
    } else {
        String::new()
    };
    // The legend vanishing whole is a loss the reader can act on and the address is the act, so it
    // is spelled in both forms — unlike the rail above, which is a long-form claim.
    let keys = if keys_not_drawn > 0 {
        format!(
            " | {keys_not_drawn} keys not drawn: {}",
            super::renderer::HELP_ADDRESS
        )
    } else {
        String::new()
    };
    let mut short = format!(
        "{head}{keys} | {} routes | {} regions not drawn{named}{counts} | gx tui --wide",
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

/// Resolve the grid, for a reading with no records in it.
///
/// 🔴 Kept as its own name because the plan's other members — the columns, the dropped set, the
/// disclosure, the provenance's rung — do not depend on the list at all, and the gates that measure
/// those ask for them by this name. A frame that is going to be **drawn** goes through
/// [`resolve_attended`], which is the only one of the two that can fill in the window.
#[must_use]
pub fn resolve(width: u16, height: u16, measured: &Measured, wide: bool, subject: Subject) -> Plan {
    resolve_attended(width, height, measured, wide, subject, Attention::default())
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
///
/// 🔴 And the window the subject region draws is decided here too, at the bottom, once the rows are
/// known. `req/942` §11-3 is the reason it is not decided by the region: a screen that picks its own
/// slice of the list cannot be asked what it let go of.
#[must_use]
pub fn resolve_attended(
    width: u16,
    height: u16,
    measured: &Measured,
    wide: bool,
    subject: Subject,
    attention: Attention,
) -> Plan {
    // The parameter's name is spent further down on the row count the subject region gets, and the
    // shape is wanted after that point, so it is held here rather than recomputed.
    let shape = subject;
    let (columns, grid_dropped_fields) = columns_for(width);
    // While a record is open no field is dropped by **width**: the record draws every member the
    // wire carried, one per row. So the plan's dropped set is empty, and it is empty as a computed
    // fact rather than as a special case in whoever reads it.
    // The help face is the same case one step further out: it draws no wire value at all, so a set
    // of wire keys it "did not draw" would be counting a grid that is not on the screen.
    let dropped_fields = match subject {
        Subject::Grid => grid_dropped_fields,
        Subject::Record | Subject::Help => Vec::new(),
    };
    let total_fields = LEDGER_COLUMNS.len() + LEDGER_PAGE_KEYS.len();
    let subject_floor = region(RegionRole::Subject).min_rows;
    let apparatus_rows = region(RegionRole::Apparatus).min_rows;

    // The declaration's order, read once. Three passes at most walk it, and it is four elements.
    let order = letting_go_order();

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
                // 🔴 Nought **inside the loop**, and the real count once below it. How many rows the
                // note gets depends on how many rows the subject region gets, which depends on how
                // tall this disclosure is — the order inversion `req/964` §16 named. So the loop
                // settles the shape of the screen without this clause, and the clause is added
                // afterwards against the rows the loop actually produced.
                keys_not_drawn: 0,
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
        // 🔴 Which region goes next is [`letting_go_order`]'s answer and not this loop's. The two
        // steps were spelled here in that order by hand and they were right; what was missing was
        // anything that made them *have* to be. A role with no step is stepped over rather than
        // stopping the walk: "cannot be let go of" and "has already been let go of" are different
        // facts, and treating the first as the second would end the loop at the subject every time.
        let mut stepped = false;
        for &role in &order {
            let took = match role {
                RegionRole::Apparatus if !dropped.contains(&RegionRole::Apparatus) => {
                    dropped.push(RegionRole::Apparatus);
                    true
                }
                RegionRole::Provenance if !folded => {
                    folded = true;
                    true
                }
                _ => false,
            };
            if took {
                stepped = true;
                break;
            }
        }
        if stepped {
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

    // 🔴 The window, decided beside everything else the screen is handed. The region's row count is
    // read back out of the list that was just built, which is the same list `super::renderer::draw`
    // splits the frame by — so the rows the window is computed against are the rows that exist.
    //
    // An opened record has no window: it is one record drawing every member it carries, the rows it
    // gives up are given up by height rather than by count, and its own line is what says how many.
    let subject_rows = rows
        .iter()
        .find(|(role, _)| *role == RegionRole::Subject)
        .map_or(0, |(_, count)| *count) as usize;
    // One row for the header, and the note is paid for out of what the records leave over.
    // `super::renderer::note_rows` is that ruling as a function, and this **reads** it rather than
    // restating it: a second copy of the budget here is a second answer, and the region and the plan
    // would disagree the day one of them was edited.
    //
    // 🔴 `max(1)` is a repair the note's disclosure made necessary and worth naming. `occupied` is
    // what the region draws *before* the note, and for a reading with no records in it that is the
    // one kind-of-nothing row — which `super::renderer::subject` has always passed and this call
    // site had not. The two were computing different budgets for the empty list; it was invisible
    // because the only consumer was a window that is nought rows wide on an empty list either way.
    // Disclosing the number makes it visible, so it is repaired rather than disclosed wrongly.
    let body_rows = subject_rows.saturating_sub(1);
    let note_rows = match shape {
        Subject::Grid => super::renderer::note_rows(attention.items.max(1), body_rows),
        Subject::Record | Subject::Help => 0,
    };
    let window = match shape {
        Subject::Record | Subject::Help => Window::default(),
        Subject::Grid => window(
            attention.selected,
            attention.items,
            body_rows.saturating_sub(note_rows),
        ),
    };

    // 🔴 **The second composition, and the only one there is.** The loop above settled how tall this
    // region is without knowing whether the note would survive, because the note's budget is a
    // function of the rows the loop was still deciding. Now the rows exist, so the count is known,
    // and the one clause that depends on it is added against the screen that was actually produced.
    //
    // The height is **not** recomputed from the longer line: re-running the ladder here would let
    // the disclosure grow itself a row and take it from the records, which is exactly the ruling
    // `super::renderer::note_rows` was written from. The budget stands, and if the longer line no
    // longer fits inside it the cut is marked — by the same check the first composition gets, fired
    // again here, because a cut that happens after a check is a cut nothing checked.
    if note_rows == 0 && matches!(shape, Subject::Grid) {
        let keys_not_drawn = super::renderer::offered(attention.items).len();
        if keys_not_drawn > 0 {
            disclosure = compose_disclosure(
                &dropped_fields,
                total_fields,
                &dropped,
                folded.then(|| measured.clone()).as_ref(),
                width,
                wide,
                Shape {
                    subject: shape,
                    counts_dropped: !folded && !rung.carries_counts(),
                    keys_not_drawn,
                },
            );
            if rows_needed(&disclosure, width) > disclosure_rows {
                truncated = true;
            }
        }
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
        window,
        note_rows,
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
