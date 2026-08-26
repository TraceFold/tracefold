// SPDX-License-Identifier: Apache-2.0
// The window against a real engine, asserted rather than admired.
//
//   node tools/serve.mjs --port 8807 --bed http://127.0.0.1:8795 --bed-token <hex>
//   node tools/bound_smoke.mjs --origin http://127.0.0.1:8807
//
// req/803's C3 -- "wired to a REAL backend" -- was 0/16 for every surface, and the
// reason it stayed 0 for so long is that nothing could tell the difference from the
// outside: a face that has not been asked and a face that was asked and answered draw
// almost the same shape, and both look fine in a screenshot. So this file does not read
// the screen for reassurance. It reads three things that cannot be faked by drawing:
//
//   1. what the membrane's own notice ledger says left this window (method, outcome,
//      status) -- the membrane writes one row per call and invents none;
//   2. whether an `answered` outcome carries the engine's own bytes;
//   3. the console, drained per face, because a face that throws still paints.
//
// The negative control is the point of the whole file: the same run is made against the
// same window with no bed named, and it must FAIL the real-data assertion. A check never
// seen failing is not a check.

import { startRenderer } from '../../tools/rig/renderer.mjs';

const argOf = (name, fallback) => {
  const at = process.argv.indexOf(name);
  return at === -1 ? fallback : process.argv[at + 1];
};

const ORIGIN = argOf('--origin', 'http://127.0.0.1:8807');
const EXPECT = argOf('--expect', 'bound'); // 'bound' | 'unbound'

export const SMOKE_MESSAGES = {
  NOT_BOUND: 'the window did not bind the membrane, so nothing below was asked of an engine',
  NO_CALL: 'the membrane ledger is empty: this window made no call at all',
  NO_ANSWER: 'every call this window made came back without the engine answering one of them',
  CONSOLE: 'the window logged an error while being driven',
};

const results = [];
const check = (id, ok, detail) => {
  results.push({ id, ok: Boolean(ok), detail });
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${id}  ${detail}`);
};

const renderer = await startRenderer({ viewport: { width: 1440, height: 900 } });
const page = await renderer.openPage();

const errors = [];
page.raw.on('Runtime.consoleAPICalled', (event) => {
  if (event.type === 'error') errors.push((event.args ?? []).map((a) => a.value ?? a.description).join(' '));
});
page.raw.on('Runtime.exceptionThrown', (event) => {
  errors.push(event.exceptionDetails?.exception?.description ?? 'uncaught');
});
await page.raw.send('Runtime.enable');

await page.open(`${ORIGIN}/app.html`);
// A condition, never a duration (req/06 §3-3): the boot writes this the moment it has
// decided which port this window carries, so waiting on it waits on the decision itself.
await page.hold('document.documentElement.dataset.bound !== undefined');

const readValue = async (expression) => {
  const got = await page.raw.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (got.exceptionDetails) throw new Error(`${expression}: ${got.exceptionDetails.text}`);
  return got.result?.value;
};

/**
 * Every face reachable from the tab strip is visited, because a face nobody opened is a
 * face nobody measured -- and after req/811's default-tab ruling the stage opens on one,
 * so five of the six would otherwise never be asked anything at all.
 */
const visitAll = `(async () => {
  const seen = [];
  for (const tab of [...document.querySelectorAll('.tab')]) {
    tab.click();
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    seen.push(tab.querySelector('.tab-label')?.textContent ?? '?');
  }
  // req/867: the standing column is gone, and with it the list this loop used to walk to
  // reach the faces the stage did not open on. The capability did not go with it -- it
  // moved into the palette -- so this walks the palette instead, which is now the one
  // place a nowhere-standing face can be placed from. Same coverage, one indirection more.
  //
  // Reopened per row on purpose: placing a face closes the palette (that is what the
  // control does when a person uses it), and holding a stale row list across a redraw is
  // how an instrument ends up clicking a node the document has already replaced.
  const openFind = () => document.querySelector('.bar-find .chrome-act')?.click();
  for (let round = 0; round < 12; round += 1) {
    openFind();
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    const field = document.querySelector('.palette-field');
    if (!field) break;
    field.value = 'box:nowhere';
    field.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    const row = document.querySelector('.palette-row[data-places="true"]');
    if (!row) { document.querySelector('.palette')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); break; }
    row.click();
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  }
  await new Promise((r) => setTimeout(r, 1200));
  return seen;
})()`;

const visited = await readValue(visitAll);

const bound = await readValue('window.gxWired?.bound === true');
const ledger = await readValue('JSON.stringify((window.gxWired?.notices ?? []).map((n) => ({ m: n.method, o: n.outcome, s: n.status })))');
const calls = JSON.parse(ledger ?? '[]');
const byOutcome = calls.reduce((into, row) => ({ ...into, [row.o]: (into[row.o] ?? 0) + 1 }), {});
const answered = calls.filter((row) => row.o === 'answered');

check('B1 the window bound the membrane', bound === (EXPECT === 'bound'),
  `data bound=${bound}, expected ${EXPECT}; visited ${JSON.stringify(visited)}`);

check('B2 calls left this window', calls.length > 0 || EXPECT === 'unbound',
  `${calls.length} call(s) in the membrane's own ledger: ${JSON.stringify(byOutcome)}`);

check('B3 the engine answered at least one of them', (answered.length > 0) === (EXPECT === 'bound'),
  `${answered.length} answered: ${answered.slice(0, 6).map((r) => `${r.m}=${r.s}`).join(', ') || 'none'}`);

// The row's own bytes, taken from the port rather than from the screen: a face may choose
// not to draw a field, and that is a drawing decision, not evidence about the wire.
const sample = await readValue(`(async () => {
  const port = window.gxWired?.port ?? null;
  if (!port || typeof port.get_transformations !== 'function') return JSON.stringify({ none: 'no real port on this window' });
  const got = await port.get_transformations({});
  return JSON.stringify({ outcome: got.outcome, status: got.status ?? null, first: got.body?.items?.[0] ?? null });
})()`);
const wire = JSON.parse(sample ?? '{}');
const carriesEngineRow = typeof wire.first?.transformation === 'string' && wire.first.transformation.startsWith('gx1:');
check('B4 an answered read carries the engine\'s own row', carriesEngineRow === (EXPECT === 'bound'),
  `outcome=${wire.outcome ?? wire.none} first=${wire.first ? wire.first.transformation : 'none'}`);

check('B5 no console error while every face was opened', errors.length === 0,
  errors.length ? `${errors.length}: ${errors.slice(0, 3).join(' | ')}` : 'none');

await renderer.stop();

const failed = results.filter((r) => !r.ok);
console.log(`\n${results.length - failed.length}/${results.length} checks passed against ${ORIGIN} (expecting ${EXPECT})`);
process.exitCode = failed.length === 0 ? 0 : 1;
