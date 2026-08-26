// SPDX-License-Identifier: Apache-2.0
// The bench every module in this tree declares (req/98 five-principles audit).
// Statistics/persistence delegate to tools/rig/bench.mjs, the shared harness
// extracted after req/38 §227 caught five near-identical inline bench bodies; this
// file supplies only the measured function.
//
// What is measured: `view(state)`, exported from graph.mjs alongside `read`/`mount`
// -- the exact pure function `mount()` calls on every paint. Unlike faces/receipt's
// single-record bench, this face's `view()` does real grouping/ordering/edge-
// resolution work over a population, so the axis this bench moves across 1,000
// calls is population size: each call is handed a distinct, independently-built
// state of 60 transformations across 20 paths (3 touches each, so every path is a
// graph subject and every touch after the first exercises the edge-resolution path).
// `read(port)` (the I/O half) is called once per state, untimed, to build it; only
// the tree-build is timed. Mount ms and real paint are separate, unmeasured axes --
// the same open gap every other face's own bench states.
//
//   node faces/graph/tools/bench.mjs

import path from 'node:path';
import url from 'node:url';
import { face } from '../graph.mjs';
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
    let prev = null;
    for (let t = 0; t < TOUCHES_PER_PATH; t += 1) {
      const id = `t-${stateIndex}-${p}-${t}`;
      items.push({
        id, sequence: seq, prev, at: `2026-08-24T10:${String(seq % 60).padStart(2, '0')}:00Z`,
        actor: 'agent:bench', effect: seq % 5 === 0 ? 'delete' : 'write', verdict: 'Admit',
        path: `/work/report-${p}.md`, digest: `bench${String(stateIndex).padStart(4, '0')}${String(seq).padStart(3, '0')}`,
      });
      prev = id;
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
  console.log(`FELL: setup did not read back ${N} graph states of ${PATHS * TOUCHES_PER_PATH} items each (got ${gotStates})`);
  process.exitCode = 1;
} else {
  let i = 0;
  await runBench({
    label: 'faces/graph bench',
    moduleRoot: ROOT,
    note: `faces/graph row-render ms -- median time for view(state) (graph.mjs, exported alongside read/mount) to build the full screen tree for one read of ${PATHS} paths x ${TOUCHES_PER_PATH} touches (${PATHS * TOUCHES_PER_PATH} transformations, every path a graph subject, every non-first touch exercising edge resolution), cycled across 1,000 distinct synthetic states. I/O (read) is untimed setup; mount ms and real paint are separate, unmeasured axes.`,
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
