// SPDX-License-Identifier: Apache-2.0
// The graph face: what has been touched twice or more, drawn as the second (and
// later) projection of a row faces/ledger already shows once, in order.
//
// req/03 F-4 (section 5): "同じ行の第2の投影。両端が見えている物だけ描き、外へ出る線は
// 「描いていない」と言う。絵に到達しなかった行を必ず数えて出す." Three facts follow
// directly from that sentence and are held structurally below, not by a comment
// alone:
//
//   1. A path this window read exactly once is not a graph subject -- the
//      question is "twice or more", so a once-touched path contributes nothing to
//      the picture. It is still counted (declaration.mjs UNDRAWN, notDrawn.touchedOnce).
//   2. An edge is drawn only when this window actually read both the touch it
//      starts from and the touch it ends at, for the same path. A touch that
//      names a predecessor this window did not read -- or names one that turns
//      out to belong to a different path -- is a line leaving the window, and is
//      declared not drawn (notDrawn.edgesOutside) rather than guessed at.
//   3. Every one of those counts is computed once, in `toRecord()`, and read as
//      already-decided data everywhere else in this file. This is the same split
//      req/100 section 6 states for faces/receipt (attest = facts read once, verbatim;
//      render = pure functions of what attest produced, never reaching back for a
//      second raw read) and the same trap req/101 section 6 names for MODO/JIN's
//      enabled/disabled decision: nothing under `view()` below re-derives "is this
//      path a subject" or "is this edge drawn" by a second pass over the raw wire
//      answer, a live re-fetch, or a mutation of an already-attested node. Every
//      node object toRecord() builds is frozen; `childOf` (the one field that
//      turns a plain row into a chained one) is set once, at construction, never
//      assigned onto an existing node afterward.
//
// Two independent things are computed over the same subject population and must
// not be mistaken for one another: `buildGraph()` below (this face's own decide-step,
// the kind of helper faces/receipt's digestAgreement() and faces/ledger's
// budgetFor() already are) is the authoritative source for what is actually drawn
// as a connected row on screen -- childOf set, or the structure/outside
// annotation. `parts/src/checkable.mjs`'s `prev-names-the-record-before` claim,
// reused per path group in the claims section, is a broader chain-integrity
// statement a reader can check by hand; it can legitimately read "does not hold"
// on a group whose edge was in fact drawn (a real prev-chain gap elsewhere in the
// same group does not undo an edge this window did read both ends of), and the two
// are shown side by side rather than folded into one verdict.

import {
  DECLARATION, CONSUMES, READS, ORDER, ROWS, UNDRAWN, QUESTION, FACE_ID, ACTS,
} from './declaration.mjs';
import { parts as defaultParts } from './binding.mjs';
// `order()` is decide-logic (parts/src/row-order.mjs), not drawing -- imported
// directly here the same way faces/ledger's toRecord() reads parts.src modules it
// needs for pure computation, so toRecord()/buildGraph() stay callable with no
// document and no injected `parts` seam, the same shape faces/receipt's toRecord()
// already has.
import { order as orderRows } from '../../parts/src/row-order.mjs';

export const GRAPH_MESSAGES = {
  NO_HOST: 'a face is mounted into a host element, and none was given',
  NO_PORT: 'a face is mounted with the port it is to speak through, and none was given',
  UNDECLARED: 'this face may not call a method it did not declare',
  READING: 'reading what has been touched twice or more',
  LIST_UNREAD: 'the list of transformations could not be read',
  // req/822_c7 (Owner #387/#388 冗長文字全掃): this used to read "this member was
  // looked for and was not there: <member>", built with the member's own name
  // stitched onto the end of it in normalizeNode() below. Drawn as a note line
  // (noteLines()), that put the member's name on the screen twice -- once as the
  // row label beside the sentence, once again inside the sentence itself. The
  // member is still the row label; the sentence carries no second copy of it.
  MEMBER_ABSENT: 'not in this record',
  MEMBER_NOT_SCALAR: 'this member arrived as a structure this face does not read',
  NOTE_SUMMARY: 'what this touch holds in full, and what is missing from it',
  OUTSIDE_TITLE: 'the edge into this row is not drawn',
  NO_SUBJECTS: 'nothing in what this window read was touched twice or more',
  SOURCE: 'one list of transformations',
  BAND: 'the size and shape of what this window read, stated before a word of it is read; a dash is a count the read never gave, and is never drawn as a zero',
  SUBJECTS: 'a path is drawn here once this window has read more than one touch for it -- until then there is nothing to connect',
  GROUP_HEAD: 'every touch this window read for this path, in the order the issuer numbered them; the standing beside the count is the verdict recorded on the latest of them',
  // Owner #348 (2). The words the menu is built from, in one place, because a menu
  // that spells its own reasons at each item is a menu whose reasons drift.
  MENU_SAID: 'what can be done with this touch, from the same declaration the row is drawn from',
  MENU_NO_ACT: 'no act here: this screen only reads',
  MENU_NO_VALUE: 'the pointer was not over a value, so there is nothing here to take',
  MENU_HOLE: 'this member was declared missing, so there is no value to take',
  COPY_DONE: 'copied',
  COPY_FAILED: 'this window has no clipboard, so nothing was copied',
  DISMISS: 'Escape, or a press anywhere else, puts this away',
};

const ANSWERED = 'answered';

/** The five members this face reads off each transformation item, the same member
 * names faces/ledger, faces/held and faces/receipt read off their own list items --
 * one row grammar, read the same way on every face that draws it. */
const NODE_MEMBERS = Object.freeze([
  { key: 'at', member: 'at' },
  { key: 'actor', member: 'actor' },
  { key: 'effect', member: 'effect' },
  { key: 'verdict', member: 'verdict' },
  { key: 'path', member: 'path' },
]);

const isScalar = (value) => typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean';

/** A caller that cannot reach a method the declaration does not hold. */
export function callerFor(port, allowed = CONSUMES) {
  const allow = new Set(allowed);
  const guard = (name) => {
    if (!allow.has(name)) throw new Error(`${GRAPH_MESSAGES.UNDECLARED}: ${name}`);
  };
  return {
    async fold(name, input) {
      guard(name);
      return port.fold(name, input);
    },
    async invoke(name, input) {
      guard(name);
      const method = port[name];
      if (typeof method !== 'function') return { outcome: 'absent', reason: 'no_such_method', requested: { name } };
      return method(input);
    },
  };
}

/**
 * One touch, read verbatim. Every member that is absent or arrives as something
 * other than a scalar becomes a named hole rather than a silent omission -- the
 * same conformance discipline faces/receipt's toRecord() holds for its own two
 * reads (glovrex/req/405 SS5 conditions 1/2).
 */
function normalizeNode(item, index) {
  const holes = {};
  const cells = {};
  for (const { key, member } of NODE_MEMBERS) {
    const value = item ? item[member] : undefined;
    if (value === undefined || value === null || value === '') holes[key] = GRAPH_MESSAGES.MEMBER_ABSENT;
    else if (!isScalar(value)) holes[key] = `${GRAPH_MESSAGES.MEMBER_NOT_SCALAR}: ${member}`;
    else cells[key] = String(value);
  }

  let id = null;
  if (typeof item?.id === 'string' && item.id !== '') id = item.id;
  else holes.id = GRAPH_MESSAGES.MEMBER_ABSENT;

  let digest = null;
  if (isScalar(item?.digest) && String(item.digest) !== '') digest = String(item.digest);
  else holes.digest = GRAPH_MESSAGES.MEMBER_ABSENT;

  let prev = null;
  if (item?.prev === null || item?.prev === undefined) prev = null;
  else if (typeof item.prev === 'string' && item.prev !== '') prev = item.prev;
  else holes.prev = `${GRAPH_MESSAGES.MEMBER_NOT_SCALAR}: prev`;

  const n = Number.isInteger(item?.sequence) ? item.sequence : undefined;

  return Object.freeze({
    index,
    id,
    n,
    prev,
    digest,
    // req/768 AC-7 (retrofit round 2): the wire's undo_of, read verbatim and
    // named `undoOf` -- deliberately not `childOf`, which this face's own
    // buildGraph() already spends on a different meaning (a resolved chain
    // predecessor, set from `prev`, drawn as the graph's edges). Overloading
    // `childOf` with this second, undo-specific meaning would make every
    // existing chain-edge test in this face also a silent claim about undo,
    // which it is not: a chain edge is any two touches naming each other in
    // sequence, and most are ordinary edits, never a reversal.
    ...(isScalar(item?.undo_of) && String(item.undo_of) !== '' ? { undoOf: String(item.undo_of) } : {}),
    ...cells,
    lifecycle: 'settled',
    holes: Object.freeze(holes),
  });
}

