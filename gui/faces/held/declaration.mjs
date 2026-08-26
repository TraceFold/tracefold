// SPDX-License-Identifier: Apache-2.0
// What this face says about itself, before it draws anything.
//
// held answers one question -- what has not happened yet -- and the answer must
// never be drawn wearing the receipt face's clothes. A held candidate is a proposal:
// it has a digest and a path the way a settled row does, and it has no seal, because
// nothing about it has been checked or committed. Every cell that would claim
// otherwise is withheld at the row level (declaration.mjs holes) rather than at the
// screen level (a paragraph saying "these are not receipts"), so the discipline is
// structural and a reviewer does not have to trust a sentence.
//
// req/03 F-2's reference row (ui_proto FACE.json, read only as a contract, never
// opened as code) declared six consumed methods: escalate / candidates / commit /
// cancel / undo / subscribe. Five of those six are real names on this rebuilt
// membrane's own route table (membrane/route-table.json, verified against
// membrane/src/membrane.mjs's tableRows() at declaration-test time, the same C-1
// discipline faces/ledger holds). The sixth, "subscribe", has no membrane
// equivalent here: this project's membrane exposes get_stream (a general SSE feed,
// already declared out of scope for faces/ledger's own UNDRAWN list) and nothing
// narrower. Rather than invent a method name the wire does not carry -- which would
// itself violate C-1 the moment a caller tried to reach it -- this declaration
// states the deviation honestly instead of silently matching the reference's count
// (req/03 §7 Q1: reading the reference's declared shape and re-deriving this
// project's own is not the same as inheriting it).

/** The face's own name. The shell learns it from here; the face never learns the shell's. */
export const FACE_ID = 'held';

export const QUESTION = 'What has not happened yet.';

/** Every method this face is permitted to reach. Real membrane names only -- see
 * the file header for why this is five and not the reference's six. */
export const CONSUMES = Object.freeze([
  'get_candidates',
  'post_candidates_id_commit',
  'post_candidates_id_cancel',
  'post_candidates_id_escalation',
  'post_transformations_id_undo',
]);

/** The one read this face performs when it is mounted. */
export const READS = Object.freeze({
  held: 'get_candidates',
});

/**
 * The acts offered on a row. Two are sent; two are declared, offered, and
 * withheld, each for a different reason -- reachable is not the same as offered
 * (req/03 §2 F-2: "undo is reachable through the registry but is not actually
 * offered"), and this declaration keeps the two withholdings apart rather than
 * folding them into one dimmed row of buttons with one shared excuse.
 */
export const ACTS = Object.freeze([
  {
    act: 'commit',
    method: 'post_candidates_id_commit',
    label: 'commit',
    sends: true,
  },
  {
    act: 'cancel',
    method: 'post_candidates_id_cancel',
    label: 'cancel',
    sends: true,
  },
  {
    act: 'escalate',
    method: 'post_candidates_id_escalation',
    label: 'escalate',
    sends: false,
    why: 'the members of this request were never read out of the crate that serves it, so this window does not know what a correct one looks like and will not send a guess. Declared, offered, and dimmed until the body is read (the same withholding the ledger screen states for the same route).',
  },
  {
    act: 'undo',
    method: 'post_transformations_id_undo',
    label: 'undo',
    sends: false,
    why: 'a held candidate has not produced a transformation yet, so there is no transformation id on this row for undo to target. The method is declared because this window could reach it, not because this screen has anything to send it.',
  },
]);

export const SENDS = Object.freeze([
  ...Object.values(READS),
  ...ACTS.filter((a) => a.sends).map((a) => a.method),
]);

export const WITHHELD = Object.freeze(
  ACTS.filter((a) => !a.sends).map((a) => ({ method: a.method, act: a.act, why: a.why })),
);

