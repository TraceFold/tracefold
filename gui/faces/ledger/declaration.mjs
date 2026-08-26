// SPDX-License-Identifier: Apache-2.0
// What this face says about itself, before it draws anything.
//
// The declaration is the only place in this face where a method of the server is
// spelled out. Everything else reaches the wire through a caller that refuses a name
// this file does not hold, so "the face called something it never declared" is not a
// thing a reviewer has to look for -- it raises.
//
// Two asymmetries are deliberate.
//
// `consumes` is an upper bound and not a record of use. A method may be declared and
// never sent, and that is normal: the gate is red only in the other direction. So
// `sends` and `withheld` split the declaration into what this window will actually
// put on a socket and what it will not, each withholding carrying the reason it is
// withheld, because "the button does nothing" and "the button is not offered" are
// both worse than "the button is dimmed and says why".
//
// `undrawn` exists because a screen that shows nothing and a screen that could not
// read anything look identical to a reader, and this product is one where a record
// going quiet is the worst available failure. Every member this face receives and
// does not draw is named here with its reason, and the face prints the list.

/** The face's own name. The shell learns it from here; the face never learns the shell's. */
export const FACE_ID = 'ledger';

export const QUESTION = 'What happened, in order.';

/**
 * Every method this face is permitted to reach. Names are the membrane's, which are
 * functions of a verb and a path, so nobody chose them and there is nothing here that
 * could be inherited from another implementation's vocabulary.
 */
export const CONSUMES = Object.freeze([
  'get_transformations',
  'get_candidates',
  'get_ledger_consistency',
  'post_candidates_id_commit',
  'post_candidates_id_cancel',
  'post_candidates_id_escalation',
  'post_transformations_id_undo',
]);

/** The reads this face performs when it is mounted. */
export const READS = Object.freeze({
  settled: 'get_transformations',
  held: 'get_candidates',
  consistency: 'get_ledger_consistency',
});

/**
 * The acts offered on a row, and the method each one is. `sends: false` is an act that
 * is drawn dimmed with its reason and never put on a socket.
 */
