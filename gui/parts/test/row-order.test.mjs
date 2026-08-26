// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { order, repeated, ORDERS, DROP_REASONS, ORDER_MESSAGES } from '../src/row-order.mjs';

const WELL_FORMED = [
  { id: 'r-03', n: 3, at: '2026-08-24T09:14:19Z' },
  { id: 'r-01', n: 1, at: '2026-08-24T09:14:02Z' },
  { id: 'r-02', n: 2, at: '2026-08-24T09:14:07Z' },
];

test('there is no default order: asking for none raises instead of picking one', () => {
  assert.throws(() => order(WELL_FORMED), new RegExp(ORDER_MESSAGES.ORDER_REQUIRED));
  assert.throws(() => order(WELL_FORMED, {}), new RegExp(ORDER_MESSAGES.ORDER_REQUIRED));
  assert.throws(() => order(WELL_FORMED, { by: null }), new RegExp(ORDER_MESSAGES.ORDER_REQUIRED));
});

test('an order nobody defined is refused rather than approximated', () => {
  assert.throws(() => order(WELL_FORMED, { by: 'alphabetical' }), new RegExp(ORDER_MESSAGES.UNKNOWN_ORDER));
});

test('every order carries the reason it is that order', () => {
  for (const [name, spec] of Object.entries(ORDERS)) {
    assert.ok(spec.reason.length > 20, `${name} states why`);
    assert.ok(Array.isArray(spec.assumes));
  }
});

test('a stated order is applied and reported back with its reason', () => {
  const result = order(WELL_FORMED, { by: 'by-sequence' });
  assert.deepEqual(result.rows.map((r) => r.id), ['r-01', 'r-02', 'r-03']);
  assert.equal(result.substituted, false);
  assert.equal(result.by, 'by-sequence');
  assert.equal(result.reason, ORDERS['by-sequence'].reason);
});

test('the order that needs no assumption keeps what arrived', () => {
  const result = order(WELL_FORMED, { by: 'as-recorded' });
  assert.deepEqual(result.rows.map((r) => r.id), ['r-03', 'r-01', 'r-02']);
  assert.deepEqual(result.assumptions, []);
});

test('comparing times as strings is checked against the data before it is trusted', () => {
  const result = order(WELL_FORMED, { by: 'by-time-then-id' });
  assert.deepEqual(result.assumptions.map((a) => a.holds), [true, true, true]);
  assert.deepEqual(result.rows.map((r) => r.id), ['r-01', 'r-02', 'r-03']);
});

test('mixed zones break the assumption, and the recorded order is kept with the substitution stated', () => {
  const mixed = [
    { id: 'r-01', n: 1, at: '2026-08-24T09:14:02Z' },
    { id: 'r-02', n: 2, at: '2026-08-24T09:14:07+09:00' },
  ];
  const result = order(mixed, { by: 'by-time-then-id' });
  assert.equal(result.substituted, true);
  assert.equal(result.requested, 'by-time-then-id');
  assert.equal(result.by, 'as-recorded');
  assert.match(result.reason, /times-share-one-zone/);
  assert.deepEqual(result.rows.map((r) => r.id), ['r-01', 'r-02'], 'nothing was sorted on a broken assumption');
});

test('ragged widths break it too, and each broken assumption is named', () => {
  const ragged = [{ id: 'a', at: '2026-08-24T09:14:02Z' }, { id: 'b', at: '9:14' }];
  const result = order(ragged, { by: 'by-time-then-id' });
  assert.equal(result.substituted, true);
  const broken = result.assumptions.filter((a) => !a.holds).map((a) => a.name);
  assert.ok(broken.includes('times-are-fixed-width'));
  assert.ok(broken.includes('times-are-zero-padded'));
  for (const assumption of result.assumptions) assert.ok(assumption.detail.length > 5, assumption.name);
});

test('a sequence order over unnumbered records substitutes rather than sorting undefined', () => {
  const result = order([{ id: 'a', n: 1 }, { id: 'b' }], { by: 'by-sequence' });
  assert.equal(result.substituted, true);
  assert.match(result.reason, /every-record-has-a-sequence/);
});

test('rows that cannot be drawn are dropped with a word for why, not with a count', () => {
  const result = order([{ id: 'a', n: 1 }, { n: 2 }, null, 'string', { id: '', n: 3 }], { by: 'as-recorded' });
  assert.deepEqual(result.dropped.map((d) => d.why), [
    DROP_REASONS.NO_IDENTITY, DROP_REASONS.NOT_AN_OBJECT, DROP_REASONS.NOT_AN_OBJECT, DROP_REASONS.NO_IDENTITY,
  ]);
  assert.deepEqual(result.dropped.map((d) => d.index), [1, 2, 3, 4]);
  assert.equal(result.rows.length, 1);
});

test('the drop reasons are words a caller can tell apart, which a boolean is not', () => {
  assert.equal(new Set(Object.values(DROP_REASONS)).size, Object.keys(DROP_REASONS).length);
  for (const reason of Object.values(DROP_REASONS)) assert.match(reason, /^[a-z-]+$/);
});

test('a value that was meant to appear once and appears twice is reported', () => {
  const twice = [{ id: 'a', n: 1 }, { id: 'a', n: 2 }, { id: 'c', n: 2 }];
  const result = order(twice, { by: 'as-recorded' });
  assert.deepEqual(result.repeated.id, [{ value: 'a', count: 2 }]);
  assert.deepEqual(result.repeated.n, [{ value: 2, count: 2 }]);
  assert.deepEqual(order(WELL_FORMED, { by: 'as-recorded' }).repeated, { id: [], n: [] });
});

test('repeated ignores what was never there rather than counting absences as a repeat', () => {
  assert.deepEqual(repeated([{ id: 'a' }, {}, {}], 'id'), []);
  assert.deepEqual(repeated([{ n: null }, { n: null }], 'n'), []);
});

test('ordering does not disturb what it was given', () => {
  const given = WELL_FORMED.map((r) => ({ ...r }));
  const before = JSON.stringify(given);
  order(given, { by: 'by-sequence' });
  assert.equal(JSON.stringify(given), before);
});

test('nothing given is answered with nothing, not with a throw', () => {
  const result = order([], { by: 'as-recorded' });
  assert.deepEqual(result.rows, []);
  assert.deepEqual(result.dropped, []);
  assert.deepEqual(order(undefined, { by: 'as-recorded' }).rows, []);
});
