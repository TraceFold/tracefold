// SPDX-License-Identifier: Apache-2.0
// What this face says about itself, before it draws anything.
//
// atlas is the sixth face built (F-6) and, per app req/08 SS2-2 (2026-08-24 Owner
// Owner ruling 263), the intended primary/document-view face -- the "map" a reader
// would consult before opening any one of the other five questions. There is no
// reference row for this face: `ui_proto`'s FACE.json set never had an atlas/main
// face (req/08 SS2-1 rejects the "hub" shape outright -- a main face that names
// other faces would break req/03 SS1's "faces do not import faces" line from the
// face side). Every count below is therefore this lane's own, not a deviation from
// a reference count the way faces/held/graph/receipt state theirs.
//
// The question this face answers is deliberately not faces/graph's (F-4, "what has
// been touched twice or more"). graph filters to paths touched 2+ times and draws
// per-touch chain edges; atlas draws every distinct path this window read, once
// each, as a folded summary line -- no edges, no chain resolution, just "what
// subjects exist and what was last said about each." The two screens can disagree
// about how many rows they draw for the same read population and both be right,
// because they are answering different questions over it (req/03 SS5's own
// discipline: 1 face = 1 question).
//
// req/08 SS2-3/SS2-4 describe a `default_slot`/`emits`/`handles` declaration shape
// so a shell can resolve "which face sits in the primary slot" and "which face
// answers a drill-down address of a given kind" without ever holding a face-id
// literal. Those fields are declared below as data -- this lane's honest claim
// about where this face wants to sit and what it could, in principle, hand out --
// but no shell-side resolution mechanism reads them yet (SLOT_WIRING/EMITS_REASON/
// HANDLES_REASON state the gap explicitly; wiring `shell/kernel` to consume these
// fields is not this lane's write scope, the same boundary req/100 SS7 draws around
// faces/graph's own RC-5).

export const FACE_ID = 'atlas';

export const QUESTION = 'What subjects has this window read, and what was last said about each.';

/** Every method this face is permitted to reach. One real membrane name -- the
 * same route faces/graph reads, because a subject index and a recurrence index
 * are both projections of the one list envelope the membrane already folds to
 * its end (membrane/src/pages.mjs). */
export const CONSUMES = Object.freeze([
  'get_transformations',
]);

export const READS = Object.freeze({
  transformations: 'get_transformations',
});

/** A read-only screen states its silence structurally (req/03 SS8's repair note),
 * the same shape faces/receipt and faces/graph already carry. */
export const ACTS = Object.freeze([]);

/**
 * Why that list is empty, in words a reader is shown rather than in a comment only a
 * maintainer reads.
 *
 * Owner #348 (2) puts a menu under every interactive row. A menu on a face with no
 * acts could be built two ways: draw nothing and let the platform's own menu appear,
 * or draw the menu and say what is not in it. The first is the silent refusal this
 * tree refuses everywhere else -- a reader who right-clicks a row on the ledger face
 * and gets three acts, and right-clicks a row here and gets the browser's page menu,
 * has learnt nothing about why. So the menu opens here too and states this line.
 */
export const ACTS_REASON = 'this screen only reads. It sends nothing, so there is no act to offer here.';

/**
 * What a right-click can take away from this screen, as against what it can send.
 *
 * These are deliberately not ACTS and never will be, the same distinction
 * faces/notice's own declaration draws: an act names a method of the server, and this
 * face declares none. An offer names a value already drawn on this screen and a way
 * to have it, which is the whole of what a face with nothing to send can honestly put
 * under a hand.
 *
 * One list, one consumer here (there is no act gutter on this face to be the second),
 * so the menu cannot offer something this declaration does not hold: atlas.mjs builds
 * its entries by mapping this array, never by naming an offer inline.
 *
 * `why` is what the reader is told when the thing under the pointer has nothing to
 * give -- a cell drawn as a stated gap rather than a value. That entry is drawn
 * disabled carrying this sentence, which is the same rule an unavailable act follows
 * in the gutter faces that have one.
 */
export const OFFERS = Object.freeze([
  {
    offer: 'copy',
    label: 'copy value',
    why: 'this states a gap rather than a value, so there is nothing here to take.',
  },
]);

export const SENDS = Object.freeze([...Object.values(READS)]);

export const WITHHELD = Object.freeze([]);

/**
 * Ten marks. Nine are reused where the meaning already matches an existing
 * drawing (C-5: one meaning, one mark, checked cross-face in test/declaration.test.mjs);
 * one is new (`structure/subject`) because no existing mark means "many touches,
 * folded into one line" -- `structure/child` means the opposite kind of fact (one
 * row, descending from one specific earlier row). `structure/fold-shut` and
 * `structure/fold-open` already existed in the shared sheet (added when the sheet
 * itself was authored) but this is the first face to actually draw either of them
 * as content rather than let the browser's own native <details> marker carry the
 * fold state alone.
 */
