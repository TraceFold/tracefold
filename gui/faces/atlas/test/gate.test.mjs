// SPDX-License-Identifier: Apache-2.0
// The gates, run against the shipped source, and then run against source written to
// break them. A gate that has never gone red is a gate nobody has seen work.

import test from 'node:test';
import assert from 'node:assert/strict';

import { CHECKS, checkSource, report, shippedSources } from '../tools/gate.mjs';
import { DECLARATION } from '../declaration.mjs';
import { face } from '../atlas.mjs';
import { stubPort, page } from '../../ledger/test/stub-port.mjs';

function item(id, sequence, path, extra = {}) {
  return {
    id, sequence, path, at: `2026-08-24T09:${String(sequence).padStart(2, '0')}:00Z`,
    actor: 'agent:packer', effect: 'write', verdict: 'Admit', digest: `a1b2c3d4e5f6${String(sequence).padStart(4, '0')}`,
    ...extra,
  };
}

const ITEMS = [
  item('t-001', 1, '/work/a.md'),
  item('t-002', 2, '/work/a.md'),
  item('t-003', 3, '/work/b.md'),
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
  ['rows-are-not-edited', 'subject.touchCount = 999;'],
  ['no-hardcoded-subject-open', 'const open = true;'],
  ['no-scrolling-container', "style({ 'overflow-y': 'auto', 'max-height': '520px' })"],
  ['no-inline-cursor', "style({ padding: '0 10px', cursor: 'default' })"],
  // Owner #348 (5). Both spellings of the same mistake are planted, because the rule
  // is one pattern with two halves and a plant that only exercises one of them leaves
  // the other unfired.
  ['no-raw-motion', "style({ transition: 'background-color 140ms ease' })"],
  ['no-raw-motion', "style({ 'transition-duration': '90ms' })"],
  ['no-raw-corner', "style({ 'border-radius': '4px' })"],
  ['weights-come-from-the-scale', "style({ 'font-weight': '600' })"],
];

for (const [id, planted] of PLANTS) {
  test(`negative control: ${id} goes red on planted source`, () => {
    const check = CHECKS.find((c) => c.id === id);
    assert.ok(check, `no such check: ${id}`);
    const result = checkSource(check, [{ file: 'planted.mjs', text: planted }]);
    assert.equal(result.holds, false, `${id} did not notice: ${planted}`);
  });
}

test('the hardcoded-subject-open check does not fire on the legitimate computed-decision assignment', () => {
  const check = CHECKS.find((c) => c.id === 'no-hardcoded-subject-open');
  const result = checkSource(check, [{ file: 'planted.mjs', text: 'const open = needsOpen(subject);' }]);
  assert.equal(result.holds, true, 'calling needsOpen() (a computed decision) is exactly what the attest/render split requires');
});

test('the hardcoded-subject-open check does not fire on the object-literal attribute key (open: ...), only on the assignment form (open = ...)', () => {
  const check = CHECKS.find((c) => c.id === 'no-hardcoded-subject-open');
  const result = checkSource(check, [{ file: 'planted.mjs', text: "el('details', { open: open || null })" }]);
  assert.equal(result.holds, true, 'this is the legitimate el() attribute form, not a hardcoded assignment');
});

/**
 * The three rules added in Owner #348 (5) each exempt exactly one spelling -- the one
 * that reads the value off its own scale. The exemption is where a grep rule goes
 * wrong: written as `:\s*(?!T\.radius)` the `\s*` backtracks to zero, the lookahead is
 * tested against a space, and the rule fires on the correct code. It did that on first
 * run, on the line this round had just fixed, which is why these three exist.
 */
for (const [id, legitimate] of [
  ['no-raw-corner', "style({ 'border-radius': T.radiusControl })"],
  ['weights-come-from-the-scale', "style({ 'font-weight': TYPE.body.weight })"],
  ['weights-come-from-the-scale', "style({ 'font-weight': TYPE[role].weight })"],
  ['no-raw-motion', 'const ms = ended - started;'],
]) {
  test(`${id} does not fire on the one spelling it exempts: ${legitimate}`, () => {
    const check = CHECKS.find((c) => c.id === id);
    const result = checkSource(check, [{ file: 'planted.mjs', text: legitimate }]);
    assert.equal(result.holds, true, `${id} fired on the form it exists to require`);
  });
}

