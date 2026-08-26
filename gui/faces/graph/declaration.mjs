// SPDX-License-Identifier: Apache-2.0
// What this face says about itself, before it draws anything.
//
// graph answers one question -- what has been touched twice or more -- and draws
// the second (and later) projection of a row this app already has a first
// projection of on faces/ledger: the same `path` recurring across more than one
// transformation. req/03 F-4's reference row (ui_proto FACE.json, read only as a
// contract per req/03 §7 Q1, never opened as code) declared two consumed methods:
// transformations / subscribe. Only one of those has a real name on this rebuilt
// membrane's own route table (membrane/route-table.json: GET /transformations,
// verified against membrane/src/membrane.mjs's tableRows() at declaration-test
// time, the same C-1 discipline every other face in this tree holds). "subscribe"
// has no membrane equivalent here -- the same honest deviation faces/held and
// faces/ledger already state for the identical reference-row word, not silently
// matched by inventing a route the wire does not carry.
//
// req/03 §5's own line for this face is the design brief for everything below it:
// "同じ行の第2の投影。両端が見えている物だけ描き、外へ出る線は「描いていない」と言う。
// 絵に到達しなかった行を必ず数えて出す" -- draw an edge only when both the row it
// starts from and the row it ends at were actually read by this window, declare a
// line that would leave the window as not drawn, and count every row that never
// reached the picture rather than let it vanish into a shorter list (C-3).

/** The face's own name. The shell learns it from here; the face never learns the shell's. */
export const FACE_ID = 'graph';

export const QUESTION = 'What has been touched twice or more.';

/** Every method this face is permitted to reach. Real membrane names only -- see
 * the file header for why this is one and not the reference's two. */
export const CONSUMES = Object.freeze([
  'get_transformations',
]);

/** The one read this face performs when it is mounted. */
export const READS = Object.freeze({
  transformations: 'get_transformations',
});

/**
 * No act changes anything here -- this screen is a second view of rows faces/ledger
 * already draws, not a place a delta is acted on, so this is the same empty-but-
 * declared shape req/03 §8's repair note asks every face to carry
 * (`ACTS = Object.freeze([])`, structural, not an empty row of buttons stating the
 * same thing visually).
 */
export const ACTS = Object.freeze([]);

export const SENDS = Object.freeze([...Object.values(READS)]);

export const WITHHELD = Object.freeze([]);

/**
 * The marks this face is permitted to put on a screen. Nine, not the reference's
 * eleven -- derived from what this face's binding actually draws (verified:
 * test/declaration.test.mjs's C-5 cross-check confirms every meaning graph shares
 * with faces/ledger/faces/held/faces/receipt uses the identical mark, not a second
 * drawing for one already-owned meaning). One mark is new and belongs to this face
 * alone: `structure/outside`, drawn on a row whose declared predecessor was not
 * among the rows this window actually read -- the "line that leaves the window" the
 * question above requires this face to be able to say out loud. No standing/held
 * mark (every row this face draws is, by construction, already settled -- a path
 * touched zero or one time never becomes a graph subject in the first place); no
 * seal mark (this screen states no seal claim, per UNDRAWN below).
 */
export const MARKS = Object.freeze([
  { mark: 'verdict/Admit', means: 'verdict.admit', from: 'the engine\'s recorded word for this touch was Admit' },
  { mark: 'verdict/Deny', means: 'verdict.deny', from: 'the engine\'s recorded word for this touch was Deny' },
  { mark: 'verdict/Escalate', means: 'verdict.escalate', from: 'the engine\'s recorded word for this touch was Escalate' },
  { mark: 'effect/write', means: 'effect.write', from: 'this touch was a write' },
  { mark: 'effect/delete', means: 'effect.delete', from: 'this touch was a delete' },
  { mark: 'structure/child', means: 'structure.child', from: 'this touch names the identity of the touch immediately before it in this path\'s history, and that touch is one this window also read -- both ends of the edge are visible, so the edge is drawn' },
  { mark: 'structure/hole', means: 'structure.hole', from: 'a member that was looked for and not found' },
  { mark: 'structure/outside', means: 'structure.outside', from: 'this touch names a predecessor this window did not read -- the edge would leave the window, so it is declared not drawn instead of guessed at' },
  { mark: 'structure/fold-shut', means: 'structure.fold.shut', from: 'the why/legend control, or a touch\'s own disclosure, is folded closed (SS657 retrofit: bordered controlToggle()/openableRow() controls, same marks faces/atlas already draws)' },
  { mark: 'structure/fold-open', means: 'structure.fold.open', from: 'the why/legend control, or a touch\'s own disclosure, is drawn open' },
  { mark: 'standing/reversed', means: 'standing.reversed', from: 'the reversibility chip (req/768 F-I, retrofit round 2): a later touch in this same path names this touch as its predecessor, so its own escrowed inverse was already used' },
  { mark: 'standing/none', means: 'standing.none', from: 'the reversibility chip: whether this touch can still be undone is not observable from what this window reads -- a declared hole in the membrane\'s own wire fields, not a guess' },
  { mark: 'undefined', means: 'mark.undefined', from: 'a word arrived where a known one was expected' },
]);

