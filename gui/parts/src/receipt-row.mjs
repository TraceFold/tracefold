// SPDX-License-Identifier: Apache-2.0
// C4 -- one line of the ledger, and the note that opens under it.
//
// This is where N-1 happened. The note was an absolutely positioned element given a
// left offset and allowed to grow to two lines; absolute elements do not push their
// siblings, so the second line was drawn on top of the next row, with no background
// under it, and both texts became unreadable. The DOM measurements were fine. The
// picture was not (req/03 N-1, req/04a C4).
//
// The fix is not a larger offset. It is that nothing in this part is positioned at
// all: the note is an ordinary block that follows its row in flow, so the rows below
// it move down because that is what blocks do. `positionedNodes` holds that at zero
// and the boundary test fires it red on a planted `position:absolute`. The note also
// carries an opaque background of its own, so even a future stacking mistake would
// leave one of the two texts readable rather than both.
//
// The same change retires the other half of the defect. `--detail-x` collapses to 0
// under 1000px and the shipping window is narrower than that, so the note that was
// meant to sit beside the row sat on top of it as the normal case. A note that is
// always underneath has no width at which "beside" stops working (req/04a C1).
//
// A row is not edited once written. Undo appends a child row, which is why the spine
// column exists and why the parent is never handed to this function again.

import { el, style, walk, isText, openWhereClipped } from './element.mjs';
import {
  glyph, markOf, MIN_READABLE, MIN_ACT,
} from './glyph-sheet.mjs';
import { badge } from './verdict-badge.mjs';
import { serialOf, cutOf } from './serial.mjs';
import { CONSUMED } from './tokens.mjs';

export const ROW_MESSAGES = {
  NEEDS_ID: 'a row without an identity cannot be drawn, because it cannot be pointed at later',
  NO_CLAIM: 'no seal claim was passed for this row',
  SLOT: 'no value',
  HOLE: 'declared missing',
  DREW: 'row drawn',
  // "in the pane", not "in the row's note". Owner directive #335 removed the per-row note
  // and put a row's detail in one pane; this sentence went on promising a place that no
  // longer exists, on a product surface, and two faces repeat it verbatim in their own
  // legends. Re-confirmed as still wrong by the faces/held lane after a full round.
  AT_FORM: 'the time of day, taken from an ISO-8601 timestamp. This is a declared cut, not a clip: the date and the whole timestamp are in the cell\'s own title and in the pane for this row, so nothing about when this happened is only ever shown cut off',
};

/**
 * req/97 gap-list item 1, the root of it. This column declares 72px, and every face that
 * fed it fed a twenty-character ISO-8601 timestamp, which does not fit in 72px at
 * any font this application ships. Each face's own clip predicate then read that
 * mismatch as "this value is longer than its column can show" and opened the row's
 * note -- on every row, of four faces, at every data volume, forever, which is what
 * two rounds of retrofit were looking at when they measured two records to a screen.
 *
 * The cure is not a wider column (the row's other seven columns are already tight at
 * this app's own 720px viewport -- see GUTTER_WIDTH below, where twenty pixels of
 * gutter measurably clipped the fingerprint cell). It is that the drawn form is a
 * *declared* one, the same standing `fingerprint` already has: serialOf() cuts a
 * digest to six characters and cutOf() states the cut in words, and nobody calls
 * that column clipped. `req/100_PLACEMENT_SPEC.md` SS1 already declared this column
 * a truncation ("an ISO-8601 timestamp truncated to date+hour") and faces/atlas
 * already implements one for its own wider bespoke `at` cell (dateHourOf); the
 * shared grid is the place where the spec was never actually built. Date+hour is
 * thirteen characters and does not fit 72px either, so the form declared here is the
 * time of day -- eight characters, which does -- and the date travels with it in
 * `title` and in the note, never dropped.
 *
 * A value this cannot read is handed back whole and may then genuinely clip, which
 * is the property the negative control in parts/test/receipt-row.test.mjs pins: a
 * declared cut must not become a blanket excuse that silences a real overflow.
 */
const ISO_AT = /^\d{4}-\d{2}-\d{2}[T ](\d{2}:\d{2}(?::\d{2})?)/;

/** The time of day an ISO-8601 timestamp names, or the value untouched if it is not one. */
export function drawnAt(value) {
  const text = String(value ?? '');
  const found = ISO_AT.exec(text);
  return found ? found[1] : text;
}

/**
 * What this row will actually put on screen for a column, which is what a caller
 * measuring against a column's width budget has to measure. Every column but `at`
 * draws its value as it arrived; asking through this function rather than reading
 * the record directly is what keeps a face's budget arithmetic and this package's
 * drawing from drifting apart again.
 */
export function drawnTextFor(key, value) {
  return key === 'at' ? drawnAt(value) : String(value ?? '');
}

