// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R-930-B1** — the `DesignToken` object (`.gx` kind 15): the character kernel of a design
//! board, and the four-layer declarations that name parts of a face, given an identity they can be
//! addressed by.
//!
//! Spec: `req/939_KIND_BATCH1_C1_DESIGNTOKEN_2026-08-30.md` §3, which rules on an internal
//! contradiction in `req/930` §6-13-1 (below). `req/930` is the design; this module is the ruling
//! plus the code.
//!
//! # Why a kind at all, and why it is not a GUI feature
//!
//! The board compiler already knows that what it emits has no identity. Its own header says so:
//! "the digest of that text is a JCS-route digest and DOES NOT CONSTITUTE IDENTITY ... Identity is
//! BLAKE3 over canonical DAG-CBOR". A board today therefore has a digest and no name. This kind is
//! the missing piece, and it is a piece of the object format rather than of the interface: nothing
//! here draws anything.
//!
//! # 🔴 The body is the kernel, and `req/930` §6-13-1's field list is not the kernel
//!
//! The compiler splits a board into two islands and takes two digests, so that a redesign is
//! *provably* not a rewording: the shell digest must move and the text digest must not. Its
//! `split()` puts the characters in one island and `axes`, `projections`, `edges`, and every
//! node's `size`, `shape`, `state`, `glyph`, `coord` and `views` in the other. `req/930` §6-13-1
//! names the second island's members while its own 🔴 note requires the first — a contradiction
//! `req/930` §11-8 predicts, since that file read the compiler's header and not its `split()`.
//! `req/939` §3-2 rules for the note: **the body is the characters**. Putting the shell inside the
//! identity would move a board's name every time it is re-drawn, which would delete at the identity
//! layer the one property the compiler proves at the digest layer.
//!
//! # 🔴 The kind is named inside the bytes the identity covers (R-939-1)
//!
//! `.gx` identity is `BLAKE3(enc(body))` and the header's `kind` is outside it, so a body alone
//! does not say which kind it is (`req/930` §4 Q3, defect C-1). `Receipt` is safe by accident:
//! its payload type sits inside the DSSE pre-authentication encoding, so its signature witnesses
//! its kind — [`crate::dsse`] gives that same reason for each of its five payload types ("it is
//! inside the signed bytes, so a signature over one cannot be replayed as a signature over
//! another").
//!
//! A kind with no signer has no such bytes, so this one carries [`DesignToken::gx_kind`] **inside
//! its canonical body**. Two kinds then cannot share a preimage, which is the property
//! `gx_log::tile`'s `0x00`/`0x01` prefixes buy for leaves and nodes — obtained here with a member
//! instead of a prefix byte, because the prefixed road in gx-canon is closed at exactly the two
//! domains 42 §3.11 defines and opening it is a spec change. Constant within a kind and different
//! between kinds is what a domain tag *is*; that is why it is not the constant-valued field
//! DR-46-26 rejects.
//!
//! # 🔴 What an object of this kind does not say
//!
//! [`Unsaid`] is exhaustive and each arm has a different future. The first two matter most: this
//! file says nothing about what a board **looks like** (the shell is not in it) and nothing about
//! whether it is **good** (no mechanism decides that, and a green check here is not a design
//! verdict). A verifier that let either be read out of an identity check would be lying with a
//! true statement.

use core::fmt;

use gx_core::Cid;
use serde::{Deserialize, Serialize};

/// The in-band kind witness (R-939-1, module header): the value [`DesignToken::gx_kind`] carries
/// in every well-formed document of this kind.
///
/// Versioned in the string rather than in a separate member, so that a second generation of this
/// body is a different tag and therefore a different preimage, without a schema member whose only
/// job is to be compared.
pub const DESIGN_TOKEN_TAG: &str = "gx.design-token.v1";

/// Which of the four layers a declaration is allowed to name.
///
/// 🔴 `token` and `value` are **absent by construction**. The four-layer rule is that a component
/// may name a role or an intent and never a token name or a raw value; an enum with four arms would
/// make the two forbidden ones spellable and leave a test to say they must not be spelled. This is
/// `req/929`'s INV-A2 discipline: the shape says it, so no later hand can spell it without deleting
/// an arm and breaking every match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    /// Why the reader is being told something. The layer that can be checked against the product's
    /// own vocabulary, which is what connects a design system to an engine rather than placing it
    /// alongside one.
    Intent,
    /// Which part of a face this is (`surface.raised`, `edge.focus`, `layout.rail`).
    Role,
}