/**
 * Attest. Reads the one list envelope verbatim, groups the touches it names by
 * `path`, and states three counts every time -- the total distinct paths read, how
 * many were touched once (not a subject), and how many declared edges could not be
 * drawn because the far end left the window. No comparison a reader could disagree
 * with happens here; that is `edgesOf()`'s job below, kept apart so this function
 * stays a record of what was actually read.
 */
export function toRecord({ transformations } = {}) {
  const listRead = transformations?.outcome === ANSWERED;
  const items = listRead && Array.isArray(transformations.items) ? transformations.items : [];

  if (!listRead) {
    return Object.freeze({
      listOutcome: transformations?.outcome ?? 'absent',
      rawTransformations: transformations ?? null,
      nodes: Object.freeze([]),
      groups: Object.freeze([]),
      distinctPaths: null,
      notDrawn: Object.freeze({
        touchedOnce: Object.freeze({ count: null, paths: Object.freeze([]) }),
        edgesOutside: Object.freeze({ count: null, edges: Object.freeze([]) }),
      }),
      holes: Object.freeze({ list: `${GRAPH_MESSAGES.LIST_UNREAD}: ${transformations?.outcome ?? 'absent'}` }),
    });
  }

  const nodes = items.map((item, index) => normalizeNode(item, index));
  return buildGraph(nodes, transformations);
}

/**
 * Decide (face-local, the same kind of helper faces/receipt's digestAgreement() and
 * faces/ledger's budgetFor() are -- domain logic this one screen needs, not a
 * second implementation of a shared part). Groups the read nodes by path, orders
 * each group by sequence (parts/src/row-order.mjs), and resolves every subject
 * touch's declared predecessor against the nodes this window actually read. A
 * predecessor that is not among them -- or that is, but under a different path --
 * is never drawn as an edge; it is named and counted instead.
 */
function buildGraph(nodes, transformations) {
  const byId = new Map();
  for (const node of nodes) if (node.id) byId.set(node.id, node);

  const byPath = new Map();
  const unreadablePath = [];
  for (const node of nodes) {
    if (!node.path) { unreadablePath.push(node); continue; }
    if (!byPath.has(node.path)) byPath.set(node.path, []);
    byPath.get(node.path).push(node);
  }

  const touchedOncePaths = [];
  const subjectEntries = [];
  for (const [path, group] of byPath.entries()) {
    if (group.length === 1) touchedOncePaths.push(path);
    else subjectEntries.push([path, group]);
  }

  const edgesOutside = [];
  const unidentifiable = [];

  const groups = subjectEntries.map(([path, groupNodes]) => {
    const ordered = orderRows(groupNodes, { by: 'by-sequence' });
    for (const drop of ordered.dropped) unidentifiable.push({ path, index: drop.index, why: drop.why });

    // Resolution runs against `byId` (every node this window read, any path), not
    // against "the row before this one in the local array" -- a node's own `prev`
    // claim is a fact about that node, independent of where local re-ordering put
    // it. This is what lets the earliest touch of a path this window happens to
    // hold also be correctly flagged as an edge leaving the window, when its own
    // `prev` names something this window never read.
    const rows = ordered.rows.map((node) => {
      if (node.prev === null) return node;
      const predecessor = byId.get(node.prev);
      if (!predecessor || predecessor.id === node.id) {
        edgesOutside.push({ to: node.id, wantedPrev: node.prev, why: 'the predecessor this touch names was not among the rows this window read' });
        return node;
      }
      if (predecessor.path !== path) {
        edgesOutside.push({ to: node.id, wantedPrev: node.prev, why: 'the predecessor this touch names belongs to a different path, so this window cannot vouch for the edge' });
        return node;
      }
      return Object.freeze({ ...node, childOf: predecessor.id });
    });

    return Object.freeze({
      path,
      touchCount: groupNodes.length,
      rows: Object.freeze(rows),
      orderRequested: ordered.requested,
      orderApplied: ordered.by,
      orderSubstituted: ordered.substituted,
      orderReason: ordered.reason,
    });
  }).sort((a, b) => (b.touchCount - a.touchCount) || a.path.localeCompare(b.path));

  return Object.freeze({
    listOutcome: transformations.outcome,
    rawTransformations: transformations,
    nodes: Object.freeze(nodes),
    groups: Object.freeze(groups),
    distinctPaths: byPath.size,
    unreadablePathCount: unreadablePath.length,
    notDrawn: Object.freeze({
      touchedOnce: Object.freeze({ count: touchedOncePaths.length, paths: Object.freeze(touchedOncePaths) }),
      edgesOutside: Object.freeze({ count: edgesOutside.length, edges: Object.freeze(edgesOutside) }),
      unidentifiable: Object.freeze({ count: unidentifiable.length, entries: Object.freeze(unidentifiable) }),
    }),
    holes: Object.freeze({}),
  });
}

/**
 * Owner #348 (4): the weight hierarchy, made mechanical.
 *
 * Three steps and no fourth, chosen by what a thing IS rather than by eye -- a number
 * is bold, the word naming a number is medium, a sentence is regular. Every weight and
 * every font triple on this screen is derived from the table below; tools/gate.mjs
 * refuses a bare `font-weight` number anywhere in this face's own source, so a fourth
 * weight cannot arrive by hand the way the four corner radii once did.
 */
const WEIGHT = Object.freeze({ figure: '700', label: '500', body: '400' });

/**
 * Owner #348 (4), the line-breaking half.
 *
 * Two wrap routes, and which one a thing gets is decided by what it is made of, not by
 * where it sits. `overflow-wrap: anywhere` breaks a word at any character AND lets the
 * box shrink to its narrowest possible measure, which in a grid track declared
 * `minmax(0, ...)` is how a sentence ends up one letter to a line. It is right for a
 * path or an identity -- a string with no spaces in it must break somewhere or it
 * overflows the screen -- and wrong for every sentence. Prose takes `break-word`, which
 * only breaks a word that could not fit a line of its own, plus `text-wrap: pretty`, so
 * a paragraph does not leave a single short word stranded on its last line.
 */
const WRAP = Object.freeze({ prose: 'break-word', token: 'anywhere' });

