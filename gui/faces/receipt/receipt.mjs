// SPDX-License-Identifier: Apache-2.0
// The receipt face: one delta, confirmed without asking this window.
//
// Two reads, one record, and a discipline this file exists to hold structurally
// rather than by sentence: glovrex/req/405 SS5 (frozen SS605) splits a receipt into
// an attest layer (facts, moved for nothing) and a render layer (derived from
// attest, adding nothing). `toRecord()` below is the attest step -- it reads the
// two membrane answers verbatim and turns every absent or unreadable member into a
// declared hole, never a silent drop (conditions 1/2). Everything under `view()` is
// the render step: a badge, a claim, a summary line -- every one of them computed
// from what `toRecord()` (and the two decide-only helpers directly beneath it)
// already produced, never by reaching back into a raw wire answer a second time
// (condition 5). The seal claim in particular is computed exactly once, by
// parts/src/seal-claim.mjs's claimOf(), and every place this screen draws it reads
// that one answer -- there is no code path in this file that could ever print
// `sealed: true` because a payload said so.
//
// req/03 F-3's own three-part contract (section 5): the same screen carries (a) a
// permanent address, (b) the bytes extracted with their cut and their caveat, and
// (c) the procedure to confirm this without asking this window again. All three
// read from the one record; none of them is drawn until the record exists.
//
// What the r4 pass changed, and why. This screen used to open each of its five
// groups with a heading and then a sentence explaining the group, so a reader met six
// sentences of commentary before their first fact and never learnt how much of the
// receipt had actually arrived. Now: a band of counts at the head (how much of one
// delta and its receipt is here, how much is missing, and how much of what a stranger
// would need to confirm it is present), every group a bordered box whose head states
// its own count, and each of those sentences moved onto the container it explains,
// where it is reachable and no longer standing between the name and the facts. Every
// figure on the screen is counted from the record -- `census()` and `heldCount()`
// below are the only places a number on this face comes from, and the invariant that
// held + missing is every field looked for is asserted rather than assumed.

import {
  DECLARATION, CONSUMES, READS, ORDER, ROWS, UNDRAWN, QUESTION, FACE_ID,
} from './declaration.mjs';
import { parts as defaultParts } from './binding.mjs';

export const RECEIPT_MESSAGES = {
  NO_HOST: 'a face is mounted into a host element, and none was given',
  NO_PORT: 'a face is mounted with the port it is to speak through, and none was given',
  UNDECLARED: 'this face may not call a method it did not declare',
  READING: 'reading this receipt',
  NO_ID: 'this window was mounted with no delta id to read a receipt for (data-receipt-id on the host element was empty or absent)',
  DELTA_UNREAD: 'the delta this receipt is about could not be read',
  RECEIPT_UNREAD: 'the receipt itself could not be read',
  // req/822_c7 (Owner #387/#388 冗長文字全掃): this used to read "this member was
  // looked for and was not there: <member>", built with the member's own name
  // stitched onto the end of it in toRecord() below. Drawn as a note line
  // (noteLines()) or beside its own field label (addressSection(), bytesSection()),
  // that put the member's name on the screen twice. The member is still the row
  // label or the field label; the sentence carries no second copy of it.
  MEMBER_ABSENT: 'not in this record',
  MEMBER_UNREAD: 'this member could not be read because the read that would have carried it did not answer',
  MEMBER_NOT_SCALAR: 'this member arrived as a structure this face does not read',
  NOTE_SUMMARY: 'what this row holds in full, and what is missing from it',
  NOT_A_PROOF_HEADING: 'this screen never says checked unless a verifier is present',
  ADDRESS_MISSING: 'no permanent address is on this receipt yet',
  // Was 'permanent address (anchor)', inside a box whose own head reads "(a) permanent
  // address". Two of those three words were the head said twice; `anchor` is the one
  // word the head does not carry, and it is the name a stranger has to ask the server
  // for, so it is the one that earns the line (Owner #348 (4)).
  ANCHOR_LABEL: 'anchor',
  // -- the menu (Owner #348 (2)) ------------------------------------------------
  MENU_COPY: 'copy',
  MENU_WHOLE: 'the whole value, not the shortened form drawn here',
  MENU_HOLE: 'there is nothing to take: this member was looked for and was not there',
  MENU_NO_CLIPBOARD: 'this window has no clipboard, so nothing was taken',
  MENU_COPIED: 'copied',
  MENU_REFUSED: 'the clipboard refused, so nothing was taken',
  MENU_NO_ACTS: 'this screen changes nothing, so the only thing it can offer is to hand a value over',
  // The three sentences that used to stand between a section's name and its first
  // fact. They are still said -- on the container they belong to, where a reader who
  // wants them can reach them and a reader who wants the fact is not made to read
  // them first (req/97 section 3, this face's own "repeated prose" row).
  WHY_ADDRESS: 'the one field on this screen a third party could use to find this receipt without coming back through this window.',
  WHY_BYTES: 'getting the actual bytes out is the next step once there is an address to check them against.',
  BAND: 'the size and shape of this screen before a word of it is read: one delta, how much of it and of its receipt arrived, and how much of what a stranger would need to confirm it is here.',
};

const ANSWERED = 'answered';

/**
 * Weight, spelled once (Owner #348 (4): "explicit weight hierarchy, mechanical rather
 * than incidental").
 *
 * Three roles and no fourth, because a fourth is one nobody could name: `strong` is a
 * figure or the one word a group turns on, `label` is the name of a thing, and body
 * text is the page's own weight and is therefore not spelled at all. Every call site
 * spreads `weight(role)` rather than writing a number, so the file contains the string
 * `font-weight` exactly once -- which is what tools/gate.mjs's `weight-is-spelled-once`
 * check counts. A number typed at a call site would pass every visual review and is
 * exactly how a system that declared three weights ends up drawing five.
 */
const WEIGHTS = Object.freeze({ strong: '700', label: '500' });
const weight = (role) => ({ 'font-weight': WEIGHTS[role] });

/**
 * Where a line is allowed to break (Owner #348 (4): "no mid-word breaks and no
 * orphaned single characters on their own line").
 *
 * Two kinds of string are drawn on this screen and they want opposite rules. Prose
 * breaks between words: `break-word` only splits a word that could not fit on a line
 * of its own, so it cannot leave one character stranded the way `anywhere` can --
 * which is what every prose block on this face asked for before this pass, including
 * the six claim sentences. An opaque value -- a digest, an anchor, a path -- carries
 * no spaces at all, so it has no between-words to break at; `anywhere` is the only
 * rule that keeps it inside its column, and it is correct there because a hexadecimal
 * string has no words to break in the middle of. The two are told apart by the node's
 * own `data-text`, and tools/gate.mjs holds the population of nodes allowed to break
 * mid-word to the ones that say they are opaque.
 */
const prose = () => ({ 'overflow-wrap': 'break-word', 'word-break': 'normal', hyphens: 'none' });
const opaque = () => ({ 'overflow-wrap': 'anywhere' });

/** The five delta members this face reads off get_transformations_id, the same
 * member names faces/ledger and faces/held read off their own list items -- one
 * row grammar, read the same way on every face that draws it. */