/** Where this face sits, and why. */
export const ORDER = Object.freeze({
  position: 4,
  reason: 'fourth, after ledger (F-1, what happened), held (F-2, what has not happened yet) and receipt (F-3, can one delta be confirmed) -- once a reader can already see one row at a time in order and confirm one of them alone, the next question this app can answer is about many rows at once: which of them are the same underlying thing touched again, which is a question about the whole read population rather than one id, so it is answered on its own screen rather than folded into the single-record receipt or the row-at-a-time ledger.',
  dock: null,
});

/** How the touches inside one path's group are ordered, and how the path groups
 * themselves are ordered -- both stated, because an order nobody chose is still an
 * order (parts/src/row-order.mjs's own header). */
export const ROWS = Object.freeze({
  draws: true,
  order: 'by-sequence',
  order_reason: 'within a path\'s own group, touches are ordered by ascending sequence number (parts/src/row-order.mjs\'s by-sequence order) -- an edge only makes sense pointing from an earlier touch to a later one, and sequence is the field the issuer itself assigns in the order it issued, not a guess this face makes from arrival order.',
  groups_order: 'most-touched-first',
  groups_order_reason: 'path groups are listed by descending touch count, so a reader\'s eye lands on the resource this window has the most evidence about before the ones it has the least about -- ties broken by the path string itself, so the order is total and repeatable.',
  reports_denominator: true,
  note: 'this screen states three denominators, not one: how many distinct paths were read at all, how many of those were touched only once (and so are not graph subjects), and how many declared edges could not be drawn because the far end left the window -- all three are counted every time this face draws, never only when they are nonzero (C-3).',
});

/**
 * Received and not drawn. Each line is a thing a reader might reasonably expect to
 * see and will not, with the reason it is missing.
 */
export const UNDRAWN = Object.freeze([
  {
    what: 'a path touched exactly once',
    why: 'this face answers "what has been touched twice or more" -- a path this window read only once is not a graph subject by the question\'s own definition. It is not silently absent: the count of such paths is stated on screen every time this face draws (C-3), never folded into the touched-twice-or-more total.',
  },
  {
    what: 'an edge whose declared predecessor this window did not read',
    why: 'req/03 F-4 requires that only an edge whose two ends were both actually read is drawn, and that a line leaving the window is stated as not drawn rather than guessed at. Drawing a line to a row that is not on screen would show a connection this window cannot actually vouch for. The count and the named predecessor id are stated instead (structure/outside).',
  },
  {
    what: 'get_stream',
    why: 'live events are not consumed by this face. This screen shows what was true when it was read, and states when that was; a path touched again a moment ago is seen on the next read, not pushed here -- the same declared gap faces/ledger and faces/held state for the same route.',
  },
  {
    what: 'the members of a transformation body',
    why: 'the crate\'s response bodies were never read (membrane/wire-fields.json states this domain contributes no fields yet), so the members named in the column map are the ones this face looked for and not the ones the server is known to send. Any member that is absent is drawn as a declared hole rather than as an empty cell.',
  },
  {
    what: 'a seal claim of any kind',
    why: 'this screen never computes or draws parts/src/seal-claim.mjs\'s claimOf() -- whether a touch can be confirmed without the issuer is faces/receipt\'s question (F-3), one delta at a time. This screen\'s seal column is a declared hole on every row, unconditionally, the same discipline faces/held holds for the identical column.',
  },
  {
    what: 'the notices this window\'s own calls produced',
    why: 'the window\'s own record of what it asked the server is handed to this face and drawn by the notice face (F-5), never by this one.',
  },
  {
    what: 'a value wider than its column, in the row itself',
    why: 'the row clips rather than wraps, the same fixed-pitch grid every other face draws with -- a value long enough to be at risk is repeated in full in the row\'s own note underneath.',
  },
]);

/** C-7 is satisfied elsewhere (faces/notice); naming which face keeps it from being
 * nobody's job. */
export const SILENT_FACE = 'notice (F-5) is the face that declares no methods at all; this one declares one and is not the control for that rule';

export const TESTS = Object.freeze([
  'test/declaration.test.mjs',
  'test/graph.test.mjs',
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
