// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The paint ladder: the second axis of `req/942` §11's four layers, one rung per module boundary.
//!
//! ```text
//! intent  ->  role  ->  token  ->  value
//! ```
//!
//! `super::layout` holds the ladder for **placement**. This module holds the one for **paint**: a
//! caller names a [`Role`] — what a thing is, in the reader's words — and never a hue. A role
//! resolves to a [`Token`], a token to an [`Ink`], and the ink is turned into the medium's own type
//! in exactly one place (`super::renderer`), which is the seam gate g5 already measures.
//!
//! # Where the paint axis's `intent` rung is, and why it is not a type in this file
//!
//! 🔴 **The ruling asked for by `req/924` §TUI-15 and `req/942_artifacts/tui_r37_2026-09-01`.**
//! That census read this file, found no `Intent`, and recorded the paint axis's `intent -> role`
//! map as UNTESTABLE — correctly, because a map with no domain has nothing to check. The remedy it
//! offered was to stand a type up here. **This face does not, and the reason is mechanical rather
//! than economical.**
//!
//! The rung is not missing. It is `super::wire`, and it is written there on purpose: an intent is
//! *what a thing is in the product's words*, and the product's words arrive on the wire.
//! [`super::wire::Nothing::role`], [`super::wire::VerdictMark::role`] and
//! [`super::wire::InverseMark::role`] are exactly the map the doctrine asks for — each takes a word
//! the engine spells and answers with the [`Role`] it is painted in — and they are the join that
//! makes this face gateable by the engine's own vocabulary rather than by its own opinion of
//! itself. An `Intent` enum here would be a **second** name for that one join, and this face has
//! now closed the same defect three times (`super::acts::grounded`, [`super::renderer::note_rows`],
//! `super::layout::subject_shape` are all "one classifier, read twice" rulings): two names for one
//! join disagree the day one of them is edited.
//!
//! What was actually missing was not a type but the **gate**, and gate g55 is it: every declared
//! paint intent resolves to a role, no two resolve to the same one, and the intent set covers
//! [`ROLES`] exactly. It would have caught `req/38` SS974 — where an empty string and a count of
//! nought were painted in one role — on the run rather than in a reading.
//!
//! **The case against this ruling, since it is not free.** Three costs, none hidden:
//!
//! 1. The two ladders are no longer the same shape. `super::layout` has four named rungs and this
//!    axis has three, with `Role` carrying the top two. The sentence that used to stand here — "the
//!    two are the same shape on purpose" — is now false and has been struck rather than left to rot.
//! 2. By the same argument, `super::layout::Intent` is *also* in bijection with
//!    `super::layout::RegionRole`, so that type is an alias table by this file's own standard. It is
//!    not deleted: it is out of the path of the defect this ruling was made against, and deleting a
//!    declaration to make an argument tidy is how a face loses the words it cannot get back.
//! 3. g55 is **green on the source it was written against**. It is a tripwire for the next edit and
//!    not a finding, and saying so is the whole difference between a gate and a decoration.
//!
//! # Why this file exists at all, said plainly
//!
//! The first build of this face had the values in the drawing code: `Color::Rgb(214, 188, 106)` sat
//! inside the method that decided what a header looks like, and the marks in the table cells had no
//! declared appearance at all — they were drawn raw, which is not "no colour by policy", it is "no
//! colour because nobody asked". The two look identical on screen and are opposite facts about the
//! program. A dictionary makes the difference readable: [`Token::Plain`] is a decision, spelled.
//!
//! # What `mono` means, exactly
//!
//! [`Tier::Mono`] resolves every token to an ink with **no** colour of any kind, and the modifiers
//! survive. That is the whole of what the tier means, and it is what keeps colour from carrying a
//! meaning: every mark in this face is distinguishable by its symbol alone (`P2` measures that on
//! `mono`), so a tier that spells no hue loses emphasis and never loses information.

/// How much colour the terminal can carry.
///
/// 🔴 Declared here rather than in the renderer, where it was: a tier is a property of the
/// **reader's terminal**, and the value a token takes in it belongs beside the token table. The
/// renderer re-exports the type so that the road a caller already walks (`renderer::Tier`) is
/// unchanged.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    /// 24-bit colour.
    Truecolor,
    /// The 256-colour cube.
    Ansi256,
    /// The original sixteen.
    Ansi16,
    /// No colour at all.
    Mono,
}

impl Tier {
    /// All four, for the sweep in `tui/tests/r942_tui.rs`.
    pub const ALL: [Tier; 4] = [Tier::Truecolor, Tier::Ansi256, Tier::Ansi16, Tier::Mono];

    /// Read the environment. `NO_COLOR` wins over everything, as its own specification asks.
    #[must_use]
    pub fn detect() -> Self {
        if std::env::var_os("NO_COLOR").is_some() {
            return Tier::Mono;
        }
        let term = std::env::var("TERM").unwrap_or_default();
        if term.is_empty() || term == "dumb" {
            return Tier::Mono;
        }
        match std::env::var("COLORTERM").unwrap_or_default().as_str() {
            "truecolor" | "24bit" => Tier::Truecolor,
            _ if term.contains("256") => Tier::Ansi256,
            _ => Tier::Ansi16,
        }
    }

    /// The name used in the report and in `--tier`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Tier::Truecolor => "truecolor",
            Tier::Ansi256 => "256",
            Tier::Ansi16 => "16",
            Tier::Mono => "mono",
        }
    }

    /// Parse `--tier`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Tier::ALL.into_iter().find(|tier| tier.name() == text)
    }
}

/// What a drawn thing **is**. The top of the paint ladder, and the only rung a component names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// A header: the column names, and the engine's own line about itself.
    Head,
    /// The lines that say where the numbers came from and what is not on the screen.
    Quiet,
    /// A value the wire carried.
    Body,
    /// The record the reader is attending to.
    Attend,
    /// The mark for a reading that has not happened yet.
    MarkLoading,
    /// The mark for measured-and-not-knowable.
    MarkUnknown,
    /// The mark for a key the wire never carried.
    MarkAbsent,
    /// The mark for an answer of no.
    MarkFalse,
    /// The mark for a count of nought.
    MarkZero,
    /// The mark for a row that was struck out.
    MarkDeleted,
    /// 🔴 The mark for a value that arrived with nothing in it (`req/38` SS974 row Q4). Its own
    /// role, and not `mark.zero`'s, because the whole of that ruling is that the two are different
    /// answers — and a shared role would put them back together one rung below the symbol.
    MarkEmpty,
    /// The engine admitted the transformation.
    VerdictAdmit,
    /// The engine refused it.
    VerdictDeny,
    /// The engine asked a human.
    VerdictEscalate,
    /// 🔴 There is no verdict on the wire for this row. Not a fourth verdict: a fourth **mark**.
    VerdictNone,
    /// The connection is up and something arrived inside [`super::live::QUIET_AFTER`].
    ///
    /// 🔴 The six `link.*` roles are one decision (`req/924` §TUI-57, `req/38` SS1088, Owner
    /// `#282-T`). The rail used to spell `ENGINE LIVE, 151 events` and `engine ok`; a dot replaces
    /// both, and a dot with one appearance would put `SS1085`'s finding back on the screen — *a
    /// quiet stream and a dead stream wearing one face*. So the states are **not** collapsed: each
    /// of [`super::live::LINKS`]'s five gets a role, and the one state that hides a second fact
    /// inside it — an open connection nothing is arriving on — gets a sixth. Gate `g19` already
    /// required the five to be drawn differently and still does.
    ///
    /// 🔴 They are **not** the seven words for nothing and must never borrow one of their roles
    /// (`req/924` §TUI-48): being connected is not an absence, and being severed is a measurement.
    LinkLive,
    /// The connection is up and nothing has arrived for [`super::live::QUIET_AFTER`] or longer.
    LinkQuiet,
    /// The connection has been asked for and has not answered yet.
    LinkOpening,
    /// The connection has been asked for and has never once been up.
    LinkNever,
    /// The connection has been up and is not up now.
    LinkClosed,
    /// This run does not subscribe at all.
    LinkOff,
}

