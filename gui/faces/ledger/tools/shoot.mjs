// SPDX-License-Identifier: Apache-2.0
// Put the fixtures in front of a real renderer, photograph them, and measure the
// things a photograph is needed to settle.
//
// The measurements are not the verdict. On the day the shipped window drew one text on
// top of another, the DOM rectangles were correct (req/03 N-1), so what this tool
// produces is a picture for a person to look at and a set of numbers saying where to
// look. Both are written down; the second without the first would be the same mistake
// again.
//
// Four readings, one for each defect in the negative truth ledger:
//   overlaps        -- two texts occupying the same pixels (N-1)
//   repeatedRows    -- one row drawn twice (N-2)
//   oversizeGlyphs  -- a glyph that drew at something other than the size it asked for,
//                      which is how an unsized mark announces itself (N-2)
//   filledGlyphs    -- a mark that is the right size in the right place and the wrong
//                      drawing, which every rectangle in the tree reports as fine

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { startRenderer } from '../../../tools/rig/renderer.mjs';
import { writeFixtures, SHOT_DIR, NARROW, WIDE } from './fixture.mjs';

export const SHOOT_MESSAGES = {
  NO_RENDERER: 'no renderer was reached, so nothing here has been seen and no visual claim may be made',
};

const PROBE = `(() => {
  // Closed <details> content (the legend, "why") is real in the DOM and real in the
  // accessibility tree, but Chrome hides it with content-visibility rather than
  // display:none, so getBoundingClientRect keeps reporting its old layout box even
  // though nothing is painted there -- a node that is honestly absent from the
  // picture would otherwise be counted as present and overlapping whatever sits
  // where it used to be. checkVisibility is the platform's own answer to exactly
  // this (it is what content-visibility was built alongside); nodes it marks
  // invisible are excluded the same way a display:none node already was.
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
  // A cell whose text is wider than its box is cut off on screen. That is allowed --
  // the row is one fixed line -- but only if the whole value is legible somewhere on
  // the page, so each clipped value is looked for a second time outside its own cell.
  const clippedCellNodes = [...document.querySelectorAll('[data-cell]')].filter((n) => n.scrollWidth > n.clientWidth + 1 && visible(n));
  const pageText = [...document.querySelectorAll('body, body *')].filter(visible).map((n) => [...n.childNodes].filter((c) => c.nodeType === 3).map((c) => c.textContent).join('')).join('');
  const clippedWithoutFull = clippedCellNodes
    .map((n) => n.textContent.trim())
    .filter((value) => value.length > 0 && pageText.split(value).length < 3);
  const clipped = clippedCellNodes.length;
  // The reading above asks the cell, and the cell is not always the thing that is cut.
  // Three of these columns put their text in a child span and let THAT span carry the
  // ellipsis, so the cell's own scrollWidth equals its clientWidth while the word
  // inside it reads "del...". Every capture of this face reported clipped=0 while a
  // verdict was drawing as "A..." on screen, which is a probe agreeing with a defect.
  const ellipsized = [...new Set([...document.querySelectorAll('[data-cell] *')]
    .filter((n) => visible(n) && n.childNodes.length > 0 && n.scrollWidth > n.clientWidth + 1)
    .map((n) => n.textContent.trim().slice(0, 24))
    .filter((t) => t.length > 0))];
  // SS553: every interactive control is at least 36x36px, measured rather than
  // eyeballed. Only counts controls a reader could actually reach right now --
  // closed <details> content is excluded the same way it is everywhere else in
  // this probe (checkVisibility), and a disabled control is excluded too, because
  // Chrome itself already refuses to focus or click one (verified interactively:
  // Playwright times out trying).
  const interactive = [...document.querySelectorAll('button, summary, a[href]')].filter((n) => visible(n) && !n.disabled);
  const underTapBudget = interactive.map((n) => {
    const r = n.getBoundingClientRect();
    return {
      tag: n.tagName.toLowerCase(), text: n.textContent.trim().slice(0, 24), w: Math.round(r.width), h: Math.round(r.height),
    };
  }).filter((n) => n.w < 36 || n.h < 36);
  // Owner #348 (3), the cost side. Raising a mark to the readable floor makes it wider
  // as well as taller, and a control whose own contents no longer fit inside it draws
  // its word running out past its border -- which the clipped-cell reading above does
  // not see, because it only looks at a cell, and which the tap-target reading
  // does not see either, because the button's box is the right size and it is the
  // contents that are not. Both were true of this face's own act gutter after the
  // floor landed, in a capture that read zero on every existing probe.
  const overflowingControls = [...document.querySelectorAll('button, [data-role="segment"] [data-role="noun"]')]
    .filter(visible)
    .map((n) => ({
      what: n.getAttribute('data-act') || n.getAttribute('data-menu-item') || n.textContent.trim().slice(0, 20),
      box: n.clientWidth,
      needs: n.scrollWidth,
      over: n.scrollWidth - n.clientWidth,
    }))
    .filter((n) => n.over > 0);
  // What a row costs, per half. The act gutter stacks its buttons, so a row offering
  // three acts is as tall as three controls whatever its own line needs -- a number
  // worth having on the record rather than in somebody's memory of a screenshot.
  const rowHeights = [...document.querySelectorAll('[data-part="row-gutter-frame"]')].map((frame) => {
    const row = frame.querySelector('[data-row]');
    const gutter = frame.querySelector('[data-part="act-gutter"]');
    return {
      id: row ? row.getAttribute('data-row') : null,
      frame: Math.round(frame.getBoundingClientRect().height),
      line: row ? Math.round(row.getBoundingClientRect().height) : null,
      acts: gutter ? gutter.querySelectorAll('button').length : 0,
    };
  });
  const menus = document.querySelectorAll('[data-role="row-menu"]').length;
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
    ellipsized,
    interactiveControls: interactive.length,
    underTapBudget,
    overflowingControls,
    rowHeights,
    menus,
    visibleTextChars: pageText.replace(/\s+/g, ' ').trim().length,
    background: getComputedStyle(document.body).backgroundColor,
    ink: getComputedStyle(document.body).color,
    tokensResolved: getComputedStyle(doc).getPropertyValue('--row').trim(),
  };
})()`;

