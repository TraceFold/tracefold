// SPDX-License-Identifier: Apache-2.0
// The seam between this face and the parts it draws with.
//
// Same one-direction contract every face in this tree holds (req/03 §1): held
// reaches down into parts, parts never reach up, and nothing here imports the
// membrane, the shell, or another face. This face draws no provenance-fold
// settled/held pair of its own -- there is no settled half on this screen to
// contrast against, that contrast is faces/ledger's job -- so `fold`/`HALVES`/
// `withoutFolds` are not part of this seam; everything else faces/ledger's binding
// exposes is reused because it is the same eight-column row grammar, the same
// glyph sheet, and the same seal-claim discipline, not a second implementation of
// any of them.
//
// tokenHref is deliberately absent here for the same reason faces/ledger's binding
// gives: it touches a real disk and belongs to the Node-side fixture writer, not to
// anything a browser loads at runtime.

import { el, text, style, walk, isText, find, findByAttr, textOf, toHtml, render } from '../../parts/src/element.mjs';
import {
  glyph, sheet, installSheet, MARKS as SHEET_MARKS, symbolId, markOf, MIN_READABLE, MIN_ACT,
} from '../../parts/src/glyph-sheet.mjs';
import { badge, chip, inkFor } from '../../parts/src/verdict-badge.mjs';
import {
  row, note, receiptRow, openableRow, selectableRow, SCAN_COLUMNS_SEALED, positionedNodes, COLUMNS, GLYPH_SIZE, drawnTextFor, ROW_MESSAGES, openMeasuredClips, MEASURED_CLIP,
} from '../../parts/src/receipt-row.mjs';
import { serialOf, cutOf, notAProof, serial } from '../../parts/src/serial.mjs';
import { claimOf, portability, SEAL_MESSAGES } from '../../parts/src/seal-claim.mjs';
import { order, ORDERS } from '../../parts/src/row-order.mjs';
import { checkable, failing } from '../../parts/src/checkable.mjs';
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
  // The four Owner #340 containers this face composes its screen out of. Placed
  // here rather than imported directly by held.mjs for the same reason everything
  // else in this seam is: the face names what it needs, and the package it happens
  // to come from today is this file's business and not the face's.
  statBand,
  box,
  runtimeFooter,
  statDash: STAT_DASH,
  symbolId,
  sheetMarks: SHEET_MARKS,
  markOf,
  // The two floors a mark may be drawn at, named rather than typed. A face that
  // spells 16 and 20 is a face that keeps its own numbers when the sheet changes
  // its mind about what a readable mark is, and the sheet is where that is decided
  // -- it measured every mark's real geometry to arrive at them.
  minReadable: MIN_READABLE,
  minAct: MIN_ACT,
  badge,
  chip,
  inkFor,
  row,
  note,
  receiptRow,
  openableRow,
  selectableRow,
  scanColumnsSealed: SCAN_COLUMNS_SEALED,
  positionedNodes,
  columns: COLUMNS,
  drawnTextFor,
  openMeasuredClips,
  measuredClip: MEASURED_CLIP,
  rowMessages: ROW_MESSAGES,
  glyphSize: GLYPH_SIZE,
  serialOf,
  cutOf,
  notAProof,
  serial,
  claimOf,
  portability,
  sealMessages: SEAL_MESSAGES,
  order,
  orders: ORDERS,
  checkable,
  failing,
  reversalOf,
});