/**
 * Eight columns, declared once. Three of them draw a glyph and every one that does
 * states its width in pixels, which is the same number the glyph is drawn at -- a
 * glyph column that trusts a stylesheet to size its contents is the shape N-2 wore.
 *
 * `lifecycle` and `fingerprint` are the composition-A additions (req/09 SS528,
 * SS531 repair): a delta's place in its own life (held, settled, or written under an
 * earlier row) and the digest it left, so both halves of "what changed and is this
 * the same thing as before" sit in the row a reader is already scanning instead of
 * only in the note underneath it.
 */
export const COLUMNS = Object.freeze([
  { key: 'lifecycle', width: '15px', role: 'held, settled, or written under an earlier row' },
  { key: 'at', width: '72px', role: 'when, as the time of day; the date and the whole timestamp travel with it', cut: 'at' },
  { key: 'actor', width: 'minmax(0,7rem)', role: 'who' },
  { key: 'effect', width: 'minmax(0,7.5rem)', role: 'what kind of change' },
  { key: 'verdict', width: 'minmax(0,8.5rem)', role: 'what the engine answered' },
  { key: 'fingerprint', width: 'minmax(0,6rem)', role: 'the digest this delta left, cut and named as a cut' },
  { key: 'seal', width: '16px', role: 'whether this record can be checked without us' },
  { key: 'path', width: 'minmax(0,1fr)', role: 'what was touched' },
]);

/**
 * The scan line, and why it is not all eight columns.
 *
 * Owner directive #335 (3) puts a row's detail in one pane beside the list, and a
 * pane that is worth having is 320px wide. Measured rather than assumed: with the
 * pane beside it the row grid gets 580px at 1426, and the eight declared columns want
 * 637px of fixed and rem track before the flexible one gets anything -- so `path`,
 * the column that carries the answer to "what was touched", was allotted **zero
 * pixels** and its value was drawn nowhere at all. That is worse than the defect this
 * lane started on.
 *
 * So the list faces scan on five columns -- lifecycle, time, what kind of change, what
 * the engine answered, and what was touched -- and `actor`, `fingerprint` and `seal`
 * move into the pane, where
 * they are already stated in full and at greater length: the fingerprint is a cut of
 * the digest and the pane carries `digest in full`, and the seal column's mark says
 * "unsealed, because no verifier is present in this window", which the legend states
 * once and the pane repeats per row as `checkable elsewhere`. Nothing is lost from
 * the screen; two things move from a 96px and a 16px column to a place with room for
 * them. COLUMNS is unchanged and is still what faces/atlas draws with.
 */
const SCAN_WIDTHS = Object.freeze({
  // A floor, not only a ceiling, on the one column that carries the answer to this
  // screen's own question. `minmax(0,1fr)` is a share of what is left over, and when
  // the seven other tracks want more than the row has, what is left over is nothing --
  // measured at exactly that: 0px wide and 440px tall, every character of the path on
  // its own line. A track that may not go below 6rem cannot be starved by its
  // neighbours; if the row is narrower than the sum, it is the row that overflows and
  // tools/shoot.mjs's horizontalOverflow reading says so out loud, rather than one
  // column silently disappearing.
  path: 'minmax(6rem,1fr)',
  effect: 'minmax(0,5rem)',
  verdict: 'minmax(0,6.5rem)',
});

export const SCAN_COLUMNS = Object.freeze(COLUMNS
  .filter((c) => !['fingerprint', 'seal', 'actor'].includes(c.key))
  .map((c) => (SCAN_WIDTHS[c.key] ? { ...c, width: SCAN_WIDTHS[c.key] } : c)));

/**
 * The same scan line with the seal column kept, for a face whose own question is
 * about sealing. faces/held's screen exists to say that nothing on it is a receipt
 * yet -- every row's seal cell is a declared hole carrying "this has not happened
 * yet, so there is no record of it to check" -- and moving that into a pane would
 * mean the screen no longer says its own answer until a reader opens a row. It costs
 * 16px and a gap, which is what a column that carries a face's whole point is worth.
 */
export const SCAN_COLUMNS_SEALED = Object.freeze(COLUMNS
  .filter((c) => !['fingerprint', 'actor'].includes(c.key))
  .map((c) => (SCAN_WIDTHS[c.key] ? { ...c, width: SCAN_WIDTHS[c.key] } : c)));

// Owner #348 (3). Both were 14, which put a 2-unit stroke from a 24-unit design under a
// pixel; the seal was already at the floor and is now named by it rather than by a
// number that happened to match.
export const GLYPH_SIZE = MIN_READABLE;
export const SEAL_GLYPH_SIZE = MIN_READABLE;

/** A cell with nothing in it, which is not the same as a cell nobody mentioned. */
function slotCell(column) {
  return el('span', {
    'data-cell': column.key, 'data-state': 'slot', 'aria-label': ROW_MESSAGES.SLOT,
    style: style({ color: CONSUMED.attendant, 'white-space': 'nowrap', overflow: 'hidden' }),
  }, []);
}

