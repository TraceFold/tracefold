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
        //
        // 🔴 **One, and it was three** (`req/924` §TUI-22, `req/38` SS1049, Owner `#266-T`). The
        // region is now the ledger's **top rail** and it draws one row. The two rows it gave up were
        // carrying, between them, a duplicate and a defect: the breadcrumb row spelled
        // `GET /v1/transformations` a second time when the rail's own title is that address, and
        // `status_reason` was drawn on every frame as `?` on a bed whose `status` is `ok` — which is
        // the face repeating an engine-side collapse of the seven words for nothing (§TUI-22's own
        // new finding: `ok` has no reason, so the truth is `--`). The reason is drawn now exactly
        // when the engine is not `ok`, which is the state it was reserved for, and it is disclosed
        // among the fields not drawn when it is not.
        min_rows: 1,
    },
    Region {
        intent: Intent::RecordsTheEngineProduced,
        role: RegionRole::Subject,
        priority: Priority::One,
        recoverable: Recoverable::Address,
        // A header and three rows. A ledger showing one row is not a smaller ledger, it is a
        // different claim: it reads as "this is what there is".
        //
        // 🔴 **Five, and it was four** (Owner #227, 2026-09-01). The fifth is the heading — which
        // screen this is and which of the three the reader is on ([`heading`]). It is a whole row
        // and it is charged here rather than taken quietly out of the records, because a region
        // that grows its own chrome out of its content is the defect `super::renderer::note_rows`
        // was written against. What the row buys is the answer to *what am I looking at*, which
        // this face could not give at any shape narrower than eighty cells.
        //
        // 🔴 **Four, and it was five** (`req/924` §TUI-22). The row is given back, and the question
        // it bought is still answered: [`heading`] is drawn on the **top rail**, in the apparatus
        // region's one row, beside the address it is the last segment of. Nothing is lost and one
        // record is gained, which is the trade this whole lane is.
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

/// The address for the long disclosure **from a shell**, for the one reading where the in-place act
/// is clamped.
///
/// 🔴 Declared beside the page's address rather than typed into the line that spells it: this is
/// the fall-back road, and a fall-back spelled inline is a road no gate can find.
pub const WIDE_ADDRESS: &str = "gx tui --wide";

/// The two routes this face reads and does not draw, declared once so the count is never quietly
/// zero (`req/942` §2: the range this face does not cover is part of what it must say).
pub const READ_NOT_DRAWN: [&str; 2] = ["GET /v1/candidates", "GET /v1/escalations"];

/// The cells the ledger's enclosure takes out of a rail's row: a corner and a space at each end.
///
/// 🔴 Declared once because two places need the same number — the disclosure is **composed**
/// against `width - FRAME_MARGIN` and `super::renderer` **draws** the corners into those cells. A
/// second spelling is how a line comes to be composed against one width and drawn at another.
pub const FRAME_MARGIN: u16 = 4;

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
    /// The engine's own line about itself, key by key, for the top rail.
    ///
    /// 🔴 **Here since `req/924` §TUI-22, and here rather than in the region that draws it.** The
    /// apparatus is one row now, so the rail has to choose between the page's address and the
    /// engine's caveats when a terminal is narrow — and *whoever makes that choice has to be the
    /// one that composes the disclosure*, because the clause naming what the rail let go of is
    /// composed in [`resolve_attended`]. Leaving the choice in the region would have put the two
    /// on opposite sides of the seam: the region cutting text off the right edge with a `~` and the
    /// disclosure, one row down, saying nothing at all. That is the defect this whole module is
    /// ordered to prevent, and it is the defect the first cut of this lane actually shipped —
    /// measured on a real terminal at 80, 66, 46 and 40 cells, where `ledger_agrees yes` left the
    /// screen behind a `~` and no line said so.
    ///
    /// Pairs, not one string, so [`heading`] can drop them one at a time from the end.
    pub engine: Vec<(String, String)>,
}

impl Measured {
    /// The long form, for a screen wide enough to carry it.
    ///
    /// 🔴 **The date is gone and the words `read` and `at` with it** (`req/924` §TUI-21, `req/38`
    /// SS1048, Owner `#265-T`). `2026-09-01T04:28:41Z` spends eleven cells telling a reader which
    /// day it is on a line whose whole point is *how fresh is this*, and the four measured facts —
    /// how many routes, when, how slow, what they answered — are unchanged. The clock is kept, so
    /// nothing that could go stale went; what went is the part that cannot.
    ///
    /// **Named ceiling**: a session left open across midnight now reads a clock with no day behind
    /// it. The full stamp is still in [`Measured::read_at`] and the help face draws this line, so
    /// closing it properly means a rung that spells the date only when the day has turned — which
    /// needs a clock this face does not keep between frames.
    #[must_use]
    pub fn long(&self) -> String {
        format!(
            "{} {} routes {} | worst {}ms | {} | {}",
            self.link.link.mark(),
            self.routes,
            self.clock(),
            self.worst_ms,
            self.statuses,
            // 🔴 The long form of the link, and it is **kept** although shortening it is the last
            // easy cell on this row. `LIVE_BADGE` is Owner #227's ruling — a face that says `LIVE`
            // must say *whose* events it counts — and this lane has no ruling that reverses it.
            // The residual ink on this row is a caveat and a badge, which `INHERITED_PRINCIPLES`
            // §削る前に3分類 puts outside the word count.
            self.link.long()
        )
    }

