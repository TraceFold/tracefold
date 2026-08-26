// SPDX-License-Identifier: Apache-2.0
// The notice face: what this window said, about itself.
//
// Every other face in this application reads a route and draws what the engine said
// about a record. This one reads nothing -- `CONSUMES` is empty (C-7) -- and draws
// what the window itself did while another face was asking: which method it called,
// through which layer, and what came back. A face that reported its own failures by
// asking the server about them would be reporting a failure through the thing that
// failed, so this screen is built to need nothing from the network to say anything
// true.
//
// Three properties are load-bearing.
//
// A window that has not called anything yet and a window whose own record was not
// handed to it are drawn as different facts, in different words -- the first is
// silence because nothing has happened, the second is silence because this face was
// not given anything to read, and a screen that cannot tell the two apart is a
// screen practising the exact failure this product exists to refuse.
//
// An entry, once drawn, is never edited. Every entry is rebuilt fresh from the
// window's array on every paint and frozen before it is drawn; there is no code
// path that holds a mutable copy of one.
//
// The count is never given alone. This screen states how many entries the window
// has recorded, how many of those are drawn, and -- once a budget is reached -- how
// many more have arrived and are deliberately left undrawn rather than pushed the
// already-drawn rows out of place to make room.

import {
  DECLARATION, ORDER, ROWS, UNDRAWN, OFFERS, QUESTION, FACE_ID,
} from './declaration.mjs';
import { parts as defaultParts } from './binding.mjs';

export const NOTICE_MESSAGES = {
  NO_HOST: 'a face is mounted into a host element, and none was given',
  NOT_GIVEN: 'this window was not handed its own record to draw. What follows is not an empty record of what it asked -- it is the absence of one',
  EMPTY: 'this window has not asked the server anything yet',
  ALL_DROPPED: 'entries arrived and none of them were records this face can draw; every one is listed below with the reason',
  ASKED: 'asked, not yet answered',
  UNKNOWN_OUTCOME: 'an outcome word this face holds no specific line for',
  // The drawn form is CAPPED_LINE: one clause with the figure in it. This is the
  // reason behind it, and it is on that line's own `title` rather than on the screen
  // -- it was two lines of prose in the open, and it said the count was "stated
  // below" when the census it meant is folded into a control above. The same reason
  // is also a declared omission (declaration.mjs UNDRAWN, "entries past the drawn
  // budget"), which is where a reader who wants it looks.
  CAPPED: 'the budget on this screen was reached. The rows already drawn are left standing rather than pushed out to make room for newer ones, because a reader partway down this screen should not find the top of it has moved.',
  CAPPED_LINE: 'more arrived and are not drawn',
  NO_METHOD_NAMED: '(no method named)',
  NO_TIME: '(no time recorded)',
  NO_OUTCOME: '(no outcome word)',
  DREW: 'notices drawn',
  // req/97 §4 (below the top five), and req/96 axis B's hard rule: a raw wire token
  // (`IDEMPOTENCY_CONFLICT`) and a raw payload fragment (`{"name":"..."}`) were drawn
  // on the product surface, which scores 0 on that axis -- the worst score there is.
  // Neither is deleted. The surface draws this plain form, and the server's own
  // spelling is one press away under a control that says what it is (the cure
  // faces/atlas built for its own leak, applied here).
  NO_ROUTE: 'no route by that name is one this window can reach',
  KEPT_WORDS: 'what the server sent back in its own spelling, kept exactly as it arrived rather than rewritten into this window\'s words. Nothing here is needed to read the screen; it is here because a word this window did not choose should still be findable.',
  KEPT_NOTHING: 'nothing on this screen came back carrying a word of the server\'s own',
  // What runtimeFooter prints after "read". This face reads no route and asks nothing:
  // its whole subject is the array of calls the window itself wrote down.
  SOURCE: 'this window\'s own record',
  BAND: 'the size and shape of what this window recorded, before any of it is read row by row',
  BOX_CALLS: 'one row per call, in the order the window recorded them. The figure here counts rows on this screen; the figure at the top of the screen counts everything the window wrote down, drawn or not.',
  BOX_RUN: 'the same call, asked again and again with nothing different about it. It is drawn once here and every occurrence is still counted and still listed inside.',
  // Owner #348 (2). What a right-click says on a face that sends nothing.
  //
  // The menu states this rather than leaving it to be inferred from a short list.
  // A reader who right-clicks a row is asking what can be done here, and "nothing
  // can be sent from this screen" is an answer; a menu that silently holds four
  // copies and no verbs looks like a menu whose verbs failed to load.
  MENU_NO_ACTS: 'nothing on this screen can be sent. This screen only ever reads what this window already wrote down, so what it can offer is a copy of it.',
  MENU_OF: 'what this row holds',
  COPIED: 'copied',
  // One sentence for both ways a copy can fail. A window with no clipboard at all and
  // a window whose write was refused are the same fact from a reader's side -- they
  // do not have the value -- and this face already spends its distinction-drawing on
  // the one place it matters (nothing happened, against nobody told me).
  COPY_FAILED: 'this window was not allowed to reach the clipboard, so nothing was copied',
};

/** The window's own outcome words, spelled here rather than imported: this face
 * reads no module of the membrane's, because reading one would be a route this
 * screen depends on existing, and C-7 is this face declaring it depends on none. */
const KNOWN_OUTCOMES = Object.freeze(['asked', 'answered', 'refused', 'failed', 'absent', 'elsewhere']);

/** How many entries this screen draws row by row before it switches to stating a
 * count instead. Chosen generously enough that an ordinary working session never
 * reaches it, and stated exactly so a session that does is not thinned in silence. */
export const DISPLAY_CAP = 200;

/** How often this face checks whether its window's array has grown. The array is a
 * plain reference the shell and the membrane both push onto directly, not something
 * this face is told about when it changes, so noticing growth means asking. */
export const POLL_MS = 400;

const isScalar = (value) => typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean';

function detailFor(raw, outcome, problem) {
  if (outcome === 'refused') {
    if (problem && typeof problem === 'object') {
      const title = isScalar(problem.title) ? String(problem.title) : '';
      const explanation = isScalar(problem.detail) ? String(problem.detail) : '';
      const joined = `${title}: ${explanation}`.replace(/^:\s*/, '').trim();
      if (joined) return joined;
    }
    if (typeof raw.said === 'string' && raw.said !== '') return raw.said;
    return null;
  }
  if (outcome === 'failed') {
    const envelope = raw.result && typeof raw.result === 'object' ? raw.result : raw;
    const reason = isScalar(envelope.reason) ? String(envelope.reason) : '';
    const explanation = isScalar(envelope.detail) ? String(envelope.detail) : '';
    const joined = `${reason}: ${explanation}`.replace(/^:\s*/, '').trim();
    return joined || null;
  }
  if (outcome === 'absent') return NOTICE_MESSAGES.NO_ROUTE;
  if (outcome === 'elsewhere') return typeof raw.said === 'string' && raw.said !== '' ? raw.said : null;
  return null;
}

/**
 * The server's own spelling, separated from this window's words at the point the
 * record is built rather than at the point it is drawn.
 *
 * Two things arrive on the wire in a shape nobody outside this codebase can read: a
 * token in capitals, and the fragment of a request echoed back verbatim. Both were
 * being drawn on the surface -- `req/96` axis B's hard rule scores that 0. Keeping
 * them on the record and off the row is what lets one control hold every one of them,
 * labelled, instead of each row carrying its own untranslatable line.
 */
function keptWordsFor(raw, outcome, code) {
  const envelope = raw.result && typeof raw.result === 'object' ? raw.result : raw;
  const kept = [];
  if (code !== null) kept.push(code);
  if (outcome === 'absent' && envelope.requested !== undefined) kept.push(JSON.stringify(envelope.requested));
  return kept.length === 0 ? null : Object.freeze(kept);
}

/**
 * One entry, as a record. Non-objects are handed back untouched so the order part
 * can drop them with a named reason instead of being handed a shape that hides it.
 */
export function toRecord(raw, index) {
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) return raw;
  const outcome = isScalar(raw.outcome) && String(raw.outcome) !== '' ? String(raw.outcome) : null;
  const through = raw.through === 'shell' ? 'shell' : 'membrane';
  const seq = Number.isInteger(raw.seq) ? raw.seq : null;
  const envelope = raw.result && typeof raw.result === 'object' ? raw.result : {};
  const problem = raw.problem && typeof raw.problem === 'object' ? raw.problem
    : (envelope.problem && typeof envelope.problem === 'object' ? envelope.problem : null);
  const status = Number.isInteger(raw.status) ? raw.status : (Number.isInteger(envelope.status) ? envelope.status : null);
  const code = isScalar(raw.gx_code) ? String(raw.gx_code)
    : (isScalar(envelope.gx_code) ? String(envelope.gx_code)
      : (isScalar(problem?.gx_code) ? String(problem.gx_code) : null));
  const at = typeof raw.at === 'number' && Number.isFinite(raw.at) ? new Date(raw.at).toISOString()
    : (typeof raw.at === 'string' && raw.at !== '' ? raw.at : null);
  return Object.freeze({
    id: seq !== null ? String(seq) : `unnumbered-${index}`,
    seq,
    at,
    through,
    method: isScalar(raw.method) && String(raw.method) !== '' ? String(raw.method) : null,
    verb: isScalar(raw.verb) ? String(raw.verb) : null,
    path: isScalar(raw.path) ? String(raw.path) : null,
    outcome,
    known: outcome !== null && KNOWN_OUTCOMES.includes(outcome),
    status,
    code,
    detail: outcome !== null ? detailFor(raw, outcome, problem) : null,
    kept: outcome !== null ? keptWordsFor(raw, outcome, code) : null,
  });
}

