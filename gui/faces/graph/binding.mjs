// SPDX-License-Identifier: Apache-2.0
// The seam between this face and the parts it draws with.
//
// Same one-direction contract every face in this tree holds (req/03 §1): graph
// reaches down into parts, parts never reach up, and nothing here imports the
// membrane, the shell, or another face. `row-order` and `checkable` are both
// reused here for the first time by a face that draws neither a settled/held pair
// nor a single record -- graph groups and orders a population, and both parts
// already exist to do exactly that over a plain array of records, so no third
// implementation of "order a list" or "state which structural claims hold" is
// written for this face. `provenance-fold` and `seal-claim` are deliberately
// absent: there is no settled/held pair on a screen where every subject is, by
// definition, already settled (see declaration.mjs UNDRAWN), and this screen
// draws no seal claim of any kind.
//
// tokenHref is deliberately absent here for the same reason every other face's
// binding gives: it touches a real disk and belongs to the Node-side fixture
// writer, not to anything a browser loads at runtime.

import { el, text, style, walk, isText, find, findByAttr, textOf, toHtml, render } from '../../parts/src/element.mjs';
import {
  glyph, sheet, installSheet, MARKS as SHEET_MARKS, symbolId, markOf, MIN_READABLE, MIN_ACT,
} from '../../parts/src/glyph-sheet.mjs';
import { badge, inkFor } from '../../parts/src/verdict-badge.mjs';
import {
  row, note, receiptRow, openableRow, selectableRow, positionedNodes, COLUMNS, SCAN_COLUMNS, GLYPH_SIZE, drawnTextFor, ROW_MESSAGES, openMeasuredClips, MEASURED_CLIP,
} from '../../parts/src/receipt-row.mjs';
import { order, repeated, ORDERS } from '../../parts/src/row-order.mjs';
import { checkable, checkableLines, failing } from '../../parts/src/checkable.mjs';
import { reversalOf } from '../../parts/src/reversibility.mjs';
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
  // Owner #340's three containers, reached the same way everything else here is.
  // `markOf`/`inkFor` are the only route this face has to a hue: a face may not
  // spell a colour, so a figure that carries a standing's ink asks the standing
  // table for it (`inkFor(markOf(namespace, key))`) and places what it is given.
  statBand,
  box,
  runtimeFooter,
  statDash: STAT_DASH,
  markOf,
  inkFor,
  symbolId,
  sheetMarks: SHEET_MARKS,
  badge,
  row,
  note,
  receiptRow,
  openableRow,
  selectableRow,
  positionedNodes,
  columns: COLUMNS,
  // The five columns a face with a detail pane beside its list scans on. This face
  // draws a subset of them (see graph.mjs GROUP_COLUMNS) and derives that subset from
  // this list rather than writing a second column table of its own.
  scanColumns: SCAN_COLUMNS,
  // Owner #348 (3). The two floors, reached by name, so this face never spells the
  // number a mark is drawn at and cannot drift from the sheet that declares it.
  minReadable: MIN_READABLE,
  minAct: MIN_ACT,
  drawnTextFor,
  openMeasuredClips,
  measuredClip: MEASURED_CLIP,
  rowMessages: ROW_MESSAGES,
  glyphSize: GLYPH_SIZE,
  order,
  repeated,
  orders: ORDERS,
  checkable,
  checkableLines,
  failing,
  reversalOf,
});
