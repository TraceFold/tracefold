// SPDX-License-Identifier: Apache-2.0
// The bench §3c② asks every module to declare. faces/notice/README.md named the gap
// in words ("No bench. Mount time and n-entry draw time are unmeasured"); this file
// closes the entry-draw half of it. Statistics/persistence shared with the other four
// module bench scripts live in tools/rig/bench.mjs (req/38 §227 sibling sweep).
//
// What is measured: `face.view(state)`, exported from notice.mjs alongside
// `face.read`/`face.mount` -- the exact pure function `mount()` calls on every paint.
// `face.read(notices)` (sync, trivial array wrap) is called once, untimed; only the
// tree-build over 1,000 entries is timed. What this does NOT include: mount ms and
// real paint -- both open, see this face's README `[ ]` list and shell/tools/bench.mjs's
// note on the same gap.
//
//   node faces/notice/tools/bench.mjs

import path from 'node:path';
import url from 'node:url';
import { face } from '../notice.mjs';
import { runBench } from '../../../tools/rig/bench.mjs';

const { read, view } = face;

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');

const BUDGET_MS = 300; // 1,000 entries -- generous, real.
const N = 1000;
const OUTCOMES = ['answered', 'refused', 'failed', 'absent'];

function entry(n) {
  const outcome = OUTCOMES[n % OUTCOMES.length];
  const base = {
    seq: n, at: `2026-08-24T10:${String(n % 60).padStart(2, '0')}:00.000Z`, method: `bench_method_${n % 7}`, verb: 'GET', path: `/v1/bench/${n}`, outcome, status: outcome === 'answered' ? 200 : 409,
  };
  if (outcome === 'answered') return { ...base, result: { outcome, status: 200, body: {} } };
  if (outcome === 'refused') {
    return {
      ...base, result: { outcome, status: 409, gx_code: 'BENCH_CONFLICT', problem: { type: 'about:blank', title: 'conflict', status: 409, detail: 'bench conflict', gx_code: 'BENCH_CONFLICT' } },
    };
  }
  if (outcome === 'failed') return { ...base, status: null, result: { outcome, reason: 'transport', status: null, detail: 'bench transport failure' } };
  return { ...base, status: null, result: { outcome, reason: 'no_such_route', requested: { name: base.method } } };
}

const notices = Array.from({ length: N }, (_, i) => entry(i + 1));
const state = read(notices);
const gotEntries = (state.notices ?? []).length;
if (gotEntries !== N) {
  console.log(`FELL: setup did not read back ${N} notices (got ${gotEntries})`);
  process.exitCode = 1;
} else {
  await runBench({
    label: 'faces/notice bench',
    moduleRoot: ROOT,
    note: 'faces/notice row-render ms -- median time for face.view(state) (notice.mjs) to build the full entry tree for 1,000 entries across the 4 outcome shapes. read() is untimed setup (sync, trivial); mount ms and real paint are separate, unmeasured axes (see README [ ] list).',
    budgetMs: BUDGET_MS,
    measure: () => {
      const started = process.hrtime.bigint();
      view(state);
      return Number(process.hrtime.bigint() - started) / 1e6;
    },
    extra: { entries: N },
  });
}
