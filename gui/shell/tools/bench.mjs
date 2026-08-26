// SPDX-License-Identifier: Apache-2.0
// The bench §3c② asks every module to declare. Five principles audit V-1 (app req/98)
// found zero timing call sites under shell/; this file closes it for the kernel's own
// mount path. Statistics/persistence shared with the other four module bench scripts
// live in tools/rig/bench.mjs (req/38 §227 sibling sweep).
//
// What is measured: `Mounted.raise()`/`lower()` from kernel/mount.mjs -- the exact
// class kernel/render.mjs's Frame drives on every act (req/02 W8's "one comparison per
// host" path) -- for a representative set of faces across a representative set of
// hosts. That is real kernel dispatch cost: the `changing()` guard, the arity check,
// the tally bookkeeping, raise/lower symmetry. What it does NOT include: a real
// document. `createShell` requires a live `root` element (kernel/shell.mjs:57), and
// standing one up needs either a browser (tools/rig/renderer.mjs, CDP, no npm
// dependency but a heavier process per sample) or a DOM-shim dependency this tree
// carries none of (AC-I10: zero dependencies). Full real-DOM mount timing is the T2/T3
// product-path assay req/98 V-1 asks for structurally and is not built here -- this
// bench is the minimal, honestly-scoped piece: the part of "mount" the kernel itself
// is answerable for, without borrowing a browser to measure it.
//
//   node shell/tools/bench.mjs

import path from 'node:path';
import url from 'node:url';
import { Mounted } from '../kernel/mount.mjs';
import { runBench } from '../../tools/rig/bench.mjs';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');

const BUDGET_MS = 50; // 400 raise/lower pairs across 4 hosts -- generous, real.
const HOSTS = ['left', 'right', 'stage-verify', 'stage-inspect'];
const CYCLES = 100;

function noopFace(id) {
  return { id, mount: () => () => {} };
}

function measureOneRun() {
  const mounted = new Mounted();
  const started = process.hrtime.bigint();
  for (let cycle = 0; cycle < CYCLES; cycle += 1) {
    for (const host of HOSTS) {
      mounted.raise(host, noopFace(`${host}-${cycle}`), {}, null, []);
      mounted.lower(host);
    }
  }
  return Number(process.hrtime.bigint() - started) / 1e6;
}

await runBench({
  label: 'shell bench',
  moduleRoot: ROOT,
  note: 'shell mount ms -- median time for Mounted.raise()/lower() (kernel/mount.mjs) across 4 hosts x 100 cycles = 400 mount/unmount pairs. Kernel dispatch only: no real document, no face body beyond a no-op mount. Full real-DOM mount timing is open work (see note in this file).',
  budgetMs: BUDGET_MS,
  measure: measureOneRun,
  extra: { hosts: HOSTS.length, cyclesPerHost: CYCLES, pairs: HOSTS.length * CYCLES },
});
