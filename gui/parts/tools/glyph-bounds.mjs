// SPDX-License-Identifier: Apache-2.0
// Does every mark fit inside the box it is drawn in?
//
// Owner #348 (3): the Owner is seeing cut-off icons. A mark is a set of paths inside a
// 24-unit viewBox, and a path whose geometry reaches the edge of that box is drawn
// clipped -- not because the SVG is wrong, but because a stroke has WIDTH. A path
// centred exactly on x=0 is drawn with half its stroke outside the box, and the half
// outside is not drawn at all. Round caps and joins push it further still.
//
// This does not approximate that with path arithmetic. Parsing SVG path data by hand is
// how you get a checker that is confidently wrong about an arc, and three of these marks
// are arcs. It asks the renderer instead: `SVGGeometryElement.getBBox()` returns the
// exact geometric bounds of the drawn path, arcs included, computed by the same engine
// that will draw it on the screen. What is measured is what ships.
//
// `getBBox()` returns the geometry WITHOUT the stroke, which is the right primitive: the
// stroke's own contribution is a known function of stroke-width and the cap/join style,
// stated once below rather than measured per shape.
//
//   node parts/tools/glyph-bounds.mjs [--json <path>]
//
// Exit 0 = every mark fits. Exit 1 = at least one is clipped, and each is named with the
// edge it crosses and by how much.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { startRenderer } from '../../tools/rig/renderer.mjs';
import { BOX, STROKE, sheetMarks } from '../src/glyph-sheet.mjs';


const HERE = path.dirname(fileURLToPath(import.meta.url));

export const BOUNDS_MESSAGES = {
  CLIPPED: 'a mark reaches past the box it is drawn in, so part of its stroke is not drawn',
  FITS: 'every mark fits inside its own box, stroke included',
  EMPTY: 'no marks were measured, which is not the same as every mark fitting',
};

/**
 * How far a stroke reaches beyond the geometry it is drawn on.
 *
 * Half the stroke width on each side, always. Round caps and round joins add nothing
 * beyond that half-width -- a round cap is a semicircle of exactly that radius centred on
 * the endpoint -- which is why this is a half and not a guess. A MITRE join would need
 * more, and this sheet declares round, so the number holds as long as that does; the
 * assertion below checks that it still does rather than assuming it.
 */
export function strokeReach(stroke = STROKE) {
  if (stroke['stroke-linejoin'] !== 'round' || stroke['stroke-linecap'] !== 'round') {
    throw new Error(`this reach is computed for round caps and joins; the sheet declares ${stroke['stroke-linecap']}/${stroke['stroke-linejoin']}`);
  }
  return Number(stroke['stroke-width']) / 2;
}

/**
 * The page: every mark drawn as a real, laid-out svg, one per mark.
 *
 * Not the shipped sprite, and the reason is a measurement rather than a preference. The
 * sprite keeps its marks inside `<symbol>` elements, which are never laid out -- they
 * exist to be referenced by `<use>` -- so `getBBox()` on a path inside one has no box to
 * report and comes back as zeros or throws, depending on the engine. The first version of
 * this tool measured the sprite and got nothing back at all, which is the failure this
 * comment exists to stop somebody re-discovering.
 *
 * So each mark is drawn here the way a face draws it: the same viewBox, the same stroke
 * declarations, the same path data, laid out for real. The path data is the identical
 * object the sprite is built from -- `sheetMarks()` -- so there is no second copy of a
 * coordinate anywhere in this file.
 */
export function page(marks = sheetMarks()) {
  const attrs = Object.entries(STROKE).map(([k, v]) => `${k}="${v}"`).join(' ');
  const body = marks.map((mark) => {
    const paths = mark.strokes.map((s) => `<path d="${s.d}"${s.dasharray ? ` stroke-dasharray="${s.dasharray}"` : ''}/>`).join('');
    return `<svg data-id="${mark.id}" width="${BOX}" height="${BOX}" viewBox="0 0 ${BOX} ${BOX}" ${attrs}>${paths}</svg>`;
  }).join('');
  return `<!doctype html><meta charset="utf-8"><body>${body}</body>`;
}

