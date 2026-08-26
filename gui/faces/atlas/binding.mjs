// SPDX-License-Identifier: Apache-2.0
// The seam between this face and the parts it draws with.
//
// Same one-direction contract every face in this tree holds (req/03 SS1): atlas
// reaches down into parts, parts never reach up, and nothing here imports the
// membrane, the shell, or another face. `row-order` and `checkable` are reused the
// same way faces/graph first reused them (grouping/ordering/checking a population,
// not a settled/held pair or a single record). `receipt-row`'s `row()`/`note()`
// are reused only for the per-touch detail this face draws once a subject's own
// history is opened -- those *are* real per-touch delta records, the same meaning
// every other face already draws them with. The always-visible subject summary
// line is this face's own bespoke shape (built directly on `element.mjs`, not
// forced into `receipt-row`'s 8-column grid, because a subject summary is not a
// delta and forcing it into that grid would misuse cells whose meaning does not
// transfer -- e.g. `receipt-row`'s `seal` column means "can this one delta be
// checked without the issuer", a question this screen never asks).
//
// `provenance-fold` and `seal-claim` are deliberately absent for the same reason
// faces/graph's binding gives: no settled/held pair exists on a screen where every
// subject is a plain read, and this screen draws no seal claim of any kind.
//
// tokenHref is deliberately absent here for the same reason every other face's
// binding gives: it touches a real disk and belongs to the Node-side fixture
// writer, not to anything a browser loads at runtime.

import { el, text, style, walk, isText, find, findByAttr, textOf, toHtml, render } from '../../parts/src/element.mjs';
import {
  glyph, sheet, installSheet, MARKS as SHEET_MARKS, symbolId, markOf, MIN_READABLE, MIN_ACT,
} from '../../parts/src/glyph-sheet.mjs';
import { badge, chip, inkFor } from '../../parts/src/verdict-badge.mjs';
import {
  row, note, receiptRow, positionedNodes, COLUMNS, GLYPH_SIZE, drawnTextFor, ROW_MESSAGES,
} from '../../parts/src/receipt-row.mjs';
import { order, repeated, ORDERS } from '../../parts/src/row-order.mjs';
import { checkable, checkableLines, failing } from '../../parts/src/checkable.mjs';
import {
  detailFrame, detailPane, installSurface, surfaceStyle, SURFACE_CSS, PANE_MESSAGES,
  statBand, box, runtimeFooter, STAT_DASH,
} from '../../parts/src/surface.mjs';
import { CONSUMED } from '../../parts/src/tokens.mjs';

/**
 * The contract, flattened. Grouped by what a caller wants rather than by which file
 * it came from, so a replacement can be written against this shape without reading
 * the package it happens to be assembled from today.
 */
export const parts = Object.freeze({
  element: Object.freeze({ el, text, style, walk, isText, find, findByAttr, textOf, toHtml, render }),
  tokens: CONSUMED,
  glyph,
  sheet,
  installSheet,
  installSurface,
  surfaceStyle,
  surfaceCss: SURFACE_CSS,
  detailFrame,
  detailPane,
  paneMessages: PANE_MESSAGES,
  symbolId,
  sheetMarks: SHEET_MARKS,
  // Owner #348 (3): the two floors a mark may be drawn at, taken from the sheet that
  // declares them rather than retyped as 16 and 20 here. A number copied out of a
  // doc comment is a number that stays behind when the doc comment moves, and the
  // floor gate in parts/test/glyph-sheet.test.mjs reads the same two constants -- so
  // a face that spells its own 16 passes the gate today and drifts from it silently.
  minReadable: MIN_READABLE,
  minAct: MIN_ACT,
  badge,
  // The three additions this face needs to state a standing as a figure rather than
  // only as a row: `markOf` resolves the engine's own word to the meaning the glyph
  // sheet holds (a table read, never a comparison against a word), `inkFor` is the
  // ink that meaning owns, and `chip` is the same filled shape badge() draws, over a
  // namespace that is not `verdict`. Nothing here decides a standing; all three are
  // lookups this face hands an already-decided word to.
  markOf,
  inkFor,
  chip,
  // The three shared containers Owner directive #340 asks every face to compose from:
  // the band of counted columns at the head of a screen, the bordered named box a
  // group of records is drawn inside, and the strip that states what this draw cost.
  statBand,
  box,
  runtimeFooter,
  statDash: STAT_DASH,
  row,
  note,
  receiptRow,
  positionedNodes,
  columns: COLUMNS,
  drawnTextFor,
  rowMessages: ROW_MESSAGES,
  glyphSize: GLYPH_SIZE,
  order,
  repeated,
  orders: ORDERS,
  checkable,
  checkableLines,
  failing,
});