/** A cell whose absence was declared, drawn with the reason attached. */
function holeCell(column, why) {
  return el('span', {
    'data-cell': column.key, 'data-state': 'hole', title: `${ROW_MESSAGES.HOLE}: ${why}`,
    'aria-label': `${ROW_MESSAGES.HOLE}: ${why}`,
    style: style({ display: 'flex', 'align-items': 'center', color: CONSUMED.attendant }),
  }, [glyph('structure', 'hole', { size: GLYPH_SIZE, label: ROW_MESSAGES.HOLE })]);
}

/**
 * The two columns that carry text of no declared length -- `actor` and `path` -- may
 * be asked to wrap instead of clip (`wrap: true`, which selectableRow() passes and
 * receiptRow()/openableRow() do not). A wrapping cell cannot lose data, which is the
 * whole of req/03 N-4 answered by geometry rather than by a note underneath: it is the
 * same move faces/atlas already makes on its own path cell, and its record says
 * clippedWithoutFull returns 0 because of it. The default is unchanged, so every
 * caller that wants the old fixed pitch still has it.
 */
/**
 * A path, cut into the pieces a browser is allowed to break between.
 *
 * `overflow-wrap: anywhere` was breaking paths mid-token -- `/work/repor` then `t.md` --
 * because a path contains no break opportunity any browser recognises, so "anywhere"
 * meant literally anywhere, including the middle of a filename. Measured by the
 * faces/ledger lane in its own shot.
 *
 * The separator is the natural place, so a `wbr` is placed after each one: an element
 * that says "you may break here" and puts no character into the value. A zero-width space
 * would do the same job visually and end up in whatever a reader copies, which on a face
 * whose whole point is copying an exact value is not a trade worth making.
 */
export function breakablePath(value) {
  const text = String(value);
  const parts = text.split('/');
  const out = [];
  parts.forEach((piece, index) => {
    const last = index === parts.length - 1;
    out.push(last ? piece : `${piece}/`);
    if (!last) out.push(el('wbr', {}, []));
  });
  return out.filter((piece) => piece !== '');
}

function valueCell(column, value, { mono = false, wrap = false } = {}) {
  const breakable = wrap && column.key === 'path';
  return el('span', {
    'data-cell': column.key, 'data-state': 'value',
    // The full value travels with the cell whatever the box does to it. A cut with no
    // full form anywhere on the page is req/03 N-4, and it was live on five cells at
    // 720px -- including the verdict words, which are this product's own answer.
    title: String(value),
    'data-full': String(value),
    style: style({
      color: CONSUMED.ink,
      'font-family': mono ? CONSUMED.mono : CONSUMED.sans,
      'font-size': mono ? CONSUMED.time : CONSUMED.record,
      ...(wrap
        // `break-word` rather than `anywhere`: break between words first and inside one
        // only when a single word cannot fit, which is what the wbr marks now provide
        // for the one column that had no such point.
        ? { 'white-space': 'normal', 'overflow-wrap': 'break-word' }
        : { 'white-space': 'nowrap', overflow: 'hidden', 'text-overflow': 'ellipsis' }),
    }),
  }, breakable ? breakablePath(value) : [String(value)]);
}

/** The columns whose length nothing declares, and which therefore may wrap. */
export const UNBOUNDED_COLUMNS = Object.freeze(['actor', 'path']);

/**
 * The `at` cell: the declared cut above, with the whole timestamp reachable on the
 * cell itself. `data-full` carries it too, so an instrument reading the page can ask
 * whether the full value is present without depending on a tooltip it cannot hover.
 */
function atCell(column, value) {
  const full = String(value);
  const shown = drawnAt(full);
  return el('span', {
    'data-cell': column.key, 'data-state': 'value', 'data-cut': String(shown !== full),
    'data-full': full, title: shown === full ? full : `${ROW_MESSAGES.AT_FORM}: ${full}`,
    style: style({
      color: CONSUMED.ink, 'font-family': CONSUMED.mono, 'font-size': CONSUMED.time,
      'white-space': 'nowrap', overflow: 'hidden', 'text-overflow': 'ellipsis',
    }),
  }, [shown]);
}

function cell(column, record, drawn, { wrap = false } = {}) {
  const why = record.holes?.[column.key];
  if (why) return holeCell(column, why);
  if (drawn !== undefined && drawn !== null) return el('span', { 'data-cell': column.key, 'data-state': 'value', style: style({ display: 'flex', 'align-items': 'center', overflow: 'hidden' }) }, [drawn]);
  const value = record[column.key];
  if (value === undefined || value === null || value === '') return slotCell(column);
  if (column.key === 'at') return atCell(column, value);
  return valueCell(column, value, { mono: column.key === 'path', wrap: wrap && UNBOUNDED_COLUMNS.includes(column.key) });
}

/**
 * The kind of change, as a glyph next to its own word. The glyph is looked up by
 * name -- a table read, not a branch on what the word means -- so this stays a
 * drawing part and not a deciding one (tools/boundary.mjs verdictBranches watches the
 * same shape for `verdict`; effect words get the identical treatment for the same
 * reason: an unrecognised one is still drawn, labelled with the word that arrived,
 * never dropped to silence).
 */
