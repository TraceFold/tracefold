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
///
/// 🔴 **A re-export, and it used to be a declaration** (`[T-r87]`, 2026-09-02). The four variants
/// are unchanged and every caller's spelling is unchanged; what moved is *where the rank is
/// decided*. It was decided **here**, at each column and at each region, by typing `Priority::Two`
/// beside the thing — so the order this face lets go of columns in was a fact about ten lines of
/// this file and not a declaration anything could swap. It is now `super::tokens::standing`, on the
/// placement ladder, beside the width and the order, and a second name for it here would be the
/// defect `super::tokens`'s own header names: two names for one join disagree the day one of them
/// is edited.
pub use super::tokens::Rank as Priority;

/// The width class this face resolves placement at, and the table it resolves against.
///
/// 🔴 Read once per call rather than threaded from the top, and the reason is stated rather than
/// left as convenience: the scheme is a property of the **reader's environment** exactly as
/// `super::tokens::Tier::detect` reads `NO_COLOR`, and every function below that needs it is
/// already taking a `width` — so threading a second argument through twenty-five call sites would
/// change every one of them to carry a value none of them decides. **Named ceiling**: this makes
/// the scheme process-global, so two frames drawn in one process cannot use two schemes. Nothing
/// asks for that today and the day something does, this is the one line that changes.
fn table(width: u16) -> (super::tokens::Grade, super::tokens::Scheme) {
    (
        super::tokens::Grade::of(width),
        super::tokens::Scheme::detect(),
    )
}

/// The grade and scheme every `const` in this file is resolved at.
///
/// 🔴 A `const` cannot read an environment, so the declarations below answer at the widest grade of
/// the scheme this face ships with. That is not a hidden second table: [`columns_for_less`] — the
/// one function that decides what is actually **drawn** — resolves at the real grade and the real
/// scheme, and these constants are the *declaration index* (which keys exist, and what they are
/// worth when nothing narrows them). Gate `g103` measures that the two agree at `Grade::Full` under
/// `Scheme::Ledger`, so the index can never quietly stop describing the shipped default.
const BASE: (super::tokens::Grade, super::tokens::Scheme) = (
    super::tokens::Grade::Full,
    super::tokens::SCHEME_DEFAULT,
);

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
        priority: super::tokens::standing(super::tokens::Slot::BandApparatus, BASE.1).0,
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
        //
        // 🔴 **The numeral left this line** (`[T-r87]`, 2026-09-02). What stands here is the same
        // one, resolved through `super::tokens::cells` at `band.apparatus`, and the paragraphs above
        // are the record of how it came to be one — kept, because a declaration that loses the
        // argument for its own value is a number nobody may ever change again.
        min_rows: super::tokens::cells(super::tokens::Slot::BandApparatus, BASE.0, BASE.1).width,
    },
    Region {
        intent: Intent::RecordsTheEngineProduced,
        role: RegionRole::Subject,
        priority: super::tokens::standing(super::tokens::Slot::BandLedger, BASE.1).0,
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
        //
        // 🔴 The numeral left this line too (`[T-r87]`): `band.ledger` is its own measure precisely
        // because it is the one region floor that is not one row, and a measure the other three
        // shared would have hidden that.
        min_rows: super::tokens::cells(super::tokens::Slot::BandLedger, BASE.0, BASE.1).width,
    },
    Region {
        intent: Intent::WhereTheNumbersCameFrom,
        role: RegionRole::Provenance,
        priority: super::tokens::standing(super::tokens::Slot::BandProvenance, BASE.1).0,
        recoverable: Recoverable::Nowhere,
        min_rows: super::tokens::cells(super::tokens::Slot::BandProvenance, BASE.0, BASE.1).width,
    },
    Region {
        intent: Intent::WhatIsNotOnTheScreen,
        role: RegionRole::Disclosure,
        priority: super::tokens::standing(super::tokens::Slot::BandDisclosure, BASE.1).0,
        recoverable: Recoverable::Nowhere,
        min_rows: super::tokens::cells(super::tokens::Slot::BandDisclosure, BASE.0, BASE.1).width,
    },
];

/// The regions that do **not** scroll: the standing chrome, declared in one place so a gate reads
/// the declaration rather than a row number.
///
/// 🔴 **One, and it was five rows across three regions** (`req/924` §TUI-57, `req/38` SS1088, Owner
/// `#282-T`). The Owner's reference faces — `Claude Code`, `OpenClaw CLI` — keep **no fixed header
/// at all**: the body scrolls whole and one row at the bottom stands. The top rail is gone, and the
/// grid's column header and its `these N` clause are content rows at the head of the scrolling
/// stream rather than pinned chrome.
///
/// The predicate is *which region draws the row*, not *which row index it is*, because a row number
/// is a fact about one terminal size and this has to hold at all of them.
pub const FIXED_REGIONS: [RegionRole; 1] = [RegionRole::Disclosure];

/// The regions that stand down **by ruling** rather than by running out of rows.
///
/// 🔴 The distinction is [`Shape::provenance_stood_down`]'s, one region wider (`req/924` §TUI-29):
/// *cut for room* and *has nothing left to say* are different facts and get different clauses.
/// These two are a third thing again — *ruled off the standing frame* — and what each of them was
/// carrying has a declared destination:
///
/// * `apparatus`: the page's address and the engine's own line. The address goes behind `?`
///   (§TUI-57: a signpost is enough once and the complete address is a detail); `status ok` and
///   `ledger_agrees yes` were furniture on every healthy frame by §TUI-29's own test, and they are
///   spelled in full in the hatch by `super::renderer::help_lines`.
/// * `provenance`: the four measured facts and the connection. The connection becomes the dot on
///   the standing row ([`super::live::LinkReport::dot`]); the four facts and the counts are the
///   hatch's [`Plan::provenance_full`], and the disclosure spells the road to them.
///
/// 🔴 Being ruled off is **not** being deleted. `§TUI-21` is the rule that a road has to arrive, and
/// gate `g76` is what confirms it does rather than this comment being believed.
pub const STOOD_DOWN_REGIONS: [RegionRole; 2] = [RegionRole::Apparatus, RegionRole::Provenance];

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
    // 🔴 **Sixteen cells, and nothing on this screen needed sixteen** (`[T-r71]`, 2026-09-02,
    // `req/942_artifacts/tui_r71_2026-09-02/RULING.md`). The one column that is never dropped had the one width nothing had
    // measured. Both halves were measured against a live engine on a twenty-nine row bed grown by
    // the product's own loop, and they disagree about what holds the cells:
    //
    // * **the values need thirteen.** Three characters past the shared `gx1:` separated every row;
    //   `super::renderer::id_separation` is that measurement and `ID_PREFIX_MARGIN` /
    //   `ID_PREFIX_FLOOR` are the budget taken on top of it for the rows a page did not carry.
    //   Eleven characters were drawn.
    // * **the label needs fourteen**, and `transformation` is a wire key that is drawn unchanged
    //   (`req/942` §9, gate P5). So the header is what binds, not the hash — and the honest width is
    //   the label's, which is what is declared here.
    //
    // 🔴 The two cells are not a rounding: at 66 cells they are the difference between four columns
    // and three, which gate `g92` measures rather than this comment asserting it.
    //
    // 🔴 And the other eight characters were not buying a **name**. Measured on both roads a reader
    // has — `gx receipt show <id>` and `GET /v1/transformations/{id}` — no prefix of an id resolves
    // at any length: three, six, eleven and **fifty-one of fifty-two** all answer
    // `VALIDATION_ERROR` / `422`, and only the whole fifty-two answers `200`. Whatever this column
    // draws is a discriminator and never an address, at any width, and
    // `super::renderer::help_lines` is where that is now said out loud.
    //
    // 🔴 **Every one of the thirty numerals and rank names that stood here is gone to the ladder**
    // (`[T-r87]`, 2026-09-02). Ten keys, ten widths and ten ranks were typed at this array, and the
    // array's own subscript was a *fourth* declaration nothing had written down: the order columns
    // are kept in. All four are `super::tokens` now — `Slot::key`, `cells(..).width`,
    // `cells(..).rank` and `cells(..).order` — so *put that column first*, *make it narrower* and
    // *let it go sooner* are table edits rather than edits to this file.
    //
    // What this array still is: the **declaration index**, resolved at [`BASE`]. It says which keys
    // exist and what they are worth when nothing narrows them, and it is what the twenty-seven
    // sites that read `LEDGER_COLUMNS` have always wanted. [`columns_for_less`] is what decides the
    // columns a screen actually draws, and it resolves at the real grade and the real scheme.
    column(super::tokens::LEDGER_SLOTS[0]),
    column(super::tokens::LEDGER_SLOTS[1]),
    column(super::tokens::LEDGER_SLOTS[2]),
    column(super::tokens::LEDGER_SLOTS[3]),
    column(super::tokens::LEDGER_SLOTS[4]),
    column(super::tokens::LEDGER_SLOTS[5]),
    column(super::tokens::LEDGER_SLOTS[6]),
    column(super::tokens::LEDGER_SLOTS[7]),
    column(super::tokens::LEDGER_SLOTS[8]),
    column(super::tokens::LEDGER_SLOTS[9]),
];