/// Every paint role this face declares. Gate g14 requires each one to resolve to a value, and gate
/// g16 pins which value.
pub const ROLES: [Role; 21] = [
    Role::Head,
    Role::Quiet,
    Role::Body,
    Role::Attend,
    Role::MarkLoading,
    Role::MarkUnknown,
    Role::MarkAbsent,
    Role::MarkFalse,
    Role::MarkZero,
    Role::MarkDeleted,
    Role::MarkEmpty,
    Role::VerdictAdmit,
    Role::VerdictDeny,
    Role::VerdictEscalate,
    Role::VerdictNone,
    Role::LinkLive,
    Role::LinkQuiet,
    Role::LinkOpening,
    Role::LinkNever,
    Role::LinkClosed,
    Role::LinkOff,
];

impl Role {
    /// The spelled name, for a gate and for a reader of a report.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Role::Head => "paint.head",
            Role::Quiet => "paint.quiet",
            Role::Body => "paint.body",
            Role::Attend => "paint.attend",
            Role::MarkLoading => "mark.loading",
            Role::MarkUnknown => "mark.unknown",
            Role::MarkAbsent => "mark.absent",
            Role::MarkFalse => "mark.false",
            Role::MarkZero => "mark.zero",
            Role::MarkDeleted => "mark.deleted",
            Role::MarkEmpty => "mark.empty",
            Role::VerdictAdmit => "verdict.admit",
            Role::VerdictDeny => "verdict.deny",
            Role::VerdictEscalate => "verdict.escalate",
            Role::VerdictNone => "verdict.none",
            Role::LinkLive => "link.live",
            Role::LinkQuiet => "link.quiet",
            Role::LinkOpening => "link.opening",
            Role::LinkNever => "link.never",
            Role::LinkClosed => "link.closed",
            Role::LinkOff => "link.off",
        }
    }

    /// The token this role resolves to.
    ///
    /// A `match` rather than a table: the compiler makes the map **total**, which is the property a
    /// lookup table can only be checked for after the fact.
    #[must_use]
    pub fn token(self) -> Token {
        match self {
            Role::Head => Token::Accent,
            Role::Body | Role::MarkZero => Token::Plain,
            Role::Attend => Token::Attend,
            // 🔴 `mark.empty` is quiet and `mark.zero` is not, and that is the pairing the SS974
            // ruling is about: a count of nought is a value the wire carried, and a value with
            // nothing in it is an absence. They are told apart at the symbol layer **and** here.
            Role::Quiet
            | Role::MarkLoading
            | Role::MarkUnknown
            | Role::MarkAbsent
            | Role::MarkFalse
            | Role::MarkEmpty
            | Role::VerdictNone => Token::Thin,
            Role::MarkDeleted | Role::VerdictDeny => Token::Refuse,
            Role::VerdictAdmit => Token::Affirm,
            Role::VerdictEscalate => Token::Accent,
            // 🔴 Four hues over six states, and the glyph is what carries the sixth distinction —
            // `INHERITED_PRINCIPLES` §3c-③''-③: no meaning may rest on **one** tier. On `mono`
            // every hue is dropped and the six dots are still six different characters, so the
            // reader who cannot see colour loses nothing. `link.never` and `link.closed` share
            // `refuse` because both are the connection being down; they are told apart by the mark,
            // which is the same rule `mark.zero`/`mark.empty` are told apart by one rung up.
            Role::LinkLive => Token::Affirm,
            Role::LinkQuiet => Token::Accent,
            Role::LinkNever | Role::LinkClosed => Token::Refuse,
            Role::LinkOpening | Role::LinkOff => Token::Thin,
        }
    }
}

/// The value layer's names. One bundle each, one spelling per tier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Token {
    /// The terminal's own foreground, whatever the reader chose it to be.
    Plain,
    /// Quieter, by a modifier rather than by a hue — dimming is the one emphasis every tier has.
    Thin,
    /// The face's one accent.
    Accent,
    /// Admitted.
    Affirm,
    /// Refused, or struck out.
    Refuse,
    /// The record being attended to: the fore and the ground swapped, which survives `mono` and a
    /// monochrome capture both.
    Attend,
}

/// Every token this face declares. Gate g14 requires each one to be named by at least one role: a
/// value nothing resolves to is a value nobody maintains.
pub const TOKENS: [Token; 6] = [
    Token::Plain,
    Token::Thin,
    Token::Accent,
    Token::Affirm,
    Token::Refuse,
    Token::Attend,
];

impl Token {
    /// The spelled name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Token::Plain => "plain",
            Token::Thin => "thin",
            Token::Accent => "accent",
            Token::Affirm => "affirm",
            Token::Refuse => "refuse",
            Token::Attend => "attend",
        }
    }
}

/// A glyph this face draws that is **outside** `U+0020..=U+007E`, the meaning it carries, and the
/// words that are not on the screen because it is.
///
/// 🔴 **A declaration, because what it widens is a gate.** Until Owner #227 (2026-09-01, the TUI
/// session's entry in `req/OWNER_VERBATIM_2026-08-29.md`) this face's own vocabulary was ASCII by
/// rule and `P6` refused everything else. That rule bought a real property — a terminal draws a
/// codepoint its font is missing as a box, and a reader reads a box as *this program is broken*,
/// which is the worst available reading of a mark that means "measured, and not knowable". The
/// ruling did not throw the property away; it moved it. The budget is now `U+0020..=U+007E`
/// **plus this array**, so the set is still small, still enumerated, and still the thing a gate
/// reads rather than something a screen may reach for.
///
/// The admission test is [`Glyph::instead_of`] and it is not decoration: a glyph earns its place
/// only when it carries a meaning and **thereby deletes a word**. A glyph beside a word that stays
/// is weight, and this face spends cells on weight last.
///
/// 🔴 The seven words for nothing (`super::wire::Nothing::mark`) are **not** in scope here and are
/// not to be respelled. They are the one vocabulary where a substitution destroys a distinction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Glyph {
    /// What is drawn.
    pub text: &'static str,
    /// What it means, in the reader's words.
    pub means: &'static str,
    /// The words that are not on the screen because this glyph is.
    pub instead_of: &'static str,
}

