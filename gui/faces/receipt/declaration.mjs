// SPDX-License-Identifier: Apache-2.0
// What this face says about itself, before it draws anything.
//
// receipt answers one question -- can one delta be confirmed without asking the
// window that shows it -- and everything else in this face exists to serve that one
// answer honestly. req/03 F-3's reference row (ui_proto FACE.json, read only as a
// contract per req/03 §7 Q1, never opened as code) declared two consumed methods:
// transformations / receipt. Both have real names on this rebuilt membrane's own
// route table (membrane/route-table.json: GET /transformations/{id}, GET
// /receipts/{tid}), verified against membrane/src/membrane.mjs's tableRows() at
// declaration-test time, the same C-1 discipline every other face in this tree holds.
// Unlike faces/held (5 of the reference's 6), this face's count matches the
// reference's declared count exactly -- 2 of 2 -- stated here rather than assumed,
// because a count that happens to match is still a claim that needs a test.
//
// glovrex/req/405 SS5 (frozen SS605, glovrex/req/38): a receipt's own body is the
// attest layer (facts only, nothing moved for how it looks) and any badge/summary
// drawn from it is the render layer (derived from attest, adds nothing). This
// declaration and receipt.mjs's own toRecord()/view() split are this face's
// implementation of that binding contract.

/** The face's own name. The shell learns it from here; the face never learns the shell's. */
export const FACE_ID = 'receipt';

export const QUESTION = 'Can this one delta be confirmed without asking this window.';

/** Every method this face is permitted to reach. Real membrane names only. */
export const CONSUMES = Object.freeze([
  'get_transformations_id',
  'get_receipts_tid',
]);

/** The two reads this face performs when it is mounted, for the one id it is given. */
export const READS = Object.freeze({
  delta: 'get_transformations_id',
  receipt: 'get_receipts_tid',
});

/**
 * No act changes anything here -- confirming a receipt is read-only by definition,
 * so this is the empty-but-declared shape req/03 §8's own repair note asks every
 * face to carry (`ACTS = Object.freeze([])`, structural, not an empty row of
 * buttons stating the same thing visually).
 */
export const ACTS = Object.freeze([]);

export const SENDS = Object.freeze([...Object.values(READS)]);

export const WITHHELD = Object.freeze([]);

/**
 * The marks this face is permitted to put on a screen. Nine, not the reference's
 * eleven -- derived from what this face's binding actually draws (verified:
 * test/declaration.test.mjs's C-5 cross-check confirms every meaning receipt shares
 * with faces/ledger/faces/held uses the identical mark, not a second drawing for one
 * already-owned meaning). No act marks (there are no acts on this screen); no
 * standing/held mark (the delta this face draws has, by definition, already
 * happened -- a receipt is never issued for a candidate).
 */
export const MARKS = Object.freeze([
  { mark: 'verdict/Admit', means: 'verdict.admit', from: 'the engine\'s recorded word for this delta was Admit' },
  { mark: 'verdict/Deny', means: 'verdict.deny', from: 'the engine\'s recorded word for this delta was Deny' },
  { mark: 'verdict/Escalate', means: 'verdict.escalate', from: 'the engine\'s recorded word for this delta was Escalate' },
  { mark: 'effect/write', means: 'effect.write', from: 'this delta was a write' },
  { mark: 'effect/delete', means: 'effect.delete', from: 'this delta was a delete' },
  { mark: 'structure/seal', means: 'structure.seal', from: 'the seal claim for this receipt is sealed -- compared exactly, by a verifier that is present' },
  { mark: 'structure/unsealed', means: 'structure.unsealed', from: 'the seal claim for this receipt is unsealed -- for any reason (no verifier present, an inexact basis, or nothing to compare)' },
  { mark: 'structure/hole', means: 'structure.hole', from: 'a member that was looked for and not found' },
  // These three `from` sentences are drawn on the screen, in the legend, so they say
  // what the mark means to a reader and name nothing internal to this repository.
  // The retrofit rounds and clause numbers they used to carry are recorded in this
  // file's own comments and in faces/receipt/README.md, where a reader who wants the
  // provenance can find it and a first viewer is not made to read it.
  { mark: 'structure/fold-shut', means: 'structure.fold.shut', from: 'the why/legend control, or the delta row\'s own disclosure, is folded closed' },
  { mark: 'structure/fold-open', means: 'structure.fold.open', from: 'the why/legend control, or the delta row\'s own disclosure, is drawn open' },
  { mark: 'standing/none', means: 'standing.none', from: 'the undo availability chip: this face reads one delta with no sibling list around it, so whether it can still be undone is never observable here -- always this one mark, never "reversed" (there is no sibling to check) and never "n/a" (a delta this face can draw has always already happened)' },
  { mark: 'undefined', means: 'mark.undefined', from: 'a word arrived where a known one was expected' },
]);

