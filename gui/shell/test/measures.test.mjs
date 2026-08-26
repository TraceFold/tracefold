// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  suiteMeasure, benchMeasure, serveMeasure, formatMeasures, NOT_WIRED,
} from '../kernel/measures.mjs';

test('an unreachable suite report reads "not wired", never a fabricated number', () => {
  assert.equal(suiteMeasure(null).text, `suite: ${NOT_WIRED}`);
  assert.equal(suiteMeasure(null).ok, null);
  assert.equal(suiteMeasure({}).ok, null);
});

test('a reachable suite report states pass/total for both assays and lint', () => {
  // req/822_c7 S5: this fired red when the strip's `suite ` frame word was purged --
  // the counts and their nouns are the fact; the format is the compact one now.
  const run = { assays: { pass: 4, fail: 2, total: 6 }, lint: { pass: 6, fail: 0, total: 6 } };
  const measure = suiteMeasure(run);
  assert.equal(measure.text, 'assays 4/6, lint 6/6');
  assert.equal(measure.ok, false);
});

test('a suite report with zero failures reads ok', () => {
  const run = { assays: { pass: 6, fail: 0, total: 6 }, lint: { pass: 6, fail: 0, total: 6 } };
  assert.equal(suiteMeasure(run).ok, true);
});

test('an unreachable bench report reads "not wired"', () => {
  assert.equal(benchMeasure(null).text, `bench: ${NOT_WIRED}`);
  assert.equal(benchMeasure({ note: 'no medianMs here' }).ok, null);
});

test('a reachable bench report states the median, and its budget rides the title', () => {
  // req/822_c7 S5: fired red on the purge -- the budget left the strip's line for the
  // hover layer. It is still stated, still derived from the report, never dropped.
  const bench = { medianMs: 0.2138, budgetMs: 50, ok: true };
  assert.equal(benchMeasure(bench).text, 'shell mount 0.2138ms');
  assert.equal(benchMeasure(bench, 'membrane request').text, 'membrane request 0.2138ms');
  assert.equal(benchMeasure(bench).title, 'median 0.2138ms against a 50ms budget');
});

test('serve state is honest about not knowing an origin', () => {
  assert.equal(serveMeasure(null).text, `served: ${NOT_WIRED}`);
  assert.equal(serveMeasure('').ok, null);
  assert.equal(serveMeasure('http://127.0.0.1:8788').ok, true);
});

test('formatMeasures returns all three slots even when every source is absent', () => {
  const measures = formatMeasures({});
  assert.ok('suite' in measures && 'bench' in measures && 'serve' in measures);
  assert.equal(measures.suite.ok, null);
});

// req/822_c5 item 1 (the row c4 §2 left open): the strip drew `.run/report.json` with no
// age and no tree comparison, so for three cycles it presented two T3 failures recorded
// against the INIT tree as a reading about the current one. The report carries its own
// tree digest; the bed can compute the digest of the tree it is serving with the same
// shipped derivation (tools/rig/manifest.mjs, never a copy -- §234); the formatter is
// handed both and may only present the numbers as current when they are about this tree.
test('a report about another tree says so, with its age, and is neither green nor red', () => {
  const run = {
    tree: '39d4714e00000000',
    assays: { pass: 4, fail: 2, total: 6 },
    lint: { pass: 6, fail: 0, total: 6 },
  };
  const now = { tree: '4a4ace81d1b4496c', atMs: 1_000_000_000, reportMtimeMs: 1_000_000_000 - 38 * 60 * 1000 };
  const measure = suiteMeasure(run, now);
  assert.equal(measure.stale, true);
  // A failing count from another tree must not turn THIS tree's strip red, and a passing
  // one must not turn it green: the reading is about a tree that no longer exists.
  assert.equal(measure.ok, null);
  assert.match(measure.text, /another tree/);
  assert.match(measure.text, /38m/);
});

test('a report about this very tree carries no stale claim and keeps its verdict', () => {
  const run = {
    tree: '4a4ace81d1b4496c',
    assays: { pass: 6, fail: 0, total: 6 },
    lint: { pass: 6, fail: 0, total: 6 },
  };
  const now = { tree: '4a4ace81d1b4496c', atMs: 5000, reportMtimeMs: 1000 };
  const measure = suiteMeasure(run, now);
  assert.equal(measure.stale, false);
  assert.equal(measure.ok, true);
  assert.doesNotMatch(measure.text, /another tree/);
});

test('with no basis for comparison the formatter claims neither fresh nor stale', () => {
  const run = { assays: { pass: 6, fail: 0, total: 6 }, lint: { pass: 6, fail: 0, total: 6 } };
  // No `now` at all (an old bed), and a report that carries no tree digest.
  assert.equal(suiteMeasure(run).stale, null);
  assert.equal(suiteMeasure(run, { tree: 'abcd', atMs: 1, reportMtimeMs: 0 }).stale, null);
  assert.equal(suiteMeasure({ ...run, tree: 'abcd' }, null).stale, null);
});

test('an age the bed could not read is stated as unknown, not invented', () => {
  const run = { tree: 'aaaa', assays: { pass: 1, fail: 0, total: 1 }, lint: { pass: 1, fail: 0, total: 1 } };
  const now = { tree: 'bbbb', atMs: 1000, reportMtimeMs: null };
  const measure = suiteMeasure(run, now);
  assert.equal(measure.stale, true);
  assert.match(measure.text, /another tree/);
  assert.doesNotMatch(measure.text, /ago/);
});

test('formatMeasures hands the comparison basis through to the suite slot', () => {
  const run = { tree: 'aaaa', assays: { pass: 1, fail: 0, total: 1 }, lint: { pass: 1, fail: 0, total: 1 } };
  const now = { tree: 'bbbb', atMs: 3 * 60 * 60 * 1000, reportMtimeMs: 0 };
  const measures = formatMeasures({ run, now });
  assert.equal(measures.suite.stale, true);
  assert.match(measures.suite.text, /3h/);
});