export function createFace({ parts = defaultParts } = {}) {
  const P = parts;
  const { el, style, find } = P.element;
  const T = P.tokens;

  // -- the weight scale, and the pieces of type that draw from it -----------------

  /**
   * Owner #348 (4): four weights, chosen by what a piece of text IS.
   *
   * Before this there were two -- 600 where somebody had reached for it and nothing
   * anywhere else -- so a count, the word naming that count, and a paragraph
   * explaining it all read at the same weight, and the eye had nothing to sort them
   * by. The rule the band already followed (a figure is what the eye lands on, the
   * word beside it is support) is here made mechanical: a caller names a role, the
   * role carries the weight, and no weight is written anywhere else in this file.
   *
   * `data-type` is stamped beside the weight so the rule is checkable from outside
   * rather than by reading -- test/notice.test.mjs walks the drawn tree and holds
   * that nothing sets a weight without naming its role, and that every role's weight
   * is the one declared here. A scale that lives only in a comment is a scale that
   * has already drifted.
   *
   * A figure is mono because a column of numbers that do not line up is a column
   * nobody can compare down.
   */
  const TYPE = Object.freeze({
    head: { weight: '700', family: T.sans },
    figure: { weight: '600', family: T.mono },
    lead: { weight: '600', family: T.sans },
    label: { weight: '500', family: T.sans },
    body: { weight: '400', family: T.sans },
  });

  /**
   * Owner #348 (4), the breaking half.
   *
   * `overflow-wrap: anywhere` was on eight of the text styles in this file, and it is
   * an instruction to break inside a word at any letter the moment a line is tight --
   * which is the mid-word break this atom forbids, written into the source as a
   * default. `break-word` breaks inside a word only when the word cannot fit its
   * column at all, which is the one case where the alternative is drawing over the
   * next column; and `text-wrap: pretty` is what stops the last line of a wrapped
   * sentence being a single stranded character. Both are measured rather than
   * asserted: tools/shoot.mjs reads every wrapped cell for a word wider than the
   * column it was given and for a last line too short to be a word.
   */
  const WRAP = Object.freeze({ 'overflow-wrap': 'break-word', 'text-wrap': 'pretty' });

  const typed = (tag, role, attrs, extra, children) => el(tag, {
    'data-type': role,
    ...attrs,
    style: style({
      'font-family': TYPE[role].family,
      'font-size': T.record,
      'line-height': T.recordLine,
      'font-weight': TYPE[role].weight,
      ...extra,
    }),
  }, children);

  const aside = (words, role = 'aside', { said = null } = {}) => typed('p', 'body', { 'data-role': role, title: said }, {
    margin: '0 0 6px', color: T.attendant, ...WRAP,
  }, [words]);

  const plain = (words, role = 'line') => typed('p', 'body', { 'data-role': role }, {
    margin: '0 0 4px', color: T.ink, ...WRAP,
  }, [words]);

  /** SS558/D-1 (req/38 SS576): the primary fact per entry (what was called) reads
   * bolder than the secondary lines under it (detail/wire-code/via), which is a
   * hierarchy that costs no column width -- this sits in the entry grid's flexible
   * `minmax(0,1fr)` cell, not a fixed one, so bolding it (rather than enlarging past
   * the body size shared with every other line) adds no clip risk the way a
   * font-size bump in a budgeted column would. */
  const lead = (words, role = 'line') => typed('p', 'lead', { 'data-role': role }, {
    margin: '0 0 4px', color: T.ink, ...WRAP,
  }, [words]);

  /** The word that names a value, and the value where the value is a number. Two
   * calls rather than two hand-written styles, so the pair cannot drift apart. */
  // `text-wrap: pretty` was on the body styles and not on this one, which is how a
  // three-word hint ended up with its last word alone on a line of its own the first
  // time the controls were made to share a row. A label wraps like anything else.
  const label = (words, extra = {}) => typed('span', 'label', {}, { color: T.attendant, 'text-wrap': 'pretty', ...extra }, [words]);
  const figure = (words, extra = {}) => typed('span', 'figure', {}, { color: T.ink, ...extra }, [words]);
  const bodyText = (words, extra = {}) => typed('span', 'body', {}, { color: T.ink, ...WRAP, ...extra }, [words]);

  const section = (name, state, children) => el('section', {
    'data-section': name,
    'data-state': state,
    style: style({ padding: '14px 0', 'border-top': `1px solid ${T.rule}`, background: T.page }),
  }, children.filter(Boolean));

  // -- compact header + bordered one-row controls (SS657 retrofit, req/38 SS657
  // Owner #317/#318; idiom proven by faces/atlas). See faces/ledger's own copy of
  // this comment for the fuller account of the five seat-confirmed defects. This
  // face's own vocabulary answer names three controls, not two: why (per-row
  // provenance expansion), legend (the symbol/glyph key) and tally (the running
  // count/aggregate line) -- all three now bordered, one row, with a hint.

  const headerLine = (words) => el('div', {
    'data-role': 'face-header',
    style: style({ display: 'flex', 'align-items': 'baseline', gap: '10px', padding: '10px 0 6px', 'font-family': T.sans }),
  }, [
    typed('span', 'head', {}, { 'font-size': T.head, 'line-height': T.headLine, color: T.ink }, [FACE_ID]),
    label(words),
  ]);

  /**
   * req/103 finding 2, on this face.
   *
   * `paint()` replaces the whole of the host on every repaint, and a disclosure's open
   * state is native `<details open>` on nodes that have just been destroyed -- so
   * whatever a reader had opened was shut the instant anything else changed. On the
   * ledger face the audit reached it by clicking a row. This face has no row to click
   * and no act to take, so the way in is the one thing that does change under a reader
   * here: the window records another call and the poll (POLL_MS) repaints. That is not
   * a rare path, it is the ordinary one -- a reader opens the legend to find out what a
   * mark means and the next call the window makes closes it again.
   *
   * The cure is that open-ness is read off the live document before the host is
   * cleared and handed back in as state, so the tree that replaces it is built already
   * open. It is deliberately not stored in a closure of its own: the document is where
   * the reader's choice actually lives, and a second copy of it in this module is a
   * second thing that can be wrong.
   */
  const isOpen = (state, key) => Array.isArray(state.open) && state.open.includes(key);

  /** A fold's own mark takes the act floor rather than the reading floor: a
   * disclosure is the one thing on this face a hand is meant to land on, and the
   * mark is doing more of the work of saying so than the word beside it is. */
  const foldMark = (open) => P.glyph('structure', open ? 'fold-open' : 'fold-shut', {
    size: P.floors.act,
    label: open ? 'open' : 'closed',
  });

  const controlToggle = (name, hint, body, { open = false } = {}) => el('details', {
    'data-role': 'control', 'data-control': name, 'data-open': String(Boolean(open)), open: open || null,
    style: style({
      // Seen in the picture, twice. First: four controls sized by their own words
      // wrapped two and two at four different widths, with a hand's-width gap left
      // after the third. Then, with a 210px basis, three and one -- and two of the
      // three narrow enough that their hints wrapped, so the row had two heights in
      // it as well as two lengths.
      //
      // The basis is what decides how many fit, so it is chosen for the two answers
      // that look deliberate and against the one that does not: at 300px, four fit on
      // a wide screen and exactly two on a narrow one. Three-and-one is arithmetically
      // out of reach at every width this face is drawn at.
      flex: '1 1 300px',
      'min-width': '0',
      border: `1px solid ${T.rule}`,
      'border-radius': T.radiusControl,
      background: T.page,
    }),
  }, [
    el('summary', {
      // req/822_c7 (Owner #387/#388 冗長文字全掃): the hint used to be drawn as its
      // own visible span next to the name, always on. `name` stays the
      // default-visible surface; `hint` rides the summary's own title (a hover)
      // and a `data-hint` attribute now.
      title: hint ?? null, 'data-hint': hint ?? null,
      style: style({
        display: 'flex', 'align-items': 'center', gap: '6px', 'min-height': '36px', 'box-sizing': 'border-box',
        padding: `0 ${T.padX}`, cursor: 'default', 'list-style': 'none',
      }),
    }, [
      foldMark(open),
      typed('span', 'lead', {}, { color: T.ink }, [name]),
    ].filter(Boolean)),
    el('div', { style: style({ padding: `0 ${T.padX} 10px` }) }, [body]),
  ]);

  const controlsRow = (children) => el('div', {
    'data-role': 'control-row',
    style: style({ display: 'flex', gap: '8px', 'flex-wrap': 'wrap', padding: '0 0 10px' }),
  }, children);

  /**
   * A key of marks that shows the marks.
   *
   * The first column printed the sheet's own name for each one -- `structure/hole`,
   * `effect/network`, `undefined` -- which is the identifier a programmer looks a mark
   * up by and is not a thing on this screen a reader can match against anything. What
   * a key is for is the shape: this column draws the mark itself, at the same size the
   * rows draw it, so a reader who saw something on a row can find the same thing here.
   * The identifier is still on the row for anything reading the tree (`data-mark-entry`),
   * where an identifier belongs.
   *
   * The stand-in mark is asked for the way this face already produces it -- by asking
   * the sheet for a key it does not hold, which is exactly what happens when an entry
   * arrives carrying an outcome word this face has no mark for.
   */
  const legendMark = (id) => {
    const [namespace, key] = id.includes('/') ? id.split('/') : ['structure', id];
    return P.glyph(namespace, key, { size: P.floors.readable, label: id });
  };

  const markTallyRows = (counts) => DECLARATION.marks.map((m) => el('div', {
    'data-mark-entry': m.mark, 'data-count': String(counts.get(m.mark) ?? 0),
    style: style({
      display: 'grid', 'grid-template-columns': '20px 2.5rem minmax(0,1fr)', gap: '10px', padding: '2px 0', 'align-items': 'start',
    }),
  }, [
    el('span', { style: style({ display: 'flex' }) }, [legendMark(m.mark)]),
    figure(String(counts.get(m.mark) ?? 0)),
    bodyText(m.from, { color: T.attendant }),
  ]));

  /** A block of explanation that carries no per-render data, folded shut behind a
   * one-word label rather than standing open as a banner (Owner #284, SS549). */
  const peripheral = (word, node, { name = null, open = false } = {}) => el('details', {
    'data-role': 'peripheral',
    'data-peripheral': name,
    'data-open': String(Boolean(open)),
    open: open || null,
    style: style({ margin: '0 0 6px' }),
  }, [
    el('summary', {
      style: style({
        display: 'flex', 'align-items': 'center', gap: '6px', 'min-height': '36px', 'box-sizing': 'border-box', cursor: 'default',
      }),
    }, [
      // This fold drew no mark, while the four controls above it drew one -- so two
      // disclosures on one screen announced themselves in two different languages,
      // and the declaration's own line for the fold marks claimed this one carried
      // them when it did not. Same mark, same floor, same meaning.
      foldMark(open),
      label(word),
    ]),
    node,
  ]);

  // -- one entry ----------------------------------------------------------------

  /**
   * How the call reached this window, as a glyph rather than a layer's name. This is
   * a product word's replacement for two internal ones: what used to print "through
   * the shell" or "through the membrane" on every row (RC-1) now draws a glyph the
   * legend explains once, and the raw word stays reachable in the glyph's own label
   * for anyone reading with assistive technology, framed as what it is rather than
   * printed as running prose.
   */
  function viaGlyph(record) {
    const known = record.through === 'shell' ? 'message' : 'network';
    return P.glyph('effect', known, { size: P.floors.readable, label: record.through === 'shell' ? 'answered inside this window' : 'answered over the network' });
  }

  function entryGlyph(record) {
    if (record.outcome === 'absent') return P.glyph('structure', 'hole', { size: P.floors.readable, label: 'a route the table does not carry' });
    if (record.outcome !== null && !record.known) return P.glyph('structure', record.outcome, { size: P.floors.readable, label: `${NOTICE_MESSAGES.UNKNOWN_OUTCOME}: ${record.outcome}` });
    return null;
  }

  /** What one entry says, split into the cells that vary and nothing that does not.
   * The comma-joined sentence this replaces put "through the shell" and "asked, not
   * yet answered" in the same run of prose as the values that actually differ row to
   * row; the legend (legendSection) says once what a call site and an outcome word
   * mean, and this line draws only what is different about this entry. */
  function entryAddress(record) {
    return record.verb && record.path ? `${record.verb} ${record.path}` : (record.method ?? NOTICE_MESSAGES.NO_METHOD_NAMED);
  }

  function entryOutcome(record) {
    return record.outcome === 'asked' ? NOTICE_MESSAGES.ASKED : (record.outcome ?? NOTICE_MESSAGES.NO_OUTCOME);
  }

  /**
   * Owner #348 (2), and the round-4 report's own worst-remaining-defect.
   *
   * What stood in this column was `reach`: one button per row, every one disabled,
   * every one carrying a paragraph saying why this face cannot jump to another. Eight
   * of them in a column on the representative screen. It was honest -- the reason was
   * true and it was stated -- and it was still the wrong thing, because a row has one
   * column for a thing a hand can do and that column was spent on a thing no hand can
   * do. Adding a right-click menu to a row whose only visible control is inert would
   * have made that worse: a second way to arrive at the same refusal.
   *
   * So the reason is retired into the declared omissions (declaration.mjs UNDRAWN, "a
   * way through to the face that reads this record" -- nothing is deleted, a reader
   * who wants to know why there is no jump still finds it, once, where the other
   * things this screen deliberately does not draw are listed), and the column carries
   * the one thing this face can genuinely make good on: handing a reader the value
   * that is in front of them.
   *
   * The width is fixed for the reason it was fixed before -- eight controls whose
   * width followed their content read as eight paragraphs, not as a column.
   */
  const GUTTER_WIDTH = '84px';

  /**
   * What one offer would hand over for one record, or null if this record has nothing
   * to give it. The four ids are declaration.mjs OFFERS and this is the only place
   * that knows what each one means.
   */
  function offerValue(record, id) {
    if (id === 'row') {
      return [
        record.at,
        entryAddress(record),
        entryOutcome(record),
        record.status === null ? null : `status ${record.status}`,
      ].filter((part) => typeof part === 'string' && part !== '').join(' ');
    }
    if (id === 'call') return record.verb && record.path ? `${record.verb} ${record.path}` : record.method;
    if (id === 'time') return record.at;
    if (id === 'code') return record.code;
    return null;
  }

  /** The row's offers, each carrying what it would give and whether it can. One
   * derivation, two drawings (the gutter and the menu), so the two cannot disagree. */
  function offersFor(record) {
    return OFFERS.map((offer) => {
      const value = offerValue(record, offer.id);
      const available = typeof value === 'string' && value !== '';
      return { ...offer, value: available ? value : null, available };
    });
  }

  const offerKey = (record, offer) => `${record.id}:${offer.id}`;

  /**
   * One offer as a control.
   *
   * `shape` is the only difference between the gutter's and the menu's: a fixed-width
   * verb in a column, or a full-width line naming what it would copy. Everything that
   * decides whether a hand may press it, and what it says when it may not, is the
   * same code -- an unavailable offer is drawn, disabled, and carries its own reason,
   * which is the rule the gutter already followed for a control it could not send.
   *
   * Whether the last press worked is drawn, never assumed. A clipboard write can be
   * refused by the browser (a document without focus, a sandbox without the
   * permission) and a control that looks identical either way is a control that lies
   * about a thing the reader cannot otherwise check.
   */
  function offerControl(record, offer, { shape = 'gutter', copied = null } = {}) {
    const mine = copied !== null && copied.key === offerKey(record, offer);
    const wide = shape === 'menu';
    return el('button', {
      type: 'button',
      'data-role': 'offer',
      'data-type': 'label',
      'data-shape': shape,
      'data-offer': offer.id,
      'data-offer-entry': record.id,
      'data-copied': mine && copied.ok ? 'true' : null,
      'data-copy-failed': mine && !copied.ok ? 'true' : null,
      disabled: offer.available ? null : true,
      title: offer.available ? `${offer.of}: ${offer.value}` : offer.why,
      style: style({
        font: 'inherit',
        display: 'flex',
        'align-items': 'center',
        gap: '8px',
        'text-align': 'left',
        width: wide ? '100%' : GUTTER_WIDTH,
        'box-sizing': 'border-box',
        'min-height': '36px',
        'font-family': T.sans,
        'font-size': T.record,
        'line-height': T.recordLine,
        'font-weight': TYPE.label.weight,
        // Seen in the picture: with the accent on the border as well as the word,
        // eight of these down the right edge were the loudest thing on the screen --
        // eight boxes drawing more eye than the eight rows they belong to. The accent
        // is spent on the word, which is the only accent on this face and therefore
        // still the thing that says a hand may act; the edge is the ordinary rule.
        color: offer.available ? T.act : T.attendant,
        background: T.page,
        border: wide ? 'none' : `1px solid ${T.rule}`,
        'border-radius': T.radiusControl,
        padding: '0 8px',
        // The application's rule set gives a pointer to rows, act gutters and
        // disclosures by selector, and this control is none of the three -- so a live
        // control drew with the ordinary arrow over it and read as text. Stated here
        // because a face cannot add a rule, and a control that does not announce
        // itself as pressable is the defect Owner #340 read on every one of these
        // screens.
        cursor: offer.available ? 'pointer' : 'not-allowed',
      }),
    }, [
      el('span', { style: style({ flex: '1', 'min-width': '0', overflow: 'hidden', 'text-overflow': 'ellipsis', 'white-space': 'nowrap' }) }, [wide ? offer.menu : offer.label]),
      mine ? label(copied.ok ? NOTICE_MESSAGES.COPIED : NOTICE_MESSAGES.COPY_FAILED, { 'white-space': 'nowrap' }) : null,
    ].filter(Boolean));
  }

  /** The one member of the declaration the gutter draws.
   *
   * It resolves that member and only that member. Building all four for every row and
   * throwing three away is a cost paid once per row on every paint -- measured at
   * 12.9ms against 8.5 for a thousand entries, which is the whole of the difference
   * this atom introduced -- and the menu, which does want all four, is drawn once. */
  const gutterOffer = OFFERS.find((offer) => offer.gutter === true) ?? null;

  function offerGutter(record, copied) {
    if (gutterOffer === null) return null;
    const value = offerValue(record, gutterOffer.id);
    const available = typeof value === 'string' && value !== '';
    return offerControl(record, { ...gutterOffer, value: available ? value : null, available }, { shape: 'gutter', copied });
  }

  /**
   * Owner #348 (2): the right-click menu.
   *
   * Three properties are held by shape rather than by a guard. It cannot stack,
   * because an open menu is one slot of state and a second right-click overwrites it
   * -- there is no code path that could put two on a screen. It cannot be left behind
   * by a repaint, because it is part of the tree a repaint rebuilds, not a node
   * appended beside one. And it is dismissed by Escape and by a press anywhere else,
   * both of which clear that one slot (mount() below).
   *
   * It is drawn over the row rather than under it. A menu in the flow of the list
   * would push every row below it down the moment a reader asked what a row holds,
   * which is the defect this application already measured once and cured everywhere
   * else; tools/gate.mjs allows exactly one positioned node on this face and checks
   * that the one drawn is this.
   *
   * The face declares no act (C-7), so this menu says so in a sentence instead of
   * quietly holding four copies and no verbs -- a reader who right-clicks is asking
   * what can be done here, and "nothing can be sent from this screen" is an answer.
   */
  /**
   * How big to assume it is before it is drawn, and why the assumption is not what
   * keeps it on the screen.
   *
   * `mount()` clamps the pointer's coordinates against this so that the ordinary case
   * puts the whole menu on screen at its natural height. That is a guess, and it was
   * wrong the first time it was measured -- 311px drawn against 300px declared, four
   * pixels of it off the bottom of a 700px window, found by right-clicking in a real
   * browser and reading the rectangle. So the guess is not what holds the property:
   * the drawn height is capped at the distance from where this menu starts to the
   * bottom of the window, which cannot be exceeded whatever the content does. The
   * width is exact because it is declared here and drawn from here.
   */
  const MENU_BOX = Object.freeze({ width: 260, height: 340 });

  function menuFor(record, at, copied) {
    return el('div', {
      'data-role': 'menu',
      'data-menu-entry': record.id,
      role: 'menu',
      style: style({
        position: 'fixed',
        left: `${at.x}px`,
        top: `${at.y}px`,
        width: `${MENU_BOX.width}px`,
        'max-height': `calc(100vh - ${at.y + 8}px)`,
        'overflow-y': 'auto',
        'box-sizing': 'border-box',
        'z-index': '2',
        // A heavier edge than a box's. Seen in the picture: with the same hairline
        // every container on this screen uses, a menu lying over the rows read as
        // part of them -- the one thing an overlay has to say about itself is that it
        // is on top. Still a neutral: the accent on this face is spent on "a hand may
        // act here" and nothing else, so it cannot be borrowed to lift a panel.
        border: `1px solid ${T.attendant}`,
        'border-radius': T.radiusContainer,
        background: T.page,
        padding: `6px ${T.padX}`,
      }),
    }, [
      el('div', { style: style({ display: 'grid', gap: '2px', padding: '0 0 6px' }) }, [
        label(NOTICE_MESSAGES.MENU_OF),
        typed('div', 'lead', {}, { color: T.ink, ...WRAP }, [entryAddress(record)]),
      ]),
      el('div', { 'data-role': 'menu-offers', style: style({ display: 'grid', gap: '2px', 'border-top': `1px solid ${T.rule}`, padding: '6px 0' }) },
        offersFor(record).map((offer) => offerControl(record, offer, { shape: 'menu', copied }))),
      aside(NOTICE_MESSAGES.MENU_NO_ACTS, 'menu-no-acts'),
    ]);
  }

  /**
   * req/97 §4, below the top five: "the notice status column wraps mid-word
   * (`partially_answe` / `red, status 207`) at 1426px".
   *
   * The width was never the viewport's. This column's track is `minmax(0, N)`, so N
   * is a ceiling and the column is that wide at 1426px and at 1280px and at 720px
   * alike -- the picture at any width shows the same break. The old ceiling was 7rem
   * (112px) and the longest word a row can carry, measured in the shipped font by
   * tools/shoot.mjs rather than guessed from a characters-per-pixel constant, needed
   * 117px with the status glued to it by a comma -- five pixels short, which is why
   * this survived two rounds of looking at it.
   *
   * Two changes, both aimed at that number. The status is its own line, so the word
   * is never asked to share one (worth about four of the five pixels on its own, and
   * therefore not a fix); and the ceiling is 9rem (144px), which is the measured
   * 117px plus the room a longer word than any this face has seen would take before
   * it broke. A word longer still is not lied about: it wraps, its full
   * text is in the cell's own `title`, and the reading in tools/shoot.mjs counts it
   * and prints the width it wanted -- an honest number to widen against, rather than
   * a defect nobody measured twice.
   */
  const OUTCOME_WIDTH = '9rem';

  /**
   * The outcome and the status, one under the other rather than joined by a comma.
   *
   * They were one string ("refused, status 409") in one cell, so the longest word on
   * the screen was always competing with a number for the same line -- see
   * OUTCOME_WIDTH above for the measurement. Two lines cost nothing here: this cell
   * already sat beside a two-line address block on the rows that carry a detail, so
   * the row's height is set by the address column, not by this one.
   */
  function outcomeCell(record) {
    return el('span', {
      'data-role': 'entry-outcome',
      title: entryOutcome(record),
      style: style({ 'min-width': '0', color: T.ink, ...WRAP }),
    }, [
      typed('span', 'body', { 'data-role': 'entry-outcome-word' }, { display: 'block', color: T.ink, ...WRAP }, [entryOutcome(record)]),
      // The number was inside the same string as the word ("status 409"), so the one
      // figure in this cell read at the weight of the sentence around it. Two spans:
      // the word that names it is a label, the number is a figure.
      record.status !== null
        ? el('span', { 'data-role': 'entry-status', style: style({ display: 'flex', gap: '5px', 'align-items': 'baseline' }) }, [
          label('status'),
          figure(String(record.status), { color: T.attendant }),
        ])
        : null,
    ].filter(Boolean));
  }

  function entryBlock(record, { last = false, copied = null } = {}) {
    const unknownGlyph = entryGlyph(record);
    return el('div', {
      'data-role': 'entry',
      'data-entry': record.id,
      // A row a right-click may be aimed at, and which record it would be about.
      //
      // Not `data-entry`, which is the count of individually reachable records
      // (tools/shoot.mjs reads it, and a second node carrying the same id would read
      // as one record drawn twice); a group's head row carries this and no
      // `data-entry`, for that reason. And not the same name the menu itself uses
      // for its own subject: the menu is drawn over the rows, so with one name for
      // both, a right-click inside an open menu found the menu as its own row and
      // reopened itself. Found by right-clicking, in a browser, not by reading.
      'data-menu-row': record.id,
      'data-through': record.through,
      style: style({
        display: 'grid',
        'grid-template-columns': `72px ${P.floors.readable + 2}px minmax(0,1fr) minmax(0,${OUTCOME_WIDTH}) ${GUTTER_WIDTH}`,
        'align-items': 'start',
        gap: '8px',
        // Inset to the box's own gutter, and no rule under the last row: the box
        // already draws one there and two 1px lines with nothing between them read
        // as one thick one nobody chose.
        padding: `4px ${T.padX}`,
        'border-bottom': last ? 'none' : `1px solid ${T.rule}`,
      }),
    }, [
      // SS558 body-text floor is 14px; this one column is a documented exception,
      // not an oversight -- it is a fixed 72px budgeted cell (matching the same
      // budget faces/ledger derives in req/100_PLACEMENT_SPEC.md), and bumping its
      // font past what that width was measured for reproduces the exact N-4 clip
      // defect req/03 found (a value clipped with its full text nowhere else on the
      // page). The full timestamp is not lost: it is the row's own data, printed
      // in full, just at the compact size its column was budgeted for.
      // The declared cut req/97 gap-list item 1 put on the shared row grid, applied
      // to this face's own 72px time cell for the same reason and by the same
      // function: an ISO-8601 timestamp drawn whole here was cut to "2026-08-2" with
      // the rest of it nowhere on the line. The time of day fits, and the whole
      // timestamp is on the cell.
      typed('span', 'body', {
        'data-role': 'entry-time',
        title: record.at ?? NOTICE_MESSAGES.NO_TIME,
        'data-full': record.at ?? null,
      }, {
        'font-family': T.mono, 'font-size': T.time, color: T.attendant, 'white-space': 'nowrap', overflow: 'hidden',
      }, [record.at ? P.drawnAt(record.at) : NOTICE_MESSAGES.NO_TIME]),
      el('span', { style: style({ display: 'flex' }) }, [unknownGlyph ?? viaGlyph(record)]),
      el('div', { style: style({ 'min-width': '0' }) }, [
        lead(entryAddress(record), 'entry-address'),
        record.detail ? aside(record.detail, 'entry-detail') : null,
      ].filter(Boolean)),
      outcomeCell(record),
      el('span', { style: style({ display: 'flex', 'justify-content': 'flex-end' }) }, [offerGutter(record, copied)].filter(Boolean)),
    ]);
  }

  /**
   * Abstraction duty (SS558, `INHERITED_PRINCIPLES.md` §24i②): a run of entries that
   * differ only in a trailing counter reads, to a person, as one repeated event, not
   * as N distinct facts -- the fixture this face ships (`notice-overflow`, 42 rows of
   * `get_transformations_0` .. `_41`) is exactly that shape. The key strips a trailing
   * run of digits off the address so `get_transformations_7` and `_8` group, while an
   * address that differs by more than a counter (a different route entirely) does
   * not -- grouping is never across `through`/`outcome`/`code`/`status`, only within
   * a run that already agrees on all four.
   */
  function abstractKey(record) {
    const addr = entryAddress(record).replace(/[0-9]+$/, '*');
    return [record.through, record.outcome ?? '', record.code ?? '', String(record.status ?? ''), addr].join('_');
  }

  /** Consecutive-only, never a global sort: two identical entries with a different
   * one between them are two facts about different moments and stay two rows. */
  function groupRuns(records) {
    const groups = [];
    for (const record of records) {
      const key = abstractKey(record);
      const last = groups[groups.length - 1];
      if (last && last.key === key) last.records.push(record);
      else groups.push({ key, records: [record] });
    }
    return groups;
  }

  /**
   * The standing a group's head carries, as a chip rather than as a loose glyph.
   *
   * This face draws no verdict, and the chip is here for its shape rather than for a
   * hue: a mark and a word on a bordered pill, drawn by the one function in this
   * application that decides how a standing looks, so a group head reads as a group
   * head wherever a reader has seen one before. Which mark it carries is the same
   * decision entryGlyph() already makes per row, and every one of the three is a mark
   * this face declares.
   */
  function standingPill(record) {
    if (record.outcome === 'absent') {
      return P.chip('structure', 'hole', { size: P.floors.readable, word: 'no such route', said: NOTICE_MESSAGES.NO_ROUTE });
    }
    if (record.outcome !== null && !record.known) {
      return P.chip('structure', record.outcome, { size: P.floors.readable, word: record.outcome, said: NOTICE_MESSAGES.UNKNOWN_OUTCOME });
    }
    const inside = record.through === 'shell';
    return P.chip('effect', inside ? 'message' : 'network', {
      size: P.floors.readable,
      word: inside ? 'in this window' : 'over the network',
      said: inside ? 'answered inside this window, without a call leaving it' : 'carried over the network to be answered',
    });
  }

  // reachLine is deliberately not drawn here: a grouped row stands for N records
  // that already agree on address/outcome/code/status (abstractKey), so "reach
  // the record this call named" would be ambiguous about which of the N it meant.
  // Each individual record is still reachable in the drill-down (data-entry on
  // every row of entry-group-detail below), just not through this control.
  //
  // Nor is the address drawn here any more, nor the group's own count, nor its
  // standing: a run is a box now (runBox below), and a box's head states its name,
  // its count and its standing. Drawing them again one line under the head would be
  // the same fact stated twice on one screen, which is the defect req/784 R-07
  // records as a class.
  function abstractedBlock(group, state) {
    const first = group.records[0];
    const times = group.records.map((r) => r.at ?? NOTICE_MESSAGES.NO_TIME);
    return el('div', {
      'data-role': 'entry-group',
      'data-count': String(group.records.length),
      // A right-click here is about the first of the run -- the record whose time and
      // whose outcome this row is already drawing. The rest are one fold away and
      // each of those rows carries its own.
      'data-menu-row': first.id,
      style: style({
        display: 'grid',
        'grid-template-columns': `72px minmax(0,1fr) minmax(0,${OUTCOME_WIDTH})`,
        'align-items': 'start',
        gap: '8px',
        padding: `4px ${T.padX}`,
        background: T.page,
      }),
    }, [
      // Found by looking at the picture, not at the markup: this cell drew the whole
      // ISO-8601 timestamp into the same 72px budget entryBlock cuts for, and the
      // overflow-hidden took it to "2026-08-2" with the "+199" beside it gone
      // entirely -- req/03's own N-4 defect, on the one row that had never been put
      // through the declared cut. `data-role` is on it now for the same reason:
      // tools/shoot.mjs's clip reading only examines nodes that carry one, so a cell
      // without one was never a candidate for the check that would have caught this.
      //
      // And the "+199" is gone rather than shortened. Putting the cut form back still
      // left "10:00:00 +199" at about 109px in a 72px cell -- the reading caught that
      // too, one round later -- and the count it carried is already in the head of the
      // box this row is inside, stated once, which is where a count belongs. What is
      // left here is the one fact this cell owns: when the first of them happened.
      typed('span', 'body', {
        'data-role': 'entry-group-time',
        title: `${times[0]} is the first of ${group.records.length}`,
        'data-full': times[0],
      }, {
        'font-family': T.mono, 'font-size': T.time, color: T.attendant, 'white-space': 'nowrap', overflow: 'hidden',
      }, [P.drawnAt(times[0])]),
      el('div', { style: style({ 'min-width': '0' }) }, [
        peripheral(`${group.records.length} occurrences`, el('div', { 'data-role': 'entry-group-detail' }, group.records.map((record) => el('div', {
          'data-entry': record.id,
          'data-menu-row': record.id,
          style: style({ display: 'grid', 'grid-template-columns': 'minmax(0,9rem) minmax(0,1fr)', gap: '10px', padding: '2px 0', 'font-family': T.mono, 'font-size': T.time, color: T.attendant }),
        }, [
          el('span', {}, [record.at ?? NOTICE_MESSAGES.NO_TIME]),
          el('span', {}, [entryAddress(record)]),
        ]))), { name: runName(first), open: isOpen(state, runName(first)) }),
      ]),
      outcomeCell(first),
    ]);
  }

  /**
   * A run of the same call, as an object on the screen.
   *
   * Owner #340 named the container idiom as the thing missing: a group of records is
   * an area with an edge, and the edge's head says what the group is, how many are in
   * it and what condition it is in. What this drew before was a row that happened to
   * carry an `x200` on the end of its address, which a reader has to already know how
   * to read. The name is the run's shared address with the counter taken off it --
   * the thing every member of the run has in common, which is exactly what the run is.
   */
  function runBox(group, state) {
    const first = group.records[0];
    return P.box({
      name: runName(first),
      count: group.records.length,
      noun: 'calls',
      pill: standingPill(first),
      said: NOTICE_MESSAGES.BOX_RUN,
      children: [abstractedBlock(group, state)],
    });
  }

  /** The address every member of a run shares: what abstractKey() grouped on, with
   * the trailing counter and the separator it hung from taken off, so the head reads
   * as a name rather than as a pattern with a wildcard in it. */
  function runName(record) {
    const stripped = entryAddress(record).replace(/[0-9]+$/, '').replace(/[_-]$/, '');
    return stripped === '' ? entryAddress(record) : stripped;
  }

  /**
   * Every group's own individual records still carry `data-entry` (inside the
   * singleton block, or inside the abstracted block's drill-down) -- abstraction
   * changes how many rows a run of identical facts draws, never how many facts are
   * still individually reachable and counted (`tools/shoot.mjs` `repeatedEntries`).
   *
   * The runs are the boxes. A run of one is not a box of its own -- a border around a
   * single row is a border that says nothing -- so consecutive runs of one collect
   * into one box for the calls they are, and a run of more than one becomes its own.
   * The walk is in recorded order and never sorts: a box appears where its first
   * member arrived, which is the only order this face claims (ROWS.order).
   */
  function entryBoxes(groups, state) {
    const boxes = [];
    let singles = [];
    const copied = state.copied;
    const flush = () => {
      if (singles.length === 0) return;
      const rows = singles;
      singles = [];
      boxes.push(P.box({
        name: 'calls',
        // Not "calls 8 calls". The head is a name and a count, and when the two are
        // the same word it reads as a stutter -- seen in the picture, not in the
        // markup. It counts rows here because that is what it holds: one row per
        // call, which is also what makes it a different figure from the band's own
        // "calls" (everything the window recorded, drawn or not).
        count: rows.length,
        noun: rows.length === 1 ? 'row' : 'rows',
        said: NOTICE_MESSAGES.BOX_CALLS,
        children: rows.map((record, index) => entryBlock(record, { last: index === rows.length - 1, copied })),
      }));
    };
    for (const group of groups) {
      if (group.records.length === 1) { singles.push(group.records[0]); continue; }
      flush();
      boxes.push(runBox(group, state));
    }
    flush();
    return boxes;
  }

  // -- the sections -------------------------------------------------------------

  /**
   * What every entry means, said once. RC-2's count -- 42 near-identical rows in the
   * overflow fixture -- was every entry restating "through the shell" and "asked, not
   * yet answered" verbatim; this section is where that meaning lives now, so a row
   * only has to carry the time, the call, and the one word that actually changed.
   */
  function legendBody(counts, shape) {
    return el('div', { 'data-role': 'legend' }, [
      el('div', { 'data-role': 'legend-marks' }, markTallyRows(counts)),
      // The tally that used to be a control of its own. Three of its words are figures
      // at the head of the screen now; what is left is the rest of the closed set,
      // still zero-inclusive, next to the marks it sits beside in meaning.
      tallyBody(shape),
      el('div', { 'data-role': 'legend-prose' }, [
        { name: 'via', value: 'a network glyph means the call crossed the network to be answered; a message glyph means it was answered inside this window without one leaving it.' },
        { name: 'outcome words', value: `${KNOWN_OUTCOMES.join(', ')} -- an outcome word outside this list is drawn with the hole mark and its own word kept in reach, never dropped to silence.` },
        { name: 'counts', value: 'each figure beside a mark counts the rows this screen drew carrying it. The band at the top of the screen counts calls, not marks, and the two are not the same tally.' },
        { name: 'the server\'s words', value: 'where the server answered in a spelling of its own rather than in words this window could use, that spelling is kept under the reference control and the row says the same thing in plain language.' },
      ].map((entry) => el('div', {
        'data-legend-entry': entry.name,
        style: style({ display: 'grid', 'grid-template-columns': 'minmax(0,7rem) minmax(0,1fr)', gap: '10px', padding: '2px 0' }),
      }, [
        label(entry.name),
        bodyText(entry.value),
      ]))),
      // What was here: the whole of the declared omissions, drawn a second time.
      //
      // Every one of the seven lines the `omitted` control draws was also drawn
      // inside this legend, in a different grid, with the same words -- about 1,700
      // visible characters of the same seven sentences, on one screen, twice. Two
      // rounds of retrofit had walked past it, because each control reads correctly
      // on its own and nothing compares two of them. It is drawn once now, in the
      // control whose whole subject it is.
    ]);
  }

  /**
   * The entries, as boxes.
   *
   * The heading this section used to open with is gone: every box states its own name
   * in its own head, and a word "entries" sitting above a box that already says what
   * it holds is a label for a label. The three silences keep their own words and each
   * one keeps a box with a border and a count, because a window that asked nothing, a
   * window whose whole record was unreadable, and a window that was never handed one
   * are three different facts and a screen that draws nothing for any of them has
   * lost two of the three.
   */
  function entriesSection(shape, state) {
    if (shape.calls === null) {
      return section('entries', 'not-given', [
        P.box({
          name: 'calls', count: null, noun: 'rows', said: NOTICE_MESSAGES.NOT_GIVEN, children: [wrapped(aside(NOTICE_MESSAGES.NOT_GIVEN, 'not-given'))],
        }),
      ]);
    }
    const drew = shape.calls === 0 ? 'empty' : (shape.drawn === 0 ? 'all-dropped' : 'drawn');
    const silence = shape.calls === 0
      ? aside(NOTICE_MESSAGES.EMPTY, 'empty')
      : (shape.drawn === 0 ? aside(NOTICE_MESSAGES.ALL_DROPPED, 'all-dropped') : null);
    return section('entries', drew, [
      shape.beyond > 0
        // `capped-line`, not `capped`: the omitted control draws a line of its own
        // about the same budget, and two different sentences answering to one name is
        // how a reading ends up measuring whichever one it happened to find first.
        ? aside(`${shape.beyond} ${NOTICE_MESSAGES.CAPPED_LINE}`, 'capped-line', { said: NOTICE_MESSAGES.CAPPED })
        : null,
      silence
        ? P.box({
          name: 'calls', count: 0, noun: 'rows', said: NOTICE_MESSAGES.BOX_CALLS, children: [wrapped(silence)],
        })
        : el('div', { 'data-role': 'entries', 'data-count': String(shape.drawn) }, entryBoxes(shape.groups, state)),
    ]);
  }

  /** A box's children sit against its own border; anything that is a line of words
   * rather than a row of a grid gets the same gutter every row in it has. */
  function wrapped(node) {
    return el('div', { style: style({ padding: `6px ${T.padX}` }) }, [node]);
  }

  /**
   * Everything this screen knows how to count, counted once.
   *
   * One walk over the records, one place the figures come from. The band at the top,
   * the boxes, the residual tally and the omitted census all read this -- so a number
   * cannot disagree with itself between two parts of the same screen, which is the
   * way two-totals defects actually get in.
   *
   * `null` throughout when this face was not handed its own record: the count of
   * calls a window made and the absence of any record of that window are two facts,
   * and a zero here would be this screen claiming the first when it holds the second.
   */
  function census(notices, ordered) {
    if (notices === null) {
      return {
        calls: null, drawn: null, beyond: null, dropped: null, repeats: null, byWord: null, groups: [], dropReasons: [],
      };
    }
    const capped = ordered.rows.slice(0, DISPLAY_CAP);
    const byWord = new Map(KNOWN_OUTCOMES.map((word) => [word, 0]));
    for (const record of ordered.rows) {
      const key = record.outcome ?? NOTICE_MESSAGES.NO_OUTCOME;
      byWord.set(key, (byWord.get(key) ?? 0) + 1);
    }
    const groups = groupRuns(capped);
    return {
      calls: notices.length,
      drawn: capped.length,
      beyond: ordered.rows.length - capped.length,
      dropped: ordered.dropped.length,
      dropReasons: ordered.dropped,
      repeats: groups.filter((group) => group.records.length > 1).length,
      byWord,
      groups,
    };
  }

  /**
   * The band, and the decision it forced.
   *
   * This face already had a `tally` control: a counted table of every outcome word,
   * zero-inclusive. Three of those words are now figures at the head of the screen,
   * and a screen that states the same count twice is the defect req/784 R-07 records
   * as a class -- so the band wins and the tally keeps only what the band does not
   * name. The two together still cover this face's closed set exactly once, which is
   * the property test/notice.test.mjs holds rather than the arrangement.
   *
   * Every figure is a count of this window's own record. None of them is a standing:
   * a call this server refused is not a candidate this engine denied, and giving the
   * refused figure the deny ink would be this screen claiming a verdict it was never
   * sent. The one mark on the band is a mark this face declares, over the one figure
   * whose meaning it already carries on the rows.
   */
  const BAND_WORDS = Object.freeze(['answered', 'refused', 'absent']);

  function bandSegments(shape) {
    return [
      { noun: 'calls', count: shape.calls, said: 'everything this window wrote down, drawn and undrawn alike -- every figure to the right of this one is part of it' },
      { noun: 'answered', count: shape.byWord?.get('answered') ?? null, said: 'calls the server answered' },
      { noun: 'refused', count: shape.byWord?.get('refused') ?? null, said: 'calls the server refused, with its reason on the row' },
      {
        noun: 'absent',
        count: shape.byWord?.get('absent') ?? null,
        mark: P.glyph('structure', 'hole', { size: P.floors.readable, label: 'a route the table does not carry' }),
        said: NOTICE_MESSAGES.NO_ROUTE,
      },
      { noun: 'repeats', count: shape.repeats, said: 'runs of the same call with nothing different about them, each drawn as one box instead of many rows' },
    ];
  }

  /**
   * What the band does not name, still zero-inclusive.
   *
   * Every word of this face's closed set that is not a figure at the top, plus two
   * things that set does not cover: an outcome word this face received and does not
   * recognise (counted, never folded into "unknown"), and the items that were not
   * records at all. The reason an item could not be placed is the server's own or
   * this application's, not a reader's, so it is stated as plain words here and the
   * machine's own name for it is kept under `reference` with everything else of that
   * kind.
   */
  function tallyRows(shape) {
    const rows = [];
    for (const [word, count] of shape.byWord ?? []) {
      if (BAND_WORDS.includes(word)) continue;
      rows.push({ word, count });
    }
    if (shape.dropped > 0) rows.push({ word: 'not a record', count: shape.dropped });
    return rows;
  }

  function tallyBody(shape) {
    if (shape.byWord === null) return aside(NOTICE_MESSAGES.NOT_GIVEN, 'tally-not-given');
    return el('div', { 'data-role': 'tally' }, tallyRows(shape).map(({ word, count }) => el('div', {
      'data-tally-entry': word, 'data-count': String(count),
      style: style({ display: 'flex', gap: '6px', 'align-items': 'baseline', padding: '2px 0' }),
    }, [
      // Two spans, the same reading ("failed: 0"): the word that names the count is a
      // label and the count itself is a figure, so a reader scanning this column for
      // a number finds numbers rather than a run of sentences with numbers in them.
      label(`${word}:`),
      figure(String(count), { color: T.attendant }),
    ])));
  }

  /**
   * The words this window did not choose, kept and labelled.
   *
   * faces/atlas's cure for the same defect, applied to data rather than to prose: the
   * surface says what happened in this application's own words, and the server's own
   * spelling of it -- a token in capitals, a fragment of a request echoed back -- is
   * one press away under a control that says what it is. Nothing is deleted; a
   * refusal's own code was evidence before this change and is evidence after it.
   */
  function referenceBody(records) {
    // A face that was handed no record has not seen the server say anything, which is
    // a different fact from having seen it say everything in words this window could
    // use -- the same distinction this whole screen exists to keep.
    if (records === null) return aside(NOTICE_MESSAGES.NOT_GIVEN, 'reference-not-given');
    const kept = records.filter((record) => record !== null && typeof record === 'object' && Array.isArray(record.kept));
    if (kept.length === 0) return aside(NOTICE_MESSAGES.KEPT_NOTHING, 'reference-empty');
    return el('div', { 'data-role': 'internal-reference' }, [
      aside(NOTICE_MESSAGES.KEPT_WORDS, 'reference-why'),
      ...kept.map((record) => el('div', {
        'data-reference': record.id,
        style: style({
          display: 'grid', 'grid-template-columns': 'minmax(0,14rem) minmax(0,1fr)', gap: '10px', padding: '2px 0', ...WRAP,
        }),
      }, [
        label(entryAddress(record), WRAP),
        el('span', {
          style: style({
            display: 'flex', 'flex-wrap': 'wrap', gap: '10px', color: T.ink, 'font-family': T.mono, 'font-size': T.time, ...WRAP,
          }),
        }, record.kept.map((word) => el('span', { 'data-kept': word }, [word]))),
      ])),
    ]);
  }

  /**
   * What is not on the screen, and why.
   *
   * The denominator line this used to open with ("N of M entries drawn") is gone from
   * here, and not from the screen: M is the band's first figure and N is the count in
   * the head of the box the rows are in, both in the open where a reader meets them
   * before this control exists. Restating both inside a fold is how a screen ends up
   * with two totals nobody can reconcile. What is left here is what genuinely has
   * nowhere else to be: the items that could not be placed, one line each with its
   * position and its reason.
   */
  function notDrawnSection(shape) {
    const lines = [];
    if (shape.calls === null) {
      lines.push(plain('nothing was drawn, because this face was not handed its own record to read', 'denominator'));
    } else {
      // The budget line that used to open this list is gone from here. The count of
      // what arrived past it is already on the open surface, above this control, and
      // the reason behind that count is already the last of the declared omissions
      // below -- so this was the third place on one screen saying one thing.
      for (const drop of shape.dropReasons) lines.push(plain(`an entry at position ${drop.index} was not drawn, reason ${drop.why}`, 'dropped'));
      if (lines.length === 0) lines.push(plain('every entry this window recorded is drawn', 'denominator'));
    }
    // No heading. This section is drawn inside a control whose own summary reads
    // `omitted`; a heading saying `omitted` under it is a label for a label.
    return section('not-drawn', shape.calls === null ? 'nothing-read' : 'stated', [
      el('div', { 'data-role': 'denominators' }, lines),
      el('div', { 'data-role': 'omissions' }, UNDRAWN.map((entry) => el('div', {
        'data-omission': entry.what,
        style: style({
          display: 'grid', 'grid-template-columns': 'minmax(0,12rem) minmax(0,1fr)', gap: '10px', padding: '3px 0',
        }),
      }, [
        label(entry.what, WRAP),
        bodyText(entry.why),
      ]))),
    ]);
  }

  // -- the whole screen -----------------------------------------------------------

  function frame(children) {
    return el('div', {
      'data-face': FACE_ID,
      'data-question': QUESTION,
      style: style({
        display: 'block', background: T.page, color: T.ink, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, padding: `12px ${T.padX} 40px`,
      }),
    }, children.filter(Boolean));
  }

  /** The header carries no figure any more. Every number on this screen is in the
   * band immediately under it, at a size a figure deserves, and a count restated in
   * 14px grey above it would be the same fact twice. What is left is what a heading
   * is for: which screen this is, and the one thing worth knowing before the figures
   * -- that nothing here is sorted. */
  function headerWords(shape) {
    return shape.calls === null ? 'not given its own record' : 'in the order this window recorded them';
  }

  /** The clock, or the honest absence of one. `performance` is in every browser this
   * face is drawn in and in the runtime its tests run under, but a face may not
   * assume its host: without a clock the footer draws a dash rather than a figure
   * nobody measured. */
  function clock() {
    return typeof globalThis.performance?.now === 'function' ? globalThis.performance.now() : null;
  }

  /**
   * The open menu, or nothing.
   *
   * It is resolved against the records this paint just built rather than against a
   * copy taken when the menu was opened: an entry is rebuilt fresh every paint and
   * frozen (this face's third load-bearing property), so a menu holding its own copy
   * of one would be a second version of a record that cannot be edited. If the record
   * the menu names is no longer among them, nothing is drawn -- a menu about a row
   * that is not on the screen is not a menu.
   */
  function menuNode(state, records) {
    if (!state.menu) return null;
    const record = records.find((entry) => entry !== null && typeof entry === 'object' && entry.id === state.menu.entry);
    return record ? menuFor(record, state.menu, state.copied) : null;
  }

  function view(state) {
    const started = clock();
    const notices = state.notices;
    const records = notices ? notices.map((raw, index) => toRecord(raw, index)) : [];
    const ordered = notices ? P.order(records, { by: 'as-recorded' }) : null;
    const shape = census(notices, ordered);
    // Owner directive #335 (1): the entries are the data and stay in the open; the
    // omitted census is explanatory and moves into the control row below, drawn once.
    const omitted = notDrawnSection(shape);
    const content = [entriesSection(shape, state)].filter(Boolean);
    const counted = [...content, omitted];
    const counts = new Map();
    for (const node of counted) {
      for (const marked of find(node, (n) => n.attrs && 'data-mark' in n.attrs)) {
        counts.set(marked.attrs['data-mark'], (counts.get(marked.attrs['data-mark']) ?? 0) + 1);
      }
    }
    const drawn = [
      headerLine(headerWords(shape)),
      // Owner #340: before a word is read, the size and shape of this screen's
      // population. Every segment is a count taken from the same census the rows and
      // the omitted table are drawn from; a number this face cannot know is a dash.
      P.statBand(bandSegments(shape), { said: NOTICE_MESSAGES.BAND }),
      // The hints. `omitted -- what is not drawn` said one thing twice in six words,
      // and every one of the four carried a leading pair of dashes that meant
      // nothing; the name and its hint are already two weights apart.
      controlsRow([
        controlToggle('why', 'the order, and why', aside(ORDER.reason, 'why-last'), { open: isOpen(state, 'why') }),
        controlToggle('legend', 'marks and counts', legendBody(counts, shape), { open: isOpen(state, 'legend') }),
        controlToggle('omitted', 'what is left out, and why', omitted, { open: isOpen(state, 'omitted') }),
        controlToggle('reference', 'the server\'s own words', referenceBody(notices === null ? null : records), { open: isOpen(state, 'reference') }),
      ]),
      ...content,
      // Last in the tree, and out of the flow: a menu drawn before the rows would be
      // painted under them.
      menuNode(state, records),
    ];
    // Measured, not estimated: the tree above is the whole of what view() builds on
    // every paint, and this is how long building it took, in this window, just now.
    // The footer itself is the only node outside the measurement, which is the one
    // thing it could not honestly include.
    const renderMs = started === null ? null : clock() - started;
    return frame([...drawn, P.runtimeFooter({ renderMs, source: NOTICE_MESSAGES.SOURCE })]);
  }

  // -- reading ------------------------------------------------------------------

  /** There is nothing to await: this face's whole record is the array it was
   * handed at mount time, and reading it is looking at it, not asking anyone.
   *
   * `open` is the second half of the state, and it is not read from the array: it is
   * which disclosures the reader has open, handed in by mount() from the document
   * that is about to be replaced (req/103 finding 2). A caller that does not have one
   * -- a test, the static fixture writer -- passes nothing and gets the screen a
   * reader meets on their first look at it. */
  function read(notices, open = null, { menu = null, copied = null } = {}) {
    return {
      notices: Array.isArray(notices) ? notices : null,
      open: Array.isArray(open) ? Object.freeze([...open]) : [],
      // One slot. A second right-click writes over the first, which is why two menus
      // on one screen is not a thing this face has to guard against -- there is no
      // shape of this state that holds two.
      menu: menu && typeof menu.entry === 'string' ? Object.freeze({ entry: menu.entry, x: Number(menu.x) || 0, y: Number(menu.y) || 0 }) : null,
      copied: copied && typeof copied.key === 'string' ? Object.freeze({ key: copied.key, ok: copied.ok === true }) : null,
    };
  }

  // -- mount ----------------------------------------------------------------------

  /**
   * `pollMs` is a fourth parameter with a default, so `mount.length` is still 3 and
   * the shell's call site is unaffected -- it always calls with exactly the three
   * arguments the contract states. A test can pass a small `pollMs` to see growth
   * noticed without waiting on a real clock; production leaves it at `POLL_MS`.
   */
  function mount(host, port, notices = [], { pollMs = POLL_MS } = {}) {
    if (!host || typeof host.appendChild !== 'function') throw new TypeError(NOTICE_MESSAGES.NO_HOST);
    // C-7: this face is handed the membrane's surface like every other face, and
    // never calls it. The line below is the whole of what it does with it.
    void port;

    const doc = host.ownerDocument ?? globalThis.document;
    // Same regression fix as the ledger face's mount (req/97 real-window row): the
    // sprite every mark's <use> points at was never installed by a real mount, only
    // by the static fixture writer's own page. Guarded the same way, for the same
    // reason -- test/dom-stand-in.mjs has neither getElementById nor a body.
    if (typeof doc.getElementById === 'function' && doc.body) P.installSheet(doc, P.element.render);
    // Owner directive #335 (2) and (4): the slim scrollbar and the figure/label type
    // scale are rules, not inline styles, so they arrive the same way the glyph
    // sprite does -- once per document, from the one module that owns them
    // (parts/src/surface.mjs). Same guard, same reason: the structural stand-in in
    // test/dom-stand-in.mjs is not a document and proves nothing about drawing.
    if (typeof doc.getElementById === 'function' && doc.body) P.installSurface(doc, P.element.render);
    const list = Array.isArray(notices) ? notices : null;
    let live = true;
    let lastLength = list ? list.length : -1;
    // The two pieces of state a hand makes. Neither is in the window's record, and
    // neither survives an unmount: what a reader last right-clicked and what they
    // last copied are facts about this viewing of the screen, not about the window.
    let menu = null;
    let copied = null;

    const clear = () => { while (host.firstChild) host.removeChild(host.firstChild); };

    /**
     * req/103 finding 2's cure, read from the document rather than remembered.
     *
     * The reader's choice lives in the live `<details open>` nodes about to be
     * destroyed; this takes it off them one instant before, and view() builds the
     * replacement already open. A stand-in host has no querySelectorAll and gets an
     * empty list, which is the correct answer for a host that draws nothing a reader
     * could have opened.
     */
    const openHere = () => {
      if (typeof host.querySelectorAll !== 'function') return [];
      const open = [];
      for (const node of host.querySelectorAll('[data-control],[data-peripheral]')) {
        const key = node.getAttribute('data-control') ?? node.getAttribute('data-peripheral');
        if (key !== null && node.open === true) open.push(key);
      }
      return open;
    };

    const paint = () => {
      if (!live) return;
      const open = openHere();
      clear();
      host.appendChild(P.element.render(doc, view(read(list, open, { menu, copied }))));
      lastLength = list ? list.length : -1;
    };

    // ---- what a hand does here (Owner #348 (2)) ---------------------------------
    //
    // Three listeners, all on the host or the document, none on a row: every row on
    // this screen is rebuilt on every paint, so a listener attached to one is a
    // listener attached to a node that is about to stop existing. Delegation means
    // there is exactly one of each for the life of the mount, and unmount takes them
    // off again -- a face that leaves a document listener behind is a face that keeps
    // answering after it has been taken off the screen.

    /** The nearest row a right-click is aimed at, and which record it is about. A
     * press inside the open menu is aimed at no row: the menu is drawn over the list,
     * so without this a right-click on it would find whatever is underneath. */
    const menuSubjectAt = (target) => {
      if (!target || typeof target.closest !== 'function') return null;
      if (target.closest('[data-role="menu"]')) return null;
      const row = target.closest('[data-menu-row]');
      return row ? row.getAttribute('data-menu-row') : null;
    };

    /**
     * Where the menu goes, decided here rather than in the drawing.
     *
     * The pointer's coordinates are the answer until the menu would hang off an edge,
     * and then the answer is the edge -- a menu half off the screen is a menu whose
     * bottom entries cannot be pressed. Clamped before it is stored, so `view()` stays
     * a function of state alone and a test can hand it coordinates without a window.
     */
    const placeMenu = (event) => {
      const box = MENU_BOX;
      const w = typeof globalThis.innerWidth === 'number' ? globalThis.innerWidth : box.width;
      const h = typeof globalThis.innerHeight === 'number' ? globalThis.innerHeight : box.height;
      const x = Math.max(4, Math.min(Number(event.clientX) || 0, w - box.width - 4));
      const y = Math.max(4, Math.min(Number(event.clientY) || 0, h - box.height - 4));
      return { x, y };
    };

    const onContextMenu = (event) => {
      const entry = menuSubjectAt(event.target);
      if (entry === null) return;
      if (typeof event.preventDefault === 'function') event.preventDefault();
      menu = { entry, ...placeMenu(event) };
      copied = null;
      paint();
    };

    /**
     * The clipboard, and saying which way it went.
     *
     * The shell's own copy control settled this shape already: try it, and record
     * whether it worked, rather than drawing a control that looks the same whether or
     * not anything happened. A window with no clipboard at all is the same answer as
     * a refused write, and both are drawn.
     */
    const takeCopy = (key, value) => {
      const clip = typeof globalThis.navigator === 'object' && globalThis.navigator ? globalThis.navigator.clipboard : null;
      if (!clip || typeof clip.writeText !== 'function') {
        copied = { key, ok: false };
        paint();
        return;
      }
      clip.writeText(value).then(
        () => { copied = { key, ok: true }; paint(); },
        () => { copied = { key, ok: false }; paint(); },
      );
    };

    /** The record an offer names, rebuilt from the window's array rather than read
     * back off the screen -- what gets copied is derived from the same frozen record
     * the row was drawn from, never from a string sitting in the document. */
    const recordFor = (id) => {
      if (!list) return null;
      for (let i = 0; i < list.length; i += 1) {
        const record = toRecord(list[i], i);
        if (record !== null && typeof record === 'object' && record.id === id) return record;
      }
      return null;
    };

    const onClick = (event) => {
      const target = event.target;
      const control = target && typeof target.closest === 'function' ? target.closest('[data-role="offer"]') : null;
      if (control && !control.disabled) {
        const id = control.getAttribute('data-offer');
        const entry = control.getAttribute('data-offer-entry');
        const record = recordFor(entry);
        const value = record ? offerValue(record, id) : null;
        // The menu closes on a press inside it too: the reader asked for one thing
        // and got it, and a menu still standing over the row is a menu they now have
        // to dismiss.
        menu = null;
        if (typeof value === 'string' && value !== '') takeCopy(`${entry}:${id}`, value);
        else paint();
        return;
      }
      // Anywhere else inside this face: a press dismisses. A press on a control this
      // face does not own (a disclosure) dismisses too, which is what a reader
      // expects and the reason this is not narrowed to "outside the menu".
      if (menu !== null) { menu = null; paint(); }
    };

    /**
     * A press somewhere else on the page.
     *
     * This is the document's listener and it only ever dismisses -- the host's own
     * listener is what presses a control, and a click inside the host reaches both.
     * By the time this runs on such a click the menu is already closed, so it does
     * nothing; splitting them this way is what stops one press being taken twice.
     */
    const onPressAway = () => {
      if (menu === null) return;
      menu = null;
      paint();
    };

    const onKeyDown = (event) => {
      if (event.key !== 'Escape' || menu === null) return;
      menu = null;
      paint();
    };

    const hostListens = typeof host.addEventListener === 'function';
    const documentListens = typeof doc.addEventListener === 'function';
    if (hostListens) {
      host.addEventListener('contextmenu', onContextMenu);
      host.addEventListener('click', onClick);
    }
    if (documentListens) {
      doc.addEventListener('keydown', onKeyDown);
      doc.addEventListener('click', onPressAway);
    }

    paint();

    const repaint = () => {
      if (!live) return false;
      const current = list ? list.length : -1;
      if (current === lastLength) return false;
      paint();
      return true;
    };

    const timer = (list && pollMs > 0 && typeof globalThis.setInterval === 'function')
      ? globalThis.setInterval(repaint, pollMs)
      : null;

    const unmount = () => {
      live = false;
      if (timer !== null) globalThis.clearInterval(timer);
      if (hostListens && typeof host.removeEventListener === 'function') {
        host.removeEventListener('contextmenu', onContextMenu);
        host.removeEventListener('click', onClick);
      }
      if (documentListens && typeof doc.removeEventListener === 'function') {
        doc.removeEventListener('keydown', onKeyDown);
        doc.removeEventListener('click', onPressAway);
      }
      menu = null;
      copied = null;
      clear();
    };
    // Not part of the mount contract (that is `(host, port, notices) -> unmount`);
    // a convenience for a test or a host that wants to force a check without
    // waiting on the interval, and the interval calls this same function.
    unmount.repaint = repaint;
    unmount.ready = Promise.resolve(read(list));
    return unmount;
  }

  return {
    DECLARATION, mount, read, view, toRecord,
  };
}

export const face = createFace();
export const mount = face.mount;
export { DECLARATION };
