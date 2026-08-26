// SPDX-License-Identifier: Apache-2.0
// Live round for AC-I11 (bench-常設): the budget in verify-all.mjs is implemented and
// folded into a hard RED (rig/report.mjs:39), but req/06:280 records it as never
// fired. req/06's own words for the negative control are "insert an artificial sleep,
// see red on budget overage" -- what matters is that a measured stage exceeds its
// budget and the run turns RED, not the specific mechanism used to make it exceed.
// This round lowers the static budget to zero, which every real static stage will
// exceed deterministically (no timing variance to chase), fires the real harness in a
// fresh process, and restores the file byte-for-byte afterward, the same edit/run/
// restore shape tools/probes/tier_red_first.mjs uses for the tier rounds.
//
//   node tools/probes/bench_budget_red_first.mjs

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import { spawnSync } from 'node:child_process';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');
const VERIFY_ALL_PATH = path.join(ROOT, 'tools', 'verify-all.mjs');

const FIND = 'const BUDGETS = { static: 5000, total: 180000 };';
const REPLACE = 'const BUDGETS = { static: 0, total: 180000 };';

function harness() {
  const run = spawnSync(process.execPath, [VERIFY_ALL_PATH], { encoding: 'utf8', cwd: ROOT });
  const report = JSON.parse(fs.readFileSync(path.join(ROOT, '.run', 'report.json'), 'utf8'));
  return { exit: run.status, outcome: report.outcome, overBudget: report.overBudget, stdout: run.stdout };
}

const original = fs.readFileSync(VERIFY_ALL_PATH, 'utf8');
const hits = original.split(FIND).length - 1;
if (hits !== 1) {
  console.log(`FELL: the budget line was not found exactly once (found ${hits}) -- verify-all.mjs moved since this round was written`);
  process.exitCode = 1;
} else {
  const unbroken = harness();
  console.log(`unbroken : exit ${unbroken.exit}  outcome ${unbroken.outcome}  overBudget ${JSON.stringify(unbroken.overBudget)}`);

  fs.writeFileSync(VERIFY_ALL_PATH, original.replace(FIND, REPLACE));
  let broken;
  try {
    broken = harness();
  } finally {
    fs.writeFileSync(VERIFY_ALL_PATH, original);
  }
  const restored = fs.readFileSync(VERIFY_ALL_PATH, 'utf8') === original;
  console.log(`broken   : exit ${broken.exit}  outcome ${broken.outcome}  overBudget ${JSON.stringify(broken.overBudget)}`);
  console.log(`restored : ${restored ? 'yes' : 'NO'}`);

  const turned = broken.outcome === 'RED' && broken.overBudget?.some((o) => o.stage === 'static') && restored;
  console.log(turned
    ? '\nAC-I11 live round: zeroing the static budget turned the run RED with the static stage named in overBudget, and the file came back byte-identical'
    : '\nAC-I11 live round: the budget did not turn the run RED as the AC requires, or the file was not restored');
  process.exitCode = turned ? 0 : 1;
}