/**
 * Every mark's drawn bounds, read from the renderer.
 *
 * Each `<symbol>` is measured through its own paths rather than through a `<use>`,
 * because `getBBox()` on a `<use>` reports the use element's box and not the geometry
 * inside it.
 */
export async function measure({ viewport = { width: 400, height: 400 } } = {}) {
  const renderer = await startRenderer({ viewport });
  try {
    const view = await renderer.openPage();
    const html = page();
    const file = path.join(HERE, '.glyph-bounds.html');
    fs.writeFileSync(file, html, 'utf8');
    try {
      await view.open(`file:///${file.replace(/\\/g, '/')}`);
      const read = await view.evaluate(`JSON.stringify([...document.querySelectorAll('svg[data-id]')].map((s) => {
        const boxes = [...s.querySelectorAll('path')].map((p) => { const b = p.getBBox(); return { x: b.x, y: b.y, w: b.width, h: b.height }; });
        if (boxes.length === 0) return { id: s.dataset.id, paths: 0 };
        return {
          id: s.dataset.id,
          paths: boxes.length,
          minX: Math.min(...boxes.map((b) => b.x)),
          minY: Math.min(...boxes.map((b) => b.y)),
          maxX: Math.max(...boxes.map((b) => b.x + b.w)),
          maxY: Math.max(...boxes.map((b) => b.y + b.h)),
        };
      }))`);
      return JSON.parse(read);
    } finally {
      fs.rmSync(file, { force: true });
    }
  } finally {
    await renderer.stop();
  }
}

/** The verdict for one measured mark: which edges its stroke crosses, and by how much. */
export function verdictFor(reading, { box = BOX, reach = strokeReach() } = {}) {
  if (!reading.paths) return { id: reading.id, fits: false, why: 'the mark draws no path at all', over: {} };
  const over = {};
  if (reading.minX - reach < 0) over.left = +(reach - reading.minX).toFixed(3);
  if (reading.minY - reach < 0) over.top = +(reach - reading.minY).toFixed(3);
  if (reading.maxX + reach > box) over.right = +(reading.maxX + reach - box).toFixed(3);
  if (reading.maxY + reach > box) over.bottom = +(reading.maxY + reach - box).toFixed(3);
  return { id: reading.id, fits: Object.keys(over).length === 0, over, reading };
}

export async function survey(options = {}) {
  const readings = await measure(options);
  const rows = readings.map((r) => verdictFor(r, options));
  const declared = sheetMarks().length;
  return {
    declared,
    measured: rows.length,
    complete: rows.length === declared && declared > 0,
    clipped: rows.filter((r) => !r.fits),
    rows,
    reach: strokeReach(),
    box: BOX,
  };
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url))) {
  const out = await survey();
  const at = process.argv.indexOf('--json');
  if (at !== -1) fs.writeFileSync(process.argv[at + 1], JSON.stringify(out, null, 2), 'utf8');
  if (!out.complete) {
    process.stdout.write(`${BOUNDS_MESSAGES.EMPTY}: measured ${out.measured} of ${out.declared} declared\n`);
    process.exit(1);
  }
  for (const row of out.rows) {
    const r = row.reading;
    process.stdout.write(`${row.fits ? 'fits ' : 'CLIPS'} ${row.id.padEnd(28)} x ${r.minX.toFixed(2)}..${r.maxX.toFixed(2)}  y ${r.minY.toFixed(2)}..${r.maxY.toFixed(2)}${row.fits ? '' : `  over ${JSON.stringify(row.over)}`}\n`);
  }
  process.stdout.write(`\n${out.measured} marks, stroke reach ${out.reach}, box ${out.box}: ${out.clipped.length === 0 ? BOUNDS_MESSAGES.FITS : `${out.clipped.length} ${BOUNDS_MESSAGES.CLIPPED}`}\n`);
  process.exit(out.clipped.length === 0 ? 0 : 1);
}
