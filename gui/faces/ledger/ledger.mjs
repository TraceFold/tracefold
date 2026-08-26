// SPDX-License-Identifier: Apache-2.0
// The ledger face: what happened, in order.
//
// One question, and everything here answers it or is not here. The screen has two
// halves because there are two kinds of thing to be in: what has happened, which can
// be pointed at, and what is being held, which has not happened and must never be
// drawn wearing a receipt's face. They get different words and different marks.
//
// Three properties are load-bearing and each one is a test.
//
// A list that could not be read is never drawn as a list with nothing in it. The two
// states are told apart at the top of every section, in different words, with the
// outcome named, because "the ledger is empty" and "the ledger could not be read" are
// opposite facts that look identical if a face fails open.
//
// A row that has been written is never edited. Undo does not rewrite the row it
// undoes; the server appends a child row and this face reads the list again. Nothing
// here holds a mutable copy of a row, so there is no code path that could edit one:
// records are built fresh from each read and frozen.
//
// The count of rows is never given alone. Every half states how many rows it drew out
// of how many it received, how many requests the walk took, and whether the walk
// stopped early -- a truncated ledger that says "forty rows" is a ledger that has gone
// quiet, and going quiet is the worst thing this product can do.
//
// What this face is not: it does not verify. No verifier is carried in this window, so
// nothing is drawn as sealed and the engine's own word about its ledger is carried up
// as the engine's word, labelled as such. Checking a receipt without its issuer is the
// third pillar of this product and it belongs to a face that can actually do it.

import {
  DECLARATION, CONSUMES, READS, ACTS, ORDER, ROWS, UNDRAWN, QUESTION, FACE_ID,
} from './declaration.mjs';
import { parts as defaultParts } from './binding.mjs';

export const LEDGER_MESSAGES = {
  NO_HOST: 'a face is mounted into a host element, and none was given',
  NO_PORT: 'a face is mounted with the port it is to speak through, and none was given',
  UNDECLARED: 'this face may not call a method it did not declare',
  UNKNOWN_ACT: 'no such act on this face',
  READING: 'reading the ledger',
  UNREAD: 'this list was not read. What is below is not a ledger with nothing in it; it is the absence of a ledger, and the two are different facts',
  EMPTY_SETTLED: 'nothing has been recorded here yet',
  EMPTY_HELD: 'nothing is being held here',
  ALL_DROPPED: 'rows arrived and none of them could be drawn; every one is listed below with the reason',
  TRUNCATED: 'the walk stopped at its budget, so there are rows behind this screen that it did not reach',
  REPEATED: 'the server handed back a cursor it had already given, so the walk stopped rather than circle',
  NOTHING_TO_SEAL: 'this has not happened yet, so there is no record of it to check',
  NO_VERIFIER_HERE: 'no verifier is present in this window, so nothing on this screen has been checked by anyone; the marks in the seal column say unsealed for that reason and not because anything is wrong',
  NOT_VERIFICATION: "this is the engine reporting on its own ledger. It is not offline re-verification: nothing here was checked without the issuer, and a window that drew this as checked would be drawing an unchecked record wearing a checked one's face",
  // req/822_c7 (Owner #387/#388 冗長文字全掃): this used to read "this member was
  // looked for on the item and was not there: <member>", built with the member's
  // own name stitched onto the end of it in toRecord() below. Drawn as a note line,
  // that put the member's name on the screen twice -- once as the row label beside
  // the sentence, once again inside the sentence itself. The member is still the
  // row label; the sentence carries no second copy of it.
  MEMBER_ABSENT: 'not in this record',
  MEMBER_NOT_SCALAR: 'this member arrived as a structure this face does not read',
  NOTE_SUMMARY: 'what is missing from this row, what it holds in full, and how to check what is not',
  CLIP_RISK: 'this row holds a value longer than its column can show, so the note under it carries every value in full. The budget per column is worked out from the width the column declares, before anything is drawn; the real width is only known in front of a renderer, and this application is checked against a real one so that gap between a guess and a measurement is never the reason a value goes unseen',
  WITHHELD: 'declared, offered, and not sent',
  SENT: 'sent',
  DREW: 'ledger drawn',
  BOX_SETTLED: 'what has happened, and can be pointed at',
  BOX_HELD: 'what has not happened yet: proposed, and waiting on somebody',
  SOURCE_ENGINE: 'the engine',
  IN_FLIGHT: 'this act has been sent and this window is waiting for the answer. It is offered again as soon as one arrives, whatever the answer is.',
  MENU_ROW: 'the acts this row offers, and the value under the pointer',
  MENU_NO_VALUE: 'this cell holds no value to copy',
  COPY_ASKED: 'the clipboard was asked to take this value and has not answered yet',
  COPIED: 'copied',
  COPY_REFUSED: 'this window has no clipboard to write to, so nothing was copied. Every value on this screen is also in the pane for its row, in full, where it can be taken by hand',
  COPY_FAILED: 'the clipboard refused the write, so nothing was copied',
};

const HALF = Object.freeze({ settled: 'settled', held: 'held' });

const ANSWERED = 'answered';

/**
 * The five columns whose values come straight off the item, and the member each one is
 * read from. The members are what this face looked for, not what the server is known
 * to send -- the crate's response bodies were never read (declaration, undrawn) -- so
 * anything absent becomes a declared hole with the member named in it rather than an
 * empty cell that reads as "there was nothing here".
 */
const MEMBERS = Object.freeze([
  { key: 'at', member: 'at' },
  { key: 'actor', member: 'actor' },
  { key: 'effect', member: 'effect' },
  { key: 'verdict', member: 'verdict' },
  { key: 'path', member: 'path' },
]);

/**
 * Three weights, and every piece of type on this face is drawn at one of them
 * (Owner #348 (4)).
 *
 * The rule the shared layer already states for a stat band -- the number is what the
 * eye lands on and the word beside it is support -- was true of the band and of nothing
 * else here: every other figure on the screen was set at the same weight as the prose
 * around it, so a reader looking for a count had to read a sentence to find one. These
 * three are what that rule looks like when it is applied mechanically rather than at
 * whichever call site happened to think of it: a figure is heavy, the word naming a
 * figure is a step above the body it sits in, and everything else is body. A fourth
 * weight would be one nobody could name, which is the same argument the corner scale
 * makes about a fourth radius.
 */
const WEIGHT = Object.freeze({ figure: '700', label: '500', body: '400' });

/**
 * The five figures at the head of the screen, in the order they are drawn.
 *
 * Owner #340 read this face as hard to take in at a glance, and the reading was right:
 * what opened the screen was a sentence, and a reader had to count rows to learn how
 * many there were of anything. These five are the answer -- the size of each half, and
 * the shape of the half that has happened.
 *
 * Every one of them is counted from the rows this render is about to draw. None is a
 * constant, and none is dropped for being zero: "none of these arrived" is a
 * measurement, and a half that could not be read states neither a number nor a zero but
 * a dash, which is the same distinction this whole face is built around.
 *
 * `half` names the population the figure is counted from and `verdict` narrows it to
 * one of the engine's three words. The two halves take their marks from the parts'
 * HALVES rather than choosing a second drawing for a meaning that already has one
 * (C-5); the three verdicts name their own, which are the marks the rows themselves
 * already wear.
 *
 * The three verdict figures are labelled with the engine's own words rather than with
 * an English adjective made out of each (Owner #348 (4), the redundant-word pass). The
 * screen was teaching two vocabularies for one set of facts: every row badge said
 * `Admit`, and the figure counting those rows said `admitted`. One word per fact. It
 * also happened to be the only way to fit the label -- measured at this application's
 * own narrow viewport, `escalated` overran its column by 8px and drew as `ESCALA...`,
 * which is a label that has stopped being a label.
 */
const BAND = Object.freeze([
  {
    noun: 'settled',
    half: 'settled',
    said: 'rows drawn on the settled half. The line above states them against the number that arrived.',
  },
  {
    noun: 'admit',
    half: 'settled',
    verdict: 'Admit',
    mark: ['verdict', 'Admit'],
    said: 'settled rows the engine answered Admit for.',
  },
  {
    noun: 'deny',
    half: 'settled',
    verdict: 'Deny',
    mark: ['verdict', 'Deny'],
    said: 'settled rows the engine answered Deny for.',
  },
  {
    noun: 'escalate',
    half: 'settled',
    verdict: 'Escalate',
    mark: ['verdict', 'Escalate'],
    said: 'settled rows the engine answered Escalate for. A row carrying a word that is not one of the three is counted under none of these and wears that word on its own line.',
  },
  {
    noun: 'held',
    half: 'held',
    said: 'rows on the held half. Nothing here has happened, so none of the three figures beside this one counts any of them.',
  },
]);

const isScalar = (value) => typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean';

/**
 * The clock this face reports its own cost from, or null where there is not one.
 *
 * A figure that was not measured is drawn as a dash by the strip at the foot, never as
 * a zero -- so a host with no clock on it says it did not measure, rather than saying
 * the work took no time.
 */
const clock = () => (typeof performance === 'object' && typeof performance?.now === 'function' ? performance.now() : null);

