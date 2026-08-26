// SPDX-License-Identifier: Apache-2.0
// KA-I1, measured rather than assumed: is a capture of the same page the same
// capture? req/06 §1-2 says answer this before a single assay is written, because
// if the answer is no at every level of normalisation then the pixel tier is a
// quarantine factory and §466(2) is unreachable.
//
// Three levels, because they are three different claims:
//   A  one page, captured five times      -- is the compositor steady
//   B  one renderer, five fresh loads     -- is layout+paint steady across a load
//   C  three renderers, one load each     -- is a run comparable to another run
// Level C is the one that matters: an independent re-run restarts the renderer.
//
//   node tools/probes/pixel_determinism.mjs [fixture.html]

import path from 'node:path';
import url from 'node:url';
import { startRenderer } from '../rig/renderer.mjs';
import { decodePng, inkMask, maskDigest, comparePixels, compareMasks, digestOf } from '../rig/raster.mjs';

const here = path.dirname(url.fileURLToPath(import.meta.url));
const fixture = process.argv[2] ?? path.join(here, '..', 'fixtures', 'clean-baseline.html');
const target = url.pathToFileURL(path.resolve(fixture)).href;
const VIEWPORT = { width: 720, height: 320 };

const summarise = (label, shots) => {
  const byteDigests = new Set(shots.map((s) => s.byteDigest));
  const maskDigests = new Set(shots.map((s) => s.maskDigest));
  const first = shots[0];
  let worstPixels = 0;
  let worstDelta = 0;
  let worstMask = 0;
  for (const shot of shots.slice(1)) {
    const px = comparePixels(first.raster, shot.raster);
    const mk = compareMasks(first.ink, shot.ink);
    worstPixels = Math.max(worstPixels, px.differingPixels);
    worstDelta = Math.max(worstDelta, px.maxChannelDelta);
    worstMask = Math.max(worstMask, mk.differing);
  }
  const total = first.raster.width * first.raster.height;
  console.log(`\n${label}  (n=${shots.length}, ${first.raster.width}x${first.raster.height} = ${total} px, ink ${first.ink.inkPixels})`);
  console.log(`  distinct png byte digests   : ${byteDigests.size}  ${byteDigests.size === 1 ? '(identical bytes)' : '(bytes move)'}`);
  console.log(`  distinct ink mask digests   : ${maskDigests.size}  ${maskDigests.size === 1 ? '(identical ink)' : '(ink moves)'}`);
  console.log(`  worst pixel diff vs first   : ${worstPixels} px (${(100 * worstPixels / total).toFixed(4)}%), max channel delta ${worstDelta}`);
  console.log(`  worst ink diff vs first     : ${worstMask} px (${(100 * worstMask / total).toFixed(4)}%)`);
  return { label, bytesStable: byteDigests.size === 1, inkStable: maskDigests.size === 1, worstPixels, worstDelta, worstMask, total };
};

async function shoot(page) {
  const png = await page.capture();
  const raster = decodePng(png);
  const ink = inkMask(raster);
  return { byteDigest: digestOf(png), raster, ink, maskDigest: maskDigest(ink) };
}

console.log(`fixture : ${target}`);
const results = [];

// Level A and B share one renderer.
{
  const renderer = await startRenderer({ viewport: VIEWPORT });
  console.log(`renderer: ${renderer.product}`);
  const page = await renderer.openPage();
  await page.open(target);
  const levelA = [];
  for (let i = 0; i < 5; i += 1) levelA.push(await shoot(page));
  results.push(summarise('A  one page, five captures', levelA));
  await page.close();

  const levelB = [];
  for (let i = 0; i < 5; i += 1) {
    const fresh = await renderer.openPage();
    await fresh.open(target);
    levelB.push(await shoot(fresh));
    await fresh.close();
  }
  results.push(summarise('B  one renderer, five fresh loads', levelB));
  await renderer.stop();
}

// Level C restarts the process, which is what an independent re-run does.
{
  const levelC = [];
  for (let i = 0; i < 3; i += 1) {
    const renderer = await startRenderer({ viewport: VIEWPORT });
    const page = await renderer.openPage();
    await page.open(target);
    levelC.push(await shoot(page));
    await page.close();
    await renderer.stop();
  }
  results.push(summarise('C  three renderers, one load each', levelC));
}

console.log('\nverdict');
const bytesEverywhere = results.every((r) => r.bytesStable);
const inkEverywhere = results.every((r) => r.inkStable);
for (const r of results) console.log(`  ${r.label.padEnd(36)} bytes ${r.bytesStable ? 'stable' : 'MOVE'}   ink ${r.inkStable ? 'stable' : 'MOVE'}`);
if (bytesEverywhere) console.log('  KA-I1 : byte comparison is enough at every level');
else if (inkEverywhere) console.log('  KA-I1 : bytes move, binarised ink is stable -- the pixel tier compares ink masks');
else console.log('  KA-I1 : ink moves too -- a tolerance has to be declared, and its size is the number above');
process.exitCode = inkEverywhere || bytesEverywhere ? 0 : 1;
