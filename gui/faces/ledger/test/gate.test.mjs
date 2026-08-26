// SPDX-License-Identifier: Apache-2.0
// The gates, run against the shipped source, and then run against source written to
// break them.
//
// A gate that has never gone red is a gate nobody has seen work. Every check below is
// fired twice: once at the face, where it must hold, and once at a planted string,
// where it must not.

import test from 'node:test';
import assert from 'node:assert/strict';

import { CHECKS, checkSource, report, shippedSources } from '../tools/gate.mjs';
import { DECLARATION } from '../declaration.mjs';
import { face } from '../ledger.mjs';
import { stubPort, page, answered, SAMPLE } from './stub-port.mjs';

const port = () => stubPort({
  get_transformations: page([SAMPLE.transformation(1), SAMPLE.transformation(2)]),
  get_candidates: page([SAMPLE.candidate(3)]),
  get_ledger_consistency: answered({ consistent: true, checked_from: 1, checked_to: 2 }),
}, { methods: DECLARATION.consumes });

test('the shipped face passes every source gate', async () => {
  const state = await face.read(port());
  const result = report({ trees: [face.view(state)] });
  const failing = result.checks.filter((c) => !c.holds);
  assert.deepEqual(failing.map((c) => `${c.id}: ${c.detail}`), []);
  assert.ok(result.checks.length >= 12, 'too few checks to call this a gate');
});

test('the gate reads a non-empty set of shipped files', () => {
  const sources = shippedSources();
  assert.ok(sources.length >= 4);
  for (const source of sources) assert.ok(source.text.length > 200, `${source.file} is suspiciously small`);
});

const PLANTS = [
  ['no-network', "const r = await fetch('http://127.0.0.1:8787/v1/candidates');"],
  ['no-foreign-import', "import { createMembrane } from '../../membrane/src/index.mjs';"],
  ['no-verification', 'const ok = verify(record.digest, record.anchor);'],
  ['no-actor-named', "const body = { actor: { Human: { id: 'me' } } };"],
  ['no-colour-literal', "const ink = '#1a1a1a';"],
  ['no-borrowed-symbol', "const bullet = '●';"],
  ['nothing-out-of-flow', "const s = 'position:absolute;left:0';"],
  ['no-method-literals-outside-the-declaration', "await port.fold('get_transformations');"],
  ['no-dynamic-code', 'const f = new Function("return 1");'],
  ['rows-are-not-edited', 'record.verdict = "Admit";'],
  // Owner #348 (5). Each of these is the shape the face had, or nearly had, before the
  // number it spells was given a name to be asked for instead.
  ['no-raw-motion', "style({ transition: 'background 120ms ease' })"],
  ['no-raw-motion', "style({ animation: 'fade 180 ms' })"],
  ['no-raw-corner', "style({ 'border-radius': '4px' })"],
  ['no-raw-corner', 'const css = `.thing{border-radius:8px}`;'],
  ['no-mid-word-breaking', "style({ 'overflow-wrap': 'anywhere' })"],
];

for (const [id, planted] of PLANTS) {
  test(`negative control: ${id} goes red on planted source -- ${planted.slice(0, 34)}`, () => {
    const check = CHECKS.find((c) => c.id === id);
    assert.ok(check, `no such check: ${id}`);
    const result = checkSource(check, [{ file: 'planted.mjs', text: planted }]);
    assert.equal(result.holds, false, `${id} did not notice: ${planted}`);
  });
}

test('negative control: an undeclared mark on screen goes red', async () => {
  const state = await face.read(port());
  const tree = face.view(state);
  const planted = { tag: 'svg', attrs: { 'data-mark': 'weather/rain', 'data-means': 'weather.rain' }, children: [] };
  tree.children.push(planted);
  const result = report({ trees: [tree] });
  const marks = result.checks.find((c) => c.id === 'declared-marks-only');
  assert.equal(marks.holds, false);
});

test('negative control: one meaning with two marks goes red', async () => {
  const state = await face.read(port());
  const tree = face.view(state);
  tree.children.push({ tag: 'svg', attrs: { 'data-mark': 'structure/hole', 'data-means': 'verdict.admit' }, children: [] });
  const result = report({ trees: [tree] });
  const single = result.checks.find((c) => c.id === 'one-meaning-one-mark');
  assert.equal(single.holds, false);
});

test('negative control: a named test that is gone goes red', () => {
  const result = report({ trees: [], declaration: { ...DECLARATION, tests: [...DECLARATION.tests, 'test/vanished.mjs'] } });
  const named = result.checks.find((c) => c.id === 'named-tests-exist');
  assert.equal(named.holds, false);
});

test('the mark check refuses to pass on an empty population', () => {
  const result = report({ trees: [] });
  const marks = result.checks.find((c) => c.id === 'declared-marks-only');
  assert.equal(marks.holds, false, 'a rule that passes when nothing was drawn is not a rule');
});