    /// The reading's time of day: what the shorter rungs already spell, named once.
    fn clock(&self) -> &str {
        self.read_at.split('T').next_back().unwrap_or(&self.read_at)
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
    /// The top rail's cells: the page's address, and which of the three screens this is.
    ///
    /// 🔴 Drawn by the **apparatus** region since `req/924` §TUI-22, in the one row that region
    /// now has, rather than by the subject region in a row taken off the ledger. See [`heading`].
    pub heading: Vec<HeadingCell>,
    /// Whether the ledger's enclosure is drawn: a corner at each end of each rail.
    ///
    /// 🔴 Decided **here** rather than by either region that draws a corner, for the reason every
    /// other member of this struct is: two regions draw the enclosure and one line discloses its
    /// absence, so a screen where the three disagreed would be a screen whose own account of
    /// itself is false. The disclosure is composed against [`FRAME_MARGIN`] fewer cells when this
    /// is true, which is what makes the corners free rather than something that pushes a clause
    /// off the row.
    pub framed: bool,
    /// The provenance region's text, when it has a region.
    pub provenance: String,
    /// The provenance in full, at every width, whether or not the region drew it.
    ///
    /// 🔴 **The other half of the fold** (Owner #227: *"do not throw the present display away —
    /// separate what is always shown from what is disclosed on demand"*). The region's own text is
    /// a rung of a ladder and the bottom rung gives the connection's counts up; this is the whole
    /// of it, and the help face is where a reader reaches it. Composed here rather than in the
    /// region so that the standing line and the disclosed line come from one measurement.
    pub provenance_full: String,
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
    /// The row budget [`window`] was actually asked for, before the item count capped it.
    ///
    /// 🔴 **Not recoverable from `window` alone** (`req/984` §10-33, independent audit
    /// 2026-09-01): [`window`]'s own body is `items.min(capacity)`, so `window.rows <=
    /// items.len()` always holds and a reader who compares `items.len()` against `window.rows`
    /// to ask "was there spare room?" gets the same answer -- no -- every time, whether or not
    /// there actually was. A caller that needs to know whether the region had rows to spare (for
    /// example, whether a summary line can be added for free) has to be handed the number that
    /// was capped, not the number that survived the cap. Nought for an opened record, the same as
    /// [`note_rows`](Self::note_rows).
    pub grid_capacity: usize,
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

/// Every shape the subject region takes. Gate g41 requires each one to have a name and requires the
/// heading to spell all three.
pub const SUBJECTS: [Subject; 3] = [Subject::Grid, Subject::Record, Subject::Help];

impl Subject {
    /// The name a reader sees for this shape, in the heading.
    ///
    /// 🔴 `list` and not `grid`. The three names are what a reader would call the three screens;
    /// `Grid` is what the code calls the shape of one of them, and a screen that spells the code's
    /// word for its own internals is a screen that has forgotten who it is drawn for.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Subject::Grid => "list",
            Subject::Record => "record",
            Subject::Help => "help",
        }
    }
}

/// One cell of the heading: a word, and what that word **is**.
///
/// 🔴 A role and never a colour, for the reason the whole of this module exists: the heading is
/// composed here, above the seam, so `super::renderer` binds the medium and decides nothing. The
/// attended shape wears [`super::tokens::Role::Attend`], which resolves to a swap of fore and
/// ground — so the reader is told which of the three screens they are on **in every tier**,
/// including `mono`, and not by a hue.
#[derive(Clone, Debug)]
pub struct HeadingCell {
    /// The word.
    pub text: String,
    /// What it is.
    pub role: super::tokens::Role,
}

/// The page's own name: the last segment of the address it is read from.
///
/// 🔴 Derived rather than typed. A second string saying `transformations` beside
/// [`LEDGER_ADDRESS`] is a second name for one thing, and the day the route moves the heading would
/// keep announcing the old one.
#[must_use]
pub fn page_name() -> &'static str {
    LEDGER_ADDRESS.rsplit('/').next().unwrap_or(LEDGER_ADDRESS)
}

