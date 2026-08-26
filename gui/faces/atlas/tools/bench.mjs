// SPDX-License-Identifier: Apache-2.0
// The bench every module in this tree declares (req/98 five-principles audit).
// Statistics/persistence delegate to tools/rig/bench.mjs, the shared harness
// extracted after req/38 SS227 caught five near-identical inline bench bodies;
// this file supplies only the measured function.
//
// What is measured: `view(state)`, exported from atlas.mjs alongside `read`/`mount`
// -- the exact pure function `mount()` calls on every paint. Like faces/graph's
// bench (and unlike faces/receipt's single-record one), this face's `view()` does
// real grouping/ordering/fold-decision work over a population, so the axis moved
// across 1,000 calls is population size: each call is handed a distinct,
// independently-built state of 60 transformations across 20 paths (3 touches each).
// `read(port)` (the I/O half) is called once per state, untimed, to build it; only
// the tree-build is timed. Mount ms and real paint are separate, unmeasured axes --
// the same open gap every other face's own bench states.
//
//   node faces/atlas/tools/bench.mjs

import path from 'node:path';
import url from 'node:url';
import { face } from '../atlas.mjs';
import { runBench } from '../../../tools/rig/bench.mjs';

const { read, view } = face;

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');

const BUDGET_MS = 300; // 1,000 independent view(state) calls, 60 transformations across 20 paths each.
const N = 1000;
const PATHS = 20;
const TOUCHES_PER_PATH = 3;

function itemsFor(stateIndex) {
  const items = [];
  let seq = 1;
  for (let p = 0; p < PATHS; p += 1) {
    for (let t = 0; t < TOUCHES_PER_PATH; t += 1) {
      const id = `t-${stateIndex}-${p}-${t}`;
      items.push({
        id, sequence: seq, at: `2026-08-24T10:${String(seq % 60).padStart(2, '0')}:00Z`,
        actor: 'agent:bench', effect: seq % 5 === 0 ? 'delete' : 'write', verdict: 'Admit',
        path: `/work/report-${p}.md`, digest: `a1b2c3d4e5f6${String(stateIndex).padStart(2, '0')}${String(seq).padStart(2, '0')}`,
      });
      seq += 1;
    }
  }
  return items;
}

function stateOf(n) {
  const items = itemsFor(n);
  const port = { fold: () => Promise.resolve({ outcome: 'answered', items, requests: 1, pages: 1, stopped_at_budget: false, repeated_cursor: false, budget: 64 }) };
  return port;
}

const inputs = Array.from({ length: N }, (_, i) => stateOf(i + 1));
const states = await Promise.all(inputs.map((port) => read(port)));
const gotStates = states.filter((s) => s.transformations?.outcome === 'answered' && s.transformations.items.length === PATHS * TOUCHES_PER_PATH).length;
if (gotStates !== N) {
  console.log(`FELL: setup did not read back ${N} atlas states of ${PATHS * TOUCHES_PER_PATH} items each (got ${gotStates})`);
  process.exitCode = 1;
} else {
  let i = 0;
  await runBench({
    label: 'faces/atlas bench',
    moduleRoot: ROOT,
    note: `faces/atlas row-render ms -- median time for view(state) (atlas.mjs, exported alongside read/mount) to build the full screen tree for one read of ${PATHS} paths x ${TOUCHES_PER_PATH} touches (${PATHS * TOUCHES_PER_PATH} transformations, every path a subject), cycled across 1,000 distinct synthetic states. I/O (read) is untimed setup; mount ms and real paint are separate, unmeasured axes.`,
    budgetMs: BUDGET_MS,
    measure: () => {
      const state = states[i % N];
      i += 1;
      const started = process.hrtime.bigint();
      view(state);
      return Number(process.hrtime.bigint() - started) / 1e6;
    },
    extra: { states: N, paths: PATHS, touchesPerPath: TOUCHES_PER_PATH },
  });
}
