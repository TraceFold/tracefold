// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The paint ladder: the second axis of `req/942` §11's four layers, one rung per module boundary.
//!
//! ```text
//! intent  ->  role  ->  token  ->  value
//! ```
//!
//! `super::layout` holds the ladder for **placement**. This module holds the one for **paint**, and
//! the two are the same shape on purpose: a caller names a [`Role`] — what a thing is, in the
//! reader's words — and never a hue. A role resolves to a [`Token`], a token to an [`Ink`], and the
//! ink is turned into the medium's own type in exactly one place (`super::renderer`), which is the
//! seam gate g5 already measures.
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
