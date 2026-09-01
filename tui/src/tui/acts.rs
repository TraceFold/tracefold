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
    /// See what this face can do, in its own declared words.
    ///
    /// 🔴 A **canon amendment**, not a runtime table. The ruling that fixed this set at eight
    /// (`req/984` §8-7) fixed it against *runtime and theme*, not against a dated amendment to the
    /// source — the same distinction `Nothing` was grown 6 -> 7 under (`req/38` SS974).
    ///
    /// Before this, the only road to the help text was to leave the process and type
    /// `gx tui --help`: a face that declares its capabilities and offers no way to read the
    /// declaration from inside itself.
    Help,
    /// Spell the disclosure in full, here, without restarting.
    ///
    /// 🔴 The flag `--wide` already existed and could only be answered by **relaunching the
    /// process** (`req/984` §8-14). A face whose remedy for "I cannot read what was let go of" is
    /// "start again" has put the cost of its own disclosure on the reader.
    Wide,
    /// Stop reading and give the terminal back.
    Leave,
}

/// Every act this face declares. Gate g12 requires each one to move the state.
///
/// 🔴 **The count is read from this array everywhere it is used**, which is the rule the sister
/// vocabulary `super::wire::Nothing::ALL` already states of itself. A test that spelled the number
/// as a literal was asserting the array's *length* rather than its *shape*, and the length is the
/// one property no reader depends on (`req/984` §8-17).
pub const ACTS: [Act; 10] = [
    Act::Prev,
    Act::Next,
    Act::First,
    Act::Last,
    Act::Open,
    Act::Close,
    Act::Read,
    Act::Help,
    Act::Wide,
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
    /// Show, or stop showing, what this face can do.
    Help,
    /// Spell the disclosure in full, or stop.
    Wide,
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
            Act::Help => "act.help",
            Act::Wide => "act.wide",
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
            Act::Help => "see what this face can do, and what each key is for",
            Act::Wide => "spell what was let go of in full, without restarting",
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
            Act::Help => Effect::Help,
            Act::Wide => Effect::Wide,
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
            // A bare `?` and not a modified key: `super::renderer::key_name` carries a
            // `KeyCode` and no modifiers by design, so binding `Ctrl-O` -- the way
            // `gemini-cli` spells this -- would be a change to the medium layer rather than
            // to this declaration. The reference face's pixel is worth taking; its key is not.
            Act::Help => &["?"],
            // The first letter of the flag this act makes answerable in place.
            Act::Wide => &["w"],
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
    /// 🔴 **It said "the rows the subject region drew", and that was false** (`req/38` SS999). What
    /// [`apply`] clamps against is the number of records the list *holds*, which is what a reducer
    /// with no screen in front of it can know; the subject region draws as many of them as its rows
    /// allow and starts at the first one, so on a list that is being cut the attention can be moved
    /// on to a record that is not drawn and the mark then appears nowhere. Measured at 100x5 against
    /// the two-record fixture: nought records drawn, and the note reporting `record 2 of 2`
    /// (`req/942_artifacts/tui_r4_2026-08-31/`). Closing it needs the drawn row count to reach this
    /// reducer — the same missing edge as `req/964` §16's third row — or a window that scrolls,
    /// which is a concept this face does not have. **Declared, not repaired**; the sentence above is
    /// corrected rather than the code, because the code is what a reducer is allowed to know.
    ///
    /// 🔴 **Repaired, and not here** (`req/38` SS999 T-r4-B). The paragraph above is still true of
    /// this member — a reducer clamps against the records the list holds, and that is all it may
    /// know — and it was wrong that the way out was a scrolling window this face does not have.
    /// `super::layout::window` is a **function of this state**, computed where the row count is
    /// already known, so nothing is remembered and no act learned about it. Gate g28 is the
    /// invariant: whenever the region draws a record at all, it draws this one.
    pub selected: usize,
    /// Whether the attended record is opened in place.
    pub open: bool,
    /// Whether the reader has asked what this face can do.
    ///
    /// 🔴 **Here, and not in the caller** (`req/984` §9-7). Opening the help face is something the
    /// reader *did*, and this struct is defined as what the reader has done; a copy of it held in
    /// `super::renderer::interactive` would put half of that answer outside the type whose whole
    /// job is to be it. It would also put the help face out of reach of
    /// `super::renderer::render_view_to_buffer`, which every gate and every capture draws through —
    /// a screen no instrument can photograph.
    pub help: bool,
    /// Whether the reader has asked for the disclosure spelled in full.
    ///
    /// 🔴 Held here rather than as a [`Signal`], for the reason `Signal` exists: a signal is one
    /// instruction to the caller about *now*, and this is a state that persists across frames,
    /// reads and keypresses.
    pub wide: bool,
    /// How far the reader has moved the **face**, as against the attention.
    ///
    /// 🔴 **`req/924` §TUI-62 裁定3** (`req/38` SS1093, Owner `#284-T`, 2026-09-01): *scroll down and
    /// the content moves up — and "relative scroll" includes the face moving independently of the
    /// selection*. Until this the window was a pure function of `selected`, so the only way to move
    /// the screen was to move the attention, and a wheel had nothing to turn.
    ///
    /// A **signed offset added to the window's own answer**, not an absolute top. That is what keeps
    /// the reducer allowed to hold it: a reducer knows how many records the list has and nothing
    /// about how tall the terminal is, so it cannot compute a top — but it can say *this many rows
    /// further down than wherever the face would otherwise be standing*, and
    /// `super::layout::scrolled` clamps it against a height only it knows.
    ///
    /// 🔴 **The consequence is deliberate and is disclosed**: with this away from nought the attended
    /// record can be off the screen. The standing row spells `N of M` at every shape (`req/924`
    /// §TUI-57), so the reader is told where they are standing even when they cannot see it — which
    /// is the property `req/38` SS999 T-r4-B was actually about. Gate `g28` measures the window's
    /// invariant with this at nought and says so.
    pub glide: isize,
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

/// Does what the view points at still exist? Asked once, here, by everything that asks it.
///
/// 🔴 The two lines this is made of used to sit at the bottom of [`apply`] and **nowhere else**, so
/// the question was asked of a *key* and never of a *reading*. A key is not the only thing that can
/// make the answer no: the list can come back shorter. `super::renderer::interactive` re-reads on
/// `Subscription::due` and redraws with no act applied at all, and on that road the attention was
/// left pointing past the end of a list nobody had pressed a key against — records on the screen and
/// the mark on none of them, which is the shape `req/38` SS999 T-r4-B is named for, reached by a
/// road that repair did not cover. Found by the r6 verification lane's probe V2 and out of its
/// scope (`req/942_artifacts/tui_r6_verify_2026-08-31/00_VERIFY.md`).
///
/// 🔴 It is a function rather than two lines copied into the draw road, and that is the whole
/// point: the same question spelled in two places is two answers the day one of them is edited,
/// which is the defect this face has now walked into three times (`super::layout::window` reading
/// `renderer::note_rows` rather than restating the budget is the same ruling, one axis over). Gate
/// g31 is what measures that the draw road asks it.
/// 🔴 **`help` and `wide` are clamped by the same line, and that is a ruling rather than a
/// convenience** (`req/984` §9-7, 2026-08-31).
///
/// `req/988` §3-3-③ asked for the opposite: that the help face open on a list with nothing in it,
/// on the argument that an empty screen is exactly the one a reader cannot work out by trying keys
/// on. That argument was **retracted**, and the reason is gate g21. g21 is written over the whole
/// of [`ACTS`] rather than over [`Act::Open`], expressly "so an act added later is measured by it
/// without anybody remembering to add a line" — and it holds two things at once: that an empty list
/// is a **fixed point** of this reducer, and that every act the note *offers* at a given row count
/// **moves something** at that row count. An act that is inert on an empty list is therefore
/// allowed; an act that is inert and *advertised* is not.
///
/// So the shape of the repair is: clamp both members here, and leave both acts out of
/// `super::renderer::NOTE_ORDER_EMPTY`. Narrowing g21 to spare these two would have bought one
/// screen and given up the invariant that catches the next act somebody adds — the defect
/// `NOTE_ORDER_ONE` was created for (`req/38` SS974), one row count up.
///
/// **The cost is real and is not hidden**: on a reading with no records in it, `?` and `w` do
/// nothing, and the note does not name them. `gx tui --help` is still the address, and the
/// disclosure still spells it.
#[must_use]
pub fn grounded(view: &View, rows: usize) -> View {
    let mut next = *view;
    next.selected = next.selected.min(rows.saturating_sub(1));
    next.open &= rows > 0;
    next.help &= rows > 0;
    next.wide &= rows > 0;
    // 🔴 The face's own offset is clamped by the same line, for the same reason the three above it
    // are: a list with nothing in it has no stream to stand in, and a record or the hatch is not a
    // stream at all. Left standing, the offset would survive a read that emptied the list and the
    // next frame would open somewhere nobody scrolled to.
    if rows == 0 || next.open || next.help {
        next.glide = 0;
    }
    next
}

/// The pointer's road: attend to the record under it.
///
/// 🔴 **A declared function and deliberately not an [`Act`]** (`req/924` §TUI-62 裁定2, `req/38`
/// SS1093, Owner `#284-T`). [`ACTS`] is the **key** binding table — `act-table.json` and its gate
/// read it as such — and a pointer is not a key: it carries a position, which no entry in that table
/// can. Adding a keyed act for it would put a row in the ledger that no key answers.
///
/// What it does is exactly what `j` and `k` do, arrived at differently, and that is why it may be
/// wired now: **clicking to select is a read**. `req/924` §TUI-50's order — *an act with an effect
/// comes after the consent screen* — is untouched, because this has no effect. The seat's earlier
/// ruling that the mouse waits for the input surface was withdrawn on exactly that ground.
///
/// Clamped here rather than by the caller, so a click below the last record attends to the last
/// record instead of to a row that is not there.
#[must_use]
pub fn attend(view: &View, index: usize, rows: usize) -> View {
    let mut next = *view;
    next.selected = index.min(rows.saturating_sub(1));
    // 🔴 Bringing the attention back into view is the point of clicking on it. A click that left
    // the face standing somewhere else would answer the reader's *position* and ignore their
    // *gesture*.
    next.glide = 0;
    grounded(&next, rows)
}

/// The wheel's road: move the face, and leave the attention where it is.
///
/// 🔴 **`req/924` §TUI-62 裁定3.** Scrolling down moves the content **up**, which is `delta`
/// positive. Not an [`Act`] for the reason [`attend`] is not one, and additionally because it is a
/// *rate* rather than a step: a wheel reports how far it turned.
///
/// The offset is unbounded here and clamped in `super::layout::scrolled`, which is the only place
/// that knows how tall the stream and the region are. A reducer that clamped it would be a reducer
/// claiming to know the size of a terminal.
#[must_use]
pub fn glide(view: &View, delta: isize, rows: usize) -> View {
    let mut next = *view;
    next.glide = next.glide.saturating_add(delta);
    grounded(&next, rows)
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
        // 🔴 Both **toggle**, so the key that opens is the key that closes and the note that names
        // it is therefore also the way out. A one-way act would need a second act to undo it, and a
        // second act nothing names is a room with no door. Both are then clamped by [`grounded`]
        // below, which is what keeps the empty list a fixed point.
        Effect::Help => {
            next.help = !next.help;
            Signal::None
        }
        Effect::Wide => {
            next.wide = !next.wide;
            Signal::None
        }
        Effect::Leave => Signal::Leave,
    };
    // The list can shrink between reads; an index into rows that are gone would draw an attention
    // mark on a record nobody is looking at.
    //
    // 🔴 Both of the lines that used to stand here are [`grounded`] now, and the paragraphs below
    // are kept beside the reducer they were written for. What moved is only *who else may ask*:
    // the question belongs to the draw road as much as to an act, because a reading can shrink the
    // list without any act being applied (`req/38` SS999, T-r9-B). Leaving a copy of the clamp here
    // as well would have been the two-answers defect the move exists to close.
    //
    // 🔴 **`req/38` SS974, design round 2's third finding — and the repair is one line further than
    // the finding was.** (The lines it produced are in [`grounded`]; this is why they exist.)
    //
    // The report said `act.open` moved the view on an empty list while `super::renderer::subject`
    // declined to open anything, so the declaration and the screen described two different
    // programs, and `super::renderer::offered` carried a paragraph explaining the disagreement
    // instead of anybody closing it. The obvious repair is a row count inside the `Open` arm.
    //
    // That repair is wrong, and gate g21 said so on the first run: it leaves `act.prev` carrying an
    // opened flag on a list that has emptied, because only the arm that was patched asks the
    // question. **The question is not "may this act open something", it is "does what the view
    // points at still exist"** — and the clamp on `selected` has been asking exactly that since the
    // first build. So it is asked once, about both members, and every act inherits it, including
    // the ones nobody has written yet. It is asked in [`grounded`] rather than on the next line,
    // which is the same sentence with one more caller in it: a reading inherits it too.
    (grounded(&next, rows), signal)
}
