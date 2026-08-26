// SPDX-License-Identifier: Apache-2.0
// What this face says about itself, before it draws anything.
//
// This is the face C-7 asks for: at least one face in the application that reaches
// no method of the server at all. `CONSUMES` is empty on purpose, and it stays that
// way structurally -- `notice.mjs` is never handed a reason to import a method name,
// because the thing it draws is not an answer from the server, it is the window's
// own record of having asked one. A face that reported its own failures by asking
// the server about them would be reporting a failure through the thing that failed.
//
// Every other face in this application answers a question about a record the engine
// holds. This one answers a question about the window itself: what did it ask, when,
// and what came back. That is why it sits last in the rail (`ORDER`) rather than
// first -- a reader comes here after wondering why another screen looked wrong, not
// before wanting to know anything at all.

/** The face's own name. The shell learns it from here; the face never learns the shell's. */
export const FACE_ID = 'notice';

export const QUESTION = 'What this window said, about itself.';

/** C-7: this face declares no method of the server. It never will -- the thing it
 * draws is not a route's answer, it is the record of the window having asked one. */
export const CONSUMES = Object.freeze([]);

/** Nothing is read from a route on mount; the whole of what this face draws is
 * already sitting in the array the shell hands it as its third argument. */
export const READS = Object.freeze({});

/** This face offers no act that reaches the server -- ACTS names a wire method the
 * way faces/ledger's does, and this face declares none, C-7's whole point.
 *
 * req/768 AC-4 (retrofit round 2, 2026-08-25): this empty array is also why the
 * round-2 act gutter (`parts/src/receipt-row.mjs`'s `actGutter()`) never draws
 * on this face -- a gutter with zero offered acts is a no-op by construction
 * (`rowWithGutter()` returns its row unchanged), not a hole this face's own
 * source has to guard against. AC-7 (the reversibility chip) is skipped for the
 * same underlying reason stated one layer up: this face's own row is a notice
 * about a network call (asked/answered/refused/failed/absent/elsewhere), never
 * a committed delta, so "does this row's escrowed inverse still exist" is not a
 * question this face's own data could ever honestly answer -- not even
 * "unknown", which would still be answering a question that does not apply
 * here. See req/03 §2 F-5's own round-2 addendum for the full accounting.
 *
 * Owner #348 (2), 2026-08-25. What used to stand here was a paragraph about a
 * control called `reach`: a button drawn on every row, permanently disabled,
 * carrying the reason this face cannot jump to another one. Eight of them stood
 * in a column on the representative screen -- eight invitations that went
 * nowhere, which the round-4 report named as the worst thing left on this face.
 * The honest reading is that a disabled control answering a question nobody
 * asked is worse than no control: it takes the one column a row has for a thing
 * a hand can do, and spends it on a thing no hand can do. It is retired, its
 * reason is kept as a declared omission below (UNDRAWN, "a way through to the
 * face that reads this record"), and what stands in that column now is OFFERS --
 * things this face can actually make good on. */
export const ACTS = Object.freeze([]);

/**
 * What a row lets a reader take away with them.
 *
 * These are deliberately not ACTS and never will be: an act names a method of the
 * server, and this face has none (C-7). An offer names a value already on this
 * screen and a way to have it -- which is the whole of what a face with nothing to
 * send can honestly put under a hand.
 *
 * One list, two consumers: the gutter draws the member marked `gutter` on every row
 * that can make good on it, and the right-click menu draws all four. They cannot
 * disagree about what a row offers, because there is one place that says.
 *
 * `why` is what the reader is told when this record has nothing to give -- the
 * control is still drawn, disabled, carrying that sentence, which is the same rule
 * the gutter already followed for a control it could not send. Absent for the two
 * offers every record with a method can always answer.
 */
export const OFFERS = Object.freeze([
  {
    id: 'row',
    label: 'copy row',
    menu: 'copy this whole row',
    of: 'the time, the call and how it came back, on one line',
    gutter: true,
  },
  {
    id: 'call',
    label: 'copy call',
    menu: 'copy the call',
    of: 'what was asked: the verb and the path where the window recorded both, and the method name where it did not',
    why: 'this entry was recorded without a method name, so there is nothing here to name the call by',
  },
  {
    id: 'time',
    label: 'copy time',
    menu: 'copy the time',
    of: 'the whole timestamp, not the shortened form the row has room to draw',
    why: 'this entry was recorded without a time',
  },
  {
    id: 'code',
    label: 'copy word',
    menu: 'copy the word the server sent',
    of: 'the server\'s own spelling, exactly as it arrived',
    why: 'this answer came back carrying no word of the server\'s own, so there is nothing to keep',
  },
]);

export const SENDS = Object.freeze([]);

export const WITHHELD = Object.freeze([]);

/**
 * The marks this face is permitted to put on a screen. Two, both borrowed from the
 * shared sheet with the meaning they already carry there (Z-2's stated exception: a
 * mark name is furniture the whole application shares, not a word one face owns).
 */
