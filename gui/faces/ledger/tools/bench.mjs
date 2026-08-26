// SPDX-License-Identifier: Apache-2.0
// The bench §3c② asks every module to declare. faces/ledger/README.md named the gap in
// words ("No bench... mount ms and 1,000-row draw ms are unmeasured"); this file closes
// the row-draw half of it. Statistics/persistence shared with the other four module
// bench scripts live in tools/rig/bench.mjs (req/38 §227 sibling sweep).
//
// What is measured: `view(state)`, exported from ledger.mjs alongside `read`/`mount`
// -- the exact pure function `mount()` calls on every paint (ledger.mjs
// `paint(view(state))`). It takes an already-read state and returns the element tree,
// touching no port and no document, for 1,000 settled records. `read(port)` (the I/O
// half) is called once, untimed, to build the state; only the tree-build is timed.
// What this does NOT include: mount ms (host attach + first read latency) or paint
// (render() into a real document) -- both open, see this face's README `[ ]` list and
// shell/tools/bench.mjs's note on the same gap.
//
//   node faces/ledger/tools/bench.mjs

import path from 'node:path';
import url from 'node:url';
import { face } from '../ledger.mjs';
import { runBench } from '../../../tools/rig/bench.mjs';

const { read, view } = face;

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');

const BUDGET_MS = 300; // 1,000 settled rows, full section tree -- generous, real.
const N = 1000;

function transformation(n) {
  return {
    id: `t-${String(n).padStart(4, '0')}`,
    sequence: n,
    prev: n > 1 ? `t-${String(n - 1).padStart(4, '0')}` : null,
    at: `2026-08-24T09:${String(n % 60).padStart(2, '0')}:00Z`,
    actor: 'agent:bench',
    effect: 'write',
    verdict: n % 5 === 0 ? 'Deny' : 'Admit',
    path: `/work/report-${n}.md`,
    digest: `a1b2c3d4e5f6${String(n).padStart(4, '0')}`,
    basis: 'exact',
  };
}

function page(items) {
  return {
    outcome: 'answered', items, requests: 1, pages: 1, stopped_at_budget: false, repeated_cursor: false, budget: N,
  };
}

const items = Array.from({ length: N }, (_, i) => transformation(i + 1));
const port = {
  fold: (name) => Promise.resolve(name === 'get_transformations' ? page(items) : page([])),
  get_ledger_consistency: () => Promise.resolve({ outcome: 'answered', status: 200, body: { consistent: true, checked_from: 1, checked_to: N } }),
};

const state = await read(port);
const gotRows = (state.settled?.items ?? []).length;
if (gotRows !== N) {
  console.log(`FELL: setup did not read back ${N} settled rows (got ${gotRows})`);
  process.exitCode = 1;
} else {
  await runBench({
    label: 'faces/ledger bench',
    moduleRoot: ROOT,
    note: 'faces/ledger row-render ms -- median time for view(state) (ledger.mjs, exported alongside read/mount) to build the full section tree for 1,000 settled rows. I/O (read) is untimed setup; mount ms and real paint are separate, unmeasured axes (see README [ ] list).',
    budgetMs: BUDGET_MS,
    measure: () => {
      const started = process.hrtime.bigint();
      view(state);
      return Number(process.hrtime.bigint() - started) / 1e6;
    },
    extra: { rows: N },
  });
}
