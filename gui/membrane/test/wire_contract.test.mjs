// SPDX-License-Identifier: Apache-2.0
// Contract tests written against RAW WIRE BYTES, not against post-transform shapes.
// req/01a §4-1 measured the failure mode this file exists to prevent: the reference
// tree's fixtures held the face-facing shape, so its suite never once ran the
// transform against the input the server actually sends.
//
// Every fixture below is a string. If a fixture is ever replaced by an object
// literal shaped for a face, this file has stopped testing the contract.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { createMembrane } from '../src/index.mjs';
import { OUTCOME, PROBLEM_MEDIA_TYPE, JSON_MEDIA_TYPE } from '../src/wire.mjs';

const ORIGIN = 'http://127.0.0.1:8787';
const TOKEN = 'test-token';
const ACTOR = { Human: { key: 'k1' } };

// --- raw wire fixtures (bytes as the server writes them) ---------------------

const WIRE = {
  // crates/gx-api/src/list.rs:57-70 (limit/cursor) + `{items, next_cursor}`.
  candidatesPage1:
    '{"items":[{"transformation":"t1","state":"Admitted","verdict":"Admit","enforced":true,'
    + '"created_at":"2026-08-24T00:00:00Z","actor":{"Human":{"key":"k1"}},"scope":"s1"}],'
    + '"next_cursor":"c2"}',
  candidatesPage2:
    '{"items":[{"transformation":"t2","state":"Denied","verdict":"Deny","enforced":false,'
    + '"created_at":"2026-08-24T00:00:01Z","actor":{"Process":{"key":"k2"}},"scope":"s2"}],'
    + '"next_cursor":null}',
  // crates/gx-api/src/verdict_checkpoints.rs:249-253 — integer cursor and `total`.
  checkpointPage: '{"items":[{"window_end":7}],"next_cursor":3,"total":9}',
  // crates/gx-api/src/problem.rs:335-342 + gx_code.rs:1111-1115 (type URI).
  conflict:
    '{"type":"https://glovrex.dev/errors/idempotency-conflict","title":"idempotency conflict",'
    + '"status":409,"detail":"this idempotency key was used for a different request",'
    + '"gx_code":"IDEMPOTENCY_CONFLICT"}',
  unauthorized:
    '{"type":"https://glovrex.dev/errors/unauthorized","title":"unauthorized","status":401,'
    + '"detail":"","gx_code":"UNAUTHORIZED"}',
  // crates/gx-api/src/list.rs — `consistent` is this deployment recomputing its own root.
  consistency:
    '{"old_size":2,"new_size":9,"path":["h1","h2"],"consistent":true,"checked_from":2,"checked_to":9}',
  health: '{"status":"ok","engine_version":"0.1.0"}',
  truncated: '{"items":[{"transformation":"t1"',
  gatewayHtml: '<html><body>502 Bad Gateway</body></html>',
};

// --- a fetch double that records what left the membrane ----------------------

function recorder(replies) {
  const calls = [];
  let n = 0;
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    const reply = typeof replies === 'function' ? replies(n, url, init) : replies[Math.min(n, replies.length - 1)];
    n += 1;
    if (reply instanceof Error) throw reply;
    const { body, status = 200, type = JSON_MEDIA_TYPE } = reply;
    return new Response(body, { status, headers: { 'content-type': type } });
  };
  return { calls, fetchImpl };
}

function membraneWith(replies) {
  const { calls, fetchImpl } = recorder(replies);
  const m = createMembrane({ origin: ORIGIN, token: TOKEN, actor: ACTOR, fetchImpl });
  return { calls, ...m };
}

// --- C1: the answered road keeps every wire field ---------------------------

test('C1 a list page arrives with every wire field intact (AC-M7)', async () => {
  const { port } = membraneWith([{ body: WIRE.candidatesPage1 }]);
  const got = await port.get_candidates({ query: { limit: 1 } });
  assert.equal(got.outcome, OUTCOME.ANSWERED);
  const wire = JSON.parse(WIRE.candidatesPage1);
  assert.deepEqual(got.body, wire);
  for (const key of Object.keys(wire)) assert.ok(key in got.body, `dropped ${key}`);
  for (const key of Object.keys(wire.items[0])) assert.ok(key in got.body.items[0], `dropped items[].${key}`);
});

test('C1b the checkpoint page keeps its integer cursor and its own `total` member', async () => {
  const { port } = membraneWith([{ body: WIRE.checkpointPage }]);
  const got = await port.get_verdict_checkpoints({});
  assert.equal(got.outcome, OUTCOME.ANSWERED);
  assert.equal(got.body.next_cursor, 3);
  assert.equal(got.body.total, 9);
});

