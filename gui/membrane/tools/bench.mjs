// SPDX-License-Identifier: Apache-2.0
// The bench §3c② asks every module to declare. Five principles audit V-1 (app req/98)
// found zero timing call sites anywhere under membrane/shell/parts/faces -- this file
// is the membrane's answer. Statistics/persistence shared with the other four module
// bench scripts live in tools/rig/bench.mjs (req/38 §227 sibling sweep: five
// near-identical inline bodies tripped membrane/test/discipline.test.mjs's D5 copy
// gate against an external reference tree; the fix was to answer it once, not cite it
// five times).
//
// What is measured: the in-process portion of one request through createMembrane's
// `port` -- URL assembly (address.mjs), header/idempotency-key assembly, body
// serialisation and the notice write -- using a `fetchImpl` that resolves on the next
// microtask instead of opening a socket. That is a real, exercised code path (the same
// `perform()` every caller goes through), not a stand-in measuring nothing; what it
// does NOT include is real network latency, which membrane/test/wire_contract.test.mjs
// and the wire smoke (tools/smoke_serve.mjs) already cover on a different axis. Naming
// that boundary here is the point -- an unlabelled number is the failure this bench
// exists to not repeat.
//
//   node membrane/tools/bench.mjs

import path from 'node:path';
import url from 'node:url';
import { createMembrane } from '../src/membrane.mjs';
import { runBench } from '../../tools/rig/bench.mjs';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');

// A budget wide enough to be a real red line (not a coin-flip on machine noise) and
// narrow enough that a regression that adds a synchronous disk read or a JSON.parse in
// the hot path would trip it. Chosen the same way rig/report.mjs's BUDGETS were: as a
// stated number this file is answerable for, not a tuned-to-pass one.
const BUDGET_MS = 15;

function instantResponse(body) {
  return {
    status: 200,
    headers: { get: (name) => (name.toLowerCase() === 'content-type' ? 'application/json' : null) },
    json: async () => body,
    text: async () => JSON.stringify(body),
  };
}

async function measureOneCall() {
  const membrane = createMembrane({
    origin: 'http://bench.invalid',
    token: 'bench-token',
    fetchImpl: async () => instantResponse({ id: 'bench-1', at: '2026-08-24T00:00:00Z' }),
  });
  const started = process.hrtime.bigint();
  await membrane.port.get_candidates_id({ params: { id: 'bench-1' } });
  return Number(process.hrtime.bigint() - started) / 1e6;
}

await runBench({
  label: 'membrane bench',
  moduleRoot: ROOT,
  note: 'membrane request-path ms -- in-process cost of one createMembrane().port call, mocked transport, real URL/header/idempotency/notice assembly. Excludes real network latency; that is a separate measurement (wire smoke).',
  budgetMs: BUDGET_MS,
  measure: measureOneCall,
  extra: { route: 'GET /candidates/{id}' },
});