/// The heading strip: which screen this is, and which of the three the reader is on.
///
/// 🔴 **The one thing this face had no words for at all.** Four regions were declared, given
/// intents, gated by g3/g4/g6/g10 — and at forty by ten not one of their names reached a row. The
/// side-by-side against twelve reference faces measured the result as `border 0.0 / tab 0 / region
/// heading 0` at all five common shapes, against six of the twelve that keep a named structure at
/// forty by ten (`req/942_artifacts/sidebyside_round3_2026-09-01.md` §3-1). The reader could not
/// answer *what am I looking at* without pressing a key first.
///
/// It costs **one row**, taken from the subject region's own floor
/// ([`REGIONS`], `min_rows` 4 -> 5), and it is drawn at every shape and in all three subject
/// shapes. That is the trade, said plainly: one record, for a screen that knows its own name.
///
/// It carries **no position**. `record N of M` is the note's floor and `req/38` SS999 T-r4-A2 is
/// the ruling that put it there; a second copy of the same two numbers up here would be the one
/// thing this face spends cells on last, and gates g29/g39 would then be measuring a line that no
/// longer has to carry anything.
///
/// A ladder rather than a width test, like every other line in this face: the long form is offered
/// first and taken only if it fits whole.
/// 🔴 **The tab strip is gone and the address is here instead** (`req/924` §TUI-22, `req/38`
/// SS1049, Owner `#266-T`). `transformations │ list record help` named all three screens on every
/// frame, and two of the three names were **destinations the key legend already spells** — `ret
/// open` goes to the record and `?` goes to help. The screen was saying where a reader can go in
/// two places. What is not a duplicate is *which of the three the reader is on*, so that one name
/// stays and wears [`super::tokens::Role::Attend`], which is a swap of fore and ground and
/// therefore survives `mono`.
///
/// The cells the two deleted names paid for are spent on `LEDGER_ADDRESS` in full. This is the
/// **one** row that spells it: the note used to spell it, the disclosure used to spell it, and the
/// apparatus used to spell it on a breadcrumb row of its own, so `GET` was on the screen five
/// times over. A title says where everything under it came from, once.
/// 🔴 **A ladder, and it is one because the rail is one row and a terminal cuts from the right.**
/// The rungs are, longest first:
///
/// 1. the address, the boundary, the screen's name, and every key of the engine's own line;
/// 2. the same with the engine's line one key shorter, and so on down to none;
/// 3. the page's **name** in place of the address, with the engine's line back at full length,
///    and the same walk down again;
/// 4. the screen's name alone.
///
/// The order is the ruling and not a preference. The address gives way **after** the engine's
/// caveats because the disclosure can spell an address and cannot re-measure a caveat: `status`
/// and `ledger_agrees` are what `req/924` §TUI-22 classified as the things that may be folded and
/// may not be discarded, and `LEDGER_ADDRESS` is a road the row below can carry. What the rail
/// drops is counted by [`heading_engine_dropped`] and named in the disclosure, so no key leaves
/// this row in silence — which is what the first cut of this lane did, behind a `~`, at four of
/// the seven shapes.
#[must_use]
pub fn heading(subject: Subject, width: u16, engine: &[(String, String)]) -> Vec<HeadingCell> {
    let here = HeadingCell {
        text: subject.name().to_string(),
        role: super::tokens::Role::Attend,
    };
    let strip = |kept: usize| -> Vec<HeadingCell> {
        engine
            .iter()
            .take(kept)
            .map(|(key, value)| HeadingCell {
                text: format!("{key} {value}"),
                role: super::tokens::Role::Head,
            })
            .collect()
    };
    // 🔴 The glyph, and the whole of what it buys: without it the page's address and the screen's
    // name are two phrases in a row and a reader has to be told which is which. With it they are
    // two parts of one row. The words it replaces are the labels — `page:`, `view:` — that a face
    // without a boundary mark has to spell.
    let bar = HeadingCell {
        text: super::tokens::RULE.to_string(),
        role: super::tokens::Role::Quiet,
    };
    let titles = [
        HeadingCell {
            text: LEDGER_ADDRESS.to_string(),
            role: super::tokens::Role::Head,
        },
        HeadingCell {
            text: page_name().to_string(),
            role: super::tokens::Role::Head,
        },
    ];
    for title in titles {
        for kept in (0..=engine.len()).rev() {
            let mut cells = vec![title.clone(), bar.clone(), here.clone()];
            cells.extend(strip(kept));
            if heading_width(&cells) <= width as usize {
                return cells;
            }
        }
    }
    vec![here]
}

/// How many of the engine's own keys the rail could not carry.
///
/// 🔴 Read from the cells the rail is going to draw rather than recomputed from a width, for the
/// reason [`heading_carries_address`] is: the row that drops them and the row that names the drop
/// have to be reading one decision.
#[must_use]
pub fn heading_engine_dropped(cells: &[HeadingCell], engine: &[(String, String)]) -> usize {
    engine
        .iter()
        .filter(|(key, value)| {
            let spelled = format!("{key} {value}");
            !cells.iter().any(|cell| cell.text == spelled)
        })
        .count()
}