// --- C2: the three error roads are three, and none of them is success -------

test('C2 problem+json is `refused`, and the whole problem document survives (AC-M9)', async () => {
  const { port } = membraneWith([{ body: WIRE.conflict, status: 409, type: PROBLEM_MEDIA_TYPE }]);
  const got = await port.post_candidates_id_commit({ params: { id: 't1' } });
  assert.equal(got.outcome, OUTCOME.REFUSED);
  assert.equal(got.gx_code, 'IDEMPOTENCY_CONFLICT');
  assert.deepEqual(got.problem, JSON.parse(WIRE.conflict));
});

test('C2b 401 is a refusal, not a failure', async () => {
  const { port } = membraneWith([{ body: WIRE.unauthorized, status: 401, type: PROBLEM_MEDIA_TYPE }]);
  const got = await port.get_candidates({});
  assert.equal(got.outcome, OUTCOME.REFUSED);
  assert.equal(got.gx_code, 'UNAUTHORIZED');
});

test('C2c an HTTP error that is not problem+json is `failed`, never `refused`', async () => {
  const { port } = membraneWith([{ body: WIRE.gatewayHtml, status: 502, type: 'text/html' }]);
  const got = await port.get_candidates({});
  assert.equal(got.outcome, OUTCOME.FAILED);
  assert.equal(got.reason, 'unexpected_media_type');
  assert.equal(got.status, 502);
});

test('C2d a transport throw is `failed`', async () => {
  const { port } = membraneWith([new Error('ECONNREFUSED')]);
  const got = await port.get_healthz({});
  assert.equal(got.outcome, OUTCOME.FAILED);
  assert.equal(got.reason, 'transport');
  assert.equal(got.status, null);
});

test('C2e a 200 with a truncated body does NOT wear the face of success (AC-M11)', async () => {
  const { port } = membraneWith([{ body: WIRE.truncated }]);
  const got = await port.get_candidates({});
  assert.equal(got.outcome, OUTCOME.FAILED);
  assert.equal(got.reason, 'undecodable');
  assert.notEqual(got.outcome, OUTCOME.ANSWERED);
});

test('C2f a verb+path pair outside the table is `absent`, and no request leaves', async () => {
  const { port, calls } = membraneWith([{ body: WIRE.health }]);
  // The reference tree carried a method for GET /candidates/{id}/escalation, a route
  // that has never existed (req/01a §4-7). Asking for it must not reach the network.
  const got = await port.direct({ verb: 'GET', path: '/candidates/{id}/escalation', params: { id: 't1' } });
  assert.equal(got.outcome, OUTCOME.ABSENT);
  assert.equal(calls.length, 0);
});

// --- C3: the wire is addressed under /v1 -----------------------------------

test('C3 every request goes under the base path the crate nests on (lib.rs:80)', async () => {
  const { port, calls } = membraneWith([{ body: WIRE.health }, { body: WIRE.candidatesPage2 }]);
  await port.get_healthz({});
  await port.get_candidates({});
  assert.equal(calls.length, 2);
  for (const c of calls) assert.ok(c.url.startsWith(`${ORIGIN}/v1/`), `not under /v1: ${c.url}`);
});

test('C3b an id is escaped where the path is built, so a crafted id cannot change the route', async () => {
  const { port, calls } = membraneWith([{ body: '{"transformation":"t"}' }]);
  await port.get_candidates_id({ params: { id: '../ledger/proof?x=1' } });
  assert.equal(calls[0].url, `${ORIGIN}/v1/candidates/..%2Fledger%2Fproof%3Fx%3D1`);
});

// --- C4: identity is carried by the port, not by the caller -----------------

test('C4 the Bearer header is on every route but /healthz (auth.rs:166,180)', async () => {
  const { port, calls } = membraneWith([{ body: WIRE.health }, { body: WIRE.candidatesPage2 }]);
  await port.get_healthz({});
  await port.get_candidates({});
  assert.equal(calls[0].init.headers.authorization, undefined);
  assert.equal(calls[1].init.headers.authorization, `Bearer ${TOKEN}`);
});

test('C4b the actor is attached by the membrane and cannot be spoken by the caller (P-09)', async () => {
  const { port, calls } = membraneWith([{ body: '{"transformation":"t1","state":{"Aborted":"OwnerCancelled"}}' }]);
  await port.post_candidates_id_cancel({ params: { id: 't1' } });
  assert.deepEqual(JSON.parse(calls[0].init.body).actor, ACTOR);
  await assert.rejects(
    () => port.post_candidates_id_cancel({ params: { id: 't1' }, body: { actor: { Human: { key: 'someone-else' } } } }),
    /actor/,
  );
});