/// Every glyph outside the ASCII budget this face may draw. Gate `P6` reads this array.
///
/// 🔴 **Five entries, and the four that were added are one decision** (`req/924` §TUI-22,
/// `req/38` SS1049, Owner `#266-T`). They are the corners of the enclosure the ledger is drawn
/// inside, and they are admitted under the same test every glyph here is admitted under: the mark
/// carries a meaning and **thereby deletes words**. The words it deletes are named in
/// `instead_of` and they are not hypothetical — before this the screen spelled
/// `screen: apparatus subject provenance disclosure`, an eight-word rail naming its own internal
/// regions, and it spelled `GET /v1/transformations` on four separate rows so that each part
/// could say for itself where it came from. A boundary a reader can see says both of those
/// things without a word.
///
/// The corners are four glyphs and not one because a rectangle has four corners and each names a
/// different one of them; they are **not** four decisions, and `instead_of` says so by naming the
/// same deleted words for all four. There is no horizontal or vertical rule between them, and that
/// is measured rather than aesthetic: a run of `─` is ink, a hundred and twenty cells of it is more
/// ink than the four rows the enclosure was brought in to delete, and this lane's whole purpose is
/// the ratio of ink spent on the machine to ink spent on the ledger.
pub const GLYPHS: [Glyph; 14] = [
    Glyph {
        text: "\u{2502}",
        means: "a boundary between two parts of one screen",
        instead_of: "the labels that would otherwise have to name each part in words",
    },
    Glyph {
        text: "\u{250c}",
        means: "the ledger's enclosure opens here, at the top left",
        instead_of: "`screen: apparatus subject provenance disclosure`, and the repeated \
                     `GET /v1/transformations` each part used to spell for itself",
    },
    Glyph {
        text: "\u{2510}",
        means: "the ledger's enclosure opens here, at the top right",
        instead_of: "`screen: apparatus subject provenance disclosure`, and the repeated \
                     `GET /v1/transformations` each part used to spell for itself",
    },
    Glyph {
        text: "\u{2514}",
        means: "the ledger's enclosure closes here, at the bottom left",
        instead_of: "`screen: apparatus subject provenance disclosure`, and the repeated \
                     `GET /v1/transformations` each part used to spell for itself",
    },
    Glyph {
        text: "\u{2518}",
        means: "the ledger's enclosure closes here, at the bottom right",
        instead_of: "`screen: apparatus subject provenance disclosure`, and the repeated \
                     `GET /v1/transformations` each part used to spell for itself",
    },
    // 🔴 **The three marks the note spells acts with** (`req/924` §TUI-45 row 4,
    // `INHERITED_PRINCIPLES` §3c-③''). The admission test is the one this array was built to
    // record: a mark earns its cells by carrying a meaning **and thereby deleting a word**, and
    // `instead_of` is where the deleted word is written down. A declaration with an empty
    // `instead_of` fails P6, which is that ruling made mechanical.
    Glyph {
        text: "\u{21b5}",
        means: "the return key, and the act it produces: open the attended record",
        instead_of: "`open:return` — both words go, because the mark is the key",
    },
    Glyph {
        text: "\u{2191}",
        means: "move the attention toward the top of the list",
        instead_of: "`prev`",
    },
    Glyph {
        text: "\u{2193}",
        means: "move the attention toward the bottom of the list",
        instead_of: "`next`",
    },
    // 🔴 **The six dots** (`req/924` §TUI-57, `req/38` SS1088, Owner `#282-T`). They are admitted
    // under the test this array exists to record and they pass it in the strong direction: the rail
    // that spelled `ENGINE LIVE, 151 events` and `engine ok` is **gone**, and these marks are what
    // stands in its place. `§TUI-30` refused `● CONNECTED` because the word stayed beside the mark;
    // here the word is what leaves.
    //
    // Six and not one, because `SS1085` measured the defect a single dot would ship: `ENGINE LIVE`
    // said the connection had been made and said nothing about whether anything was still arriving,
    // so *a quiet stream and a dead stream wore one face*. The states are told apart by the mark
    // itself and not only by the hue, which is `INHERITED_PRINCIPLES` §3c-③''-③.
    Glyph {
        text: "\u{25CF}",
        means: "the connection is up and an event arrived recently",
        // 🔴 **`engine ok` is not in this list** (independent audit F-05, 2026-09-02).
        // [`super::live::LinkReport::dot`] matches on the connection's state alone and never reads
        // the engine's health, so a degraded engine on a live stream draws this mark. The words
        // `engine ok` replaced are spelled by the standing row's caveat clause
        // (`super::layout::Shape::engine_caveat`), which appears exactly when one of the engine's
        // two claims stops holding. A declaration that claimed both would be this face saying the
        // mark measures more than it does.
        instead_of: "`ENGINE LIVE, N events`",
    },
    Glyph {
        text: "\u{25D0}",
        // 🔴 **`or ever`** (independent audit F-05, 2026-09-02). The `None` arm of
        // [`super::live::LinkReport::silent_for`] takes this mark, and a legend reading only *for a
        // while* is false for a connection that has been up since it opened and carried nothing.
        // The two are told apart in words by [`super::live::LinkReport::silence`], which the hatch
        // draws — `no event has arrived on this connection` against `last event Ns ago`. The mark
        // is one state; the sentence beside it is which of the two.
        means: "the connection is up and nothing has arrived inside the quiet window, or ever",
        instead_of: "`ENGINE LIVE, N events` — which said this and the state above with one phrase",
    },
    Glyph {
        text: "\u{25CC}",
        means: "the connection has been asked for and has not answered yet",
        instead_of: "`connecting`",
    },
    Glyph {
        text: "\u{00D7}",
        means: "the connection has been asked for and has never once been up",
        instead_of: "`never connected in N attempts`",
    },
    Glyph {
        text: "\u{25CB}",
        means: "the connection has been up and is not up now",
        instead_of: "`closed after N events, M reconnects`",
    },
    Glyph {
        text: "\u{00B7}",
        means: "this run does not subscribe, so there is no connection for events to arrive on",
        instead_of: "`not subscribed`",
    },
];

/// The boundary between two parts of one screen.
///
/// 🔴 Read out of [`GLYPHS`] rather than typed a second time: a second spelling is a glyph the
/// gate's allow-list does not cover, which is the failure the array exists to make impossible.
pub const RULE: &str = GLYPHS[0].text;

