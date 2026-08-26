// SPDX-License-Identifier: Apache-2.0
// The held face: what has not happened yet.
//
// One list, one question, and a single discipline that everything else in this
// file exists to serve: nothing drawn here may wear a receipt's face. A candidate
// carries a digest and a path the way a settled row does, and it is tempting to
// draw it the same way -- the columns are the same shape, the glyph sheet is the
// same sheet. What must never happen is that a reader glances at this screen and
// walks away believing something here has been checked, sealed, or committed. So
// the seal column is a declared hole on every single row, unconditionally, and the
// claims section runs the same structural checks faces/ledger runs on its settled
// rows precisely so a reader can see, in the open, that nothing here claims to be
// checked either.
//
// The other two properties carried over from faces/ledger's own discipline still
// apply: a list that could not be read is never drawn as an empty one, and a row
// that has been acted on is never edited in place -- committing or cancelling a
// candidate is answered by reading the list again, not by rewriting the row this
// window already drew.

import {
  DECLARATION, CONSUMES, READS, ACTS, GATES, ORDER, ROWS, UNDRAWN, QUESTION, FACE_ID,
} from './declaration.mjs';
import { parts as defaultParts } from './binding.mjs';

export const HELD_MESSAGES = {
  NO_HOST: 'a face is mounted into a host element, and none was given',
  NO_PORT: 'a face is mounted with the port it is to speak through, and none was given',
  UNDECLARED: 'this face may not call a method it did not declare',
  UNKNOWN_ACT: 'no such act on this face',
  READING: 'reading what is held',
  UNREAD: 'this list was not read. What is below is not an empty list of held candidates; it is the absence of one, and the two are different facts',
  EMPTY_HELD: 'nothing is being held here',
  ALL_DROPPED: 'candidates arrived and none of them could be drawn; every one is listed below with the reason',
  TRUNCATED: 'the walk stopped at its budget, so there are candidates behind this screen that it did not reach',
  REPEATED: 'the server handed back a cursor it had already given, so the walk stopped rather than circle',
  NOTHING_TO_SEAL: 'this has not happened yet, so there is no record of it to check',
  // req/822_c7 (Owner #387/#388 冗長文字全掃): this used to read "this member was
  // looked for on the item and was not there: <member>", built with the member's
  // own name stitched onto the end of it in toRecord() below. Drawn as a note line
  // (noteLines()), that put the member's name on the screen twice -- once as the
  // row label beside the sentence, once again inside the sentence itself. The
  // member is still the row label; the sentence carries no second copy of it.
  MEMBER_ABSENT: 'not in this record',
  MEMBER_NOT_SCALAR: 'this member arrived as a structure this face does not read',
  NOT_THIS_SCREENS_KIND: 'this item carries its own lifecycle and it is not held; the row wears the item\'s word, and this note is here because this screen did not assign it',
  NOTE_SUMMARY: 'what is missing from this row, what it holds in full, and what committing or cancelling it would send',
  CLIP_RISK: 'this row holds a value longer than its column can show, so the note under it carries every value in full',
  WITHHELD: 'declared, offered, and not sent',
  SENT: 'sent',
  NOT_A_RECEIPT: 'nothing on this screen has happened. Every row here is a proposal, and the seal column says so on every one of them -- not because anything is wrong, but because this is the screen where nothing is checked yet',
  // Was one constant answering four gates, so four gates drew the same 99 characters
  // byte-identical -- 43% of every visible character on the window and a named
  // violation of req/96 R-4 ("no sentence present verbatim on more than one row of the
  // same section"). Hoisting it to a single line above the ladder was tried and is
  // wrong: `held.test.mjs` asserts every gate whose act is unavailable draws its reason
  // BESIDE its disabled control, which is req/811 §8-7, and a reason stated once for
  // four controls is not stated beside any of them. So the reason is per-gate instead
  // of per-ladder. The duplication was never a formatting problem -- it was one
  // sentence being asked to answer four different questions, and now each gate says
  // which act it is that this window cannot vouch for.
  GATE_UNREAD: (act) => `the candidates were not read, so this window does not know whether ${act} is available here, and draws it dead rather than guess`,
  GATE_NO_SUBJECT: 'no candidate is chosen. Choose one in the list and this gate answers for that one',
  GATE_SUBJECT_GONE: 'the candidate that was chosen is not in the list this window last read. It was read again after the last act',
  // One sentence, because the ladder draws a reason up to its first full stop and a
  // gate that has just said "open" should spend that sentence on what happens next
  // rather than on a clause the reader has to open a control to finish.
  GATE_CAN_SEND: 'this window can send it, and whatever the engine answers is written into the acts below',
  GATE_NO_INVERSE: 'nothing on this screen has happened yet, so no inverse is held for any of these candidates',
  GATE_UNRULED: 'this gate is declared and this screen does not compute an answer for it, so it states nothing rather than a permission it has not earned',
  GATE_CUT: 'the reason beside a shut gate is drawn to the end of its first sentence. The whole of it is in the control\'s own label and under "what is not drawn"',
  IN_FLIGHT: 'this was sent and the answer has not arrived. It is drawn dead until it does, so that pressing it twice cannot send it twice',
  BAND_CANDIDATES: 'candidates drawn on this screen, against the count received under "what is not drawn"',
  BAND_READY: 'candidates this window could send a commit for: it holds their identity and the commit route is one this face sends',
  BAND_UNDRAWN: 'items that arrived in the same read and were not drawn, each one named with its reason under "what is not drawn"',
  BAND_INVERSES: 'candidates with an inverse still held. Nothing on this screen has happened yet, so this is zero here and would not be on a screen of settled rows',
  BAND: 'the size and shape of this screen, before a word of it is read',
  MENU: 'the acts this candidate is offered and the gate governing each one, in the order the ladder climbs them. Pressing one here is the same send as pressing it in the row',
  MENU_NOT_A_VALUE: 'the pointer was not over a cell holding a value, so there is nothing here to take',
  COPIED: 'taken, and on the clipboard',
  COPY_FAILED: 'this window could not reach a clipboard, so nothing was taken. It says so rather than looking as though it worked',
  LADDER: 'what has to hold before a candidate can be committed, one gate at a time, with the act each gate governs beside its answer',
  ACT_LOG: 'what was sent from this screen, and what came back',
};

const ANSWERED = 'answered';

/** The three answers a gate ever gives, in the words they are drawn in. Two of them
 * are absences and they are kept apart: `shut` is a precondition this window read
 * and found unmet, `unknown` is one it could not read at all. Nothing collapses the
 * two into a single dimmed control with one shared excuse -- the same discipline the
 * declaration already holds for its two withholdings. */
const GATE_OPEN = 'open';
const GATE_SHUT = 'shut';
const GATE_UNKNOWN = 'unknown';

/** The mark a gate's answer is drawn with. `shut` takes the standing every row on
 * this screen already carries, because a shut gate is the reason the candidate is
 * still here -- one meaning, one mark. `unknown` takes the declared hole. `open`
 * takes the act's own mark, resolved per gate, and never a verdict: a check beside
 * the word commit would read as the engine having admitted this candidate, which is
 * the one thing this face exists to stop a reader believing. */
const GATE_MARK = Object.freeze({
  [GATE_SHUT]: ['standing', 'held'],
  [GATE_UNKNOWN]: ['structure', 'hole'],
});

/** The five columns whose values come straight off the item, matching the member
 * faces/ledger reads for the same column -- one row grammar, read the same way on
 * both faces that draw it. */