// --- C5: idempotency ---------------------------------------------------------

test('C5 the key is stable across two identical calls and absent elsewhere (AC-M8)', async () => {
  const { port, calls } = membraneWith([{ body: '{"issued_at":"x"}' }]);
  await port.post_candidates_id_commit({ params: { id: 't1' } });
  await port.post_candidates_id_commit({ params: { id: 't1' } });
  await port.post_candidates_id_verify({ params: { id: 't1' } });
  const [a, b, c] = calls.map((x) => x.init.headers['idempotency-key']);
  assert.ok(a, 'commit carries a key');
  assert.equal(a, b);
  assert.equal(c, undefined, 'verify is not one of the two routes the crate caches');
  assert.ok(!/\d{13}/.test(a), 'no millisecond clock in the key');
});

test('C5b a different row gets a different key', async () => {
  const { port, calls } = membraneWith([{ body: '{}' }]);
  await port.post_transformations_id_undo({ params: { id: 't1' } });
  await port.post_transformations_id_undo({ params: { id: 't2' } });
  assert.notEqual(calls[0].init.headers['idempotency-key'], calls[1].init.headers['idempotency-key']);
});

// --- C6: the membrane states no verdict of its own --------------------------

test('C6 `consistent` is carried, never re-spelled as verified (AC-M10)', async () => {
  const { port } = membraneWith([{ body: WIRE.consistency }]);
  const got = await port.get_ledger_consistency({ query: { from: 2, to: 9 } });
  assert.equal(got.body.consistent, true);
  assert.ok(!('verified' in got.body));
  assert.ok(!('valid' in got));
});

test('C6b a lower-cased verdict is passed through unchanged and is not the frozen spelling', async () => {
  const { port } = membraneWith([{ body: '{"items":[{"verdict":"admit"}],"next_cursor":null}' }]);
  const got = await port.get_candidates({});
  assert.equal(got.body.items[0].verdict, 'admit', 'the membrane does not repair the wire');
  const { VERDICT_KINDS } = await import('../src/wire.mjs');
  assert.deepEqual(VERDICT_KINDS, ['Admit', 'Deny', 'Escalate']);
  assert.ok(!VERDICT_KINDS.includes('admit'));
});

// --- C7: streaming is not parsed -------------------------------------------

test('C7 the stream route hands back the body unparsed', async () => {
  const { port } = membraneWith([{ body: '{"kind":"a"}\n{"kind":"b"}\n', type: 'application/x-ndjson' }]);
  const got = await port.get_stream({});
  assert.equal(got.outcome, OUTCOME.ANSWERED);
  assert.equal(got.body, null);
  assert.ok(got.stream, 'the caller is handed the byte stream');
});

// --- C8: paging reports its own denominator ---------------------------------

test('C8 folding pages returns the items and how many requests it took (P-05)', async () => {
  const { port } = membraneWith([{ body: WIRE.candidatesPage1 }, { body: WIRE.candidatesPage2 }]);
  const got = await port.fold('get_candidates', {});
  assert.equal(got.outcome, OUTCOME.ANSWERED);
  assert.equal(got.items.length, 2);
  assert.equal(got.requests, 2);
  assert.equal(got.stopped_at_budget, false);
  assert.equal(got.repeated_cursor, false);
});

test('C8b a server that repeats a cursor is stopped and says so', async () => {
  const { port } = membraneWith(() => ({ body: WIRE.candidatesPage1 })); // always next_cursor "c2"
  const got = await port.fold('get_candidates', {});
  assert.equal(got.repeated_cursor, true);
  assert.ok(got.requests <= 3, `stopped early, took ${got.requests}`);
});

test('C8c a failure mid-fold is returned as the failure, not as a short page', async () => {
  const { port } = membraneWith([{ body: WIRE.candidatesPage1 }, { body: WIRE.gatewayHtml, status: 502, type: 'text/html' }]);
  const got = await port.fold('get_candidates', {});
  assert.equal(got.outcome, OUTCOME.FAILED);
  assert.ok(!('items' in got), 'a partial page must not be handed over as a page');
});

// --- C9: the watch sees every call ------------------------------------------

test('C9 the notice ledger counts exactly the calls that were made (AC-M6)', async () => {
  const { port, notices } = membraneWith([{ body: WIRE.health }]);
  await port.get_healthz({});
  await port.get_healthz({});
  await port.direct({ verb: 'DELETE', path: '/nope' });
  assert.equal(notices.length, 3);
  assert.deepEqual(notices.map((n) => n.outcome), ['answered', 'answered', 'absent']);
  assert.deepEqual(notices.map((n) => n.seq), [1, 2, 3]);
});
