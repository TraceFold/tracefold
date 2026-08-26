// SPDX-License-Identifier: Apache-2.0
// The gates, run against the shipped source, and then run against source written to
// break them. A gate that has never gone red is a gate nobody has seen work.

import test from 'node:test';
import assert from 'node:assert/strict';

import { CHECKS, checkSource, report, shippedSources } from '../tools/gate.mjs';
import { DECLARATION } from '../declaration.mjs';
import { parts } from '../binding.mjs';
import { face } from '../graph.mjs';
import { stubPort, page } from '../../ledger/test/stub-port.mjs';

function item(id, sequence, path, prev, extra = {}) {
  return {
    id, sequence, prev, path, at: `2026-08-24T09:${String(sequence).padStart(2, '0')}:00Z`,
    actor: 'agent:packer', effect: 'write', verdict: 'Admit', digest: `d${String(sequence).padStart(3, '0')}`,
    ...extra,
  };
}

const ITEMS = [
  item('t-001', 1, '/work/a.md', null),
  item('t-002', 2, '/work/a.md', 't-001'),
  item('t-003', 3, '/work/b.md', 't-900'),
  item('t-004', 4, '/work/b.md', 't-003'),
];

const port = () => stubPort({ get_transformations: page(ITEMS) }, { methods: DECLARATION.consumes });

test('the shipped face passes every source gate', async () => {
  const state = await face.read(port());
  const result = report({ trees: [face.view(state)] });
  const failing = result.checks.filter((c) => !c.holds);
  assert.deepEqual(failing.map((c) => `${c.id}: ${c.detail}`), []);
  assert.ok(result.checks.length >= 14, 'too few checks to call this a gate');
});

test('the gate reads a non-empty set of shipped files', () => {
  const sources = shippedSources();
  assert.ok(sources.length >= 4);
  for (const source of sources) assert.ok(source.text.length > 200, `${source.file} is suspiciously small`);
});

const PLANTS = [
  ['no-network', "const r = await fetch('http://127.0.0.1:8787/v1/transformations');"],
  ['no-foreign-import', "import { createMembrane } from '../../membrane/src/index.mjs';"],
  ['no-verification', 'const ok = verify(record.digest, record.anchor);'],
  ['no-actor-named', "const body = { actor: { Human: { id: 'me' } } };"],
  ['no-colour-literal', "const ink = '#1a1a1a';"],
  ['no-borrowed-symbol', "const bullet = '●';"],
  ['nothing-out-of-flow', "const s = 'position:absolute;left:0';"],
  ['no-method-literals-outside-the-declaration', "await port.get_transformations('get_transformations');"],
  ['no-dynamic-code', 'const f = new Function("return 1");'],
  ['rows-are-not-edited', 'node.verdict = "Admit";'],
  ['no-hardcoded-childof', "return { ...node, childOf: 't-001' };"],
  // Owner #348 (5). One planted line each, in the exact form the defect would take if
  // it were written by hand into this face tomorrow.
  ['no-raw-motion', "style({ transition: 'background 140ms ease' })"],
  ['no-raw-corner', "style({ 'border-radius': '4px' })"],
  ['no-raw-weight', "style({ 'font-weight': '600' })"],
  ['no-raw-mark-size', "P.glyph('structure', 'child', { size: 14, label: 'linked' })"],
];

for (const [id, planted] of PLANTS) {
  test(`negative control: ${id} goes red on planted source`, () => {
    const check = CHECKS.find((c) => c.id === id);
    assert.ok(check, `no such check: ${id}`);
    const result = checkSource(check, [{ file: 'planted.mjs', text: planted }]);
    assert.equal(result.holds, false, `${id} did not notice: ${planted}`);
  });
}

test('the hardcoded-childOf check does not fire on the legitimate resolved-lookup assignment', () => {
  const check = CHECKS.find((c) => c.id === 'no-hardcoded-childof');
  const result = checkSource(check, [{ file: 'planted.mjs', text: 'return Object.freeze({ ...node, childOf: predecessor.id });' }]);
  assert.equal(result.holds, true, 'reading predecessor.id (an already-resolved lookup) is exactly what the attest step is supposed to do');
});

