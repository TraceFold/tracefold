// SPDX-License-Identifier: Apache-2.0
// The shipped window, captured in both inks, at the width it is read at.
//
//   node tools/shoot_window.mjs --origin http://127.0.0.1:8807 --tag r6
//
// A capture is not evidence on its own -- req/97 Pass 4 records that every pre-r4 image
// of a face was of a CSP-free fixture, which is to say a strictly better surface than the
// one that shipped. So this shoots the REAL window off a real origin with the real policy
// in force, and prints alongside each file the few facts that say what was in front of the
// lens: whether the membrane was bound, how many stage tabs stood, and the title census.

import { writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { startRenderer } from '../../tools/rig/renderer.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const RECORD = join(HERE, '..', 'record');

const argOf = (name, fallback) => {
  const at = process.argv.indexOf(name);
  return at === -1 ? fallback : process.argv[at + 1];
};

const ORIGIN = argOf('--origin', 'http://127.0.0.1:8807');
const TAG = argOf('--tag', 'r6');
const WIDTH = Number(argOf('--width', '1440'));
const HEIGHT = Number(argOf('--height', '900'));

mkdirSync(RECORD, { recursive: true });

const renderer = await startRenderer({ viewport: { width: WIDTH, height: HEIGHT } });
const written = [];

for (const theme of ['light', 'dark']) {
  const page = await renderer.openPage();
  await page.open(`${ORIGIN}/app.html?theme=${theme}`);
  await page.hold('document.documentElement.dataset.bound !== undefined');
  // The faces ask the engine after they mount, so a capture taken at mount is a capture
  // of a window that has not been answered yet. Waited on as a condition, never a sleep.
  await page.hold("document.querySelector('.strip-said') !== null");
  await page.settle();
  const facts = await page.evaluate(`JSON.stringify({
    theme: document.documentElement.getAttribute('data-theme'),
    bound: document.documentElement.dataset.bound,
    tabs: document.querySelectorAll('.tab').length,
    // req/867: the standing column is gone. 'pressedInColumn' existed to catch req/811
    // section 8-2b (all six rows reporting aria-pressed at once); the population it
    // counted no longer exists, so it counts the rail, where a genuine single selection
    // lives and where the same "more than one thing claims to be selected" defect shows.
    //
    // req/884: those four lines carried BACKTICKS around the old name, inside a template
    // literal. A backtick in a comment still closes the string that comment sits in, so
    // this file has been a SyntaxError since cb3ddb0 -- the very commit req/867 section 6
    // credits with "before/after shots". The shots in record/req867_sidebar are real and
    // were taken by another tool; this instrument simply never ran again after that edit,
    // and nothing in the battery imports it, so nothing went red. Recorded rather than
    // silently repaired: an instrument that cannot parse is not a passing instrument, and
    // a battery that would not notice is the finding worth keeping.
    standing: document.querySelectorAll('.palette-row[data-places="true"]').length,
    pressedInRail: document.querySelectorAll('.rail-item[aria-pressed="true"]').length,
    title: document.title,
  })`);
  const at = join(RECORD, `${TAG}_app_${theme}.png`);
  writeFileSync(at, await page.capture());
  written.push({ at, facts });
  console.log(`${at}\n  ${facts}`);
  await page.close();
}

await renderer.stop();
console.log(`\n${written.length} captures of ${ORIGIN}/app.html at ${WIDTH}x${HEIGHT}`);
