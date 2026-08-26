// SPDX-License-Identifier: Apache-2.0
// Put the fixtures in front of a real renderer, photograph them, and measure the
// things a photograph is needed to settle.
//
// The measurements are not the verdict (req/03 §4-2: "DOM rect normal" is not "real
// drawing normal"). What this tool produces is a picture for a person to look at and
// a set of numbers saying where to look, and the second is written down without
// pretending it replaces the first.
//
// Six readings, the same population req/03 §4-3 asks every face to hold at zero or
// to state plainly:
//   overlaps            -- two texts occupying the same pixels (AC-F1)
//   repeatedEntries      -- one entry drawn twice
//   oversizeGlyphs       -- a glyph that drew at something other than the size asked
//                           for (AC-F3)
//   filledGlyphs         -- a mark at the right size and place, wrong drawing
//   clippedWithoutFull    -- a value cut off on its line with no full copy anywhere
//                           on the page (this face draws none of these on purpose --
//                           every line here is `overflow-wrap:anywhere`, never a
//                           fixed-pitch cell -- so the reading exists to prove that
//                           rather than assume it)
//   sprites              -- the glyph sheet installed exactly once

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { startRenderer } from '../../../tools/rig/renderer.mjs';
import { writeFixtures, SHOT_DIR, NARROW, WIDE } from './fixture.mjs';

export const SHOOT_MESSAGES = {
  NO_RENDERER: 'no renderer was reached, so nothing here has been seen and no visual claim may be made',
};

