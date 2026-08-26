// SPDX-License-Identifier: Apache-2.0
// The seam between this face and the parts it draws with.
//
// The face never imports a part directly. It is built with this object and can be
// built with another, which is what makes the parts replaceable without the face
// noticing -- the same property the membrane has with respect to its transport. A
// stub passed here in a test is not a mock of a part's behaviour; it is a different
// implementation of the same contract, and the face cannot tell.
//
// One direction only: the face reaches down into parts, parts never reach up. Nothing
// here imports the membrane, the shell, or another face, and the gate holds that at
// zero.
//
// tokenHref is deliberately not in this object. It resolves a real path against a
// real disk (parts/tools/token-source.mjs, node:path) and nothing the face draws at
// runtime calls it -- only the Node-side fixture writers do, to build the href a
// static page's <link> points at. Everything named here is what a browser must be
// able to run with zero node:* imports in its whole import graph (req/02 W15); a
// build-time-only helper does not belong in the runtime seam just because it once
// lived beside CONSUMED in the same file.

import { el, text, style, walk, isText, find, findByAttr, textOf, toHtml, render } from '../../parts/src/element.mjs';
import {
  glyph, sheet, installSheet, MARKS as SHEET_MARKS, symbolId, markOf, MIN_READABLE, MIN_ACT,
} from '../../parts/src/glyph-sheet.mjs';
import { badge, chip, inkFor } from '../../parts/src/verdict-badge.mjs';
import {
  row, note, receiptRow, openableRow, selectableRow, SCAN_COLUMNS_SEALED, positionedNodes, COLUMNS, GLYPH_SIZE, drawnTextFor, ROW_MESSAGES, openMeasuredClips, MEASURED_CLIP,
} from '../../parts/src/receipt-row.mjs';
import { fold, HALVES, withoutFolds } from '../../parts/src/provenance-fold.mjs';
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
 * The contract, flattened. Grouped by what a caller wants rather than by which file it
 * came from, so a replacement can be written against this shape without reading the
 * package it happens to be assembled from today.
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
  // The head, the group and the strip at the foot. All three decide nothing: they are
  // handed counts this face worked out and inks this face asked for by mark, which is
  // why a face may hold them without holding a colour.
  statBand,
  box,
  runtimeFooter,
  statDash: STAT_DASH,
  symbolId,
  sheetMarks: SHEET_MARKS,
  // The two floors a mark may be drawn at, reached by name (Owner #348 (3)). A face
  // that types 14 has picked a size by eye; a face that asks for the floor gets the
  // number the sheet decided and moves when the sheet moves. There are two because
  // there are two situations -- a mark beside a word, and a mark on the thing a hand
  // is aimed at -- and a third would be one nobody could name.
  minReadable: MIN_READABLE,
  minAct: MIN_ACT,
  badge,
  chip,
  // A mark, and the ink that mark owns. Reached as a pair, never separately: an ink
  // this face chose by any other route would be this face spelling a colour.
  markOf,
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
  fold,
  halves: HALVES,
  withoutFolds,
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