/// The declaration for one mark, found by the mark itself.
///
/// 🔴 So that a gate can ask *what did this glyph say it deleted* without indexing the array, which
/// is the pairing an audit of this lane found to be positional and therefore silent under a swap.
#[must_use]
pub fn glyph(text: &str) -> Option<&'static Glyph> {
    GLYPHS.iter().find(|glyph| glyph.text == text)
}

/// The four corners of the ledger's enclosure, read out of [`GLYPHS`] for the reason [`RULE`] is.
///
/// Order: top-left, top-right, bottom-left, bottom-right.
pub const CORNERS: [&str; 4] = [
    GLYPHS[1].text,
    GLYPHS[2].text,
    GLYPHS[3].text,
    GLYPHS[4].text,
];

/// The six marks the connection's state is drawn with, read out of [`GLYPHS`] for the reason
/// [`CORNERS`] is: a second spelling is a glyph the allow-list does not cover.
///
/// Order: live, quiet, opening, never, closed, off — the same order
/// [`super::live::LinkReport::dot`] answers in, and gate `g74` requires the six to be six.
pub const DOTS: [&str; 6] = [
    GLYPHS[8].text,
    GLYPHS[9].text,
    GLYPHS[10].text,
    GLYPHS[11].text,
    GLYPHS[12].text,
    GLYPHS[13].text,
];

/// One resolved value: a colour in the spelling this tier can carry, and the emphasis.
///
/// 🔴 At most one of the three colour members is ever `Some`. They are three spellings of one
/// decision, not three decisions, and the tier chooses the spelling.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Ink {
    /// 24-bit.
    pub rgb: Option<(u8, u8, u8)>,
    /// The 256-colour cube's index.
    pub c256: Option<u8>,
    /// One of the original sixteen.
    pub c16: Option<u8>,
    /// Heavier.
    pub bold: bool,
    /// Quieter.
    pub dim: bool,
    /// Fore and ground swapped.
    pub reversed: bool,
}

impl Ink {
    /// Whether this ink spells a colour at all. `false` for every ink on `mono`.
    #[must_use]
    pub const fn has_colour(&self) -> bool {
        self.rgb.is_some() || self.c256.is_some() || self.c16.is_some()
    }
}

/// The value a role takes in a tier — the bottom of the ladder, and the only place in this face
/// where a colour is written down.
#[must_use]
pub fn ink(role: Role, tier: Tier) -> Ink {
    let token = role.token();
    let emphasis = Ink {
        bold: token == Token::Accent,
        dim: token == Token::Thin,
        reversed: token == Token::Attend,
        ..Ink::default()
    };
    // The hues, in the three spellings. `Plain`, `Thin` and `Attend` carry none by decision: the
    // first is the reader's own foreground, and the other two say what they say with an emphasis
    // that no tier can drop.
    let (rgb, c256, c16) = match token {
        Token::Plain | Token::Thin | Token::Attend => return emphasis,
        Token::Accent => ((214, 188, 106), 179, 3),
        Token::Affirm => ((126, 166, 120), 107, 2),
        Token::Refuse => ((198, 112, 88), 167, 1),
    };
    match tier {
        Tier::Truecolor => Ink {
            rgb: Some(rgb),
            ..emphasis
        },
        Tier::Ansi256 => Ink {
            c256: Some(c256),
            ..emphasis
        },
        Tier::Ansi16 => Ink {
            c16: Some(c16),
            ..emphasis
        },
        // 🔴 The tier that spells no hue. Not a degradation: the emphasis stays, and no mark in this
        // face needed a hue to be told from another one.
        Tier::Mono => emphasis,
    }
}

// =================================================================================================
// The placement ladder
// =================================================================================================
//
// 🔴 **`[T-r87]`, 2026-09-02, and it is the answer to a question the Owner asked of this face:**
// *いかなる見た目上の要求に Token 変更のみで対応しうるんだろうな — は?どういうこと配置とか全部含め
// Token じゃないの?* (`req/OWNER_VERBATIM`). The honest answer, measured before this section was
// written, was **no**: the paint axis resolved through the ladder above and refused a raw value at
// the seam (gate `g13`), while every placement magnitude this face draws with — ten column widths,
// ten column ranks, ten column orders, five frame magnitudes and four region floors — was a
// numeral typed into `super::layout`. So a request of the form *put that column first*, *make it
// narrower*, *fold it away* was a **code** change, and the four-layer claim in `super::layout`'s
// own header was true of colour and false of placement.
//
// What follows is the same ladder, for the axis that had none:
//
// ```text
// intent  ->  slot  ->  measure  ->  cells
// ```
//
// [`Slot`] is what a placed thing **is** — the rung a caller names, exactly as [`Role`] is on the
// paint axis. [`Measure`] is the named quantity it resolves to. [`cells`] is the value, and it is
// the only place in this face where a placement magnitude is written down.
//
// # `Grade` is the placement axis's `Tier`, and it is not an analogy
//
// [`Tier`] exists because one declared colour has to be spelled differently on terminals of
// different capability, and the value layer is where the spelling is chosen. Placement has exactly
// the same shape with a different axis: **a width is a capability of the reader's terminal**, and a
// column that is honest at a hundred and twenty cells is a lie at forty. [`Grade`] is that axis, its
// five classes are the five widths this face is measured at (`[T-r87]`'s brief:
// `40 / 46 / 60 / 80 / 120`), and [`cells`] takes it for the reason [`ink`] takes a tier.
//
// # `Scheme` is what makes the answer to the Owner's question demonstrable rather than asserted
//
// One declaration table is a table. **Three** are a proof that the table is the thing the screen
// obeys: [`Scheme::Ledger`] is what this face drew before this section existed, and the other two
// are different screens produced **without one line of drawing code changing**. If a scheme swap
// did not move the screen, the ladder would be decoration and this section would be a lie a gate
// could not see.
//
// # What deliberately does **not** vary, and the mechanism that says why
//
// [`Measure::Lead`], [`Measure::Gap`], [`Measure::Frame`] and [`Measure::Floor`] answer the same
// number at every grade and in every scheme. That is a **ruling, not an oversight**, and the reason
// is `[T-r66]`: the row's price (`super::layout::row_width`) and the row's drawing
// (`super::renderer::spans_with`) are two readers of those magnitudes, and the defect `[T-r66]`
// repaired was the two disagreeing by exactly one gap — a budget two cells looser than the screen,
// a column kept that did not fit, and a value clipped in silence. Making them vary would put a
// second axis between the price and the row. They go through the ladder (so nothing types them
// twice) and they are constant along it (so the price and the row cannot diverge again).