function effectCell(column, record, size) {
  const why = record.holes?.[column.key];
  if (why) return holeCell(column, why);
  const value = record[column.key];
  if (value === undefined || value === null || value === '') return slotCell(column);
  const mark = markOf('effect', value);
  return el('span', {
    'data-cell': column.key, 'data-state': 'value',
    style: style({
      display: 'flex', 'align-items': 'center', gap: '5px', overflow: 'hidden', color: CONSUMED.ink,
    }),
  }, [
    glyph('effect', value, { size, label: String(value) }),
    el('span', {
      style: style({
        'font-family': CONSUMED.sans, 'font-size': CONSUMED.record, 'white-space': 'nowrap', overflow: 'hidden', 'text-overflow': 'ellipsis',
      }),
    }, [String(value)]),
  ]);
}

/**
 * Held, settled, or written under an earlier row -- a delta's place in its own life,
 * read off the row rather than only off which section heading it sits under. A child
 * row keeps the mark it already had (structure/child); a held row now draws a mark
 * where it used to draw nothing, closing the gap RC-6 named (an indent with no
 * legend). A settled, non-child row draws no mark -- there is no canon glyph for
 * "ordinary" and this package does not invent furniture a reader has to memorise for
 * one state alone; its word is still in reach in the row's note.
 */
function lifecycleCell(column, record, child, size) {
  if (child) {
    return el('span', {
      'data-cell': column.key, 'data-state': 'value', title: `written under ${record.childOf}`,
      style: style({ display: 'flex', color: CONSUMED.attendant }),
    }, [glyph('structure', 'child', { size, label: `written under ${record.childOf}` })]);
  }
  if (record.lifecycle === 'held') {
    return el('span', {
      'data-cell': column.key, 'data-state': 'value', title: 'held: this has not happened yet',
      style: style({ display: 'flex', color: CONSUMED.attendant }),
    }, [glyph('standing', 'held', { size, label: 'held: this has not happened yet' })]);
  }
  return slotCell(column);
}

/**
 * The digest this delta left, cut to a short serial. Only the postcondition side --
 * `receipt_view` does not carry a precondition fingerprint (req/09 SS0-3), and this
 * cell draws what the wire actually sent rather than a placeholder in its shape. What
 * that means and why the other side of the pair is not here is written once, in the
 * legend this row lives under, not repeated in every row's title.
 */
function fingerprintCell(column, record) {
  const why = record.holes?.[column.key === 'fingerprint' ? 'digest' : column.key];
  if (why) return holeCell(column, why);
  const cut = serialOf(record.digest);
  if (!cut) return slotCell(column);
  return el('span', {
    'data-cell': column.key, 'data-state': 'value', title: cutOf(record.digest),
    style: style({
      'font-family': CONSUMED.mono, 'font-size': CONSUMED.time, color: CONSUMED.ink, 'white-space': 'nowrap', overflow: 'hidden', 'text-overflow': 'ellipsis',
    }),
  }, [cut]);
}

/**
 * The line itself. By default at one fixed height, everything clipped rather than
 * allowed to grow, because a ledger an agent appends to must not reflow while it is
 * being read; the one thing permitted to take more room is the note, and it takes it
 * underneath.
 *
 * `wrap` is the shape the detail-pane faces ask for instead (Owner directive #335, 3
 * and 4). With the detail no longer drawn under the row, there is no note for a cut
 * value to be repeated in, so the two columns whose length nothing declares are
 * allowed to take a second line and the row's height becomes a floor rather than a
 * ceiling. It costs a reflow on a resize and it buys the property that no value on
 * this screen is ever shown cut off with the whole of it nowhere -- the trade
 * faces/atlas already made on its own path cell.
 */