/**
 * The gates a candidate stands at, and the one act each of them answers with.
 *
 * The four acts above were already offered on every row, side by side in a gutter,
 * with no statement anywhere on the screen of *why* one of them was live and another
 * was dimmed beyond a sentence in a title attribute. Four buttons is not four answers.
 * A gate here is the pair (a question this window can settle from what it read, the
 * one act that question governs), and the screen draws one container for each: the
 * gate's own answer first, its act inside the same container, and where the act is
 * unavailable the reason for that sits beside the control at the same size, never
 * behind a pointer.
 *
 * Three rules this list is written under, and the reason for each:
 *
 *  1. **Every gate's act is one of the four above.** No gate invents a verb. The
 *     count is exactly ACTS.length and the mapping is one to one, so the ladder is
 *     the gutter re-stated as answers rather than a second, larger set of controls a
 *     reader has to reconcile with the first.
 *  2. **`reads` names what the answer is computed from**, so a gate that claims to
 *     know something states where it read it. Two of the four are settled by this
 *     declaration alone (an act that does not send cannot become available because a
 *     row was chosen); two need a chosen row's identity.
 *  3. **A gate this window cannot settle renders as unknown, never as passed.** That
 *     case is real and is reachable on this screen: when the read did not answer,
 *     this window does not know whether there is a candidate here at all, and a gate
 *     drawn open then would be this face inventing a permission out of a failed read.
 *
 * The order is the order a reader climbs: what a person has been asked, what could be
 * put back, the withdrawal, and last the one act that ends the waiting. The terminal
 * act sits at the bottom because that is where the eye stops.
 */
export const GATES = Object.freeze([
  {
    gate: 'raised',
    name: 'raised to a person',
    act: 'escalate',
    asks: 'whether this window may put this candidate in front of a person',
    reads: ['the acts this face declares it sends', 'verdict'],
  },
  {
    gate: 'inverse',
    name: 'an inverse held',
    act: 'undo',
    asks: 'whether an inverse is held for this candidate, so that what it proposes could be put back',
    reads: ['lifecycle'],
  },
  {
    gate: 'withdrawal',
    name: 'withdrawn instead',
    act: 'cancel',
    asks: 'whether this window may withdraw this candidate',
    reads: ['the acts this face declares it sends', 'id'],
  },
  {
    gate: 'commit',
    name: 'committed',
    act: 'commit',
    asks: 'whether this window may commit this candidate',
    reads: ['the acts this face declares it sends', 'id'],
  },
]);

/**
 * The marks this face is permitted to put on a screen. Held reuses receipt-row's
 * eight-column grid (the same part faces/ledger draws its held half with), so it
 * reuses that half's meanings rather than inventing a second drawing for one
 * already-owned meaning (C-5: one meaning, one mark, held across every face that
 * shares a part). Twelve marks, not the reference's fourteen -- derived from what
 * this face actually draws, not copied from a count this project never verified
 * (see file header).
 */
export const MARKS = Object.freeze([
  { mark: 'verdict/Admit', means: 'verdict.admit', from: 'the engine\'s provisional word on this candidate was Admit' },
  { mark: 'verdict/Deny', means: 'verdict.deny', from: 'the engine\'s provisional word on this candidate was Deny' },
  { mark: 'verdict/Escalate', means: 'verdict.escalate', from: 'the engine\'s provisional word on this candidate was Escalate -- the common reason a candidate is held at all' },
  { mark: 'standing/held', means: 'standing.held', from: 'every row on this screen: nothing here has happened yet -- and every gate that is shut, which is the same fact said about one act' },
  { mark: 'effect/write', means: 'effect.write', from: 'this candidate proposes a write' },
  { mark: 'effect/delete', means: 'effect.delete', from: 'this candidate proposes a delete' },
  { mark: 'act/commit', means: 'act.commit', from: 'the commit control, offered on every row and on the gate that governs it' },
  { mark: 'act/cancel', means: 'act.cancel', from: 'the cancel control, offered on every row and on the gate that governs it' },
  { mark: 'act/escalate', means: 'act.escalate', from: 'the escalate control, offered on every row and on the gate that governs it (withheld)' },
  { mark: 'act/undo', means: 'act.undo', from: 'the undo control, offered on every row and on the gate that governs it (withheld: nothing to target yet)' },
  { mark: 'structure/hole', means: 'structure.hole', from: 'a member that was looked for and not found, a seal cell on a row that has nothing to seal, or a gate whose answer this window could not read' },
  { mark: 'structure/fold-shut', means: 'structure.fold.shut', from: 'the why/legend control, or a row\'s own disclosure, is folded closed' },
  { mark: 'structure/fold-open', means: 'structure.fold.open', from: 'the why/legend control, or a row\'s own disclosure, is drawn open' },
  { mark: 'undefined', means: 'mark.undefined', from: 'a word arrived where a known one was expected' },
]);

