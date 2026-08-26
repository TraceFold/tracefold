// SPDX-License-Identifier: Apache-2.0
// The row vocabulary projection, tested against RAW WIRE BYTES (the same discipline
// wire_contract.test.mjs states in its header: a fixture shaped for a face is a
// fixture that has stopped testing the contract).
//
// req/822_c6: the engine emits `transformation/state/verdict/enforced/created_at/
// actor/scope` (gx-api/src/list.rs row_json(), dumped live from a bound bed this
// session); the five row-drawing faces read `id/at/actor/effect/verdict/path`.
// c5 measured the result: held drew 0 of 3 real candidates. The projection under
// test here is the membrane's declared, additive translation of the M-15 members
// ("time / who / target for a GUI's list") into the row grammar's names — raw
// members preserved, nothing overridden that speaks for itself.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { createMembrane } from '../src/index.mjs';
import { OUTCOME, JSON_MEDIA_TYPE } from '../src/wire.mjs';

const ORIGIN = 'http://127.0.0.1:8787';
const TOKEN = 'test-token';

// Captured 2026-08-26 from gx 0.1.0 on /tmp/gxapp_c6 (req/822_c6 §1) — bytes, not a
// face-shaped literal. One real row, one Agent row (context.rs Actor's third shape),
// one row that already carries an `id` of its own (the speaks-for-itself case).
const WIRE = {
  candidates:
    '{"items":['
    + '{"transformation":"gx1:xj4c7j5gdxvd7sgpd37xzd2jfg5ju2rrmozn7taxywzkeh27bwya","state":"Candidate",'
    + '"verdict":null,"enforced":true,"created_at":"2026-08-25T17:05:38.113049332Z",'
    + '"actor":{"Human":{"key":"window"}},"scope":"/tmp/gxapp_c6/alpha.txt"},'
    + '{"transformation":"gx1:s3id52olzfpwev3mmax7po5snmj32p2cyrh5qcsikvttrki22htq","state":"Candidate",'
    + '"verdict":null,"enforced":true,"created_at":"2026-08-25T17:05:38.149929696Z",'
    + '"actor":{"Agent":{"key":"k2","model":"m-1"}},"scope":"/tmp/gxapp_c6/beta.txt"},'
    + '{"transformation":"gx1:szpkymu4fy5ef7tpanbfccnemk67qkbicvp2b53jfn2uyhxj5zfa","id":"already-said",'
    + '"state":"Candidate","verdict":null,"enforced":true,"created_at":"2026-08-25T17:05:38.185752014Z",'
    + '"actor":null,"scope":"/tmp/gxapp_c6/gamma.txt"}'
    + '],"next_cursor":null}',
  transformations:
    '{"items":[{"transformation":"gx1:xj4c7j5gdxvd7sgpd37xzd2jfg5ju2rrmozn7taxywzkeh27bwya",'
    + '"state":"Candidate","verdict":null,"enforced":true,"created_at":"2026-08-25T17:05:38.113049332Z",'
    + '"actor":{"Process":{"key":"k3"}},"scope":"/tmp/gxapp_c6/alpha.txt",'
    + '"superseded_by":null,"inverse_status":null,"rollback":null}],"next_cursor":null}',
  escalations: '{"items":[{"unobserved_member":"x"}],"next_cursor":null}',
};

function replies(body) {
  return async () => ({
    ok: true,
    status: 200,
    headers: { get: (name) => (name.toLowerCase() === 'content-type' ? JSON_MEDIA_TYPE : null) },
    text: async () => body,
  });
}

function bed(body) {
  return createMembrane({ origin: ORIGIN, token: TOKEN, fetchImpl: replies(body) });
}

test('V1 a folded candidates list carries the row grammar members beside the wire ones', async () => {
  const { port } = bed(WIRE.candidates);
  const got = await port.fold('get_candidates');
  assert.equal(got.outcome, OUTCOME.ANSWERED);
  assert.equal(got.vocabulary, 'row');
  const wire = JSON.parse(WIRE.candidates);
  const first = got.items[0];
  // The projection: id/at/path from transformation/created_at/scope (M-15 / DR-44-9).
  assert.equal(first.id, wire.items[0].transformation);
  assert.equal(first.at, wire.items[0].created_at);
  assert.equal(first.path, wire.items[0].scope);
  // The actor flatten rule: Variant:key, Agent appends the model.
  assert.equal(first.actor, 'Human:window');
  assert.equal(got.items[1].actor, 'Agent:k2 (m-1)');
  // Additive, never destructive: every wire member is still on the item, and the raw
  // row survives whole under `wire`.
  for (const key of Object.keys(wire.items[0])) assert.ok(key in first, `dropped ${key}`);
  assert.deepEqual(first.wire, wire.items[0]);
  assert.equal(first.state, 'Candidate');
  assert.equal(first.enforced, true);
});

test('V2 an item that speaks for itself is worn as stated (no override)', async () => {
  const { port } = bed(WIRE.candidates);
  const got = await port.fold('get_candidates');
  const third = got.items[2];
  assert.equal(third.id, 'already-said');
  // A null actor gains no invented scalar; the face draws its declared hole.
  assert.equal(third.actor, null);
});

test('V3 transformations translate; an unobserved route passes through untouched', async () => {
  const trans = bed(WIRE.transformations);
  const got = await trans.port.fold('get_transformations');
  assert.equal(got.vocabulary, 'row');
  assert.equal(got.items[0].actor, 'Process:k3');
  assert.equal(got.items[0].path, '/tmp/gxapp_c6/alpha.txt');
  assert.equal(got.items[0].inverse_status, null);

  const esc = bed(WIRE.escalations);
  const raw = await esc.port.fold('get_escalations');
  assert.equal(raw.vocabulary, undefined);
  assert.deepEqual(raw.items[0], { unobserved_member: 'x' });
  assert.ok(!('wire' in raw.items[0]));
});

test('V4 a malformed actor structure stays a structure (the hole is the honest cell)', async () => {
  const body = '{"items":[{"transformation":"t9","state":"Candidate","verdict":null,"enforced":true,'
    + '"created_at":"2026-08-25T00:00:00Z","actor":{"Human":{"key":"a"},"Agent":{"key":"b","model":"m"}},'
    + '"scope":"s9"}],"next_cursor":null}';
  const { port } = bed(body);
  const got = await port.fold('get_candidates');
  // Two variants on one actor is not a shape context.rs Actor can serialise; the
  // membrane refuses to guess which one acted and leaves the structure for the face
  // to declare MEMBER_NOT_SCALAR over.
  assert.equal(typeof got.items[0].actor, 'object');
  assert.equal(got.items[0].id, 't9');
});
