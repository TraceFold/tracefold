// SPDX-License-Identifier: Apache-2.0
// Put the fixtures in front of a real renderer, photograph them, and measure the
// things a photograph is needed to settle. Same discipline every other face's own
// shoot.mjs states: the measurements are not the verdict, the picture is -- both
// are written down, and the numbers exist so a person knows where to look, not so
// nobody has to look.
//
// The probe below reuses the exact in-page measurement every other face in this
// tree runs (overlaps / repeated rows / oversize+filled glyphs / clipped-without-
// full / tap targets). Two additions specific to this face: `subjectsDrawn` (how
// many subject summary lines this fixture actually drew) and `subjectsOpen` (how
// many were constructed open -- the happy-path fixture is built to draw at least
// one, the same "the negative-truth reading actually fires at least once" discipline
// faces/graph's own `edgeOutsideAnnotations` reading holds).

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
  // atlas's own readings: how many subject summary lines this fixture drew, and how
  // many of them were constructed open -- see faces/atlas/README.md.
  const subjectEls = [...document.querySelectorAll('[data-role="subject"]')];
  const subjectsDrawn = subjectEls.length;
  const subjectsOpen = subjectEls.filter((n) => n.getAttribute('data-open') === 'true').length;
  // Owner #348 (3), the breaking half, measured rather than asserted. Every line of
  // every visible run of text is found by asking the engine where each character was
  // actually laid out, then two things are counted: a line that begins with a letter
  // whose predecessor is also a letter (a break taken inside a word) and a line whose
  // whole content is one character (the orphan text-wrap:pretty exists to prevent).
  const lineFaults = { midWord: [], orphan: [] };
  // Where a fault is, not only that there is one. Both of the two this reading found
  // when it was first fired turned out to be in shared parts rather than in this face,
  // which is a distinction a bare count cannot make and a lane cannot act on.
  const where = (node) => {
    const bits = [];
    for (let a = node; a && a !== document.body; a = a.parentElement) {
      bits.unshift(a.tagName
        + (a.getAttribute('data-role') ? '[' + a.getAttribute('data-role') + ']' : '')
        + (a.getAttribute('data-cell') ? '{' + a.getAttribute('data-cell') + '}' : ''));
    }
    return bits.slice(-4).join(' > ');
  };
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  const range = document.createRange();
  for (let t = walker.nextNode(); t !== null; t = walker.nextNode()) {
    const owner = t.parentElement;
    if (!owner || !visible(owner)) continue;
    const value = t.textContent;
    if (!value || value.trim().length < 2) continue;
    const starts = [];
    let top = null;
    for (let i = 0; i < value.length; i += 1) {
      range.setStart(t, i);
      range.setEnd(t, i + 1);
      const r = range.getBoundingClientRect();
      if (r.width === 0 && r.height === 0) continue;
      if (top === null || r.top > top + 1) { starts.push(i); top = r.top; }
    }
    if (starts.length < 2) continue;
    for (let s = 1; s < starts.length; s += 1) {
      const i = starts[s];
      if (/[A-Za-z0-9]/.test(value[i - 1]) && /[A-Za-z0-9]/.test(value[i])) {
        lineFaults.midWord.push(where(owner) + ' ' + JSON.stringify(value.slice(Math.max(0, i - 8), i + 8)));
      }
    }
    for (let s = 0; s < starts.length; s += 1) {
      const line = value.slice(starts[s], s + 1 < starts.length ? starts[s + 1] : value.length).trim();
      if (line.length === 1) lineFaults.orphan.push(where(owner) + ' ' + JSON.stringify(value.slice(0, 24)));
    }
  }
  // The r4 report named the box head's shrink order as this face's worst remaining
  // defect. This is that claim as two numbers per head: how much of the name is drawn
  // and how much of the standing pill is, both read off the engine.
  const boxHeads = [...document.querySelectorAll('[data-role="box-head"]')].filter(visible).map((head) => {
    const named = head.querySelector('[data-role="box-name"]');
    const pill = head.querySelector('[data-part="verdict-badge"] [data-role="word"]');
    const cut = (n) => (n === null ? null : { drawn: Math.round(n.clientWidth), wanted: Math.round(n.scrollWidth), text: n.textContent.trim().slice(0, 40) });
    return { name: cut(named), pill: cut(pill) };
  });
  const clippedHeadParts = boxHeads.flatMap((h) => [
    h.name && h.name.wanted > h.name.drawn + 1 ? 'name' : null,
    h.pill && h.pill.wanted > h.pill.drawn + 1 ? 'pill' : null,
  ].filter(Boolean));
  // What one shut subject costs, so the "fewer subjects per window" figure this face
  // wrote down in r4 is re-read rather than re-assumed after the marks grew.
  const shutBoxes = [...document.querySelectorAll('[data-part="box"]')].filter((b) => {
    const inner = b.querySelector('[data-role="subject"]');
    return inner !== null && inner.getAttribute('data-open') !== 'true';
  });
  const subjectPitch = shutBoxes.map((b) => {
    const own = b.getBoundingClientRect();
    return Math.round(own.height + Number.parseFloat(getComputedStyle(b).marginBottom || '0'));
  });
  const weights = [...new Set([...document.querySelectorAll('[data-type]')].filter(visible).map((n) => getComputedStyle(n).fontWeight))].sort();
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
    subjectsDrawn,
    subjectsOpen,
    midWordBreaks: lineFaults.midWord,
    orphanLines: lineFaults.orphan,
    boxHeads,
    clippedHeadParts,
    subjectPitch,
    weights,
    visibleTextChars: pageText.replace(/\\s+/g, ' ').trim().length,
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
      process.stdout.write(`${row.fixture} [${row.scheme} ${row.width}px] overlaps=${row.overlapCount} rows=${row.rowsDrawn} repeated=${row.repeatedRows.length} glyphs=${row.glyphs} oversize=${row.oversizeGlyphs.length} filled=${row.filledGlyphs.length} sprites=${row.sprites} overflow=${row.horizontalOverflow} clipped=${row.clippedCells} clippedWithoutFull=${row.clippedWithoutFull.length} subjectsDrawn=${row.subjectsDrawn} subjectsOpen=${row.subjectsOpen} controls=${row.interactiveControls} underTapBudget=${row.underTapBudget.length} textChars=${row.visibleTextChars} bg=${row.background}\n`);
      process.stdout.write(`    midWord=${row.midWordBreaks.length} orphan=${row.orphanLines.length} weights=${row.weights.join('/')} shutSubjectPx=${row.subjectPitch.join(',') || '-'} clippedHeadParts=${row.clippedHeadParts.join(',') || 'none'}\n`);
      for (const w of row.midWordBreaks.slice(0, 4)) process.stdout.write(`    mid-word break: ${JSON.stringify(w)}\n`);
      for (const head of row.boxHeads) {
        if (head.name && head.name.wanted > head.name.drawn + 1) process.stdout.write(`    head name cut: ${JSON.stringify(head.name.text)} drawn ${head.name.drawn} of ${head.name.wanted}\n`);
        if (head.pill && head.pill.wanted > head.pill.drawn + 1) process.stdout.write(`    head pill cut: ${JSON.stringify(head.pill.text)} drawn ${head.pill.drawn} of ${head.pill.wanted}\n`);
      }
      for (const o of row.overlaps) process.stdout.write(`    overlap ${o.area}px2: ${JSON.stringify(o.a)} / ${JSON.stringify(o.b)}\n`);
      for (const g of row.oversizeGlyphs) process.stdout.write(`    glyph ${g.mark} asked ${g.asked} drew ${g.w}x${g.h}\n`);
      for (const t of row.underTapBudget) process.stdout.write(`    under tap budget: <${t.tag}> "${t.text}" ${t.w}x${t.h}\n`);
    }
  }).catch((error) => {
    process.stderr.write(`${SHOOT_MESSAGES.NO_RENDERER}: ${error.message}\n`);
    process.exitCode = 1;
  });
}