/// Whether a heading strip of these cells spells [`LEDGER_ADDRESS`] in full.
///
/// 🔴 **Read from the cells rather than from a width test.** Whether the top rail carried the
/// address decides whether the disclosure has to, and a second width comparison here would be a
/// second decision for the two rows to disagree about — the defect `Subject` was factored out to
/// close. Gate g40's property (*the page's address is on some row at every shape*) is what this
/// keeps true now that the apparatus is one row rather than three.
#[must_use]
pub fn heading_carries_address(cells: &[HeadingCell]) -> bool {
    cells.iter().any(|cell| cell.text == LEDGER_ADDRESS)
}

/// How many cells a heading needs: the words, and one space between each pair.
#[must_use]
pub fn heading_width(cells: &[HeadingCell]) -> usize {
    cells
        .iter()
        .map(|cell| cell.text.chars().count())
        .sum::<usize>()
        + cells.len().saturating_sub(1)
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

/// Whether every one of these already-resolved marks agrees, and what they agree on.
///
/// 🔴 **Layer-independent** (`req/984` §10-33): this has no idea which wire key, column width or
/// terminal shape produced the strings, and it would answer the same question about a column of
/// usernames or a column of exit codes. Fewer than two marks proves nothing repeats, so it answers
/// [`None`] there even when the lone mark is not empty — the same ruling [`resolve_shared`] is
/// built on, stated once rather than at each of its call sites.
fn uniform(marks: &[String]) -> Option<&str> {
    let (first, rest) = marks.split_first()?;
    if rest.is_empty() || !rest.iter().all(|mark| mark == first) {
        return None;
    }
    Some(first.as_str())
}

/// Split the columns [`columns_for`] already chose into the ones that vary across the rows a
/// screen is about to draw and the ones every one of those rows already says the same thing —
/// so the constant ones can be said once instead of on every row.
///
/// 🔴 **Hoists the draw, never the meaning** (`req/38` SS1019: 667 of 1,540 cells at 120x32 were
/// exactly this repetition). `rows[r][c]` is already the exact text a cell would put on screen —
/// `?` for an unknown, `--` for an absent one — so a hoisted column keeps whichever of the seven
/// marks for nothing it was without this function ever having to know the seven exist; the
/// distinction survives for free because two different marks are, by definition, not the one
/// value [`uniform`] found agreement on.
///
/// A column of fewer than two rows, or whose marks disagree, is left in the first half of the
/// return value untouched — so a caller that had no reason to hoist anything sees exactly the
/// columns [`columns_for`] gave it, byte for byte, which is what keeps the twenty-five call sites
/// that already read [`columns_for`] or [`Plan::columns`] unhurt by this function's existence
/// (`req/984` §10-33: "resolve_shared() 新設で既存25 call site 無傷").
///
/// `rows` is addressed `rows[row][column_index]`, `column_index` matching `columns`'s order — the
/// same order a caller already builds when it resolves one cell per column per row.
#[must_use]
pub fn resolve_shared(
    columns: &[Column],
    rows: &[Vec<String>],
) -> (Vec<Column>, Vec<(&'static str, String)>) {
    let mut kept = Vec::new();
    let mut shared = Vec::new();
    for (index, column) in columns.iter().enumerate() {
        let marks: Vec<String> = rows.iter().map(|row| row[index].clone()).collect();
        match uniform(&marks) {
            Some(mark) => shared.push((column.key, mark.to_string())),
            None => kept.push(*column),
        }
    }
    (kept, shared)
}

/// How many rows a line of text needs at this width, word-wrapped.
#[must_use]
pub fn rows_needed(text: &str, width: u16) -> u16 {
    if width == 0 {
        return u16::MAX;
    }
    wrap(text, width).len() as u16
}

/// The phrases this face never breaks across two rows.
///
/// 🔴 **A method and a path are one name.** [`wrap`] breaks at spaces, which is right for prose
/// and wrong for `GET /v1/candidates`: measured at a hundred cells against a live engine, the
/// disclosure ended one row with `... read and not drawn: GET` and opened the next with
/// `/v1/candidates,`. That is not a wrap a reader undoes in their head -- it reads as a defect, and
/// the ruling this face is held to asks for **one** cutting policy rather than two.
///
/// Derived from the declarations that already spell these roads rather than typed out again, so an
/// address that moves carries its own protection with it. `super::renderer::HELP_ADDRESS` is
/// reached across the seam for the same reason [`resolve_attended`] reaches for `offered`: there is
/// one vocabulary in this face, and this module is where placement is decided.
#[must_use]
pub fn unbreakable() -> Vec<&'static str> {
    let mut atoms = vec![LEDGER_ADDRESS, WIDE_ADDRESS, super::renderer::HELP_ADDRESS];
    atoms.extend(READ_NOT_DRAWN);
    // Longest first. A shorter phrase that begins the same way would otherwise be taken first and
    // the remainder of the longer one would break exactly where this exists to stop it.
    atoms.sort_by_key(|atom| std::cmp::Reverse(atom.len()));
    atoms
}

/// The units [`wrap`] may break between: the words of the text, with the declared unbreakable
/// phrases rejoined.
///
/// 🔴 Built by **rejoining** `split(' ')` rather than by scanning the string, so everything this
/// function does not protect keeps exactly the behaviour `wrap` had before it existed -- including
/// the empty word a double space produces, which is how `super::renderer::note_line` groups its
/// keys without punctuating them. A scan would have quietly changed the width of every line in this
/// face to repair one of them.
///
/// A trailing `,` or `:` that a clause added travels with the phrase. Without that, the comma in
/// `GET /v1/candidates, GET /v1/escalations` takes the phrase back out of the set and the row
/// breaks inside it again -- the repaired defect surviving in the one place it is actually spelled.
#[must_use]
fn units(text: &str) -> Vec<String> {
    let atoms = unbreakable();
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::new();
    let mut at = 0;
    'word: while at < words.len() {
        for atom in &atoms {
            let span = atom.split(' ').count();
            if span < 2 || at + span > words.len() {
                continue;
            }
            let joined = words[at..at + span].join(" ");
            if joined.starts_with(atom)
                && joined[atom.len()..]
                    .chars()
                    .all(|mark| mark == ',' || mark == ':')
            {
                out.push(joined);
                at += span;
                continue 'word;
            }
        }
        out.push(words[at].to_string());
        at += 1;
    }
    out
}