export const MARKS = Object.freeze([
  { mark: 'verdict/Admit', means: 'verdict.admit', from: 'the most recent touch this window read for this subject carried the engine\'s word Admit' },
  { mark: 'verdict/Deny', means: 'verdict.deny', from: 'the most recent touch this window read for this subject carried the engine\'s word Deny' },
  { mark: 'verdict/Escalate', means: 'verdict.escalate', from: 'the most recent touch this window read for this subject carried the engine\'s word Escalate' },
  { mark: 'effect/write', means: 'effect.write', from: 'the most recent touch this window read for this subject was a write' },
  { mark: 'effect/delete', means: 'effect.delete', from: 'the most recent touch this window read for this subject was a delete' },
  { mark: 'structure/hole', means: 'structure.hole', from: 'a member that was looked for and not found' },
  { mark: 'structure/subject', means: 'structure.subject', from: 'this line folds every touch this window read for one path into a single summary row -- a count, not a sequence' },
  { mark: 'structure/fold-shut', means: 'structure.fold.shut', from: 'this subject\'s own touch history is folded away; the row was constructed closed' },
  { mark: 'structure/fold-open', means: 'structure.fold.open', from: 'this subject\'s own touch history is drawn below; the row was constructed open (because at least one of its values needed the room)' },
  { mark: 'undefined', means: 'mark.undefined', from: 'a word arrived where a known one was expected' },
]);

/** Where this face sits, and why -- two different claims held apart on purpose. */
export const ORDER = Object.freeze({
  position: 6,
  reason: 'sixth in build order (F-6, the last face this app builds) -- this number follows req/03 SS2\'s face-index/build-order numbering, not architectural primacy. Architecturally this face is meant to sit first, as the map a reader opens before choosing one of the other five questions (app req/08 SS2-2, Owner ruling 263); that claim is the separate `default_slot` declaration below, deliberately not folded into this build-order number so the two questions ("built which turn" and "sits where by design") stay answerable independently.',
  // req/97 gap-list item 4: the same claim in words a first viewer can read. `reason`
  // is kept verbatim and is still reachable, behind the internal-reference control.
  reason_plain: 'this was the last of the six screens built, which is a fact about the order they were made in and not about which one matters most. By design this is the screen to open first: a map of what has been touched, before choosing one of the other five questions.',
  dock: null,
});

export const ROWS = Object.freeze({
  draws: true,
  order: 'by-sequence',
  order_reason: 'inside one subject\'s own folded detail, its touches are ordered by ascending sequence number (parts/src/row-order.mjs\'s by-sequence order) -- the same order faces/graph already uses for the identical reason: sequence is assigned by the issuer in the order it issued, not guessed from arrival order.',
  order_reason_plain: 'a subject\'s changes are listed in the order they were issued in, taken from the number the issuer gave each one -- not from the order they happened to arrive here.',
  groups_order: 'most-touched-first',
  groups_order_reason: 'subjects are listed by descending touch count, ties broken by the path string itself -- the same "most informative first" reasoning req/100 SS7 gives for faces/graph\'s own group order, applied here across every distinct path this window read rather than only the ones touched twice or more.',
  groups_order_reason_plain: 'the most-touched subject comes first, and subjects touched the same number of times are put in name order. Everything read is here, including a subject touched only once.',
  reports_denominator: true,
  note: 'this screen states two denominators every time it draws: how many distinct paths (subjects) this window read, and how many touches in total those subjects account for -- never only when both are nonzero (C-3).',
});

/**
 * Received and not drawn. Each line is a thing a reader might reasonably expect to
 * see and will not, with the reason it is missing.
 */
/**
 * What this screen does not draw, said twice on purpose (req/97 gap-list item 4).
 *
 * `plain` is what the screen shows: plain language, no file path, no route name, no
 * requirement number, nothing a first viewer would have to already work here to
 * decode. `why` is the accurate internal account, kept verbatim -- it is reachable
 * from the screen, labelled as an internal reference, behind its own control. The
 * surface says what is missing and the reference says which part of this codebase
 * decided it, and a viewer who wants neither reads neither.
 */
