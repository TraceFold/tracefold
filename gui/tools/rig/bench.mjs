// SPDX-License-Identifier: Apache-2.0
// The one place "median of N, persist, hard-red on budget" is written.
//
// Five modules (membrane/shell/parts/faces-ledger/faces-notice) each needed a bench
// script for the five-principles repair (app req/98 V-1). Writing "mkdirSync the
// .bench dir, writeFileSync report.json, sort samples, take the median, compare to a
// budget, set the exit code" five times produced five near-identical bodies that
// membrane/test/discipline.test.mjs's D5 copy-gate correctly flagged as an 8-name run
// shared with an external reference tree -- a real instance of the sibling-sweep
// doctrine (req/38 §227: "the same problem answered separately in several places is
// answered once, badly, N times"). This file is the one answer; every module's
// bench.mjs supplies only what is actually different about it: the measured function.

import fs from 'node:fs';
import path from 'node:path';

/**
 * @param {object} options
 * @param {string} options.label        printed on the headline, e.g. "membrane bench"
 * @param {string} options.moduleRoot   directory this module's .bench/report.json lands in
 * @param {string} options.note         what is measured and what is deliberately excluded
 * @param {number} options.budgetMs     a hard-red line, not a warning (mirrors rig/report.mjs:39)
 * @param {number} [options.samples]    default 5, per app req/98's "median of 5"
 * @param {() => number | Promise<number>} options.measure  runs the thing once, returns elapsed ms (sync or async)
 * @param {object} [options.extra]      extra fields folded into the persisted report (rows counted, etc.)
 */
export async function runBench({
  label, moduleRoot, note, budgetMs, samples = 5, measure, extra = {},
}) {
  const readings = [];
  for (let i = 0; i < samples; i += 1) readings.push(await measure());
  readings.sort((a, b) => a - b);
  const medianMs = readings[Math.floor(samples / 2)];
  const ok = medianMs <= budgetMs;

  const report = {
    note,
    measuredAt: new Date().toISOString(),
    ...extra,
    samplesMs: readings,
    medianMs,
    budgetMs,
    ok,
  };

  const at = path.join(moduleRoot, '.bench', 'report.json');
  fs.mkdirSync(path.dirname(at), { recursive: true });
  fs.writeFileSync(at, `${JSON.stringify(report, null, 2)}\n`);

  console.log(`${label}  samples=${readings.map((s) => s.toFixed(3)).join(',')}ms  median=${medianMs.toFixed(3)}ms  budget=${budgetMs}ms  ${ok ? 'PASS' : 'OVER BUDGET (RED)'}`);
  process.exitCode = ok ? 0 : 1;
  return report;
}