/// How wide the reader's terminal is, in classes. The placement axis's [`Tier`].
///
/// 🔴 Five, and they are not chosen for tidiness: they are the five widths `[T-r87]` is required to
/// capture at, so every class this face declares is a class a capture exists for. A sixth class
/// nothing is ever photographed at is a declaration no one can check.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grade {
    /// Narrower than the first ceiling. The shape the brief calls the real one: `40x10`.
    Crammed,
    /// The second class, which `46` cells falls in.
    Narrow,
    /// The third class, which `60` cells falls in.
    Snug,
    /// The fourth class, which `80` cells falls in.
    Roomy,
    /// `120` and wider.
    Full,
}

/// How many grades there are. Declared so the span table's length is the declaration's length.
pub const GRADE_COUNT: usize = 5;

/// The width at which each grade gives way to the next, ascending. One shorter than [`GRADE_COUNT`],
/// because the last class has no ceiling.
///
/// 🔴 The numerals live **here**, which is the whole of what this section is for. An `if width < 80`
/// in a screen is the shape `super::layout`'s header has always refused for regions; these four
/// numbers are that refusal made true of columns as well.
pub const GRADE_CEILINGS: [u16; GRADE_COUNT - 1] = [46, 60, 80, 120];

impl Grade {
    /// All five, for the sweep in `tui/tests/r942_tui.rs`.
    pub const ALL: [Grade; GRADE_COUNT] = [
        Grade::Crammed,
        Grade::Narrow,
        Grade::Snug,
        Grade::Roomy,
        Grade::Full,
    ];

    /// Where this grade stands in [`Self::ALL`], which is the index the span table is read at.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Grade::Crammed => 0,
            Grade::Narrow => 1,
            Grade::Snug => 2,
            Grade::Roomy => 3,
            Grade::Full => 4,
        }
    }

    /// Which class a terminal of this width is in.
    ///
    /// Read from [`GRADE_CEILINGS`] rather than written as a chain of comparisons: a chain is four
    /// numbers a gate has to parse back out of control flow, and a table is four numbers a gate
    /// reads.
    #[must_use]
    pub const fn of(width: u16) -> Grade {
        let mut at = 0;
        while at < GRADE_CEILINGS.len() {
            if width < GRADE_CEILINGS[at] {
                return Grade::ALL[at];
            }
            at += 1;
        }
        Grade::Full
    }

    /// The name used in a report.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Grade::Crammed => "crammed",
            Grade::Narrow => "narrow",
            Grade::Snug => "snug",
            Grade::Roomy => "roomy",
            Grade::Full => "full",
        }
    }

    /// Parse a grade's name.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Grade::ALL.into_iter().find(|grade| grade.name() == text)
    }
}

/// The declared order of letting go. `One` is the last to be dropped.
///
/// 🔴 **One type, and it used to be two.** `super::layout::Priority` was declared there and this
/// axis had no rank at all, so a column's rank was a name typed at the column and a region's rank
/// was a name typed at the region. `super::layout::Priority` is now a re-export of this, for the
/// reason `super::wire` rather than a second `Intent` holds the paint axis's top rung: **two names
/// for one join disagree the day one of them is edited.**
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Rank {
    /// Never dropped while anything is drawn at all.
    One,
    /// Dropped after everything below it.
    Two,
    /// Dropped early.
    Three,
    /// Dropped first.
    Four,
}

/// What a placed thing **is**. The top of the placement ladder, and the only rung a caller names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    /// The column that tells one record from the next.
    Discriminator,
    /// The column carrying the engine's answer.
    Judgement,
    /// The column carrying where the record is in its life.
    Lifecycle,
    /// The column carrying when it happened.
    Instant,
    /// The column carrying what it reached.
    Reach,
    /// The column carrying whether it was held to.
    Binding,
    /// The column carrying whether the inverse is there.
    Reversal,
    /// The column carrying whether it was taken back.
    Undoing,
    /// The column carrying which record replaced it.
    Successor,
    /// The column carrying who asked.
    Hand,
    /// The cells before the first column of a row.
    RowLead,
    /// The cells between one cell and the next.
    CellGap,
    /// The cells the ledger's enclosure takes out of a rail's row.
    Enclosure,
    /// How many records share one group before the rule.
    GroupRun,
    /// The fewest cells a cut clause may be left with.
    ClipFloor,
    /// The fewest rows the engine's own account of itself says anything in.
    BandApparatus,
    /// The fewest rows the ledger says anything in.
    BandLedger,
    /// The fewest rows the measured facts say anything in.
    BandProvenance,
    /// The fewest rows the line naming what went says anything in.
    BandDisclosure,
    /// How many different values a column may carry over the records the read carried and still be
    /// said once at the head instead of on every row.
    FoldVoices,
    /// How many records there must be before saying a column once at the head is worth the row it
    /// costs.
    FoldQuorum,
}

/// Every placement slot this face declares.
pub const SLOTS: [Slot; 21] = [
    Slot::Discriminator,
    Slot::Judgement,
    Slot::Lifecycle,
    Slot::Instant,
    Slot::Reach,
    Slot::Binding,
    Slot::Reversal,
    Slot::Undoing,
    Slot::Successor,
    Slot::Hand,
    Slot::RowLead,
    Slot::CellGap,
    Slot::Enclosure,
    Slot::GroupRun,
    Slot::ClipFloor,
    Slot::BandApparatus,
    Slot::BandLedger,
    Slot::BandProvenance,
    Slot::BandDisclosure,
    Slot::FoldVoices,
    Slot::FoldQuorum,
];

/// The ten slots that are columns of the ledger, in the order they are **declared**, which is not
/// necessarily the order they are drawn in — that is [`Cells::order`], and it is a scheme's to say.
pub const LEDGER_SLOTS: [Slot; 10] = [
    Slot::Discriminator,
    Slot::Judgement,
    Slot::Lifecycle,
    Slot::Instant,
    Slot::Reach,
    Slot::Binding,
    Slot::Reversal,
    Slot::Undoing,
    Slot::Successor,
    Slot::Hand,
];

/// A byte-for-byte comparison usable in a `const fn`, since `str::eq` is not one on this toolchain.
const fn same(left: &str, right: &str) -> bool {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    if left.len() != right.len() {
        return false;
    }
    let mut at = 0;
    while at < left.len() {
        if left[at] != right[at] {
            return false;
        }
        at += 1;
    }
    true
}