/** A caller that cannot reach a method the declaration does not hold. */
export function callerFor(port, allowed = CONSUMES) {
  const allow = new Set(allowed);
  const guard = (name) => {
    if (!allow.has(name)) throw new Error(`${LEDGER_MESSAGES.UNDECLARED}: ${name}`);
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
 * One item, as a row. Non-objects are handed back untouched so the order can drop them
 * with the reason it has for them rather than being handed a shape that hides it.
 */
export function toRecord(item, half) {
  if (item === null || typeof item !== 'object' || Array.isArray(item)) return item;
  const holes = {};
  const cells = {};
  for (const member of MEMBERS) {
    const value = item[member.member];
    if (value === undefined || value === null || value === '') holes[member.key] = LEDGER_MESSAGES.MEMBER_ABSENT;
    else if (!isScalar(value)) holes[member.key] = `${LEDGER_MESSAGES.MEMBER_NOT_SCALAR}: ${member.member}`;
    else cells[member.key] = String(value);
  }
  if (half === HALF.held) holes.seal = LEDGER_MESSAGES.NOTHING_TO_SEAL;
  return Object.freeze({
    ...cells,
    ...(isScalar(item.id) && String(item.id) !== '' ? { id: String(item.id) } : {}),
    ...(Number.isInteger(item.sequence) ? { n: item.sequence } : {}),
    ...(isScalar(item.digest) ? { digest: String(item.digest) } : {}),
    ...(isScalar(item.basis) ? { basis: String(item.basis) } : {}),
    ...(isScalar(item.undo_of) && String(item.undo_of) !== '' ? { childOf: String(item.undo_of) } : {}),
    prev: item.prev ?? null,
    // A row's place in its own life, read straight off which half it was read from --
    // not a decision this function makes, a fact it already has (composition A's
    // lifecycle cell, parts/src/receipt-row.mjs).
    lifecycle: half === HALF.held ? 'held' : 'settled',
    holes: Object.freeze(holes),
  });
}

/**
 * The row an identity names, and which half it is on, built the way the screen builds
 * it.
 *
 * A press carries an identity and nothing else -- an attribute on a control is a string
 * -- so anything a handler needs about the row it names has to be found again from the
 * state at the moment the press is dealt with, not from what was true when the control
 * was drawn. That is the same rule the act queue holds for the act log, applied to the
 * row: read the state inside the step, never the state the drawing was made from.
 */
export function locate(state, id) {
  for (const key of [HALF.settled, HALF.held]) {
    const envelope = state?.[key];
    if (!envelope || envelope.outcome !== ANSWERED) continue;
    for (const item of Array.isArray(envelope.items) ? envelope.items : []) {
      const record = toRecord(item, key);
      if (record && typeof record === 'object' && record.id === id) return { record, half: key };
    }
  }
  return null;
}

export function createFace({ parts = defaultParts } = {}) {
  const P = parts;
  const { el, style, toHtml, find } = P.element;
  const T = P.tokens;

  /**
   * Which cells hold a value a reader can take, and which member the whole of that
   * value is on (Owner #348 (2), "copy value on a data cell").
   *
   * Derived from the columns this face actually draws, not written out by hand. Written
   * by hand it named two cells this face has never drawn -- it takes a reduced scan line
   * with `actor` and `fingerprint` cut out of it -- so the menu carried two entries no
   * pointer on this screen could ever reach. A list of what a reader can point at, that
   * disagrees with what is on the screen, is a list that will disagree again.
   *
   * The cell and the member are kept as two names even though, for the four columns
   * this face draws, they are the same word today -- the one pair that differs
   * (`fingerprint` and `digest`) is on a column this scan line cuts. What is actually
   * load-bearing is the other distinction: the value copied is the member off the
   * record and never the text the cell drew, because the `at` column draws a declared
   * cut of an ISO-8601 timestamp and a copy of what it says would hand back something
   * that is not the value, quietly. The columns that draw furniture rather than data
   * (`lifecycle`, `seal`) have no member and fall out of the filter.
   */
  const COPYABLE = Object.freeze(
    P.scanColumnsSealed
      .map((column) => ({
        cell: column.key,
        from: (MEMBERS.find((member) => member.key === column.key) ?? {}).member ?? null,
      }))
      .filter((entry) => entry.from !== null),
  );

  // -- small pieces of type ---------------------------------------------------

  /**
   * One paragraph shape, and the two roles this face has for it (Owner #349 (3)).
   *
   * These were two functions that differed in a colour and two pixels of margin, which
   * is a duplication rather than a distinction: every rule about how a sentence on this
   * face breaks, sizes and weighs had to be written twice and could drift once. It is
   * one builder now, and `plain` and `aside` are the two roles that call it -- the names
   * stay because a call site reading `aside(...)` says which of the two it meant, and
   * `line(words, role, T.attendant, '6px')` at fifty call sites would not.
   *
   * `break-word`, not `anywhere` (Owner #348 (4)): both let a word too long for the
   * line be broken, and only `anywhere` lets a word that would have fitted be broken
   * anyway. What that produced is on the last capture -- `repor / t.md` across two
   * lines, and a `d` alone on a line of its own. A word is broken here only when it
   * cannot be set whole.
   */
  const line = (words, role, tone, gap) => el('p', {
    'data-role': role,
    style: style({
      margin: `0 0 ${gap}`,
      color: tone,
      'font-family': T.sans,
      'font-size': T.record,
      'line-height': T.recordLine,
      'font-weight': WEIGHT.body,
      'overflow-wrap': 'break-word',
    }),
  }, [words]);

  const aside = (words, role = 'aside') => line(words, role, T.attendant, '6px');

  const plain = (words, role = 'line') => line(words, role, T.ink, '4px');

  /**
   * One label/value row, and the four tables on this face that are made of them.
   *
   * The legend's mark tally, the legend's prose, the legend's not-drawn set and the
   * omitted section's own list were four copies of the same grid, three columns or two,
   * each spelling its own font, its own padding and its own gap. They are one shape
   * with a column track passed in, so a change to how a label sits beside its value is
   * one edit rather than four -- and the label takes the label weight in all four,
   * which is the thing that could not be true while there were four of them.
   */
  const NAME_TRACK = 'minmax(0,9rem) minmax(0,1fr)';
  const COUNT_TRACK = 'minmax(0,9rem) 2.5rem minmax(0,1fr)';
  const gridLine = (attrs, track, cells) => el('div', {
    ...attrs,
    style: style({
      display: 'grid',
      'grid-template-columns': track,
      gap: '10px',
      padding: '2px 0',
      'font-family': T.sans,
      'font-size': T.record,
      'line-height': T.recordLine,
    }),
  }, cells);

  const nameCell = (words) => el('span', {
    style: style({ color: T.attendant, 'font-weight': WEIGHT.label }),
  }, [words]);

  const valueCell = (words) => el('span', {
    style: style({ color: T.ink, 'font-weight': WEIGHT.body, 'overflow-wrap': 'break-word' }),
  }, [words]);

  /** A number in a table of numbers, drawn as one. */
  const countCell = (words) => el('span', {
    style: style({ color: T.ink, 'font-family': T.mono, 'font-weight': WEIGHT.figure }),
  }, [words]);

  /**
   * A block of explanation that carries no per-render data, folded shut by default
   * behind a one-word label (Owner #284, req/38 SS549): data first, zero preamble.
   * Native <details> rather than a bespoke toggle -- open/shut is free and
   * keyboard-reachable.
   *
   * Whether it is open is carried in this window's own state and not left to the
   * element (req/103 finding 2). It used to be left to the element, and every repaint
   * destroyed the element: a reader who opened this to find out how the rows were
   * ordered, then chose a row, watched it shut for no reason they could see. Which
   * disclosure a reader has open is a decision this window made, exactly as which row
   * is the pane's subject is, and it survives a repaint for the same reason.
   */
  const peripheral = (key, word, node, open = false) => el('details', {
    'data-role': 'peripheral',
    'data-peripheral': key,
    'data-open': String(Boolean(open)),
    open: open || null,
    style: style({ margin: '0 0 6px' }),
  }, [
    el('summary', {
      style: style({
        display: 'flex', 'align-items': 'center', 'min-height': '36px', 'box-sizing': 'border-box',
        color: T.attendant, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
        'font-weight': WEIGHT.label, cursor: 'default',
      }),
    }, [word]),
    node,
  ]);

  const section = (name, state, children) => el('section', {
    'data-section': name,
    'data-state': state,
    style: style({ padding: '14px 0', 'border-top': `1px solid ${T.rule}`, background: T.page }),
  }, children.filter(Boolean));

  // -- compact header + bordered one-row controls (SS657 retrofit, req/38 SS657
  // Owner #317/#318; idiom proven by faces/atlas). Two of the five seat-confirmed
  // defects lived here: "why"/"legend" as bare words in two full-width empty
  // bands (defect 2), and no compact header stating the screen's own counts. The
  // header is one line (face name + denominator); "why" and "legend" are drawn as
  // bordered, compact <details> controls side by side in one flex row, each
  // carrying a 2-3 word plain-language hint next to its label -- the same
  // controlToggle()/controlsRow() shape atlas.mjs proved first.

  /** A count, and the word it counts, at the two weights the rule asks for. A half
   * that was not read states the word it has instead of a figure, in the aside ink,
   * because a figure drawn heavy is a claim that something was counted. */
  const headerCount = (figure, noun) => [
    figure === null
      ? el('span', { style: style({ color: T.attendant, 'font-weight': WEIGHT.body }) }, ['unread'])
      : el('span', { style: style({ 'font-family': T.mono, 'font-weight': WEIGHT.figure, color: T.ink }) }, [figure]),
    el('span', { style: style({ color: T.attendant, 'font-weight': WEIGHT.body }) }, [` ${noun}`]),
  ];

  const headerLine = (counts) => el('div', {
    'data-role': 'face-header',
    style: style({
      display: 'flex', 'align-items': 'baseline', gap: '10px', padding: '10px 0 6px',
      'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
    }),
  }, [
    el('span', {
      style: style({
        'font-weight': WEIGHT.figure, 'font-size': T.head, 'line-height': T.headLine, color: T.ink,
      }),
    }, [FACE_ID]),
    el('span', {}, counts),
  ]);

  const controlToggle = (label, hint, body, { open = false } = {}) => el('details', {
    'data-role': 'control', 'data-control': label, 'data-open': String(Boolean(open)), open: open || null,
    style: style({ border: `1px solid ${T.rule}`, 'border-radius': T.radiusControl, background: T.page }),
  }, [
    el('summary', {
      // No dash between the label and its hint was already the rule here: two
      // characters and a space, six times over, said nothing the colour and the
      // weight beside them were not already saying -- and at this face's narrow
      // width the pair could take the end of a line on its own, which was the
      // orphan Owner #348 (4) named.
      //
      // req/822_c7 (Owner #387/#388 冗長文字全掃) goes further: the hint no longer
      // draws as its own visible span at all. `label` stays the default-visible
      // surface; `hint` rides the summary's own title (a hover) and a `data-hint`
      // attribute now.
      title: hint, 'data-hint': hint,
      style: style({
        display: 'flex', 'align-items': 'center', gap: '6px', 'min-height': '36px', 'box-sizing': 'border-box',
        padding: `0 ${T.padX}`, cursor: 'default', color: T.ink, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'list-style': 'none',
      }),
    }, [
      P.glyph('structure', open ? 'fold-open' : 'fold-shut', { size: P.minReadable, label: open ? 'open' : 'closed' }),
      el('span', { style: style({ 'font-weight': WEIGHT.label }) }, [label]),
    ]),
    el('div', { style: style({ padding: `0 ${T.padX} 10px` }) }, [body]),
  ]);

  const controlsRow = (children) => el('div', {
    'data-role': 'control-row',
    style: style({ display: 'flex', gap: '8px', 'flex-wrap': 'wrap', padding: '0 0 10px' }),
  }, children);

  /** Every declared mark, with a live count against what this render actually
   * drew -- zero-inclusive (req/768 F-B): a mark this render drew zero of still
   * gets a row, not an omission. `counts` is computed once in view() by walking
   * the content this screen is about to draw, before the legend that reports on
   * it is built. */
  const markTallyRows = (counts) => DECLARATION.marks.map((m) => gridLine(
    { 'data-mark-entry': m.mark, 'data-count': String(counts.get(m.mark) ?? 0) },
    COUNT_TRACK,
    [
      el('span', {
        style: style({
          color: T.ink, 'font-family': T.mono, 'font-size': T.time, 'font-weight': WEIGHT.label,
        }),
      }, [m.mark]),
      countCell(String(counts.get(m.mark) ?? 0)),
      valueCell(m.from),
    ],
  ));

  /** The counted table's own not-drawn rows: what this face's declared UNDRAWN
   * set says it will never draw, and why -- in the same legend a reader already
   * opened to learn what the marks mean, not only at the bottom of the screen. */
  const notDrawnLegendRows = () => UNDRAWN.map((entry) => gridLine(
    { 'data-not-drawn': entry.what },
    COUNT_TRACK,
    [nameCell('not drawn'), el('span', {}, ['']), valueCell(`${entry.what} -- ${entry.why}`)],
  ));

  function legendBody(counts) {
    return el('div', { 'data-role': 'legend' }, [
      el('div', { 'data-role': 'legend-marks' }, markTallyRows(counts)),
      el('div', { 'data-role': 'legend-prose' }, LEGEND_LINES.map((entry) => gridLine(
        { 'data-legend-entry': entry.name },
        NAME_TRACK,
        [nameCell(entry.name), valueCell(entry.value)],
      ))),
      el('div', { 'data-role': 'legend-not-drawn' }, notDrawnLegendRows()),
    ]);
  }

  // -- rows -------------------------------------------------------------------

  /**
   * What five lines used to say on every open row, said once instead. The
   * composition-A repair (req/09 SS528, req/97 RC-2): the framing prose under "how it
   * was cut", "not a proof" and the default "no verifier" seal claim is the same
   * words on every row -- about 120 of them -- while the only thing that actually
   * changes row to row is a number. Moving the prose here and leaving the numbers in
   * the row keeps every value a reader could need (clippedWithoutFull stays 0)
   * without asking them to read the same five sentences again for the tenth row in a
   * row.
   */
  const LEGEND_LINES = Object.freeze([
    { name: 'fingerprint column', value: 'the first 6 characters of the digest this delta left, upper-cased. Two different digests can begin the same way, so a match there is a hint and not a proof -- the note under a row carries the digest in full so the whole of it can be compared by hand.' },
    { name: 'checkable elsewhere', value: 'what a third party would need in hand to check a record without asking this window: the digest, the algorithm, and the anchor. A row states which of those it is missing, if any.' },
    { name: 'seal claim (default)', value: `${P.sealMessages.NO_VERIFIER}. A row only repeats this line when its own claim differs from that default -- when its basis was not an exact comparison.` },
    { name: 'held', value: 'this half has not happened yet, so nothing on it has anything to check.' },
    { name: 'the time column', value: P.rowMessages.AT_FORM },
    // Not "the pane on the right". The pane wraps below the list whenever the box it
    // is in is narrower than the two stated minimums, which at this application's own
    // narrow viewport is every time -- so on the shot a reader is most likely looking
    // at, "on the right" named a place that was not there. The same sentence in the
    // shared layer was corrected for the same reason at 821fc95.
    { name: 'what the pane holds', value: LEDGER_MESSAGES.NOTE_SUMMARY },
    { name: 'open because: clipped', value: LEDGER_MESSAGES.CLIP_RISK },
    // The name of this line used to carry a requirement number. A reader of this
    // screen has no requirement register to look it up in, and a face that prints
    // one is printing the name of a layer of this project on a product surface.
    { name: 'undo availability chip', value: '"reversed" -- a later row in this same read names this one as its predecessor, so its own escrowed inverse was already used. "unknown" -- the membrane does not yet expose whether the escrowed inverse still exists (a declared hole, not a guess: this face never calls the undo route just to find out). "n/a" -- this row is a held candidate, so nothing has happened yet and there is no inverse to hold.' },
  ]);

  /**
   * A row is one fixed-pitch line and clips what does not fit, so every value it holds
   * is repeated here in full. A value that is only ever shown cut off is a record that
   * has gone quiet, which is the one failure this product cannot have. What differs
   * from the same fact restated in prose lives here; what is the same on every row
   * lives once, in the legend (LEGEND_LINES).
   */
  function noteLines(record, claim, reversal) {
    const lines = Object.entries(record.holes ?? {}).map(([key, why]) => ({ name: key, value: why }));
    for (const member of MEMBERS) {
      if (record[member.key] !== undefined) lines.push({ name: `${member.key} in full`, value: record[member.key] });
    }
    if (record.childOf) lines.push({ name: 'written under', value: record.childOf });
    if (record.digest) lines.push({ name: 'digest in full', value: record.digest });
    lines.push({ name: 'checkable elsewhere', value: P.portability(record).why });
    // Only where the claim differs from the default every settled row would otherwise
    // repeat (no verifier is present -- stated once, in the legend). A held row never
    // reaches here: it already carries a line saying nothing has happened yet.
    if (!record.holes?.seal && claim.basis !== 'exact') lines.push({ name: 'seal claim', value: claim.why });
    // req/768 AC-7: the general meaning of the three chip states is stated once,
    // in the legend (LEGEND_LINES below) -- this row-specific line only fires for
    // "reversed", the one state whose full text names a row unique to this one
    // (the id of the row that reversed it), the same "only where it differs from
    // the default" discipline the seal-claim line just above already holds.
    if (reversal?.state === 'reversed') lines.push({ name: 'undo availability', value: reversal.why });
    return lines;
  }

  /**
   * How many characters a column can show before it starts cutting them off. Derived
   * from the column's declared width rather than guessed at once for the whole row:
   * the first version of this used a single length for every column and missed the
   * narrowest one entirely, which the renderer then reported as four values shown cut
   * off with the whole of them nowhere on the page. The character width is a guess and
   * is named as one; the reading that settles it is clippedWithoutFull in
   * tools/shoot.mjs, and it is held at zero.
   */
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
   * The acts a row offers, and whether each one can be used, worked out once.
   *
   * Owner #348 (2) puts a second control surface on every row. The first thing a second
   * surface can get wrong is offering a different set of acts from the first, so it is
   * not allowed to work one out: this is the only place on this face that turns the
   * declaration into a list of offers, and the gutter and the menu are both handed what
   * it returns. An act dimmed in the gutter is dimmed in the menu, with the same
   * sentence, because there is one answer rather than two that agree today.
   *
   * Two different facts make an act unavailable and they keep their own words. An act
   * the declaration withholds is one this window will never put on a socket; an act in
   * flight is one it is waiting on. Neither is ever dropped from the list: an act that
   * disappears when it cannot be used is indistinguishable from one that was never
   * offered (req/768 AC-4), and that is true on a menu as well as in a gutter.
   *
   * It is a copy made for drawing. The act this face would actually send is read from
   * the declaration again by act(), so nothing here can make one sendable that is not.
   */
  const offeredActs = (record, half, sending) => ACTS
    .filter((spec) => spec.half === half)
    .map((spec) => (sending && sending.act === spec.act && sending.id === record.id
      ? { ...spec, sends: false, why: LEDGER_MESSAGES.IN_FLIGHT }
      : spec));

  /**
   * What a copy item would actually put on the clipboard, or the reason there is
   * nothing to put there.
   *
   * Never what the cell says. Two of the columns a reader can point at draw something
   * shorter than the fact they are about -- the time column draws a declared cut of an
   * ISO-8601 timestamp, the fingerprint column draws the first six characters of a
   * digest -- so a copy that took the drawn text would hand back a value that is not
   * the value, silently, which is this face's one unforgivable failure wearing a
   * convenience feature's coat. The member is copied, in full, and the menu says which
   * member it is copying.
   */
  function copyOffer(record, cellKey) {
    const spec = COPYABLE.find((entry) => entry.cell === cellKey);
    if (!spec) return null;
    const why = record.holes?.[spec.from];
    if (why) return { cell: spec.cell, from: spec.from, value: null, why };
    const value = record[spec.from];
    if (value === undefined || value === null || value === '') {
      return { cell: spec.cell, from: spec.from, value: null, why: LEDGER_MESSAGES.MENU_NO_VALUE };
    }
    return { cell: spec.cell, from: spec.from, value: String(value), why: null };
  }

  /**
   * One item in the menu, and the shape every item has: a mark where the vocabulary
   * has one, a word, and -- when it cannot be used -- the reason on the control itself.
   *
   * An act item carries `data-act` and `data-target`, which is the same pair the gutter's
   * own buttons carry, so the press goes down the identical branch of the identical
   * handler into the identical queue. That is not a coincidence to be maintained: it is
   * why a menu act cannot re-open the lost-update this face's own audit found (req/103
   * finding 1) -- there is no second path for it to be lost on.
   *
   * The colours are inline here and they are not inline in the gutter, which is the one
   * asymmetry in this file. parts/src/surface.mjs's operability rules are keyed to
   * `[data-part="act-gutter"] button`, so nothing in the rule set reaches a control this
   * face draws; naming this element a gutter to inherit them would be a face lying about
   * what it is. Nothing is being outranked here (37f7ff3's defect was inline styles
   * beating rules aimed at the same element, and no rule is aimed at this one), but the
   * hover and press states a rule set can express are missing, and that is stated in the
   * report rather than worked around.
   */
  const menuItem = ({
    kind, label, mark, available, why, attrs = {},
  }) => el('button', {
    type: 'button',
    'data-menu-item': kind,
    ...attrs,
    disabled: available ? null : true,
    title: available ? label : why,
    class: 'gx-move',
    style: style({
      font: 'inherit',
      display: 'flex',
      'align-items': 'center',
      gap: '8px',
      width: '100%',
      'box-sizing': 'border-box',
      'text-align': 'left',
      'min-height': '36px',
      padding: `6px ${T.padX}`,
      background: T.page,
      border: `1px solid ${available ? T.act : T.rule}`,
      'border-radius': T.radiusControl,
      color: available ? T.act : T.attendant,
      cursor: available ? 'pointer' : 'not-allowed',
      'font-family': T.sans,
      'font-size': T.record,
      'line-height': T.recordLine,
      'font-weight': WEIGHT.label,
    }),
  }, [mark, el('span', {}, [label])].filter(Boolean));

  /**
   * The menu a right-click opens on a row (Owner #348 (2)).
   *
   * Everything about it is a function of state and of the declaration, and that is what
   * makes the three properties the directive asks for hold by construction rather than
   * by care. It cannot be left behind by a repaint, because a repaint draws the whole
   * screen from this state and a menu is in the state or it is not. A second right-click
   * cannot stack a second menu, because the state holds one menu and not a list. And it
   * offers no act the row does not send, because it does not have its own act list.
   */
  function rowMenu(record, half, cellKey, sending) {
    const acts = offeredActs(record, half, sending);
    const copy = copyOffer(record, cellKey);
    return el('div', {
      'data-role': 'row-menu',
      'data-menu-row': record.id,
      'data-menu-cell': copy ? copy.cell : null,
      'data-count': String(acts.length + (copy ? 1 : 0)),
      style: style({
        display: 'flex',
        'flex-direction': 'column',
        gap: '4px',
        width: 'max-content',
        'max-width': '100%',
        margin: '2px 0 6px',
        padding: '6px',
        'box-sizing': 'border-box',
        border: `1px solid ${T.rule}`,
        'border-radius': T.radiusContainer,
        background: T.page,
      }),
    }, [
      el('div', {
        'data-role': 'menu-subject',
        style: style({
          'font-family': T.mono,
          'font-size': T.time,
          'line-height': T.timeLine,
          'font-weight': WEIGHT.label,
          color: T.attendant,
          padding: `0 ${T.padX} 2px`,
        }),
      }, [copy ? `${record.id} ${copy.from}` : record.id]),
      ...acts.map((spec) => menuItem({
        kind: 'act',
        label: spec.label,
        // An act's mark takes the higher of the two floors, and a menu item is as much
        // the thing a hand is aimed at as a gutter button is.
        mark: P.glyph('act', spec.act, { size: P.minAct, label: spec.label }),
        available: spec.sends,
        why: spec.why,
        attrs: { 'data-act': spec.act, 'data-target': record.id },
      })),
      copy
        ? menuItem({
          kind: 'copy',
          label: `copy ${copy.from}`,
          // No mark. The sheet of record has no drawing for this meaning, and a face
          // that reached for the nearest existing one would be teaching a reader that
          // mark means two things (C-5). The seat is asked for an `act/copy` in the
          // report; until there is one, this control is a word.
          mark: null,
          available: copy.value !== null,
          why: copy.why,
          attrs: { 'data-copy-from': copy.from, 'data-target': record.id },
        })
        : null,
    ].filter(Boolean));
  }

  /**
   * Whether the last copy reached the clipboard, said on the screen.
   *
   * A control that looks identical whether or not it did anything is the shape the
   * shell's own copy control already refuses, and this borrows its vocabulary exactly
   * (`data-copied` / `data-copy-failed`) so one reading answers for both. There is no
   * timer that clears it: a face here repaints when something happens, and a repaint
   * nobody asked for to un-say something true is not something this application does.
   */
  function copyReport(copied) {
    if (!copied) return null;
    const done = copied.state === 'copied';
    const failed = copied.state === 'refused';
    return el('p', {
      'data-role': 'copy-report',
      'data-copied': done ? copied.from : null,
      'data-copy-failed': failed ? copied.from : null,
      title: copied.why,
      style: style({
        margin: '0 0 6px',
        // Not the deny ink, though this is a failure. The sheet of record spends that
        // colour on one thing -- a verdict of Deny -- and a clipboard that would not
        // take a value is not a decision anybody made about a record. The sentence
        // says what happened, which is what a reader needs and what a colour cannot
        // say precisely enough to be worth the second meaning.
        color: T.attendant,
        'font-family': T.sans,
        'font-size': T.record,
        'line-height': T.recordLine,
        'font-weight': WEIGHT.body,
        'overflow-wrap': 'break-word',
      }),
    }, [done ? `${LEDGER_MESSAGES.COPIED} ${copied.from}` : copied.why]);
  }

  /**
   * req/768 AC-4/AC-7 (retrofit round 2): the row's own offered acts, drawn as a
   * fixed-width right gutter by parts/src/receipt-row.mjs's openableRow() rather
   * than a full-width strip beneath the row (the shape this face drew before this
   * round); and the reversibility chip, decided once here by reversalOf() (never
   * inside a drawing part -- req/768 AC-P0's own division) against `siblings`,
   * which is this same read's own rows and never a second, live fetch.
   */
  function rowBlock(record, half, siblings, selected, sending, menu) {
    const claim = P.claimOf(record, { verifier: null });
    const reversal = P.reversalOf(record, siblings);
    const offered = offeredActs(record, half, sending);
    // The sibling half of the force-open defect (req/97 gap-list item 1, found by
    // sweeping the predicate rather than the one row the shot showed). toRecord()
    // gives every *held* row a `seal` hole by construction -- nothing has happened
    // yet, so there is nothing to check -- so counting that hole as a reason would
    // have marked every held row for ever, exactly the way the `at` column did.
    // faces/held excludes it for this reason and has since it was written.
    const holes = Object.keys(record.holes ?? {}).filter((key) => !(half === HALF.held && key === 'seal'));
    // Measured against what the row will actually draw, not against what arrived
    // (req/97 gap-list item 1): the `at` column draws a declared cut, so an ISO-8601
    // timestamp is not a value this row shows cut off. Since Owner directive #335 (3)
    // this no longer decides whether anything opens -- nothing opens under a row at
    // all -- it decides only whether the row is *marked* as one whose cells the grid
    // cannot hold, which is a fact about the row worth stating on it.
    const tight = MEMBERS.some((member) => P.drawnTextFor(member.key, record[member.key]).length > (BUDGETS[member.key] ?? 40));
    const lines = noteLines(record, claim, reversal);
    return el('div', {
      'data-role': 'row-block',
      'data-half': half,
      'data-open-because': holes.length > 0 ? 'declared-hole' : (tight ? 'clip-risk' : null),
      style: style({ display: 'block' }),
    }, [
      P.selectableRow(record, {
        claim,
        reversal,
        acts: offered,
        fields: lines.length,
        selected: selected === record.id,
        // The seal column stays on this face scan line: whether a record can be
        // checked without us tells a held row from a settled one here, and it is this
        // face own question rather than a detail of one row (SCAN_COLUMNS_SEALED).
        columns: P.scanColumnsSealed,
      }),
      // The menu, when this row is the one a reader asked. It is a sibling underneath
      // the row and not an overlay: the one defect this package has actually shipped
      // (req/03 N-1) was an out-of-flow element drawn over a row, and the gate that
      // came out of it -- `nothing-out-of-flow` in tools/gate.mjs -- forbids this face
      // from positioning anything. A menu pinned to the pointer would have to be
      // positioned, would overlap whatever it covered, and would be invisible to the
      // overlap reading every capture of this face is checked with. In flow it is
      // measurable, it is reachable by keyboard in the order it is read, and the rows
      // below it move down rather than being hidden behind it.
      menu && menu.id === record.id ? rowMenu(record, half, menu.cell, sending) : null,
    ].filter(Boolean));
  }

  /** The one pane this screen stores a row's detail in (Owner directive #335, 3). */
  function paneFor(selected, halves) {
    for (const [half, side] of halves) {
      const rows = side.ordered ? side.ordered.rows : [];
      const record = rows.find((r) => r.id === selected);
      if (!record) continue;
      const claim = P.claimOf(record, { verifier: null });
      const reversal = P.reversalOf(record, rows);
      return P.detailPane({
        subject: record.id,
        lines: [{ name: 'half', value: half }, ...noteLines(record, claim, reversal)],
      });
    }
    return P.detailPane({});
  }

  // -- halves -----------------------------------------------------------------

  function outcomeLines(envelope) {
    const lines = [plain(`outcome: ${envelope?.outcome ?? 'nothing came back at all'}`, 'outcome')];
    if (envelope?.outcome === 'refused') {
      lines.push(plain(`${envelope.problem?.title ?? ''}: ${envelope.problem?.detail ?? ''}`, 'refusal'));
      lines.push(plain(`code: ${envelope.gx_code ?? envelope.problem?.gx_code ?? 'none'}`, 'code'));
      lines.push(plain(`status: ${envelope.status ?? 'none'}`, 'status'));
    }
    if (envelope?.outcome === 'failed') lines.push(plain(`${envelope.reason}: ${envelope.detail ?? ''}`, 'failure'));
    if (envelope?.outcome === 'absent') lines.push(plain(`${envelope.reason}: ${JSON.stringify(envelope.requested ?? null)}`, 'absence'));
    return lines;
  }

  /**
   * Every group on this screen that sits in the open, drawn as an object with an edge
   * rather than as a heading with rows loose underneath it (Owner #340, the reference
   * tool's box idiom in this face's own terms). One shape, three call sites: the two
   * halves and the log of what this window has sent.
   *
   * The head carries the group's name, its own count and the word that count is in --
   * records for the half that has happened, candidates for the half that has not,
   * because counting both in one word is the exact confusion this face exists to
   * prevent. Only the half that has not happened wears a standing: nothing on the other
   * one has been checked by anybody in this window, so a pill there would be a claim.
   *
   * The section wrapper stays, carrying the same name and the same state it always did.
   * A box is a drawing; whether a group was read, was empty, or was drawn is a fact
   * about the read, and the two are not the same thing.
   */
  function boxSection({
    name, state: state2, count, noun, said = null, pill = null, children,
  }) {
    return el('section', {
      'data-section': name,
      'data-state': state2,
      style: style({ display: 'block' }),
    }, [
      P.box({
        name, count, noun, said, pill, children,
      }),
    ]);
  }

  /** The word a count is in. One of a thing takes the singular: the container part
   * prints the word it is handed and has no opinion about grammar, so "1 records" is
   * the caller's to not write. */
  const nounFor = (count, one, many) => (count === 1 ? one : many);

  const halfBox = (key, state2, count, children) => boxSection({
    name: key,
    state: state2,
    count,
    noun: key === HALF.settled ? nounFor(count, 'record', 'records') : nounFor(count, 'candidate', 'candidates'),
    said: key === HALF.settled ? LEDGER_MESSAGES.BOX_SETTLED : LEDGER_MESSAGES.BOX_HELD,
    pill: key === HALF.held
      ? P.chip('standing', 'held', { size: P.minReadable, word: 'held', said: LEDGER_MESSAGES.BOX_HELD })
      : null,
    children,
  });

  /** Everything in a box that is not a row. A row is drawn edge to edge so that a
   * pointer moving down the list is tracked across the whole box; a sentence is set in
   * from the edge, where it lines up with the box's own head. */
  const inset = (children) => el('div', {
    style: style({ padding: `8px ${T.padX}` }),
  }, children.filter(Boolean));

  /** Whether a reader has this disclosure open. One list, held in the window's own
   * state, read by every disclosure on the screen (req/103 finding 2). */
  const opened = (state, key) => (Array.isArray(state.opened) ? state.opened : []).includes(key);

  /** Rows of one half, plus everything a reader needs to know about the walk. */
  function halfOf(state, key, selected) {
    const envelope = state[key];
    const emptiness = key === HALF.settled ? LEDGER_MESSAGES.EMPTY_SETTLED : LEDGER_MESSAGES.EMPTY_HELD;

    if (!envelope || envelope.outcome !== ANSWERED) {
      return {
        node: halfBox(key, 'unread', null, [
          inset([
            aside(LEDGER_MESSAGES.UNREAD, 'unread'),
            ...outcomeLines(envelope),
          ]),
        ]),
        received: 0,
        ordered: null,
      };
    }

    const items = Array.isArray(envelope.items) ? envelope.items : [];
    const ordered = P.order(items.map((item) => toRecord(item, key)), { by: ROWS.order });
    const state2 = items.length === 0 ? 'empty' : (ordered.rows.length === 0 ? 'all-dropped' : 'drawn');

    // Dense doctrine (SS558/D-1, relayed req/38 SS576): the first table a reader
    // meets in a section is the data, not a paragraph about it. The order/reason
    // sentences used to sit between the heading and the rows -- read first,
    // pushing every row down a screen's worth on a loaded ledger. They still say
    // exactly what they said (C-6 only requires the text be present on the
    // section, not where), just folded behind their own one-line label after the
    // rows, the same disclosure pattern `legendSection()` already uses.
    const orderDetail = el('div', { 'data-role': 'order-detail' }, [
      plain(`requested: ${ordered.requested}, applied: ${ordered.by}`, 'order'),
      aside(ROWS.order_reason, 'order-reason'),
      ordered.substituted ? plain(ordered.reason, 'order-substituted') : null,
      key === HALF.settled ? aside(LEDGER_MESSAGES.NO_VERIFIER_HERE, 'verifier') : null,
    ].filter(Boolean));
    return {
      node: halfBox(key, state2, ordered.rows.length, [
        state2 === 'empty' ? inset([aside(emptiness, 'empty')]) : null,
        state2 === 'all-dropped' ? inset([aside(LEDGER_MESSAGES.ALL_DROPPED, 'all-dropped')]) : null,
        ordered.rows.length > 0
          // Six pixels, and the reason for them is in the picture: a row runs the full
          // width of its box so that the band under a pointer is the whole row, but the
          // act gutter is the last thing on that line, and flush against the box's own
          // edge its buttons lose their right border and read as cut off.
          ? el('div', {
            'data-role': 'rows',
            'data-count': String(ordered.rows.length),
            style: style({ padding: '0 6px' }),
          }, ordered.rows.map((record) => rowBlock(record, key, ordered.rows, selected, state.sending ?? null, state.menu ?? null)))
          : null,
        inset([peripheral(`${key} order`, `order: ${ordered.by}`, orderDetail, opened(state, `${key} order`))]),
      ]),
      received: items.length,
      ordered,
      envelope,
    };
  }

  /**
   * The band, built from the state this render is about to draw.
   *
   * Every figure here is a count of rows in hand, and a half that did not answer gives
   * null rather than nought all the way through -- so a screen whose settled list never
   * arrived reads as four dashes and a number, which is a true sentence, instead of as
   * a ledger in which nothing has ever happened, which is not.
   */
  function bandCount(entry, rows) {
    if (rows === null) return null;
    if (!entry.verdict) return rows.length;
    return rows.filter((record) => record.verdict === entry.verdict).length;
  }

  function bandOf(settled, held) {
    return P.statBand(BAND.map((entry) => {
      const side = entry.half === HALF.held ? held : settled;
      const spec = P.halves.find((h) => h.key === entry.half);
      const [namespace, key] = entry.mark ?? spec.mark;
      const rows = side.ordered ? side.ordered.rows : null;
      return {
        noun: entry.noun,
        count: bandCount(entry, rows),
        // The floor, asked for by name, and not a number chosen beside a 22px figure:
        // 18 was picked by eye and is exactly the drift the two named floors exist to
        // stop. It also buys the escalate label the 2px it was overrunning by.
        mark: P.glyph(namespace, key, { size: P.minReadable, label: entry.noun }),
        // The ink a mark owns, asked for by the mark and never spelled here. A mark
        // with no standing of its own hands back the ordinary ink, which is what the
        // figure would have been drawn in anyway.
        tone: P.inkFor(P.markOf(namespace, key)),
        said: entry.said,
      };
    }));
  }

  // -- the sections that are about the screen rather than about a row ----------

  function claimsSection(settled) {
    const records = settled.ordered ? settled.ordered.rows : [];
    const claims = P.checkable(records, []);
    // No heading. Every one of these three sections is drawn inside a control whose
    // own label is the section's name, so the heading said the word the reader had
    // just pressed, one line under it (Owner #348 (4)).
    return section('claims', records.length === 0 ? 'no-records' : 'checked', [
      el('div', { 'data-role': 'claims', 'data-count': String(claims.length) }, claims.map((claim) => el('div', {
        'data-claim': claim.id,
        'data-holds': String(claim.holds),
        style: style({
          display: 'grid',
          'grid-template-columns': 'minmax(0,8rem) minmax(0,1fr)',
          gap: '10px',
          padding: '4px 0',
          'border-bottom': `1px solid ${T.rule}`,
          'font-family': T.sans,
          'font-size': T.record,
          'line-height': T.recordLine,
        }),
      }, [
        el('span', { 'data-role': 'verdict', style: style({ color: claim.holds ? T.ink : T.deny }) }, [claim.holds ? 'holds' : 'does not hold']),
        // Two blocks rather than two inline spans. A wrapped inline box reports a
        // rectangle that covers every line it touches, so two of them in a row read as
        // overlapping whether or not anything is drawn on top of anything -- and a
        // reading that cannot tell the difference is a reading that cannot be used on
        // the defect it exists for.
        el('div', {}, [
          el('div', { 'data-role': 'claim', style: style({ color: T.ink, 'overflow-wrap': 'break-word' }) }, [claim.claim]),
          el('div', { 'data-role': 'detail', style: style({ color: T.attendant, 'overflow-wrap': 'break-word' }) }, [claim.detail]),
        ]),
      ]))),
    ]);
  }

  function consistencySection(result) {
    const answeredBody = result?.outcome === ANSWERED ? (result.body ?? {}) : null;
    const members = answeredBody ? Object.entries(answeredBody) : [];
    return section('consistency', result?.outcome === ANSWERED ? 'answered' : 'unread', [
      aside(LEDGER_MESSAGES.NOT_VERIFICATION, 'not-verification'),
      ...(answeredBody
        ? [el('div', { 'data-role': 'members', 'data-count': String(members.length) }, members.map(([name, value]) => plain(`${name}: ${isScalar(value) ? String(value) : JSON.stringify(value)}`, 'member')))]
        : outcomeLines(result)),
    ]);
  }

  function notDrawnSection(settled, held) {
    const walks = [settled, held].filter((h) => h.envelope);
    const lines = [];
    for (const h of [settled, held]) {
      const key = h === settled ? HALF.settled : HALF.held;
      if (!h.ordered) {
        lines.push(plain(`${key}: nothing was drawn, because the list was not read`, 'denominator'));
        continue;
      }
      lines.push(plain(`${key}: ${h.ordered.rows.length} of ${h.received} rows drawn, read in ${h.envelope.requests ?? h.envelope.pages ?? 0} requests`, 'denominator'));
      if (h.envelope.stopped_at_budget) lines.push(plain(`${key}: ${LEDGER_MESSAGES.TRUNCATED}`, 'truncated'));
      if (h.envelope.repeated_cursor) lines.push(plain(`${key}: ${LEDGER_MESSAGES.REPEATED}`, 'repeated'));
      for (const drop of h.ordered.dropped) lines.push(plain(`${key}: an item at position ${drop.index} was not drawn, reason ${drop.why}`, 'dropped'));
      for (const repeat of h.ordered.repeated.id) lines.push(plain(`${key}: the identity ${repeat.value} arrived ${repeat.count} times`, 'repeated-id'));
    }
    return section('not-drawn', walks.length === 0 ? 'nothing-read' : 'stated', [
      el('div', { 'data-role': 'denominators' }, lines),
      el('div', { 'data-role': 'omissions' }, UNDRAWN.map((entry) => gridLine(
        { 'data-omission': entry.what },
        NAME_TRACK,
        [nameCell(entry.what), valueCell(entry.why)],
      ))),
      DECLARATION.withheld.length > 0
        ? el('div', { 'data-role': 'withheld' }, DECLARATION.withheld.map((entry) => plain(`${entry.act}: ${LEDGER_MESSAGES.WITHHELD}. ${entry.why}`, 'withheld-act')))
        : null,
    ]);
  }

  function provenanceFold(settled, held, acts) {
    const done = [];
    for (const [key, h] of [[HALF.settled, settled], [HALF.held, held]]) {
      if (!h.ordered) continue;
      done.push({ name: key, value: `${h.ordered.rows.length} rows over ${h.envelope.requests ?? h.envelope.pages ?? 0} requests` });
    }
    const pending = acts.filter((entry) => entry.outcome !== ANSWERED).map((entry) => ({
      name: `${entry.act} ${entry.id}`,
      value: `${entry.outcome}: ${entry.detail ?? ''}`,
    }));
    return P.fold({
      // The control this fold is drawn inside is labelled "where from" and hinted
      // "the provenance of this screen", so a summary saying the same thing a third
      // time is three sentences for one fact. It states what the two halves under it
      // are instead, which is the one thing neither of the other two says.
      summary: 'each half, and anything still waiting',
      settled: done,
      held: pending,
      open: false,
      size: P.minReadable,
    });
  }

  /** What this window has sent, and what came back. A group like any other, so it is
   * drawn like one: named, counted, and inside its own edge. */
  function actsSection(acts) {
    if (acts.length === 0) return null;
    return boxSection({
      name: 'acts',
      state: 'sent',
      count: acts.length,
      noun: nounFor(acts.length, 'entry', 'entries'),
      children: inset([
        el('div', { 'data-role': 'act-log', 'data-count': String(acts.length) }, acts.map((entry) => plain(
          `${entry.act} ${entry.id}: ${entry.outcome}${entry.code ? `, code ${entry.code}` : ''}${entry.detail ? `, ${entry.detail}` : ''}`,
          'act-entry',
        ))),
      ]),
    });
  }

  // -- the whole screen --------------------------------------------------------

  function frame(children) {
    return el('div', {
      'data-face': FACE_ID,
      'data-question': QUESTION,
      style: style({
        display: 'block',
        background: T.page,
        color: T.ink,
        'font-family': T.sans,
        'font-size': T.record,
        'line-height': T.recordLine,
        padding: `12px ${T.padX} 40px`,
      }),
    }, children.filter(Boolean));
  }

  function waitingView() {
    return frame([
      plain(LEDGER_MESSAGES.READING, 'reading'),
    ]);
  }

  /** One compact denominator line: how many of each half were drawn out of how
   * many were received. Two halves stated together, never one alone (ROWS.note's
   * own discipline, now also read at a glance before anything else on screen). */
  function headerWords(settled, held) {
    const figure = (side) => (side.ordered ? `${side.ordered.rows.length} of ${side.received}` : null);
    return [
      ...headerCount(figure(settled), 'settled'),
      el('span', { style: style({ color: T.attendant }) }, [', ']),
      ...headerCount(figure(held), 'held'),
    ];
  }

  /**
   * What this screen was drawn from, in the words of whoever knows.
   *
   * A caller that knows its state is a stand-in says so and is printed unedited -- the
   * fixtures this face is photographed from do exactly that, so a picture of this face
   * never claims an engine was on the other end of it. Where nobody said, the face
   * names what it believes it was talking to, and only if something actually answered:
   * a window where all three reads fell over drew nothing from anywhere, and says so
   * with a dash rather than naming a source it never reached.
   */
  function sourceOf(state) {
    if (typeof state.source === 'string' && state.source.trim() !== '') return state.source;
    const answered = [state.settled, state.held, state.consistency]
      .filter((envelope) => envelope?.outcome === ANSWERED);
    return answered.length > 0 ? LEDGER_MESSAGES.SOURCE_ENGINE : null;
  }

  function view(state) {
    const started = clock();
    const selected = state.selected ?? null;
    const settled = halfOf(state, HALF.settled, selected);
    const held = halfOf(state, HALF.held, selected);
    const acts = state.acts ?? [];
    const data = [settled.node, held.node, actsSection(acts)].filter(Boolean);
    const band = bandOf(settled, held);
    // Content is built first, then walked for its own data-mark tally, so the
    // legend can report a live, zero-inclusive count against what this render is
    // actually about to draw (req/768 F-B) -- the legend never counts itself.
    //
    // The band is counted with the rest. It is content and not chrome: it states this
    // screen's own population, and it draws a verdict's mark for a verdict this read
    // holds none of -- so a legend that skipped it would report zero for a mark a
    // reader can see, which is the one thing a counted legend must never do.
    const content = [
      band,
      settled.node,
      held.node,
      actsSection(acts),
      claimsSection(settled),
      consistencySection(state.consistency),
      notDrawnSection(settled, held),
      provenanceFold(settled, held, acts),
    ].filter(Boolean);
    const counts = new Map();
    for (const node of content) {
      for (const marked of find(node, (n) => n.attrs && 'data-mark' in n.attrs)) {
        counts.set(marked.attrs['data-mark'], (counts.get(marked.attrs['data-mark']) ?? 0) + 1);
      }
    }
    // Owner directive #335 (1): what is explanatory is behind a click, and the
    // clicks are in one compact row. Before this, "claims", "consistency" and
    // "omitted" were three always-open bands of prose below the rows -- about two
    // thirds of the window on a short read -- and a reader met them before they met
    // a second record. Nothing is removed: every one of them is still on this
    // screen, still says exactly what it said, and is one press away. The two halves
    // and the act log are the data, so they stay in the open (directive #335, 1's
    // own line: "anything *explanatory* lives behind a click").
    const drawn = [
      headerLine(headerWords(settled, held)),
      copyReport(state.copied ?? null),
      band,
      controlsRow([
        controlToggle('why', 'about this screen', aside(ORDER.reason, 'why-first'), { open: opened(state, 'why') }),
        controlToggle('legend', 'symbols and counts', legendBody(counts), { open: opened(state, 'legend') }),
        controlToggle('claims', 'what you can check', claimsSection(settled), { open: opened(state, 'claims') }),
        controlToggle('consistency', 'the engine on itself', consistencySection(state.consistency), { open: opened(state, 'consistency') }),
        controlToggle('omitted', 'what is not drawn', notDrawnSection(settled, held), { open: opened(state, 'omitted') }),
        controlToggle('where from', 'what answered, and when', provenanceFold(settled, held, acts), { open: opened(state, 'where from') }),
      ]),
      P.detailFrame(el('div', { 'data-role': 'halves' }, data), paneFor(selected, [[HALF.settled, settled], [HALF.held, held]])),
    ];
    // Taken here, before the strip that reports it is built: a figure that included
    // the drawing of itself would be a figure written after the thing it measured. It
    // is the time to build this screen's tree and nothing else -- no read, no paint --
    // and the strip says so by naming what it is rather than calling it a total.
    const renderMs = started === null ? null : clock() - started;
    return frame([...drawn, P.runtimeFooter({ renderMs, source: sourceOf(state) })]);
  }

  // -- reading and acting -------------------------------------------------------

  /**
   * The three reads, and what a fresh read is not allowed to throw away.
   *
   * What the server said is replaced wholesale, which is the point of reading again.
   * What this window decided -- which row the pane is describing, which disclosures the
   * reader has open, which row has a menu open on it, whether the last copy reached the
   * clipboard, what has been sent so far -- is carried across, because none of it is the
   * server's to overwrite (req/103 finding 2: an act used to close every panel on the
   * screen and forget which row was open, for no reason a reader could see). The menu
   * and the copy report join that list for the same reason and not as an afterthought:
   * a menu that a fresh read silently threw away would be the same defect with a newer
   * name on it.
   */
  async function read(port, previous = null) {
    const caller = callerFor(port);
    const settled = await caller.fold(READS.settled);
    const held = await caller.fold(READS.held);
    const consistency = await caller.invoke(READS.consistency);
    return {
      settled,
      held,
      consistency,
      acts: previous?.acts ?? [],
      selected: previous?.selected ?? null,
      opened: previous?.opened ?? [],
      menu: previous?.menu ?? null,
      copied: previous?.copied ?? null,
    };
  }

  function describeAct(result, spec, id) {
    const base = { act: spec.act, method: spec.method, id, outcome: result?.outcome ?? 'failed' };
    if (result?.outcome === 'refused') {
      return {
        ...base,
        code: result.gx_code ?? result.problem?.gx_code ?? null,
        detail: `${result.problem?.title ?? ''}: ${result.problem?.detail ?? ''}`,
      };
    }
    if (result?.outcome === 'failed') return { ...base, detail: `${result.reason}: ${result.detail ?? ''}` };
    if (result?.outcome === 'absent') return { ...base, detail: String(result.reason ?? '') };
    return { ...base, detail: `${LEDGER_MESSAGES.SENT}, status ${result?.status ?? 'none'}` };
  }

  /**
   * An act, and then a fresh read. The face does not write the outcome into a row: the
   * rows on the next screen are the rows the server has, which is the only way an undo
   * can appear as a child row rather than as an edit somebody's window invented.
   */
  async function act(port, state, { act: name, id }) {
    const spec = ACTS.find((candidate) => candidate.act === name);
    if (!spec) throw new Error(`${LEDGER_MESSAGES.UNKNOWN_ACT}: ${name}`);
    let entry;
    if (!spec.sends) {
      entry = { act: spec.act, method: spec.method, id, outcome: 'withheld', detail: spec.why };
    } else {
      const caller = callerFor(port);
      try {
        entry = describeAct(await caller.invoke(spec.method, { params: { id }, body: {} }), spec, id);
      } catch (error) {
        entry = { act: spec.act, method: spec.method, id, outcome: 'failed', detail: error.message };
      }
    }
    const next = await read(port, state);
    return { ...next, acts: [...(state.acts ?? []), entry] };
  }

  // -- mount ---------------------------------------------------------------------

  function mount(host, port, notices = []) {
    if (!host || typeof host.appendChild !== 'function') throw new TypeError(LEDGER_MESSAGES.NO_HOST);
    if (!port || typeof port.fold !== 'function') throw new TypeError(LEDGER_MESSAGES.NO_PORT);
    void notices;

    const doc = host.ownerDocument ?? globalThis.document;
    // The sprite every mark's <use> points at. Static fixtures (tools/fixture.mjs)
    // built their own page around toHtml(parts.sheet()) and so drew marks a real
    // mount never did -- this line is the regression fix: a real window and a
    // headless-Chrome smoke both call this same mount(), and neither carried the
    // sprite until now (req/97 real-window row, verdict 0: "not one canon mark
    // renders in the real window"). installSheet is idempotent per document, so a
    // second face mounted into the same page does not duplicate it. Guarded on
    // getElementById/body because test/dom-stand-in.mjs is a structural stand-in with
    // neither (its own header says so: "it proves nothing about drawing") -- every
    // visual claim about marks is made in front of a real renderer, never here.
    if (typeof doc.getElementById === 'function' && doc.body) P.installSheet(doc, P.element.render);
    // Owner directive #335 (2) and (4): the slim scrollbar and the figure/label type
    // scale are rules, not inline styles, so they arrive the same way the glyph
    // sprite does -- once per document, from the one module that owns them
    // (parts/src/surface.mjs). Same guard, same reason: the structural stand-in in
    // test/dom-stand-in.mjs is not a document and proves nothing about drawing.
    if (typeof doc.getElementById === 'function' && doc.body) P.installSurface(doc, P.element.render);
    let live = true;
    let state = null;
    // Acts happen one at a time, in the order a hand asked for them. See sendAct().
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
    };

    /**
     * One act at a time, in the order they were asked for (req/103 finding 1).
     *
     * What this replaces lost data. Every press started an act against whatever
     * `state` held at the moment of the press, and wrote the result back when its own
     * promise resolved -- so two presses landing before the first answer both read the
     * same act log, both appended one entry to that same list, and the second write
     * overwrote the first. The audit measured it on all four sendable acts: two commits
     * reached the server and this window's record of them said one. On the face whose
     * whole question is what happened, in order, an act that happened and left no
     * record is the worst failure available.
     *
     * The cure is that the read of `state` happens inside the queued step rather than
     * at the moment of the press, and a step only runs when the one before it has
     * finished writing. A press does not race a press; it waits behind it. Nothing is
     * dropped: two presses are two acts and two entries, because a person who pressed
     * twice asked twice, and refusing the second silently would be the same kind of lie
     * in the other direction.
     */
    const enqueue = (step) => {
      queue = queue.then(async () => {
        if (!live || !state) return;
        await step();
      });
      return queue;
    };

    const sendAct = (name, id) => enqueue(async () => {
      state = { ...state, menu: null, sending: { act: name, id } };
      paint(view(state));
      try {
        state = { ...(await act(port, state, { act: name, id })), sending: null };
      } catch (error) {
        state = {
          ...state,
          sending: null,
          acts: [...(state.acts ?? []), { act: name, id, outcome: 'failed', detail: error.message }],
        };
      }
      paint(view(state));
    });

    /**
     * Taking a value, through the same queue an act goes through.
     *
     * A clipboard write is not an act and never reaches the server, so the obvious
     * shape is to do it where the press lands and write the answer back when the
     * promise settles. That is exactly the shape req/103 finding 1 measured losing an
     * act: two writes to `state` from two promises that both read the state they
     * started against. A copy landing while an act is in flight would overwrite the
     * act's own record of itself with a state captured before it. So the copy waits
     * behind whatever is in front of it and reads `state` inside its own step, for the
     * same reason and by the same mechanism -- there is one queue on this face, not one
     * per kind of thing a hand can do.
     */
    const copyCell = (from, id) => enqueue(async () => {
      const found = locate(state, id);
      const cell = (COPYABLE.find((entry) => entry.from === from) ?? {}).cell ?? null;
      const offer = found ? copyOffer(found.record, cell) : null;
      if (!offer || offer.value === null) {
        state = { ...state, menu: null, copied: { from, state: 'refused', why: offer?.why ?? LEDGER_MESSAGES.MENU_NO_VALUE } };
        paint(view(state));
        return;
      }
      const clip = typeof navigator === 'object' && navigator !== null ? navigator.clipboard : undefined;
      if (!clip || typeof clip.writeText !== 'function') {
        state = { ...state, menu: null, copied: { from, state: 'refused', why: LEDGER_MESSAGES.COPY_REFUSED } };
        paint(view(state));
        return;
      }
      state = { ...state, menu: null, copied: { from, state: 'asked', why: LEDGER_MESSAGES.COPY_ASKED } };
      paint(view(state));
      try {
        await clip.writeText(offer.value);
        state = { ...state, copied: { from, state: 'copied', why: null } };
      } catch (error) {
        state = { ...state, copied: { from, state: 'refused', why: `${LEDGER_MESSAGES.COPY_FAILED}: ${error.message}` } };
      }
      paint(view(state));
    });

    const onClick = (event) => {
      // A click's target is whatever element sits under the pointer, and an act
      // button now carries a glyph and a word as children (SS553: glyph-first
      // controls) -- a click landing on either child used to read data-act off a
      // node that never carried it and silently do nothing. closest() finds the
      // button itself no matter which of its children was actually hit; this is
      // the interaction-pass regression that requirement's own control surface
      // introduced, caught by pressing the control rather than reading the source.
      const hit = event?.target;
      const reach = hit && typeof hit.closest === 'function' ? (selector) => hit.closest(selector) : () => null;
      // A copy item in a menu. It is the only control on this face that is neither an
      // act nor a decision about what is on the screen, so it is taken first and taken
      // by itself; every other press falls through to the branches that were already
      // here, which is what "additive" means in Owner #348 (2).
      const copying = reach('[data-copy-from]');
      if (copying && typeof copying.getAttribute === 'function' && state) {
        const from = copying.getAttribute('data-copy-from');
        const id = copying.getAttribute('data-target');
        if (from && id) copyCell(from, id);
        return;
      }
      // Click-away. A press anywhere that is not inside the open menu dismisses it, and
      // then goes on to be whatever press it was -- choosing a row with a menu open
      // chooses that row, it does not spend the click on shutting the menu. The state
      // is dropped here and every branch below repaints from it; the one path that
      // repaints nothing of its own is caught at the end.
      const dismissing = Boolean(state?.menu) && !reach('[data-menu-row]');
      if (dismissing) state = { ...state, menu: null };
      // A disclosure the reader opened or shut. Which ones are open is this window's
      // own state (req/103 finding 2), so the press is taken here and the element's
      // own toggle is stopped -- otherwise the element and the state would both be
      // keeping the answer and they would disagree on the first repaint. Only the
      // summary counts: a press anywhere inside an open panel is a press on what is
      // in the panel, not a request to shut it.
      const pressed = hit && typeof hit.closest === 'function' ? hit.closest('summary') : null;
      const holder = pressed && pressed.parentNode ? pressed.parentNode : null;
      const disclosure = holder && typeof holder.getAttribute === 'function'
        ? (holder.getAttribute('data-control') ?? holder.getAttribute('data-peripheral'))
        : null;
      if (disclosure && state) {
        const already = Array.isArray(state.opened) ? state.opened : [];
        state = {
          ...state,
          opened: already.includes(disclosure) ? already.filter((key) => key !== disclosure) : [...already, disclosure],
        };
        if (typeof event.preventDefault === 'function') event.preventDefault();
        paint(view(state));
        return;
      }
      // Owner directive #335 (3): choosing a row names it as the subject of the one
      // detail pane on this screen. It changes nothing on the server and nothing in
      // the ledger -- it is this window deciding which record it is describing -- so
      // it repaints from the state already in hand and sends nothing.
      const chosen = reach('[data-select-row]');
      if (chosen && typeof chosen.getAttribute === 'function') {
        const id = chosen.getAttribute('data-select-row');
        if (id && state) {
          state = { ...state, selected: state.selected === id ? null : id };
          paint(view(state));
        } else if (dismissing) paint(view(state));
        return;
      }
      const target = reach('[data-act]');
      if (!target || typeof target.getAttribute !== 'function') {
        if (dismissing) paint(view(state));
        return;
      }
      const name = target.getAttribute('data-act');
      const id = target.getAttribute('data-target');
      if (!name || !id || !state) {
        if (dismissing) paint(view(state));
        return;
      }
      // A menu act and a gutter act are this line. Both controls carry the same two
      // attributes, both arrive here, and both go into the one queue -- there is no
      // second route for an act to be lost on.
      sendAct(name, id);
    };

    /**
     * The other button (Owner #348 (2)).
     *
     * Where the press landed decides two things and nothing else: which row the menu
     * is about, and whether a cell was under the pointer. `data-select-row` is on the
     * row control and `data-target` is on the gutter and its buttons, so a right-click
     * anywhere along a row -- on the line, on an act, on the space between them --
     * finds the same identity, which is what "every interactive row and every act"
     * asks for. The row is then found in the state rather than read off the drawing.
     *
     * The native menu is refused. Offering the browser's own menu beside this one would
     * be two menus for one press, and the browser's has no idea what any of this is.
     */
    const onContextMenu = (event) => {
      const hit = event?.target;
      if (!hit || typeof hit.closest !== 'function' || !state) return;
      const row = hit.closest('[data-select-row]');
      const near = hit.closest('[data-target]');
      const id = (row && typeof row.getAttribute === 'function' ? row.getAttribute('data-select-row') : null)
        ?? (near && typeof near.getAttribute === 'function' ? near.getAttribute('data-target') : null);
      if (!id) return;
      const found = locate(state, id);
      if (!found) return;
      const cellNode = hit.closest('[data-cell]');
      const cell = cellNode && typeof cellNode.getAttribute === 'function' ? cellNode.getAttribute('data-cell') : null;
      if (typeof event.preventDefault === 'function') event.preventDefault();
      // One slot, so a second right-click replaces a menu and cannot stack one on top
      // of another. Right-clicking the row that already has one closes it, which is
      // the one dismissal a pointer can perform without moving.
      const same = state.menu && state.menu.id === id && (state.menu.cell ?? null) === cell;
      state = { ...state, menu: same ? null : { id, cell } };
      paint(view(state));
    };

    /** Escape, from wherever the focus is. A menu that can only be dismissed with a
     * pointer is a menu half the people looking at this screen cannot get out of.
     * It listens on the document and not on the host, because a keypress made while
     * the focus has left this face is still that reader asking for the menu to go, and
     * because two listeners for one key would run this twice for every press inside
     * the host. */
    const onKeyDown = (event) => {
      if (event?.key !== 'Escape' || !state?.menu) return;
      state = { ...state, menu: null };
      paint(view(state));
    };

    /** A press that landed outside this face entirely -- on the shell's own chrome, or
     * on another face in the same window. The host's own handler cannot see one, so
     * this is the half of click-away that needs the document. It is guarded on
     * `contains` because a host that cannot answer "is this node inside me" cannot be
     * asked this question safely, and answering it wrong would dismiss a menu on a
     * press that was inside it. */
    const away = typeof host.contains === 'function' ? (event) => {
      if (!state?.menu || (event?.target && host.contains(event.target))) return;
      state = { ...state, menu: null };
      paint(view(state));
    } : null;

    if (typeof host.addEventListener === 'function') {
      host.addEventListener('click', onClick);
      host.addEventListener('contextmenu', onContextMenu);
    }
    if (doc && typeof doc.addEventListener === 'function') {
      doc.addEventListener('keydown', onKeyDown);
      if (away) doc.addEventListener('click', away);
    }

    paint(waitingView());

    const ready = read(port)
      .then((first) => {
        state = first;
        paint(view(state));
        return state;
      })
      .catch((error) => {
        paint(frame([aside(`${LEDGER_MESSAGES.UNREAD}. ${error.message}`, 'unread')]));
        return null;
      });

    const unmount = () => {
      live = false;
      if (typeof host.removeEventListener === 'function') {
        host.removeEventListener('click', onClick);
        host.removeEventListener('contextmenu', onContextMenu);
      }
      // Both of these were put on a document this face does not own, so both come off
      // it. A listener left on the document by a face that has been taken down is a
      // face still answering presses on a screen it is no longer part of.
      if (doc && typeof doc.removeEventListener === 'function') {
        doc.removeEventListener('keydown', onKeyDown);
        if (away) doc.removeEventListener('click', away);
      }
      clear();
    };
    unmount.ready = ready;
    /** When nothing this window sent is still waiting for an answer. A caller that
     * needs to know what the screen says after a press has to be able to wait for the
     * press to finish, and a fixed delay would be a guess. */
    unmount.quiet = () => queue;
    return unmount;
  }

  return {
    DECLARATION, mount, read, act, view, waitingView, toRecord, callerFor, toHtml,
  };
}

export const face = createFace();
export const mount = face.mount;
export { DECLARATION };
