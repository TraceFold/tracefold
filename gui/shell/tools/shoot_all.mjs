// SPDX-License-Identifier: Apache-2.0
// Every face and every chrome state of the shipped window, captured for a critique round.
//
//   node tools/shoot_all.mjs --origin http://127.0.0.1:8807 --tag c1
//
// req/822's cycle protocol asks for the shot list to come from the window's own registry
// rather than from memory, because a face nobody remembered is exactly the face nobody
// looked at. So the tab strip and the palette's own rows are read out of the live document
// and the list is whatever they hold, not a literal here. (req/867: it was the tab strip
// and the launcher column; the column is gone and the palette carries what was unique to it.)
//
// Alongside each capture the few facts that say what was in front of the lens are printed
// and written to a sidecar JSON: a screenshot alone cannot say whether the rows on it came
// from an engine or from a stand-in, and those two windows draw nearly the same shape.

import { writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { startRenderer } from '../../tools/rig/renderer.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));

const argOf = (name, fallback) => {
  const at = process.argv.indexOf(name);
  return at === -1 ? fallback : process.argv[at + 1];
};

const ORIGIN = argOf('--origin', 'http://127.0.0.1:8807');
const TAG = argOf('--tag', 'c1');
const OUT = join(HERE, '..', 'record', `critique_${TAG}`);

mkdirSync(OUT, { recursive: true });

const shots = [];

/** The facts a capture cannot state about itself. */
const FACTS = `JSON.stringify({
  theme: document.documentElement.getAttribute('data-theme'),
  bound: document.documentElement.dataset.bound,
  realFaces: document.documentElement.dataset.realFaces,
  tabs: [...document.querySelectorAll('.tab-label')].map((t) => t.textContent.trim()),
  standing: [...document.querySelectorAll('.palette-row[data-places="true"]')].length,
  hosts: [...document.querySelectorAll('[data-host]')].map((h) => ({
    id: h.getAttribute('data-host'),
    face: h.querySelector('[data-face]')?.getAttribute('data-face') ?? null,
    characters: (h.innerText || '').replace(/\\s+/g, '').length,
    elements: h.querySelectorAll('*').length,
  })),
  paletteOpen: document.querySelector('.palette') ? document.querySelector('.palette').hidden === false : false,
  menuOpen: document.querySelector('.chrome-menu') ? document.querySelector('.chrome-menu').hidden === false : false,
  scrollHeight: document.documentElement.scrollHeight,
  innerHeight: window.innerHeight,
  offscreen: [...document.querySelectorAll('button,[role="button"],summary,a[href],input,select')]
    .filter((el) => { const b = el.getBoundingClientRect(); return b.height > 0 && (b.bottom > window.innerHeight || b.right > window.innerWidth); })
    .map((el) => ((el.getAttribute('aria-label') || el.textContent || '?').trim().slice(0, 30) + '@' + Math.round(el.getBoundingClientRect().top))),
})`;

async function shoot(page, name, note) {
  await page.settle();
  const facts = await page.evaluate(FACTS);
  const at = join(OUT, `${name}.png`);
  writeFileSync(at, await page.capture());
  shots.push({ name, at, note, facts: JSON.parse(facts) });
  console.log(`${name}  ${note}`);
}

const run = async ({ width, height, theme, sizeTag }) => {
  const renderer = await startRenderer({ viewport: { width, height } });
  const page = await renderer.openPage();
  const open = async () => {
    await page.open(`${ORIGIN}/app.html?theme=${theme}`);
    await page.hold('document.documentElement.dataset.bound !== undefined');
    await page.hold("document.querySelector('.strip-said') !== null");
    await page.settle();
  };
  const key = `${sizeTag}_${theme}`;

  await open();
  await shoot(page, `${key}_00_default`, `the window as it opens, ${width}x${height}, ${theme}`);

  // Every tab the strip itself holds. Read from the strip, never listed here.
  const tabs = JSON.parse(await page.evaluate(
    `JSON.stringify([...document.querySelectorAll('.tab')].map((t, i) => ({ i, label: (t.querySelector('.tab-label')?.textContent ?? '?').trim() })))`,
  ));
  for (const tab of tabs) {
    await open();
    await page.evaluate(`document.querySelectorAll('.tab')[${tab.i}].click()`);
    await page.settle();
    await shoot(page, `${key}_10_tab_${tab.i}_${tab.label.toLowerCase().replace(/[^a-z0-9]+/g, '')}`, `stage tab "${tab.label}"`);
  }

  // req/867: the standing column is gone, so the shot list for "a face being placed" comes
  // from the palette's own nowhere rows instead. Still read out of the live document and
  // never listed here, for the reason at the top of this file.
  await open();
  await page.evaluate(`document.querySelector('.bar-find .chrome-act')?.click()`);
  await page.evaluate(`(() => { const f = document.querySelector('.palette-field'); if (f) { f.value = 'box:nowhere'; f.dispatchEvent(new Event('input', { bubbles: true })); } return true; })()`);
  await page.settle();
  const stood = JSON.parse(await page.evaluate(
    `JSON.stringify([...document.querySelectorAll('.palette-row[data-places="true"]')].map((t, i) => ({ i, label: (t.querySelector('.palette-row-name')?.textContent ?? '?').trim().slice(0, 24) })))`,
  ));
  await shoot(page, `${key}_20_stand_list`, `the palette's nowhere rows -- ${stood.length} face(s) placeable from here`);
  for (const row of stood) {
    await open();
    await page.evaluate(`document.querySelector('.bar-find .chrome-act')?.click()`);
    await page.evaluate(`(() => { const f = document.querySelector('.palette-field'); if (f) { f.value = 'box:nowhere'; f.dispatchEvent(new Event('input', { bubbles: true })); } return true; })()`);
    await page.settle();
    await page.evaluate(`document.querySelectorAll('.palette-row[data-places="true"]')[${row.i}]?.click()`);
    await page.settle();
    await shoot(page, `${key}_20_stand_${row.i}_${row.label.toLowerCase().replace(/[^a-z0-9]+/g, '').slice(0, 14)}`, `placed "${row.label}" from the palette`);
  }

  // The palette, invoked the way a person invokes it -- `mod+p`, which is what
  // `kernel/keys.mjs` binds `palette:open` to. The first draft of this file pressed
  // `mod+k`, and the three palette captures it produced showed no palette and came back
  // in the OPPOSITE theme to the one their filename claimed, because `mod+k` is bound to
  // `theme:set`. That was an instrument defect and it is recorded as one -- but the reason
  // it was made is a finding about the product and is filed as one: `mod+k` is the chord
  // this decade puts a command palette on, and pressing it here silently restyles the
  // whole window instead.
  await open();
  await page.raw.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'p', code: 'KeyP', modifiers: 2, text: 'p' });
  await page.raw.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'p', code: 'KeyP', modifiers: 2 });
  await page.settle();
  const paletteUp = await page.evaluate(`document.querySelector('.palette') ? document.querySelector('.palette').hidden === false : false`);
  if (!paletteUp) console.log(`!! ${key}: the palette chord was sent and no palette opened -- the three shots below are of a window without one`);
  await shoot(page, `${key}_30_palette_open`, `the palette, invoked by its chord -- ${paletteUp ? 'OPEN' : 'DID NOT OPEN'}`);

  await page.evaluate(`(() => { const f = document.querySelector('.palette-field'); if (f) { f.value = 'box:'; f.dispatchEvent(new Event('input', { bubbles: true })); } return true; })()`);
  await page.settle();
  await shoot(page, `${key}_31_palette_faceted`, 'the palette with a facet typed');

  await page.evaluate(`(() => { const f = document.querySelector('.palette-field'); if (f) { f.value = 'zzzznothing'; f.dispatchEvent(new Event('input', { bubbles: true })); } return true; })()`);
  await page.settle();
  await shoot(page, `${key}_32_palette_empty`, 'the palette asked something it does not hold');

  // The chrome menus, opened with a real right-click on each surface that offers one.
  // The element that actually carries the listener, which is not always the region the
  // menu is named after. The first draft asked `.dock` and `.sash`, and both came back
  // with no menu: the dock's menu is bound to `.dock-name` (the face tab inside the dock
  // bar), and `document.querySelector('.sash')` is the LEFT sash, which is `display:none`
  // because the left dock is shut -- so that right-click was dispatched at 0,0, onto the
  // sidebar. Two "the product does not answer a right-click" findings came out of that,
  // and both were about this file. Named with the listener's own selector now.
  const surfaces = [
    ['strip', '.strip'],
    ['tab', '.tab'],
    ['dock', '.dock-right .dock-name'],
    // req/867: `standing` is not in this list any more. The menu target still exists in
    // kernel/menu.mjs and is still unit-tested there, but the launcher rows were its only
    // opener, so there is no surface to right-click. Named here as a known gap rather
    // than dropped silently -- an unreachable menu is a finding, and it is filed in
    // req/867 as one rather than papered over by deleting the line.
    ['sash', '.sash-bottom'],
  ];
  for (const [named, selector] of surfaces) {
    await open();
    const at = JSON.parse(await page.evaluate(`(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return JSON.stringify({ none: 'no such element' });
      const b = el.getBoundingClientRect();
      // A zero-sized target is not a target. Dispatching at its "centre" sends the click
      // to 0,0 and captures a window that was never right-clicked where it was claimed.
      if (b.width < 1 || b.height < 1) return JSON.stringify({ none: 'drawn at ' + Math.round(b.width) + 'x' + Math.round(b.height) + ', so no click was sent' });
      return JSON.stringify({ x: Math.round(b.left + b.width / 2), y: Math.round(b.top + b.height / 2) });
    })()`));
    if (at.none) { console.log(`!! ${key} ${named}: ${at.none} -- NOT CAPTURED`); continue; }
    for (const type of ['mousePressed', 'mouseReleased']) {
      await page.raw.send('Input.dispatchMouseEvent', { type, x: at.x, y: at.y, button: 'right', buttons: 2, clickCount: 1 });
    }
    await page.settle();
    // Whether the menu opened is asked, never assumed: a capture of a window with no menu
    // on it is evidence about this file until this line says otherwise.
    const opened = await page.evaluate(`document.querySelector('.chrome-menu') ? document.querySelector('.chrome-menu').hidden === false : false`);
    await shoot(page, `${key}_40_menu_${named}`, `right-click on the ${named} at ${at.x},${at.y} -- menu ${opened ? 'OPENED' : 'DID NOT OPEN'}`);
  }

  // The bottom of the document, which at 900px tall is where the acts were found to live.
  await open();
  await page.evaluate('window.scrollTo(0, document.documentElement.scrollHeight)');
  await page.settle();
  await shoot(page, `${key}_50_scrolled_bottom`, 'the same window scrolled to the end of the document');

  await renderer.stop();
};

for (const theme of ['light', 'dark']) {
  await run({ width: 1440, height: 900, theme, sizeTag: '1440' });
}
await run({ width: 1100, height: 800, theme: 'light', sizeTag: '1100' });

writeFileSync(join(OUT, 'shots.json'), `${JSON.stringify(shots, null, 2)}\n`);
console.log(`\n${shots.length} captures in ${OUT}`);