/// Word-wrap, breaking inside a word only when the word is wider than the screen, and never inside
/// one of the phrases [`unbreakable`] declares.
#[must_use]
pub fn wrap(text: &str, width: u16) -> Vec<String> {
    let width = width.max(1) as usize;
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in units(text) {
        let mut word = word.as_str();
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
struct Shape<'a> {
    subject: Subject,
    counts_dropped: bool,
    /// Where a reader goes for the keys this screen did not spell.
    ///
    /// 🔴 **The address has to be reachable from where the reader is standing** (`req/942_artifacts/
    /// sidebyside_round3_2026-09-01.md` §8-2). `?` opens the help face in place and has since
    /// `req/984` §8-7 landed `Act::Help`, and every line on the screen went on naming
    /// `gx tui --help` — a command that requires **leaving the process**. The mechanism moved on
    /// and the words did not, which is the exact mirror of `Intent::sentence` being declared and
    /// called by nothing: there, a declaration nothing read; here, a capability nothing announced.
    ///
    /// It is the caller's because the caller knows how many records there are, and
    /// `super::acts::grounded` clamps `?` on a list with nothing in it — so on that one screen the
    /// shell command is the honest address and this carries it.
    keys_address: &'a str,
    /// The same question for the long disclosure: `w` in place, or the flag from a shell.
    wide_address: &'a str,
    /// How many of the acts this state offers the note is not going to spell — because it was given
    /// nought rows and is not drawn at all.
    ///
    /// 🔴 A member of the description rather than an eighth argument, for the reason the two above
    /// it are. It is the second half of a partition: `super::renderer::note_line` already discloses
    /// the keys it folded when it *is* drawn, and this is what discloses **all** of them when it is
    /// not (`req/988` §3-2).
    keys_not_drawn: usize,
    /// Whether the top rail spelled [`LEDGER_ADDRESS`] in full on this screen.
    ///
    /// 🔴 **The address is spelled once and this is how the second spelling is refused**
    /// (`req/924` §TUI-22: `GET` was on the screen five times). The rule is the one already
    /// written into `super::renderer::Rung::keeps` — *what a line gives up may be given up only if
    /// the same screen spells it somewhere else* — read from the other end: this clause is
    /// **added** only when nothing above it carried the address. At the shapes where the apparatus
    /// is let go of by its declared priority, or where the rail was too narrow to hold the address,
    /// that is exactly what happens, so gate g40's property still holds.
    address_above: bool,
    /// Whether the ledger's enclosure is drawn at this shape.
    ///
    /// 🔴 A renderer cannot `invert`, so what it owes is to name what it let go of, and the
    /// enclosure carries meaning: it is what says where the ledger begins and ends now that no row
    /// spells `screen: apparatus subject provenance disclosure`. Dropping it at a width that cannot
    /// hold it is right; dropping it **silently** is the one drop this face is not allowed to make.
    framed: bool,
    /// How many of the engine's own keys the top rail had no cells for.
    ///
    /// 🔴 **The caveat clause, and it exists because the first cut of this lane lost one.**
    /// `req/924` §TUI-22 classified `status ok` and `ledger_agrees yes` as things that may be
    /// folded and may **not** be discarded; the rail is one row and at four of the seven measured
    /// shapes `ledger_agrees yes` went off the right edge behind a `~` with no line saying so.
    /// A marked cut is better than a silent one and it is still not disclosure.
    engine_dropped: usize,
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
        keys_address,
        wide_address,
        keys_not_drawn,
        address_above,
        framed,
        engine_dropped,
    } = shape;
    // 🔴 One spelling of the page's address on the whole screen (`req/924` §TUI-22). Empty when the
    // top rail is carrying it, which is every shape wide enough to hold it with the apparatus drawn.
    let road = if address_above {
        String::new()
    } else {
        format!(" | {LEDGER_ADDRESS}")
    };
    let mut long: Vec<String> = Vec::new();
    // 🔴 The field count belongs to the **grid**. While a record is open there is no grid, every
    // member the wire carried is a row of its own, and the record's own line is what says how many
    // of them the height allowed. Saying `4 of 11 fields not drawn` over a record that is drawing
    // all eleven is the disclosure describing a screen that is not there.
    match subject {
        Subject::Grid => {
            if !dropped_fields.is_empty() {
                long.push(format!(
                    "{} of {total_fields} fields not drawn{road}",
                    dropped_fields.len()
                ));
            }
        }
        Subject::Record => long.push(format!(
            "a record is open: its own line counts what it drew{road}"
        )),
        // The help face draws the declaration, not the wire, so a count of wire fields would be
        // describing a screen nobody is looking at -- the error the record arm exists to avoid.
        Subject::Help => long.push(format!(
            "what this face can do is on the screen; the records are not{road}"
        )),
    }
    // 🔴 **The folded provenance is spelled second, before every other clause.** It carries
    // [`NO_ADDRESS_PHRASE`], and the facts it describes are the only ones on this screen that a
    // second read cannot recover — so of everything this line says, it is the sentence that must
    // not be the one a cut takes. A terminal cuts from the end, and at forty by six the cut landed
    // in the middle of `no address, measured here`, which reads as a fact with an address that
    // simply ran out of room. Same ruling as [`Measured::bare`] leading with the connection's
    // mark: what cannot be recovered goes at the front.
    if let Some(measured) = fold {
        long.push(measured.folded());
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
        long.push(format!("{keys_not_drawn} keys not drawn: {keys_address}"));
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
    // 🔴 **The count stays on the screen and the two names move behind `?`** (`req/924` §TUI-21:
    // *the numbers stay, the names may go to the escape hatch — but do not call it moved until a
    // gate has confirmed the hatch actually lists them*). `GET /v1/candidates,
    // GET /v1/escalations` is forty cells of a line whose job is a count, and both names are
    // drawn in full by `super::renderer::help_lines`, which gate `g61` measures. A hatch that is
    // empty is not a hatch, and this clause would then be a deletion wearing a signpost's face.
    long.push(format!(
        "{} routes read and not drawn -> {keys_address}",
        READ_NOT_DRAWN.len()
    ));
    // 🔴 The engine's own keys the rail had no room for. The route is the region's declared
    // `Recoverable::Route`, so this names the road as well as the count — these are caveats, and a
    // caveat with no way back is a fact the screen destroyed.
    if engine_dropped > 0 {
        let route = match region(RegionRole::Apparatus).recoverable {
            Recoverable::Route(route) => route,
            _ => LEDGER_ADDRESS,
        };
        long.push(format!("{engine_dropped} engine keys not drawn | {route}"));
    }
    // 🔴 The enclosure, when it is not on the screen. It is a mark that carries meaning, so its
    // absence is a fact about what this frame let go of and not a matter of taste.
    if !framed {
        long.push("frame not drawn at this width".to_string());
    }
    // 🔴 **The region rail is deleted, and it is deleted *because* the enclosure is drawn**
    // (`req/924` §TUI-22, `req/38` SS1049, Owner `#266-T`). It read
    // `screen: apparatus subject provenance disclosure` — the face spelling the names of its own
    // internal parts, on a row, in words a reader cannot act on. §TUI-21 classified it as
    // persuasion and §TUI-22 made the deletion conditional on the thing that replaces it: a
    // boundary a reader can *see* says where one part ends and the next begins, which is the whole
    // of what those four words were for.
    //
    // 🔴 The two move **together**, and that is the admission test for the glyphs
    // (`INHERITED_PRINCIPLES` §3c-③''): a mark earns its cells by carrying meaning and thereby
    // deleting words. Adding the corners and keeping this clause would have been a mark beside a
    // word that stays, which is weight. Gate `g60` is that pairing fired as an assertion, so the
    // clause cannot come back while the corners are drawn and the corners cannot be drawn beside
    // it.
    //
    // What the deleted rail *did* carry that nothing else did is which regions are **present**; the
    // clause below still names the ones that are **absent**, which is the half a reader can act on.
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
    // 🔴 **The address is in the short form too, and it is the one clause here that had to be
    // bought rather than written.** The short form gave the count and kept the address for the
    // long one, which held while the apparatus region drew the page's address in its own spare
    // cells. It does not hold now: the heading costs the subject region a row
    // ([`REGIONS`]), so at forty by ten the apparatus is let go of by its declared priority — and
    // it took the only spelling of `GET /v1/transformations` on the screen with it. Gate g40
    // measured exactly that and named the shape. A count of fields nobody can go and read is a
    // number without a road, so the road is spelled here and the row it costs is paid.
    //
    // 🔴 And it is **conditional** since `req/924` §TUI-22, for the reason `road` carries: at the
    // shapes where the top rail holds the address, spelling it again here is the second of the
    // five spellings that lane was opened to delete. `address_above` is read from the rail's own
    // cells, so the two rows cannot disagree about whether the road is on the screen.
    let head = match subject {
        Subject::Grid => format!("{}/{total_fields} fields{road}", dropped_fields.len()),
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
        format!(" | {keys_not_drawn} keys not drawn: {keys_address}")
    } else {
        String::new()
    };
    // The same order as the long form, and for the same reason: the unrecoverable facts lead, so
    // that a cut takes a clause a reader can get back rather than the one they cannot.
    let folded = match fold {
        Some(measured) => format!("{} | ", measured.folded()),
        None => String::new(),
    };
    // 🔴 **Largest loss first, because a terminal cuts from the end.** The order is: what cannot be
    // recovered at all (the folded provenance), then a whole region that is not on the screen,
    // then the fields the rows gave up and the address that answers for them, then the keys, then
    // the routes, then the road to the long form. It was written the other way round and at forty
    // by eight the cut landed inside `1 regions not drawn: apparatus` — the screen dropped a
    // region and the sentence saying which was itself the thing that did not fit, which P4 caught.
    // The enclosure is named in the short form too: it is dropped most often at exactly the widths
    // this form is chosen at, and a mark that carries meaning cannot go quietly at the shapes where
    // it always goes.
    let frame = if framed { "" } else { " | no frame" };
    // The caveats are in the short form too, and for the reason they are in the long one: they
    // cannot be re-measured from anything else on the screen.
    let engine = if engine_dropped > 0 {
        format!(" | {engine_dropped} engine keys")
    } else {
        String::new()
    };
    format!(
        "{folded}{} regions not drawn{named} | {head}{keys}{counts}{engine}{frame} | {} routes | {wide_address}",
        dropped_regions.len(),
        READ_NOT_DRAWN.len()
    )
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
    // 🔴 Which road out is real **on this reading**, asked once and handed to every line that
    // spells one. `super::acts::grounded` clamps `?` and `w` on a list with nothing in it, so the
    // in-place keys are the honest address exactly when the reducer offers them, and the shell
    // command is the honest address when it does not. `super::renderer::offered` is the same
    // function the note asks, so the note and the disclosure cannot name two different roads.
    let offered = super::renderer::offered(attention.items);
    let keys_address = if offered.contains(&super::acts::Act::Help) {
        super::renderer::spelled(super::acts::Act::Help)
    } else {
        super::renderer::HELP_ADDRESS.to_string()
    };
    // 🔴 **The flag, and `w` is deliberately not put here — a named ceiling, measured rather than
    // preferred.** `w` reaches the long form in place and the short disclosure still sends the
    // reader to a shell, which is the same defect the help address above repairs. Spelling
    // `wide:w` here closes it and breaks something worse: `super::renderer::spelled(Act::Wide)` is
    // character for character what the note spells, so the disclosure would put a *second* act on
    // the screen that the note is still counting among the ones it folded away — and gate g34's
    // partition (`declared = spelled + disclosed`) came to one more than there are, in 81 of 493
    // shapes, when this was tried. The note cannot see the disclosure, so the honest repair is a
    // parameter on `super::renderer::note_line` naming the acts the rest of the screen spells;
    // that is the upgrade path and this lane did not take it. The flag is true, it is simply not
    // the cheapest road. The help face names `w` in full, and the note spells it wherever it fits.
    let wide_address = WIDE_ADDRESS.to_string();
    let subject_floor = region(RegionRole::Subject).min_rows;
    let apparatus_rows = region(RegionRole::Apparatus).min_rows;
    // 🔴 The top rail, resolved once and read three times: it is what the apparatus draws, it is
    // what decides whether the disclosure has to spell the address, and its width is what decides
    // whether the enclosure's corners fit. Three readings of one decision, which is this module's
    // rule — the alternative is three width tests that disagree.
    //
    // 🔴 The rungs are chosen against `width - FRAME_MARGIN` and **not** against `width`, which is
    // what breaks the circle: whether the enclosure is drawn depends on whether the rail fits with
    // its corners, and which rung the rail takes would otherwise depend on whether the enclosure is
    // drawn. Reserving the corners' cells up front means the chosen rung fits either way, and the
    // enclosure is refused only when even the floor of the ladder overflowed.
    let head_cells = heading(shape, width.saturating_sub(FRAME_MARGIN), &measured.engine);
    let address_above = heading_carries_address(&head_cells);
    let engine_dropped = heading_engine_dropped(&head_cells, &measured.engine);
    // The enclosure is never offered where the apparatus has been let go of: there is no top rail
    // to open it with, and a screen closed at the bottom and open at the top is not an enclosure.
    let frame_room = heading_width(&head_cells) + FRAME_MARGIN as usize <= width as usize;

    // The declaration's order, read once. Three passes at most walk it, and it is four elements.
    let order = letting_go_order();

    let mut dropped: Vec<RegionRole> = Vec::new();
    let mut folded = false;
    let mut disclosure;
    let mut disclosure_rows;
    let mut truncated = false;
    let mut rung;
    let mut framed;
    loop {
        let fold = folded.then(|| measured.clone());
        // Decided at the top of the pass beside the provenance's rung, because the clause that
        // discloses its absence is composed in the same call and the row budget below depends on
        // how many rows that composition takes.
        framed = frame_room && !dropped.contains(&RegionRole::Apparatus);
        let inner = if framed {
            width.saturating_sub(FRAME_MARGIN)
        } else {
            width
        };
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
            inner,
            wide,
            Shape {
                subject,
                // A folded provenance carries its counts into the disclosure; only the bottom rung
                // of a provenance that still has a region of its own gives them up.
                counts_dropped: !folded && !rung.carries_counts(),
                keys_address: &keys_address,
                wide_address: &wide_address,
                // 🔴 Nought **inside the loop**, and the real count once below it. How many rows the
                // note gets depends on how many rows the subject region gets, which depends on how
                // tall this disclosure is — the order inversion `req/964` §16 named. So the loop
                // settles the shape of the screen without this clause, and the clause is added
                // afterwards against the rows the loop actually produced.
                keys_not_drawn: 0,
                address_above,
                framed,
                engine_dropped,
            },
        );
        disclosure_rows = rows_needed(&disclosure, inner).min(disclosure_cap(folded));
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
    // that says what was cut. If the cap bit, the screen has to admit it. Measured against the
    // width the line was **composed** at, which is four cells narrower once the enclosure is drawn.
    let inner = if framed {
        width.saturating_sub(FRAME_MARGIN)
    } else {
        width
    };
    if rows_needed(&disclosure, inner) > disclosure_rows {
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
    // 🔴 Two rows, and it was one: the heading and the grid's column header both stand above the
    // records now. The note is paid for out of what is left after both, which is the same ruling
    // one row further down — a region does not fund its own chrome out of its content.
    // 🔴 **One again, and the row is a record** (`req/924` §TUI-22). The heading moved to the top
    // rail, which the apparatus region draws in the row it already had, so this region stands over
    // the grid's column header alone. The row is not saved, it is spent — on the ledger.
    let body_rows = subject_rows.saturating_sub(1);
    let note_rows = match shape {
        Subject::Grid => super::renderer::note_rows(attention.items.max(1), body_rows),
        Subject::Record | Subject::Help => 0,
    };
    // 🔴 Held rather than only fed to `window` below: the cap `window` applies is exactly the
    // fact a caller needs and cannot get back out of `Window` once applied (see
    // `Plan::grid_capacity`'s own doc). Nought for the two shapes with no window, matching
    // `Window::default()` below rather than leaking a number nothing was capped against.
    let grid_capacity = match shape {
        Subject::Record | Subject::Help => 0,
        Subject::Grid => body_rows.saturating_sub(note_rows),
    };
    let window = match shape {
        Subject::Record | Subject::Help => Window::default(),
        Subject::Grid => window(attention.selected, attention.items, grid_capacity),
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
        // 🔴 Minus the one the address spells, when the address is a key rather than a command —
        // the same count `super::renderer::note_line` makes, for the same reason: `help:?` on the
        // screen is a key the reader can see, and counting it among the ones they cannot would
        // make `declared = spelled + disclosed` come to one more than there are (gate g34).
        let keys_not_drawn = super::renderer::offered(attention.items)
            .len()
            .saturating_sub(usize::from(offered.contains(&super::acts::Act::Help)));
        if keys_not_drawn > 0 {
            disclosure = compose_disclosure(
                &dropped_fields,
                total_fields,
                &dropped,
                folded.then(|| measured.clone()).as_ref(),
                inner,
                wide,
                Shape {
                    subject: shape,
                    counts_dropped: !folded && !rung.carries_counts(),
                    keys_address: &keys_address,
                    wide_address: &wide_address,
                    keys_not_drawn,
                    address_above,
                    framed,
                    engine_dropped,
                },
            );
            if rows_needed(&disclosure, inner) > disclosure_rows {
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
        heading: head_cells,
        framed,
        // The rung was decided at the top of the last pass, beside the disclosure that describes
        // it. Spelling it here would be a second decision, and the two could differ.
        provenance: match rung {
            Rung::Long => measured.long(),
            Rung::Short => measured.short(),
            Rung::Bare => measured.bare(),
        },
        provenance_full: measured.long(),
        provenance_rung: rung,
        disclosure,
        truncated,
        window,
        note_rows,
        grid_capacity,
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
