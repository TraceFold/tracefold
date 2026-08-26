// SPDX-License-Identifier: Apache-2.0
// Put the fixtures in front of a real renderer, photograph them, and measure the
// things a photograph is needed to settle. Same discipline faces/ledger's,
// faces/held's and faces/receipt's own shoot.mjs state: the measurements are not
// the verdict, the picture is -- both are written down, and the numbers exist so a
// person knows where to look, not so nobody has to look.
//
// The probe below reuses the exact in-page measurement every other face in this
// tree runs (overlaps / repeated rows / oversize+filled glyphs / clipped-without-
// full / tap targets). One addition specific to this face: `edgeOutsideAnnotations`,
// which records how many `structure/outside` annotations this fixture actually drew
// -- the happy-path fixture is built to draw at least one, so this reading is what
// would catch a future fixture edit that silently stopped exercising the negative
// case (unlike faces/receipt's `sealedMarksDrawn`, this one is expected to be
// nonzero on the `graph` fixture, not zero).

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { startRenderer } from '../../../tools/rig/renderer.mjs';
import { writeFixtures, SHOT_DIR, NARROW, WIDE } from './fixture.mjs';

export const SHOOT_MESSAGES = {
  NO_RENDERER: 'no renderer was reached, so nothing here has been seen and no visual claim may be made',
};

const PROBE = `(() => {
  const visible = (node) => (typeof node.checkVisibility === 'function'
    ? node.checkVisibility({ checkVisibilityCSS: true, checkOpacity: false, checkContentVisibilityAuto: false })
    : true);
  const boxes = [];
  for (const node of document.querySelectorAll('body *')) {
    const own = [...node.childNodes].some((c) => c.nodeType === 3 && c.textContent.trim() !== '');
    if (!own) continue;
    if (!visible(node)) continue;
    const r = node.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) continue;
    boxes.push({ text: node.textContent.trim().slice(0, 48), x: r.x, y: r.y, w: r.width, h: r.height });
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
  const rows = [...document.querySelectorAll('[data-row]')].map((n) => n.getAttribute('data-row'));
  const counted = {};
  for (const id of rows) counted[id] = (counted[id] || 0) + 1;
  const repeatedRows = Object.entries(counted).filter(([, n]) => n > 1).map(([id, n]) => ({ id, drawn: n }));
  const glyphNodes = [...document.querySelectorAll('svg[data-mark]')].filter(visible);
  const glyphs = glyphNodes.map((s) => {
    const r = s.getBoundingClientRect();
    return { mark: s.getAttribute('data-mark'), asked: s.getAttribute('width'), w: Math.round(r.width), h: Math.round(r.height) };
  });
  const oversizeGlyphs = glyphs.filter((g) => String(g.w) !== String(g.asked) || g.w > 40 || g.h > 40);
  const filledGlyphs = glyphNodes.map((s) => {
    const c = getComputedStyle(s);
    return { mark: s.getAttribute('data-mark'), fill: c.fill, stroke: c.stroke };
  }).filter((g) => g.fill !== 'none' || g.stroke === 'none');
  const doc = document.documentElement;
  const clippedCellNodes = [...document.querySelectorAll('[data-cell]')].filter((n) => n.scrollWidth > n.clientWidth + 1 && visible(n));
  const pageText = [...document.querySelectorAll('body, body *')].filter(visible).map((n) => [...n.childNodes].filter((c) => c.nodeType === 3).map((c) => c.textContent).join('')).join('');
  const clippedWithoutFull = clippedCellNodes
    .map((n) => n.textContent.trim())
    .filter((value) => value.length > 0 && pageText.split(value).length < 3);
  const clipped = clippedCellNodes.length;
  const interactive = [...document.querySelectorAll('button, summary, a[href]')].filter((n) => visible(n) && !n.disabled);
  const underTapBudget = interactive.map((n) => {
    const r = n.getBoundingClientRect();
    return { tag: n.tagName.toLowerCase(), text: n.textContent.trim().slice(0, 24), w: Math.round(r.width), h: Math.round(r.height) };
  }).filter((n) => n.w < 36 || n.h < 36);
  // graph's own reading: the happy-path fixture is built to draw at least one
  // outside-window annotation -- see faces/graph/README.md.
  const edgeOutsideAnnotations = [...document.querySelectorAll('[data-role="edge-outside"]')].length;
  const chainedRows = [...document.querySelectorAll('[data-child-of]')].filter((n) => n.getAttribute('data-child-of')).length;
  return {
    textBoxes: boxes.length,
    overlapCount: overlaps.length,
    overlaps: overlaps.slice(0, 10),
    rowsDrawn: rows.length,
    repeatedRows,
    glyphs: glyphs.length,
    oversizeGlyphs,
    filledGlyphs,
    sprites: document.querySelectorAll('#gx-glyph-sheet').length,
    horizontalOverflow: doc.scrollWidth > doc.clientWidth ? doc.scrollWidth - doc.clientWidth : 0,
    clippedCells: clipped,
    clippedWithoutFull,
    interactiveControls: interactive.length,
    underTapBudget,
    edgeOutsideAnnotations,
    chainedRows,
    visibleTextChars: pageText.replace(/\s+/g, ' ').trim().length,
    background: getComputedStyle(document.body).backgroundColor,
    ink: getComputedStyle(document.body).color,
    tokensResolved: getComputedStyle(doc).getPropertyValue('--row').trim(),
  };
})()`;

