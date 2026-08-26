// SPDX-License-Identifier: Apache-2.0
// The only entry point that is allowed to say a word about the whole tree.
//
// Stages run in a fixed order and the report is one report. Nothing else in this
// directory prints a verdict, because four instruments with four verdicts is four
// chances to run one of them and call it done.
//
//   node tools/verify-all.mjs [--repeat N] [--json <path>]
//
// Exit codes are the state machine in rig/verdict.mjs. 0 is the only green.

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import { spawnSync } from 'node:child_process';
import { buildManifest, environmentDigest } from './rig/manifest.mjs';
import { buildAcLedger, coverage } from './rig/ac_ledger.mjs';
import { runCatalogue } from './rig/runner.mjs';
import { assemble, headline } from './rig/report.mjs';
import { buildCatalogue } from './assays.mjs';
import { startRenderer, findRenderer } from './rig/renderer.mjs';
import { EXIT, OUTCOME, VERDICT } from './rig/verdict.mjs';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
// req/06 §11 states its own denominator in words: "本fileがgateする物: req/01 AC-M0-M12 /
// req/02 W1-W15 / req/03 AC-F0-F5 / req/04 AC-P0- の全AC行". Until this repair the
// coverage denominator was req/06's own 39 AC-I* rows only, so a reader of the headline
// saw instrument-completeness in the place they would read product-completeness. Widened
// to the four product reqdefs plus this instrument's own file, matching §11 verbatim, and
// declared on the headline (see report.mjs) so no future run can grade its own paper
// without saying so out loud.
const AC_SOURCES = [
  'req/01_MEMBRANE.md',
  'req/02_SHELL_WLAYER.md',
  'req/03_FACES_REBUILD.md',
  'req/04_PARTS_REBUILD.md',
  'req/06_INSTRUMENTS.md',
];
const BUDGETS = { static: 5000, total: 180000 };
const VIEWPORT = { width: 720, height: 320 };

const argv = process.argv.slice(2);
const repeat = Number(argv[argv.indexOf('--repeat') + 1]) || 1;
const jsonAt = argv.includes('--json') ? argv[argv.indexOf('--json') + 1] : path.join(ROOT, '.run', 'report.json');
const argOf = (name, fallback) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : fallback);

/**
 * Did this run reach a real engine? Asked, not assumed.
 *
 *   node tools/verify-all.mjs --bed http://127.0.0.1:8795
 *
 * The one route outside the engine's auth guard is its health road, so this needs no
 * token and cannot be made to pass by holding one. A body that does not carry the
 * engine's own health shape is not an engine answering, however encouraging its status
 * code -- the same "an HTTP 200 is not evidence of what answered" discipline
 * `membrane/src/transport.mjs` applies to every other call this application makes.
 */
async function reachedEngine(origin) {
  if (!origin) return false;
  try {
    const answer = await fetch(`${String(origin).replace(/\/+$/, '')}/v1/healthz`, { signal: AbortSignal.timeout(4000) });
    if (!answer.ok) return false;
    const body = await answer.json();
    return typeof body?.status === 'string' && typeof body?.engine_version === 'string';
  } catch {
    return false;
  }
}

/**
 * Did a real window, bound to that engine, actually get driven? Asked, not assumed —
 * the same discipline as reachedEngine above, one layer up.
 *
 *   node tools/verify-all.mjs --bed http://127.0.0.1:8821 --window http://127.0.0.1:8815
 *
 * The instrument is `shell/tools/bound_smoke.mjs --expect bound` — five checks that
 * read the membrane's own notice ledger, the engine's bytes on the port and the
 * console, none of which a drawing can fake. Only its exit code is believed: no
 * origin named, a smoke that fails, a smoke that cannot start — all stay false,
 * because a run that never drove a bound window cannot claim one (req/822_c5 §4
 * named this bit as the last hard-coded `false`; req/822_c6 makes it a measurement).
 */
function windowDriven(origin) {
  if (!origin) return false;
  const run = spawnSync(
    process.execPath,
    [path.join(ROOT, 'shell', 'tools', 'bound_smoke.mjs'), '--origin', origin, '--expect', 'bound'],
    { encoding: 'utf8', timeout: 120000 },
  );
  return run.status === 0;
}