export function row(record, {
  claim = null, size = GLYPH_SIZE, wrap = false, columns = COLUMNS,
} = {}) {
  if (!record || typeof record.id !== 'string' || record.id === '') throw new Error(ROW_MESSAGES.NEEDS_ID);
  const child = typeof record.childOf === 'string' && record.childOf !== '';
  const sealMark = claim?.mark ?? null;
  const track = columns.map((c) => c.width).join(' ');
  return el('div', {
    'data-part': 'receipt-row',
    'data-row': record.id,
    'data-child-of': child ? record.childOf : null,
    style: style({
      display: 'grid',
      'grid-template-columns': track,
      'align-items': 'center',
      gap: '10px',
      ...(wrap ? { 'min-height': CONSUMED.pitch, padding: `4px ${CONSUMED.padX}` } : { height: CONSUMED.pitch, padding: `0 ${CONSUMED.padX}` }),
      'border-bottom': `1px solid ${CONSUMED.rule}`,
      // Transparent, not the page.
      //
      // This line is inside the button a selectable row is, and it covered the whole of
      // it -- so the hover and selected grounds the rule set draws on that button were
      // painted over by an opaque child before a reader could see either. Measured in a
      // real window: a row under a pointer read exactly the same as a row away from one.
      // The note under a row keeps its own opaque ground (see note() below), because
      // that one IS load-bearing -- it is the half of the N-1 cure that survives a
      // future stacking mistake. A row has nothing to stack over.
      background: 'transparent',
      color: CONSUMED.ink,
      'font-size': CONSUMED.record,
      'line-height': CONSUMED.recordLine,
      ...(wrap ? {} : { overflow: 'hidden' }),
    }),
  }, columns.map((column) => {
    if (column.key === 'lifecycle') return lifecycleCell(column, record, child, size);
    if (column.key === 'effect') return effectCell(column, record, size);
    if (column.key === 'fingerprint') return fingerprintCell(column, record);
    if (column.key === 'verdict') return cell(column, record, badge(record.verdict, { size }), { wrap });
    if (column.key === 'seal') {
      if (!sealMark) return holeCell(column, ROW_MESSAGES.NO_CLAIM);
      return cell(column, record, glyph(sealMark[0], sealMark[1], { size: SEAL_GLYPH_SIZE, label: claim.standing }), { wrap });
    }
    return cell(column, record, undefined, { wrap });
  }));
}

/**
 * The note. In flow, opaque, and allowed to wrap -- the three properties whose
 * absence produced N-1. It is a sibling that follows, never an overlay.
 */
export function note(lines, { summary = null } = {}) {
  const entries = Array.isArray(lines) ? lines : [];
  return el('div', {
    'data-part': 'receipt-note',
    'data-count': String(entries.length),
    style: style({
      background: CONSUMED.page,
      color: CONSUMED.ink,
      'border-bottom': `1px solid ${CONSUMED.rule}`,
      padding: `8px ${CONSUMED.padX}`,
      'font-family': CONSUMED.sans,
      // SS558 body-text floor: was CONSUMED.meta (12px), now CONSUMED.record (14px).
      'font-size': CONSUMED.record,
      'line-height': CONSUMED.recordLine,
      'white-space': 'normal',
      'overflow-wrap': 'anywhere',
    }),
  }, [
    // SS558/D-1 (req/38 SS576): explicit size hierarchy, not one flat size under
    // the section heading. This is the one fact a reader opened the row to see
    // (the verdict in full, or why a held row has no claim yet), so it leads at
    // CONSUMED.head/full ink rather than reading at the same weight as the
    // key/value lines under it -- and it is free-flowing (no fixed-pitch column
    // to clip), so the larger size carries no width-budget risk the way a row
    // cell's own font would.
    summary ? el('p', {
      'data-role': 'note-summary',
      style: style({
        margin: '0 0 6px', color: CONSUMED.ink, 'font-size': CONSUMED.head, 'line-height': CONSUMED.headLine, 'font-weight': '600',
      }),
    }, [summary]) : null,
    ...entries.map((entry) => el('div', {
      'data-role': 'note-line',
      style: style({ display: 'grid', 'grid-template-columns': 'minmax(0,9rem) minmax(0,1fr)', gap: '10px', padding: '2px 0' }),
    }, [
      el('span', { style: style({ color: CONSUMED.attendant }) }, [String(entry.name)]),
      el('span', { style: style({ color: CONSUMED.ink }) }, [String(entry.value)]),
    ])),
  ]);
}

/** A row, and its note when it is open. Two blocks, one after the other. */
export function receiptRow(record, { claim = null, note: lines = null, open = false, size = GLYPH_SIZE } = {}) {
  return el('div', {
    'data-part': 'receipt-row-group',
    'data-open': String(Boolean(open && lines)),
    style: style({ display: 'block' }),
  }, [
    row(record, { claim, size }),
    open && lines ? note(lines, { summary: record.noteSummary ?? null }) : null,
  ]);
}

/**
 * req/768 F-I (retrofit round 2, AC-7): the reversibility chip, drawn from a fact
 * `parts/src/reversibility.mjs`'s reversalOf() already decided -- this function
 * places what it is handed and decides nothing (the same division row()'s seal
 * cell already holds for `claim`). Self-evident per SS553/req/768 R-1: a glyph
 * next to its own short word, never a bare glyph, with the full honest reason in
 * `title` (the same reach-for-more-text convention every hole cell in this
 * package already uses).
 */
const REVERSAL_LABEL = Object.freeze({ reversed: 'reversed', 'not-observable': 'unknown', 'not-committed': 'n/a' });

