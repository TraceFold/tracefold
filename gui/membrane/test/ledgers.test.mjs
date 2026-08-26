// SPDX-License-Identifier: Apache-2.0
// The three uncovered-range declarations, and the two directions their gate refuses in.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { createMembrane, tableRows, COVERAGE, WIRE_FIELDS } from '../src/membrane.mjs';
import { deriveLedgers } from '../src/ledgers.mjs';

const ROWS = tableRows();
const FIELDS = WIRE_FIELDS.fields;
const clone = (x) => JSON.parse(JSON.stringify(x));

test('L1 today every route is uncovered, and the count is the table\'s own', () => {
  const { port } = createMembrane({ origin: 'http://127.0.0.1:8787' });
  const { ledgers, ok, problems } = port.ledgers();
  assert.equal(ok, true, problems.join('; '));
  assert.equal(ledgers.NOT_CONSUMED.N, ROWS.length);
  assert.equal(ledgers.NOT_CONSUMED.n, ROWS.length);
  assert.equal(ledgers.NOT_DRAWN.N, FIELDS.length);
  assert.equal(ledgers.NOT_DRAWN.n, FIELDS.length);
  assert.equal(ledgers.NOT_A_ROUTE.n, 0);
  // Every member carries a reason, and the reasons are as many as the members.
  assert.equal(ledgers.NOT_CONSUMED.entries.length, ledgers.NOT_CONSUMED.n);
});

test('L2 a face that starts calling a route leaves the ledger, and its stale reason is refused', () => {
  const coverage = clone(COVERAGE);
  coverage.consumed = { get_candidates: ['ledger-face'] };
  const { ledgers, problems, ok } = deriveLedgers({ routes: ROWS, coverage, fields: FIELDS });
  assert.equal(ledgers.NOT_CONSUMED.n, ROWS.length - 1);
  assert.ok(!ledgers.NOT_CONSUMED.members.includes('get_candidates'));
  assert.equal(ok, false);
  assert.ok(problems.some((p) => /get_candidates.*not a member/.test(p)), problems.join('; '));
});

test('L3 a route with no reason is refused (the forgotten-entry road)', () => {
  const routes = [...ROWS, { verb: 'GET', path: '/newly-served', name: 'get_newly_served' }];
  const { ledgers, ok, problems } = deriveLedgers({ routes, coverage: clone(COVERAGE), fields: FIELDS });
  assert.equal(ok, false);
  assert.ok(problems.some((p) => p.includes('get_newly_served')));
  assert.equal(ledgers.NOT_CONSUMED.n, routes.length);
});

test('L4 a wanted address is matched on the verb and the path together, never on a prefix', () => {
  const coverage = clone(COVERAGE);
  // The route the reference tree believed in: the path exists, the verb does not.
  coverage.requested = [{ verb: 'GET', path: '/candidates/{id}/escalation', face: 'held' }];
  const first = deriveLedgers({ routes: ROWS, coverage, fields: FIELDS });
  assert.deepEqual(first.ledgers.NOT_A_ROUTE.members, ['GET /candidates/{id}/escalation']);
  assert.equal(first.ok, false, 'and it must be explained before it passes');

  coverage.reasons.NOT_A_ROUTE = {
    'GET /candidates/{id}/escalation': { tag: 'backend_missing', note: 'the crate serves POST here and no GET' },
  };
  const second = deriveLedgers({ routes: ROWS, coverage, fields: FIELDS });
  assert.equal(second.ok, true, second.problems.join('; '));

  // The same address with the verb the server does serve is not a hole at all.
  coverage.requested = [{ verb: 'POST', path: '/candidates/{id}/escalation', face: 'held' }];
  coverage.reasons.NOT_A_ROUTE = {};
  const third = deriveLedgers({ routes: ROWS, coverage, fields: FIELDS });
  assert.equal(third.ledgers.NOT_A_ROUTE.n, 0);
});

test('L5 a reason tag outside the three is refused', () => {
  const coverage = clone(COVERAGE);
  coverage.reasons.NOT_CONSUMED.get_healthz = { tag: 'later', note: '' };
  const { ok, problems } = deriveLedgers({ routes: ROWS, coverage, fields: FIELDS });
  assert.equal(ok, false);
  assert.ok(problems.some((p) => p.includes('later')));
});

test('L6 a face claiming a route the table does not hold is refused', () => {
  const coverage = clone(COVERAGE);
  coverage.consumed = { get_receipts: ['receipt-face'] }; // the route is get_receipts_tid
  const { ok, problems } = deriveLedgers({ routes: ROWS, coverage, fields: FIELDS });
  assert.equal(ok, false);
  assert.ok(problems.some((p) => p.includes('get_receipts')));
});