const PROBE = `(() => {
  // Same reasoning as the ledger face's shoot.mjs: closed <details> (the legend,
  // "why") is hidden by Chrome with content-visibility, not display:none, so
  // getBoundingClientRect keeps answering for it. checkVisibility is excluded the
  // same way a display:none node already was.
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
  const entryNodes = [...document.querySelectorAll('[data-entry]')].map((n) => n.getAttribute('data-entry'));
  const counted = {};
  for (const id of entryNodes) counted[id] = (counted[id] || 0) + 1;
  const repeatedEntries = Object.entries(counted).filter(([, n]) => n > 1).map(([id, n]) => ({ id, drawn: n }));
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
  // The clip reading, over the population it should always have had.
  //
  // It examined nodes carrying a data-role and nothing else, which is a population
  // this face decides one attribute at a time -- and it cost a real defect: the
  // grouped row's time cell carried no data-role, so a whole ISO-8601 timestamp drawn
  // into a 72px box was never a candidate for the check that would have caught it,
  // and it took a person looking at a picture. A cell that can cut a value off is any
  // element holding text of its own, which is the same predicate the overlap reading
  // above already uses; the reading is over that now, so a cell cannot leave the
  // population by not being labelled.
  const textCarriers = [...document.querySelectorAll('body *')].filter((node) => {
    if (!visible(node)) return false;
    return [...node.childNodes].some((c) => c.nodeType === 3 && c.textContent.trim() !== '');
  });
  const clippedCellNodes = textCarriers.filter((n) => n.scrollWidth > n.clientWidth + 1);
  const pageText = [...document.querySelectorAll('body, body *')].filter(visible).map((n) => [...n.childNodes].filter((c) => c.nodeType === 3).map((c) => c.textContent).join('')).join('');
  const clippedWithoutFull = clippedCellNodes
    .map((n) => n.textContent.trim())
    .filter((value) => value.length > 0 && pageText.split(value).length < 3);
  // req/97 §4's below-the-top-five reading for this face, measured rather than
  // recounted: the status column wrapped inside a word ("partially_answe" /
  // "red, status 207"). A cell fails here when the widest single word it holds needs
  // more width than the column gave it -- which is what separates a wrap the writer
  // chose (two words, two lines) from a wrap the column forced (one word cut in two).
  //
  // The first version of this reading returned zero on a page whose picture plainly
  // showed the break, and the reason is worth keeping: a Range's *bounding* rect over
  // a word that has already broken is the union of its two line fragments, so it
  // reports the column's own width and the comparison can never fire. The unbroken
  // width is the sum of the fragment rects, not their union. The walk is over every
  // descendant text node rather than direct children, so nesting the word inside a
  // span later cannot silently empty the population either.
  const lineHeightOf = (node) => {
    const declared = parseFloat(getComputedStyle(node).lineHeight);
    return Number.isFinite(declared) ? declared : 20;
  };
  const textNodesIn = (root) => {
    const out = [];
    const walk = (node) => {
      for (const child of node.childNodes) {
        if (child.nodeType === 3 && child.textContent.trim() !== '') out.push(child);
        else if (child.nodeType === 1) walk(child);
      }
    };
    walk(root);
    return out;
  };
  // Owner #348 (4), the line-breaking half, and the same widening the clip reading
  // just got: this measured the outcome column and nothing else, because
  // that is the one column somebody had already found a break in. Every cell that
  // holds text can break inside a word; the population is all of them now.
  const measureCell = (n) => {
    const r = n.getBoundingClientRect();
    const line = lineHeightOf(n);
    const range = document.createRange();
    let longest = '';
    let wordWidth = 0;
    const rects = [];
    for (const textNode of textNodesIn(n)) {
      const value = textNode.textContent;
      const pattern = /\\S+/g;
      let hit = pattern.exec(value);
      while (hit !== null) {
        range.setStart(textNode, hit.index);
        range.setEnd(textNode, hit.index + hit[0].length);
        const boxes = [...range.getClientRects()];
        const width = Math.ceil(boxes.reduce((sum, box) => sum + box.width, 0));
        if (width > wordWidth) { wordWidth = width; longest = hit[0]; }
        for (const box of boxes) rects.push({ top: Math.round(box.top), width: box.width, word: hit[0] });
        hit = pattern.exec(value);
      }
    }
    // The last line of a wrapped run, and what is on it. A run that broke over more
    // than one line and put a single short word (or a fragment of one) alone on the
    // last of them is the orphan this atom forbids.
    const lines = new Map();
    for (const box of rects) lines.set(box.top, [...(lines.get(box.top) ?? []), box]);
    const tops = [...lines.keys()].sort((a, b) => a - b);
    const lastLine = tops.length > 1 ? lines.get(tops[tops.length - 1]) : null;
    return {
      role: n.getAttribute('data-role') || n.getAttribute('data-type') || n.tagName.toLowerCase(),
      text: n.textContent.trim().slice(0, 40),
      w: Math.round(r.width),
      h: Math.round(r.height),
      lines: Math.max(1, Math.round(r.height / line)),
      longest,
      longestWordPx: wordWidth,
      lastLineWords: lastLine === null ? null : lastLine.length,
      lastLinePx: lastLine === null ? null : Math.ceil(lastLine.reduce((sum, box) => sum + box.width, 0)),
      lastLineText: lastLine === null ? null : lastLine.map((box) => box.word).join(' ').slice(0, 24),
    };
  };
  const cellsMeasured = textCarriers.map(measureCell);
  const outcomeMeasured = cellsMeasured.filter((c) => c.role === 'entry-outcome');
  // Two conditions, and the first one is what the narrow population was hiding.
  //
  // Widened to every cell, the bare comparison fired five times per page on cells that
  // had not wrapped at all: a single-line cell whose one word measured 50px in a 49px
  // box, which is the sum of the word's own client rects against a rounded box width,
  // not a break. A cell drawing one line has broken nothing by definition, and a pixel
  // of rounding is not a defect -- so the reading asks for a cell that actually took
  // more than one line, and for the word to want more than a pixel more than it got.
  const wrappedInsideAWord = cellsMeasured.filter((c) => c.lines > 1 && c.longestWordPx > c.w + 1);
  // One word on the last line, and that word narrower than a fifth of the column it
  // is in: a stranded fragment rather than a short final word that happens to be
  // short. The threshold is stated rather than tuned -- a number this reading prints
  // beside every hit, so a hit can be argued with.
  const orphanLastLine = cellsMeasured.filter((c) => c.lastLineWords === 1 && c.lastLinePx !== null && c.lastLinePx * 5 < c.w);
  // SS553: same tap-target reading as the ledger face's shoot.mjs.
  const interactive = [...document.querySelectorAll('button, summary, a[href]')].filter((n) => visible(n) && !n.disabled);
  const underTapBudget = interactive.map((n) => {
    const r = n.getBoundingClientRect();
    return {
      tag: n.tagName.toLowerCase(), text: n.textContent.trim().slice(0, 24), w: Math.round(r.width), h: Math.round(r.height),
    };
  }).filter((n) => n.w < 36 || n.h < 36);
  return {
    textBoxes: boxes.length,
    overlapCount: overlaps.length,
    overlaps: overlaps.slice(0, 10),
    entriesDrawn: entryNodes.length,
    repeatedEntries,
    glyphs: glyphs.length,
    oversizeGlyphs,
    filledGlyphs,
    sprites: document.querySelectorAll('#gx-glyph-sheet').length,
    horizontalOverflow: doc.scrollWidth > doc.clientWidth ? doc.scrollWidth - doc.clientWidth : 0,
    textCarriers: textCarriers.length,
    clippedCells: clippedCellNodes.length,
    clippedWithoutFull,
    outcomeCells: outcomeMeasured,
    wrappedInsideAWord,
    orphanLastLine,
    interactiveControls: interactive.length,
    underTapBudget,
    visibleTextChars: pageText.replace(/\s+/g, ' ').trim().length,
    // Owner #348 (4) asks for the visible-character count before and after, and the
    // reading above cannot answer it alone: a closed disclosure is hidden, so 1,700
    // characters of prose duplicated inside two folds counted zero either way.
    // This one counts every character the page holds, folded or open.
    drawnTextChars: (document.body.textContent || '').replace(/\s+/g, ' ').trim().length,
    // What the page is actually as tall as. The shot was clipped to a 1400px viewport
    // for about 700px of content, so every capture carried half a page of empty.
    contentHeight: Math.ceil(document.body.getBoundingClientRect().height),
    background: getComputedStyle(document.body).backgroundColor,
    ink: getComputedStyle(document.body).color,
    tokensResolved: getComputedStyle(doc).getPropertyValue('--row').trim(),
  };
})()`;