/**
 * The other side of each new rule: the legitimate form it must NOT fire on. A rule that
 * cannot tell the cure from the defect is a rule that gets deleted the first time it is
 * inconvenient, which is how a scale stops being a scale.
 */
const ALLOWED = [
  ['no-raw-motion', "el('button', { class: 'gx-move' })"],
  ['no-raw-corner', "style({ 'border-radius': T.radiusControl })"],
  ['no-raw-weight', "style({ 'font-weight': WEIGHT.label })"],
  ['no-raw-mark-size', "P.glyph('structure', 'child', { size: P.minReadable, label: 'linked' })"],
  // The one that would be easiest to write by accident: a type size is not a mark size,
  // and a rule that could not tell them apart would forbid the whole type scale.
  ['no-raw-mark-size', "style({ 'font-size': T.record })"],
];

for (const [id, allowed] of ALLOWED) {
  test(`${id} does not fire on the form it exists to require: ${allowed.slice(0, 40)}`, () => {
    const check = CHECKS.find((c) => c.id === id);
    const result = checkSource(check, [{ file: 'planted.mjs', text: allowed }]);
    assert.equal(result.holds, true, `${id} fired on the legitimate form: ${allowed}`);
  });
}

test('negative control: a mark drawn under the floor goes red, and the floor is the sheet\'s own number', async () => {
  const state = await face.read(port());
  const tree = face.view(state);
  const result = report({ trees: [tree] });
  const floorGate = result.checks.find((c) => c.id === 'marks-at-or-above-the-floor');
  assert.equal(floorGate.holds, true, 'the shipped face draws a mark under the floor');
  assert.match(floorGate.name, new RegExp(String(parts.minReadable)));
  // A size arriving from a variable rather than from a typed number is exactly the half
  // the source rule cannot see, so this is the one that has to be planted in the tree.
  tree.children.push({
    tag: 'svg',
    attrs: {
      'data-mark': 'structure/child', 'data-means': 'structure.child', width: String(parts.minReadable - 1), height: String(parts.minReadable - 1),
    },
    children: [],
  });
  const after = report({ trees: [tree] });
  assert.equal(after.checks.find((c) => c.id === 'marks-at-or-above-the-floor').holds, false);
});

test('the floor gate refuses to pass on an empty population', () => {
  const result = report({ trees: [] });
  const floorGate = result.checks.find((c) => c.id === 'marks-at-or-above-the-floor');
  assert.equal(floorGate.holds, false, 'a rule that passes when nothing was drawn is not a rule');
});

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
  // 'verdict.admit' is the meaning t-001's row already draws -- planting a second
  // mark for that same meaning is what makes this a real collision rather than a
  // lone, uncontested entry.
  tree.children.push({ tag: 'svg', attrs: { 'data-mark': 'structure/hole', 'data-means': 'verdict.admit' }, children: [] });
  const result = report({ trees: [tree] });
  const single = result.checks.find((c) => c.id === 'one-meaning-one-mark');
  assert.equal(single.holds, false);
});

test('negative control: a row drawn as both chained and outside-declared-not-drawn goes red', async () => {
  const state = await face.read(port());
  const tree = face.view(state);
  // The shipped tree never contradicts itself (both readings come from the one
  // buildGraph() pass): plant a second, contradicting outside-annotation naming a
  // row this fixture's own chain already resolved (t-002, chained under t-001).
  tree.children.push({
    tag: 'div',
    attrs: { 'data-role': 'edge-outside', 'data-to': 't-002', 'data-wanted-prev': 't-001' },
    children: [],
  });
  const result = report({ trees: [tree] });
  const edgeGate = result.checks.find((c) => c.id === 'edge-state-is-not-contradictory');
  assert.equal(edgeGate.holds, false, 'the edge-state-is-not-contradictory gate did not notice a planted disagreement');
});

test('the edge-state-is-not-contradictory gate refuses to pass on an empty population', () => {
  const result = report({ trees: [] });
  const edgeGate = result.checks.find((c) => c.id === 'edge-state-is-not-contradictory');
  assert.equal(edgeGate.holds, false, 'a rule that passes when nothing was drawn is not a rule');
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
