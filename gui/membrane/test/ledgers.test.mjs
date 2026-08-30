// SPDX-License-Identifier: Apache-2.0
// The three uncovered-range declarations, and the two directions their gate refuses in.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { createMembrane, tableRows, COVERAGE, WIRE_FIELDS } from '../src/membrane.mjs';
import { deriveLedgers } from '../src/ledgers.mjs';

const ROWS = tableRows();
const FIELDS = WIRE_FIELDS.fields;
const clone = (x) => JSON.parse(JSON.stringify(x));

test('L1 the denominator is the table\'s own, and today\'s coverage is the terminal face\'s own registration', () => {
  // req/967 §4-2: the terminal face landed and registered itself in COVERAGE (the JSON
  // beside this module, not a number written here) -- so "every route is uncovered" is
  // no longer this file's claim. What stays true, and is asserted the same way it was
  // before the face existed, is that N is read off the table/field domain and n is
  // whatever COVERAGE's own consumed/drawn currently leave uncovered -- derived from
  // COVERAGE, never hand-counted, so a face landing or leaving cannot make this stale.
  const { port } = createMembrane({ origin: 'http://127.0.0.1:8787' });
  const { ledgers, ok, problems } = port.ledgers();
  assert.equal(ok, true, problems.join('; '));
  const calledRoutes = new Set(
    Object.entries(COVERAGE.consumed ?? {}).filter(([, faces]) => faces.length > 0).map(([name]) => name),
  );
  const drawnFields = new Set(COVERAGE.drawn ?? []);
  assert.equal(ledgers.NOT_CONSUMED.N, ROWS.length);
  assert.equal(ledgers.NOT_CONSUMED.n, ROWS.length - calledRoutes.size);
  assert.equal(ledgers.NOT_DRAWN.N, FIELDS.length);
  assert.equal(ledgers.NOT_DRAWN.n, FIELDS.length - drawnFields.size);
  assert.equal(ledgers.NOT_A_ROUTE.n, 0);
  // Every member carries a reason, and the reasons are as many as the members.
  assert.equal(ledgers.NOT_CONSUMED.entries.length, ledgers.NOT_CONSUMED.n);
  // And the face that IS registered really did leave the ledger, not just shrink the count.
  for (const route of calledRoutes) assert.ok(!ledgers.NOT_CONSUMED.members.includes(route), `${route} is registered as consumed and is still in NOT_CONSUMED`);
});

test('L2 a face that starts calling a route leaves the ledger, and its stale reason is refused', () => {
  const coverage = clone(COVERAGE);
  // post_candidates, not get_candidates: the terminal face already registered
  // get_candidates as consumed (req/967 §4-2) and its old reason was removed in the
  // same change that registered it, which is the behaviour this test exists to prove --
  // so a route that still carries a real, un-removed reason is what demonstrates the
  // refusal here. post_candidates is one (no face proposes a change yet).
  coverage.consumed = { post_candidates: ['ledger-face'] };
  const { ledgers, problems, ok } = deriveLedgers({ routes: ROWS, coverage, fields: FIELDS });
  assert.equal(ledgers.NOT_CONSUMED.n, ROWS.length - 1);
  assert.ok(!ledgers.NOT_CONSUMED.members.includes('post_candidates'));
  assert.equal(ok, false);
  assert.ok(problems.some((p) => /post_candidates.*not a member/.test(p)), problems.join('; '));
});

test('L3 a route with no reason is refused (the forgotten-entry road)', () => {
  const routes = [...ROWS, { verb: 'GET', path: '/newly-served', name: 'get_newly_served' }];
  const { ledgers, ok, problems } = deriveLedgers({ routes, coverage: clone(COVERAGE), fields: FIELDS });
  assert.equal(ok, false);
  assert.ok(problems.some((p) => p.includes('get_newly_served')));
  // Not routes.length: the terminal face's own registration (req/967 §4-2) already
  // takes some of them out of NOT_CONSUMED, so the count is derived from COVERAGE the
  // same way L1 derives it, rather than assumed to be every route.
  const calledRoutes = new Set(
    Object.entries(COVERAGE.consumed ?? {}).filter(([, faces]) => faces.length > 0).map(([name]) => name),
  );
  assert.equal(ledgers.NOT_CONSUMED.n, routes.length - calledRoutes.size);
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
  // get_ledger_proof stays genuinely NOT_CONSUMED (no face carries a verifier for it,
  // req/967 §5-1); a route the terminal face already registered as consumed (e.g.
  // get_healthz, get_stream) is no longer a member of NOT_CONSUMED at all, so a bad tag
  // written against one of those would be refused for the different reason of naming a
  // non-member, not for the tag itself.
  coverage.reasons.NOT_CONSUMED.get_ledger_proof = { tag: 'later', note: '' };
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
