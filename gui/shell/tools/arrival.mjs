// SPDX-License-Identifier: Apache-2.0
// req/884 (Owner: 標準で画面分割はなし). What a first-time reader meets.
//
//   node shell/tools/arrival.mjs --origin http://127.0.0.1:8893
//
// Measures the real window at /app.html, at the width it is read at, and reports the
// three numbers the ruling is actually about: how many content regions stand, how many
// controls are on screen, and how much of the viewport belongs to the thing the reader
// came for.
//
// It measures BOTH states off one build. Arrival is what openingState now gives. The
// "docks open" row is the OLD default reproduced by pressing the two dock presses --
// which is also the proof the default change removed no capability: if the old arrangement
// could not be reached again, this tool could not produce that row.

import { startRenderer } from '../../tools/rig/renderer.mjs';

const argOf = (name, fallback) => {
  const at = process.argv.indexOf(name);
  return at === -1 ? fallback : process.argv[at + 1];
};

const ORIGIN = argOf('--origin', 'http://127.0.0.1:8893');
const WIDTH = Number(argOf('--width', '1440'));
const HEIGHT = Number(argOf('--height', '900'));

const MEASURE = `(() => {
  const box = (sel) => {
    const n = document.querySelector(sel);
    if (!n) return null;
    const r = n.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) return null;
    return { w: Math.round(r.width), h: Math.round(r.height), area: Math.round(r.width * r.height) };
  };
  const vw = window.innerWidth, vh = window.innerHeight;
  const stage = box('.stage');
  const regions = ['.stage', 'aside.dock-left', 'aside.dock-right', 'aside.dock-bottom']
    .filter((s) => box(s) !== null);
  const visible = (n) => {
    const r = n.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  };
  return JSON.stringify({
    viewport: { w: vw, h: vh, area: vw * vh },
    contentRegions: regions.length,
    regionNames: regions,
    stage,
    stageShareOfViewport: stage ? Math.round((stage.area / (vw * vh)) * 1000) / 10 : null,
    controlsOnScreen: [...document.querySelectorAll('button, [role="button"], input, summary')].filter(visible).length,
    tabs: document.querySelectorAll('.tab').length,
    dockPresses: document.querySelectorAll('.chrome-docks .chrome-act').length,
  });
})()`;

const renderer = await startRenderer({ viewport: { width: WIDTH, height: HEIGHT } });
const page = await renderer.openPage();
await page.open(`${ORIGIN}/app.html?theme=light`);
await page.hold('document.documentElement.dataset.bound !== undefined');
await page.hold("document.querySelector('.strip-said') !== null");
await page.settle();

const arrival = JSON.parse(await page.evaluate(MEASURE));

// Reproduce the old default by pressing the presses -- the capability proof.
await page.evaluate(`(() => {
  for (const b of document.querySelectorAll('.chrome-docks .chrome-act')) {
    if (b.getAttribute('aria-pressed') === 'false') b.click();
  }
  return 'ok';
})()`);
await page.settle();
const opened = JSON.parse(await page.evaluate(MEASURE));

console.log(`# arrival at ${WIDTH}x${HEIGHT}, ${ORIGIN}/app.html\n`);
const row = (name, m) => console.log(
  `${name.padEnd(22)} regions=${m.contentRegions}  controls=${String(m.controlsOnScreen).padStart(3)}`
  + `  stage=${m.stage ? `${m.stage.w}x${m.stage.h}` : '--'}`
  + `  stage share=${m.stageShareOfViewport}%  tabs=${m.tabs}`,
);
row('arrival (shipped)', arrival);
row('docks opened again', opened);
console.log(`\nregions on arrival: ${arrival.regionNames.join(', ')}`);
console.log(`regions when opened: ${opened.regionNames.join(', ')}`);
console.log(`dock presses on the bar: ${arrival.dockPresses}`);
console.log(`\n${JSON.stringify({ arrival, opened }, null, 2)}`);

await page.close();
await renderer.stop();