const DELTA_MEMBERS = Object.freeze([
  { key: 'at', member: 'at' },
  { key: 'actor', member: 'actor' },
  { key: 'effect', member: 'effect' },
  { key: 'verdict', member: 'verdict' },
  { key: 'path', member: 'path' },
]);

/** The three receipt-only members this face reads off get_receipts_tid, beyond the
 * digest (handled separately below because it is the one field this face compares
 * against the delta's own digest). */
const RECEIPT_MEMBERS = Object.freeze(['algorithm', 'anchor', 'basis']);

/**
 * The four groups this screen draws, as the lists of record members each one holds.
 *
 * Every count in a box head and every figure in the band above them is derived from
 * one of these lists rather than typed beside it, so a denominator on this screen
 * cannot drift from the set of things actually looked for: add a member here and the
 * head that states it counts one more.
 */
const DELTA_FIELDS = Object.freeze([...DELTA_MEMBERS.map((m) => m.key), 'digest']);
const RECEIPT_FIELDS = Object.freeze(['receiptDigest', ...RECEIPT_MEMBERS]);
const ADDRESS_FIELDS = Object.freeze(['anchor']);
const BYTES_FIELDS = Object.freeze(['receiptDigest', 'algorithm']);

const isScalar = (value) => typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean';

/** How many of a named set of members this record actually holds. */
export function heldCount(record, fields) {
  return fields.filter((field) => typeof record?.[field] === 'string').length;
}

/**
 * The population of this screen, counted once.
 *
 * A receipt face has no list to count, so the honest measure of its size is how much
 * of the one delta and the one receipt arrived: `held` members against `looked`
 * members looked for, and `missing` the holes `toRecord()` named. The three are not
 * independent -- `toRecord()` writes a hole for every member it does not keep a cell
 * for, so held + missing is always looked, and the test that asserts it is what would
 * catch a member being dropped without being declared.
 */
export function census(record) {
  return Object.freeze({
    held: heldCount(record, DELTA_FIELDS) + heldCount(record, RECEIPT_FIELDS),
    looked: DELTA_FIELDS.length + RECEIPT_FIELDS.length,
    missing: Object.keys(record?.holes ?? {}).length,
  });
}

/** A caller that cannot reach a method the declaration does not hold. */
export function callerFor(port, allowed = CONSUMES) {
  const allow = new Set(allowed);
  const guard = (name) => {
    if (!allow.has(name)) throw new Error(`${RECEIPT_MESSAGES.UNDECLARED}: ${name}`);
  };
  return {
    async invoke(name, input) {
      guard(name);
      const method = port[name];
      if (typeof method !== 'function') return { outcome: 'absent', reason: 'no_such_method', requested: { name } };
      return method(input);
    },
  };
}

/**
 * Attest. Reads the two envelopes verbatim; every member that is absent, unread,
 * or arrives as something other than a scalar becomes a named hole rather than a
 * silent omission (glovrex/req/405 SS5 conditions 1/2). No comparison and no
 * badge-worthy judgement happens in this function -- that is `digestAgreement()`
 * and `view()`'s job, kept apart on purpose so this one stays a record of what was
 * actually read.
 */
export function toRecord({ id, delta, receipt }) {
  const deltaRead = delta?.outcome === ANSWERED;
  const receiptRead = receipt?.outcome === ANSWERED;
  const deltaBody = deltaRead && delta.body && typeof delta.body === 'object' && !Array.isArray(delta.body) ? delta.body : null;
  const receiptBody = receiptRead && receipt.body && typeof receipt.body === 'object' && !Array.isArray(receipt.body) ? receipt.body : null;

  const holes = {};
  const cells = {};
  for (const { key, member } of DELTA_MEMBERS) {
    if (!deltaRead) { holes[key] = `${RECEIPT_MESSAGES.MEMBER_UNREAD}: ${member}`; continue; }
    const value = deltaBody ? deltaBody[member] : undefined;
    if (value === undefined || value === null || value === '') holes[key] = RECEIPT_MESSAGES.MEMBER_ABSENT;
    else if (!isScalar(value)) holes[key] = `${RECEIPT_MESSAGES.MEMBER_NOT_SCALAR}: ${member}`;
    else cells[key] = String(value);
  }

  let digest = null;
  if (!deltaRead) holes.digest = `${RECEIPT_MESSAGES.MEMBER_UNREAD}: digest`;
  else if (!isScalar(deltaBody?.digest) || String(deltaBody.digest) === '') holes.digest = RECEIPT_MESSAGES.MEMBER_ABSENT;
  else digest = String(deltaBody.digest);

  const receiptCells = {};
  for (const field of RECEIPT_MEMBERS) {
    const key = `receipt_${field}`;
    if (!receiptRead) { holes[key] = `${RECEIPT_MESSAGES.MEMBER_UNREAD}: receipt.${field}`; continue; }
    const value = receiptBody ? receiptBody[field] : undefined;
    if (value === undefined || value === null || value === '') holes[key] = RECEIPT_MESSAGES.MEMBER_ABSENT;
    else if (!isScalar(value)) holes[key] = `${RECEIPT_MESSAGES.MEMBER_NOT_SCALAR}: receipt.${field}`;
    else receiptCells[field] = String(value);
  }

  let receiptDigest = null;
  if (!receiptRead) holes.receipt_digest = `${RECEIPT_MESSAGES.MEMBER_UNREAD}: receipt.digest`;
  else if (!isScalar(receiptBody?.digest) || String(receiptBody.digest) === '') holes.receipt_digest = RECEIPT_MESSAGES.MEMBER_ABSENT;
  else receiptDigest = String(receiptBody.digest);

  return Object.freeze({
    id: typeof id === 'string' && id !== '' ? id : null,
    ...cells,
    ...(digest ? { digest } : {}),
    ...(Number.isInteger(deltaBody?.sequence) ? { n: deltaBody.sequence } : {}),
    prev: deltaBody?.prev ?? null,
    lifecycle: 'settled',
    receiptDigest,
    algorithm: receiptCells.algorithm ?? null,
    anchor: receiptCells.anchor ?? null,
    basis: receiptCells.basis ?? null,
    deltaOutcome: delta?.outcome ?? 'absent',
    receiptOutcome: receipt?.outcome ?? 'absent',
    noteSummary: RECEIPT_MESSAGES.NOTE_SUMMARY,
    holes: Object.freeze(holes),
  });
}

/**
 * Decide (face-local, the same kind of helper faces/ledger's budgetFor() is --
 * domain logic this one screen needs, not a second implementation of a shared
 * part). Compares two facts `toRecord()` already read; invents nothing new. In the
 * same claim shape parts/src/checkable.mjs already uses so it can sit in the same
 * list without a caller having to know which function produced which entry.
 */
