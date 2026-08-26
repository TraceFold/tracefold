// SPDX-License-Identifier: Apache-2.0
// Put the fixtures in front of a real renderer, photograph them, and measure the two
// things a photograph is needed for.
//
// The measurement is not the verdict. DOM rectangles were correct on the day the
// shipped window drew one text on top of another (req/03 §4-2), so this writes both:
// the rectangles, which can be wrong while looking right, and the picture, which a
// person then looks at. The rectangles are here to say where to look.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { startRenderer } from '../../tools/rig/renderer.mjs';
import { writeFixtures, FIXTURE_DIR, SHOT_DIR, NARROW, WIDE } from './fixtures.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));

export const SHOOT_MESSAGES = {
  NO_RENDERER: 'no renderer to photograph with, so nothing here has been seen',
  SHOT: 'captured',
};

/**
 * Every pair of text-bearing boxes that overlap. Text-bearing means the element has a
 * string of its own -- a container overlapping its child is the layout working.
 */
const OVERLAP_PROBE = `(() => {
  const boxes = [];
  for (const node of document.querySelectorAll('body *')) {
    const ownText = [...node.childNodes].some((c) => c.nodeType === 3 && c.textContent.trim() !== '');
    if (!ownText) continue;
    const r = node.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) continue;
    boxes.push({ tag: node.tagName.toLowerCase(), role: node.getAttribute('data-role') || node.getAttribute('data-cell') || '', text: node.textContent.trim().slice(0, 40), x: r.x, y: r.y, w: r.width, h: r.height });
  }
  const overlaps = [];
  for (let i = 0; i < boxes.length; i += 1) {
    for (let j = i + 1; j < boxes.length; j += 1) {
      const a = boxes[i]; const b = boxes[j];
      const ox = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
      const oy = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
      if (ox > 1 && oy > 1) overlaps.push({ a: a.text, b: b.text, area: Math.round(ox * oy) });
    }
  }
  const glyphs = [...document.querySelectorAll('svg[data-mark]')].map((s) => {
    const r = s.getBoundingClientRect();
    return { mark: s.getAttribute('data-mark'), w: Math.round(r.width), h: Math.round(r.height), asked: s.getAttribute('width') };
  });
  const oversize = glyphs.filter((g) => g.w > 40 || g.h > 40 || String(g.w) !== String(g.asked));
  // A mark whose paths fill instead of stroking is the right size, in the right
  // place, and the wrong drawing. Measured on the instance, after the cascade.
  const filled = [...document.querySelectorAll('svg[data-mark]')].map((s) => {
    const c = getComputedStyle(s);
    return { mark: s.getAttribute('data-mark'), fill: c.fill, stroke: c.stroke, width: c.strokeWidth };
  }).filter((g) => g.fill !== 'none' || g.stroke === 'none');
  const doc = document.documentElement;
  return {
    textBoxes: boxes.length,
    overlaps: overlaps.slice(0, 12),
    overlapCount: overlaps.length,
    glyphs: glyphs.length,
    oversizeGlyphs: oversize,
    filledGlyphs: filled,
    horizontalOverflow: doc.scrollWidth > doc.clientWidth ? doc.scrollWidth - doc.clientWidth : 0,
    background: getComputedStyle(document.body).backgroundColor,
    ink: getComputedStyle(document.body).color,
    sprites: document.querySelectorAll('#gx-glyph-sheet').length,
    tokensResolved: getComputedStyle(document.documentElement).getPropertyValue('--row').trim(),
  };
})()`;

/**
 * Each capture states the light preference it was taken under rather than inheriting
 * whatever the renderer happens to prefer. That is not a formality: the first run of
 * this tool came back with a dark page on every shot, because the stylesheet of
 * record declares the dark palette on bare :root and headless prefers dark. Light is
 * this application's stated default and it does not arrive on its own -- something
 * has to ask for it. Since no part nails a theme, the ask lives here, and the shots
 * are labelled with which one they are.
 */
const VIEWPORTS = {
  narrow: { viewport: NARROW, scheme: 'light' },
  wide: { viewport: WIDE, scheme: 'light' },
  dark: { viewport: NARROW, scheme: 'dark' },
};

export async function shoot({ only = null } = {}) {
  const written = writeFixtures();
  fs.mkdirSync(SHOT_DIR, { recursive: true });
  const report = [];

  for (const name of Object.keys(VIEWPORTS)) {
    const wanted = written.filter((f) => f.viewports.includes(name) && (!only || f.name === only));
    if (wanted.length === 0) continue;
    const { viewport, scheme } = VIEWPORTS[name];
    const renderer = await startRenderer({ viewport });
    try {
      for (const fixture of wanted) {
        const page = await renderer.openPage();
        await page.raw.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: scheme }] });
        await page.open(pathToFileURL(fixture.path).href);
        const measured = await page.evaluate(OVERLAP_PROBE);
        const png = await page.capture();
        const shot = path.join(SHOT_DIR, `${fixture.name.replace('.html', '')}_${name}.png`);
        fs.writeFileSync(shot, png);
        report.push({ fixture: fixture.name, viewport: name, scheme, width: viewport.width, shot, ...measured });
        await page.close();
      }
    } finally {
      await renderer.stop();
    }
  }

  fs.writeFileSync(path.join(SHOT_DIR, 'measurements.json'), `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  return report;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const only = process.argv[2] ?? null;
  shoot({ only }).then((report) => {
    for (const row of report) {
      process.stdout.write(`${row.fixture} [${row.scheme} ${row.width}px] overlaps=${row.overlapCount} glyphs=${row.glyphs} oversize=${row.oversizeGlyphs.length} filled=${row.filledGlyphs.length} sprites=${row.sprites} overflow=${row.horizontalOverflow} bg=${row.background}\n`);
      for (const o of row.overlaps) process.stdout.write(`    overlap ${o.area}px2: ${JSON.stringify(o.a)} / ${JSON.stringify(o.b)}\n`);
      for (const g of row.oversizeGlyphs) process.stdout.write(`    glyph ${g.mark} asked ${g.asked} drew ${g.w}x${g.h}\n`);
    }
    process.stdout.write(`${HERE}\n`);
  }).catch((error) => {
    process.stderr.write(`${SHOOT_MESSAGES.NO_RENDERER}: ${error.message}\n`);
    process.exitCode = 1;
  });
}