/** Where this face sits, and why. */
export const ORDER = Object.freeze({
  position: 2,
  reason: 'second, directly after the ledger: held is not a new subject, it is the ledger\'s own second half, pulled out to its own screen so what has not happened yet is never scrolled past on the way to what has. A reader who has just read what happened is the reader who most needs to see what is waiting.',
  dock: null,
});

export const ROWS = Object.freeze({
  draws: true,
  order: 'by-sequence',
  order_reason: 'ascending sequence number, the same order the ledger applies to its own settled half -- an identity is not a clock, and the two screens should not disagree about which one is',
  reports_denominator: true,
  note: 'the count of candidates drawn is stated against the count received and the number of requests the walk took, for the same reason the ledger states it: a truncated list that only says a row count has gone quiet',
});

/**
 * Received and not drawn. Each line is a thing a reader might reasonably expect to
 * see and will not, with the reason it is missing.
 */
export const UNDRAWN = Object.freeze([
  {
    what: 'next_cursor',
    why: 'this window folds the pages together before this face ever sees them, so the cursor never reaches here at all. The walk it performed is drawn instead.',
  },
  {
    what: 'notices',
    why: 'the window\'s own record of what it asked the server is handed to this face and drawn by the notice screen, never by this one.',
  },
  {
    what: 'get_stream',
    why: 'live events are not consumed by this face. This screen shows what was true when it was read, and states when that was; a candidate that changed standing a moment ago is seen on the next read, not pushed here.',
  },
  {
    what: 'the members of a candidate body',
    // req/822_c6: no longer "never read". The list-row members were dumped from a live
    // engine and the membrane carries a declared projection of them into this column
    // map (membrane/src/vocabulary.mjs — M-15's "time / who / target"). What is still
    // true: any member absent on a row is drawn as a declared hole, never as an empty
    // cell; `effect` and `digest` have no wire counterpart on a list row and stay holes.
    why: 'the engine\'s list-row members are read and carried through a declared membrane projection (time / who / target); a member absent on a row is still drawn as a declared hole rather than as an empty cell, and the columns with no wire counterpart (effect, the fingerprint digest) are holes on every wire row.',
  },
  {
    what: 'state and enforced, and the raw row under wire',
    why: 'the engine also sends its own words for a row -- state (its lifecycle vocabulary), enforced, and the whole untranslated row is preserved under a wire member. This screen does not draw them: state is the engine\'s vocabulary and this screen\'s standing chip already says held for every row (the endpoint\'s own filter), and the raw row is for a reader of the port, not of this screen.',
  },
  {
    what: 'a sealed record of any kind',
    why: 'nothing on this screen has happened yet, so nothing on it can be sealed. The seal column is a declared hole on every row, unconditionally -- this is the one column on this screen whose emptiness is never data, only the fact this face exists to state.',
  },
  {
    what: 'anything about a row that is not one of the eight columns',
    why: 'the row is a fixed-pitch line, the same grid the ledger draws its rows with. What does not fit goes into the pane beside the list.',
  },
  {
    what: 'a value wider than its column, in the row itself',
    why: 'the row clips rather than wraps. Every value the row holds is repeated in full in the note underneath, opened by itself for any row carrying a value long enough to be at risk.',
  },
]);

/** C-7 is satisfied elsewhere; naming which face keeps it from being nobody's job. */
export const SILENT_FACE = 'notice (F-5) is the face that declares no methods at all; this one declares five and is not the control for that rule';

export const TESTS = Object.freeze([
  'test/declaration.test.mjs',
  'test/held.test.mjs',
  'test/gate.test.mjs',
]);

export const DECLARATION = Object.freeze({
  id: FACE_ID,
  question: QUESTION,
  consumes: CONSUMES,
  reads: READS,
  acts: ACTS,
  gates: GATES,
  sends: SENDS,
  withheld: WITHHELD,
  marks: MARKS,
  order: ORDER,
  rows: ROWS,
  undrawn: UNDRAWN,
  silent_face: SILENT_FACE,
  tests: TESTS,
});
