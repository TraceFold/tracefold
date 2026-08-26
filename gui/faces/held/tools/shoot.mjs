// SPDX-License-Identifier: Apache-2.0
// Put the fixtures in front of a real renderer, photograph them, and measure the
// things a photograph is needed to settle. Same discipline faces/ledger's shoot.mjs
// states: the measurements are not the verdict, the picture is -- both are written
// down, and the numbers exist so a person knows where to look, not so nobody has to
// look.
//
// The probe below reuses the exact in-page measurement faces/ledger and
// faces/notice both run (overlaps / repeated rows / oversize+filled glyphs /
// clipped-without-full / tap targets): it is this project's one shared instrument
// for "does the DOM say this is fine while the picture says otherwise", not a
// per-face reinvention of the same four readings. One addition specific to this
// face: `sealValues`, which must be zero on every capture -- the held screen's own
// negative truth is a seal cell that ever reads as anything but a hole.

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
  // held's own reading: a seal cell that is not a declared hole is this face's
  // one defect a generic probe could not otherwise name -- see faces/held/README.md.
  const sealValues = [...document.querySelectorAll('[data-cell="seal"]')].filter((n) => n.getAttribute('data-state') !== 'hole').length;
  // What one candidate costs in height, and what everything above the first one
  // costs. req/97 measured this face at 8 rows to a thousand pixels against a bar of
  // about 40 and named the cause: the act gutter stacks four buttons beside a
  // one-line row, so the row's own floor is the gutter's height. The two figures are
  // kept apart because they move for different reasons -- the pitch is per row and
  // the head is paid once -- and a retrofit that adds a fixed head to the screen
  // must not be able to report the pitch getting worse when it did not.
  // What the two things added above the list cost in height, so the head can be
  // read as its parts rather than as one number that grew.
  const heightOf = (selector) => {
    const node = document.querySelector(selector);
    return node ? Math.round(node.getBoundingClientRect().height) : null;
  };
  const ladderPx = heightOf('[data-section="gates"]');
  const bandPx = heightOf('[data-part="stat-band"]');
  // Owner #348 (4), the half of it no static tree can answer. Where a line ends is
  // the renderer's decision, so a claim about mid-word breaks and about a last line
  // holding a single character has to be read off the line boxes it actually drew.
  // A Range over a paragraph's contents reports one rectangle per line box; a last
  // rectangle no wider than the paragraph's own em is a line with about one
  // character on it, which is the orphan the directive names.
  const proseNodes = [...document.querySelectorAll('p, [data-role="menu-why"], [data-role="gate-why"]')].filter(visible);
  const orphanLines = [];
  for (const node of proseNodes) {
    const range = document.createRange();
    range.selectNodeContents(node);
    const rects = [...range.getClientRects()].filter((r) => r.width > 0);
    if (rects.length < 2) continue;
    const last = rects[rects.length - 1];
    const em = parseFloat(getComputedStyle(node).fontSize) || 14;
    if (last.width <= em) orphanLines.push({ text: node.textContent.trim().slice(0, 40), lines: rects.length, lastPx: Math.round(last.width) });
  }
  const breakAnywhere = proseNodes.filter((n) => getComputedStyle(n).overflowWrap === 'anywhere').length;
  // The reading the cell-only clip check cannot make. A control is not a [data-cell],
  // so a button whose label no longer fits the track it was given is invisible to
  // clippedCells -- and raising a mark's size raises exactly that risk, on exactly
  // the element a hand is aimed at. Both readings are taken: content wider than the
  // box, and a box drawn past the right edge of the container it sits in.
  const clippedControls = [...document.querySelectorAll('button')].filter(visible).map((n) => {
    const box = n.getBoundingClientRect();
    const owner = n.closest('[data-part="box"]') || document.body;
    const bounds = owner.getBoundingClientRect();
    return {
      text: n.textContent.trim().slice(0, 24),
      overflowPx: Math.max(0, n.scrollWidth - n.clientWidth),
      pastOwnerPx: Math.round(Math.max(0, box.right - bounds.right)),
    };
  }).filter((n) => n.overflowPx > 1 || n.pastOwnerPx > 1);
  const menusDrawn = document.querySelectorAll('[data-menu]').length;
  const rowTops = [...document.querySelectorAll('[data-part="selectable-row"]')].filter(visible).map((n) => n.getBoundingClientRect().y);
  const gaps = rowTops.slice(1).map((y, i) => y - rowTops[i]);
  const rowPitchPx = gaps.length > 0 ? Math.round(gaps.reduce((a, b) => a + b, 0) / gaps.length) : null;
  return {
    firstRowTopPx: rowTops.length > 0 ? Math.round(rowTops[0]) : null,
    ladderPx,
    bandPx,
    rowPitchPx,
    rowsPerThousandPx: rowPitchPx ? Math.round((1000 / rowPitchPx) * 10) / 10 : null,
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
    sealValues,
    proseBlocks: proseNodes.length,
    orphanLines,
    breakAnywhere,
    clippedControls,
    menusDrawn,
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
      process.stdout.write(`${row.fixture} [${row.scheme} ${row.width}px] overlaps=${row.overlapCount} rows=${row.rowsDrawn} repeated=${row.repeatedRows.length} glyphs=${row.glyphs} oversize=${row.oversizeGlyphs.length} filled=${row.filledGlyphs.length} sprites=${row.sprites} overflow=${row.horizontalOverflow} clipped=${row.clippedCells} clippedWithoutFull=${row.clippedWithoutFull.length} sealValues=${row.sealValues} controls=${row.interactiveControls} underTapBudget=${row.underTapBudget.length} rowPitch=${row.rowPitchPx} rowsPer1000=${row.rowsPerThousandPx} firstRowTop=${row.firstRowTopPx} ladder=${row.ladderPx} band=${row.bandPx} menus=${row.menusDrawn} prose=${row.proseBlocks} orphans=${row.orphanLines.length} breakAnywhere=${row.breakAnywhere} clippedControls=${row.clippedControls.length} textChars=${row.visibleTextChars} bg=${row.background}\n`);
      for (const o of row.overlaps) process.stdout.write(`    overlap ${o.area}px2: ${JSON.stringify(o.a)} / ${JSON.stringify(o.b)}\n`);
      for (const g of row.oversizeGlyphs) process.stdout.write(`    glyph ${g.mark} asked ${g.asked} drew ${g.w}x${g.h}\n`);
      for (const t of row.underTapBudget) process.stdout.write(`    under tap budget: <${t.tag}> "${t.text}" ${t.w}x${t.h}\n`);
      for (const c of row.clippedControls) process.stdout.write(`    control "${c.text}" overflows its own box by ${c.overflowPx}px and its container by ${c.pastOwnerPx}px\n`);
    }
  }).catch((error) => {
    process.stderr.write(`${SHOOT_MESSAGES.NO_RENDERER}: ${error.message}\n`);
    process.exitCode = 1;
  });
}