export function createFace({ parts = defaultParts, clipboard = null } = {}) {
  const P = parts;
  const { el, style, find } = P.element;
  const T = P.tokens;

  // -- one type table, and every piece of type on this screen derived from it -----
  //
  // req/38 SS649 (#349 (3)): each of the six faces grew its own heading/aside/plain/
  // kvLine, and inside this one file the same three declarations -- family, size and
  // line height -- were written out twelve times. They are written once here. The
  // three steps are the same three the weight scale above names.

  const typeOf = (step) => ({
    'font-family': step === 'figure' ? T.mono : T.sans,
    'font-size': T.record,
    'line-height': T.recordLine,
    'font-weight': WEIGHT[step],
    ...(step === 'body' ? { 'overflow-wrap': WRAP.prose, 'text-wrap': 'pretty' } : {}),
  });

  /** A string with no spaces in it -- a path, an identity -- which may break anywhere. */
  const tokenType = () => ({
    'font-family': T.mono, 'font-size': T.time, 'line-height': T.recordLine,
    'font-weight': WEIGHT.body, 'overflow-wrap': WRAP.token,
  });

  const heading = (words, mark) => el('h2', {
    style: style({
      display: 'flex', 'align-items': 'center', gap: '8px', margin: '0 0 6px',
      color: T.ink, 'font-family': T.sans, 'font-size': T.head, 'line-height': T.headLine, 'font-weight': WEIGHT.label,
    }),
  }, [
    mark ? P.glyph(mark[0], mark[1], { size: P.minReadable, label: words }) : null,
    el('span', {}, [words]),
  ]);

  const aside = (words, role = 'aside') => el('p', {
    'data-role': role,
    style: style({ margin: '0 0 6px', color: T.attendant, ...typeOf('body') }),
  }, [words]);

  const plain = (words, role = 'line') => el('p', {
    'data-role': role,
    style: style({ margin: '0 0 4px', color: T.ink, ...typeOf('body') }),
  }, [words]);

  const peripheral = (word, node) => el('details', {
    'data-role': 'peripheral',
    style: style({ margin: '0 0 6px' }),
  }, [
    el('summary', {
      style: style({
        display: 'flex', 'align-items': 'center', 'min-height': '36px', 'box-sizing': 'border-box',
        color: T.attendant, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, cursor: 'default',
      }),
    }, [word]),
    node,
  ]);

  const section = (name, state, children, extraAttrs = {}) => el('section', {
    'data-section': name,
    'data-state': state,
    ...extraAttrs,
    style: style({ padding: '14px 0', 'border-top': `1px solid ${T.rule}`, background: T.page }),
  }, children.filter(Boolean));

  // The label track has a floor now, and that is the line-breaking fix rather than a
  // taste. `minmax(0,12rem)` lets the naming column be squeezed to nothing by the value
  // beside it, and a squeezed column carrying `overflow-wrap: anywhere` prints one
  // character per line -- which is exactly the orphan Owner #348 (4) names. A track
  // that may not go below 7rem cannot be starved, and the label reads as prose.
  const kvLine = (name, value) => el('div', {
    'data-role': 'kv-line',
    style: style({
      display: 'grid', 'grid-template-columns': 'minmax(7rem,12rem) minmax(0,1fr)', gap: '10px', padding: '3px 0',
    }),
  }, [
    el('span', { style: style({ color: T.attendant, ...typeOf('label') }) }, [name]),
    el('span', { style: style({ color: T.ink, ...typeOf('body') }) }, [value]),
  ]);

  // -- compact header + bordered one-row controls (SS657 retrofit, req/38 SS657
  // Owner #317/#318; idiom proven by faces/atlas). See faces/ledger's own copy of
  // this comment for the fuller account of the five seat-confirmed defects.

  const headerLine = (words) => el('div', {
    'data-role': 'face-header',
    style: style({ display: 'flex', 'align-items': 'baseline', gap: '10px', padding: '10px 0 6px', 'font-family': T.sans }),
  }, [
    el('span', { style: style({ 'font-weight': WEIGHT.figure, 'font-size': T.head, 'line-height': T.headLine, color: T.ink }) }, [FACE_ID]),
    el('span', { style: style({ color: T.attendant, 'font-size': T.record, 'line-height': T.recordLine }) }, [words]),
  ]);

  /**
   * A folded control, stating how much is behind the fold.
   *
   * What the second word used to be was a synonym of the first -- `omitted -- what is
   * not drawn`, `claims -- what you can check` -- a hint that told a reader nothing the
   * label had not already told them, four times across the row. It is a count now: the
   * same counted-disclosure rule this face already holds for a row (never a silent
   * affordance, req/768 F-A), applied to the controls above the rows as well. A control
   * with nothing to count states its label alone rather than a phrase restating it.
   */
  const controlToggle = (label, count, noun, body, { open = false } = {}) => el('details', {
    'data-role': 'control', 'data-control': label, 'data-open': String(Boolean(open)),
    'data-behind': count === null ? null : String(count), open: open || null,
    style: style({ border: `1px solid ${T.rule}`, 'border-radius': T.radiusControl, background: T.page }),
  }, [
    el('summary', {
      style: style({
        display: 'flex', 'align-items': 'center', gap: '6px', 'min-height': '36px', 'box-sizing': 'border-box',
        padding: `0 ${T.padX}`, color: T.ink, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'list-style': 'none',
      }),
    }, [
      P.glyph('structure', open ? 'fold-open' : 'fold-shut', { size: P.minReadable, label: open ? 'open' : 'closed' }),
      el('span', { style: style({ 'font-weight': WEIGHT.label }) }, [label]),
      count === null ? null : el('span', {
        'data-role': 'behind-count',
        style: style({ color: T.attendant, 'font-family': T.mono, 'font-weight': WEIGHT.figure }),
      }, [`${count} ${noun}`]),
    ].filter(Boolean)),
    el('div', { style: style({ padding: `0 ${T.padX} 10px` }) }, [body]),
  ]);

  const controlsRow = (children) => el('div', {
    'data-role': 'control-row',
    style: style({ display: 'flex', gap: '8px', 'flex-wrap': 'wrap', padding: '0 0 10px' }),
  }, children);

  const markTallyRows = (counts) => DECLARATION.marks.map((m) => el('div', {
    'data-mark-entry': m.mark, 'data-count': String(counts.get(m.mark) ?? 0),
    style: style({
      display: 'grid', 'grid-template-columns': 'minmax(6rem,9rem) 2.5rem minmax(0,1fr)', gap: '10px', padding: '2px 0',
    }),
  }, [
    el('span', { style: style({ color: T.ink, ...tokenType() }) }, [m.mark]),
    el('span', { style: style({ color: T.attendant, ...typeOf('figure') }) }, [String(counts.get(m.mark) ?? 0)]),
    el('span', { style: style({ color: T.attendant, ...typeOf('body') }) }, [m.from]),
  ]));

  // -- the outside-edge annotation, req/03 section 5's "外へ出る線は「描いていない」と言う" --

  // Said once, not twice. The sentence this drew before stated the fact and then
  // restated it: "...this window did not read a matching touch for this path -- the
  // predecessor this touch names was not among the rows this window read." Two clauses,
  // one fact, 178 characters. The clause that survives is the one carrying the id a
  // reader can act on; `edge.why` is the longer form and is reachable on the node
  // itself, where the reason for a thing on these screens already lives.
  function outsideAnnotation(edge) {
    return el('div', {
      'data-role': 'edge-outside',
      'data-to': edge.to,
      'data-wanted-prev': edge.wantedPrev,
      title: `${GRAPH_MESSAGES.OUTSIDE_TITLE}: ${edge.why}`,
      style: style({
        display: 'flex', 'align-items': 'flex-start', gap: '6px', padding: '4px 0',
        color: T.attendant, ...typeOf('body'),
      }),
    }, [
      P.glyph('structure', 'outside', { size: P.minReadable, label: GRAPH_MESSAGES.OUTSIDE_TITLE }),
      el('span', {}, [`not drawn: names ${edge.wantedPrev}, unread here`]),
    ]);
  }

  // -- one path group: heading, ordered rows, edge annotations ----------------

  // `in full` is a promise, and it is only true of the two members the row draws short:
  // the time (a declared cut to the time of day) and the digest (a declared cut to six
  // characters). It was on all five, so three lines of every pane carried a word that
  // said nothing -- `effect in full` beside `write` (Owner #348 (4)).
  const CUT_MEMBERS = Object.freeze(['at']);
  const paneName = (key) => (CUT_MEMBERS.includes(key) ? `${key} in full` : key);

  function noteLines(record) {
    const lines = Object.entries(record.holes ?? {}).map(([key, why]) => ({ name: key, value: why }));
    for (const { key } of NODE_MEMBERS) {
      if (record[key] !== undefined) lines.push({ name: paneName(key), value: record[key] });
    }
    if (record.n !== undefined) lines.push({ name: 'sequence', value: String(record.n) });
    // The row draws this cut to six characters (parts/src/receipt-row.mjs serialOf), so
    // the whole of it belongs somewhere a reader can reach, and this is that place. A
    // digest that is a declared hole already has its line from `holes` above.
    if (record.digest !== null && record.digest !== undefined) lines.push({ name: 'digest in full', value: record.digest });
    lines.push({ name: 'names as predecessor', value: record.prev ?? '(none)' });
    return lines;
  }

  // The same clipped-without-full defence req/03 N-4 named on faces/ledger:
  // parts/src/receipt-row.mjs clips a cell's text rather than growing the row, so a
  // row whose note is never opened can leave a truncated value with no full copy
  // anywhere on the page. A hole always opens the note (there is always something
  // to explain); a value that would not fit its own column's character budget opens
  // it too, the identical `budgetFor()` computation faces/receipt's deltaSection()
  // already performs against the same shared parts/src/receipt-row.mjs COLUMNS.
  const CHARACTER_PX = 7;
  const budgetFor = (column) => {
    const fixed = /^(\d+)px$/.exec(column.width);
    if (fixed) return Math.floor(Number(fixed[1]) / CHARACTER_PX);
    const rem = /minmax\(0,\s*([\d.]+)rem\)/.exec(column.width);
    if (rem) return Math.floor(Number(rem[1]) * 2);
    return 40;
  };

  /**
   * Owner #348 (4), the densest instance of it on this face.
   *
   * A path group is a box whose head IS the path. Every row inside that box then drew
   * the same string again in its own widest column -- four copies of `/work/report.md`
   * in one box of three touches, and the longest string on the screen repeated the most
   * times. A column whose value is identical on every row of a container that already
   * states it is not a column, it is an echo.
   *
   * So the rows inside a group scan on the shared five columns less `path`, derived from
   * parts/src/receipt-row.mjs's own SCAN_COLUMNS rather than listed again here. The
   * value has not left the screen: it is in the box head above the row, it is in the
   * pane when the row is chosen, and it is what the row's own menu copies.
   *
   * What takes the width it leaves behind is `fingerprint`, and this is the second half
   * of the same change rather than a separate wish. Measured in the photograph: with the
   * echo simply deleted, the drawn tracks ended around 310px of a 780px row and the chip
   * and the field count sat alone at the far right, so a row read as two clusters with a
   * canyon between them. `fingerprint` is the column SCAN_COLUMNS had to drop when eight
   * tracks starved `path` to zero pixels, it is a value that differs on every row where
   * `path` was identical on all of them, and it is the one member this face read on every
   * touch and then drew nowhere at all -- a digest normalised in `normalizeNode` and
   * dropped. It is a declared cut, so the whole of it goes into the pane in the same
   * change (`noteLines` below), and nothing is shown short with the full form missing.
   */
  const GROUP_COLUMNS = Object.freeze([
    ...P.scanColumns.filter((column) => column.key !== 'path'),
    P.columns.find((column) => column.key === 'fingerprint'),
  ]);
  const DRAWN_MEMBERS = Object.freeze(NODE_MEMBERS.filter(
    ({ key }) => GROUP_COLUMNS.some((column) => column.key === key),
  ));
  const BUDGETS = Object.fromEntries(GROUP_COLUMNS.map((column) => [column.key, budgetFor(column)]));

  function needsOpen(record) {
    const holeKeys = Object.keys(record.holes ?? {});
    if (holeKeys.length > 0) return true;
    // Against the drawn form, not the arrived value (req/97 gap-list item gap 1): the `at`
    // column draws a declared cut, so an ISO-8601 timestamp is not a clip here. And
    // against the columns this face actually draws -- a member that is only ever in the
    // pane cannot be clipped by a row cell that does not exist.
    return DRAWN_MEMBERS.some(({ key }) => P.drawnTextFor(key, record[key]).length > (BUDGETS[key] ?? 40));
  }

  /**
   * req/768 AC-7 (retrofit round 2): a path's own touches are exactly the
   * sibling set to check for a reversal -- but reversalOf()'s contract reads
   * `entry.childOf`, which on a graph node means "resolved chain predecessor"
   * (buildGraph's own concern, see normalizeNode's comment on `undoOf`), not
   * "this touch is an undo of the target". `asUndoSibling` re-shapes each
   * sibling for reversalOf's purposes only -- a plain {id, childOf: undoOf}
   * pair, never written back onto the real node -- so this face's own chain-
   * edge `childOf` is never read for a meaning it does not carry. AC-4 does
   * not apply to this face -- declaration.mjs's CONSUMES is transformations/
   * subscribe only, no commit/cancel/undo route, so there is no act a gutter
   * could ever hold (the same honest absence faces/receipt states, for the
   * same reason).
   */
  const asUndoSibling = (r) => ({ id: r.id, childOf: r.undoOf ?? null });

  /**
   * Every touch's reversal fact, decided once for the whole paint and read from a
   * map everywhere afterwards.
   *
   * It was decided inside groupSection() before this round, which was correct while
   * the chip was the only reader of it. The stat band is a second reader -- it states
   * how many of this screen's touches have already been reversed -- and two readers
   * each running the same scan is the shape a number drifts in: the band could state
   * a count no chip on the page agrees with, and nothing would notice. One pass, one
   * answer, keyed by the frozen node itself so a row and its fact cannot be paired by
   * an id that turns out to be shared.
   */
  function reversalsOf(record) {
    const facts = new Map();
    for (const group of record.groups ?? []) {
      const siblings = group.rows.map(asUndoSibling);
      for (const row of group.rows) {
        facts.set(row, P.reversalOf(row, siblings.filter((sibling) => sibling.id !== row.id)));
      }
    }
    return facts;
  }

  /**
   * A path's own standing, drawn on the head of its box: the verdict the engine
   * recorded on the most recent touch this window read for that path.
   *
   * It is the one fact about a whole group that is not a count, and it is what lets
   * four boxes on one screen tell themselves apart at arm's length -- the badge takes
   * the standing's own bed, so a path whose latest touch was denied is a different
   * area of colour from one whose latest touch was admitted, not a different 14px
   * stroke. A group whose latest touch arrived with no verdict has no standing to
   * state and is drawn with no pill at all, rather than with an invented one.
   */
  function standingPill(group) {
    const latest = group.rows[group.rows.length - 1];
    return latest?.verdict === undefined ? null : P.badge(latest.verdict);
  }

  // -- the menu a second mouse button opens (Owner #348 (2)) ---------------------

  /**
   * What a touch offers, taken from the declaration and not from a second list.
   *
   * `ACTS` is empty on this face and that is a fact about the screen, not an oversight:
   * declaration.mjs states one read and no act route, so there is no act a gutter could
   * draw and none a menu may invent. The map below is still written against `ACTS`
   * rather than around it -- the day this face is given an act, the menu grows it with
   * no second declaration to keep in step -- and the empty case says so out loud
   * instead of opening onto nothing.
   */
  function actItems() {
    return ACTS.map((spec) => ({
      id: `act-${spec.act}`,
      words: spec.label ?? spec.act,
      sends: false,
      why: GRAPH_MESSAGES.MENU_NO_ACT,
    }));
  }

  /**
   * The value under the pointer, read off the record rather than off the drawn cell.
   *
   * This matters for `at`: the cell draws a declared cut (the time of day) and the
   * timestamp it was cut from is the thing worth taking. Copying what was drawn would
   * hand back the abbreviation. A member with a declared hole has no value at all and
   * says so rather than copying the word "undefined".
   */
  function copyItemsFor(record, cell) {
    const member = NODE_MEMBERS.find(({ key }) => key === cell);
    const hole = member ? record.holes?.[member.key] : null;
    const value = member && !hole ? record[member.key] : null;
    return [
      {
        id: 'copy-value',
        words: member ? `copy ${member.key}` : 'copy value',
        value,
        sends: typeof value === 'string' && value !== '',
        why: hole ? GRAPH_MESSAGES.MENU_HOLE : GRAPH_MESSAGES.MENU_NO_VALUE,
      },
      {
        id: 'copy-identity', words: 'copy identity', value: record.id, sends: true, why: null,
      },
    ];
  }

  function menuItemsFor(record, cell) {
    return [...actItems(), ...copyItemsFor(record, cell)];
  }

  /** Whether the menu has to say out loud that there is no act to offer. */
  const statesNoAct = () => actItems().length === 0;

  /**
   * The menu, in flow, under the touch it belongs to.
   *
   * It is not floated over the list, and that is a rule this application already paid
   * for: req/03 N-1 was an out-of-flow element drawn on top of a row with nothing
   * underneath it, and tools/gate.mjs still refuses `position` in this face's source for
   * that reason. So the menu takes room rather than borrowing it -- the rows below move
   * down while it is open and move back when it is dismissed, which no reader has ever
   * had to learn.
   *
   * Three properties it holds structurally rather than by care, because all three are
   * ways a hand-rolled menu goes wrong:
   *   1. It cannot be left behind by a repaint: it is drawn from `state.menu` on every
   *      paint like everything else, so a paint that carries no menu draws no menu.
   *   2. Two of them cannot stack: `state.menu` names one touch, and a second press
   *      replaces it.
   *   3. It offers nothing the row does not send. An unavailable item is drawn disabled
   *      with its reason, the same rule the act gutter already follows.
   */
  function menuFor(record, menu) {
    const items = menuItemsFor(record, menu.cell);
    return el('div', {
      'data-part': 'face-menu', 'data-face-menu': record.id, 'data-count': String(items.length),
      'data-cell': menu.cell ?? null, 'data-outcome': menu.outcome ?? null,
      title: GRAPH_MESSAGES.MENU_SAID,
      style: style({
        display: 'flex', 'flex-wrap': 'wrap', 'align-items': 'center', gap: '6px',
        'box-sizing': 'border-box', padding: `6px ${T.padX}`,
        // The same 3px accent edge the chosen row carries, so a menu reads as hanging
        // off the touch above it rather than as a band of its own. Not the accent bed:
        // a chosen row already takes that, and a menu drawn on it under a chosen row
        // would be one continuous area of colour with no edge between the two.
        'border-left': `3px solid ${T.act}`,
        'border-bottom': `1px solid ${T.rule}`, background: T.page,
      }),
    }, [
      el('span', { 'data-role': 'menu-head', style: style({ color: T.attendant, ...typeOf('label') }) }, [record.id]),
      ...items.map((item) => el('button', {
        type: 'button',
        'data-menu-item': item.id,
        'data-target': record.id,
        'data-sends': String(item.sends),
        // What the face decided to offer, carried on the control that offers it, so the
        // press reads back exactly what was drawn rather than looking the value up a
        // second time down a second path.
        'data-value': item.sends ? String(item.value) : null,
        // The shell's own copy control does not pretend either: it states whether the
        // clipboard took the value rather than looking identical both ways.
        'data-copied': menu.outcome === 'copied' && menu.item === item.id ? 'true' : null,
        'data-copy-failed': menu.outcome === 'failed' && menu.item === item.id ? 'true' : null,
        disabled: item.sends ? null : true,
        title: item.sends ? null : item.why,
        // The one motion route, as the class parts/src/surface.mjs declares it for
        // exactly this case -- something a face draws that the shared rule set does not
        // name. A duration written here would be the fifth duration in a system that
        // declared two, which is what Owner #348 (1) closed.
        class: 'gx-move',
        style: style({
          font: 'inherit', 'font-weight': WEIGHT.label, display: 'inline-flex', 'align-items': 'center',
          padding: '8px 10px', 'min-height': '36px', 'box-sizing': 'border-box',
          cursor: item.sends ? 'pointer' : 'not-allowed',
          color: item.sends ? T.act : T.attendant,
          border: `1px solid ${item.sends ? T.act : T.rule}`, 'border-radius': T.radiusControl, background: T.page,
        }),
      }, [
        menu.item === item.id && menu.outcome
          ? `${item.words} -- ${menu.outcome === 'copied' ? GRAPH_MESSAGES.COPY_DONE : GRAPH_MESSAGES.COPY_FAILED}`
          : item.words,
      ])),
      // A menu that simply had no act row in it would be indistinguishable from a menu
      // whose acts failed to draw. It says which, once, in the place a reader is
      // already looking.
      statesNoAct() ? el('span', {
        'data-role': 'menu-empty', style: style({ color: T.attendant, ...typeOf('body') }),
      }, [GRAPH_MESSAGES.MENU_NO_ACT]) : null,
    ].filter(Boolean));
  }

  function groupSection(group, edgesOutsideByTo, selected, reversals, menu) {
    const rowNodes = group.rows.flatMap((record) => {
      const outside = edgesOutsideByTo.get(record.id);
      // Owner directive #335 (3): a touch's own facts are stored in this screen's one
      // pane, not drawn underneath the touch. needsOpen() below is still computed --
      // it says whether the grid can hold this touch's values -- but it no longer
      // pushes every touch after it down the page.
      return [
        outside ? outsideAnnotation(outside) : null,
        el('div', {
          'data-role': 'row-block',
          'data-open-because': needsOpen(record) ? 'tight' : null,
          style: style({ display: 'block' }),
        }, [
          P.selectableRow(record, {
            reversal: reversals.get(record) ?? null,
            fields: noteLines(record).length,
            selected: selected === record.id,
            columns: GROUP_COLUMNS,
          }),
          menu && menu.row === record.id ? menuFor(record, menu) : null,
        ].filter(Boolean)),
      ];
    });
    // Owner #340: a group of records is an object with an edge round it, and the edge
    // states what the group is, how many are in it and what condition it is in. What
    // stood here before was a heading, a rule, and then rows -- so where one path
    // ended and the next began was carried by spacing alone, which is an encoding with
    // no legend. The heading's two facts (the path, and `N touches`) are not repeated
    // underneath the head: they are the head.
    return el('div', {
      'data-section': 'path-group',
      'data-state': 'drawn',
      'data-path': group.path,
      'data-touch-count': String(group.touchCount),
    }, [
      P.box({
        name: group.path,
        count: group.touchCount,
        noun: 'touches',
        pill: standingPill(group),
        said: GRAPH_MESSAGES.GROUP_HEAD,
        children: [
          group.orderSubstituted ? aside(`${group.orderReason}`, 'order-substituted') : null,
          ...rowNodes,
        ],
      }),
    ]);
  }

  // -- claims: parts/src/checkable.mjs's chain-integrity claim, per group -----

  function claimsSection(record) {
    if (record.groups.length === 0) {
      return section('claims', 'empty', [
        heading('what you can check here yourself'),
        aside('there is no subject population yet, so there is nothing to check.', 'claims-empty'),
      ]);
    }
    const perGroup = record.groups.map((group) => {
      const claims = P.checkable(group.rows, []);
      const chain = claims.find((c) => c.id === 'prev-names-the-record-before');
      return { path: group.path, chain };
    });
    return section('claims', 'stated', [
      heading('what you can check here yourself'),
      aside('for each path drawn above, parts/src/checkable.mjs\'s own chain claim walks the touches this window read for that path, in sequence, and states whether each one names the identity of the one before it. This is a broader statement than "the edge was drawn": a chain can still name gaps this window never asked about, even where the one edge drawn above is genuine.', 'claims-why'),
      el('div', { 'data-role': 'chain-claims' }, perGroup.map(({ path, chain }) => el('div', {
        'data-claim-path': path, 'data-holds': String(Boolean(chain?.holds)),
        style: style({
          display: 'grid', 'grid-template-columns': 'minmax(0,10rem) minmax(0,1fr)', gap: '10px', padding: '4px 0',
          'border-bottom': `1px solid ${T.rule}`, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
        }),
      }, [
        el('span', { style: style({ color: chain?.holds ? T.ink : T.deny, 'font-family': T.mono, 'overflow-wrap': 'anywhere' }) }, [path]),
        el('div', {}, [
          el('div', { style: style({ color: T.ink }) }, [chain?.holds ? 'holds' : 'does not hold']),
          el('div', { style: style({ color: T.attendant, 'overflow-wrap': 'anywhere' }) }, [chain?.detail ?? '']),
        ]),
      ]))),
    ]);
  }

  // -- not drawn: the three counted denominators -------------------------------

  function notDrawnSection(record) {
    const nd = record.notDrawn;
    const touchedOnce = nd.touchedOnce;
    const edgesOutsideCount = nd.edgesOutside;
    const unidentifiable = nd.unidentifiable ?? { count: 0, entries: [] };
    return section('not-drawn', 'stated', [
      heading('omitted'),
      kvLine('paths touched exactly once', touchedOnce.count === null ? '(not counted: the list was not read)' : `${touchedOnce.count} -- ${touchedOnce.count === 0 ? 'none' : touchedOnce.paths.slice(0, 12).join(', ') + (touchedOnce.paths.length > 12 ? ', ...' : '')}`),
      kvLine('declared edges not drawn (predecessor outside this window)', edgesOutsideCount.count === null ? '(not counted: the list was not read)' : String(edgesOutsideCount.count)),
      kvLine('touches this face could not place in a chain (no usable identity)', String(unidentifiable.count)),
      el('div', { 'data-role': 'omissions' }, UNDRAWN.map((entry) => el('div', {
        'data-omission': entry.what,
        style: style({
          display: 'grid', 'grid-template-columns': 'minmax(0,14rem) minmax(0,1fr)', gap: '10px', padding: '3px 0',
          'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
        }),
      }, [
        el('span', { style: style({ color: T.attendant }) }, [entry.what]),
        el('span', { style: style({ color: T.ink }) }, [entry.why]),
      ]))),
    ]);
  }

  // -- the whole screen --------------------------------------------------------

  function frame(children) {
    return el('div', {
      'data-face': FACE_ID,
      'data-question': QUESTION,
      style: style({
        display: 'block', background: T.page, color: T.ink, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
        padding: `12px ${T.padX} 40px`,
      }),
    }, children.filter(Boolean));
  }

  function waitingView() {
    return frame([plain(GRAPH_MESSAGES.READING, 'reading')]);
  }

  function listOutcomeLines(record) {
    const raw = record.rawTransformations;
    const lines = [plain(`outcome: ${record.listOutcome}`, 'outcome')];
    if (record.listOutcome === 'refused' && raw) {
      lines.push(plain(`${raw.problem?.title ?? ''}: ${raw.problem?.detail ?? ''}`, 'refusal'));
      lines.push(plain(`code: ${raw.gx_code ?? raw.problem?.gx_code ?? 'none'}`, 'code'));
    }
    if (record.listOutcome === 'failed' && raw) lines.push(plain(`${raw.reason}: ${raw.detail ?? ''}`, 'failure'));
    if (record.listOutcome === 'absent' && raw) lines.push(plain(`${raw.reason}: ${JSON.stringify(raw.requested ?? null)}`, 'absence'));
    return lines;
  }

  /**
   * The legend, after Owner #348 (4) was measured against it.
   *
   * It held three prose lines explaining `structure/child`, `structure/outside` and the
   * undo chip -- and the table directly above them already prints each of those marks
   * beside its own count and its own `from` sentence, taken from the declaration. The
   * prose was a second, longer copy of what the table states, one scroll further down
   * the same fold: 780 characters saying again what 3 rows had just said. What is left
   * here is what the table does NOT carry -- the shape of the time column, what the pane
   * holds, and the order the boxes are in.
   *
   * The undrawn census is gone from here for the same reason: the `omitted` control
   * beside this one states all seven entries with their reasons, and this drew all seven
   * again. Two copies of a census is worse than none, because a reader who finds one
   * cannot tell whether the other is a different list.
   */
  function legendBody(counts) {
    return el('div', { 'data-role': 'legend' }, [
      el('div', { 'data-role': 'legend-marks' }, markTallyRows(counts)),
      el('div', { 'data-role': 'legend-prose' }, [
        kvLine('the time column', P.rowMessages.AT_FORM),
        kvLine('the pane', GRAPH_MESSAGES.NOTE_SUMMARY),
        kvLine('box order', ROWS.groups_order_reason),
        kvLine('a second press', `${GRAPH_MESSAGES.MENU_SAID}. ${GRAPH_MESSAGES.DISMISS}.`),
      ]),
    ]);
  }

  function headerWords(record) {
    return `${record.groups.length} of ${record.distinctPaths ?? '?'} paths touched twice or more`;
  }

  /**
   * The size and shape of this screen's population, stated before a word of it is
   * read (Owner #340: understandable at a glance).
   *
   * Four figures, all four of them counts this face had already computed and was
   * spending on prose or hiding behind a click. `linked` in particular is the one
   * fact this screen computes that no other face in this tree has -- a touch whose
   * named predecessor this window also read -- and until this round it was drawn
   * only as a 14px elbow at the head of a row, so a reader could see every link that
   * could *not* be drawn (the annotation says so in a sentence) and could not see
   * that any had been. The pair `linked` / `undrawn` puts both on the screen as
   * numbers, in that order, which is the way round this face's own question asks for.
   *
   * Four and not five, measured rather than chosen: at the narrow width this face is
   * photographed at, five equal columns leave a noun about seventy pixels and the
   * photograph came back reading `NOT DR...`, then `UNDRA...` after the noun was cut
   * to one word. Four columns clear it with room to spare. What was dropped is what
   * the header line one row above already states -- `N of M paths touched twice or
   * more` is the path count and the repeated-path count, and a band that repeated
   * them would have spent half its width saying the sentence above it again.
   *
   * Three rules the band's own module enforces rather than requests: a figure with no
   * noun cannot be built; 0 is drawn because "none of these" is a measurement; and a
   * count this read never gave is null, which draws a dash. The last is why every
   * figure here is guarded on `listOutcome` rather than read off an array that is
   * empty for two entirely different reasons -- a refused read has no touches, and no
   * zero to state about them either.
   */
  function statBandFor(record, reversals) {
    const answered = record.listOutcome === ANSWERED;
    const reversed = answered
      ? [...reversals.values()].filter((fact) => fact.state === 'reversed').length
      : null;
    const linked = answered
      ? record.groups.reduce((n, group) => n + group.rows.filter((row) => typeof row.childOf === 'string').length, 0)
      : null;
    return P.statBand([
      {
        noun: 'touches',
        count: answered ? record.nodes.length : null,
        said: 'every transformation this window read, including the ones on paths this screen does not draw',
      },
      {
        noun: 'linked',
        count: linked,
        mark: P.glyph('structure', 'child', { size: P.minReadable, label: 'linked' }),
        said: 'touches whose named predecessor this window also read, on the same path, so both ends of the link are on this screen and it is drawn',
      },
      {
        noun: 'reversed',
        count: reversed,
        mark: P.glyph('standing', 'reversed', { size: P.minReadable, label: 'reversed' }),
        // The ink a standing owns, asked for rather than chosen: this face may not
        // spell a colour, so it names the mark and places whatever the standing table
        // gives back. standing/reversed has no hue of its own in that table today and
        // this reads as the ordinary figure ink -- which is the honest drawing of a
        // standing that has not been given one, and picks the hue up on the day it is.
        tone: P.inkFor(P.markOf('standing', 'reversed')),
        said: 'touches a later touch on the same path names as its own predecessor, so the inverse held for them was already used',
      },
      {
        noun: 'undrawn',
        count: record.notDrawn.edgesOutside.count,
        mark: P.glyph('structure', 'outside', { size: P.minReadable, label: 'undrawn' }),
        said: 'links this window was told about and could not draw, because the touch at the far end of them was never read here',
      },
    ], { said: GRAPH_MESSAGES.BAND });
  }

  /**
   * The one box a screen with no path group to draw still has.
   *
   * A group that is empty and a group that was never read are different facts and are
   * drawn differently: an answered read of nothing states a measured `0`, and a read
   * that did not answer states a dash. Neither vanishes -- the border is what says
   * "this is where the paths would be", and a screen that drops it says nothing at all
   * about a population it is entirely silent on.
   */
  function fallbackBox(state, count, children) {
    return el('div', { 'data-section': 'list', 'data-state': state }, [
      P.box({
        name: 'touched twice or more',
        count,
        noun: 'paths',
        said: GRAPH_MESSAGES.SUBJECTS,
        // A box holds its contents to its own edge -- rows bring their own grid
        // padding and lines of type do not, so these are inset here rather than
        // left flush against the border, which is what the photograph showed.
        children: [el('div', { style: style({ padding: `8px ${T.padX} 4px` }) }, children)],
      }),
    ]);
  }

  /**
   * The one pane this screen stores a touch's detail in (directive #335, 3).
   *
   * It used to open with the group's own path and then draw `path in full` underneath
   * it with the identical string, because noteLines() already reads that member off the
   * touch -- two lines, one value, at the top of every pane. The touch's own line is the
   * one that survives: it is the value this face read on this touch rather than the key
   * its group was gathered under, and the two cannot disagree (the group IS that value).
   * The field count on the row is now exactly the number of lines the pane adds.
   */
  function paneFor(selected, record) {
    for (const group of record.groups ?? []) {
      const found = group.rows.find((r) => r.id === selected);
      if (found) return P.detailPane({ subject: found.id, lines: noteLines(found) });
    }
    return P.detailPane({});
  }

  function view(state) {
    // The figure the footer prints is this call's own, taken around the work view()
    // was already doing. It is not an estimate and it is not a build-time constant:
    // a number printed on every screenshot cannot go stale the way a number in a
    // document does, which is the whole reason the strip exists.
    const startedAt = performance.now();
    const record = toRecord(state);
    const selected = state.selected ?? null;
    // Which disclosures are open is this window's own answer and is carried here, not
    // left to the elements. Before this round nothing on this face could cause a second
    // paint, so a native <details> kept its own state safely -- the moment a press
    // repaints, an element's state and this window's state are two answers to one
    // question and they drift (req/103 finding 2, on the faces that already had a
    // listener). One answer, and it is this one.
    const opened = Array.isArray(state.opened) ? state.opened : [];
    const menu = state.menu ?? null;
    const reversals = reversalsOf(record);
    const edgesOutsideByTo = new Map(record.notDrawn.edgesOutside.edges?.map?.((e) => [e.to, e]) ?? []);
    const content = [
      record.listOutcome !== ANSWERED
        ? fallbackBox('unread', null, [aside(GRAPH_MESSAGES.LIST_UNREAD, 'unread'), ...listOutcomeLines(record)])
        : null,
      record.listOutcome === ANSWERED && record.groups.length === 0
        ? fallbackBox('no-subjects', 0, [aside(GRAPH_MESSAGES.NO_SUBJECTS, 'no-subjects')])
        : null,
      ...(record.listOutcome === ANSWERED
        ? record.groups.map((group) => groupSection(group, edgesOutsideByTo, selected, reversals, menu))
        : []),
    ].filter(Boolean);
    const claims = record.listOutcome === ANSWERED ? claimsSection(record) : null;
    const omitted = notDrawnSection(record);
    content.push(...[claims, omitted].filter(Boolean));
    const band = statBandFor(record, reversals);
    // The band is counted with the rest of the screen and not exempted from it. Two
    // of its five figures carry a mark, and a legend tally that skipped them would
    // report zero of a mark a reader can see -- the one thing a counted legend exists
    // to make impossible.
    const counts = new Map();
    for (const node of [band, ...content]) {
      for (const marked of find(node, (n) => n.attrs && 'data-mark' in n.attrs)) {
        counts.set(marked.attrs['data-mark'], (counts.get(marked.attrs['data-mark']) ?? 0) + 1);
      }
    }
    // Owner directive #335 (1): the claims and the omitted census are behind a click,
    // in the same compact row as why and legend. The path groups are the data.
    const data = content.filter((node) => node !== claims && node !== omitted);
    const body = [
      headerLine(record.listOutcome === ANSWERED ? headerWords(record) : 'not yet read'),
      band,
      controlsRow([
        controlToggle('why', null, null, aside(ORDER.reason, 'why-first'), { open: opened.includes('why') }),
        controlToggle('legend', DECLARATION.marks.length, 'marks', legendBody(counts), { open: opened.includes('legend') }),
        claims ? controlToggle('claims', record.groups.length, 'paths', claims, { open: opened.includes('claims') }) : null,
        omitted ? controlToggle('omitted', UNDRAWN.length, 'reasons', omitted, { open: opened.includes('omitted') }) : null,
      ].filter(Boolean)),
      P.detailFrame(el('div', { 'data-role': 'groups' }, data), paneFor(selected, record)),
    ];
    return frame([...body, P.runtimeFooter({
      // Rounded to a thousandth of a millisecond, which is a hundred times finer than
      // the figure the strip prints. Rounding coarser than what is drawn would let the
      // attribute and the printed word disagree; not rounding at all writes a float's
      // full tail into a page that is regenerated and read by eye.
      renderMs: Math.round((performance.now() - startedAt) * 1000) / 1000,
      // What this face read, in its own words. Named only when the read actually
      // answered: a strip that said "read one list of transformations" over a refused
      // read would be claiming a source it never reached.
      source: record.listOutcome === ANSWERED ? GRAPH_MESSAGES.SOURCE : null,
    })]);
  }

  // -- reading -------------------------------------------------------------------

  async function read(port) {
    const caller = callerFor(port);
    const transformations = await caller.fold(READS.transformations);
    return { transformations };
  }

  /**
   * The clipboard, asked for at the moment it is used.
   *
   * A face reaches no network and owns no window, but it may hand a value to the one a
   * reader is sitting in front of. Read at call time rather than captured at import
   * time, because a page can be given a clipboard after a module has loaded, and
   * overridable through createFace() so a test can watch a copy succeed and a copy fail
   * without either being a claim about a real browser.
   */
  function clipboardNow() {
    return clipboard ?? globalThis.navigator?.clipboard ?? null;
  }

  function copyValue(text) {
    const clip = clipboardNow();
    if (!clip || typeof clip.writeText !== 'function') return Promise.resolve('failed');
    return Promise.resolve(clip.writeText(text)).then(() => 'copied', () => 'failed');
  }

  // -- mount ---------------------------------------------------------------------

  function mount(host, port, notices = []) {
    if (!host || typeof host.appendChild !== 'function') throw new TypeError(GRAPH_MESSAGES.NO_HOST);
    if (!port || typeof port !== 'object') throw new TypeError(GRAPH_MESSAGES.NO_PORT);
    void notices;

    const doc = host.ownerDocument ?? globalThis.document;
    if (typeof doc.getElementById === 'function' && doc.body) P.installSheet(doc, P.element.render);
    // Owner directive #335 (2) and (4): the slim scrollbar and the figure/label type
    // scale are rules, not inline styles, so they arrive the same way the glyph
    // sprite does -- once per document, from the one module that owns them
    // (parts/src/surface.mjs). Same guard, same reason: the structural stand-in in
    // test/dom-stand-in.mjs is not a document and proves nothing about drawing.
    if (typeof doc.getElementById === 'function' && doc.body) P.installSurface(doc, P.element.render);
    let live = true;
    let state = null;
    let takeFocus = false;
    let queue = Promise.resolve();

    const clear = () => { while (host.firstChild) host.removeChild(host.firstChild); };
    const paint = (tree) => {
      if (!live) return;
      clear();
      host.appendChild(P.element.render(doc, tree));
      // req/97 gap-list item 1. The rows are drawn shut; the renderer, not this
      // face's character arithmetic, decides whether any of them is actually
      // cutting a value off, and re-opens only those. Guarded because the
      // structural stand-in in test/dom-stand-in.mjs has no layout to measure --
      // there it is a no-op, which is the honest answer for a host that draws
      // nothing (parts/src/receipt-row.mjs openMeasuredClips states the rest).
      if (typeof host.querySelectorAll === 'function') P.openMeasuredClips(host);
      // A menu that opens under the pointer and leaves the keyboard behind is a menu
      // half the people looking at this screen cannot use, and it is also how Escape
      // stops working: a key press is delivered to what has focus, and right-clicking
      // a button does not focus it. So the first thing a hand can press takes focus
      // when a menu opens -- once, on opening, never again on the repaint a copy
      // causes, which would otherwise pull focus off the item just pressed.
      if (takeFocus && typeof host.querySelector === 'function') {
        const first = host.querySelector('[data-menu-item]:not([disabled])');
        if (first && typeof first.focus === 'function') first.focus();
      }
      takeFocus = false;
    };

    /** Put the menu away, if one is open. Answers whether it had to. */
    const dismiss = () => {
      if (!state?.menu) return false;
      state = { ...state, menu: null };
      paint(view(state));
      return true;
    };

    const inMenu = (hit) => Boolean(hit && typeof hit.closest === 'function' && hit.closest('[data-face-menu]'));

    const onClick = (event) => {
      const hit = event?.target;
      const enclosing = (selector) => (hit && typeof hit.closest === 'function' ? hit.closest(selector) : null);
      if (!state) return;

      const item = enclosing('[data-menu-item]');
      if (item && typeof item.getAttribute === 'function') {
        const which = item.getAttribute('data-menu-item');
        const value = item.getAttribute('data-value');
        if (which && state.menu && typeof value === 'string') {
          const asked = state.menu;
          queue = queue.then(() => copyValue(value)).then((outcome) => {
            // Only if the same menu is still the one open. A reader who put it away
            // while the clipboard was thinking gets no menu re-appearing at them.
            if (!live || state?.menu !== asked) return;
            state = { ...state, menu: { ...asked, item: which, outcome } };
            paint(view(state));
          });
        }
        if (typeof event.preventDefault === 'function') event.preventDefault();
        return;
      }

      // A press that lands inside the menu but on none of its controls -- its own
      // padding, its head, the sentence about there being no act -- is not a press on
      // anything and is certainly not a request to put the menu away.
      if (inMenu(hit)) return;

      // A press anywhere that is not inside the open menu puts it away -- in the same
      // paint as whatever else that press does, not in a paint of its own. It does not
      // swallow the press: choosing a touch while a menu is open both puts the menu away
      // and chooses the touch, which is what a hand aiming at that row meant.
      let next = state.menu ? { ...state, menu: null } : state;

      // A disclosure the reader opened or shut. The element's own toggle is stopped and
      // the answer is kept here instead, because a repaint would otherwise put every
      // open fold back to shut and the two would disagree.
      const pressed = enclosing('summary');
      const holder = pressed && pressed.parentNode ? pressed.parentNode : null;
      const control = holder && typeof holder.getAttribute === 'function' ? holder.getAttribute('data-control') : null;
      if (control) {
        const already = Array.isArray(next.opened) ? next.opened : [];
        next = {
          ...next,
          opened: already.includes(control) ? already.filter((key) => key !== control) : [...already, control],
        };
        if (typeof event.preventDefault === 'function') event.preventDefault();
      } else {
        // Owner directive #335 (3): choosing a touch names it as the subject of the one
        // detail pane on this screen. It changes nothing on the server -- it is this
        // window deciding which touch it is describing -- so it repaints from the state
        // already in hand and sends nothing.
        //
        // Until this round no press did anything at all here: every row was drawn as a
        // real button carrying `aria-pressed` and a field count, `view()` read
        // `state.selected`, and no code path ever produced a state carrying one, so the
        // pane read "no row is open" for the whole life of a live window.
        const chosen = enclosing('[data-select-row]');
        const id = chosen && typeof chosen.getAttribute === 'function' ? chosen.getAttribute('data-select-row') : null;
        if (id) next = { ...next, selected: next.selected === id ? null : id };
      }

      if (next === state) return;
      state = next;
      paint(view(state));
    };

    /**
     * Owner #348 (2). The second mouse button, on the same row the first one chooses.
     *
     * The platform menu is refused only where this face has something better to offer,
     * which is over a touch; a press anywhere else on this screen still gets the browser
     * its reader is used to. Which cell the pointer was over is carried too, because
     * "copy value" with no value in mind is a menu item that has to guess.
     */
    const onContext = (event) => {
      const hit = event?.target;
      const chosen = hit && typeof hit.closest === 'function' ? hit.closest('[data-select-row]') : null;
      if (!chosen || typeof chosen.getAttribute !== 'function' || !state) return;
      const id = chosen.getAttribute('data-select-row');
      if (!id) return;
      const cellNode = typeof hit.closest === 'function' ? hit.closest('[data-cell]') : null;
      const cell = cellNode && typeof cellNode.getAttribute === 'function' ? cellNode.getAttribute('data-cell') : null;
      // One menu, named by the touch it belongs to. A second press replaces it rather
      // than adding a second, and a repaint cannot leave one behind, because this is the
      // only place a menu comes from.
      state = { ...state, menu: { row: id, cell, item: null, outcome: null } };
      takeFocus = true;
      if (typeof event.preventDefault === 'function') event.preventDefault();
      paint(view(state));
    };

    const onKey = (event) => {
      if (event?.key !== 'Escape') return;
      if (dismiss() && typeof event.preventDefault === 'function') event.preventDefault();
    };

    /**
     * The other half of click-away. The press handler above catches a press that lands
     * on this face; a reader who presses the shell around it is just as clearly done
     * with the menu, and that press never reaches this host. A press that lands inside
     * the menu is left alone here, or the item would be dismissed out from under the
     * hand aiming at it.
     */
    const onAway = (event) => { if (!inMenu(event?.target)) dismiss(); };

    if (typeof host.addEventListener === 'function') {
      host.addEventListener('click', onClick);
      host.addEventListener('contextmenu', onContext);
      host.addEventListener('keydown', onKey);
    }
    const watchingDoc = typeof doc?.addEventListener === 'function';
    if (watchingDoc) {
      doc.addEventListener('pointerdown', onAway);
      doc.addEventListener('keydown', onKey);
    }

    paint(waitingView());

    const ready = read(port)
      .then((first) => {
        state = first;
        paint(view(state));
        return state;
      })
      .catch((error) => {
        paint(frame([plain(`${GRAPH_MESSAGES.LIST_UNREAD}. ${error.message}`, 'unread')]));
        return null;
      });

    const unmount = () => {
      live = false;
      if (typeof host.removeEventListener === 'function') {
        host.removeEventListener('click', onClick);
        host.removeEventListener('contextmenu', onContext);
        host.removeEventListener('keydown', onKey);
      }
      if (watchingDoc) {
        doc.removeEventListener('pointerdown', onAway);
        doc.removeEventListener('keydown', onKey);
      }
      clear();
    };
    unmount.ready = ready;
    /** When nothing this window asked the clipboard for is still outstanding. A caller
     * that needs to know what the screen says after a press has to be able to wait for
     * the press to finish, and a fixed delay would be a guess. */
    unmount.quiet = () => queue;
    return unmount;
  }

  return {
    DECLARATION, mount, read, view, waitingView, toRecord, callerFor, toHtml: P.element.toHtml,
  };
}

export const face = createFace();
export const mount = face.mount;
export { DECLARATION };