const VIEWS = {
  narrow: { viewport: NARROW, scheme: 'light' },
  wide: { viewport: WIDE, scheme: 'light' },
  dark: { viewport: NARROW, scheme: 'dark' },
};

export async function shoot() {
  const written = writeFixtures();
  fs.mkdirSync(SHOT_DIR, { recursive: true });
  const report = [];

  for (const view of Object.keys(VIEWS)) {
    const wanted = written.filter((f) => f.viewports.includes(view));
    if (wanted.length === 0) continue;
    const { viewport, scheme } = VIEWS[view];
    const renderer = await startRenderer({ viewport });
    try {
      for (const fixture of wanted) {
        const page = await renderer.openPage();
        await page.raw.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: scheme }] });
        await page.open(pathToFileURL(fixture.path).href);
        const measured = await page.evaluate(PROBE);
        const shot = path.join(SHOT_DIR, `${fixture.name}_${view}.png`);
        fs.writeFileSync(shot, await page.capture());
        report.push({ fixture: fixture.name, view, scheme, width: viewport.width, shot, ...measured });
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
  shoot().then((report) => {
    for (const row of report) {
      process.stdout.write(`${row.fixture} [${row.scheme} ${row.width}px] overlaps=${row.overlapCount} rows=${row.rowsDrawn} repeated=${row.repeatedRows.length} glyphs=${row.glyphs} oversize=${row.oversizeGlyphs.length} filled=${row.filledGlyphs.length} sprites=${row.sprites} overflow=${row.horizontalOverflow} clipped=${row.clippedCells} clippedWithoutFull=${row.clippedWithoutFull.length} edgeOutsideAnnotations=${row.edgeOutsideAnnotations} chainedRows=${row.chainedRows} controls=${row.interactiveControls} underTapBudget=${row.underTapBudget.length} textChars=${row.visibleTextChars} bg=${row.background}\n`);
      for (const o of row.overlaps) process.stdout.write(`    overlap ${o.area}px2: ${JSON.stringify(o.a)} / ${JSON.stringify(o.b)}\n`);
      for (const g of row.oversizeGlyphs) process.stdout.write(`    glyph ${g.mark} asked ${g.asked} drew ${g.w}x${g.h}\n`);
      for (const t of row.underTapBudget) process.stdout.write(`    under tap budget: <${t.tag}> "${t.text}" ${t.w}x${t.h}\n`);
    }
  }).catch((error) => {
    process.stderr.write(`${SHOOT_MESSAGES.NO_RENDERER}: ${error.message}\n`);
    process.exitCode = 1;
  });
}