/// One string of a board's text island, with the address it sits at.
///
/// `at` is the island-internal address the compiler's `split()` implies (`<node>.body.rows.3.2`),
/// not a coordinate on a canvas: it says where in the document the characters live, and it stays
/// the same when the drawing changes. Members are declared in canonical map-key order (length
/// first, then bytewise), which is the order the canonical encoder requires.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct KernelString {
    /// Where in the text island these characters are.
    pub at: String,
    /// The characters themselves.
    pub text: String,
}

/// The character kernel of one board.
///
/// Flattened to `(address, characters)` pairs on purpose: the island is a nested bag of strings,
/// and a second nested schema on this side would mean the island has two shapes that have to be
/// kept in step. A pair list survives a change to the island's nesting without a change here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    /// The kernel, strictly ascending by [`KernelString::at`] — see [`DesignToken::check`] for why
    /// the order is a refusal and not a convenience.
    pub strings: Vec<KernelString>,
    /// The name this board is addressed by.
    pub board_id: String,
}

/// One declaration of the design vocabulary: a name, the layer it belongs to, what it means, and
/// which declarations contain it.
///
/// 🔴 `parents` is a DAG and not a tree, and **how two parents compose is not defined** anywhere in
/// this workspace (`req/930` R-930-10). Branching is free; joining needs structure that does not
/// exist yet. So the member is carried and no merge is computed from it — an undefined join
/// rendered as though it were defined is the same defect as an unknown drawn as a false.
///
/// The spelling `parents` is deliberately the same word `Transformation.parents` uses and the
/// meaning is deliberately different: there it is the causal history of a change, here it is
/// containment of vocabulary. Same name, different object; saying so is cheaper than a rename that
/// would make one of the two read worse.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declaration {
    /// The name being declared (`raised`, `rail`, `focus`).
    pub name: String,
    /// Which layer it belongs to. Never a token name and never a value.
    pub layer: Layer,
    /// What it means, in the words a reader of the product would use.
    pub meaning: String,
    /// The declarations that contain this one, by identity. Strictly ascending, no repeats.
    pub parents: Vec<Cid>,
    /// The hierarchical namespace (`surface`, `layout.rail`, `component.button`).
    ///
    /// Free text and not an enumeration, which is what makes the vocabulary additive: a family of
    /// names that does not exist yet needs no change here to be expressible.
    pub namespace: String,
}

/// The two shapes a design-token document takes: a whole board, or one declaration.
///
/// One kind with two variants rather than two kinds. `req/930` §4 Q6 records what happens when a
/// registry's numbers are fixed before anything they name has been written, and the answer is not
/// to do it twice: an internal variant can be joined by a third without touching a number a
/// stranger's file carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Document {
    /// The character kernel of a board.
    Board(Board),
    /// A single four-layer declaration.
    Declaration(Declaration),
}

/// A `.gx` kind-15 body: the in-band kind witness, and the document.
///
/// Members in canonical map-key order (`gx_kind` is seven bytes, `document` is eight).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignToken {
    /// R-939-1's witness. Always [`DESIGN_TOKEN_TAG`] in a document this build wrote; carried
    /// rather than assumed on the way in, because the check belongs to the layer that holds the
    /// header (see `gxfile`'s `GxKind::body_witness`), and a value compared in two places is a
    /// predicate that can come to disagree with itself.
    pub gx_kind: String,
    /// What this object is.
    pub document: Document,
}

