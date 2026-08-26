// SPDX-License-Identifier: Apache-2.0
// The bench §3c② asks every module to declare. parts/README.md named the gap in words
// ("bench未計測(5原則②)。1,000行描画のms宣言0件"); this file closes it. Statistics/
// persistence shared with the other four module bench scripts live in
// tools/rig/bench.mjs (req/38 §227 sibling sweep).
//
// What is measured: `row()` from src/receipt-row.mjs building its element tree for
// 1,000 representative records -- the exact function every consumer (faces/ledger)
// calls per row. `row()` never touches `document` (parts/src/element.mjs:1-12: "a part
// returns a tree, it does not touch a document"), so this is real, product-path
// construction cost with no browser and no stand-in. What it does NOT include: paint,
// layout or the `toHtml`/`render` serialisation step, which needs a real renderer and
// is a separate, larger measurement (tools/rig/renderer.mjs already exists for that;
// wiring a T2/T3 assay for it is the structural repair req/98 V-1 asks for and is not
// done here).
//
//   node parts/tools/bench.mjs

import path from 'node:path';
import url from 'node:url';
import { row } from '../src/receipt-row.mjs';
import { runBench } from '../../tools/rig/bench.mjs';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');

const BUDGET_MS = 200; // 1,000 rows, plain tree construction -- generous but real.
const N = 1000;

const EFFECTS = ['create', 'update', 'delete', 'verify'];
const VERDICTS = ['admit', 'deny', 'unknown'];

function records(n) {
  const out = [];
  for (let i = 0; i < n; i += 1) {
    out.push({
      id: `r-${i}`,
      at: '2026-08-24T00:00:00Z',
      actor: 'bench-actor',
      effect: EFFECTS[i % EFFECTS.length],
      verdict: VERDICTS[i % VERDICTS.length],
      digest: 'a'.repeat(64),
      path: `/candidates/${i}`,
    });
  }
  return out;
}

const data = records(N);

function measureOneRun() {
  const started = process.hrtime.bigint();
  for (const record of data) row(record);
  return Number(process.hrtime.bigint() - started) / 1e6;
}

await runBench({
  label: 'parts bench',
  moduleRoot: ROOT,
  note: 'parts row-render ms -- median time to build the element tree (row() from src/receipt-row.mjs, no document, no paint) for 1,000 records. Paint/layout is a separate, unmeasured axis (see note above).',
  budgetMs: BUDGET_MS,
  measure: measureOneRun,
  extra: { rows: N },
});