/// One column, resolved off the placement ladder.
///
/// 🔴 The join between the two vocabularies, and it is one function so that it is one join. `key`
/// is the wire's own word (`super::tokens::Slot::key`, gate `P5`), `width` and `priority` are what
/// the table says at the grade and scheme it is asked at.
#[must_use]
const fn column_at(
    slot: super::tokens::Slot,
    grade: super::tokens::Grade,
    scheme: super::tokens::Scheme,
) -> Column {
    let cells = super::tokens::cells(slot, grade, scheme);
    Column {
        key: match slot.key() {
            Some(key) => key,
            // Unreachable from `LEDGER_SLOTS`, and the compiler proves it on every build rather
            // than a comment claiming it: a slot that is not a column has no key, and a const
            // evaluation is where that is caught.
            None => panic!("a ledger column's slot must carry the wire's own key"),
        },
        width: cells.width,
        priority: cells.rank,
    }
}

/// The same column at [`BASE`], for the declaration index above.
#[must_use]
const fn column(slot: super::tokens::Slot) -> Column {
    column_at(slot, BASE.0, BASE.1)
}

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
///
/// 🔴 Resolved off the ladder (`[T-r87]`), and it is one of the four measures the ladder holds
/// **constant** along both axes — see `super::tokens::Measure::welded`. The reason is this
/// constant's own doc above: two places need the same number, and a measure that varied would put
/// an axis between them.
pub const FRAME_MARGIN: u16 =
    super::tokens::cells(super::tokens::Slot::Enclosure, BASE.0, BASE.1).width;

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
    /// The engine's own line **unfolded**, whatever the rail is drawing.
    ///
    /// 🔴 **The other half of the fold, and it is the shape [`Plan::provenance_full`] already
    /// proved.** `req/924` §TUI-29 lets the rail spell `engine ok` while both of the engine's claims
    /// hold, and `req/38` SS842 is the reason that is only half a decision: a reduction pass takes
    /// the caveats out with the padding unless the caveat is *moved* rather than dropped. This is
    /// where it moved to, and the help face is where a reader reaches it — so `engine_version`, which
    /// no other row spells, is still on a screen this process can draw.
    pub engine_full: Vec<(String, String)>,
    /// Whether every one of the reads answered `200`.
    ///
    /// 🔴 A bool beside [`Measured::statuses`] rather than a `contains` on it later (`req/924`
    /// §TUI-29). That sentence is composed for a reader and may be reworded; the decision this fact
    /// drives — whether the provenance region has anything left to say — must not move when it is.
    /// Two readings of one measurement, and only one of them is a string.
    pub all_200: bool,
    /// Whether both of the engine's claims about itself hold: `status ok` and `ledger_agrees yes`.
    ///
    /// 🔴 A bool beside [`Measured::engine`] for the reason [`Measured::all_200`] is one beside
    /// `statuses`. `req/924` §TUI-29's test — *does this row change the reader's next act when
    /// everything is normal?* — is now the thing that decides whether the one standing row spends
    /// cells on the engine at all, and a decision that important may not be a `len() == 1` on a
    /// vector composed for a reader. `super::renderer::engine_line` is where it is measured, which
    /// is the one place that reads the two words the fold turns on.
    pub healthy: bool,
    /// The columns whose every value in this reading is a mark for nothing.
    ///
    /// 🔴 **`req/924` §TUI-45 (`req/38` SS1076, Owner `#275-T`): a column that says nothing on every
    /// row is a column spent saying nothing.** `created_at ?` down twenty-three rows says *measured,
    /// and not knowable* twenty-three times where once is the whole of it. The column is not drawn
    /// and is counted among the fields the disclosure names instead — and that second half is what
    /// makes it a disclosure rather than a deletion.
    ///
    /// **The boundary is the ruling's**: only when **every** value is a mark for nothing. A column
    /// where some rows carry a value is information and is kept, and so is one where the marks
    /// disagree — `?` on one row and `--` on another is this face telling two kinds of nothing
    /// apart, which is the distinction the whole vocabulary exists for.
    ///
    /// 🔴 **The mark travels with the key** (independent audit, finding 4). A `Vec<&str>` of keys
    /// said only *this column was nothing*, and the help face could then say no more than "a mark
    /// for nothing" — collapsing `?` and `--` and `...` for every column it dropped, inside the
    /// lane whose other half exists to keep them apart. The pair is what the hatch spells.
    ///
    /// Measured in `super::renderer::vacant_columns`, which is where a record may be read.
    pub vacant: Vec<(&'static str, String)>,
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
            "{} {} routes {} | worst {}ms | {} | {} | {}",
            self.link.link.mark(),
            self.routes,
            self.clock(),
            self.worst_ms,
            self.statuses,
            // 🔴 **When something last arrived** (`req/38` SS1085, `req/924` §TUI-57). The dot on
            // the standing row says *live* or *quiet*; the sentence that says **how** quiet is
            // spelled here, which is the one screen a reader reaches it on. `§TUI-21` is why gate
            // `g76` confirms this line is actually on the hatch rather than trusting this comment.
            self.link.silence(),
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
    /// The engine's own line unfolded, for the face that discloses what the rail folded.
    pub engine_full: Vec<(String, String)>,
    /// The columns dropped because every value in this reading was a mark for nothing.
    ///
    /// 🔴 Held **separately** from [`Plan::dropped_fields`], which it is a subset of, because the
    /// two are dropped for different reasons and the reader can act on the difference: a column let
    /// go of by width comes back on a wider terminal, and one let go of by this rule does not. The
    /// help face names them under a label of their own for that reason.
    ///
    /// 🔴 And it is **not** emptied for the record and help shapes the way `dropped_fields` is:
    /// vacancy is a property of the reading rather than of the shape being drawn, and a reader who
    /// pressed `?` is asking about the grid they came from.
    pub vacant_fields: Vec<(&'static str, String)>,
    /// The one standing row, cell by cell: where the reader is, the keys, the connection's dot, and
    /// what is not on the screen.
    ///
    /// 🔴 **Four lines became one** (`req/924` §TUI-57). They are cells rather than one string
    /// because the dot carries a paint role of its own — six of them
    /// ([`super::live::LinkReport::dot`]) — and a role cannot travel inside a `String`. The same
    /// reason [`Plan::heading`] is a `Vec<HeadingCell>`.
    pub status: Vec<HeadingCell>,
    /// The note the standing row carries: where the reader is, the keys, and the count of keys it
    /// folded away.
    ///
    /// 🔴 Held beside [`Plan::status`] rather than dug back out of it, for the reason
    /// [`Plan::disclosure`] is held beside it: the two are what the row is **composed from**, and a
    /// caller that had to find the note by index would be re-deriving a decision this module made.
    /// `status` is the same three strings with their paint roles attached and is what the region
    /// draws; a gate that wants the words asks for them here.
    pub note: String,
    /// How many rows the grid puts above its records in the scrolling stream: the column header,
    /// and the `all N` clause when there is one.
    ///
    /// 🔴 **They are content, not chrome** (`req/924` §TUI-57). Both are derived from the records —
    /// the header names the columns *this reading* has, the clause states what *these records*
    /// agree on — so pinning them would be a region growing its own chrome out of its content, the
    /// defect `super::renderer::note_rows` was written against. They sit at the head of the stream
    /// and scroll away like any other row.
    pub preamble: usize,
    /// How many of those rows are still on the screen at this scroll position.
    ///
    /// Nought once the reader has moved far enough down the ledger, which is what makes them
    /// scrolling content rather than a fixed header, and what gate `g73` measures.
    pub preamble_shown: usize,
    /// How many of an opened record's own rows the height took.
    ///
    /// 🔴 Decided **here**, for the reason [`Plan::window`] is: the order in which a screen lets go
    /// of what it was asked to draw is a declaration, and a region that chose its own cut could not
    /// be asked what it let go of (`req/942` §11-3).
    pub record_members_shown: usize,
    /// How many of the rows beyond an opened record's members the height took.
    pub record_beyond_shown: usize,
    /// Whether either of the two above is short of what was asked for, so the region owes a line
    /// saying so.
    ///
    /// 🔴 The row that line costs is **inside** the budget above: it is subtracted before the two
    /// counts are settled, so the disclosure of a cut can never be the thing the cut takes.
    pub record_cut: bool,
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

    /// How many rows of this frame do not scroll.
    ///
    /// 🔴 Read from [`FIXED_REGIONS`] rather than restated, so the number and the declaration cannot
    /// drift. Gate `g73` asserts this is at most one at every ruled shape; the declaration is what
    /// it reads, and the behaviour it reads beside it is [`Plan::preamble_shown`] falling to nought.
    #[must_use]
    pub fn fixed_rows(&self) -> u16 {
        self.rows
            .iter()
            .filter(|(role, _)| FIXED_REGIONS.contains(role))
            .map(|(_, n)| *n)
            .sum()
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

/// How an opened record's rows and the ledger's rows share the subject region.
///
/// 🔴 One function, called twice by [`resolve_attended`] — once to decide whether the disclosure has
/// a ledger to describe, and once with the settled height to fill the plan in — so the clause that
/// says what is missing and the region that does the missing cannot arrive at two answers
/// (`req/964` §16, applied to the split `[T-r58]` introduced).
///
/// Returns, in order: how many of the record's own rows the height took, how many of the rows beyond
/// them it took, whether either is short of what was asked for, and how many rows are left for the
/// ledger underneath.
#[must_use]
fn record_split(subject_height: usize, members: usize, beyond: usize) -> (usize, usize, bool, usize) {
    let cut = members + beyond > subject_height;
    let room = subject_height.saturating_sub(usize::from(cut));
    let members_shown = members.min(room);
    let beyond_shown = beyond.min(room - members_shown);
    let ledger = subject_height.saturating_sub(members_shown + beyond_shown + usize::from(cut));
    (members_shown, beyond_shown, cut, ledger)
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
    /// How far the reader has moved the face, as against the attention
    /// ([`super::acts::View::glide`], `req/924` §TUI-62 裁定3).
    ///
    /// A third member rather than a fourth argument, for the reason the two above it travel
    /// together: a plan is resolved for a reading, for a reader, and now for where that reader has
    /// pushed the stream.
    pub glide: isize,
    /// How many rows an opened record's **own** part needs: the head, the members that carry a
    /// value, and one row per kind of nothing the wire gave.
    ///
    /// 🔴 A count and not the rows themselves, for the reason [`Attention::items`] is a count: this
    /// module is built from a width and a height and never reaches into a record
    /// (`req/984` §10-33). `super::renderer::record_extent` is the one function that counts them,
    /// and both of the callers that draw a held record ask it rather than counting again.
    ///
    /// Nought at every shape but [`Subject::Record`], and it is read nowhere else.
    pub record_members: usize,
    /// How many rows the routes **beyond** an opened record's own members need.
    ///
    /// Held apart from [`Attention::record_members`] because the two are not cut in the same order:
    /// the members are what the wire carried about this row and are paid for first; the rows below
    /// them are what other routes said about it (`super::renderer::record_rows`).
    pub record_beyond: usize,
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

/// The same question over a stream that has `preamble` content rows in front of the records.
///
/// 🔴 **`req/924` §TUI-57's third ruling, and the whole of it.** The Owner withdrew the seat's
/// refusal of a wholly scrolling body: the reference faces keep no fixed header, so the grid's
/// column row and its `all N` clause stop being pinned and become the first rows of one stream.
/// From here on there is a single content list — `preamble` rows, then the records — and one
/// viewport over it.
///
/// Returns how many of the preamble rows are still visible and which records are drawn. The two
/// answers come from one arithmetic, which is why they are one function: computing them separately
/// is how a screen comes to draw a header it did not budget a row for.
///
/// 🔴 It stays a **function of the state** rather than a remembered scroll offset, exactly as
/// [`window`] is: the top is derived from where the attention is standing, so no act had to learn
/// about it and there is nothing to fall out of step with.
/// 🔴 `glide` is the reader's own offset (`req/924` §TUI-62 裁定3): **positive moves the content
/// up**, which is what scrolling down does in the face the ruling names. It is added to the answer
/// the attention would have produced and then clamped here — the only place that knows both how
/// tall the stream is and how tall the region is.
#[must_use]
pub fn scrolled(
    selected: usize,
    items: usize,
    preamble: usize,
    height: usize,
    glide: isize,
) -> (usize, Window) {
    if height == 0 {
        return (0, Window { first: 0, rows: 0 });
    }
    let total = preamble + items;
    if items == 0 {
        return (preamble.min(height), Window { first: 0, rows: 0 });
    }
    // Where the attended record sits in the stream, counting the preamble.
    let attended = preamble + selected.min(items - 1);
    // The least the stream has to move for the attention to be on the screen. `+1` because the
    // attended row is inside the window, not the row after it.
    let top = if attended < height {
        0
    } else {
        attended + 1 - height
    };
    // 🔴 And never past the end: at the bottom of a short ledger the stream stops rather than
    // scrolling blank rows up under the reader. `saturating_sub` answers nought when the whole
    // stream fits, which is the case where the header is on the screen at every position.
    let ceiling = total.saturating_sub(height);
    let top = top.min(ceiling);
    // The reader's own push, on top of that, clamped to the stream. With `glide` at nought this is
    // the identity and the window is exactly the function of the state it has always been.
    let top = (top as isize)
        .saturating_add(glide)
        .clamp(0, ceiling as isize) as usize;
    let shown = preamble.saturating_sub(top);
    let first = top.saturating_sub(preamble);
    let rows = (items - first).min(height - shown);
    (shown, Window { first, rows })
}

/// The cells between one column and the next, and before the first.
///
/// 🔴 **`req/924` §TUI-62 裁定3, 余裕** (`req/38` SS1093, Owner `#284-T`): *a terminal has no line
/// height, so the room is made in the column gap and the left margin.* Two constants and not a
/// number typed into four places — `columns_for_less` prices a column against the gap,
/// `super::renderer::spans` draws it, `header_width` measures it and the margin is charged to the
/// width the plan is resolved against. A second spelling is how a row comes to be composed at one
/// width and drawn at another, which [`FRAME_MARGIN`] is declared once to prevent.
///
/// 🔴 Resolved off the ladder (`[T-r87]`), and **welded**: `super::tokens::span` answers this
/// measure before it consults the scheme, so no table can make the price and the row disagree.
pub const COLUMN_GAP: u16 =
    super::tokens::cells(super::tokens::Slot::CellGap, BASE.0, BASE.1).width;

/// The cells before the first column of every row of the ledger.
///
/// 🔴 Resolved off the ladder (`[T-r87]`), and welded for the reason [`COLUMN_GAP`] is.
pub const LEFT_MARGIN: u16 =
    super::tokens::cells(super::tokens::Slot::RowLead, BASE.0, BASE.1).width;

/// How many cells a ledger row carrying exactly these columns occupies on the screen.
///
/// 🔴 **One function, and it is the price *and* the row** (`[T-r66]`, 2026-09-02). The arithmetic
/// this replaces was written twice — once as an incremental cost inside [`columns_for_less`] and
/// once as a sentence in `super::renderer::subject` — and the two disagreed by exactly
/// [`COLUMN_GAP`]. The sentence said `LEFT_MARGIN + sum(width) + (n - 1) * COLUMN_GAP`; the row is
/// drawn as [`LEFT_MARGIN`] **in a cell of its own** (`super::renderer::margin`, ruled that way by
/// `req/924` §TUI-62 so that `pad` never eats a character to make room for a space), and
/// `super::renderer::spans_with` puts a gap after **every** cell it has already drawn — including
/// that one. So the row is the margin, and then `n` times *(gap, column)*: one gap more than the
/// price knew about.
///
/// The consequence was not a rounding error. A budget two cells looser than the screen keeps one
/// column too many, the terminal clips from the right in silence, and the value in the last column
/// ends early with nothing saying so — `2026-08-30T09:00:00Z` drawn as `2026-08-30T09:00:0` at
/// sixty-six cells. That is the face asserting a fact it does not have, which is the one thing this
/// membrane exists not to do.
///
/// Nought for no columns rather than [`LEFT_MARGIN`]: a row with no column on it is not drawn as a
/// lone margin — there is nothing for the margin to be before.
///
/// 🔴 **This is what a row *takes*, not a promise that it fits.** [`columns_for_less`] keeps one
/// column whatever the width (its floor), so at a screen narrower than
/// `LEFT_MARGIN + COLUMN_GAP + the first column` this answers a number larger than the screen and
/// that is the ruled outcome, not a defect: `super::renderer::fit` then cuts the row **and marks the
/// cut**. Gate `g90` states the invariant in exactly that shape.
#[must_use]
pub fn row_width(columns: &[Column]) -> u16 {
    if columns.is_empty() {
        return 0;
    }
    LEFT_MARGIN
        + columns
            .iter()
            .map(|column| COLUMN_GAP + column.width)
            .sum::<u16>()
}

/// How many records share one group before the next rule.
///
/// 🔴 **`req/924` §TUI-62 裁定3, 区切り**: *a rule every N rows, or a blank line between groups.*
/// Neither is free — a blank row costs a record and a rule row costs a record — so this face draws
/// the rule **as an underline on the last row of each group**, which is the one form of a horizontal
/// line a terminal can draw without spending a row on it. Ink added: **nought**; rows given up:
/// **nought**. The chrome budget is reported either way, because the ruling asked for the
/// measurement rather than for the answer.
///
/// 🔴 Resolved off the ladder (`[T-r87]`) at `frame.run`, and **not** welded: how many records share
/// a group is a legibility judgement rather than an arithmetic one, so a scheme may move it without
/// putting anything out of agreement with anything.
pub const GROUP_ROWS: usize =
    super::tokens::cells(super::tokens::Slot::GroupRun, BASE.0, BASE.1).width as usize;

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
    columns_for_less(width, &[])
}

/// The same question, with the columns this reading found nothing in taken out of it first.
///
/// 🔴 **`req/924` §TUI-45's rule, applied *before* the width fit rather than after** (`req/38`
/// SS1076, Owner `#275-T`). A column whose every value is a mark for nothing is not competing for
/// cells a column carrying a value could use, so taking it out first is not a tidy-up: it is what
/// lets a narrow terminal draw one more real column. Measured against the live bed, five of the ten
/// keys came back a mark for nothing on all thirty-one records.
///
/// The dropped half is the union of the two reasons in **declaration** order, and both reasons end
/// in the same place — [`Plan::dropped_fields`], which is what the disclosure counts. 🔴 That union
/// is the ruling's other half: *do not drop it quietly; the disclosed number going up is the correct
/// shape of this change.*
#[must_use]
pub fn columns_for_less(width: u16, vacant: &[&'static str]) -> (Vec<Column>, Vec<&'static str>) {
    let mut drawn = Vec::new();
    let mut dropped: Vec<&'static str> = Vec::new();
    // 🔴 An explicit flag, and it is a repair this rule made necessary. The fold below used
    // `!dropped.is_empty()` to mean *the budget has already overflowed*, which was true while the
    // only way into that vector was overflowing. A vacant column now enters it before any width has
    // been spent, so the old reading would have dropped every column declared after the first
    // vacant one — a silent and total loss, caused by a tidier line above it.
    let mut overflowed = false;
    // 🔴 The declaration decides which column goes first, and this **reads** it. The fold below
    // walks the array and gives up whatever is past the budget, so before this line the order was
    // the order the array happened to be typed in; `LEDGER_COLUMNS` was typed in priority order, so
    // the two agreed, and `Column::priority` was a member no code read. Reordering the array — for
    // readability, for a new column, for anything — would have silently made it a lie.
    //
    // Ascending and stable: `Priority::One` is kept first, and among equals the column declared
    // first is kept first, which is the "last first" order the array says of itself it is in.
    //
    // 🔴 **Resolved at the screen's own grade and the run's own scheme** (`[T-r87]`, 2026-09-02).
    // This walked [`LEDGER_COLUMNS`] — the declaration index, whose widths are [`BASE`]'s — so a
    // column was the same number of cells wide at forty cells as at a hundred and twenty, and the
    // only lever a narrow terminal had was **dropping the column whole**. That is the shape of the
    // Owner's finding: at `40x10` this face answered *fewer facts*, never *smaller ones*, because
    // there was no table to ask. There is one now, and this is where it is asked.
    let (grade, scheme) = table(width);
    let mut ordered: Vec<Column> = super::tokens::LEDGER_SLOTS
        .into_iter()
        .map(|slot| column_at(slot, grade, scheme))
        .collect();
    // 🔴 Rank first, then the declared order within a rank — and the second key is the repair of a
    // thing that was never written down. The sort here was by priority alone and **stable**, so ties
    // kept the order the array happened to be typed in; the order was therefore a fact about a
    // literal's line numbers that no declaration carried and no gate could read. It is
    // `super::tokens::Cells::order` now, so a scheme can say *this column comes first* without an
    // array being retyped.
    ordered.sort_by_key(|column| {
        let cells = super::tokens::cells(
            super::tokens::Slot::of_key(column.key).expect("a drawn column has a slot"),
            grade,
            scheme,
        );
        (cells.rank, cells.order)
    });
    for column in ordered {
        if vacant.contains(&column.key) {
            dropped.push(column.key);
            continue;
        }
        // 🔴 **The price is [`row_width`], which is the row** (`[T-r66]`, 2026-09-02, repairing the
        // defect `[T-r58]` found and left open). The arithmetic that stood here charged
        // [`LEFT_MARGIN`] for the first column and [`COLUMN_GAP`] for each one after it — the
        // sentence `LEFT_MARGIN + sum(width) + (n - 1) * COLUMN_GAP`, written in this loop and again
        // as a comment in `super::renderer::subject`. The row is drawn with the margin **in a cell
        // of its own**, so `super::renderer::spans_with` puts a gap after it too: `n` gaps, not
        // `n - 1`. Every kept set was therefore two cells wider than the width it was chosen
        // against, and at the widths where the budget binds the terminal clipped the last column in
        // silence. Asking [`row_width`] instead of adding up a second time is what makes the two
        // unable to disagree again.
        //
        // Push, measure, and take it back off if it did not fit: the candidate set is the thing the
        // question is about, so it is the thing that is measured.
        //
        // 🔴 **The floor: `drawn.len() > 1`, so the first column is kept whatever the width**
        // (`[T-r66]`, 2026-09-02, and it is a repair of this lane's own first attempt). Refusing the
        // first column too was arithmetically correct and drew, at eighteen cells on the live bed,
        // **nine blank rows** — a ledger of thirty-one records showing nothing at all. That is the
        // worse half of the defect being repaired, not the repair: a screen that cuts a value in
        // silence at least tells the reader something, and a screen that draws nothing tells them
        // the ledger is empty.
        //
        // So the ruling in `super::renderer::fit` is taken at this layer too, in the direction that
        // layer's degenerate case calls for: the identity column is kept, the row goes out wider
        // than the screen, and `fit` cuts it **with the mark**. `   gx1:f6hb5y2r3~` at eighteen
        // cells is a true partial answer; `gx1:f6hb5y2r3oi` — the same row with the `~` itself
        // clipped off, which is what stood here before this lane — is a false whole one.
        if !overflowed {
            drawn.push(column);
            if drawn.len() > 1 && row_width(&drawn) > width {
                drawn.pop();
                overflowed = true;
            }
        }
        if overflowed {
            dropped.push(column.key);
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
    // 🔴 **One voice and a quorum of two**, which is exactly what this function has always meant
    // (`[T-r87]`, 2026-09-02): fold only when *every* record agrees, and answer nothing at all about
    // fewer than two records. Expressed as the declaration rather than as its own loop, so that the
    // day a scheme allows a second voice, this function and [`resolve_folded`] cannot disagree about
    // what the first one meant.
    let (voices, quorum) = super::tokens::strict_fold();
    let (kept, folded) = resolve_folded(columns, rows, voices, quorum);
    let shared = folded
        .into_iter()
        .filter_map(|(key, tally)| tally.into_iter().next().map(|(mark, _)| (key, mark)))
        .collect();
    (kept, shared)
}

/// Which values a column carried over these rows, and how many rows said each.
///
/// 🔴 Ordered by **count, descending, ties in the order first seen** — and the tie-break is the
/// point rather than tidiness. A tally is drawn, so its order is a fact on the screen; ordering by
/// the value's own spelling would put `?` above `Admit` on some beds and below it on others, and a
/// line whose order is a function of the alphabet of the data is the same defect `req/38` SS1047
/// killed one layer up (*a screen whose shape is a function of something the reader cannot see*).
///
/// Layer-independent for the reason [`uniform`] is: this has no idea what a wire key, a column or a
/// mark for nothing is, and would answer the same question about exit codes.
#[must_use]
pub fn tally(marks: &[String]) -> Vec<(String, usize)> {
    let mut counted: Vec<(String, usize)> = Vec::new();
    for mark in marks {
        match counted.iter_mut().find(|(seen, _)| seen == mark) {
            Some((_, count)) => *count += 1,
            None => counted.push((mark.clone(), 1)),
        }
    }
    // Stable, so the order things were first seen in survives a tie.
    counted.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    counted
}

/// The same split [`resolve_shared`] makes, with the two numbers that decide it taken from the
/// placement ladder instead of being implicit.
///
/// 🔴 **`[T-r87]`, 2026-09-02, and it is the declaration the Owner's finding needed.** The grid was
/// measured at thirty-two records of which thirty are byte-identical in every column but the
/// discriminator, and twenty-six screen rows went to drawing that repetition. [`resolve_shared`]
/// could not touch it, because its rule is *all or nothing* — two dissenting records out of
/// thirty-two keep all thirty repetitions on the screen — and that rule was **not written down
/// anywhere a request could reach**. It is `fold.voices` and `fold.quorum` now.
///
/// * `voices` — how many **different** values a column may carry and still be said once at the head.
///   One is [`resolve_shared`]'s rule exactly.
/// * `quorum` — how many rows there must be before saying it once is worth the row it costs. Two is
///   [`uniform`]'s rule exactly, and it is why fewer than two rows proves nothing about repetition.
///
/// 🔴 **What a fold with more than one voice gives up, said plainly, because a renderer that cannot
/// invert owes the disclosure instead** (`req/942` §1-1): the **distribution** survives — every
/// value the column carried is on the screen with the count of rows that said it — and what goes is
/// the binding of a value to a *particular* row. That is recoverable and the road is `open`: the
/// record face draws every member of the attended record, including the ones the grid has no column
/// for. The caller is what spells the road; this function is what makes the loss.
///
/// **Named ceiling**, inherited whole from [`resolve_shared`]: the domain is the rows it is handed,
/// and the caller is responsible for handing it every record the read carried rather than the slice
/// on screen (`req/924` §TUI-20, `req/38` SS1047). Nothing here can tell the difference.
#[must_use]
pub fn resolve_folded(
    columns: &[Column],
    rows: &[Vec<String>],
    voices: usize,
    quorum: usize,
) -> (Vec<Column>, Vec<(&'static str, Vec<(String, usize)>)>) {
    let mut kept = Vec::new();
    let mut folded = Vec::new();
    for (index, column) in columns.iter().enumerate() {
        let marks: Vec<String> = rows.iter().map(|row| row[index].clone()).collect();
        if marks.len() < quorum {
            kept.push(*column);
            continue;
        }
        if voices <= 1 {
            // The old road, walked by the old function, so the one-voice answer cannot drift from
            // what [`uniform`] has always said it is.
            match uniform(&marks) {
                Some(mark) => folded.push((column.key, vec![(mark.to_string(), marks.len())])),
                None => kept.push(*column),
            }
            continue;
        }
        let counted = tally(&marks);
        if counted.len() <= voices {
            folded.push((column.key, counted));
        } else {
            kept.push(*column);
        }
    }
    (kept, folded)
}

/// How many different values a column may carry at this width and still be folded, and how many
/// rows make folding worth a row — read off the ladder.
///
/// 🔴 Two slots and one function, so the pair is asked for together. They are a single ruling
/// (`fold.voices` alone would fold a two-record ledger; `fold.quorum` alone has nothing to fold),
/// and a caller that could take one without the other is a caller that can take half a decision.
#[must_use]
pub fn fold_budget(width: u16) -> (usize, usize) {
    let (grade, scheme) = table(width);
    (
        super::tokens::cells(super::tokens::Slot::FoldVoices, grade, scheme).width as usize,
        super::tokens::cells(super::tokens::Slot::FoldQuorum, grade, scheme).width as usize,
    )
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
            if span < 2 || at + span > words.len() { // g100: a count of words, not of cells -- a one-word atom cannot be broken across a break, so it is not a phrase this loop is about
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
    /// The engine's own line, spelled in full, exactly when one of its two claims has stopped
    /// holding.
    ///
    /// 🔴 **The other half of `req/924` §TUI-29, and this lane shipped the first half without it.**
    /// The ruling is *fold to one word while everything is normal, and expand the moment either
    /// claim stops holding, with `status_reason` in front*. The fold moved to the dot and the
    /// expansion had nowhere to go, so an engine answering `ledger disagrees` drew no reason on any
    /// row — caught by gate `g59`, which had been written for exactly this and was measuring the
    /// rail. Empty while the engine is well, which is what keeps it out of the standing row's cells
    /// on every normal frame.
    engine_caveat: &'a str,
}

// 🔴 **Three members left this description with `req/924` §TUI-57, and where each one went is
// recorded here rather than deleted** (`INHERITED_PRINCIPLES` no-delete, and SS842: a reduction
// pass takes the caveats out with the padding unless the caveat is *moved*):
//
// * `address_above` — whether the top rail spelled [`LEDGER_ADDRESS`]. There is no top rail. The
//   complete address is a detail and lives behind `?` (§TUI-57), so the clause below carries a
//   road (`keys_address`) instead of a second spelling of the address. Gate `g75` is what refuses
//   a second spelling now, and `g76` is what confirms the hatch really carries the first.
// * `framed` — whether the ledger's enclosure was drawn. An enclosure needs two rails and there is
//   one row; the corners went with the rail. Nothing the corners deleted has come back, so their
//   absence destroys no fact — which is the test §TUI-22 admitted them under, read backwards.
// * `provenance_stood_down` — the provenance region stands down **by ruling** now
//   ([`STOOD_DOWN_REGIONS`]) rather than per frame, and §TUI-29's own test is why that clause is
//   not on the standing row: a sentence true on every frame forever changes no reader's next act.
//   `super::renderer::help_lines` spells the region, its intent and its line in full.

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
        engine_caveat,
    } = shape;
    // 🔴 **No road on this clause, and the address is not here either** (`req/924` §TUI-57,
    // `req/38` SS1088, Owner `#282-T`). `GET /v1/transformations` was on this screen once and the
    // ruling moves it behind `?`: *a signpost is enough once, and the complete address is a
    // detail*. The road is spelled once on this line — the routes clause below ends `-> {keys}` —
    // and §TUI-21's own words are why it is not spelled twice: **a signpost printed twice is not
    // two signposts**, and on a row this face now has exactly one of, ten cells is the difference
    // between the key legend spelling four acts and spelling two.
    let road = String::new();
    let mut long: Vec<String> = Vec::new();
    // 🔴 **First, because it is the one clause that changes what the reader does next**
    // (`req/924` §TUI-29). While the engine is well this is empty and costs nothing; the moment a
    // claim stops holding it leads the row, with `status_reason` at the front of it — which is the
    // order `super::renderer::engine_line` puts the keys in, read rather than restated here.
    if !engine_caveat.is_empty() {
        long.push(engine_caveat.to_string());
    }
    // 🔴 The field count belongs to whatever **grid** is on the screen, and it is asked of the set
    // rather than of the shape. The set is empty for the help face, which draws no wire value at
    // all, and empty for a record with no room under it; it is not empty for a record with a ledger
    // beneath it, which is a screen that did not exist until `[T-r58]` (2026-09-02) gave the rows a
    // record does not need back to the list it was opened from. Written as a branch on the shape,
    // that third screen would have been the one screen that drops columns and says nothing.
    if !dropped_fields.is_empty() {
        long.push(format!(
            "{} of {total_fields} fields not drawn{road}",
            dropped_fields.len()
        ));
    }
    match subject {
        Subject::Grid => {}
        // 🔴 **Nothing, and the sentence that stood here is deleted rather than shortened**
        // (`[T-r58]`, 2026-09-02, the seat's ruling on the real capture, defect 4). It read
        // *a record is open: its own line counts what it drew* — a sentence about **this face's own
        // arrangement**, addressed to nobody who is trying to read a record. It named no fact of the
        // engine, no fact of the reading, and no road; it told the reader which line of the screen to
        // trust, which is a thing a screen should not need to say. Five words
        // (`own line counts drew`, and `record` beside them) paid for by the one row this face has.
        //
        // What it was standing in for is still on the screen and is measured: `Subject::name`
        // already puts `record open` on this row, so *which of the three screens is this* is
        // answered; and the count of members the height would not take is drawn by the record
        // itself, in the region that made the cut, exactly when there is a cut
        // (`super::renderer::subject`). A clause that is true on every frame changes no reader's
        // next act — §TUI-29's own test, applied to a sentence that was exempt from it because it
        // was about the face rather than about the world.
        //
        // 🔴 What is kept is **the two words the short form already spells**, and nothing more: this
        // line does have to say which of the three screens it is describing, or a reader cannot tell
        // a count about a ledger from a count about a record. Two forms of one clause, one wording;
        // the long form used to pay five extra words for the same fact.
        Subject::Record => long.push("record open".to_string()),
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
    // 🔴 **Nought by construction since `req/924` §TUI-57**: there is no rail to run out of cells,
    // and `super::renderer::help_lines` spells every one of the engine's keys, in full, at every
    // shape. The clause is kept as a comment rather than deleted because the *obligation* it served
    // did not go anywhere — §TUI-22 classified those two claims as things that may be folded and
    // may not be discarded — and the hatch is now where it is discharged.
    //
    // 🔴 **`provenance -> ?` and `frame not drawn at this width` left with it, and for one reason**
    // (`req/924` §TUI-57). Both were true on **every** frame once the rail went, and §TUI-29's own
    // test — *does this row change the reader's next act when everything is normal?* — is what puts
    // a permanently true sentence off the standing row. Neither fact is discarded: the provenance
    // region, its intent and its line in full are in the hatch ([`Plan::provenance_full`],
    // `super::renderer::help_lines`), and the enclosure's corners deleted nothing that has come
    // back, so their absence destroys no fact. Gate `g76` is what checks the first of those rather
    // than this paragraph being believed.
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
        // 🔴 **Two facts, and the second one is only there when it is true** (`[T-r58]`,
        // 2026-09-02, defect 3). *Which screen is this* is what the record's head has always
        // carried; a record with a ledger under it also drops columns, and the short form is the
        // form the narrow shapes choose — so a record that said only `record open` would be the one
        // form of the one screen that drops columns in silence. The set is empty for a record that
        // fills its region, so the clause disappears exactly where there is nothing to say.
        Subject::Record if !dropped_fields.is_empty() => format!(
            "record open {}/{total_fields} fields{road}",
            dropped_fields.len()
        ),
        Subject::Record => "record open".to_string(),
        Subject::Help => "help open".to_string(),
    };
    // 🔴 **The count of regions no longer leads this line** (`req/924` §TUI-57). Nothing is dropped
    // by the ladder any more — the apparatus and the provenance are off the standing frame by
    // ruling and the two that remain cannot be given up — so `0 regions not drawn` would have been
    // a number that is nought on every frame forever, printed first, on the one row this face has.
    // The declaration that says which regions are not drawn, and where what they carried went, is
    // [`STOOD_DOWN_REGIONS`], and the hatch is where a reader reads it.
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
    // 🔴 `no frame`, `provenance -> ?` and `N engine keys` are gone from this form for the reason
    // the long form's copies are: each was true on every frame once the rail went, and a caveat
    // that never changes is furniture by `req/924` §TUI-29's own test. The facts are in the hatch,
    // which is what `g76` measures.
    // 🔴 **`gx tui --wide` is off this form and `?` is on it** (`req/924` §TUI-57). The short form
    // used to end with the address of the *long form*, which is a command that requires leaving the
    // process; `?` opens the hatch in place and the hatch spells every field name, every route name
    // and the whole of the provenance — strictly more than `--wide` answers for. This is the same
    // argument `Shape::keys_address` was introduced with, applied to the one clause that had not
    // taken it. `w` is still a declared act and the hatch still names it.
    let _ = wide_address;
    // The caveat leads this form too, and for the reason it leads the long one: a terminal cuts
    // from the end, and this is the clause a reader has to act on.
    let engine = if engine_caveat.is_empty() {
        String::new()
    } else {
        format!("{engine_caveat} | ")
    };
    format!(
        "{engine}{folded}{head}{keys}{counts} | {} routes -> {keys_address}",
        READ_NOT_DRAWN.len()
    )
}

/// The disclosure may take three rows, or four once the provenance has folded into it — the row the
/// provenance gave up is the row the fold is allowed to spend.
/// Cut a line to `room` cells and **mark** the cut, which is what `super::renderer::pad` does for a
/// table cell and what this row had no equivalent of.
///
/// 🔴 A terminal clips from the right without saying so, and the clause this row clips is the one
/// that says what is missing. The mark is inside the budget, so a reader sees *which* clause ran
/// out rather than a sentence that stops mid-word.
/// 🔴 **And it refuses to cut below [`CLIP_FLOOR`].** A clause cut to two cells is not a shorter
/// clause, it is a `~` where a sentence was — measured at twenty-four cells, where the clause
/// naming the connection's counts as unrecoverable came back as the single character `~`. Below the
/// floor the text is left whole and the row's own `!` is what says it was cut; a mark that says
/// *this was cut* is worth more than a mark that says *something was here*.
#[must_use]
fn clip(text: &str, room: usize) -> String {
    if text.chars().count() <= room {
        return text.to_string();
    }
    if room < CLIP_FLOOR {
        return text.to_string();
    }
    let head: String = text.chars().take(room.saturating_sub(1)).collect();
    format!("{head}~")
}

/// The fewest cells a cut clause may be left with before cutting stops being a shortening.
///
/// Sixteen: enough for `N/M fields` and a mark. Declared rather than typed into [`clip`] because it
/// is a judgement about legibility and the day it is wrong there is one line to change.
///
/// 🔴 Resolved off the ladder (`[T-r87]`) at `frame.floor`, and welded: a cut that stopped at a
/// different length in a different scheme would make the mark `~` mean two things.
const CLIP_FLOOR: usize =
    super::tokens::cells(super::tokens::Slot::ClipFloor, BASE.0, BASE.1).width as usize;

const fn disclosure_cap(_folded: bool) -> u16 {
    // 🔴 **One, and it was three or four** (`req/924` §TUI-57, `req/38` SS1088, Owner `#282-T`).
    // The standing chrome of this face is one row, so the line that says what is not on the screen
    // gets one row and the ladder above chooses the form that fits it. The parameter is kept
    // because the question it asked — *did the provenance fold into this line, and may it therefore
    // spend the row the region gave up?* — is still a real question the day this cap moves again;
    // it is answered `one` for both values now and that is a decision, not an oversight.
    1
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
    // 🔴 `req/924` §TUI-45: the reading's own answer is applied before the width's, so the cells a
    // column that says nothing was holding go to one that does not, and both reasons for a column
    // being absent end in `dropped_fields`.
    let vacant_keys: Vec<&'static str> = measured.vacant.iter().map(|(key, _)| *key).collect();
    let (columns, grid_dropped_fields) = columns_for_less(width, &vacant_keys);
    // While a record is open no field is dropped by **width**: the record draws every member the
    // wire carried, one per row. So the plan's dropped set is empty, and it is empty as a computed
    // fact rather than as a special case in whoever reads it.
    // The help face is the same case one step further out: it draws no wire value at all, so a set
    // of wire keys it "did not draw" would be counting a grid that is not on the screen.
    // 🔴 **An opened record with room under it is drawing a ledger, and a ledger drops columns by
    // width** (`[T-r58]`, 2026-09-02, defect 3). The sentence this replaces — *a record draws every
    // member the wire carried, so nothing is dropped by width* — is still true of the **record**, and
    // it was true of the whole screen only while the record was the whole screen. It is not any more:
    // the rows the record does not need go back to the ledger it was opened from, and those rows
    // carry the columns that fit. Saying nothing about them would be the disclosure describing the
    // screen this face used to draw.
    //
    // 🔴 **Named ceiling.** The split is asked here against the **ruled** standing frame of one row
    // (`req/924` §TUI-57, gate `g73`), because the disclosure cannot be composed against a height
    // that is not settled until the disclosure exists. Under `w` the long form may take more than
    // one row, the region then has fewer, and at a height where that takes the ledger's last row
    // this clause names columns of a ledger with no rows left. The region clamps
    // (`super::renderer::subject` draws what the plan settled), so the count is the only thing that
    // can overstate, and it overstates only under `w`.
    let ledger_below = match subject {
        Subject::Grid | Subject::Help => false,
        Subject::Record => {
            let standing = height.saturating_sub(u16::from(height > 0)) as usize;
            record_split(standing, attention.record_members, attention.record_beyond).3 > 0
        }
    };
    let dropped_fields = match subject {
        Subject::Grid => grid_dropped_fields,
        Subject::Record if ledger_below => grid_dropped_fields,
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
    // 🔴 **The top rail is gone, and with it four readings of one row** (`req/924` §TUI-57,
    // `req/38` SS1088, Owner `#282-T`). The heading ladder chose between the page's address, the
    // screen's name and the engine's keys; it opened the enclosure; it decided whether the
    // disclosure had to spell the address; and it decided whether the provenance region could
    // stand down. There is no such row now, so [`Plan::heading`] is empty at every shape and the
    // apparatus is never given rows. [`STOOD_DOWN_REGIONS`] is the declaration that says so and
    // where what it carried went; gate `g73` is what holds the standing chrome to one row.
    //
    // 🔴 **Named ceiling.** [`heading`], [`heading_carries_address`] and [`heading_engine_dropped`]
    // are still declared and are now called by nothing that draws. They are kept rather than
    // deleted because `no-delete` is the rule and because the ladder they encode is the answer the
    // day a rail comes back; they are named here so that "declared and read by nothing" is a fact
    // this module states about itself rather than one an audit has to find.
    let head_cells: Vec<HeadingCell> = Vec::new();
    let framed = false;
    // Nothing is given up by the ladder any more: the two regions that could be are off the frame
    // by ruling, and the two that remain are what the screen is *for* and the line that says what
    // is missing. The vector is kept because `compose_disclosure` still names whatever is in it,
    // so a region that grows a step later arrives there rather than in a new branch.
    let dropped: Vec<RegionRole> = Vec::new();
    // 🔴 **§TUI-29's test, applied in both directions.** A caveat takes a row when it is a caveat:
    // while every route answers `200` **and** both of the engine's claims hold, the dot is the
    // whole of what the old rail was saying and the four measured facts are in the hatch. The
    // moment either stops holding, the facts fold **into** the standing row — which is
    // [`Measured::folded`], the shape that already existed for exactly this, and it leads with the
    // clause no second read can recover.
    //
    // 🔴 [`Measured::healthy`] and not a re-derivation from [`Measured::engine`]: the decision is
    // measured where the fold is made (`super::renderer::engine_line`), so the two cannot disagree.
    let folded = !(measured.all_200 && measured.healthy);
    // The rung is still chosen by measuring the line rather than against a width the line used to
    // fit at, and it still decides what [`Plan::provenance`] carries for the hatch.
    let rung = if super::tokens::Grade::of(width).index()
        >= super::tokens::Grade::Snug.index()
        && rows_needed(&measured.long(), width) <= 1
    {
        Rung::Long
    } else if rows_needed(&measured.short(), width) <= 1 {
        Rung::Short
    } else {
        Rung::Bare
    };

    // 🔴 **The standing row, composed here** — where the disclosure has always been composed, and
    // now the reader's position, the keys and the connection's dot with it. The three parts are
    // budgeted in the order they may **not** be lost and drawn in the order Owner `#282-T`'s sketch
    // has them.
    //
    // It is composed **before** the rows are settled, and that is the order `req/964` §16 argued
    // for: the row this line lives on is one row by ruling, so its height is no longer a function
    // of its own text and the inversion that forced two passes is gone.
    let (dot_mark, dot_role) = measured.link.dot();
    // One space either side of the dot: `super::renderer::spans` puts one between cells, and the
    // row then reads as three parts rather than as one run.
    let dot_cells = dot_mark.chars().count() as u16 + 2; // g100: a count of sides -- a mark has two of them, and this is one cell at each
    let room = width.saturating_sub(dot_cells);
    let position = if attention.items == 0 {
        String::new()
    } else {
        format!(
            "{} of {}",
            attention.selected.min(attention.items - 1) + 1,
            attention.items
        )
    };
    // 🔴 **The floor is reserved before the caveat is composed.** The caveat may have the whole of
    // the row **except** the note's floor, which is the one thing on this row that no other row can
    // say. Without the reservation the caveat took every cell at forty and the row read
    // `! 8 keys: help:? · 8/11 fields | 2 rout~` — with no position on it at all.
    //
    // 🔴 It is the note's **whole floor line** and not just the position: since `req/924` §TUI-57
    // closed `super::renderer::fold_note`'s named ceiling, the shortest note this face draws is the
    // position *and* the clause naming where the keys went. Reserving only the position was this
    // lane's own defect, measured at eighty by twenty-four on a terminal wide enough to have
    // carried both.
    let floor = super::renderer::fold_note(std::slice::from_ref(&position), offered, 1, 1)
        .chars()
        .count() as u16;
    // 🔴 The engine's own line, spelled exactly when a claim has stopped holding (`req/924`
    // §TUI-29's other half). `measured.engine` is already the unfolded list in that state, because
    // `super::renderer::engine_line` folds to one pair only while both claims hold — so this reads
    // the fold's own decision rather than making a second one.
    let engine_caveat = if measured.healthy {
        String::new()
    } else {
        measured
            .engine
            .iter()
            .map(|(key, value)| format!("{key} {value}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    // 🔴 **What the caveat may have.** On the standing frame: everything except the note's floor
    // and the space beside it. Under `w` it may have the row, because `super::acts::Act::Wide` is
    // *spell what was let go of in full, without restarting* and a face that answered it by cutting
    // the line to one row would be answering it with a `~`.
    let budget = if wide {
        width
    } else {
        room.saturating_sub(floor + 1)
    };
    let disclosure = compose_disclosure(
        &dropped_fields,
        total_fields,
        &dropped,
        folded.then(|| measured.clone()).as_ref(),
        budget,
        wide,
        Shape {
            subject: shape,
            counts_dropped: !folded && !rung.carries_counts(),
            keys_address: &keys_address,
            wide_address: &wide_address,
            // 🔴 **Nought, and that is a repair rather than a regression** (`req/988` §3-2). That
            // clause existed for the one case where the note was given nought rows and spelled no
            // key at all; the note is on the standing row now and is drawn at every shape, so the
            // case cannot arise. What discloses a folded legend is
            // `super::renderer::note_line`'s own `N more keys` clause, which is the half of the
            // partition gate `g34` measures that never went away. Composing both would put the
            // same count on one row twice.
            keys_not_drawn: 0,
            engine_caveat: &engine_caveat,
        },
    );
    // 🔴 **The reservation is enforced and not merely offered.** `compose_disclosure` has a long
    // form and a short one and nothing below that, so at forty cells it answered thirty-one against
    // a budget of thirteen and took the note's floor with it. Cutting the caveat back to its budget
    // here is what makes the floor a floor; the cut is marked, and what it carries is on the hatch.
    let mut truncated = false;
    let disclosure = if !wide && disclosure.chars().count() > budget as usize {
        truncated = true;
        clip(&disclosure, budget as usize)
    } else {
        disclosure
    };

    // 🔴 **One row of standing chrome, and no loop.** The ladder that walked the regions existed to
    // decide which of four to give up; with two of them ruled off and the row capped at one there
    // is nothing left for it to decide. So the subject region's height is the screen's minus the
    // standing row — the same ordering the loop was enforcing, arrived at by arithmetic.
    //
    // 🔴 More than one row only while the reader is holding `w`. The **standing** frame is one row
    // (`req/924` §TUI-57) and gate `g73` measures the standing frame for that reason.
    let disclosure_rows: u16 = if height == 0 {
        0
    } else if wide {
        rows_needed(&disclosure, width)
            .max(1)
            .min(height.saturating_sub(subject_floor).max(1))
    } else {
        1
    };
    let subject_height = height.saturating_sub(disclosure_rows);
    if subject_height < subject_floor {
        truncated = true;
    }

    // 🔴 **The grid's column header is content and scrolls with the records** (`req/924` §TUI-57).
    // `preamble` is how many rows the grid puts in front of them; `super::renderer::subject` adds
    // one more when `hoist` produces a shared line and asks [`scrolled`] the same question with the
    // larger number. Both answers come from one function, so the plan and the region cannot
    // disagree about where the stream is standing.
    let preamble = match shape {
        Subject::Grid => 1usize,
        Subject::Record | Subject::Help => 0,
    };
    // 🔴 **How an opened record is cut, and it is cut here** (`[T-r58]`, 2026-09-02, defect 3).
    //
    // The members are paid for first and the rows beyond them take what is left — the order
    // `super::renderer::record_rows` argues for: the members are what the wire carried *about this
    // row* and the rows below them are what other routes said about it, so a face that cut the
    // subject to make room for the commentary would be answering a question nobody asked.
    //
    // The row the cut's own disclosure needs is taken off the budget **before** the two counts are
    // settled. Taken after, the last row the height allowed would have gone to a member and the
    // sentence saying a member was dropped would itself have been dropped.
    let (record_members_shown, record_beyond_shown, record_cut, record_ledger) = match shape {
        Subject::Grid | Subject::Help => (0usize, 0usize, false, 0usize),
        Subject::Record => record_split(
            subject_height as usize,
            attention.record_members,
            attention.record_beyond,
        ),
    };
    // 🔴 **An opened record does not own the whole region, and this is the row count that says so**
    // (`[T-r58]`, 2026-09-02, defect 3). Measured on the real capture: at 120x32 the record drew
    // eighteen rows and **thirteen rows of the screen were blank** — a third of the terminal
    // standing empty, which is furniture rather than information (`SS831`: do not stand an empty
    // panel). The rows the record does not need go back to the ledger it was opened from, so the
    // reader keeps the one fact a detail face otherwise destroys — **where this record sits among
    // the others** — and the sentence `record 1 of 31` is *shown* instead of spelled.
    //
    // Nought when the record fills the region, which is every shape at 46x12 and below.
    let grid_capacity = match shape {
        Subject::Help => 0,
        Subject::Grid => subject_height as usize,
        Subject::Record => record_ledger,
    };
    let (preamble_shown, window) = match shape {
        Subject::Help => (0usize, Window::default()),
        Subject::Grid => scrolled(
            attention.selected,
            attention.items,
            preamble,
            grid_capacity,
            attention.glide,
        ),
        // 🔴 [`window`] and not [`scrolled`]: the ledger under an opened record has no preamble to
        // scroll away and no glide of its own — the reader is not steering it, they are steering the
        // record above it. What `window` guarantees is the one property this needs, which is that
        // the attended record is inside the slice (gate `g28`), so the row whose detail is drawn
        // above is the row marked below.
        Subject::Record => (0usize, window(attention.selected, attention.items, grid_capacity)),
    };
    // 🔴 **Nought, and it is not a loss** (`req/924` §TUI-57). The note used to be the last line of
    // the subject region, paid for out of the rows the records left over — and
    // `super::renderer::note_rows`'s own documentation names the shapes where that spare was nought
    // and the legend vanished with nothing saying so. It is on the standing row now, so it is drawn
    // at **every** shape and that bounded defect is closed rather than bounded. The number stays in
    // the plan because the region still reads it, and it is nought because the region no longer
    // draws the note.
    let note_rows = 0usize;

    let ladder = super::renderer::note_ladder(
        &position,
        attention.items.checked_sub(window.rows).filter(|d| *d > 0),
        offered,
    );
    // 🔴 **The floor is a floor even when the caveat could not be cut to fit beside it.** `clip`
    // refuses to reduce a clause below [`CLIP_FLOOR`], so at thirty cells the caveat kept fifteen
    // of the twenty-seven available and the note was handed eleven — below its own floor, and the
    // row came back `8 keys: help:? · 9/11 fields` with **the reader's position gone**. `req/984`
    // §10-8 is the ruling that the position is the ladder's floor and is never given up for a
    // legend, and gates g38/g39 are what caught this lane breaking it.
    //
    // So the note is composed against at least its floor. The row then overflows, which is a cut —
    // and a cut this face marks (`!`, and `~` wherever the cut could be placed inside the budget)
    // rather than one a terminal makes in silence. What is cut is the caveat's tail, and every
    // clause of it is on the hatch.
    let note_room = room
        .saturating_sub(disclosure.chars().count() as u16 + 1)
        .max(floor)
        .max(1);
    let note = super::renderer::fold_note(
        &super::renderer::afford(&ladder, offered, note_room, 1),
        offered,
        note_room,
        1,
    );
    // 🔴 **A cut is marked twice: `!` in front of the row, and `~` where the cut fell.** A terminal
    // clips from the right without saying so, and the clause it would clip is the one that says
    // what is missing. Cutting it here means the mark is inside the budget and the reader can see
    // *which* clause ran out rather than being handed a sentence that stops mid-word.
    //
    // The `!` and the space after it are two cells the row spends the moment it is cut, so the room
    // a cut row has is two fewer than the room an uncut one has. Left out of this arithmetic the
    // marks themselves went off the right edge, which is a cut nothing marked.
    let spent = note.chars().count() + disclosure.chars().count() + dot_cells as usize;
    let marked_room = width.saturating_sub(2) as usize; // g100: the cut mark and the space after it, each one character, so this is a count of characters rather than a chosen width
    let disclosure = if !wide && spent > width as usize {
        truncated = true;
        clip(
            &disclosure,
            disclosure
                .chars()
                .count()
                .saturating_sub(spent.saturating_sub(marked_room)),
        )
    } else {
        disclosure
    };
    // 🔴 **The dot survives every width, and it did not** (gate `g19b`, 2026-09-02). It is drawn
    // after the note, so a note wider than the row took the dot off the screen with it — and at
    // forty cells **all five states of the connection drew the same frame**, which is the collapse
    // `req/38` SS1085 is named for, reintroduced by a repair to the note.
    //
    // `Measured::bare`'s ruling, one row down: *the mark is never given up, and it leads the line
    // for that reason — a terminal cuts from the right, so what cannot be recovered goes at the
    // front.* The Owner's sketch puts the dot in the middle, so instead of moving it the **note**
    // is cut back to leave it room. The order of survival on this row is: the reader's position,
    // then the connection's state, then the road, then the caveat's tail.
    let note = {
        let room = width.saturating_sub(dot_cells + u16::from(truncated) * 2) as usize; // g100: the same two characters as `marked_room` above -- the mark and its space
        if note.chars().count() > room {
            truncated = true;
            clip(&note, room)
        } else {
            note
        }
    };
    let mut status: Vec<HeadingCell> = Vec::new();
    if !note.is_empty() {
        status.push(HeadingCell {
            text: note.clone(),
            role: super::tokens::Role::Quiet,
        });
    }
    status.push(HeadingCell {
        text: dot_mark.to_string(),
        role: dot_role,
    });
    if !disclosure.is_empty() {
        status.push(HeadingCell {
            text: disclosure.clone(),
            role: super::tokens::Role::Quiet,
        });
    }
    let mut rows: Vec<(RegionRole, u16)> = Vec::new();
    if subject_height > 0 {
        rows.push((RegionRole::Subject, subject_height));
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
        heading: head_cells,
        framed,
        // The rung was decided above, beside the fold that describes it. Spelling it here would be
        // a second decision, and the two could differ.
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
        engine_full: measured.engine_full.clone(),
        vacant_fields: measured.vacant.clone(),
        status,
        note,
        preamble,
        preamble_shown,
        record_members_shown,
        record_beyond_shown,
        record_cut,
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