impl Slot {
    /// The spelled name, for a gate and for a reader of a report.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Slot::Discriminator => "cell.discriminator",
            Slot::Judgement => "cell.judgement",
            Slot::Lifecycle => "cell.lifecycle",
            Slot::Instant => "cell.instant",
            Slot::Reach => "cell.reach",
            Slot::Binding => "cell.binding",
            Slot::Reversal => "cell.reversal",
            Slot::Undoing => "cell.undoing",
            Slot::Successor => "cell.successor",
            Slot::Hand => "cell.hand",
            Slot::RowLead => "frame.lead",
            Slot::CellGap => "frame.gap",
            Slot::Enclosure => "frame.enclosure",
            Slot::GroupRun => "frame.run",
            Slot::ClipFloor => "frame.floor",
            Slot::BandApparatus => "band.apparatus",
            Slot::BandLedger => "band.ledger",
            Slot::BandProvenance => "band.provenance",
            Slot::BandDisclosure => "band.disclosure",
            Slot::FoldVoices => "fold.voices",
            Slot::FoldQuorum => "fold.quorum",
        }
    }

    /// The wire's own key for a slot that is a column, and [`None`] for one that is not.
    ///
    /// 🔴 The key is the **wire's**, never a synonym, which is the rule
    /// `super::layout::Column::key` has always carried and gate `P5` measures. This is where the two
    /// vocabularies are joined, and it is the only place: `super::layout::LEDGER_COLUMNS` is built
    /// out of this rather than typing the keys a second time.
    #[must_use]
    pub const fn key(self) -> Option<&'static str> {
        Some(match self {
            Slot::Discriminator => "transformation",
            Slot::Judgement => "verdict",
            Slot::Lifecycle => "state",
            Slot::Instant => "created_at",
            Slot::Reach => "scope",
            Slot::Binding => "enforced",
            Slot::Reversal => "inverse_status",
            Slot::Undoing => "rollback",
            Slot::Successor => "superseded_by",
            Slot::Hand => "actor",
            _ => return None,
        })
    }

    /// Which slot draws this wire key, if any.
    #[must_use]
    pub const fn of_key(key: &str) -> Option<Slot> {
        let mut at = 0;
        while at < LEDGER_SLOTS.len() {
            let slot = LEDGER_SLOTS[at];
            if let Some(own) = slot.key() {
                if same(own, key) {
                    return Some(slot);
                }
            }
            at += 1;
        }
        None
    }

    /// Whether this slot's value is a count of **things** rather than of cells.
    ///
    /// 🔴 Declared rather than left to be inferred from the name. [`Cells::width`] carries both, and
    /// a reader who assumes every number in that member is a cell count would read `fold.voices = 3`
    /// as three cells. The two are told apart here, which is the same discipline the seven words for
    /// nothing are told apart by: a value and what kind of value it is are two facts.
    #[must_use]
    pub const fn counts(self) -> bool {
        matches!(
            self,
            Slot::GroupRun
                | Slot::BandApparatus
                | Slot::BandLedger
                | Slot::BandProvenance
                | Slot::BandDisclosure
                | Slot::FoldVoices
                | Slot::FoldQuorum
        )
    }

    /// The measure this slot resolves to.
    ///
    /// A `match` rather than a table, for the reason [`Role::token`] is one: the compiler makes the
    /// map **total**.
    #[must_use]
    pub const fn measure(self) -> Measure {
        match self {
            Slot::Discriminator => Measure::Ident,
            Slot::Judgement => Measure::Word,
            Slot::Lifecycle => Measure::Stage,
            Slot::Instant => Measure::Stamp,
            Slot::Reach => Measure::Path,
            Slot::Binding => Measure::Flag,
            Slot::Reversal => Measure::Status,
            Slot::Undoing => Measure::Undo,
            Slot::Successor => Measure::Successor,
            Slot::Hand => Measure::Hand,
            Slot::RowLead => Measure::Lead,
            Slot::CellGap => Measure::Gap,
            Slot::Enclosure => Measure::Frame,
            Slot::GroupRun => Measure::Run,
            Slot::ClipFloor => Measure::Floor,
            // 🔴 Three regions share one measure and the ledger does not, and that is the one place
            // on this axis where the map is genuinely many-to-one today. It is worth saying plainly
            // rather than dressing up: the ten column slots map to ten measures, so on the current
            // table `Slot -> Measure` is **injective for columns**. That is not the ladder failing —
            // it is the ladder **measuring** something true, which is that these widths were never
            // shared quantities. A scheme is where they are made to share (`Scheme::Compact` gives
            // `Stamp` and `Ident` one number at the two narrow grades), and that is a decision a
            // table can carry and a numeral typed at a column cannot.
            Slot::BandApparatus | Slot::BandProvenance | Slot::BandDisclosure => Measure::Band,
            Slot::BandLedger => Measure::Deck,
            Slot::FoldVoices => Measure::Voices,
            Slot::FoldQuorum => Measure::Quorum,
        }
    }
}

/// The named quantities. One entry per thing this face has an opinion about the size of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Measure {
    /// An opaque identifier, and the label over it.
    Ident,
    /// One word the engine spells.
    Word,
    /// A lifecycle, which may be two words.
    Stage,
    /// An RFC 3339 instant.
    Stamp,
    /// A scope path.
    Path,
    /// Yes or no.
    Flag,
    /// An inverse's status word.
    Status,
    /// Whether it was taken back.
    Undo,
    /// Another record's identifier.
    Successor,
    /// Who asked.
    Hand,
    /// The cells before a row.
    Lead,
    /// The cells between two cells.
    Gap,
    /// The cells an enclosure takes out of a row.
    Frame,
    /// How many rows share a group.
    Run,
    /// The fewest cells a cut may leave.
    Floor,
    /// The fewest rows a region says anything in.
    Band,
    /// The fewest rows the ledger says anything in.
    Deck,
    /// How many different values a column may carry and still be said once.
    Voices,
    /// How many records make saying it once worth a row.
    Quorum,
}

/// Every measure this face declares. Gate `g101` requires each one to be named by at least one slot
/// and to answer at every grade of every scheme: a quantity nothing resolves to is a quantity nobody
/// maintains, which is the sentence [`TOKENS`] carries one axis over.
pub const MEASURES: [Measure; 19] = [
    Measure::Ident,
    Measure::Word,
    Measure::Stage,
    Measure::Stamp,
    Measure::Path,
    Measure::Flag,
    Measure::Status,
    Measure::Undo,
    Measure::Successor,
    Measure::Hand,
    Measure::Lead,
    Measure::Gap,
    Measure::Frame,
    Measure::Run,
    Measure::Floor,
    Measure::Band,
    Measure::Deck,
    Measure::Voices,
    Measure::Quorum,
];

impl Measure {
    /// The spelled name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Measure::Ident => "ident",
            Measure::Word => "word",
            Measure::Stage => "stage",
            Measure::Stamp => "stamp",
            Measure::Path => "path",
            Measure::Flag => "flag",
            Measure::Status => "status",
            Measure::Undo => "undo",
            Measure::Successor => "successor",
            Measure::Hand => "hand",
            Measure::Lead => "lead",
            Measure::Gap => "gap",
            Measure::Frame => "frame",
            Measure::Run => "run",
            Measure::Floor => "floor",
            Measure::Band => "band",
            Measure::Deck => "deck",
            Measure::Voices => "voices",
            Measure::Quorum => "quorum",
        }
    }

    /// Whether this measure is one of the four the ladder holds constant along both axes, and the
    /// reason it is.
    ///
    /// 🔴 See this section's header: the row's **price** and the row's **drawing** are two readers
    /// of these four, and `[T-r66]` is the record of what happens when two readers of one magnitude
    /// disagree. Constant is what keeps them from being able to.
    #[must_use]
    pub const fn welded(self) -> bool {
        matches!(
            self,
            Measure::Lead | Measure::Gap | Measure::Frame | Measure::Floor
        )
    }
}