// Light is this application's stated default and it does not arrive on its own: the
// roster of record declares the dark palette on a bare :root and a headless renderer
// prefers dark, so every capture states which preference it was taken under.
const VIEWS = {
  narrow: { viewport: NARROW, scheme: 'light' },
  wide: { viewport: WIDE, scheme: 'light' },
  dark: { viewport: NARROW, scheme: 'dark' },
};

/**
 * The picture, cut to the page rather than to the window.
 *
 * The shared rig's own capture clips to the viewport, which is right for a tool that
 * wants to know what fits on a screen and wrong for these: this face's representative
 * page is about 700px of content in a 1400px viewport, so every shot carried half a
 * page of empty ground and a reader looking at one had to hunt for where the screen
 * stopped. This asks the renderer for exactly the height the body measured, and past
 * the viewport where the page is longer than one -- the overflow page is 200 rows and
 * was being photographed from the top down to wherever 1400px landed.
 *
 * The ceiling is stated rather than assumed: a 200-row page is tall, and a PNG of the
 * whole of it is a file nobody opens. What is above the ceiling is not hidden, it is
 * reported -- `contentHeight` is in the measurements beside the shot.
 */
const SHOT_CEILING = 3600;

async function capturePage(page, viewport, contentHeight) {
  const height = Math.min(Math.max(contentHeight, 1), SHOT_CEILING);
  const { data } = await page.raw.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: height > viewport.height,
    fromSurface: true,
    clip: { x: 0, y: 0, width: viewport.width, height, scale: 1 },
  });
  return Buffer.from(data, 'base64');
}

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
        fs.writeFileSync(shot, await capturePage(page, viewport, measured.contentHeight));
        report.push({
          fixture: fixture.name, view, scheme, width: viewport.width, shot, shotHeight: Math.min(measured.contentHeight, SHOT_CEILING), ...measured,
        });
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
      process.stdout.write(`${row.fixture} [${row.scheme} ${row.width}px] overlaps=${row.overlapCount} entries=${row.entriesDrawn} repeated=${row.repeatedEntries.length} glyphs=${row.glyphs} oversize=${row.oversizeGlyphs.length} filled=${row.filledGlyphs.length} sprites=${row.sprites} overflow=${row.horizontalOverflow} cells=${row.textCarriers} clipped=${row.clippedCells} clippedWithoutFull=${row.clippedWithoutFull.length} wrappedInsideAWord=${row.wrappedInsideAWord.length} orphanLastLine=${row.orphanLastLine.length} controls=${row.interactiveControls} underTapBudget=${row.underTapBudget.length} textChars=${row.visibleTextChars} drawnChars=${row.drawnTextChars} content=${row.contentHeight}px shot=${row.shotHeight}px bg=${row.background}\n`);
      for (const c of row.wrappedInsideAWord) process.stdout.write(`    wrapped inside a word: ${JSON.stringify(c.longest)} needs ${c.longestWordPx}px, the column gave it ${c.w}px (${c.lines} lines, ${c.role})\n`);
      for (const c of row.orphanLastLine) process.stdout.write(`    one word alone on the last line: ${JSON.stringify(c.lastLineText)} at ${c.lastLinePx}px in a ${c.w}px column (${c.role})\n`);
      for (const c of row.clippedWithoutFull) process.stdout.write(`    cut off with no full copy anywhere: ${JSON.stringify(c.slice(0, 48))}\n`);
      for (const o of row.overlaps) process.stdout.write(`    overlap ${o.area}px2: ${JSON.stringify(o.a)} / ${JSON.stringify(o.b)}\n`);
      for (const g of row.oversizeGlyphs) process.stdout.write(`    glyph ${g.mark} asked ${g.asked} drew ${g.w}x${g.h}\n`);
      for (const t of row.underTapBudget) process.stdout.write(`    under tap budget: <${t.tag}> "${t.text}" ${t.w}x${t.h}\n`);
    }
  }).catch((error) => {
    process.stderr.write(`${SHOOT_MESSAGES.NO_RENDERER}: ${error.message}\n`);
    process.exitCode = 1;
  });
}