export const UNDRAWN = Object.freeze([
  {
    what: 'a chain edge between two touches of the same subject',
    plain: 'which change came after which. Another screen answers that one; this screen only counts how many times a thing was touched and shows the latest.',
    why: 'drawing which touch names which as its predecessor is faces/graph\'s exclusive question (F-4, "what has been touched twice or more"). This screen states a touch count and the most recent touch\'s own facts; it never draws structure/child or structure/outside, and never resolves a `prev` field.',
  },
  {
    what: 'a seal claim of any kind',
    plain: 'whether a record can be checked without us. Another screen answers that, one record at a time.',
    why: 'whether a touch can be confirmed without the issuer is faces/receipt\'s question (F-3), one delta at a time. This screen computes no seal claim and its row grammar carries no seal column.',
  },
  {
    what: 'get_stream',
    label: 'live updates as they happen',
    plain: 'live updates. This screen shows what was true when it was read, and says when that was.',
    why: 'live events are not consumed by this face. This screen shows what was true when it was read, and states when that read happened; a path touched again a moment ago is seen on the next read, the same declared gap every other reading face in this tree states for the same route.',
  },
  {
    what: 'the members of a transformation body beyond at/actor/effect/verdict/path/digest/sequence',
    plain: 'any field beyond the seven listed. What arrives is what was asked for, and anything missing is drawn as a stated gap rather than an empty space.',
    why: 'the crate\'s response bodies were never read (membrane/wire-fields.json states this domain contributes no fields yet), so the members this face reads are the ones it looked for, not the ones the server is known to send. An absent member is drawn as a declared hole, never a silent blank.',
  },
  {
    what: 'the notices this window\'s own calls produced',
    plain: 'this window\'s own record of what it asked for. A different screen draws that.',
    why: 'the window\'s own record of what it asked the server is drawn by the notice face (F-5), never by this one.',
  },
  {
    what: 'code coordinates, backing assays, or any other drill-down address (source/assay/receipt/transformation kinds, app req/08 SS2-4)',
    plain: 'a way to jump from a subject to the code or evidence behind it. Nothing here can honestly produce that address yet, so nothing pretends to.',
    why: 'the extraction tooling that would let this face honestly emit a `source` or `assay` address does not exist in this repo yet (app req/08 SS6-2 8-C: the one semantics manifest that exists is hand-written, not machine-generated, and this lane does not build the M0-M7 extraction pipeline req/08 SS7 describes). `EMITS`/`HANDLES` below are declared empty rather than aspirationally populated -- an emitted kind with no honest source would be exactly the "silent refusal" app req/08 SS2-4\'s own AC-S2 forbids.',
  },
  {
    what: 'a link to another face, drawn inside this face\'s own source',
    plain: 'a link to another screen. Moving between screens is the frame\'s job, not this screen\'s.',
    why: 'a main face naming another face is the "hub" shape app req/08 SS2-1 rejects by name (a main face that names other faces breaks req/03 SS1\'s "a face\'s source carries zero other-face-path imports" from the face side, the same line every face in this tree already holds at zero). If a face-switcher is built, it belongs in shell chrome (the same place req/101 places MODO/JIN), not inside any one face\'s own source -- out of this lane\'s write scope.',
  },
  {
    what: 'a graph or timeline lens over this same subject population (app req/08 SS14)',
    plain: 'a graph or timeline view of the same subjects. Only the list view is built.',
    why: 'app req/08 SS14 describes document/graph/timeline as three interaction models over one atlas node population, with document as the recommended default when the read population is layer-A-thin (SS14-3, the state this repo is actually in today). This lane implements the document model only; the lens-switching machinery is not built, and this is stated here rather than silently matched to the three-mode design.',
  },
]);

/** C-7 is satisfied elsewhere (faces/notice); naming which face keeps it from being
 * nobody's job. */
export const SILENT_FACE = 'notice (F-5) is the face that declares no methods at all; this one declares one and is not the control for that rule';

/**
 * app req/08 SS2-3/SS2-4's declared-not-coded slot/address-resolution shape,
 * stated as data this face claims about itself. `SLOT_WIRING`/`EMITS_REASON`/
 * `HANDLES_REASON` are the honest gap statements: nothing under shell/kernel
 * reads any of the three fields below in this lane.
 */
export const DEFAULT_SLOT = 'primary';

export const SLOT_WIRING = 'declared only -- no shell/kernel code in this tree resolves default_slot into an actual mounted position for any face; wiring that resolution is future, out-of-scope work (the same boundary req/100 SS7 draws around faces/graph\'s RC-5).';

export const EMITS = Object.freeze([]);

export const EMITS_REASON = 'this face could, in principle, hand out `source` and `assay` addresses per app req/08 SS2-4\'s table (both assigned to "the main face itself" there), but no tool in this repo extracts a code coordinate or an assay id for a transformation today -- declaring a kind this face cannot honestly back would be the silent refusal app req/08 AC-S2 forbids, so EMITS is declared empty rather than aspirationally populated.';

export const HANDLES = Object.freeze([]);

export const HANDLES_REASON = 'no other face in this tree emits an address of any kind today (faces/receipt and faces/graph, the two faces app req/08 SS2-4\'s table assigns the `receipt`/`transformation` kinds to, declare no `emits` field at all -- checked directly against their own declaration.mjs by this face\'s own declaration test), so there is nothing for this face to handle yet. HANDLES is declared empty rather than claiming a resolution this repo has no sender for.';

export const TESTS = Object.freeze([
  'test/declaration.test.mjs',
  'test/atlas.test.mjs',
  'test/gate.test.mjs',
]);

export const DECLARATION = Object.freeze({
  id: FACE_ID,
  question: QUESTION,
  consumes: CONSUMES,
  reads: READS,
  acts: ACTS,
  acts_reason: ACTS_REASON,
  offers: OFFERS,
  sends: SENDS,
  withheld: WITHHELD,
  marks: MARKS,
  order: ORDER,
  rows: ROWS,
  undrawn: UNDRAWN,
  silent_face: SILENT_FACE,
  default_slot: DEFAULT_SLOT,
  slot_wiring: SLOT_WIRING,
  emits: EMITS,
  emits_reason: EMITS_REASON,
  handles: HANDLES,
  handles_reason: HANDLES_REASON,
  tests: TESTS,
});