/// 🔴 What an object of this kind does not say. Four arms, four different futures.
///
/// Modelled on [`crate::attach::NotAttested`], and for its reason: an absence folded into one word
/// makes a gap a later lane can close look like one nothing can.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unsaid {
    /// What the board looks like. The shell — size, shape, state, glyph, coordinate, axes,
    /// projections, edges, theme, effects — is not in the body, so the identity covers none of it.
    /// **Structural**: it is not missing data, it is data this object is defined not to hold.
    Appearance,
    /// Whether the design is any good: whether it reads at a glance, whether it is not slop.
    /// **Permanent**: no mechanism here decides it, a passing check is not a design verdict, and
    /// this workspace has twice measured that a green gate can sit on a visual defect.
    VisualQuality,
    /// Whether any implementation obeys these declarations. **Releasable**: it needs a gate that
    /// reads components against this vocabulary, and that gate is not this object.
    Conformance,
    /// What a declaration with two parents composes to. **Undefined in core**: branching is free
    /// and joining is not defined, so nothing here resolves one.
    Join,
}

impl Unsaid {
    /// Every arm, so a caller can print the whole disclosure without keeping a list of its own.
    pub const ALL: [Unsaid; 4] = [
        Unsaid::Appearance,
        Unsaid::VisualQuality,
        Unsaid::Conformance,
        Unsaid::Join,
    ];

    /// The reason, spelled out. An exhaustive match: a fifth silence stops the build.
    #[must_use]
    pub const fn because(self) -> &'static str {
        match self {
            Unsaid::Appearance => {
                "the shell -- size, shape, state, glyph, coordinate, axes, projections, edges, \
                 theme, effects -- is not part of this body, so a re-drawn board keeps this name \
                 and this name says nothing about the drawing"
            }
            Unsaid::VisualQuality => {
                "whether the design reads well is not decided by any mechanism here; a verified \
                 identity is not a design verdict, and reading one out of the other would be a \
                 false claim made out of a true one"
            }
            Unsaid::Conformance => {
                "whether an implementation obeys these declarations needs a gate that reads \
                 components against this vocabulary, and this object is the vocabulary rather \
                 than that gate"
            }
            Unsaid::Join => {
                "a declaration may name two parents and how two parents compose is defined \
                 nowhere in this workspace, so no merge is computed and none is shown"
            }
        }
    }
}

/// Why a design-token document was not admitted.
///
/// A local vocabulary, as `attach`'s is: these are refusals about the shape of one body, and
/// folding them into the crate's `Error` would move tables that belong to the HTTP surface.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// The kernel is not strictly ascending by address, or an address repeats.
    ///
    /// A refusal rather than a sort, and the reason is the one the canonical encoder gives for map
    /// keys: an order the writer chose is an order the reader has to be told, and two orderings of
    /// one board would be two preimages of one fact. `gx-canon` refuses unsorted map keys; the one
    /// place this schema uses a list instead of a map is held to the same rule.
    UnorderedKernel {
        /// The address that broke the order.
        at: String,
        /// The address before it.
        after: String,
    },
    /// A member that names nothing: an empty address, board id, declaration name, namespace, or
    /// meaning. Refused rather than carried, because a declaration with no meaning is a name
    /// wearing a declaration's clothes.
    Empty {
        /// Which member.
        member: &'static str,
    },
    /// `parents` is not strictly ascending, or a parent repeats. Same reason as
    /// [`Refusal::UnorderedKernel`]: containment is a set, and a set with two spellings has two
    /// identities.
    UnorderedParents {
        /// The position of the parent that broke the order.
        index: usize,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::UnorderedKernel { at, after } => write!(
                f,
                "the kernel is not strictly ascending by address: {at:?} follows {after:?}, and \
                 two orderings of one board would be two identities of one fact"
            ),
            Refusal::Empty { member } => {
                write!(
                    f,
                    "{member} is empty, and a member that names nothing is refused"
                )
            }
            Refusal::UnorderedParents { index } => write!(
                f,
                "parents[{index}] does not follow its predecessor; containment is a set and a set \
                 with two spellings would have two identities"
            ),
        }
    }
}

impl std::error::Error for Refusal {}

impl DesignToken {
    /// A board document, checked.
    ///
    /// # Errors
    /// Whatever [`DesignToken::check`] refuses.
    pub fn board(board_id: impl Into<String>, strings: Vec<KernelString>) -> Result<Self, Refusal> {
        let token = DesignToken {
            gx_kind: DESIGN_TOKEN_TAG.to_string(),
            document: Document::Board(Board {
                strings,
                board_id: board_id.into(),
            }),
        };
        token.check()?;
        Ok(token)
    }