/** Where this face sits, and why. */
export const ORDER = Object.freeze({
  position: 3,
  // Drawn on the screen, in the why control: plain language, no face codes.
  reason: 'third, after ledger (what happened) and held (what has not happened yet): once a reader knows a delta happened, the next question this app can answer is whether that one delta can be confirmed without asking this window again -- which is a question about one delta at a time, not a list, so it is answered on its own screen rather than folded into either list.',
  dock: null,
});

/** This screen draws one record, not a list -- there is no order to declare over a
 * population of one, so ROWS states that plainly rather than carrying a `row-order`
 * declaration this screen has nothing to apply it to. */
export const ROWS = Object.freeze({
  draws: true,
  order: 'single-record',
  order_reason: 'this screen is handed exactly one delta id and draws exactly one row for it; parts/src/row-order.mjs exists to order a list, and there is no list here to order',
  reports_denominator: false,
  note: 'a receipt is drawn for one delta at a time; the denominator this screen states is not a row count but a claim count (how many of the confirm-without-us claims below hold)',
});

/**
 * Received and not drawn. Each line is a thing a reader might reasonably expect to
 * see and will not, with the reason it is missing.
 *
 * Both halves are drawn: the `what` in the omitted box on the screen, the pair in
 * full in the legend, and the `why` reachable on the omitted line itself. So these
 * sentences are product surface, and they name nothing internal -- no module path, no
 * file, no clause number. The provenance for each of them is in this file's comments
 * and in the README:
 *
 *  1. membrane/wire-fields.json states that this domain contributes no fields yet.
 *  2. req/97 RC-5, still open on this face.
 *  3. faces/notice (F-5) is the face that draws this window's own calls.
 *  4. parts/src/seal-claim.mjs's claimOf() is what refuses to say sealed.
 */
export const UNDRAWN = Object.freeze([
  {
    what: 'the members of a transformation or receipt body',
    why: 'the server\'s own response bodies have never been read, so the members this screen looks for are the ones it was written to look for and not the ones the server is known to send. Any member that is absent is drawn as a declared hole rather than as an empty cell.',
  },
  {
    what: 'a second delta',
    why: 'this screen draws exactly one delta at a time, by id -- a reader wanting a second receipt opens this screen again. Reaching it from a row on the list of what happened, or from one still held, is a control that does not exist yet.',
  },
  {
    what: 'the notices this window\'s own calls produced',
    why: 'the window\'s own record of what it asked the server is handed to this screen and drawn by the screen that exists for it, never by this one.',
  },
  {
    what: 'a verified seal, when no verifier is present',
    why: 'the one part of this app that decides whether something is sealed never says so without a verifier in hand -- in this environment there is none, so this screen states unsealed and the reason, never a guess dressed as a check.',
  },
]);

/** C-7 is satisfied elsewhere (faces/notice); naming which face keeps it from being
 * nobody's job. */
export const SILENT_FACE = 'notice (F-5) is the face that declares no methods at all; this one declares two and is not the control for that rule';

export const TESTS = Object.freeze([
  'test/declaration.test.mjs',
  'test/receipt.test.mjs',
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
