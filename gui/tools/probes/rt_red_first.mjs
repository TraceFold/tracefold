// SPDX-License-Identifier: Apache-2.0
// KA-I2: the pixel tier is not allowed to be believed until the defects it exists
// for make it go red. req/06 §4-3 -- the fixtures come before the faces.
//
// Read the last column, not the first: a detector that fails the clean baseline as
// well as the three regressions has found nothing. Both directions are printed.
//
//   node tools/probes/rt_red_first.mjs

import path from 'node:path';
import url from 'node:url';
import { startRenderer } from '../rig/renderer.mjs';
import { inkBoxOverlap, usedGlyphSize, baselineInk, captureInk } from '../tiers/pixel.mjs';

const here = path.dirname(url.fileURLToPath(import.meta.url));
const fixtureUrl = (name) => url.pathToFileURL(path.join(here, '..', 'fixtures', name)).href;
const VIEWPORT = { width: 720, height: 320 };

// what each fixture is a live round for, and what each detector is expected to say
const PLAN = [
  { file: 'clean-baseline.html', expect: { 'P-a': 'pass', 'P-b': 'pass', 'P-c': 'pass' }, why: 'positive control' },
  { file: 'rt07-note-overlap.html', expect: { 'P-a': 'fail', 'P-b': 'pass', 'P-c': 'fail' }, why: 'RT-07 margin note over row text' },
  { file: 'rt08-unsized-glyph.html', expect: { 'P-a': 'fail', 'P-b': 'fail', 'P-c': 'fail' }, why: 'RT-08 glyph on the replaced default' },
  { file: 'rt08-double-draw.html', expect: { 'P-a': 'fail', 'P-b': 'pass', 'P-c': 'fail' }, why: 'RT-08 sheet painted twice' },
];

const renderer = await startRenderer({ viewport: VIEWPORT });
console.log(`renderer : ${renderer.product}`);

// The baseline is taken from the clean fixture and every fixture is measured
// against it, because that is the regression the face-level tier will run.
let baseline;
{
  const page = await renderer.openPage();
  await page.open(fixtureUrl('clean-baseline.html'));
  baseline = await captureInk(page);
  await page.close();
  console.log(`baseline : ${baseline.digest}  ${baseline.width}x${baseline.height}  ink ${baseline.inkPixels}px\n`);
}

const rows = [];
for (const entry of PLAN) {
  const page = await renderer.openPage();
  await page.open(fixtureUrl(entry.file));
  const readings = [await inkBoxOverlap(page), await usedGlyphSize(page), await baselineInk(page, { baseline })];
  await page.close();

  console.log(`${entry.file}   (${entry.why})`);
  for (const reading of readings) {
    const want = entry.expect[reading.id];
    const got = reading.ok ? 'pass' : 'fail';
    const agrees = want === got;
    rows.push({ fixture: entry.file, detector: reading.id, want, got, agrees });
    console.log(`  ${reading.id}  want ${want.padEnd(4)} got ${got.padEnd(4)} ${agrees ? 'ok ' : 'MISMATCH'}  n=${reading.population}  ${reading.message}`);
    for (const finding of reading.findings.slice(0, 3)) console.log(`        - ${JSON.stringify(finding)}`);
  }
  console.log('');
}

const mismatches = rows.filter((r) => !r.agrees);
const reds = rows.filter((r) => r.got === 'fail').length;
const greens = rows.filter((r) => r.got === 'pass').length;
console.log(`readings : ${rows.length}   red ${reds}   green ${greens}   disagreeing with the plan ${mismatches.length}`);
for (const m of mismatches) console.log(`  MISMATCH ${m.fixture} ${m.detector}: wanted ${m.want}, got ${m.got}`);
console.log(mismatches.length === 0
  ? 'KA-I2 : the three regressions go red and the clean control does not -- the pixel tier has live rounds'
  : 'KA-I2 : not yet -- a detector disagrees with what the fixture was built to be');

await renderer.stop();
process.exitCode = mismatches.length === 0 ? 0 : 1;