export const ACTS = Object.freeze([
  {
    act: 'commit',
    method: 'post_candidates_id_commit',
    half: 'held',
    label: 'commit',
    sends: true,
  },
  {
    act: 'cancel',
    method: 'post_candidates_id_cancel',
    half: 'held',
    label: 'cancel',
    sends: true,
  },
  {
    act: 'escalate',
    method: 'post_candidates_id_escalation',
    half: 'held',
    label: 'escalate',
    sends: false,
    why: 'the members of this request were never read out of the crate that serves it, so this window does not know what a correct one looks like and will not send a guess. Declared, offered, and dimmed until the body is read.',
  },
  {
    act: 'undo',
    method: 'post_transformations_id_undo',
    half: 'settled',
    label: 'undo',
    sends: true,
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
 * The marks this face is permitted to put on a screen, each with the one meaning it
 * carries. The meaning is declared rather than inferred because no machine can decide
 * whether two drawings say the same thing, and a screen where two marks say one thing
 * teaches a reader a vocabulary that is not real.
 */
export const MARKS = Object.freeze([
  { mark: 'verdict/Admit', means: 'verdict.admit', from: 'the engine answered Admit' },
  { mark: 'verdict/Deny', means: 'verdict.deny', from: 'the engine answered Deny' },
  { mark: 'verdict/Escalate', means: 'verdict.escalate', from: 'the engine answered Escalate' },
  { mark: 'standing/held', means: 'standing.held', from: 'the half that has not happened' },
  { mark: 'structure/seal', means: 'structure.sealed', from: 'the half that has happened' },
  { mark: 'structure/unsealed', means: 'structure.unsealed', from: 'no verifier is present, so no record here is sealed' },
  { mark: 'structure/child', means: 'structure.child', from: 'a row written under an earlier row' },
  { mark: 'structure/hole', means: 'structure.hole', from: 'a member that was looked for and not found' },
  { mark: 'effect/write', means: 'effect.write', from: 'the engine reported this delta as a write' },
  { mark: 'effect/delete', means: 'effect.delete', from: 'the engine reported this delta as a delete' },
  { mark: 'effect/undo', means: 'effect.undo', from: 'the engine reported this delta as an undo' },
  { mark: 'act/commit', means: 'act.commit', from: 'the commit control offered on a held row' },
  { mark: 'act/cancel', means: 'act.cancel', from: 'the cancel control offered on a held row' },
  { mark: 'act/undo', means: 'act.undo', from: 'the undo control offered on a settled row' },
  { mark: 'act/escalate', means: 'act.escalate', from: 'the escalate control offered on a held row (withheld)' },
  { mark: 'structure/fold-shut', means: 'structure.fold.shut', from: 'the why/legend control, or a row\'s own disclosure, is folded closed (SS657 retrofit: bordered controlToggle()/openableRow() controls, same marks faces/atlas already draws)' },
  { mark: 'structure/fold-open', means: 'structure.fold.open', from: 'the why/legend control, or a row\'s own disclosure, is drawn open' },
  { mark: 'standing/reversed', means: 'standing.reversed', from: 'the reversibility chip (req/768 F-I, retrofit round 2): a later row in this same read names this row as its predecessor, so its own escrowed inverse was already used' },
  { mark: 'standing/none', means: 'standing.none', from: 'the reversibility chip: whether this settled row can still be undone is not observable from what this window reads -- a declared hole in the membrane\'s own wire fields, not a guess' },
  { mark: 'undefined', means: 'mark.undefined', from: 'a word arrived where a known one was expected' },
]);

/** Where this face sits, and why it sits there rather than wherever a sort put it. */
export const ORDER = Object.freeze({
  position: 1,
  reason: 'first, because every other face in this application is a second look at something this one has already shown: the receipt face magnifies one of these rows, the graph face projects the same rows a second way, and the held face is this face\'s second half on its own. A reader who starts anywhere else is starting at a detail.',
  dock: null,
});

export const ROWS = Object.freeze({
  draws: true,
  order: 'by-sequence',
  order_reason: 'ascending sequence number, because the issuer assigns it in the order it issued, and an identity is not a clock',
  reports_denominator: true,
  note: 'the count of rows drawn is stated against the count received and the number of requests the walk took, because "two hundred rows" reads the same whether the list ended or the walk stopped',
});

/**
 * Received and not drawn. Each line is a thing a reader might reasonably expect to see
 * and will not, with the reason it is missing.
 */
export const UNDRAWN = Object.freeze([
  {
    what: 'next_cursor',
    why: 'this window folds the pages together before this face ever sees them, so the cursor never reaches here at all. The walk it performed is drawn instead.',
  },
  {
    what: 'notices',
    why: 'the window\'s own record of what it asked the server is handed to this face and drawn by the notice face. A face that reported its own failures would be reporting them through the thing that failed.',
  },
  {
    what: 'get_stream',
    why: 'live events are not consumed by this face. What is on this screen is what was true when it was read, and it says when that was.',
  },
  {
    what: 'the members of a transformation body',
    why: 'the crate\'s response bodies were never read, so the members named in the column map are the ones this face looked for and not the ones the server is known to send. Any member that is absent is drawn as a declared hole rather than as an empty cell.',
  },
  {
    what: 'offline re-verification',
    why: 'checking a receipt without its issuer is the third pillar of this product and it is not this face\'s job. No verifier is carried here, so nothing on this screen is drawn as checked.',
  },
  {
    what: 'anything about a row that is not one of the seven columns',
    why: 'the row is a fixed pitch line so an appended ledger does not reflow while it is being read. What does not fit goes into the note that opens under the row.',
  },
  {
    what: 'a value wider than its column, in the row itself',
    why: 'the row clips rather than wraps, so a long path is cut off on the line. Every value the row holds is repeated in full in the note underneath, and the note opens by itself for any row carrying a value long enough to be at risk, because a value that is only ever shown cut off is a record that has gone quiet.',
  },
]);

/** C-7 is satisfied elsewhere; naming which face keeps it from being nobody's job. */
export const SILENT_FACE = 'notice (F-5) is the face that declares no methods at all; this one declares seven and is not the control for that rule';

export const TESTS = Object.freeze([
  'test/declaration.test.mjs',
  'test/ledger.test.mjs',
  'test/gate.test.mjs',
]);

export const DECLARATION = Object.freeze({
  id: FACE_ID,
  question: QUESTION,
  consumes: CONSUMES,
  reads: READS,
  acts: ACTS,
  sends: SENDS,
  withheld: WITHHELD,
  marks: MARKS,
  order: ORDER,
  rows: ROWS,
  undrawn: UNDRAWN,
  silent_face: SILENT_FACE,
  tests: TESTS,
});