function reversalChip(fact, { size = MIN_READABLE } = {}) {
  const state = fact?.state ?? 'not-observable';
  const label = REVERSAL_LABEL[state] ?? REVERSAL_LABEL['not-observable'];
  const mark = Array.isArray(fact?.mark) ? fact.mark : ['standing', 'none'];
  return el('span', {
    'data-part': 'reversal-chip', 'data-state': state, title: fact?.why ?? '',
    style: style({
      display: 'inline-flex', 'align-items': 'center', gap: '5px', flex: 'none',
      color: CONSUMED.ink, 'font-family': CONSUMED.sans, 'font-size': CONSUMED.time, 'line-height': CONSUMED.recordLine, 'white-space': 'nowrap',
    }),
  }, [glyph(mark[0], mark[1], { size, label }), el('span', {}, [label])]);
}

/**
 * req/768 F-C (retrofit round 2, AC-4): a row's own acts, as a permanent,
 * fixed-width right gutter attached to the row -- not a full-width strip drawn
 * underneath it, which is what this package drew before this export existed. An
 * act this row does not send is still drawn here, disabled, in place, with its
 * reason in `title`: a withheld act rendered as blank space is indistinguishable
 * from an act that was never offered, which is the one shape req/768 AC-4 refuses.
 * No act is invented here -- `acts` is always exactly the spec list a face's own
 * declaration.mjs already offers on this row; this function only lays it out.
 */
// 84px, not the rounder 104px this constant first shipped at: at this app's own
// narrow viewport (720px, tools/fixture.mjs NARROW), the wider gutter took
// enough width from the row's own fixed 8-column grid (parts/src/receipt-row.mjs
// COLUMNS, untouched by this round -- shared with faces/atlas's receiptRow(),
// which this round does not modify) that the fingerprint column's already-cut
// 6-character digest abbreviation clipped a second time, with its exact clipped
// text no longer reachable elsewhere on the page (tools/shoot.mjs's own N-4
// clippedWithoutFull reading, measured red before this value was chosen). 84px
// still comfortably fits every offered act's glyph+label (the longest, "escalate",
// measured against this padding/font), and clippedWithoutFull returns to 0 at
// 720px with it -- a real, measured budget, not a guess.
// 92px, and the six pixels over the old 84 are exactly the six the mark grew by.
//
// The number above was measured against a 14px act mark. Owner #348 (3) raised act marks
// to MIN_ACT (20) and this constant did not move with them, so every act button
// overflowed its own gutter -- measured by the faces/held lane at 3px on `commit` and 5px
// on `escalate`, on every row of every row-bearing state at both viewports, and visible in
// its shot. A constant derived from a size is a constant that has to be re-derived when
// the size changes; this one was re-derived rather than nudged until it looked right.
//
// The old note's warning still stands and is why this is 92 and not 104: at this app's
// own 720px viewport a wider gutter takes enough from the row's own grid to clip the
// fingerprint cell, which was measured when that number was chosen.
export const GUTTER_WIDTH = '92px';
// An act's own mark carries more of the meaning than a mark beside a word does, and it
// sits on the one thing a hand is aimed at, so it takes the higher of the two floors.
const ACT_GLYPH_SIZE = MIN_ACT;

function actGutter(acts, record, { size = ACT_GLYPH_SIZE } = {}) {
  const offered = Array.isArray(acts) ? acts : [];
  if (offered.length === 0) return null;
  return el('div', {
    'data-part': 'act-gutter', 'data-target': record?.id ?? null, 'data-count': String(offered.length),
    style: style({
      display: 'flex', 'flex-direction': 'column', gap: '4px', flex: `0 0 ${GUTTER_WIDTH}`, width: GUTTER_WIDTH,
      'box-sizing': 'border-box', 'border-left': `1px solid ${CONSUMED.rule}`, 'margin-left': '8px', 'padding-left': '6px',
    }),
  }, offered.map((spec) => el('button', {
    type: 'button',
    'data-act': spec.act, 'data-target': record?.id ?? null, 'data-sends': String(spec.sends),
    disabled: spec.sends ? null : true,
    title: spec.sends ? spec.label : spec.why,
    // Colour, border and cursor are NOT written here, and their absence is the fix
    // rather than an omission. An inline declaration outranks a stylesheet, so while
    // this element spelled its own `color`, `background`, `border` and
    // `cursor:default`, every operability rule in parts/src/surface.mjs aimed at it was
    // dead on arrival -- measured in a real window by the faces/ledger lane: a live act
    // button drew the plain ink and a default cursor while the rule set said accent and
    // pointer. Six of seven declarations were inert. What stays inline is geometry, which
    // nothing else claims; what a reader can press, and what colour that is, belongs to
    // the one rule set that owns it.
    style: style({
      font: 'inherit', display: 'inline-flex', 'align-items': 'center', gap: '5px', width: '100%',
      'font-family': CONSUMED.sans, 'font-size': CONSUMED.record, 'line-height': CONSUMED.recordLine,
      padding: '8px 6px', 'min-height': '36px', 'box-sizing': 'border-box',
    }),
  }, [glyph('act', spec.act, { size, label: spec.label }), el('span', {}, [spec.label])])));
}

