// SPDX-License-Identifier: Apache-2.0
// The seam between this face and the parts it draws with.
//
// The face never imports a part directly. It is built with this object and could be
// built with another, which is the same replaceability the membrane offers below it
// -- a stub handed in here is a different implementation of the same contract, not a
// mock of one part's behaviour, and the face cannot tell which it was given.
//
// One direction only: the face reaches down into parts, parts never reach up.
// Nothing here imports the membrane, the shell, or another face, and the gate holds
// that at zero.
//
// This face draws no receipt row, badge, fold or seal, because it draws nothing that
// claims to be checkable -- there is no claim on this screen for a verifier to agree
// or disagree with, only a record of a call and how it came back. `parts/src/order`
// is reused for exactly the property it already offers for free: a record with no
// usable identity is dropped with a named reason, and that reason is reported rather
// than swallowed.

import { el, text, style, walk, isText, find, findByAttr, textOf, toHtml, render } from '../../parts/src/element.mjs';
import {
  glyph, sheet, installSheet, MARKS as SHEET_MARKS, symbolId, MIN_READABLE, MIN_ACT,
} from '../../parts/src/glyph-sheet.mjs';
import { order, ORDERS } from '../../parts/src/row-order.mjs';
// The declared cut for a time cell, from the one module that decides it -- this face
// has its own bespoke row grid but the same 72px time column and the same problem.
import { drawnAt } from '../../parts/src/receipt-row.mjs';
import {
  detailFrame, detailPane, installSurface, surfaceStyle, SURFACE_CSS, PANE_MESSAGES,
  statBand, box, runtimeFooter, STAT_DASH,
} from '../../parts/src/surface.mjs';
// The filled standing chip. This face draws no verdict (see notice.mjs's own note on
// why a refused call is not a denied candidate), so what it reaches this for is the
// shape rather than the hue: a bordered pill on a group's head, over a mark this face
// already declares, drawn by the one function that decides how a standing looks.
import { chip } from '../../parts/src/verdict-badge.mjs';
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
  /**
   * The two floors a mark may be drawn at, carried through the seam rather than
   * spelled as numbers in the face.
   *
   * Owner #348 (3). A 24-unit design with a 2-unit stroke drawn into 14 or 15 pixels
   * puts that stroke under a pixel, and this face was drawing seven of its eight marks
   * at 15 -- a number nobody chose for legibility, taken from the 15px row-line type
   * scale so that a mark would match the text beside it. Matching the type is the wrong
   * question: a letterform at 15px is a shape a reader already knows and a 24-unit mark
   * at 15px is a shape nobody can resolve. `readable` is the floor for a mark sitting in
   * a line beside its own word; `act` is the floor for a mark on something a hand aims
   * at, which on this face is a disclosure rather than an act on a record (this face
   * declares none). Passing these through here rather than importing glyph-sheet's
   * constants into notice.mjs keeps the one-direction rule this file exists for: the
   * face is built from an object, and a replacement can hand it different floors.
   */
  floors: Object.freeze({ readable: MIN_READABLE, act: MIN_ACT }),
  drawnAt,
  sheet,
  installSheet,
  installSurface,
  surfaceStyle,
  surfaceCss: SURFACE_CSS,
  detailFrame,
  detailPane,
  paneMessages: PANE_MESSAGES,
  statBand,
  box,
  chip,
  runtimeFooter,
  statDash: STAT_DASH,
  symbolId,
  sheetMarks: SHEET_MARKS,
  order,
  orders: ORDERS,
});