// Stage zero. If the harness cannot notice being broken, the rest of this file is
// producing numbers about a tree using an instrument nobody has reason to believe,
// and the honest thing to print is that fact rather than the numbers.
function verifySelf() {
  const started = Date.now();
  const run = spawnSync(process.execPath, [path.join(HERE, 'verify-self.mjs')], { encoding: 'utf8' });
  const landed = /landed (\d+)  inert-or-blank (\d+)/.exec(run.stdout ?? '');
  return {
    blind: run.status !== 0,
    rounds: landed ? Number(landed[1]) + Number(landed[2]) : 0,
    inert: landed ? Number(landed[2]) : null,
    ms: Date.now() - started,
    tail: (run.stdout ?? '').trim().split('\n').slice(-2).join(' | '),
  };
}

async function oneRun() {
  const started = Date.now();
  const timings = {};

  const self = verifySelf();
  timings.self = self.ms;

  const manifest = buildManifest(ROOT);
  const patterns = JSON.parse(manifest.at('tools/rig/lint-patterns.json').text);
  const faces = JSON.parse(manifest.at('tools/faces.json').text).faces;
  const baselineEntry = manifest.at('tools/baselines/LEDGER.json');
  const baselines = baselineEntry ? JSON.parse(baselineEntry.text).baselines : {};

  const rendererBinary = findRenderer();
  const present = new Set();
  if (rendererBinary) present.add('renderer');
  if (Object.keys(baselines).length > 0) present.add('baselines');
  // Whether this run reached a real server is now measured rather than declared. It was
  // a hardcoded `false` carrying the note "no real server is stood up yet", which was
  // true when it was written and stopped being true the day the membrane was bound into
  // a window (req/803 gap 1): a bed can be named on the command line and this asks it.
  //
  // It asks, and it believes only an answer. No bed named, no answer, a body that is not
  // the engine's health shape -- all three stay false, because the whole worth of this
  // flag is that a run which never touched an engine cannot call itself green.
  const wire = await reachedEngine(argOf('--bed', null));
  // Measured after wire and independently of it: a bed can stand with no window served
  // over it, and a window origin with no live bed behind it fails the smoke's own B3/B4.
  const windowBit = windowDriven(argOf('--window', null));

  let renderer = null;
  if (present.has('renderer')) renderer = await startRenderer({ viewport: VIEWPORT });

  const world = {
    manifest,
    patterns,
    faces,
    renderer,
    faceUrl: (face) => url.pathToFileURL(path.join(ROOT, 'tools', face.source)).href,
    baselineFor: (id) => baselines[id] ?? null,
    async openFace(face) {
      const page = await renderer.openPage();
      await page.open(world.faceUrl(face));
      return page;
    },
  };

  const catalogue = buildCatalogue();
  const staticStarted = Date.now();
  let all;
  try {
    all = await runCatalogue(catalogue, world, { present });
  } finally {
    // A renderer left running holds the process open, and a run that cannot exit is
    // a run nobody ever sees the numbers from.
    if (renderer) await renderer.stop();
  }
  timings.static = all.filter((r) => r.tier === 'T0' || r.tier === 'T1').reduce((sum, r) => sum + r.ms, 0);
  timings.staticWall = Date.now() - staticStarted;

  const ledger = buildAcLedger(manifest, AC_SOURCES);
  const acLedger = coverage(ledger, catalogue.all());

  // Built again, so a run whose tree moved underneath it says so rather than
  // averaging two trees into one number.
  const after = buildManifest(ROOT);
  const treeStable = after.treeDigest === manifest.treeDigest;
  const treeMovement = treeStable ? [] : diffManifests(manifest, after);

  timings.total = Date.now() - started;
  const environment = environmentDigest({
    renderer: renderer ? renderer.product : 'absent',
    viewport: `${VIEWPORT.width}x${VIEWPORT.height}`,
  });

  const { body, exit } = assemble({
    results: all.filter((r) => r.tier !== 'T0'),
    lint: all.filter((r) => r.tier === 'T0'),
    acLedger,
    // Handed to assemble() rather than bolted onto body afterwards. It used to be set at
    // the bottom of this function, which is after the outcome had already been decided,
    // so the one fact that invalidates the coverage number could not reach the verdict
    // that quoted it (req/883).
    missingAcSources: ledger.missingSources,
    tree: manifest.treeDigest,
    environment: environment.digest,
    wire,
    window: windowBit,
    treeStable,
    selfBlind: self.blind,
    timings,
    budgets: BUDGETS,
  });
  body.selfCheck = self;
  body.environmentFacts = environment.facts;
  body.acSources = AC_SOURCES;
  body.criteriaClaimedButNotWritten = acLedger.claimsNothingWritten;
  body.treeMovement = treeMovement;
  return { body, exit, all };
}

