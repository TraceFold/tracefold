// SPDX-License-Identifier: Apache-2.0
// The bench every module in this tree declares (req/98 five-principles audit).
// Statistics/persistence delegate to tools/rig/bench.mjs, the shared harness
// extracted after req/38 §227 caught five near-identical inline bench bodies; this
// file supplies only the measured function.
//
// What is measured: `view(state)`, exported from receipt.mjs alongside
// `read`/`mount` -- the exact pure function `mount()` calls on every paint. This
// screen draws one record, not a list, so there is no "1,000 rows" axis the way
// faces/ledger's/faces/held's own bench has one; instead this measures 1,000
// independent calls to view() against 1,000 distinct synthetic states, the same
// unit of work a real window repeats on every re-render. `read(port, id)` (the I/O
// half) is called once per state, untimed, to build it; only the tree-build is
// timed. Mount ms and real paint are separate, unmeasured axes -- the same open
// gap every other face's own bench states.
//
//   node faces/receipt/tools/bench.mjs

import path from 'node:path';
import url from 'node:url';
import { face } from '../receipt.mjs';
import { runBench } from '../../../tools/rig/bench.mjs';

const { read, view } = face;

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');

const BUDGET_MS = 300; // 1,000 independent view(state) calls, full screen tree each.
const N = 1000;

function stateOf(n) {
  const digest = `a1b2c3d4e5f6${String(n).padStart(4, '0')}`;
  const port = {
    get_transformations_id: () => Promise.resolve({
      outcome: 'answered',
      status: 200,
      body: {
        id: `t-${String(n).padStart(4, '0')}`,
        sequence: n,
        prev: n > 1 ? `t-${String(n - 1).padStart(4, '0')}` : null,
        at: `2026-08-24T10:${String(n % 60).padStart(2, '0')}:00Z`,
        actor: 'agent:bench',
        effect: n % 3 === 0 ? 'delete' : 'write',
        verdict: 'Admit',
        path: `/work/report-${n}.md`,
        digest,
      },
    }),
    get_receipts_tid: () => Promise.resolve({
      outcome: 'answered',
      status: 200,
      body: { digest, algorithm: 'sha256', anchor: `https://example.test/anchor/t-${n}`, basis: 'exact' },
    }),
  };
  return { port, id: `t-${String(n).padStart(4, '0')}` };
}

const inputs = Array.from({ length: N }, (_, i) => stateOf(i + 1));
const states = await Promise.all(inputs.map(({ port, id }) => read(port, id)));
const gotStates = states.filter((s) => s.delta?.outcome === 'answered').length;
if (gotStates !== N) {
  console.log(`FELL: setup did not read back ${N} receipt states (got ${gotStates})`);
  process.exitCode = 1;
} else {
  let i = 0;
  await runBench({
    label: 'faces/receipt bench',
    moduleRoot: ROOT,
    note: 'faces/receipt row-render ms -- median time for view(state) (receipt.mjs, exported alongside read/mount) to build the full screen tree for one receipt state, cycled across 1,000 distinct synthetic states. I/O (read) is untimed setup; mount ms and real paint are separate, unmeasured axes.',
    budgetMs: BUDGET_MS,
    measure: () => {
      const state = states[i % N];
      i += 1;
      const started = process.hrtime.bigint();
      view(state);
      return Number(process.hrtime.bigint() - started) / 1e6;
    },
    extra: { states: N },
  });
}