// Light is this application's stated default and it does not arrive on its own: the
// roster of record declares the dark palette on a bare :root and a headless renderer
// prefers dark, so every capture states which preference it was taken under instead of
// inheriting one and calling the result the default.
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
      process.stdout.write(`${row.fixture} [${row.scheme} ${row.width}px] overlaps=${row.overlapCount} rows=${row.rowsDrawn} repeated=${row.repeatedRows.length} glyphs=${row.glyphs} oversize=${row.oversizeGlyphs.length} filled=${row.filledGlyphs.length} sprites=${row.sprites} overflow=${row.horizontalOverflow} clipped=${row.clippedCells} clippedWithoutFull=${row.clippedWithoutFull.length} controls=${row.interactiveControls} underTapBudget=${row.underTapBudget.length} overflowingControls=${row.overflowingControls.length} ellipsized=${row.ellipsized.length} menus=${row.menus} textChars=${row.visibleTextChars} bg=${row.background}\n`);
      for (const o of row.overlaps) process.stdout.write(`    overlap ${o.area}px2: ${JSON.stringify(o.a)} / ${JSON.stringify(o.b)}\n`);
      for (const g of row.oversizeGlyphs) process.stdout.write(`    glyph ${g.mark} asked ${g.asked} drew ${g.w}x${g.h}\n`);
      for (const t of row.underTapBudget) process.stdout.write(`    under tap budget: <${t.tag}> "${t.text}" ${t.w}x${t.h}\n`);
      if (row.ellipsized.length > 0) process.stdout.write(`    text cut inside a cell, with no ellipsis reading on the cell itself: ${row.ellipsized.map((t) => JSON.stringify(t)).join(', ')}
`);
      for (const c of row.overflowingControls) process.stdout.write(`    control overflows its own box by ${c.over}px: "${c.what}" needs ${c.needs} in ${c.box}\n`);
      for (const h of row.rowHeights) process.stdout.write(`    row ${h.id}: ${h.frame}px for a ${h.line}px line, ${h.acts} act${h.acts === 1 ? '' : 's'}\n`);
    }
  }).catch((error) => {
    process.stderr.write(`${SHOOT_MESSAGES.NO_RENDERER}: ${error.message}\n`);
    process.exitCode = 1;
  });
}
