// SPDX-License-Identifier: Apache-2.0
// The seam between this face and the parts it draws with.
//
// Same one-direction contract every face in this tree holds (req/03 §1): receipt
// reaches down into parts, parts never reach up, and nothing here imports the
// membrane, the shell, or another face. Two shared parts faces/ledger and
// faces/held both draw with are deliberately absent from this seam:
// `provenance-fold` (no settled/held pair exists on a screen that draws one already-
// settled delta) and `row-order` (no list exists on a screen that draws one record
// by id). Everything else is reused because it is the same eight-column row
// grammar, the same glyph sheet, and the same seal-claim/serial/checkable
// discipline every other face already carries, not a second implementation of any
// of them.
//
// tokenHref is deliberately absent here for the same reason faces/ledger's and
// faces/held's bindings give: it touches a real disk and belongs to the Node-side
// fixture writer, not to anything a browser loads at runtime.

import { el, text, style, walk, isText, find, findByAttr, textOf, toHtml, render } from '../../parts/src/element.mjs';
import {
  glyph, sheet, installSheet, MARKS as SHEET_MARKS, symbolId, markOf, MIN_READABLE, MIN_ACT,
} from '../../parts/src/glyph-sheet.mjs';
import { badge, chip, inkFor } from '../../parts/src/verdict-badge.mjs';
import {
  row, note, receiptRow, openableRow, selectableRow, positionedNodes, COLUMNS, GLYPH_SIZE, drawnTextFor, ROW_MESSAGES, openMeasuredClips, MEASURED_CLIP,
} from '../../parts/src/receipt-row.mjs';
import { serialOf, cutOf, notAProof, serial } from '../../parts/src/serial.mjs';
import { claimOf, portability, SEAL_MESSAGES, PORTABLE_FIELDS } from '../../parts/src/seal-claim.mjs';
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
  // Owner directive #340's four containers, reached the same way everything else in
  // this seam is: the band that states this screen's size, the box every group on it
  // is drawn as, the standing chip a box head wears, the ink a standing owns, and the
  // strip that states what the draw cost. This face spells no colour of its own --
  // `inkFor(markOf(...))` is the only way a hue reaches the band, and it comes from
  // the same table the badge and the chip read.
  statBand,
  box,
  runtimeFooter,
  statDash: STAT_DASH,
  chip,
  inkFor,
  markOf,
  symbolId,
  sheetMarks: SHEET_MARKS,
  // Owner #348 (3). The two floors reach this face the same way every other shared
  // fact does -- through the seam -- rather than as the numbers 16 and 20 typed into
  // a call site, where raising the floor once would leave this face behind at the old
  // value and nothing would say so. `minAct` is bound here and drawn nowhere: this
  // face puts no mark on an act control (see receipt.mjs's own note on the menu), and
  // binding it is how the seam states that the floor was read and found not to apply
  // rather than overlooked.
  minReadable: MIN_READABLE,
  minAct: MIN_ACT,
  badge,
  row,
  note,
  receiptRow,
  openableRow,
  selectableRow,
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
  portableFields: PORTABLE_FIELDS,
  sealMessages: SEAL_MESSAGES,
  checkable,
  failing,
  reversalOf,
});
