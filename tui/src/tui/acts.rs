// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! What this face can **do**, declared the way a colour is declared.
//!
//! ```text
//! intent  ->  act  ->  effect  ->  key
//! ```
//!
//! The same four layers as `super::layout` and `super::tokens`, one axis over. An act carries an
//! intent (the sentence a reader would say) and an [`Effect`] (what it does to the state, in the
//! state's own words). The **keys are the value layer** — the one rung a different medium
//! substitutes. A window would bind [`Act::Open`] to a double click and this terminal binds it to
//! Return, and neither of them decides what opening *means*.
//!
//! # Why the acts are data and a `match` on a key code is not enough
//!
//! A `match` in the drawing loop is wiring no gate can read: it can be half-done, and half-done key
//! handling is the exact defect where a list moves with `j` and refuses to move with the down arrow
//! — which a reader reads as "this program is broken" rather than as "that key was never bound".
//! The build this module replaces had three keys (`q`, `Esc`, `r`) and no state to move at all,
//! while its own documentation described a face that could be moved through. Here the declaration
//! is the source, [`apply`] is the single reducer that resolves it, and gate g12 fires **every**
//! declared act through that reducer and requires each one to move something.
//!
//! # The state is small on purpose
//!
//! [`View`] is what the reader has done, and nothing else: which record is attended to, and whether
//! it is opened. Nothing measured, nothing fetched — those live in `super::wire`, which is what
//! keeps a keypress from being able to invent a fact.

/// One thing a reader can do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    /// Attend to the record above this one.
    Prev,
    /// Attend to the record below this one.
    Next,
    /// Attend to the first record.
    First,
    /// Attend to the last record.
    Last,
    /// See everything the attended record carries, including what the grid has no column for.
    Open,
    /// Stop seeing one record and see the list again.
    Close,
    /// Ask the engine again, now.
    Read,
    /// Stop reading and give the terminal back.
    Leave,
}

/// Every act this face declares. Gate g12 requires each one to move the state.
pub const ACTS: [Act; 8] = [
    Act::Prev,
    Act::Next,
    Act::First,
    Act::Last,
    Act::Open,
    Act::Close,
    Act::Read,
    Act::Leave,
];

/// What an act does to the state, in the state's own words.
///
/// 🔴 The whole vocabulary is here. A reducer that meets a sixth verb does not exist, because there
/// is no way to write one: the compiler's exhaustiveness check over this enum is the gate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    /// Which record is attended to.
    Select(Select),
    /// The attended record is opened in place.
    Open,
    /// It is closed again.
    Close,
    /// Ask the engine now rather than at the next keypress.
    Read,
    /// Give the terminal back.
    Leave,
}

/// Where the attention goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Select {
    /// One up.
    Up,
    /// One down.
    Down,
    /// The top of the list.
    First,
    /// The bottom of it.
    Last,
}

impl Act {
    /// The spelled name, for a gate and for the report.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Act::Prev => "act.prev",
            Act::Next => "act.next",
            Act::First => "act.first",
            Act::Last => "act.last",
            Act::Open => "act.open",
            Act::Close => "act.close",
            Act::Read => "act.read",
            Act::Leave => "act.leave",
        }
    }

    /// The sentence a reader would say, which is the layer above the effect.
    #[must_use]
    pub fn intent(self) -> &'static str {
        match self {
            Act::Prev => "attend to the record above this one",
            Act::Next => "attend to the record below this one",
            Act::First => "attend to the first record",
            Act::Last => "attend to the last record",
            Act::Open => "see everything this record carries",
            Act::Close => "stop seeing one record and see the list again",
            Act::Read => "ask the engine again, now",
            Act::Leave => "stop reading and give the terminal back",
        }
    }

    /// What it does.
    #[must_use]
    pub fn effect(self) -> Effect {
        match self {
            Act::Prev => Effect::Select(Select::Up),
            Act::Next => Effect::Select(Select::Down),
            Act::First => Effect::Select(Select::First),
            Act::Last => Effect::Select(Select::Last),
            Act::Open => Effect::Open,
            Act::Close => Effect::Close,
            Act::Read => Effect::Read,
            Act::Leave => Effect::Leave,
        }
    }

    /// The keys that produce it, by **name**. The bytes an arrow key sends are the renderer's
    /// problem; a name is what a declaration can carry across two media.
    ///
    /// The first of them is the one the help text spells.
    #[must_use]
    pub fn keys(self) -> &'static [&'static str] {
        match self {
            Act::Prev => &["k", "up"],
            Act::Next => &["j", "down"],
            Act::First => &["g", "home"],
            Act::Last => &["G", "end"],
            Act::Open => &["return", "l", "right"],
            Act::Close => &["escape", "h", "left"],
            Act::Read => &["r"],
            Act::Leave => &["q"],
        }
    }
}

