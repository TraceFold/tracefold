// SPDX-License-Identifier: Apache-2.0
// The membrane against a running server rather than a fixture.
//
//   node tools/smoke_serve.mjs <origin> <token>
//
// Exit 0 only if every check below holds. The checks are deliberately the ones a
// fixture cannot answer: that the base path is required, that a wrong token comes
// back as a refusal and not as a failure, and that the page shapes the table declares
// are the shapes the server writes.

import { createMembrane } from '../src/index.mjs';
import { OUTCOME, VERDICT_KINDS } from '../src/wire.mjs';

const [, , ORIGIN = 'http://127.0.0.1:8791', TOKEN = ''] = process.argv;

const results = [];
const check = (id, ok, detail) => {
  results.push({ id, ok: Boolean(ok), detail });
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${id}  ${detail}`);
};

const actor = { Human: { key: 'ed25519-smoke' } };
// An independent count of what left the process, so the notice ledger is compared
// against something other than itself.
const onTheWire = { requests: 0 };
const counting = (...args) => { onTheWire.requests += 1; return globalThis.fetch(...args); };
const { port, notices } = createMembrane({ origin: ORIGIN, token: TOKEN, actor, fetchImpl: counting });
const wrong = createMembrane({ origin: ORIGIN, token: 'not-this-servers-token', actor }).port;

// S1 -- reachability, on the one route outside the guard.
const health = await port.get_healthz({});
check('S1 healthz answers without a token', health.outcome === OUTCOME.ANSWERED && typeof health.body?.status === 'string',
  JSON.stringify(health.body ?? health));

// S2 -- an authorised read.
const candidates = await port.get_candidates({});
check('S2 an authorised list answers', candidates.outcome === OUTCOME.ANSWERED && Array.isArray(candidates.body?.items),
  JSON.stringify(candidates.body ?? candidates));

// S3 -- the wrong token is a refusal, with the engine's own code.
const denied = await wrong.get_candidates({});
check('S3 a wrong token is refused, not failed', denied.outcome === OUTCOME.REFUSED && denied.gx_code === 'UNAUTHORIZED',
  `${denied.outcome} ${denied.gx_code ?? ''} ${denied.status}`);

// S4 -- a row that is not there is a refusal too, and a different code.
const missing = await port.get_candidates_id({ params: { id: 'no-such-transformation' } });
check('S4 an unknown row is refused with its own code', missing.outcome === OUTCOME.REFUSED && missing.gx_code !== 'UNAUTHORIZED',
  `${missing.outcome} ${missing.gx_code ?? ''} ${missing.status}`);

// S5 -- the base path is not decoration. Asked without it, the same address is not there.
// Raw, because the membrane cannot address a route off its own table -- which is the point.
const bare = await fetch(`${ORIGIN}/candidates`, { headers: { authorization: `Bearer ${TOKEN}` } });
check('S5 the same path without /v1 is not served', bare.status === 404, `HTTP ${bare.status} at ${ORIGIN}/candidates`);

// S6 -- folding a list against a real server.
const folded = await port.fold('get_transformations', {});
check('S6 a list folds and reports its own denominator',
  folded.outcome === OUTCOME.ANSWERED && folded.requests >= 1 && folded.stopped_at_budget === false,
  `${folded.items?.length} items in ${folded.requests} request(s)`);

// S7 -- the second cursor flavour, which the table declares as an index.
const checkpoints = await port.get_verdict_checkpoints({});
const shape = checkpoints.body ?? {};
check('S7 the checkpoint page carries `total` and an integer-or-null cursor',
  checkpoints.outcome === OUTCOME.ANSWERED && 'total' in shape
    && (shape.next_cursor === null || Number.isInteger(shape.next_cursor)),
  JSON.stringify(shape));

// S8 -- a write road, with the actor the membrane attaches and no argument for it.
const created = await port.post_candidates({
  body: {
    substrate: 'fs',
    locator: '/tmp/gxapp_smoke/hello.txt',
    goal: 'hello from the membrane smoke',
    // gx-core/src/context.rs:16-32 -- a fieldless variant, so the wire form is its name.
    context: 'Representation',
  },
});
check('S8 a write road is either answered or refused in problem+json, never failed',
  created.outcome === OUTCOME.ANSWERED || created.outcome === OUTCOME.REFUSED,
  `${created.outcome} ${created.gx_code ?? created.status} ${JSON.stringify(created.problem?.detail ?? created.body ?? '').slice(0, 220)}`);

// S9 -- the verdict spelling, read off the wire rather than argued from a type.
// This is the check the casing ruling rests on: the crate's enum carries no serde
// rename, so the wire should hold the variant's own name. Here is the wire saying so.
let verdictNote = 'no candidate was created, so no verdict reached the wire';
let verdictOk = true;
const rowId = created.body?.id ?? created.body?.transformation?.id;
if (created.outcome === OUTCOME.ANSWERED && typeof rowId === 'string') {
  const verified = await port.post_candidates_id_verify({ params: { id: rowId } });
  const seen = verified.body?.verdict;
  verdictOk = VERDICT_KINDS.includes(seen);
  verdictNote = `verify was ${verified.outcome}${verified.gx_code ? ` (${verified.gx_code})` : ''}; `
    + `wire wrote verdict=${JSON.stringify(seen)} state=${JSON.stringify(verified.body?.state)}; `
    + `the frozen spellings are ${VERDICT_KINDS.join('|')}`;
}
check('S9 the verdict on the wire is spelled as the crate declares it', verdictOk, verdictNote);

// S10 -- every call this membrane made is in this membrane's ledger, and the second
// membrane's call (S3) is in its own. Two ports, two ledgers, no leakage between them.
check('S10 the notice ledger counted exactly what left this port', notices.length === onTheWire.requests,
  `${notices.length} notices, ${onTheWire.requests} requests actually sent (S3 belongs to a second membrane, S5 was raw on purpose)`);

const failed = results.filter((r) => !r.ok);
console.log(`\n${results.length - failed.length}/${results.length} checks passed against ${ORIGIN}`);
process.exitCode = failed.length === 0 ? 0 : 1;
