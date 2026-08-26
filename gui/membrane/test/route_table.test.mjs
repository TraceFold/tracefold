// SPDX-License-Identifier: Apache-2.0
// The table against the crate that serves it.
//
// The number of routes is not written in prose anywhere in this module. It is read
// off the router, and this file is what says so.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { TABLE, tableRows, methodName } from '../src/membrane.mjs';
import { extractFromCrate, crateAvailable, crateLibPath, CRATE_ABSENT_REASON } from '../tools/route_table_from_crate.mjs';

const ROWS = tableRows();
const pair = (r) => `${r.verb} ${r.path}`;

// A skip here is a reading, not an absence of one, so it says what went unread. The
// crate is in the engine's tree; a clone of this tree alone cannot see it and must not
// pretend the bijection held (req/883).
if (!crateAvailable()) console.log(`      T1/T1-neg UNMEASURED -- ${CRATE_ABSENT_REASON}`);

test('T1 the table holds exactly the routes the crate registers, and the base path it nests on', {
  skip: !crateAvailable() && CRATE_ABSENT_REASON,
}, () => {
  const extracted = extractFromCrate();
  assert.equal(TABLE.base_path, extracted.base_path);
  const mine = ROWS.map(pair).sort();
  const theirs = extracted.routes.map(pair).sort();
  assert.deepEqual(mine, theirs);
  console.log(`      T1 ${theirs.length} routes compared against ${crateLibPath()}`);
});

test('T1-neg the comparison notices a route the table would have missed', {
  skip: !crateAvailable() && CRATE_ABSENT_REASON,
}, () => {
  const extracted = extractFromCrate();
  const dropped = extracted.routes.slice(1).map(pair).sort();
  assert.notDeepEqual(ROWS.map(pair).sort(), dropped);
});

test('T2 a name is a function of the verb and the path, and no two rows collide', () => {
  const names = ROWS.map((r) => r.name);
  assert.equal(new Set(names).size, names.length);
  for (const row of ROWS) assert.equal(row.name, methodName(row.verb, row.path));
  assert.equal(methodName('GET', '/verdict-checkpoints/{window_end}'), 'get_verdict_checkpoints_window_end');
});

test('T3 every row declares every column, with values from the fixed sets', () => {
  const allowed = {
    effect: ['read', 'write'],
    kind: ['single', 'list', 'stream'],
    auth: ['bearer', 'none'],
    idempotency: ['accepts', 'none'],
    cursor: ['opaque', 'index', null],
    actor: ['required', 'optional', 'none'],
  };
  for (const row of ROWS) {
    for (const [column, values] of Object.entries(allowed)) {
      assert.ok(column in row, `${row.name} has no ${column}`);
      assert.ok(values.includes(row[column]), `${row.name}.${column} = ${row[column]}`);
    }
    assert.equal(typeof row.accepts_body, 'boolean');
  }
});

test('T4 exactly the three routes the crate caches a key for accept one', () => {
  const accepts = ROWS.filter((r) => r.idempotency === 'accepts').map((r) => r.name).sort();
  // gx-api/src/handlers.rs:323 defines the reader; it is called at :917 and :1339. Since
  // req/824 A4 (crate commit 16863593, absorbed req/822_c6), attach_sources.rs register()
  // also reads `idempotency-key`, into its own replay cache -- a third caching route.
  assert.deepEqual(accepts, ['post_attach_sources', 'post_candidates_id_commit', 'post_transformations_id_undo']);
});

test('T5 exactly one route sits outside the bearer guard', () => {
  const open = ROWS.filter((r) => r.auth === 'none').map((r) => r.name);
  assert.deepEqual(open, ['get_healthz']);
});

test('T6 a route that takes no body declares no actor', () => {
  for (const row of ROWS) {
    if (!row.accepts_body) assert.equal(row.actor, 'none', `${row.name}`);
  }
});

test('T7 the port exposes one callable per row and nothing besides the four helpers', async () => {
  const { createMembrane } = await import('../src/membrane.mjs');
  const { port } = createMembrane({ origin: 'http://127.0.0.1:1' });
  const helpers = ['direct', 'fold', 'ledgers', 'routes'];
  const names = Object.keys(port).filter((k) => !helpers.includes(k));
  assert.equal(names.length, ROWS.length);
  assert.deepEqual(names.sort(), ROWS.map((r) => r.name).sort());
});