const MEMBERS = Object.freeze([
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
    if (!allow.has(name)) throw new Error(`${HELD_MESSAGES.UNDECLARED}: ${name}`);
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
 * One candidate, as a row. Non-objects are handed back untouched so the order can
 * drop them with the reason it has for them. The seal hole is set unconditionally
 * -- there is no half argument here the way faces/ledger's toRecord takes one,
 * because every row on this screen is the held half; there is no other kind of row
 * this function could ever be asked to build.
 *
 * req/822_c5 (B1, carried since c2 §3): `lifecycle: 'held'` used to be stamped
 * unconditionally, so an item that arrived carrying its own contrary word was silently
 * relabelled -- the one thing this screen's opening comment promises never to let a
 * reader believe. 'held' is still the default, and it is honest as a default: the list
 * this face reads is `GET /candidates`, whose own filter (gx-api/src/list.rs) returns
 * only rows that have not reached a terminal state, so "held" for a wire row is the
 * endpoint's contract and not this function's invention. What is no longer done is
 * overriding an item that speaks for itself: a stated `lifecycle` is worn as stated,
 * and when it is not 'held' the contradiction is written into the row's own note
 * rather than erased. (The wire's `state` member -- the engine's word for a row's
 * place in its life -- is a different vocabulary and a different member; what this
 * face does not read of it is measured in req/822_c5's re-measure, not silently
 * mapped here.)
 */
export function toRecord(item) {
  if (item === null || typeof item !== 'object' || Array.isArray(item)) return item;
  const holes = { seal: HELD_MESSAGES.NOTHING_TO_SEAL };
  const cells = {};
  for (const member of MEMBERS) {
    const value = item[member.member];
    if (value === undefined || value === null || value === '') holes[member.key] = HELD_MESSAGES.MEMBER_ABSENT;
    else if (!isScalar(value)) holes[member.key] = `${HELD_MESSAGES.MEMBER_NOT_SCALAR}: ${member.member}`;
    else cells[member.key] = String(value);
  }
  const claimed = isScalar(item.lifecycle) && String(item.lifecycle) !== '' ? String(item.lifecycle) : null;
  if (claimed !== null && claimed !== 'held') {
    holes.lifecycle = `${HELD_MESSAGES.NOT_THIS_SCREENS_KIND}: ${claimed}`;
  }
  return Object.freeze({
    ...cells,
    ...(isScalar(item.id) && String(item.id) !== '' ? { id: String(item.id) } : {}),
    ...(Number.isInteger(item.sequence) ? { n: item.sequence } : {}),
    ...(isScalar(item.digest) ? { digest: String(item.digest) } : {}),
    prev: item.prev ?? null,
    lifecycle: claimed ?? 'held',
    holes: Object.freeze(holes),
  });
}

export function createFace({ parts = defaultParts } = {}) {
  const P = parts;
  const { el, style, find } = P.element;
  const T = P.tokens;

  // -- small pieces of type ---------------------------------------------------

  /**
   * Three weights, chosen by what a run of text IS rather than by how much it wants
   * to be noticed -- the same discipline the corner scale holds for radii, and for
   * the same reason: four weights is four numbers nobody chose once.
   *
   *   figure  a number. It matches the 600 the shared figure rule already draws the
   *           band's counts at, so a count in this face's own type and a count in a
   *           shared container are one weight and not two.
   *   name    a word that names a thing -- a gate, a legend key, this screen.
   *   body    a sentence.
   *
   * No call site spells a weight; tools/gate.mjs holds that at zero over the source,
   * so a fourth weight cannot arrive by hand.
   */
  const WEIGHT = Object.freeze({ figure: '600', name: '500', body: '400' });

  /**
   * How prose breaks, in one place.
   *
   * `overflow-wrap: anywhere` -- what every paragraph on this face used to carry --
   * breaks a word at whatever character the line ends on and lets the box itself
   * shrink to one character, so ordinary words come apart mid-word on a narrow
   * screen. `break-word` breaks a word only when that single word cannot fit a line
   * of its own, which is the case the rule exists for and no other. `text-wrap:
   * pretty` is the renderer's own answer to the second half of the same problem: a
   * last line holding one or two characters. Both are stated once, here, because a
   * paragraph rule written per paragraph is a rule with four spellings.
   */
  const PROSE = Object.freeze({ 'overflow-wrap': 'break-word', 'text-wrap': 'pretty' });

  const aside = (words, role = 'aside') => el('p', {
    'data-role': role,
    style: style({
      margin: '0 0 6px', color: T.attendant, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'font-weight': WEIGHT.body, ...PROSE,
    }),
  }, [words]);

  const plain = (words, role = 'line') => el('p', {
    'data-role': role,
    style: style({
      margin: '0 0 4px', color: T.ink, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'font-weight': WEIGHT.body, ...PROSE,
    }),
  }, [words]);

  /** Disclosure folded shut by default, native <details> -- data first, zero
   * preamble (the same SS549 discipline faces/ledger's own peripheral() applies).
   * Named, and drawn from the screen's own state rather than from the browser's,
   * because this face repaints its whole tree on every act: a fold whose open-ness
   * lives only in the element is a fold that shuts itself the moment a reader
   * presses anything, which is what this one did. */
  const peripheral = (name, word, node, { open = false } = {}) => el('details', {
    'data-role': 'peripheral', 'data-control': name, 'data-open': String(Boolean(open)), open: open || null,
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

  const section = (name, state, children) => el('section', {
    'data-section': name,
    'data-state': state,
    style: style({ padding: '14px 0', 'border-top': `1px solid ${T.rule}`, background: T.page }),
  }, children.filter(Boolean));

  // -- compact header + bordered one-row controls (SS657 retrofit, req/38 SS657
  // Owner #317/#318; idiom proven by faces/atlas). See faces/ledger's own copy of
  // this comment for the fuller account of the five seat-confirmed defects.

  const headerLine = (words) => el('div', {
    'data-role': 'face-header',
    style: style({ display: 'flex', 'align-items': 'baseline', gap: '10px', padding: '10px 0 6px', 'font-family': T.sans }),
  }, [
    el('span', { style: style({ 'font-weight': WEIGHT.name, 'font-size': T.head, 'line-height': T.headLine, color: T.ink }) }, [FACE_ID]),
    el('span', { style: style({ color: T.attendant, 'font-size': T.record, 'line-height': T.recordLine, 'font-weight': WEIGHT.body }) }, words),
  ]);

  /** Which disclosures the reader has open. It is part of the state this face draws
   * from, so a repaint -- and every act on this screen causes one -- redraws them as
   * the reader left them. */
  const opened = (state, name) => (state?.open ?? []).includes(name);

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
      el('span', { style: style({ 'font-weight': WEIGHT.name }) }, [label]),
    ]),
    el('div', { style: style({ padding: `0 ${T.padX} 10px` }) }, [body]),
  ]);

  const controlsRow = (children) => el('div', {
    'data-role': 'control-row',
    style: style({ display: 'flex', gap: '8px', 'flex-wrap': 'wrap', padding: '0 0 10px' }),
  }, children);

  const markTallyRows = (counts) => DECLARATION.marks.map((m) => el('div', {
    'data-mark-entry': m.mark, 'data-count': String(counts.get(m.mark) ?? 0),
    style: style({
      display: 'grid', 'grid-template-columns': 'minmax(0,9rem) 2.5rem minmax(0,1fr)', gap: '10px', padding: '2px 0',
      'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
    }),
  }, [
    el('span', { style: style({ color: T.ink, 'font-family': T.mono, 'font-size': T.time, 'font-weight': WEIGHT.name }) }, [m.mark]),
    el('span', { style: style({ color: T.ink, 'font-family': T.mono, 'font-weight': WEIGHT.figure }) }, [String(counts.get(m.mark) ?? 0)]),
    el('span', { style: style({ color: T.attendant, 'font-weight': WEIGHT.body, ...PROSE }) }, [m.from]),
  ]));

  const notDrawnLegendRows = () => UNDRAWN.map((entry) => el('div', {
    'data-not-drawn': entry.what,
    style: style({
      display: 'grid', 'grid-template-columns': 'minmax(0,9rem) 2.5rem minmax(0,1fr)', gap: '10px', padding: '2px 0',
      'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
    }),
  }, [
    el('span', { style: style({ color: T.attendant, 'font-weight': WEIGHT.name }) }, ['not drawn']),
    el('span', {}, ['']),
    el('span', { style: style({ color: T.ink, 'font-weight': WEIGHT.body, ...PROSE }) }, [`${entry.what} -- ${entry.why}`]),
  ]));

  function legendBody(counts) {
    return el('div', { 'data-role': 'legend' }, [
      el('div', { 'data-role': 'legend-marks' }, markTallyRows(counts)),
      el('div', { 'data-role': 'legend-prose' }, LEGEND_LINES.map((entry) => el('div', {
        'data-legend-entry': entry.name,
        style: style({
          display: 'grid', 'grid-template-columns': 'minmax(0,9rem) minmax(0,1fr)', gap: '10px', padding: '2px 0', 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
        }),
      }, [
        el('span', { style: style({ color: T.attendant, 'font-weight': WEIGHT.name }) }, [entry.name]),
        el('span', { style: style({ color: T.ink, 'font-weight': WEIGHT.body, ...PROSE }) }, [entry.value]),
      ]))),
      el('div', { 'data-role': 'legend-not-drawn' }, notDrawnLegendRows()),
    ]);
  }

  // -- rows -------------------------------------------------------------------

  const LEGEND_LINES = Object.freeze([
    { name: 'fingerprint column', value: 'the first 6 characters of this candidate\'s own digest, upper-cased. A match here is a hint, never a proof -- the note under a row carries the digest in full.' },
    { name: 'seal column (always a hole)', value: HELD_MESSAGES.NOT_A_RECEIPT },
    { name: 'checkable elsewhere', value: 'what a third party would need to check this candidate without asking this window: the digest, the algorithm, and the anchor. Every held candidate is missing at least the anchor, because nothing has committed it yet -- that is expected, not a defect.' },
    { name: 'the time column', value: P.rowMessages.AT_FORM },
    { name: 'what the pane on the right holds', value: HELD_MESSAGES.NOTE_SUMMARY },
    { name: 'open because: clipped', value: HELD_MESSAGES.CLIP_RISK },
    { name: 'undo availability chip', value: 'always "n/a" on this screen -- every row here is a candidate that has not happened yet, so there is no escrowed inverse to hold. The other two states this chip can read ("reversed", "unknown") belong to a committed row, which is the ledger\'s settled half, not this one.' },
    { name: 'the gates, and their three answers', value: `${GATE_OPEN}: this window can send the act beside it, for the candidate named at the top of the ladder. ${GATE_SHUT}: a condition this window read and found unmet, with the condition beside the control. ${GATE_UNKNOWN}: a condition this window could not read at all -- an unread gate is never drawn as a passed one.` },
    { name: 'the reason beside a gate', value: HELD_MESSAGES.GATE_CUT },
  ]);

  // req/768 AC-7: every row on this screen reads not-committed -- a held
  // candidate can never be the one positive fact ("reversed") this chip states,
  // so unlike faces/ledger's settled rows there is no row-specific reversibility
  // text to add here; the general meaning is stated once, in the legend below.
  function noteLines(record) {
    const lines = Object.entries(record.holes ?? {}).map(([key, why]) => ({ name: key, value: why }));
    for (const member of MEMBERS) {
      if (record[member.key] !== undefined) lines.push({ name: `${member.key} in full`, value: record[member.key] });
    }
    if (record.digest) lines.push({ name: 'digest in full', value: record.digest });
    lines.push({ name: 'checkable elsewhere', value: P.portability(record).why });
    return lines;
  }

  /**
   * The acts as they stand for one row at this moment, which is not always as they
   * stand in the declaration.
   *
   * An act that has been sent and not yet answered does not send when it is pressed
   * again -- it would send a second commit for a candidate the first one may already
   * have committed -- so for as long as it is in flight it is exactly what the
   * declaration's two permanently withheld acts are: declared, offered, drawn, and
   * dead, with the reason for it in the same place theirs is. One override, and both
   * surfaces that draw an act (the row's gutter and the readiness ladder) answer the
   * same way, because both read this list rather than the declaration directly.
   */
  const inFlight = (pending, act, id) => (pending ?? []).some((entry) => entry.act === act && entry.id === id);
  const actsFor = (id, pending) => ACTS.map((spec) => (
    inFlight(pending, spec.act, id) ? { ...spec, sends: false, why: HELD_MESSAGES.IN_FLIGHT } : spec
  ));

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
   * req/768 AC-4/AC-7 (retrofit round 2): the row's own acts now draw as a
   * fixed-width right gutter (parts/src/receipt-row.mjs's openableRow(), not the
   * full-width strip this face drew before this round -- see faces/ledger's own
   * copy of this comment for the fuller account); and the reversibility chip,
   * always not-committed here (reversalOf() short-circuits on lifecycle:'held'
   * before it ever looks at a sibling) -- `siblings` is threaded through anyway,
   * for the same reason faces/ledger threads it, not because held's own answer
   * ever depends on it.
   */
  function rowBlock(record, siblings, selected, pending, menuFor) {
    const claim = P.claimOf(record, { verifier: null });
    const reversal = P.reversalOf(record, siblings);
    const holes = Object.keys(record.holes ?? {});
    // Every row has a `seal` hole (NOTHING_TO_SEAL) by construction, so the
    // "declared-hole" open reason would fire on every row if it counted that one
    // -- which would open every note on a screen whose whole point is that nothing
    // is sealed, drowning the one signal (a *missing member*) that should open a
    // note under the seal signal that always does. The seal hole is excluded from
    // the open-because decision for that reason; it is still drawn, in the row and
    // repeated in the note when the note opens for another reason.
    const openHoles = holes.filter((key) => key !== 'seal');
    // Against the drawn form, not the arrived value (req/97 gap-list item gap 1): the `at`
    // column draws a declared cut, so an ISO-8601 timestamp is not a clip here and
    // must not force every note on this screen open. Same reading for faces/ledger,
    // faces/receipt and faces/graph, through the same P.drawnTextFor().
    // Owner directive #335 (3): nothing opens under a row any more, so this decides
    // only whether the row is marked as one the grid cannot hold -- a fact about the
    // row, not a reason to push the rows below it down a screen.
    const tight = MEMBERS.some((member) => P.drawnTextFor(member.key, record[member.key]).length > (BUDGETS[member.key] ?? 40));
    const lines = noteLines(record);
    return el('div', {
      'data-role': 'row-block',
      'data-open-because': openHoles.length > 0 ? 'declared-hole' : (tight ? 'clip-risk' : null),
      style: style({ display: 'block' }),
    }, [
      P.selectableRow(record, {
        claim, reversal, acts: actsFor(record.id, pending), fields: lines.length, selected: selected === record.id,
        // The seal column stays on this face's scan line: "nothing here is sealed" is
        // this screen's own answer, not a detail of one row (parts SCAN_COLUMNS_SEALED).
        columns: P.scanColumnsSealed,
      }),
      // The menu a right-click opened on this row, if it was opened on this row. It
      // follows the row in flow rather than floating over it -- see MENU_PLACEMENT.
      menuFor(menuAt.row(record.id)),
    ]);
  }

  /** The one pane this screen stores a candidate's detail in (directive #335, 3). */
  function paneFor(selected, held) {
    const rows = held.ordered ? held.ordered.rows : [];
    const record = rows.find((r) => r.id === selected);
    if (!record) return P.detailPane({});
    return P.detailPane({ subject: record.id, lines: noteLines(record) });
  }

  // -- the one list -------------------------------------------------------------

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

  /** The standing every row in the list shares, on the container that holds them.
   * The group states its own condition once, in the same chip a row states it in,
   * rather than leaving a reader to infer it from every line separately. */
  const heldPill = () => P.chip('standing', 'held', { word: 'held', said: HELD_MESSAGES.NOT_A_RECEIPT });

  /** Inside a box. The rows carry their own horizontal padding, so this pads the
   * group vertically and lets the lines run nearly the full width of the container
   * the way a list of records should -- the four horizontal pixels are the gap
   * between the act gutter's outermost button and the container's own border, which
   * the row grammar has no way to know about: the gutter is attached outside the
   * row's padding, and in a bordered container that put a button's edge flush on the
   * border (measured at the narrow viewport, where the gutter is widest relative to
   * the row). */
  const boxed = (children) => el('div', {
    'data-role': 'box-body',
    style: style({ padding: '6px 4px' }),
  }, children.filter(Boolean));

  /**
   * The read, settled before a single thing is drawn from it.
   *
   * This was the first half of heldList(): one pass that read the envelope, ordered
   * the records and handed back a finished section. Everything that needed to know
   * what had been read -- the band, the ladder, the pane -- got it from the object
   * that pass returned, which was fine while all of those were drawn beside the list.
   * The menu a right-click opens is drawn *inside* a row and answers with the same
   * gate answers the ladder gives, so it needs the ordered rows before the rows are
   * built. Deciding first and drawing second is what lets one screen hold both, and
   * it is the shape the rest of this file already wanted.
   */
  function readHeld(state) {
    const envelope = state.held;
    if (!envelope || envelope.outcome !== ANSWERED) {
      return {
        envelope, answered: false, received: 0, ordered: null, listState: 'unread',
      };
    }
    const items = Array.isArray(envelope.items) ? envelope.items : [];
    const ordered = P.order(items.map(toRecord), { by: ROWS.order });
    return {
      envelope,
      answered: true,
      received: items.length,
      ordered,
      listState: items.length === 0 ? 'empty' : (ordered.rows.length === 0 ? 'all-dropped' : 'drawn'),
    };
  }

  function heldSection(read, state, selected, menuFor) {
    if (!read.answered) {
      return section('held', 'unread', [
        P.box({
          name: 'candidates',
          // Not zero. A list that was not read and a list with nothing in it are
          // different facts, and the dash is the one the band and this head both
          // draw for the first of them.
          count: null,
          // The head reads "candidates -- N drawn", against the band's own "not
          // drawn" figure beside it. Repeating the container's name as its noun
          // ("candidates 3 candidates") says one thing twice and the second time
          // says nothing.
          noun: 'drawn',
          pill: heldPill(),
          said: HELD_MESSAGES.UNREAD,
          children: boxed([
            el('div', { style: style({ padding: `0 ${T.padX}` }) }, [
              aside(HELD_MESSAGES.UNREAD, 'unread'),
              ...outcomeLines(read.envelope),
            ]),
          ]),
        }),
      ]);
    }

    const { ordered, listState } = read;
    const orderDetail = el('div', { 'data-role': 'order-detail' }, [
      plain(`requested: ${ordered.requested}, applied: ${ordered.by}`, 'order'),
      aside(ROWS.order_reason, 'order-reason'),
      ordered.substituted ? plain(ordered.reason, 'order-substituted') : null,
    ].filter(Boolean));

    return section('held', listState, [
      P.box({
        name: 'candidates',
        count: ordered.rows.length,
        noun: 'drawn',
        pill: heldPill(),
        said: HELD_MESSAGES.NOT_A_RECEIPT,
        children: boxed([
          listState === 'empty' || listState === 'all-dropped'
            ? el('div', { style: style({ padding: `0 ${T.padX}` }) }, [
              listState === 'empty' ? aside(HELD_MESSAGES.EMPTY_HELD, 'empty') : null,
              listState === 'all-dropped' ? aside(HELD_MESSAGES.ALL_DROPPED, 'all-dropped') : null,
            ].filter(Boolean))
            : null,
          ordered.rows.length > 0
            ? el('div', { 'data-role': 'rows', 'data-count': String(ordered.rows.length) }, ordered.rows.map((record) => rowBlock(record, ordered.rows, selected, state.pending, menuFor)))
            : null,
          el('div', { style: style({ padding: `0 ${T.padX}` }) }, [
            peripheral('order', `order: ${ordered.by}`, orderDetail, { open: opened(state, 'order') }),
          ]),
        ]),
      }),
    ]);
  }

  // -- the readiness ladder -----------------------------------------------------
  //
  // The four acts were already offered on every row, in a gutter, each one either
  // live or dimmed. What the screen never said was why. This is that, and it is the
  // one structure on this face taken deliberately from what the reference round
  // measured as the strongest donor shape available: one container per declared
  // gate, the gate's own answer first, the act that gate governs inside the same
  // container, and the reason an unavailable act is unavailable beside the disabled
  // control at the size the rest of the screen is read at -- never in a title alone,
  // and never as blank space where a control would have been.
  //
  // Two properties of it are this application's own rather than the donor's. The
  // first is that a gate this window cannot settle draws `unknown` and says what it
  // could not read; the donor has no such state because its own gates are policies a
  // person configured, always readable. The second is the inverse gate, which the
  // donor cannot draw at all: its reverse act is an ordinary forward commit, so
  // there is nothing for it to report. Ours reports it, and reports it honestly --
  // this screen's answer is that no inverse is held, because nothing here has
  // happened yet, and it is computed per candidate rather than assumed.

  /** A declared cut, not a clip. The reason beside a gate is drawn to the end of its
   * first sentence; the whole of it is on this page twice over (the control's own
   * label, and the withheld list under "what is not drawn"). A four-line reason in
   * every gate pushes the candidates themselves off the screen, which is the trade
   * the time column already makes for the same reason and states in the legend. */
  const firstSentenceOf = (words) => {
    const line = String(words ?? '');
    const stop = line.indexOf('. ');
    return stop === -1 ? line : line.slice(0, stop + 1);
  };

  /**
   * The chain every sending gate is answered by, in the order the conditions have to
   * be met. It is written once and shared by the three gates that need it rather
   * than copied per gate: three copies of a precondition chain is three places for
   * one of them to be edited and the others not.
   */
  const sendableGate = ({ spec, read, chosen, subject, gate }) => {
    if (!spec.sends) return [GATE_SHUT, spec.why];
    if (!read) return [GATE_UNKNOWN, HELD_MESSAGES.GATE_UNREAD(gate.act)];
    if (!chosen) return [GATE_SHUT, HELD_MESSAGES.GATE_NO_SUBJECT];
    if (!subject) return [GATE_SHUT, HELD_MESSAGES.GATE_SUBJECT_GONE];
    return [GATE_OPEN, HELD_MESSAGES.GATE_CAN_SEND];
  };

  /**
   * How each declared gate is answered. Keyed by the gate's own declared name, so a
   * gate declared with no rule here is answered `unknown` with the reason for that,
   * rather than falling through to whatever the last branch happened to be -- a
   * screen that fails open on a gate it has no rule for is exactly the failure this
   * ladder exists to make impossible.
   *
   * The inverse gate is the one that is not a question about permission. It is asked
   * of parts/src/reversibility.mjs, which answers from records this window already
   * read and never by calling a route to find out, and its three answers map onto
   * two of ours: an inverse that was already used and one that never existed are
   * both `shut` (with different words), and an inverse whose presence the membrane
   * exposes no field for is `unknown`. It has no path to `open` on this face, and
   * that is the honest state of it rather than an omission.
   */
  const GATE_RULES = Object.freeze({
    raised: sendableGate,
    withdrawal: sendableGate,
    commit: sendableGate,
    inverse: ({ read, chosen, subject, siblings, gate }) => {
      if (!read) return [GATE_UNKNOWN, HELD_MESSAGES.GATE_UNREAD(gate.act)];
      if (!chosen) return [GATE_SHUT, HELD_MESSAGES.GATE_NO_INVERSE];
      if (!subject) return [GATE_SHUT, HELD_MESSAGES.GATE_SUBJECT_GONE];
      const reversal = P.reversalOf(subject, siblings);
      return [reversal.state === 'not-observable' ? GATE_UNKNOWN : GATE_SHUT, reversal.why];
    },
  });

  function gateAnswers(held, selected, pending) {
    const rows = held.ordered ? held.ordered.rows : null;
    const subject = rows === null ? null : (rows.find((row) => row.id === selected) ?? null);
    // The same list the row gutter reads, so an act in flight shuts its gate and
    // dims its control from one decision rather than two that can disagree.
    const specs = actsFor(selected, pending);
    return GATES.map((gate) => {
      const spec = specs.find((candidate) => candidate.act === gate.act);
      const rule = GATE_RULES[gate.gate];
      const [state, why] = rule
        ? rule({ spec, read: rows !== null, chosen: selected !== null, subject, siblings: rows ?? [], gate })
        : [GATE_UNKNOWN, HELD_MESSAGES.GATE_UNRULED];
      // The act carries a target only where the gate is open. A dead control that
      // still names a row is a control one stray click away from sending an act the
      // screen has just finished saying it may not send.
      return {
        ...gate, spec, state, why, subject: state === GATE_OPEN && subject ? subject.id : null,
      };
    });
  }

  const gateChip = (answer, word = answer.state) => {
    const mark = answer.state === GATE_OPEN ? ['act', answer.act] : GATE_MARK[answer.state];
    return P.chip(mark[0], mark[1], { word, said: answer.asks });
  };

  /** The act, in the gate's own container. It is the row gutter's button at the row
   * gutter's own size and in the row gutter's own part, so the one rule set that
   * paints a live act in the accent and refuses the pointer on a dead one applies
   * here without a second copy of it being written. */
  const GATE_ACT_WIDTH = '104px';
  function gateActControl(answer, place = {}) {
    const live = answer.state === GATE_OPEN;
    return el('div', {
      'data-part': 'act-gutter', 'data-role': 'gate-act', 'data-count': '1', 'data-target': answer.subject,
      // `width` as well as `flex`: the gate line is a grid now, and a flex basis is
      // inert in a grid track, which would have let this control size to its text.
      style: style({ flex: `0 0 ${GATE_ACT_WIDTH}`, width: GATE_ACT_WIDTH, ...place }),
    }, [
      el('button', {
        type: 'button',
        'data-act': answer.spec.act,
        'data-target': answer.subject,
        'data-sends': String(answer.spec.sends),
        disabled: live ? null : true,
        title: answer.why,
        // Geometry only, and the absence of the rest is the fix rather than an
        // omission. This control sits inside a `data-part="act-gutter"` container so
        // that the one rule set which paints a live act in the accent, refuses the
        // pointer on a dead one and inverts it on press applies to it -- and every
        // one of those rules was dead on arrival while this button spelled its own
        // `color`, `background`, `border` and `cursor:default` inline, because an
        // inline declaration outranks a stylesheet. The row gutter learned this in
        // the previous round; the ladder's copy of the control kept the defect.
        style: style({
          font: 'inherit', display: 'inline-flex', 'align-items': 'center', gap: '5px', width: '100%', 'box-sizing': 'border-box',
          'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'font-weight': WEIGHT.name,
          padding: '8px 6px', 'min-height': '36px',
        }),
      }, [P.glyph('act', answer.spec.act, { size: P.minAct, label: answer.spec.label }), el('span', {}, [answer.spec.label])]),
    ]);
  }

  function gateRow(answer, index, menuFor, terminal = false) {
    // The gate's own line, and under it the menu if this is the gate a right-click
    // was made on.
    //
    // The line was a `flex-wrap` row of four fixed-basis children: chip, a
    // `flex:0 0 8.5rem` name, a 104px act and a `flex:1 1 15rem` prose `why`. In the
    // ~300px content width of a dock that is four bases that cannot share a line, so
    // every one of them wrapped onto its own and the prose wrapped to five more:
    // 205px for a gate that carries one word and one button. Four of those made a
    // 869px ladder inside a 517px scroller, which is why `commit` sat at y=1168 --
    // the act was not below the fold because the face is long, it was below the fold
    // because each row was five times taller than its content.
    //
    // It is now a grid that states the sharing instead of leaving it to wrapping:
    // chip and act take exactly what they need, the name takes the rest of that line
    // and may wrap inside `minmax(0,1fr)` without pushing anything off it, and the
    // `why` -- when this gate still has one of its own -- spans the full width
    // underneath. The prose is never clamped and never ellipsised: req/811 §8-7 wants
    // a refusal's reason readable, and a reason you cannot finish reading is a reason
    // that was not given.
    const menu = menuFor(menuAt.gate(answer.gate));
    const said = firstSentenceOf(answer.why);
    return el('div', {
      'data-role': 'gate',
      'data-gate': answer.gate,
      'data-state': answer.state,
      'data-subject': answer.subject,
      // The face NAMES the region whose act must stay reachable; it does not place it.
      // `faces/held/tools/gate.mjs` `nothing-out-of-flow` bans `position:` in face
      // source, so the pin itself is a shell rule keyed on this attribute. The name is
      // the whole contract: the shell decides where, the face decides which.
      ...(terminal ? { 'data-pin': 'terminal-act' } : {}),
      style: style({ display: 'block', ...(index === 0 ? {} : { 'border-top': `1px solid ${T.rule}` }) }),
    }, [
      el('div', {
        'data-role': 'gate-line',
        style: style({
          display: 'grid', 'grid-template-columns': 'auto minmax(0,1fr)', 'align-items': 'center',
          'column-gap': '8px', 'row-gap': '3px', padding: `5px ${T.padX}`,
        }),
      }, [
        gateChip(answer),
        el('span', {
          'data-role': 'gate-name',
          style: style({
            'min-width': '0', 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
            'font-weight': WEIGHT.name, color: T.ink,
            // Deliberately NOT `PROSE`: `overflow-wrap:break-word` on a track this
            // narrow shatters a name into one letter per line -- measured, `an
            // inverse held` in a 6px track was 260px tall.
            'overflow-wrap': 'normal', 'word-break': 'normal',
          }),
        }, [answer.name]),
        gateActControl(answer, { 'grid-column': '1 / -1', 'justify-self': 'start' }),
        el('p', {
          'data-role': 'gate-why',
          style: style({
            'grid-column': '1 / -1', 'min-width': '0', margin: '0', color: T.attendant,
            'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'font-weight': WEIGHT.body, ...PROSE,
          }),
        }, [said]),
      ]),
      menu ? el('div', { style: style({ padding: `0 ${T.padX} 6px` }) }, [menu]) : null,
    ]);
  }

  function ladderSection(answers, selected, menuFor) {
    const terminal = answers[answers.length - 1];
    return section('gates', terminal.state, [
      P.box({
        name: selected ? `readiness: ${selected}` : 'readiness',
        count: answers.length,
        noun: 'gates',
        pill: gateChip(terminal, `${terminal.act} ${terminal.state}`),
        said: HELD_MESSAGES.LADDER,
        children: answers.map((answer, index) => gateRow(answer, index, menuFor, index === answers.length - 1)),
      }),
    ]);
  }

  // -- the menu a right-click opens ---------------------------------------------
  //
  // What it offers is not a second set of verbs. It is the row's own gutter -- the
  // four declared acts, every one of them drawn whether or not it sends -- answered
  // by the ladder's own gates, for the candidate the pointer was on: open, shut or
  // unknown, with the reason beside the word. One declaration, read through one
  // rule, on all three surfaces. A menu item carries `data-act` and carries a
  // `data-target` only where its gate is open, so pressing one travels the exact
  // path a gutter button travels -- the same queue, the same per-candidate lock, the
  // same log -- and there is no second sender anywhere in this file.
  //
  // MENU_PLACEMENT. A menu like this conventionally floats at the pointer. Nothing
  // this face draws is allowed to leave the flow: the defect the row grammar was
  // rebuilt around was an absolutely positioned note drawn on top of the row below
  // it with both texts left unreadable, and tools/gate.mjs holds out-of-flow
  // elements at zero over this face's source. So the menu is an ordinary block that
  // follows what it was opened on -- under the row for a right-click on a row or on
  // that row's gutter, under the gate for one in the ladder. It pushes what is below
  // it down, which is what a block does, and it is gone on the next click.

  const menuAt = Object.freeze({
    row: (id) => `row:${id}`,
    gate: (gate) => `gate:${gate}`,
  });

  const MENU_ITEM_HEIGHT = '36px';

  /** One act, as a line of the menu. Live only where its gate is open, and dead
   * lines keep their place with the reason beside them rather than disappearing --
   * the same refusal the gutter and the ladder both already make. */
  function menuAct(answer) {
    const live = answer.state === GATE_OPEN;
    return el('button', {
      type: 'button',
      'data-role': 'menu-act',
      'data-act': answer.spec.act,
      'data-target': answer.subject,
      'data-sends': String(answer.spec.sends),
      'data-state': answer.state,
      disabled: live ? null : true,
      title: answer.why,
      class: 'gx-move',
      // Colour is written here, unlike on the two act surfaces beside it, and the
      // difference is not carelessness: parts/src/surface.mjs names the act gutter
      // and the rows, and it does not name this part, so there is no rule for an
      // inline declaration to outrank. The two inks are the two that set already
      // uses for the same two states. A home for this part in that rule set is the
      // right end state and is named in this face's report rather than taken.
      style: style({
        font: 'inherit', display: 'flex', 'align-items': 'center', gap: '8px', width: '100%',
        'box-sizing': 'border-box', 'text-align': 'left', padding: `4px ${T.padX}`,
        'min-height': MENU_ITEM_HEIGHT, border: 'none', 'border-top': `1px solid ${T.rule}`,
        background: T.page, color: live ? T.act : T.attendant, cursor: live ? 'pointer' : 'not-allowed',
        'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'font-weight': WEIGHT.body,
      }),
    }, [
      P.glyph('act', answer.spec.act, { size: P.minAct, label: answer.spec.label }),
      el('span', { style: style({ flex: '0 0 5rem', 'font-weight': WEIGHT.name }) }, [answer.spec.label]),
      el('span', {
        'data-role': 'menu-why',
        style: style({ 'min-width': '0', color: T.attendant, 'font-weight': WEIGHT.body, ...PROSE }),
      }, [firstSentenceOf(answer.why)]),
    ]);
  }

  /** The value under the pointer, and whether taking it worked. The outcome is part
   * of this screen's state rather than an attribute set on a live element, because
   * every act on this face repaints the whole tree and an attribute written into the
   * old one would be gone before a reader read it. */
  function menuCopy(menu) {
    const value = menu.value ?? null;
    const said = value === null
      ? HELD_MESSAGES.MENU_NOT_A_VALUE
      : (menu.copy === 'copied' ? HELD_MESSAGES.COPIED : (menu.copy === 'failed' ? HELD_MESSAGES.COPY_FAILED : value));
    return el('button', {
      type: 'button',
      'data-role': 'menu-copy',
      'data-copy-value': value,
      'data-copied': menu.copy === 'copied' ? 'true' : null,
      'data-copy-failed': menu.copy === 'failed' ? 'true' : null,
      disabled: value === null ? true : null,
      title: said,
      class: 'gx-move',
      style: style({
        font: 'inherit', display: 'flex', 'align-items': 'center', gap: '8px', width: '100%',
        'box-sizing': 'border-box', 'text-align': 'left', padding: `4px ${T.padX}`,
        'min-height': MENU_ITEM_HEIGHT, border: 'none', 'border-top': `1px solid ${T.rule}`,
        background: T.page, color: value === null ? T.attendant : T.act, cursor: value === null ? 'not-allowed' : 'pointer',
        'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'font-weight': WEIGHT.body,
      }),
    }, [
      el('span', { style: style({ flex: '0 0 5rem', 'font-weight': WEIGHT.name }) }, ['copy value']),
      el('span', {
        'data-role': 'menu-copied',
        style: style({ 'min-width': '0', color: T.attendant, 'font-family': T.mono, 'font-size': T.time, 'font-weight': WEIGHT.body, ...PROSE }),
      }, [said]),
    ]);
  }

  function actionMenu(menu, read, pending) {
    const answers = gateAnswers(read, menu.subject, pending);
    return el('div', {
      'data-part': 'row-menu',
      'data-menu': menu.at,
      'data-subject': menu.subject,
      'data-count': String(answers.length + 1),
      role: 'menu',
      style: style({
        display: 'block', margin: '2px 0 6px', 'box-sizing': 'border-box',
        border: `1px solid ${T.rule}`, 'border-radius': T.radiusControl, background: T.page, overflow: 'hidden',
      }),
    }, [
      el('div', {
        'data-role': 'menu-head',
        title: HELD_MESSAGES.MENU,
        style: style({
          padding: `4px ${T.padX}`, color: T.attendant,
          'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'font-weight': WEIGHT.name,
        }),
      }, [menu.subject ?? firstSentenceOf(HELD_MESSAGES.GATE_NO_SUBJECT)]),
      ...answers.map((answer) => menuAct(answer)),
      menuCopy(menu),
    ]);
  }

  /** The one menu on the screen, drawn where it was opened and nowhere else. There
   * is no list of menus and no way to hold two: the state carries one `at`, so a
   * second right-click replaces the first rather than stacking on it. */
  const menuForState = (state, read) => (at) => (
    state.menu && state.menu.at === at ? actionMenu(state.menu, read, state.pending) : null
  );

  // -- the size and shape of the screen, before a word of it is read ------------

  function statBandFor(held) {
    const rows = held.ordered ? held.ordered.rows : null;
    const commit = ACTS.find((candidate) => candidate.act === 'commit');
    const heldMark = P.markOf('standing', 'held');
    return P.statBand([
      {
        noun: 'candidates',
        count: rows === null ? null : rows.length,
        mark: P.glyph('standing', 'held', { size: P.minReadable, label: 'held' }),
        tone: P.inkFor(heldMark),
        said: HELD_MESSAGES.BAND_CANDIDATES,
      },
      {
        // Short enough to be drawn. The band gives every segment the same width and
        // cuts a noun that does not fit with an ellipsis, and at this application's
        // own narrow viewport four columns leave about eleven characters: "commit-
        // ready" and "inverses held" both came back from the renderer as COMMIT-REA...
        // and INVERSES HE..., which is a label that has to be hovered to be read on
        // the one strip that exists to be read at a glance.
        noun: 'to commit',
        count: rows === null ? null : rows.filter((row) => commit.sends && typeof row.id === 'string' && row.id !== '').length,
        mark: P.glyph('act', 'commit', { size: P.minReadable, label: 'commit' }),
        said: HELD_MESSAGES.BAND_READY,
      },
      {
        noun: 'not drawn',
        count: rows === null ? null : held.received - rows.length,
        mark: P.glyph('structure', 'hole', { size: P.minReadable, label: 'not drawn' }),
        said: HELD_MESSAGES.BAND_UNDRAWN,
      },
      {
        noun: 'inverses',
        count: rows === null ? null : rows.filter((row) => P.reversalOf(row, rows).state === 'reversed').length,
        mark: P.glyph('act', 'undo', { size: P.minReadable, label: 'undo' }),
        said: HELD_MESSAGES.BAND_INVERSES,
      },
    ], { said: HELD_MESSAGES.BAND });
  }

  // -- the sections that are about the screen rather than about a row ----------

  function claimsSection(held) {
    const records = held.ordered ? held.ordered.rows : [];
    const claims = P.checkable(records, []);
    // No heading. This section is only ever drawn inside the control whose own label
    // is the word `claims`, so an h2 saying `claims` under a control saying `claims`
    // is the screen reading its own label back to the reader.
    return section('claims', records.length === 0 ? 'no-records' : 'checked', [
      aside(HELD_MESSAGES.NOT_A_RECEIPT, 'not-a-receipt'),
      el('div', { 'data-role': 'claims', 'data-count': String(claims.length) }, claims.map((claim) => el('div', {
        'data-claim': claim.id,
        'data-holds': String(claim.holds),
        style: style({
          display: 'grid', 'grid-template-columns': 'minmax(0,8rem) minmax(0,1fr)', gap: '10px', padding: '4px 0',
          'border-bottom': `1px solid ${T.rule}`, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
        }),
      }, [
        el('span', { 'data-role': 'verdict', style: style({ color: claim.holds ? T.ink : T.deny, 'font-weight': WEIGHT.name }) }, [claim.holds ? 'holds' : 'does not hold']),
        el('div', {}, [
          el('div', { 'data-role': 'claim', style: style({ color: T.ink, 'font-weight': WEIGHT.body, ...PROSE }) }, [claim.claim]),
          el('div', { 'data-role': 'detail', style: style({ color: T.attendant, 'font-weight': WEIGHT.body, ...PROSE }) }, [claim.detail]),
        ]),
      ]))),
    ]);
  }

  function notDrawnSection(held) {
    // Every line here used to open with `held: `. There is one face on this screen
    // and it is this one, so the word said which screen the reader was already
    // looking at, six times over, in the one place on the surface where the words
    // are all about what a reader cannot see.
    const lines = [];
    if (!held.ordered) {
      lines.push(plain('nothing was drawn, because the list was not read', 'denominator'));
    } else {
      lines.push(plain(`${held.ordered.rows.length} of ${held.received} rows drawn, read in ${held.envelope.requests ?? held.envelope.pages ?? 0} requests`, 'denominator'));
      if (held.envelope.stopped_at_budget) lines.push(plain(HELD_MESSAGES.TRUNCATED, 'truncated'));
      if (held.envelope.repeated_cursor) lines.push(plain(HELD_MESSAGES.REPEATED, 'repeated'));
      for (const drop of held.ordered.dropped) lines.push(plain(`an item at position ${drop.index} was not drawn, reason ${drop.why}`, 'dropped'));
      for (const repeat of held.ordered.repeated.id) lines.push(plain(`the identity ${repeat.value} arrived ${repeat.count} times`, 'repeated-id'));
    }
    // No heading, for the reason claimsSection above states: this section is only
    // drawn inside the control already labelled `omitted`.
    return section('not-drawn', held.answered ? 'stated' : 'nothing-read', [
      el('div', { 'data-role': 'denominators' }, lines),
      el('div', { 'data-role': 'omissions' }, UNDRAWN.map((entry) => el('div', {
        'data-omission': entry.what,
        style: style({
          display: 'grid', 'grid-template-columns': 'minmax(0,12rem) minmax(0,1fr)', gap: '10px', padding: '3px 0',
          'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
        }),
      }, [
        el('span', { style: style({ color: T.attendant, 'font-weight': WEIGHT.name }) }, [entry.what]),
        el('span', { style: style({ color: T.ink, 'font-weight': WEIGHT.body, ...PROSE }) }, [entry.why]),
      ]))),
      // The three words that are true of every withheld act are said once, above the
      // list, rather than at the head of each line -- and each act's own reason
      // already opens with them ("Declared, offered, and dimmed until ..."), so the
      // line used to say them twice in a row.
      DECLARATION.withheld.length > 0
        ? el('div', { 'data-role': 'withheld' }, [
          aside(HELD_MESSAGES.WITHHELD, 'withheld-standing'),
          ...DECLARATION.withheld.map((entry) => plain(`${entry.act}: ${entry.why}`, 'withheld-act')),
        ])
        : null,
    ]);
  }

  /** Zero-inclusive, unlike the band this section used to be: a screen from which
   * nothing has been sent yet keeps the container and states 0, because "nothing has
   * been sent from here" and "this screen does not keep a record of what it sends"
   * are different facts and the second one is not true. */
  function actsSection(acts) {
    return section('acts', acts.length === 0 ? 'none' : 'sent', [
      P.box({
        name: 'acts',
        count: acts.length,
        noun: 'answers',
        said: HELD_MESSAGES.ACT_LOG,
        children: boxed([
          el('div', {
            'data-role': 'act-log', 'data-count': String(acts.length), style: style({ padding: `0 ${T.padX}` }),
          }, acts.map((entry) => plain(
            `${entry.act} ${entry.id}: ${entry.outcome}${entry.code ? `, code ${entry.code}` : ''}${entry.detail ? `, ${entry.detail}` : ''}`,
            'act-entry',
          ))),
        ]),
      }),
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
    return frame([plain(HELD_MESSAGES.READING, 'reading')]);
  }

  /** The denominator, with the counts at the weight a number takes and the noun at
   * the weight a word takes -- the big-number/small-label rule applied to a line
   * that used to be one flat string of both.
   *
   * req/822_c7 (Owner #387/#388 冗長文字全掃): "N of M candidates" said the same
   * number twice when nothing was dropped between what was received and what was
   * drawn -- the ordinary case, not the exception. The denominator only earns its
   * place when it differs from the count; when the two agree, the count says so
   * once. */
  function headerWords(held) {
    if (!held.ordered) return [el('span', {}, ['unread'])];
    const drawn = held.ordered.rows.length;
    const said = drawn === held.received ? `${drawn}` : `${drawn} of ${held.received}`;
    return [
      el('span', {
        'data-role': 'header-count',
        style: style({ color: T.ink, 'font-family': T.mono, 'font-weight': WEIGHT.figure }),
      }, [said]),
      el('span', { style: style({ 'font-weight': WEIGHT.body }) }, [' candidates']),
    ];
  }

  function view(state) {
    // Measured, not estimated, and measured over exactly the work this function does:
    // the tree build, which is the same span faces/held's own bench times. It stops
    // before the footer that carries it, because a figure cannot include the drawing
    // of itself. What it is not is the paint -- that is a separate axis and this
    // window does not claim it.
    const started = performance.now();
    const selected = state.selected ?? null;
    const held = readHeld(state);
    const menuFor = menuForState(state, held);
    const heldNode = heldSection(held, state, selected, menuFor);
    const acts = state.acts ?? [];
    const actsNode = actsSection(acts);
    const band = statBandFor(held);
    const ladder = ladderSection(gateAnswers(held, selected, state.pending), selected, menuFor);
    const content = [
      band,
      ladder,
      heldNode,
      actsNode,
      claimsSection(held),
      notDrawnSection(held),
    ].filter(Boolean);
    const counts = new Map();
    for (const node of content) {
      for (const marked of find(node, (n) => n.attrs && 'data-mark' in n.attrs)) {
        counts.set(marked.attrs['data-mark'], (counts.get(marked.attrs['data-mark']) ?? 0) + 1);
      }
    }
    // Owner directive #335 (1): the explanatory surfaces are behind a click, in one
    // compact row. The candidates and the act log are the data and stay in the open.
    // Owner #340, in the order a stranger reads them: how many and of what kind
    // (the band), then what has to hold before any of them moves (the ladder), then
    // the candidates themselves.
    const screen = [
      headerLine(headerWords(held)),
      band,
      controlsRow([
        controlToggle('why', 'about this screen', aside(ORDER.reason, 'why-first'), { open: opened(state, 'why') }),
        controlToggle('legend', 'symbols and counts', legendBody(counts), { open: opened(state, 'legend') }),
        controlToggle('claims', 'what you can check', claimsSection(held), { open: opened(state, 'claims') }),
        controlToggle('omitted', 'what is not drawn', notDrawnSection(held), { open: opened(state, 'omitted') }),
      ]),
      ladder,
      P.detailFrame(el('div', { 'data-role': 'candidates' }, [heldNode, actsNode]), paneFor(selected, held)),
    ];
    return frame([
      ...screen,
      P.runtimeFooter({
        renderMs: performance.now() - started,
        source: held.ordered ? 'candidates' : null,
      }),
    ]);
  }

  // -- reading and acting -------------------------------------------------------

  async function read(port, previous = null) {
    const caller = callerFor(port);
    const held = await caller.fold(READS.held);
    return { held, acts: previous?.acts ?? [] };
  }

  function describeAct(result, spec, id) {
    const base = { act: spec.act, method: spec.method, id, outcome: result?.outcome ?? 'failed' };
    if (result?.outcome === 'refused') {
      return { ...base, code: result.gx_code ?? result.problem?.gx_code ?? null, detail: `${result.problem?.title ?? ''}: ${result.problem?.detail ?? ''}` };
    }
    if (result?.outcome === 'failed') return { ...base, detail: `${result.reason}: ${result.detail ?? ''}` };
    if (result?.outcome === 'absent') return { ...base, detail: String(result.reason ?? '') };
    return { ...base, detail: `${HELD_MESSAGES.SENT}, status ${result?.status ?? 'none'}` };
  }

  /** An act, and then a fresh read -- committing or cancelling a candidate is
   * answered by re-reading the list, never by this face rewriting the row it had
   * already drawn (the same rows-are-not-edited discipline faces/ledger holds).
   *
   * What comes back is the state that went in with the read and the log replaced,
   * rather than a fresh two-member object: the chosen row, the folds the reader has
   * open and the acts still in flight are this window's, not the server's, and a
   * read is not an event that should clear any of them. It used to return only the
   * read and the log, so pressing any act on this screen silently emptied the detail
   * pane -- the row stayed chosen in the reader's mind and nowhere else. */
  async function act(port, state, { act: name, id }) {
    const spec = ACTS.find((candidate) => candidate.act === name);
    if (!spec) throw new Error(`${HELD_MESSAGES.UNKNOWN_ACT}: ${name}`);
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
    return { ...state, ...next, acts: [...(state.acts ?? []), entry] };
  }

  /**
   * What a cell holds, for a menu offering to take it.
   *
   * The time cell draws a declared cut and carries the whole timestamp on itself as
   * `data-full`, so what a reader gets is the whole value and not the eight
   * characters of it they can see. Handing back the drawn form and calling it the
   * value would be the same failure this face's own legend spends a paragraph
   * refusing. A cell with nothing in it -- a declared hole -- answers null, and the
   * menu draws the offer dead with that as the reason rather than copying an empty
   * string and reporting success.
   */
  const valueOf = (cell) => {
    if (!cell || typeof cell.getAttribute !== 'function') return null;
    const full = cell.getAttribute('data-full');
    if (typeof full === 'string' && full !== '') return full;
    const shown = typeof cell.textContent === 'string' ? cell.textContent.trim() : '';
    return shown === '' ? null : shown;
  };

  // -- mount ---------------------------------------------------------------------

  function mount(host, port, notices = []) {
    if (!host || typeof host.appendChild !== 'function') throw new TypeError(HELD_MESSAGES.NO_HOST);
    if (!port || typeof port.fold !== 'function') throw new TypeError(HELD_MESSAGES.NO_PORT);
    void notices;

    const doc = host.ownerDocument ?? globalThis.document;
    // Same regression this project already found once, in faces/ledger: a mount
    // that never installs the sprite draws no marks in a real window even though a
    // fixture built around toHtml(parts.sheet()) draws every one. Guarded on
    // getElementById/body for the same reason faces/ledger's mount is -- the
    // structural test stand-in has neither.
    if (typeof doc.getElementById === 'function' && doc.body) P.installSheet(doc, P.element.render);
    // Owner directive #335 (2) and (4): the slim scrollbar and the figure/label type
    // scale are rules, not inline styles, so they arrive the same way the glyph
    // sprite does -- once per document, from the one module that owns them
    // (parts/src/surface.mjs). Same guard, same reason: the structural stand-in in
    // test/dom-stand-in.mjs is not a document and proves nothing about drawing.
    if (typeof doc.getElementById === 'function' && doc.body) P.installSurface(doc, P.element.render);
    let live = true;
    let state = null;

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
     * One act at a time, and every one of them written down.
     *
     * What this replaces read the closure's `state` at the moment of the click and
     * wrote the result back when its own answer arrived. Two clicks in one tick
     * therefore both read the state from before either of them, and the second
     * answer to arrive overwrote the first -- an act that was sent, answered, and
     * then not in the log, on a screen whose entire subject is what has and has not
     * happened. The two halves of the cure:
     *
     *  - the queue. Each act runs after the previous one has finished writing back,
     *    and reads `state` at that point rather than at its own click, so its log
     *    entry is appended to the log the one before it left.
     *  - the pending list. An act already in flight for the same row is not sent a
     *    second time, and both surfaces that draw that act draw it dead with the
     *    reason while it is out (actsFor above). A commit is not idempotent from
     *    this side of the wire, so this is a correctness rule and not a courtesy.
     */
    let queue = Promise.resolve();
    const withoutPending = (list, entry) => (list ?? []).filter((p) => !(p.act === entry.act && p.id === entry.id));

    const send = (name, id) => {
      const entry = { act: name, id };
      if (inFlight(state.pending, name, id)) return;
      // A menu is a question about one candidate, and sending an act is the answer.
      // It closes here rather than in the handler that opened it, so that the one
      // path both surfaces travel is also the one path that puts the menu away.
      state = { ...state, menu: null, pending: [...(state.pending ?? []), entry] };
      paint(view(state));
      queue = queue
        .then(() => act(port, state, entry))
        .then((next) => { state = { ...next, pending: withoutPending(next.pending, entry) }; })
        .catch((error) => {
          state = {
            ...state,
            pending: withoutPending(state.pending, entry),
            acts: [...(state.acts ?? []), { ...entry, outcome: 'failed', detail: error.message }],
          };
        })
        .then(() => { paint(view(state)); });
    };

    /**
     * Taking a value, and saying whether it was taken.
     *
     * The outcome goes into this screen's state and the screen is drawn again, rather
     * than being written as an attribute on the element that was pressed: every act
     * here repaints the whole tree, so an attribute set on the old one would be gone
     * before a reader could read it -- the same reason the open folds are state.
     * A window with no reachable clipboard says so; it does not draw the same control
     * it would have drawn on success.
     */
    const copyValue = (value) => {
      const settle = (outcome) => {
        if (!state.menu) return;
        state = { ...state, menu: { ...state.menu, copy: outcome } };
        paint(view(state));
      };
      const clip = typeof navigator === 'undefined' ? undefined : navigator.clipboard;
      if (!clip || typeof clip.writeText !== 'function') { settle('failed'); return; }
      clip.writeText(value).then(() => settle('copied'), () => settle('failed'));
    };

    const onClick = (event) => {
      const hit = event?.target;
      if (!hit || typeof hit.closest !== 'function' || !state) return;
      // Click-away. A press anywhere outside the menu is the reader saying "not
      // that" -- the menu goes, and whatever else the press meant still happens.
      // `closed` is carried because most branches below repaint on their own and
      // the ones that do not still have to draw the menu's absence.
      const inMenu = Boolean(hit.closest('[data-menu]'));
      const closed = !inMenu && Boolean(state.menu);
      if (closed) state = { ...state, menu: null };
      const settle = () => { if (closed) paint(view(state)); };

      // A fold is drawn from this face's own state, so the browser's default toggle
      // is refused and the state decides. Without this the two disagree on the next
      // repaint, and the repaint wins.
      const summary = hit.closest('summary');
      if (summary && typeof summary.closest === 'function') {
        const owner = summary.closest('[data-control]');
        const name = owner && typeof owner.getAttribute === 'function' ? owner.getAttribute('data-control') : null;
        if (name) {
          const open = new Set(state.open ?? []);
          if (open.has(name)) open.delete(name); else open.add(name);
          state = { ...state, open: [...open] };
          if (typeof event.preventDefault === 'function') event.preventDefault();
          paint(view(state));
          return;
        }
        settle();
        return;
      }
      const taking = hit.closest('[data-copy-value]');
      if (taking && typeof taking.getAttribute === 'function') {
        copyValue(taking.getAttribute('data-copy-value'));
        return;
      }
      // Owner directive #335 (3): choosing a row names it as the subject of the one
      // detail pane on this screen. It sends nothing and changes nothing on the
      // server -- it is this window deciding which record it is describing.
      const chosen = hit.closest('[data-select-row]');
      if (chosen && typeof chosen.getAttribute === 'function') {
        const id = chosen.getAttribute('data-select-row');
        if (id) {
          state = { ...state, selected: state.selected === id ? null : id };
          paint(view(state));
          return;
        }
        settle();
        return;
      }
      const target = hit.closest('[data-act]');
      if (!target || typeof target.getAttribute !== 'function') { settle(); return; }
      const name = target.getAttribute('data-act');
      const id = target.getAttribute('data-target');
      if (!name || !id) { settle(); return; }
      send(name, id);
    };

    /**
     * The second way to reach an act, and it reaches the same one.
     *
     * One listener rather than a handler attached per row and per control: a row is
     * redrawn on every act on this screen, so handlers hung on rows are handlers
     * re-hung on every paint, and one of those paints eventually forgets. What is
     * under the pointer decides the subject -- a row, or a control that already names
     * a row, or a gate in the ladder -- and everything else the menu says is computed
     * from the declaration the same way the gutter and the ladder compute it.
     *
     * A second right-click cannot stack a second menu: there is one `menu` on the
     * state and this replaces it.
     */
    const onContextMenu = (event) => {
      const hit = event?.target;
      if (!hit || typeof hit.closest !== 'function' || !state) return;
      const cell = hit.closest('[data-cell]');
      const gate = hit.closest('[data-gate]');
      const holder = hit.closest('[data-select-row]') ?? hit.closest('[data-target]');
      const at = gate
        ? menuAt.gate(gate.getAttribute('data-gate'))
        : (holder ? menuAt.row(holder.getAttribute('data-select-row') ?? holder.getAttribute('data-target')) : null);
      if (!at) return;
      const subject = gate
        ? (gate.getAttribute('data-subject') ?? state.selected ?? null)
        : (holder.getAttribute('data-select-row') ?? holder.getAttribute('data-target'));
      if (typeof event.preventDefault === 'function') event.preventDefault();
      state = { ...state, menu: { at, subject, value: valueOf(cell), copy: null } };
      paint(view(state));
    };

    /** Escape puts the menu away. Nothing else on this face listens for a key, so
     * this listener exists for exactly one thing and says so by being this short. */
    const onKey = (event) => {
      if (!state || event?.key !== 'Escape' || !state.menu) return;
      if (typeof event.preventDefault === 'function') event.preventDefault();
      state = { ...state, menu: null };
      paint(view(state));
    };

    if (typeof host.addEventListener === 'function') {
      host.addEventListener('click', onClick);
      host.addEventListener('contextmenu', onContextMenu);
      host.addEventListener('keydown', onKey);
    }

    paint(waitingView());

    const ready = read(port)
      .then((first) => {
        state = first;
        paint(view(state));
        return state;
      })
      .catch((error) => {
        paint(frame([aside(`${HELD_MESSAGES.UNREAD}. ${error.message}`, 'unread')]));
        return null;
      });

    const unmount = () => {
      live = false;
      if (typeof host.removeEventListener === 'function') {
        host.removeEventListener('click', onClick);
        host.removeEventListener('contextmenu', onContextMenu);
        host.removeEventListener('keydown', onKey);
      }
      clear();
    };
    unmount.ready = ready;
    return unmount;
  }

  return {
    DECLARATION, mount, read, act, view, waitingView, toRecord, callerFor, toHtml: P.element.toHtml,
  };
}

export const face = createFace();
export const mount = face.mount;
export { DECLARATION };