test('the scrolling-container check does not fire on clipping or on wrapping, which are not scrolling', () => {
  const check = CHECKS.find((c) => c.id === 'no-scrolling-container');
  const result = checkSource(check, [{ file: 'planted.mjs', text: "style({ overflow: 'hidden', 'overflow-wrap': 'anywhere' })" }]);
  assert.equal(result.holds, true, 'a clipped cell and a wrapped word are both things this face does on purpose');
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
  tree.children.push({ tag: 'svg', attrs: { 'data-mark': 'structure/hole', 'data-means': 'structure.subject' }, children: [] });
  const result = report({ trees: [tree] });
  const single = result.checks.find((c) => c.id === 'one-meaning-one-mark');
  assert.equal(single.holds, false);
});

test('negative control: a foldable whose fold mark disagrees with its own open state goes red', async () => {
  const state = await face.read(port());
  const tree = face.view(state);
  // A closed subject (data-open="false") carrying the OPEN fold mark instead of
  // the shut one -- the two facts (open state, fold mark) are computed together
  // in atlas.mjs's subjectLine()/summaryRow(); this plants a genuine disagreement
  // between them the way graph's own edge-state-is-not-contradictory test does.
  const planted = {
    tag: 'details',
    attrs: { 'data-role': 'subject', 'data-path': '/planted', 'data-open': 'false' },
    children: [{ tag: 'svg', attrs: { 'data-mark': 'structure/fold-open', 'data-means': 'structure.fold.open' }, children: [] }],
  };
  tree.children.push(planted);
  const result = report({ trees: [tree] });
  const foldGate = result.checks.find((c) => c.id === 'fold-mark-agrees-with-open-state');
  assert.equal(foldGate.holds, false, 'the fold-mark-agrees-with-open-state gate did not notice a planted disagreement');
});

test('the fold-mark-agrees-with-open-state gate refuses to pass on an empty population', () => {
  const result = report({ trees: [] });
  const foldGate = result.checks.find((c) => c.id === 'fold-mark-agrees-with-open-state');
  assert.equal(foldGate.holds, false, 'a rule that passes when nothing was drawn is not a rule');
});

test('negative control: a named test that is gone goes red', () => {
  const result = report({ trees: [], declaration: { ...DECLARATION, tests: [...DECLARATION.tests, 'test/vanished.mjs'] } });
  const named = result.checks.find((c) => c.id === 'named-tests-exist');
  assert.equal(named.holds, false);
});

test('negative control: a mark drawn under the readable floor goes red', async () => {
  const state = await face.read(port());
  const tree = face.view(state);
  // 14 is the number every one of this face's eleven call sites carried before Owner
  // #348 (3), planted back as a mark that reached the tree at that size.
  tree.children.push({
    tag: 'svg',
    attrs: {
      'data-mark': 'structure/hole', 'data-means': 'structure.hole', width: '14', height: '14',
    },
    children: [],
  });
  const result = report({ trees: [tree] });
  const floor = result.checks.find((c) => c.id === 'marks-are-at-or-above-the-floor');
  assert.equal(floor.holds, false, 'the floor gate did not notice a 14px mark on the tree');
  assert.match(floor.detail, /structure\/hole at 14/);
});

test('the floor gate refuses to pass on an empty population', () => {
  const result = report({ trees: [] });
  const floor = result.checks.find((c) => c.id === 'marks-are-at-or-above-the-floor');
  assert.equal(floor.holds, false, 'a rule that passes when nothing was drawn is not a rule');
});

test('the mark check refuses to pass on an empty population', () => {
  const result = report({ trees: [] });
  const marks = result.checks.find((c) => c.id === 'declared-marks-only');
  assert.equal(marks.holds, false, 'a rule that passes when nothing was drawn is not a rule');
});
