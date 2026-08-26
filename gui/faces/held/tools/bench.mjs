// SPDX-License-Identifier: Apache-2.0
// The bench every module in this tree declares (req/98 five-principles audit).
// Statistics/persistence delegate to tools/rig/bench.mjs, the shared harness
// extracted after req/38 §227 caught five near-identical inline bench bodies; this
// file supplies only the measured function.
//
// What is measured: `view(state)`, exported from held.mjs alongside `read`/`mount`
// -- the exact pure function `mount()` calls on every paint. It takes an already-
// read state and returns the element tree, touching no port and no document, for
// 1,000 held candidates. `read(port)` (the I/O half) is called once, untimed, to
// build the state; only the tree-build is timed. Mount ms and real paint are
// separate, unmeasured axes -- the same open gap faces/ledger's own bench states.
//
//   node faces/held/tools/bench.mjs

import path from 'node:path';
import url from 'node:url';
import { face } from '../held.mjs';
import { runBench } from '../../../tools/rig/bench.mjs';

const { read, view } = face;

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');

const BUDGET_MS = 300; // 1,000 held rows, full screen tree -- generous, real.
const N = 1000;

function candidate(n) {
  return {
    id: `c-${String(n).padStart(4, '0')}`,
    sequence: n,
    at: `2026-08-24T10:${String(n % 60).padStart(2, '0')}:00Z`,
    actor: 'agent:bench',
    effect: n % 3 === 0 ? 'delete' : 'write',
    verdict: 'Escalate',
    path: `/work/pending-${n}.md`,
    digest: `f6e5d4c3b2a1${String(n).padStart(4, '0')}`,
  };
}

function page(items) {
  return {
    outcome: 'answered', items, requests: 1, pages: 1, stopped_at_budget: false, repeated_cursor: false, budget: N,
  };
}

const items = Array.from({ length: N }, (_, i) => candidate(i + 1));
const port = { fold: (name) => Promise.resolve(name === 'get_candidates' ? page(items) : page([])) };

const state = await read(port);
const gotRows = (state.held?.items ?? []).length;
if (gotRows !== N) {
  console.log(`FELL: setup did not read back ${N} held rows (got ${gotRows})`);
  process.exitCode = 1;
} else {
  await runBench({
    label: 'faces/held bench',
    moduleRoot: ROOT,
    note: 'faces/held row-render ms -- median time for view(state) (held.mjs, exported alongside read/mount) to build the full screen tree for 1,000 held candidates. I/O (read) is untimed setup; mount ms and real paint are separate, unmeasured axes.',
    budgetMs: BUDGET_MS,
    measure: () => {
      const started = process.hrtime.bigint();
      view(state);
      return Number(process.hrtime.bigint() - started) / 1e6;
    },
    extra: { rows: N },
  });
}