/// Which declaration table is in force.
///
/// 🔴 **Three, and the first is the face as it stood.** [`Scheme::Ledger`] answers, at every grade,
/// exactly the numerals `super::layout` used to carry — so the screen this face drew before the
/// placement ladder existed is reproducible **as a scheme** rather than as a git revision, and the
/// other two are the demonstration that the table is what the screen obeys.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scheme {
    /// What `super::layout` carried as numerals before `[T-r87]`: one width per column, the same at
    /// every grade, and a column that does not fit is dropped whole.
    Ledger,
    /// The same columns and the same order, sized to the grade. A narrow terminal gets narrower
    /// columns instead of fewer of them.
    Compact,
    /// Sized to the grade, ordered so that what tells one record from the next comes first, and —
    /// the part no width can buy — a column whose values repeat is **said once at the head with its
    /// counts** instead of on every row.
    Digest,
}

/// Every scheme this face declares.
pub const SCHEMES: [Scheme; 3] = [Scheme::Ledger, Scheme::Compact, Scheme::Digest];

/// The scheme a run uses when nothing says otherwise.
///
/// 🔴 [`Scheme::Ledger`], deliberately: a lane that lands a new default is a lane that has already
/// made the Owner's ruling for them. `req/924` §TUI-48's order is 見本 → Owner 裁定 → 量産, and
/// shipping the sample as the default would be the third step taken before the second.
pub const SCHEME_DEFAULT: Scheme = Scheme::Ledger;

/// The environment variable that names the placement scheme.
///
/// 🔴 An environment name and not an act. `super::acts::ACTS` is the declaration of what a **reader
/// can do**, and adding a key to it would be a change to the product's behaviour surface for the
/// sake of a photograph. This is the same road [`Tier::detect`] takes for the paint axis: the
/// reader's environment carries a preference and one function reads it.
pub const SCHEME_ENV: &str = "GX_TUI_PLACEMENT";

impl Scheme {
    /// The name used in a report and in [`SCHEME_ENV`].
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Scheme::Ledger => "ledger",
            Scheme::Compact => "compact",
            Scheme::Digest => "digest",
        }
    }

    /// Parse a scheme's name.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        SCHEMES.into_iter().find(|scheme| scheme.name() == text)
    }

    /// Read the environment. An unrecognised name is [`SCHEME_DEFAULT`] rather than an error: a
    /// misspelling must not take a reader's ledger away from them.
    #[must_use]
    pub fn detect() -> Self {
        std::env::var(SCHEME_ENV)
            .ok()
            .as_deref()
            .and_then(Scheme::parse)
            .unwrap_or(SCHEME_DEFAULT)
    }
}

/// One resolved placement: how many cells (or, for [`Slot::counts`], how many things), when the
/// slot is let go of, and where it stands among its siblings.
///
/// 🔴 The shape [`Ink`] has on the other axis: the members that vary along the axis and the members
/// that do not, in one value, so that a caller asks one question.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cells {
    /// Cells, or — for the slots [`Slot::counts`] names — a count of things.
    pub width: u16,
    /// When this slot is let go of.
    pub rank: Rank,
    /// Where it stands among its siblings, ascending. Ties keep declaration order.
    pub order: u16,
}

/// The value a measure takes at each grade, in [`Grade::ALL`]'s order.
///
/// 🔴 This is the placement axis's `match token { ... }` inside [`ink`], and it is the **only** place
/// in this face a placement magnitude is written down. Gate `g100` is what says so: a numeral in
/// `super::layout` that is not an arithmetic identity turns it red.
#[must_use]
pub const fn span(measure: Measure, scheme: Scheme) -> [u16; GRADE_COUNT] {
    // The four welded measures answer before the scheme is consulted, so a scheme cannot make the
    // row's price and the row's drawing disagree even by accident.
    match measure {
        Measure::Lead => return [1; GRADE_COUNT],
        Measure::Gap => return [2; GRADE_COUNT],
        Measure::Frame => return [4; GRADE_COUNT],
        Measure::Floor => return [16; GRADE_COUNT],
        _ => {}
    }
    match scheme {
        // 🔴 Every number below was read out of `super::layout` as it stood at
        // `pre-pivot_2026-09-02_tui_face_rebuild`. Constant along the grade axis, which is exactly
        // the fact that made the Owner's question answerable with *no*: the face had no notion that
        // a width is a capability.
        Scheme::Ledger => match measure {
            Measure::Ident => [14; GRADE_COUNT],
            Measure::Word => [9; GRADE_COUNT],
            Measure::Stage => [13; GRADE_COUNT],
            Measure::Stamp => [20; GRADE_COUNT],
            Measure::Path => [18; GRADE_COUNT],
            Measure::Flag => [8; GRADE_COUNT],
            Measure::Status => [14; GRADE_COUNT],
            Measure::Undo => [10; GRADE_COUNT],
            Measure::Successor => [16; GRADE_COUNT],
            Measure::Hand => [12; GRADE_COUNT],
            Measure::Run => [5; GRADE_COUNT],
            Measure::Band => [1; GRADE_COUNT],
            Measure::Deck => [4; GRADE_COUNT],
            // One voice is what `uniform` has always required: a column folds only when **every**
            // record agrees. Two records is what it has always required before it will answer at
            // all. So the fold's declaration reproduces the old behaviour exactly, and the two
            // schemes below are the only place it changes.
            Measure::Voices => [1; GRADE_COUNT],
            Measure::Quorum => [2; GRADE_COUNT],
            Measure::Lead | Measure::Gap | Measure::Frame | Measure::Floor => [0; GRADE_COUNT],
        },
        // 🔴 The labels are wire keys drawn unchanged (`req/942` §9, gate `P5`), so a column can
        // never be narrower than the key over it without the header being the thing that is cut.
        // Gate `g102` is what holds that rather than this sentence, and it is the gate that made
        // this table's first draft red: `ident` was cut to ten at the two narrow grades and
        // `transformation` is fourteen characters, so the header would have been clipped to say a
        // key the wire does not spell.
        Scheme::Compact => match measure {
            Measure::Ident => [14; GRADE_COUNT],
            Measure::Word => [7, 7, 8, 9, 9],
            Measure::Stage => [8, 9, 11, 13, 13],
            Measure::Stamp => [10, 10, 16, 20, 20],
            Measure::Path => [8, 9, 12, 18, 18],
            Measure::Flag => [8; GRADE_COUNT],
            Measure::Status => [14; GRADE_COUNT],
            Measure::Undo => [10; GRADE_COUNT],
            Measure::Successor => [13, 13, 14, 16, 16],
            Measure::Hand => [7, 7, 9, 12, 12],
            Measure::Run => [5; GRADE_COUNT],
            Measure::Band => [1; GRADE_COUNT],
            Measure::Deck => [3, 3, 4, 4, 4],
            Measure::Voices => [1; GRADE_COUNT],
            Measure::Quorum => [2; GRADE_COUNT],
            Measure::Lead | Measure::Gap | Measure::Frame | Measure::Floor => [0; GRADE_COUNT],
        },
        // 🔴 The scheme the Owner's finding is actually about. `[T-r87]`'s brief measured the grid
        // at thirty-two records of which **thirty are byte-identical** in every column but the
        // discriminator, and twenty-six screen rows went to drawing that repetition. Three voices
        // over a quorum of eight is the declaration that answers it: a column whose values, over the
        // records the read carried, come to three or fewer is said **once, with its counts**, and
        // the rows get the cells back.
        Scheme::Digest => match measure {
            Measure::Ident => [14; GRADE_COUNT],
            Measure::Word => [7, 7, 8, 9, 9],
            Measure::Stage => [8, 9, 11, 13, 13],
            Measure::Stamp => [10, 10, 16, 20, 20],
            Measure::Path => [8, 9, 12, 18, 18],
            Measure::Flag => [8; GRADE_COUNT],
            Measure::Status => [14; GRADE_COUNT],
            Measure::Undo => [10; GRADE_COUNT],
            Measure::Successor => [13, 13, 14, 16, 16],
            Measure::Hand => [7, 7, 9, 12, 12],
            Measure::Run => [5; GRADE_COUNT],
            Measure::Band => [1; GRADE_COUNT],
            Measure::Deck => [3, 3, 4, 4, 4],
            Measure::Voices => [3; GRADE_COUNT],
            Measure::Quorum => [8; GRADE_COUNT],
            Measure::Lead | Measure::Gap | Measure::Frame | Measure::Floor => [0; GRADE_COUNT],
        },
    }
}