/** A row (or a row's own disclosure), and a fixed-width gutter beside it, never
 * under it. `align-items:flex-start` so the gutter's own height is only as tall
 * as its own buttons -- an open note underneath the row line does not stretch a
 * one-button gutter down to match it. A null gutter draws nothing extra: this
 * function is a no-op for every caller that never offers a row-level act. */
export function rowWithGutter(rowNode, gutterNode) {
  if (!gutterNode) return rowNode;
  return el('div', {
    'data-part': 'row-gutter-frame',
    style: style({ display: 'flex', 'align-items': 'flex-start' }),
  }, [
    el('div', { style: style({ flex: '1', 'min-width': '0' }) }, [rowNode]),
    gutterNode,
  ]);
}

/**
 * A row, wrapped in a native disclosure control around its own note -- the SS657
 * retrofit (req/768 F-A, "collapsed-by-default disclosure, counted"). receiptRow()
 * above stays exactly as it was and is still what faces/atlas uses for its own
 * per-touch rows (nested inside a subject's own bespoke <details>, where the outer
 * fold already carries the disclosure); this is a second, additive export for a
 * caller that wants each row itself to be independently open/closable by a reader,
 * not only by whatever heuristic computed its initial `open` state.
 *
 * Two properties this shape adds over receiptRow(): (1) the wrapper is a real
 * <details>/<summary> pair, so a reader can open or close any row by hand
 * regardless of which way the initial-state heuristic (a genuine reason: a
 * declared hole, or a value long enough to clip) decided it -- "open one on row
 * activation"; (2) the control states, on its own face, how many fields its note
 * withholds -- never a silent chevron with no count on it (req/768 F-A's own
 * phrase). The row's own one-line summary (the dense fact line: at/actor/effect/
 * verdict/.../path) is always visible as the <summary>'s content, exactly as it
 * was before this wrapper existed; only the note is behind the fold.
 *
 * Retrofit round 2 (req/768 AC-4/AC-6/AC-7) adds two more optional facts, both
 * additive and both `null` by default -- a caller that passes neither (every call
 * site that existed before this round: faces/receipt, faces/graph) gets back
 * exactly the tree this function already produced, unchanged. `reversal` is a
 * fact object from parts/src/reversibility.mjs's reversalOf() (never computed
 * here -- this function draws, it does not decide); `acts` is a face's own
 * declaration.mjs ACTS list already filtered to what this one row offers.
 */
export function openableRow(record, {
  claim = null, note: lines = null, open = false, size = GLYPH_SIZE, reversal = null, acts = null,
} = {}) {
  const entries = Array.isArray(lines) ? lines : [];
  const chip = reversal ? reversalChip(reversal, { size: MIN_READABLE }) : null;
  const gutter = actGutter(acts, record);
  let inner;
  if (entries.length === 0) {
    // Nothing to disclose. A details wrapper with an empty body would be a fold
    // that opens onto nothing -- the same silent-chevron shape this export exists
    // to refuse, just pointed at itself. Drawn as a plain row instead.
    inner = el('div', {
      'data-part': 'receipt-row-group', 'data-open': 'false', 'data-withholds': '0',
      style: style({ display: 'flex', 'align-items': 'center', gap: '8px' }),
    }, [
      el('div', { style: style({ flex: '1', 'min-width': '0' }) }, [row(record, { claim, size })]),
      chip,
    ].filter(Boolean));
  } else {
    inner = el('details', {
      'data-part': 'receipt-row-disclosure',
      'data-withholds': String(entries.length),
      'data-open': String(Boolean(open)),
      open: open || null,
      style: style({ display: 'block' }),
    }, [
      el('summary', {
        // No cursor here either -- the rule set draws the pointer over a summary and an
        // inline `cursor:default` was outranking it.
        style: style({
          display: 'flex', 'align-items': 'center', gap: '8px', 'list-style': 'none',
        }),
      }, [
        el('div', { style: style({ flex: '1', 'min-width': '0' }) }, [row(record, { claim, size })]),
        chip,
        el('span', {
          'data-role': 'withheld-count',
          style: style({
            'font-family': CONSUMED.sans, 'font-size': CONSUMED.time, color: CONSUMED.attendant, 'white-space': 'nowrap', padding: '0 4px',
          }),
        }, [`${entries.length} more field${entries.length === 1 ? '' : 's'}`]),
      ].filter(Boolean)),
      note(entries, { summary: record.noteSummary ?? null }),
    ]);
  }
  return rowWithGutter(inner, gutter);
}

