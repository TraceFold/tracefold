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
}

/// Every paint role this face declares. Gate g14 requires each one to resolve to a value, and gate
/// g16 pins which value.
pub const ROLES: [Role; 15] = [
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
pub const GLYPHS: [Glyph; 8] = [
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