/// When a slot is let go of and where it stands, in this scheme.
///
/// 🔴 Rank and order do not vary with the grade, and that is the same shape [`Ink`]'s `bold` / `dim`
/// / `reversed` have: they are properties of the **decision**, and the grade chooses the spelling of
/// the quantity alone. A rank that changed with the width would mean the order the screen lets go of
/// things in was itself a function of the screen, which is what `super::layout`'s header refuses.
#[must_use]
pub const fn standing(slot: Slot, scheme: Scheme) -> (Rank, u16) {
    match scheme {
        // Read out of `super::layout::LEDGER_COLUMNS` and `REGIONS` as they stood at
        // `pre-pivot_2026-09-02_tui_face_rebuild`: the array's own subscript was the order, which is
        // the third of the four things `[T-r87]` found typed rather than declared.
        Scheme::Ledger | Scheme::Compact => match slot {
            Slot::Discriminator => (Rank::One, 0),
            Slot::Judgement => (Rank::One, 1),
            Slot::Lifecycle => (Rank::One, 2),
            Slot::Instant => (Rank::Two, 3),
            Slot::Reach => (Rank::Two, 4),
            Slot::Binding => (Rank::Two, 5),
            Slot::Reversal => (Rank::Three, 6),
            Slot::Undoing => (Rank::Three, 7),
            Slot::Successor => (Rank::Four, 8),
            Slot::Hand => (Rank::Four, 9),
            Slot::BandApparatus => (Rank::Three, 0),
            Slot::BandLedger => (Rank::One, 1),
            Slot::BandProvenance => (Rank::One, 2),
            Slot::BandDisclosure => (Rank::One, 3),
            Slot::RowLead
            | Slot::CellGap
            | Slot::Enclosure
            | Slot::GroupRun
            | Slot::ClipFloor
            | Slot::FoldVoices
            | Slot::FoldQuorum => (Rank::One, 0),
        },
        // 🔴 What changes here is a **sentence about what the screen is for**: the columns that tell
        // one record from the next go first, and the ones the fold is about go last. Nothing is
        // deleted — a column that ranks last is still drawn at a width that has room for it, and one
        // that folds is said at the head with its counts. The reordering is the declaration doing
        // the job `[T-r87]` was opened because it could not do.
        Scheme::Digest => match slot {
            Slot::Discriminator => (Rank::One, 0),
            Slot::Instant => (Rank::One, 1),
            Slot::Reach => (Rank::Two, 2),
            Slot::Hand => (Rank::Two, 3),
            Slot::Judgement => (Rank::Three, 4),
            Slot::Lifecycle => (Rank::Three, 5),
            Slot::Reversal => (Rank::Three, 6),
            Slot::Binding => (Rank::Four, 7),
            Slot::Undoing => (Rank::Four, 8),
            Slot::Successor => (Rank::Four, 9),
            Slot::BandApparatus => (Rank::Three, 0),
            Slot::BandLedger => (Rank::One, 1),
            Slot::BandProvenance => (Rank::One, 2),
            Slot::BandDisclosure => (Rank::One, 3),
            Slot::RowLead
            | Slot::CellGap
            | Slot::Enclosure
            | Slot::GroupRun
            | Slot::ClipFloor
            | Slot::FoldVoices
            | Slot::FoldQuorum => (Rank::One, 0),
        },
    }
}

/// The value a slot takes at this grade, in this scheme — the bottom of the placement ladder.
///
/// 🔴 The placement axis's [`ink`], and it takes its arguments in the same order for the same
/// reason: what a thing is, then the capability of the medium it is being drawn on.
#[must_use]
pub const fn cells(slot: Slot, grade: Grade, scheme: Scheme) -> Cells {
    let (rank, order) = standing(slot, scheme);
    Cells {
        width: span(slot.measure(), scheme)[grade.index()],
        rank,
        order,
    }
}

/// The fold budget the strict rule means: one voice, and a quorum of two.
///
/// 🔴 Declared rather than typed at `super::layout::resolve_shared` (`[T-r87]`, and gate `g100`
/// found it in this lane's own first cut — the gate refusing its author's line on its first run is
/// the only kind of evidence that it can refuse anything). *Every record agrees* and *fewer than two
/// records prove nothing about repetition* are the two numbers `uniform` has always meant, and they
/// are read out of the shipped default's own table so that the strict rule and the default screen
/// cannot come apart.
#[must_use]
pub fn strict_fold() -> (usize, usize) {
    (
        cells(Slot::FoldVoices, Grade::Full, SCHEME_DEFAULT).width as usize,
        cells(Slot::FoldQuorum, Grade::Full, SCHEME_DEFAULT).width as usize,
    )
}