/**
 * The reading a build-time budget cannot make, named here and made in the door.
 *
 * Every face on this tree decides a row's opening state from a character budget
 * derived from the width a column *declares*. That budget is a guess and this
 * package has always said so in as many words (faces/ledger's CLIP_RISK: "the real
 * width is only known in front of a renderer"). It is wrong in one direction that
 * matters: the four flexible columns shrink well below their declared widths at
 * this application's own narrow viewport, so a value the budget passes can still be
 * cut on screen -- and once the notes are shut by default (req/97 gap-list item 1),
 * a cut value's full form sits behind a fold rather than on the page, which is
 * req/03 N-4 wearing a different coat.
 *
 * So the shut state is the default and a measurement is what re-opens a row: after
 * a paint, any row whose own summary cell is actually overflowing its own box opens
 * itself and says why (`data-open-because="measured-clip"`), and every other row
 * stays shut. That is "genuinely exceptional truncation" decided by the renderer
 * rather than by arithmetic about it -- on this app's own fixtures it opens nothing
 * at 1280px and 1426px, and at 720px it opens exactly the rows whose paths the grid
 * cannot hold, where tools/shoot.mjs's clippedWithoutFull reading returns to 0.
 *
 * What this package owns is the two selectors; the reach into a document is
 * element.mjs's openWhereClipped(), because a document is reached from two modules
 * in this package and nowhere else (parts/test/boundary.test.mjs holds that).
 */
export const MEASURED_CLIP = Object.freeze({
  fold: 'details[data-part="receipt-row-disclosure"]',
  cell: '[data-cell]',
  because: 'measured-clip',
});

/** The same reading, with this package's own selectors already filled in. */
export function openMeasuredClips(root) {
  return openWhereClipped(root, MEASURED_CLIP);
}

const POSITIONED = /position\s*:\s*(absolute|fixed|sticky)/i;

/** Every node in a tree that takes itself out of flow. For rows the answer is none. */
export function positionedNodes(node) {
  const hits = [];
  walk(node, (n) => {
    if (isText(n)) return;
    if (POSITIONED.test(n.attrs.style ?? '')) hits.push({ tag: n.tag, style: n.attrs.style });
  });
  return hits;
}

/**
 * Owner directive #335 (3): a row whose detail is stored, not inlined.
 *
 * openableRow() above put a row's own facts in a note directly underneath it. That is
 * the shape req/97 measured at two records to a screen, and it is the shape Studio
 * does not have: Studio's detail surface is a separate pane holding exactly one
 * object, opened from the row. This is that -- the row is a control that names itself
 * as the subject of the one pane on the screen (parts/src/surface.mjs detailPane), and
 * choosing a different row changes what the pane says and nothing about the list's
 * geometry.
 *
 * Three properties it holds that a <details> per row cannot:
 *   1. The list never reflows. Every row is the same height whatever is chosen, so a
 *      thousand rows are a thousand lines and the count of records a screen holds is a
 *      property of the row, not of what a reader has opened.
 *   2. Exactly one object is ever described, so the pane can be read as "this row"
 *      without a heading on every row saying which row it belongs to -- which is where
 *      the repeated sentence req/96 R-4 refuses came from in the first place.
 *   3. The row carries the count of what the pane will add (`n fields`), so the
 *      control is never a silent affordance (req/768 F-A's own phrase, kept).
 *
 * It is a <button>, not a div with a handler: a control a keyboard cannot reach is a
 * control half the people looking at this screen do not have. `aria-pressed` states
 * which one is the pane's subject, and the chosen row is drawn with a left marker so
 * that is legible without a pointer as well.
 */
export function selectableRow(record, {
  claim = null, size = GLYPH_SIZE, reversal = null, acts = null, fields = 0, selected = false,
  columns = SCAN_COLUMNS,
} = {}) {
  const chip = reversal ? reversalChip(reversal, { size: MIN_READABLE }) : null;
  const gutter = actGutter(acts, record);
  const inner = el('button', {
    type: 'button',
    'data-part': 'selectable-row',
    'data-select-row': record.id,
    'data-selected': String(Boolean(selected)),
    'data-fields': String(fields),
    'aria-pressed': String(Boolean(selected)),
    class: 'gx-select gx-row-line',
    // Same reason as the act gutter above: no cursor, no background, no border colour
    // inline. The selected row's own marker is drawn from `data-selected` by the rule
    // set, which is also where its accent lives -- it used to be a 3px bar in the plain
    // ink, which on a dark page is a dark line on a dark ground and on a light page is
    // a dark line the eye reads as a rule rather than as a selection.
    style: style({
      font: 'inherit', display: 'flex', 'align-items': 'center', gap: '8px', width: '100%',
      'box-sizing': 'border-box', 'text-align': 'left', color: CONSUMED.ink,
      border: 'none', 'border-left-style': 'solid', 'border-left-width': '3px',
      padding: '0',
    }),
  }, [
    el('div', { style: style({ flex: '1', 'min-width': '0' }) }, [row(record, { claim, size, wrap: true, columns })]),
    chip,
    fields > 0 ? el('span', {
      'data-role': 'field-count',
      style: style({
        'font-family': CONSUMED.sans, 'font-size': CONSUMED.time, color: CONSUMED.attendant,
        'white-space': 'nowrap', padding: `0 ${CONSUMED.padX} 0 4px`,
      }),
    }, [`${fields} fields`]) : null,
  ].filter(Boolean));
  return rowWithGutter(inner, gutter);
}