export function digestAgreement(record) {
  const deltaDigest = record?.digest ?? null;
  const receiptDigest = record?.receiptDigest ?? null;
  const bothPresent = typeof deltaDigest === 'string' && typeof receiptDigest === 'string';
  return {
    id: 'receipt-digest-agrees-with-delta',
    // Four words came off this sentence and one of them was wrong to be there. "own"
    // twice and "stated" said nothing a reader could act on; "provably" said more than
    // a string comparison does, on the one screen in this application that refuses to
    // say checked without a verifier in hand. What is left is the whole of what the
    // test performs and the whole of what it buys.
    claim: 'The receipt\'s digest is the identical string to the delta\'s postcondition digest, so this receipt is about this delta and not another wearing its id.',
    holds: bothPresent && deltaDigest === receiptDigest,
    detail: !bothPresent
      ? `cannot compare -- ${deltaDigest ? '' : 'delta digest missing; '}${receiptDigest ? '' : 'receipt digest missing'}`.trim().replace(/;\s*$/, '')
      : (deltaDigest === receiptDigest ? `both read ${deltaDigest}` : `delta reads ${deltaDigest}, receipt reads ${receiptDigest} -- these do not agree`),
  };
}

export function createFace({ parts = defaultParts } = {}) {
  const P = parts;
  const { el, style, find } = P.element;
  const T = P.tokens;

  // -- small pieces of type ---------------------------------------------------

  /**
   * A paragraph. One shape, two callers (Owner #349 (3)).
   *
   * `aside` and `plain` were two nine-line function bodies differing in a colour and a
   * margin, which is the same duplication every other face in this tree grew
   * independently -- faces/ledger has already collapsed its own pair into exactly this
   * `line(words, role, colour, margin)` shape, and writing a third variant of the pair
   * here rather than the shape it collapses to would be inventing the work twice.
   */
  const line = (words, role, colour, margin) => el('p', {
    'data-role': role, 'data-text': 'prose',
    style: style({
      margin: `0 0 ${margin}`, color: colour, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, ...prose(),
    }),
  }, [words]);

  const aside = (words, role = 'aside') => line(words, role, T.attendant, '6px');
  const plain = (words, role = 'line') => line(words, role, T.ink, '4px');

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

  /**
   * A group on this screen (Owner directive 340's box, in this face's own terms).
   *
   * What it replaces is a heading, a rule and then facts: a reader had to work out
   * where one group ended and the next began from spacing, and the group never said
   * how much it was holding. A box states its name and its own count in one strip,
   * so the head answers "how much of this is here" before the body is read -- and it
   * is the one place a section's explanatory sentence now lives, as `said`, reachable
   * on the container it explains rather than standing between the name and the first
   * fact.
   *
   * The wrapper carries the section's name and state and draws nothing. It exists so
   * that the object a reader sees and the object an instrument asks for by
   * `data-section` are the same object rather than two.
   */
  const sectionBox = (name, state, {
    title, count, noun, pill = null, said = null, children,
  }) => el('div', { 'data-section': name, 'data-state': state }, [
    P.box({
      name: title,
      count,
      noun,
      pill,
      said,
      children: el('div', {
        'data-role': 'box-body',
        style: style({ padding: `8px ${T.padX} 10px` }),
      }, children.filter(Boolean)),
    }),
  ]);

  /**
   * The block a menu is appended to.
   *
   * A menu on this application may not float: `tools/gate.mjs`'s `nothing-out-of-flow`
   * forbids a positioned element in a face, and it forbids it because a positioned note
   * once covered the row it belonged to -- a menu drawn over the receipt it is copying
   * from is that same defect with a different name. So the menu opens in the flow,
   * inside the block that holds the line it was asked for, which pushes what is below
   * it down and leaves the line the reader is looking at exactly where it was.
   *
   * This wrapper exists so that appending is enough. A menu appended into a grid line
   * would become another cell of that grid; appended into a plain block above it, it is
   * a block under a line.
   */
  const copyAnchor = (what, node) => el('div', {
    'data-copy-anchor': what,
    style: style({ display: 'block' }),
  }, [node]);

  const kvLine = (name, value, { mono = false, copy = null, whole = false } = {}) => {
    // `grid`, not `line`: `line()` above is this file's paragraph, and a local name
    // that shadows a helper defined thirty lines up is how the wrong one gets called.
    const grid = el('div', {
      'data-role': 'kv-line',
      style: style({
        display: 'grid', 'grid-template-columns': 'minmax(0,10rem) minmax(0,1fr)', gap: '10px', padding: '3px 0',
        'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
      }),
    }, [
      el('span', { 'data-role': 'kv-name', style: style({ color: T.attendant, ...weight('label') }) }, [name]),
      el('span', {
        'data-role': 'kv-value',
        'data-text': mono ? 'opaque' : 'prose',
        'data-copy': copy,
        'data-copy-whole': copy && whole ? 'true' : null,
        style: style({ color: T.ink, 'font-family': mono ? T.mono : T.sans, ...(mono ? opaque() : prose()) }),
      }, [value]),
    ]);
    return copy === null ? grid : copyAnchor(name, grid);
  };

  // -- compact header + bordered one-row controls (SS657 retrofit, req/38 SS657
  // Owner #317/#318; idiom proven by faces/atlas). See faces/ledger's own copy of
  // this comment for the fuller account of the five seat-confirmed defects.

  /**
   * The head of the screen: what this face is, which delta it is about, and what
   * happened when it asked.
   *
   * The id was not on this screen before this pass, and it is the one string a reader
   * needs in order to come back to this receipt, quote it to somebody, or ask the
   * server for it again -- the anchor is the address a stranger uses, this is the
   * address we use. It is drawn where the subject of a screen belongs (beside the
   * screen's own name) and it is the cell the menu's copy entry matters most on.
   */
  const headerLine = ({ id, outcomes }) => copyAnchor('subject', el('div', {
    'data-role': 'face-header',
    style: style({ display: 'flex', 'align-items': 'baseline', gap: '10px', 'flex-wrap': 'wrap', padding: '10px 0 6px', 'font-family': T.sans }),
  }, [
    el('span', { style: style({ ...weight('strong'), 'font-size': T.head, 'line-height': T.headLine, color: T.ink }) }, [FACE_ID]),
    id ? el('span', {
      'data-role': 'subject', 'data-text': 'opaque', 'data-copy': id,
      style: style({ color: T.ink, 'font-family': T.mono, 'font-size': T.record, 'line-height': T.recordLine, ...opaque() }),
    }, [id]) : null,
    el('span', { style: style({ color: T.attendant, 'font-size': T.record, 'line-height': T.recordLine, ...prose() }) }, [outcomes]),
  ]));

  const controlToggle = (label, hint, body, { open = false } = {}) => el('details', {
    'data-role': 'control', 'data-control': label, 'data-open': String(Boolean(open)), open: open || null,
    style: style({ border: `1px solid ${T.rule}`, 'border-radius': T.radiusControl, background: T.page }),
  }, [
    el('summary', {
      // req/822_c7 (Owner #387/#388 冗長文字全掃): the hint used to be drawn as its
      // own visible span next to the label, always on. `label` stays the
      // default-visible surface; `hint` rides the summary's own title (a hover)
      // and a `data-hint` attribute now.
      title: hint, 'data-hint': hint,
      style: style({
        display: 'flex', 'align-items': 'center', gap: '6px', 'min-height': '36px', 'box-sizing': 'border-box',
        padding: `0 ${T.padX}`, cursor: 'default', color: T.ink, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'list-style': 'none',
      }),
    }, [
      P.glyph('structure', open ? 'fold-open' : 'fold-shut', { size: P.minReadable, label: open ? 'open' : 'closed' }),
      el('span', { style: style(weight('label')) }, [label]),
    ]),
    el('div', { style: style({ padding: `0 ${T.padX} 10px` }) }, [body]),
  ]);

  const controlsRow = (children) => el('div', {
    'data-role': 'control-row',
    style: style({ display: 'flex', gap: '8px', 'flex-wrap': 'wrap', padding: '0 0 10px' }),
  }, children);

  /**
   * The legend's tally. The first column was `minmax(0,9rem)` and the longest mark
   * name in this face's declaration is nineteen characters of 14px monospace, which is
   * wider than that -- so `structure/unsealed` overflowed its track and painted over
   * the count beside it, and `structure/fold-shut` broke after its hyphen onto a
   * second line. Neither shows up in tools/shoot.mjs, because the legend is folded
   * shut in every fixture and an invisible node cannot overlap anything; both are
   * plainly visible in the interaction pass's own shots, which open it. The column is
   * now wide enough for the longest name this declaration holds, the name does not
   * wrap, and test/receipt.test.mjs asserts the fit rather than trusting the eye.
   */
  // 13rem, arrived at by measuring rather than by counting characters: 11rem still
  // overflowed `structure/fold-shut` in the interaction pass's own reading, because a
  // monospace advance is a font's business and not arithmetic this file may do.
  const MARK_COLUMN = '13rem';

  /**
   * One legend row shape, used by both of the legend's tables. They were two copies of
   * the same three-column grid differing only in what went into the cells, and the
   * copies had already drifted -- the widened column had to be applied twice for the
   * two tables to keep lining up with each other.
   */
  const legendRow = (attrs, cells) => el('div', {
    ...attrs,
    style: style({
      display: 'grid', 'grid-template-columns': `minmax(0,${MARK_COLUMN}) 2.5rem minmax(0,1fr)`,
      // Each cell is its own height rather than stretched to the tallest in the row.
      // Without this the mark name is as tall as a three-line description beside it,
      // and "did this name wrap" cannot be asked of it at all.
      'align-items': 'start',
      gap: '10px', padding: '2px 0',
      'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
    }),
  }, cells);

  const markTallyRows = (counts) => DECLARATION.marks.map((m) => legendRow(
    { 'data-mark-entry': m.mark, 'data-count': String(counts.get(m.mark) ?? 0) },
    [
      el('span', { style: style({ color: T.ink, 'font-family': T.mono, 'font-size': T.time, 'white-space': 'nowrap' }) }, [m.mark]),
      el('span', { style: style({ color: T.attendant, 'font-family': T.mono }) }, [String(counts.get(m.mark) ?? 0)]),
      el('span', { 'data-text': 'prose', style: style({ color: T.attendant, ...prose() }) }, [m.from]),
    ],
  ));

  const notDrawnLegendRows = () => UNDRAWN.map((entry) => legendRow(
    { 'data-not-drawn': entry.what },
    [
      el('span', { style: style({ color: T.attendant, ...weight('label') }) }, ['not drawn']),
      el('span', {}, ['']),
      el('span', { 'data-text': 'prose', style: style({ color: T.ink, ...prose() }) }, [`${entry.what} -- ${entry.why}`]),
    ],
  ));

  // -- (a) the delta row (Composition A, one call, not a list) ----------------

  const DELTA_HOLE_KEYS = new Set([...DELTA_MEMBERS.map((m) => m.key), 'digest']);

  /** The delta row's own note carries only the delta's own holes -- a receipt-side
   * hole (receipt_anchor, receipt_algorithm, ...) belongs to the address/bytes/
   * verify sections below the row, which already state it in cleaner language, not
   * repeated here under its raw internal key name. */
  function noteLines(record) {
    const lines = Object.entries(record.holes ?? {})
      .filter(([key]) => DELTA_HOLE_KEYS.has(key))
      .map(([key, why]) => ({ name: key, value: why }));
    for (const { key } of DELTA_MEMBERS) {
      // The name is the member's name and nothing else. Every one of these used to
      // read "<key> in full", which the note's own opening line already says once
      // ("what this row holds in full, and what is missing from it") -- eight
      // characters of the same sentence, five times, under the sentence itself.
      if (record[key] !== undefined) lines.push({ name: key, value: record[key] });
    }
    if (record.n !== undefined) lines.push({ name: 'sequence', value: String(record.n) });
    lines.push({ name: 'prev', value: record.prev ?? '(none)' });
    return lines;
  }

  function deltaOutcomeLines(record) {
    const outcome = record.deltaOutcome;
    const lines = [plain(`outcome: ${outcome}`, 'outcome')];
    const raw = record.rawDelta;
    if (outcome === 'refused' && raw) {
      lines.push(plain(`${raw.problem?.title ?? ''}: ${raw.problem?.detail ?? ''}`, 'refusal'));
      lines.push(plain(`code: ${raw.gx_code ?? raw.problem?.gx_code ?? 'none'}`, 'code'));
      lines.push(plain(`status: ${raw.status ?? 'none'}`, 'status'));
    }
    if (outcome === 'failed' && raw) lines.push(plain(`${raw.reason}: ${raw.detail ?? ''}`, 'failure'));
    if (outcome === 'absent' && raw) lines.push(plain(`${raw.reason}: ${JSON.stringify(raw.requested ?? null)}`, 'absence'));
    return lines;
  }

  const CHARACTER_PX = 7;
  const budgetFor = (column) => {
    const fixed = /^(\d+)px$/.exec(column.width);
    if (fixed) return Math.floor(Number(fixed[1]) / CHARACTER_PX);
    const rem = /minmax\(0,\s*([\d.]+)rem\)/.exec(column.width);
    if (rem) return Math.floor(Number(rem[1]) * 2);
    return 40;
  };
  const BUDGETS = Object.fromEntries(P.columns.map((column) => [column.key, budgetFor(column)]));

  /**
   * req/768 AC-7 (retrofit round 2): this screen reads exactly one delta and one
   * receipt -- there is no list of siblings to check for a row naming this delta
   * as its predecessor, so reversalOf() is always called with an empty sibling
   * set. It answers not-observable every time, honestly: this face's own limited
   * scope is the reason, not a special case coded around it (see
   * parts/src/reversibility.mjs's own file header). AC-4 does not apply here --
   * this face's declaration.mjs offers no acts at all (req/03 section 3-1: "transformations
   * / receipt (2)", no commit/cancel/undo route), so there is nothing a gutter
   * could ever hold; `acts` is never passed to openableRow() below.
   */
  /**
   * The standing this delta was given, in the box head, as the filled badge the rest
   * of this app draws a verdict with -- `badge()` and not `chip()`, because the
   * verdict namespace has its own entry point and the chip exists for the namespaces
   * that do not (parts/src/verdict-badge.mjs's own account of the split).
   *
   * Nothing is invented for a delta that did not arrive: a head with no pill says the
   * group has no standing, which is true, where a neutral pill would be this screen
   * classifying something it never read.
   */
  const verdictPill = (record) => {
    if (typeof record.verdict !== 'string') return null;
    return P.markOf('verdict', record.verdict).defined ? P.badge(record.verdict) : null;
  };

  /**
   * Which row cell hands over which whole value, and whether the drawn form is a
   * shortening of it.
   *
   * The row part draws what fits: the time column draws a declared cut of the
   * timestamp, the fingerprint column draws six characters of a digest, and at 720px
   * the verdict column draws whatever survives its width. A menu entry that handed
   * over the drawn characters would hand over a truncation -- which on this face,
   * whose whole purpose is a receipt somebody can check elsewhere, is the worst thing
   * a copy could do. So the value taken is always the record's, and where the drawn
   * form is shorter the entry says so before it is pressed.
   *
   * `lifecycle` and `seal` are absent because they draw marks and not values; there is
   * no string under them to hand anybody.
   */
  const WHOLE_VALUES = Object.freeze([
    { cell: 'at', from: 'at', shortened: true },
    { cell: 'actor', from: 'actor', shortened: false },
    { cell: 'effect', from: 'effect', shortened: false },
    { cell: 'verdict', from: 'verdict', shortened: false },
    { cell: 'fingerprint', from: 'digest', shortened: true },
    { cell: 'path', from: 'path', shortened: false },
  ]);

  /**
   * The row, with what each of its cells actually holds written onto it.
   *
   * This walks the tree this call has just built and nothing else -- it never reaches
   * into parts/src, which owns the row's shape and has no business knowing that this
   * screen offers a menu. A cell that drew a hole is left alone and answers with its
   * own reason when the menu asks; a cell whose value this record does not hold gets
   * no copy attribute rather than an empty one, because "nothing to take" and "take
   * this empty string" are different answers.
   */
  function withWholeValues(rowNode, record) {
    const by = new Map(WHOLE_VALUES.map((entry) => [entry.cell, entry]));
    P.element.walk(rowNode, (node) => {
      if (P.element.isText(node)) return;
      const key = node.attrs['data-cell'];
      if (!key || node.attrs['data-state'] !== 'value') return;
      const entry = by.get(key);
      if (!entry) return;
      const value = record[entry.from];
      if (typeof value !== 'string' || value === '') return;
      node.attrs['data-copy'] = value;
      if (entry.shortened) node.attrs['data-copy-whole'] = 'true';
    });
    return rowNode;
  }

  function deltaSection(record, claim, rawDelta) {
    const head = {
      title: 'the delta',
      noun: `of ${DELTA_FIELDS.length} read`,
      said: 'the one delta this screen is about, as the engine recorded it.',
    };
    if (!record.id) {
      return sectionBox('delta', 'no-id', {
        ...head,
        count: 0,
        children: [aside(RECEIPT_MESSAGES.NO_ID, 'no-id')],
      });
    }
    const holeKeys = Object.keys(record.holes ?? {}).filter((k) => DELTA_HOLE_KEYS.has(k));
    // Against the drawn form, not the arrived value (req/97 gap-list item gap 1): the `at`
    // column draws a declared cut, so an ISO-8601 timestamp is not a clip here.
    const open = holeKeys.length > 0
      || DELTA_MEMBERS.some((m) => P.drawnTextFor(m.key, record[m.key]).length > (BUDGETS[m.key] ?? 40));
    const reversal = P.reversalOf(record, []);
    return sectionBox('delta', record.deltaOutcome === ANSWERED ? 'read' : 'unread', {
      ...head,
      count: heldCount(record, DELTA_FIELDS),
      pill: verdictPill(record),
      children: [
        record.deltaOutcome !== ANSWERED ? aside(RECEIPT_MESSAGES.DELTA_UNREAD, 'unread') : null,
        ...(record.deltaOutcome !== ANSWERED ? deltaOutcomeLines({ ...record, rawDelta }) : []),
        record.deltaOutcome === ANSWERED
          ? copyAnchor('delta', withWholeValues(
            P.openableRow(record, { claim, note: noteLines(record), open, reversal }),
            record,
          ))
          : null,
      ],
    });
  }

  // -- (a) permanent address ----------------------------------------------------

  function addressSection(record) {
    const hole = record.holes?.receipt_anchor;
    return sectionBox('address', hole ? 'hole' : (record.anchor ? 'present' : 'absent'), {
      title: '(a) permanent address',
      count: heldCount(record, ADDRESS_FIELDS),
      noun: `of ${ADDRESS_FIELDS.length} read`,
      said: RECEIPT_MESSAGES.WHY_ADDRESS,
      children: [
        record.anchor
          ? kvLine(RECEIPT_MESSAGES.ANCHOR_LABEL, record.anchor, { mono: true, copy: record.anchor })
          : plain(hole ? `${RECEIPT_MESSAGES.ANCHOR_LABEL}: ${hole}` : RECEIPT_MESSAGES.ADDRESS_MISSING, 'address-missing'),
      ],
    });
  }

  // -- (b) bytes extraction ------------------------------------------------------

  function bytesSection(record) {
    const hole = record.holes?.receipt_digest;
    const head = {
      title: '(b) bytes, extracted',
      count: heldCount(record, BYTES_FIELDS),
      noun: `of ${BYTES_FIELDS.length} read`,
      said: RECEIPT_MESSAGES.WHY_BYTES,
    };
    if (hole || !record.receiptDigest) {
      return sectionBox('bytes', 'hole', {
        ...head,
        children: [plain(hole ?? RECEIPT_MESSAGES.MEMBER_ABSENT, 'bytes-missing')],
      });
    }
    return sectionBox('bytes', 'present', {
      ...head,
      children: [
        P.serial(record.receiptDigest, { take: 6, size: 16 }),
        kvLine('digest in full', record.receiptDigest, { mono: true, copy: record.receiptDigest }),
        record.algorithm
          ? kvLine('algorithm', record.algorithm, { copy: record.algorithm })
          : plain(record.holes?.receipt_algorithm ?? RECEIPT_MESSAGES.MEMBER_ABSENT, 'algorithm-missing'),
      ],
    });
  }

  // -- (c) confirm without the issuer --------------------------------------------

  function verifySection(record, claim, portable, claims) {
    return sectionBox('verify', claim.sealed ? 'sealed' : 'unsealed', {
      title: '(c) confirm without the issuer',
      count: claims.filter((c) => c.holds).length,
      noun: `of ${claims.length} hold`,
      pill: P.chip('structure', claim.sealed ? 'seal' : 'unsealed', {
        word: claim.standing,
        said: claim.why,
      }),
      said: RECEIPT_MESSAGES.NOT_A_PROOF_HEADING,
      children: [
        // Two labels came off these lines. This box's own head already wears the
        // standing as a pill, so `seal claim: unsealed -- ` was the head repeated and
        // then a colon; and `checkable elsewhere: ` prefixed a sentence that already
        // ends in the word "elsewhere". What is left on each line is the part the head
        // does not carry: the reason. The standing itself has not left the tree -- it
        // is on this node as `data-standing`, which is what tools/gate.mjs now reads to
        // check it against the row's seal mark, and reading an attribute is a stricter
        // check than the sentence match it replaces.
        el('p', {
          'data-role': 'seal-claim', 'data-standing': claim.standing, 'data-text': 'prose',
          style: style({
            margin: '0 0 4px', color: T.ink, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, ...prose(),
          }),
        }, [claim.why]),
        plain(portable.why, 'portability'),
        el('div', { 'data-role': 'claims', 'data-count': String(claims.length) }, claims.map((c) => el('div', {
        'data-claim': c.id,
        'data-holds': String(c.holds),
        style: style({
          display: 'grid', 'grid-template-columns': 'minmax(0,8rem) minmax(0,1fr)', gap: '10px', padding: '4px 0',
          'border-bottom': `1px solid ${T.rule}`, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
        }),
      }, [
        el('span', { 'data-role': 'verdict', style: style({ color: c.holds ? T.ink : T.deny, ...weight('strong') }) }, [c.holds ? 'holds' : 'does not hold']),
        el('div', {}, [
          el('div', { 'data-role': 'claim', 'data-text': 'prose', style: style({ color: T.ink, ...prose() }) }, [c.claim]),
          el('div', { 'data-role': 'detail', 'data-text': 'prose', style: style({ color: T.attendant, ...prose() }) }, [c.detail]),
        ]),
      ]))),
      ],
    });
  }

  // -- omitted --------------------------------------------------------------------

  /**
   * What a reader might expect and will not get. Each line names the thing; the
   * reason for it is on the line, reachable, and drawn in full in the legend -- where
   * it was already drawn, verbatim, at the same time as here. Four long reasons
   * printed twice on one screen is the same defect req/97 named as repeated prose,
   * and the cure is the one that rule always has: the sentence is kept once, in the
   * place a reader goes to look things up.
   */
  function notDrawnSection() {
    return sectionBox('not-drawn', 'stated', {
      title: 'omitted',
      count: UNDRAWN.length,
      // Was 'stated, with a reason each', under the word `omitted`, which is what
      // stating them is. The head now reads `omitted 4 with a reason each`.
      noun: 'with a reason each',
      said: 'things a reader might expect on this screen and will not find, each with the reason it is not here.',
      children: [
        el('div', { 'data-role': 'omissions' }, UNDRAWN.map((entry) => el('div', {
          'data-omission': entry.what,
          'data-text': 'prose',
          title: entry.why,
          style: style({
            padding: '3px 0', color: T.ink,
            'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, ...prose(),
          }),
        }, [entry.what]))),
      ],
    });
  }

  // -- the menu (Owner #348 (2)) ------------------------------------------------

  /**
   * What a right-click on this screen offers, decided from what the cell holds.
   *
   * This face declares no acts (`ACTS = []`, structurally, in declaration.mjs), so
   * there is no gutter to mirror and there is nothing here that sends anything. What
   * is left is the one entry Owner #348 (2) asks a face like this for anyway -- and on
   * this screen it is not a consolation prize. Every other face answers a question
   * about a population; this one exists so that one delta can be confirmed somewhere
   * that is not this window, and the four strings that lets somebody do it are the id,
   * the anchor, the digest and the algorithm. Handing those over was the act this face
   * has been missing since it was written: a reader who wanted the digest had to select
   * it with a mouse across a monospace run and hope they got both ends.
   *
   * No act is invented and none is offered that the row does not send, because the row
   * sends nothing. A cell that drew a declared hole gets the entry disabled with the
   * hole's own reason, which is the same rule the act gutter follows for an act a row
   * cannot take.
   *
   * The entries carry no mark. This application's marks are bespoke and live in one
   * sheet, that sheet has no mark for handing a value over, and this lane may not add
   * one to it -- so the choice was a word or somebody else's mark meaning something
   * else, and one-meaning-one-mark decides that. `MIN_ACT` is therefore not exercised
   * anywhere on this face; it is bound in binding.mjs so that the fact was read rather
   * than missed.
   */
  function menuEntries(cell) {
    const value = cell.getAttribute('data-copy');
    if (typeof value === 'string' && value !== '') {
      return [{
        id: 'copy-value',
        word: RECEIPT_MESSAGES.MENU_COPY,
        said: cell.getAttribute('data-copy-whole') === 'true' ? RECEIPT_MESSAGES.MENU_WHOLE : null,
        value,
        enabled: true,
      }];
    }
    const why = cell.getAttribute('data-state') === 'hole'
      ? (cell.getAttribute('title') ?? RECEIPT_MESSAGES.MENU_HOLE)
      : RECEIPT_MESSAGES.MENU_HOLE;
    return [{
      id: 'copy-value', word: RECEIPT_MESSAGES.MENU_COPY, said: why, value: null, enabled: false,
    }];
  }

  /**
   * The menu as a tree. In the flow (see copyAnchor), bordered like every other
   * container on this screen, and it borrows the shared motion route as a class rather
   * than writing a duration -- the one route this application has is not a face's to
   * respell (tools/gate.mjs's `no-raw-motion` holds that).
   */
  function menuTree(entries) {
    return el('div', {
      'data-part': 'menu', 'data-count': String(entries.length),
      style: style({
        display: 'block', margin: '4px 0 2px', 'box-sizing': 'border-box',
        border: `1px solid ${T.rule}`, 'border-radius': T.radiusControl, background: T.page,
        'max-width': '22rem',
      }),
    }, entries.map((entry) => el('button', {
      type: 'button',
      class: 'gx-move',
      'data-entry': entry.id,
      'data-enabled': String(entry.enabled),
      disabled: entry.enabled ? null : '',
      // No `title` here. Whatever this entry has to say is drawn on it, in the
      // `entry-said` span below, where it is read without a hover -- putting the same
      // sentence in a tooltip as well is the drawn-twice defect this pass is removing
      // everywhere else on this screen.
      style: style({
        display: 'flex', 'align-items': 'center', gap: '8px', width: '100%', 'min-height': '36px',
        'box-sizing': 'border-box', padding: `6px ${T.padX}`, 'text-align': 'left',
        border: 'none', background: 'transparent', 'border-radius': T.radiusControl,
        color: entry.enabled ? T.ink : T.attendant,
        'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
        cursor: entry.enabled ? 'pointer' : 'not-allowed',
      }),
    }, [
      el('span', { 'data-role': 'entry-word', style: style(weight('label')) }, [entry.word]),
      // Always drawn, even when it has nothing to say yet. It is the line the outcome
      // is written onto once the entry is pressed, and an entry that only recorded its
      // outcome in an attribute would be a control that looks identical whether or not
      // it did anything -- which is the exact failure the shell's own copy control was
      // written to avoid, repeated one layer up.
      el('span', {
        'data-role': 'entry-said', 'data-text': 'prose',
        style: style({ color: T.attendant, ...prose() }),
      }, [entry.said ?? '']),
    ])));
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
    return frame([plain(RECEIPT_MESSAGES.READING, 'reading')]);
  }

  function legendBody(counts) {
    return el('div', { 'data-role': 'legend' }, [
      el('div', { 'data-role': 'legend-marks' }, markTallyRows(counts)),
      el('div', { 'data-role': 'legend-prose' }, [
        // "(top row)" came off two of these. This screen draws exactly one row, so
        // there is no other row for a reader to have confused it with, and naming
        // where a thing sits is the kind of word Owner #348 (4) asks to be removed.
        kvLine('the fingerprint column', 'the first 6 characters of the delta\'s own postcondition digest, upper-cased. A match here is a hint, never a proof.'),
        // Said in this screen's own words rather than quoted from the shared row part.
        // That sentence now ends "in the cell's own title and in the pane for this
        // row", and this screen has no pane -- it draws one row and puts the whole
        // timestamp in the row's own note. A legend that quotes a sentence naming a
        // container this screen does not have is a legend that is wrong.
        kvLine('the time column', 'the time of day, taken from an ISO-8601 timestamp. This is a declared cut, not a clip: the date and the whole timestamp are in the cell\'s own title and in this row\'s note, so nothing about when this happened is only ever shown cut off.'),
        kvLine('(a)/(b)/(c)', 'the three things this screen has to carry before a receipt is any use to anybody: a permanent address, the bytes extracted with their cut and their caveat, and the procedure to confirm this without asking this window again.'),
        kvLine('the count in a box head', 'how many of the members that group holds actually arrived, against how many were looked for. A group that got none of them says 0 and keeps its border.'),
        // Was "computed once by this app's seal-claim part" -- the name of a component,
        // on a product surface, in a legend that claims to hold no internal names.
        kvLine('seal claim', 'never a boolean read off the payload. It is decided once, in one place, and it will not say sealed without an exact comparison and a verifier that is present.'),
        // The comparison to another screen came off the end of this one. What a
        // different screen would answer for a different record is not a fact about
        // this chip; the reason it always reads unknown here is, and that is what is
        // left.
        kvLine('undo availability chip', 'always reads "unknown" on this screen. This face reads exactly one delta with no list around it, so there is no sibling to check for a row naming this one as its predecessor.'),
      ]),
      el('div', { 'data-role': 'legend-not-drawn' }, notDrawnLegendRows()),
    ]);
  }

  /**
   * The two reads are still named one at a time rather than folded into the footer's
   * `N of 2 answers`. The counted form cannot say which of the two failed, and on a
   * screen whose whole subject is one delta and its receipt, "the delta answered and
   * the receipt did not" is a different situation from the reverse.
   */
  function headerWords(record) {
    return { id: record.id, outcomes: `delta ${record.deltaOutcome}, receipt ${record.receiptOutcome}` };
  }

  /** How many of the two reads this face performs came back with a body. */
  function answersRead(record) {
    return [record.deltaOutcome, record.receiptOutcome].filter((outcome) => outcome === ANSWERED).length;
  }

  /**
   * The band at the head of the screen: this face's population, before a word of it
   * is read.
   *
   * A receipt face's population is one delta, so the first figure is 1 and it carries
   * the standing -- the engine's own word for that delta, in the ink and the mark the
   * badge and the chip already spend on it, so what the screen decided is legible
   * from across a room rather than from a 14px stroke. The other three are the ones
   * this screen exists to answer: how much of what was looked for arrived, how much
   * did not, and how much of what somebody who is not us would need to confirm this
   * receipt is actually here. Every figure is counted from the record; a count this
   * face cannot know is null and draws a dash, and none of them is a constant.
   */
  function bandSegments(record, portable) {
    const counted = census(record);
    const mark = typeof record.verdict === 'string' ? P.markOf('verdict', record.verdict) : null;
    const standing = mark?.defined ? mark : null;
    return [
      {
        noun: 'delta',
        count: record.deltaOutcome === ANSWERED ? 1 : null,
        mark: standing ? P.glyph('verdict', record.verdict, { size: 16, label: record.verdict }) : null,
        tone: standing ? P.inkFor(standing) : null,
        said: standing
          ? `one delta, and the engine's own recorded word for it: ${record.verdict}`
          : `one delta, and what happened when this window asked for it: ${record.deltaOutcome}`,
      },
      {
        noun: `of ${counted.looked} fields`,
        count: counted.held,
        said: 'members of the delta and of its receipt that arrived and are drawn below.',
      },
      {
        noun: 'missing',
        count: counted.missing,
        mark: P.glyph('structure', 'hole', { size: 16, label: 'missing' }),
        said: 'members this face looked for and did not get. Each one is named where it would have been drawn, never left as blank space.',
      },
      {
        noun: `of ${P.portableFields.length} to confirm`,
        count: P.portableFields.length - portable.missing.length,
        said: portable.why,
      },
    ];
  }

  function view(state) {
    const startedAt = performance.now();
    const record = toRecord(state);
    const claim = P.claimOf(record, { verifier: state.verifier ?? null });
    // Both computed once, here, and handed down: the band states how much of what a
    // stranger needs is present and the verify section states the same reading in
    // words, and two calls could drift where one cannot.
    const portable = P.portability({ digest: record.receiptDigest, algorithm: record.algorithm, anchor: record.anchor });
    const claims = [digestAgreement(record), ...P.checkable(record.id ? [record] : [], [])];
    const content = [
      deltaSection(record, claim, state.delta),
      record.id ? addressSection(record) : null,
      record.id ? bytesSection(record) : null,
      record.id ? verifySection(record, claim, portable, claims) : null,
      notDrawnSection(),
    ].filter(Boolean);
    const band = P.statBand(bandSegments(record, portable), { said: RECEIPT_MESSAGES.BAND });
    // The tally counts the band as well as the sections. It is a count of what this
    // render drew, and the band draws marks; leaving it out would make the legend
    // say zero of a mark a reader can see on the screen.
    const counts = new Map();
    for (const node of [band, ...content]) {
      for (const marked of find(node, (n) => n.attrs && 'data-mark' in n.attrs)) {
        counts.set(marked.attrs['data-mark'], (counts.get(marked.attrs['data-mark']) ?? 0) + 1);
      }
    }
    const drawn = [
      headerLine(headerWords(record)),
      band,
      controlsRow([
        controlToggle('why', 'about this screen', aside(ORDER.reason, 'why-first')),
        controlToggle('legend', 'symbols and counts', legendBody(counts)),
      ]),
      ...content,
    ];
    // Measured, not estimated: the figure the footer prints is the time this call
    // spent building everything above the footer, read off the clock twice around the
    // work `view()` already does. It excludes the footer itself and the frame, which
    // cannot be built before the number they carry exists -- said here rather than in
    // a sentence on the screen.
    return frame([...drawn, P.runtimeFooter({
      renderMs: performance.now() - startedAt,
      source: `${answersRead(record)} of ${Object.keys(READS).length} answers`,
    })]);
  }

  // -- reading -------------------------------------------------------------------

  async function read(port, id) {
    const caller = callerFor(port);
    if (!id) {
      return {
        id: null,
        delta: { outcome: 'absent', reason: 'no_id_given', requested: { id: null } },
        receipt: { outcome: 'absent', reason: 'no_id_given', requested: { id: null } },
      };
    }
    const delta = await caller.invoke(READS.delta, { params: { id } });
    const receipt = await caller.invoke(READS.receipt, { params: { tid: id } });
    return { id, delta, receipt };
  }

  // -- mount ---------------------------------------------------------------------

  function idOf(host) {
    const raw = typeof host.getAttribute === 'function' ? host.getAttribute('data-receipt-id') : null;
    return typeof raw === 'string' && raw !== '' ? raw : null;
  }

  function mount(host, port, notices = []) {
    if (!host || typeof host.appendChild !== 'function') throw new TypeError(RECEIPT_MESSAGES.NO_HOST);
    if (!port || typeof port !== 'object') throw new TypeError(RECEIPT_MESSAGES.NO_PORT);
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
    // The one menu. It is one variable and not a list on purpose: the second
    // right-click that stacks a second menu over the first is the defect Owner #348
    // (2) names, and a single slot that is closed before it is filled cannot hold two.
    let menu = null;

    const dismiss = () => {
      if (!menu) return;
      if (menu.node.parentNode) menu.node.parentNode.removeChild(menu.node);
      menu = null;
    };

    const clear = () => { while (host.firstChild) host.removeChild(host.firstChild); };
    const paint = (tree) => {
      if (!live) return;
      // Before the host is emptied, not after. A repaint destroys the element the menu
      // is inside, and a `menu` variable still pointing at it would then refuse to
      // open the next one ("there is already a menu") while nothing was on the screen
      // -- a menu left behind in the only place it can be left behind, which is the
      // bookkeeping rather than the document.
      dismiss();
      clear();
      host.appendChild(P.element.render(doc, tree));
      // req/97 gap-list item 1. The rows are drawn shut; the renderer, not this
      // face's character arithmetic, decides whether any of them is actually
      // cutting a value off, and re-opens only those. Guarded because the
      // structural stand-in in test/dom-stand-in.mjs has no layout to measure --
      // there it is a no-op, which is the honest answer for a host that draws
      // nothing (parts/src/receipt-row.mjs openMeasuredClips states the rest).
      if (typeof host.querySelectorAll === 'function') P.openMeasuredClips(host);
    };

    /**
     * Copying, and saying which. The clipboard is asked of the window this face was
     * mounted into rather than of whatever global happens to be in scope, because a
     * face that reaches past its own document is a face that behaves differently
     * depending on who loaded it. A window with no clipboard -- a sandboxed frame, a
     * structural stand-in, a page served without the permission -- is answered with
     * the failed state and the reason, never with a control that looks identical
     * whether or not it did anything. The shell's own `command-copy` sets the same two
     * attributes for the same reason.
     */
    /** The entry's own line for saying what happened, found by asking its children
     * rather than by their position, and without a selector engine the structural
     * stand-in does not have. */
    const saidNodeOf = (button) => Array.from(button.childNodes ?? [])
      .find((n) => typeof n.getAttribute === 'function' && n.getAttribute('data-role') === 'entry-said') ?? null;

    const takeValue = (button, value) => {
      // Both attributes are always written, rather than one being set and the other
      // removed the way the shell's own copy control does it. Three states have to be
      // told apart -- not tried, worked, did not work -- and presence-or-absence can
      // only carry two: an entry nobody has pressed and an entry whose copy failed
      // would look the same. Neither attribute is on a fresh entry; both are on a
      // pressed one. `data-copy-said` carries the reason in the same words the entry
      // would say them.
      // The outcome is drawn, not only recorded. Rewriting the one text node under the
      // entry's own said span works the same way in a window and in the structural
      // stand-in the unit tests mount into, where `textContent` would set a property
      // on an object and change nothing a reader or a test could see.
      const say = (node, words) => {
        if (!node) return;
        while (node.firstChild) node.removeChild(node.firstChild);
        node.appendChild(doc.createTextNode(words));
      };
      const settle = (worked, said) => {
        button.setAttribute('data-copied', String(worked));
        button.setAttribute('data-copy-failed', String(!worked));
        button.setAttribute('data-copy-said', said);
        say(saidNodeOf(button), said);
      };
      const clip = doc.defaultView?.navigator?.clipboard;
      if (!clip || typeof clip.writeText !== 'function') {
        settle(false, RECEIPT_MESSAGES.MENU_NO_CLIPBOARD);
        return;
      }
      Promise.resolve(clip.writeText(value)).then(
        () => settle(true, RECEIPT_MESSAGES.MENU_COPIED),
        () => settle(false, RECEIPT_MESSAGES.MENU_REFUSED),
      );
    };

    /**
     * A right-click, answered only where there is something to answer with. On empty
     * space, on prose, on a heading, this face does not preventDefault and the reader
     * gets their own window's menu -- taking that away everywhere in order to offer it
     * somewhere would be a worse trade than the entry is worth.
     */
    const onContextMenu = (event) => {
      const target = event.target ?? null;
      if (!target || typeof target.closest !== 'function') return;
      const cell = target.closest('[data-copy]') ?? target.closest('[data-cell]');
      if (!cell) return;
      const anchor = cell.closest('[data-copy-anchor]');
      if (!anchor) return;
      if (typeof event.preventDefault === 'function') event.preventDefault();
      dismiss();
      const entries = menuEntries(cell);
      const node = P.element.render(doc, menuTree(entries));
      anchor.appendChild(node);
      menu = { node, entries };
    };

    /** Ancestry by walking, because a structural stand-in has parents and not a
     * `contains`, and this face's mount is exercised against both. */
    const inside = (node, root) => {
      let at = node;
      while (at) {
        if (at === root) return true;
        at = at.parentNode;
      }
      return false;
    };

    const onHostClick = (event) => {
      const target = event.target ?? null;
      if (!menu || !target || typeof target.closest !== 'function') return;
      const pressed = target.closest('[data-entry]');
      // Anywhere in this face that is not an entry of the open menu closes it. That is
      // the click-away half of Owner #348 (2)'s dismissal, for the case where the
      // click lands on the face itself.
      if (!pressed || !inside(pressed, menu.node)) { dismiss(); return; }
      const entry = menu.entries.find((e) => e.id === pressed.getAttribute('data-entry'));
      if (!entry || !entry.enabled) return;
      takeValue(pressed, entry.value);
    };

    /** The other half: a click that never reaches this face at all. Attached to the
     * document, and deliberately not the same handler as the one above -- one listener
     * on two nodes would run twice for a press inside the host and write the clipboard
     * twice for one press. */
    const onAwayClick = (event) => {
      const target = event.target ?? null;
      if (!menu) return;
      if (inside(target, host)) return;
      dismiss();
    };

    const onKeyDown = (event) => {
      if (event.key === 'Escape') dismiss();
    };

    const listen = (node, pairs) => {
      if (!node || typeof node.addEventListener !== 'function') return () => {};
      for (const [type, handler] of pairs) node.addEventListener(type, handler);
      return () => {
        for (const [type, handler] of pairs) node.removeEventListener(type, handler);
      };
    };

    // Escape is listened for on both, because a key pressed while the focus is outside
    // this face never reaches the host; dismissing an already-dismissed menu is a
    // no-op, so the pair firing together is the same as one firing.
    const deafen = [
      listen(host, [['contextmenu', onContextMenu], ['click', onHostClick], ['keydown', onKeyDown]]),
      listen(doc, [['click', onAwayClick], ['keydown', onKeyDown]]),
    ];

    paint(waitingView());

    const ready = read(port, idOf(host))
      .then((first) => {
        state = first;
        paint(view(state));
        return state;
      })
      .catch((error) => {
        paint(frame([plain(`${RECEIPT_MESSAGES.RECEIPT_UNREAD}. ${error.message}`, 'unread')]));
        return null;
      });

    const unmount = () => {
      live = false;
      dismiss();
      for (const stop of deafen) stop();
      clear();
    };
    unmount.ready = ready;
    return unmount;
  }

  return {
    DECLARATION, mount, read, view, waitingView, toRecord, digestAgreement, callerFor, toHtml: P.element.toHtml,
  };
}

export const face = createFace();
export const mount = face.mount;
export { DECLARATION };