    /// A single declaration, checked.
    ///
    /// # Errors
    /// Whatever [`DesignToken::check`] refuses.
    pub fn declaration(declaration: Declaration) -> Result<Self, Refusal> {
        let token = DesignToken {
            gx_kind: DESIGN_TOKEN_TAG.to_string(),
            document: Document::Declaration(declaration),
        };
        token.check()?;
        Ok(token)
    }

    /// 🔴 The one predicate, run on the way in **and** on the way out.
    ///
    /// A writer that could produce a document its own reader refuses would put files into the world
    /// that this build cannot read back, so `gxfile`'s writer runs this before it writes and its
    /// reader runs it after it decodes. One question, one function: the sibling-sweep rule
    /// (`req/38` §227) exists because the same question spelled in two places is the same question
    /// answered two ways.
    ///
    /// It does **not** check [`DesignToken::gx_kind`]. That comparison is against the header's
    /// kind, which this module cannot see, so it belongs to `gxfile` and is done there for every
    /// kind at once rather than here for this one.
    ///
    /// # Errors
    /// One [`Refusal`] per condition.
    pub fn check(&self) -> Result<(), Refusal> {
        match &self.document {
            Document::Board(board) => {
                if board.board_id.is_empty() {
                    return Err(Refusal::Empty { member: "board_id" });
                }
                let mut previous: Option<&str> = None;
                for entry in &board.strings {
                    if entry.at.is_empty() {
                        return Err(Refusal::Empty { member: "at" });
                    }
                    if let Some(before) = previous {
                        if entry.at.as_str() <= before {
                            return Err(Refusal::UnorderedKernel {
                                at: entry.at.clone(),
                                after: before.to_string(),
                            });
                        }
                    }
                    previous = Some(entry.at.as_str());
                }
                Ok(())
            }
            Document::Declaration(declaration) => {
                for (member, value) in [
                    ("name", &declaration.name),
                    ("meaning", &declaration.meaning),
                    ("namespace", &declaration.namespace),
                ] {
                    if value.is_empty() {
                        return Err(Refusal::Empty { member });
                    }
                }
                for index in 1..declaration.parents.len() {
                    if declaration.parents[index].0 <= declaration.parents[index - 1].0 {
                        return Err(Refusal::UnorderedParents { index });
                    }
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(at: &str, text: &str) -> KernelString {
        KernelString {
            at: at.to_string(),
            text: text.to_string(),
        }
    }

    /// The order is a refusal, not a preference.
    #[test]
    fn a_kernel_out_of_order_is_refused() {
        let out_of_order = DesignToken::board("b", vec![line("n2.one", "b"), line("n1.one", "a")]);
        assert!(matches!(out_of_order, Err(Refusal::UnorderedKernel { .. })));
        let repeated = DesignToken::board("b", vec![line("n1.one", "a"), line("n1.one", "a")]);
        assert!(matches!(repeated, Err(Refusal::UnorderedKernel { .. })));
        assert!(DesignToken::board("b", vec![line("n1.one", "a"), line("n2.one", "b")]).is_ok());
    }

    /// A member that names nothing is refused wherever it appears.
    #[test]
    fn empty_members_are_refused_by_name() {
        assert!(matches!(
            DesignToken::board("", vec![]),
            Err(Refusal::Empty { member: "board_id" })
        ));
        assert!(matches!(
            DesignToken::board("b", vec![line("", "a")]),
            Err(Refusal::Empty { member: "at" })
        ));
        let nameless = Declaration {
            name: String::new(),
            layer: Layer::Role,
            meaning: "m".to_string(),
            parents: vec![],
            namespace: "surface".to_string(),
        };
        assert!(matches!(
            DesignToken::declaration(nameless),
            Err(Refusal::Empty { member: "name" })
        ));
    }

    /// Every silence has its own sentence, and no two share one.
    #[test]
    fn the_disclosure_is_total_and_its_reasons_are_distinct() {
        let mut seen: Vec<&str> = Unsaid::ALL.iter().map(|u| u.because()).collect();
        assert_eq!(seen.len(), 4);
        assert!(seen.iter().all(|reason| !reason.is_empty()));
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 4, "two silences share a sentence");
    }
}