/// Which act a key name produces, or none.
///
/// 🔴 One road from a key to an act. A second `match` somewhere in the drawing loop would be a
/// second binding table, and two binding tables disagree the day one of them is edited.
#[must_use]
pub fn for_key(key: &str) -> Option<Act> {
    ACTS.into_iter().find(|act| act.keys().contains(&key))
}

/// What the reader has done. Not what was measured — that is `super::wire`'s.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct View {
    /// Which record is attended to, as an index into the records the read carried.
    ///
    /// 🔴 **It said "the rows the subject region drew", and that was false** (`req/38` SS996). What
    /// [`apply`] clamps against is the number of records the list *holds*, which is what a reducer
    /// with no screen in front of it can know; the subject region draws as many of them as its rows
    /// allow and starts at the first one, so on a list that is being cut the attention can be moved
    /// on to a record that is not drawn and the mark then appears nowhere. Measured at 100x5 against
    /// the two-record fixture: nought records drawn, and the note reporting `record 2 of 2`
    /// (`req/942_artifacts/tui_r4_2026-08-31/`). Closing it needs the drawn row count to reach this
    /// reducer — the same missing edge as `req/964` §16's third row — or a window that scrolls,
    /// which is a concept this face does not have. **Declared, not repaired**; the sentence above is
    /// corrected rather than the code, because the code is what a reducer is allowed to know.
    pub selected: usize,
    /// Whether the attended record is opened in place.
    pub open: bool,
}

/// What the caller has to do about an act, once the state has been moved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Signal {
    /// Nothing; the new view is the whole of the answer.
    None,
    /// Read the four routes again.
    Read,
    /// Give the terminal back.
    Leave,
}

/// The single reducer: the declaration resolved, once.
///
/// Pure — the new view is returned rather than written, so a gate can fire every act through it
/// without a terminal, a socket or a clock. `rows` is how many records the list holds, which is the
/// only thing outside the view that a selection depends on.
///
/// 🔴 An empty list is not a special case with its own branch: `rows == 0` clamps every selection to
/// `0`, which is where it already is, so the acts that move the attention are correctly inert on a
/// screen with nothing to attend to.
///
/// 🔴 **That sentence was true of the attention and false of the flag beside it** (`req/38` SS974).
/// `selected` was clamped and `open` was not, so an act could leave a record opened on a list with
/// no records in it; the disagreement with the screen was written down in
/// `super::renderer::offered` instead of being repaired. Both members are now clamped by the same
/// two lines, so the sentence is true of the whole state rather than of most of it, and the
/// paragraph in `offered` records that the disagreement is closed rather than being deleted.
#[must_use]
pub fn apply(view: &View, act: Act, rows: usize) -> (View, Signal) {
    let last = rows.saturating_sub(1);
    let mut next = *view;
    let signal = match act.effect() {
        Effect::Select(select) => {
            next.selected = match select {
                Select::Up => view.selected.saturating_sub(1),
                Select::Down => (view.selected + 1).min(last),
                Select::First => 0,
                Select::Last => last,
            };
            Signal::None
        }
        Effect::Open => {
            next.open = true;
            Signal::None
        }
        Effect::Close => {
            next.open = false;
            Signal::None
        }
        Effect::Read => Signal::Read,
        Effect::Leave => Signal::Leave,
    };
    // The list can shrink between reads; an index into rows that are gone would draw an attention
    // mark on a record nobody is looking at.
    next.selected = next.selected.min(last);
    // 🔴 **`req/38` SS974, design round 2's third finding — and the repair is one line further than
    // the finding was.**
    //
    // The report said `act.open` moved the view on an empty list while `super::renderer::subject`
    // declined to open anything, so the declaration and the screen described two different
    // programs, and `super::renderer::offered` carried a paragraph explaining the disagreement
    // instead of anybody closing it. The obvious repair is a row count inside the `Open` arm.
    //
    // That repair is wrong, and gate g21 said so on the first run: it leaves `act.prev` carrying an
    // opened flag on a list that has emptied, because only the arm that was patched asks the
    // question. **The question is not "may this act open something", it is "does what the view
    // points at still exist"** — and the line above has been asking exactly that about `selected`
    // since the first build. So it is asked here, once, about both members. Every act inherits it,
    // including the ones nobody has written yet.
    next.open &= rows > 0;
    (next, signal)
}