// A tree that moved is only useful as a finding if it says what moved.
function diffManifests(before, after) {
  const b = new Map(before.files.map((f) => [f.path, f.digest]));
  const a = new Map(after.files.map((f) => [f.path, f.digest]));
  const moved = [];
  for (const [p, d] of b) {
    if (!a.has(p)) moved.push({ path: p, how: 'removed during the run' });
    else if (a.get(p) !== d) moved.push({ path: p, how: `content changed ${d} -> ${a.get(p)}` });
  }
  for (const [p] of a) if (!b.has(p)) moved.push({ path: p, how: 'appeared during the run' });
  return moved;
}

const runs = [];
for (let i = 0; i < repeat; i += 1) runs.push(await oneRun());

let { body, exit, all } = runs[0];

// Repeat is not an average. A reading that answered differently across runs is
// named, and the rest keep their answers.
if (repeat > 1) {
  const unstable = [];
  for (const reading of body.readings) {
    const answers = new Set(runs.map((r) => r.body.readings.find((x) => x.id === reading.id)?.verdict));
    if (answers.size > 1) unstable.push({ id: reading.id, answers: [...answers] });
  }
  for (const u of unstable) {
    const reading = body.readings.find((x) => x.id === u.id);
    reading.verdict = VERDICT.FLAKY;
    reading.note = `answered ${u.answers.join(' and ')} across ${repeat} runs`;
  }
  const reassembled = assemble({
    results: all.filter((r) => r.tier !== 'T0').map((r) => (unstable.some((u) => u.id === r.id) ? { ...r, verdict: VERDICT.FLAKY } : r)),
    lint: all.filter((r) => r.tier === 'T0'),
    acLedger: { total: body.coverage.total, backed: body.coverage.backed, unbacked: body.coverage.unbacked },
    // Carried into the second assemble too. Every other field in this block was, and
    // this one was not, so `--repeat 2` silently dropped the absent-sources finding and
    // reported a coverage number the single-run path would have refused (req/883).
    missingAcSources: body.missingAcSources ?? [],
    tree: body.tree, environment: body.environment, wire: body.wire, window: body.window,
    treeStable: runs.every((r) => r.body.outcome !== OUTCOME.NON_CANONICAL),
    timings: body.timings, budgets: BUDGETS,
  });
  reassembled.body.repeat = { runs: repeat, digests: runs.map((r) => r.body.digest), unstable };
  // Carried across, because a headline that says the self-check did not run when it
  // ran on every pass is a false statement about evidence, safe direction or not.
  reassembled.body.selfCheck = runs[0].body.selfCheck;
  reassembled.body.treeMovement = runs.flatMap((r) => r.body.treeMovement ?? []);
  reassembled.body.environmentFacts = runs[0].body.environmentFacts;
  reassembled.body.criteriaClaimedButNotWritten = runs[0].body.criteriaClaimedButNotWritten;
  reassembled.body.acSources = runs[0].body.acSources;
  ({ body, exit } = reassembled);
}

fs.mkdirSync(path.dirname(jsonAt), { recursive: true });
fs.writeFileSync(jsonAt, `${JSON.stringify(body, null, 2)}\n`);

console.log(headline(body));
for (const reading of body.readings.concat(body.lintReadings)) {
  if (reading.verdict === 'PASS') continue;
  console.log(`\n  ${reading.verdict.padEnd(5)} ${reading.id}  n=${reading.population}  ${reading.note}`);
  for (const failure of reading.failures ?? []) console.log(`        - ${failure.member}: ${failure.why}`);
}
if (body.treeMovement?.length) {
  console.log(`\n  the tree moved while the run was in progress (${body.treeMovement.length}):`);
  for (const m of body.treeMovement) console.log(`        - ${m.path}: ${m.how}`);
}
if (body.coverage.unbacked.length) {
  console.log(`\n  unbacked acceptance criteria (${body.coverage.unbacked.length}): ${body.coverage.unbacked.join(' ')}`);
}
if (body.criteriaClaimedButNotWritten.length) {
  console.log(`  readings standing behind criteria written nowhere: ${body.criteriaClaimedButNotWritten.join(' ')}`);
}
console.log(`\n  report written to ${path.relative(ROOT, jsonAt)}  (citable as evidence: ${body.citableAsEvidence})`);

process.exitCode = exit;