export const MARKS = Object.freeze([
  { mark: 'structure/hole', means: 'structure.hole', from: 'a call this window made came back naming a route the table does not carry' },
  { mark: 'effect/network', means: 'effect.network', from: 'this entry was carried over the network to be answered' },
  { mark: 'effect/message', means: 'effect.message', from: 'this entry was answered inside this window, without a call leaving it' },
  // These two sentences are drawn on the screen, in the legend's third column. They
  // used to end in a parenthesis naming a directive number, two function names and
  // another face's directory -- four things a reader of this application has no way
  // to look up and no reason to meet. That provenance belongs in a comment, which is
  // where it now is: the fold marks were adopted from the bordered-disclosure idiom
  // this application's other faces already draw, under Owner #317/#318.
  { mark: 'structure/fold-shut', means: 'structure.fold.shut', from: 'something on this screen is folded away' },
  { mark: 'structure/fold-open', means: 'structure.fold.open', from: 'a fold is open and what it held is below' },
  { mark: 'undefined', means: 'mark.undefined', from: 'an entry arrived carrying an outcome word this face holds no specific rendering for' },
]);

/** Where this face sits, and why it sits there rather than wherever a sort put it. */
export const ORDER = Object.freeze({
  position: 5,
  reason: 'last, because every other face in this application answers a question about a record the engine holds, and this one answers a question about the window that asked for it. A reader arrives here after a row elsewhere looked wrong, not before wanting to know anything at all.',
  dock: null,
});

export const ROWS = Object.freeze({
  draws: true,
  order: 'as-recorded',
  order_reason: 'the order entries arrived in, because the shell writes one down the instant a call is asked or answered -- that instant already is the order it happened in, and sorting it a second way would be presenting an order nobody produced',
  reports_denominator: true,
  note: 'the number of entries drawn is stated against the number the window has recorded and the number left undrawn because the budget on this screen was reached, so a quiet screen and a busy one that stopped drawing read as two different facts',
});

/**
 * Received (in the loose sense: sitting in the window's own record) and not drawn.
 * Each line is a thing a reader might expect this screen to hold and will not find,
 * with the reason it is missing.
 */
export const UNDRAWN = Object.freeze([
  {
    what: 'the body of an answered call',
    why: 'this screen states that a call happened and how it came back -- an outcome word, a status, a code where the engine gave one -- not the payload the answer carried. The face that asked for that data already draws it, and drawing it a second time here would let the two screens disagree about the same fact.',
  },
  {
    what: 'individual frames of a live stream',
    why: 'the window records the request that opened a stream and how the socket answered opening it, not each frame the stream goes on to carry. Reading the frames themselves belongs to whichever face consumes that stream.',
  },
  {
    what: 'who is acting',
    why: 'the membrane attaches an actor to a write and never hands this face one to draw. Every entry here names the method that was asked and what came back, never who asked it.',
  },
  {
    what: 'a colour that marks a call as having gone well',
    why: 'a call that answered and a call still waiting read in the same ink here. The one colour this application spends is reserved for the deny mark, and this face does not draw one -- success is not a thing this screen colours in.',
  },
  {
    what: 'an exception the shell caught while raising or lowering a face',
    why: 'nothing writes such an exception into this window\'s record today. If the shell begins doing so, it is drawn through the same generic entry every other line here goes through, because every line is drawn from its outcome word and its detail rather than from a fixed list of shapes this face was told in advance to expect.',
  },
  {
    what: 'the server\'s own spelling, on the row',
    why: 'where an answer came back carrying a word this window did not choose -- a token in capitals, a fragment of the request echoed back -- the row says what happened in this application\'s own words and that spelling is kept under the reference control instead. Nothing is dropped: a refusal\'s own code was evidence before this and is evidence after it, one press away and labelled as what it is, rather than printed on a product surface where it reads as undecodable to anyone who does not already work here.',
  },
  {
    what: 'a way through to the face that reads this record',
    why: 'this window cannot reach another screen from here. What this face is handed when it is put on screen is a host to draw into, the surface it never calls, and its own record -- no way to address another screen and no way to import one. The method on each row names the record the call reached; a reader who wants to see that record opens the screen that reads it and looks it up there. This was a button on every row until 2026-08-25, dimmed and carrying this sentence; a control that can never act is not worth the column a control that can act could have.',
  },
  {
    what: 'entries past the drawn budget',
    why: 'the count still on screen after the budget is reached names how many more have arrived. The rows already drawn are left standing rather than pushed out to make room for newer ones, because a reader partway down this screen should not find the top of it has moved.',
  },
]);

/** C-7 is held here. Every other face in this application declares at least one
 * method; this is the one that declares none, and it says so by naming itself. */
export const SILENT_FACE_NOTE = 'this is the face C-7 asks for: it declares zero methods of the server and calls none. No other face in this application may claim to be the one that satisfies C-7 -- there is exactly one, and it is named here.';

export const TESTS = Object.freeze([
  'test/declaration.test.mjs',
  'test/notice.test.mjs',
  'test/gate.test.mjs',
]);

export const DECLARATION = Object.freeze({
  id: FACE_ID,
  question: QUESTION,
  consumes: CONSUMES,
  reads: READS,
  acts: ACTS,
  offers: OFFERS,
  sends: SENDS,
  withheld: WITHHELD,
  marks: MARKS,
  order: ORDER,
  rows: ROWS,
  undrawn: UNDRAWN,
  silent_face_note: SILENT_FACE_NOTE,
  tests: TESTS,
});
