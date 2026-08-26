// SPDX-License-Identifier: Apache-2.0
// The census-diff gate for req/99_SEMANTICS_REGISTRY.md: every source file under
// membrane/ shell/ parts/ faces/ tools/ has exactly one row, and every row points at a
// file that still exists. A row that used to point somewhere and now points at nothing
// is a stale claim; a file with no row is an unregistered one -- neither is inferred
// from the other, and a path with two rows is refused rather than silently kept.
//
//   node tools/sem_registry_gate.mjs              -> exit 0 clean, 1 if either list is non-empty
//   node tools/sem_registry_gate.mjs --self-test  -> fires both directions on purpose first (see SELF-TEST below)
//
// Exclusion rule (mirrors req/99_SEMANTICS_REGISTRY.md §1 -- kept here as code so the
// registry's prose and this gate's predicate cannot drift apart silently): PNG
// screenshots, *.gen.mjs / generated/ code, the fixture HTML + entry scripts each
// face's own fixture.mjs/browser-mount-smoke.mjs writes, the measurements/
// browser-mount-smoke run-output JSON and dated smoke logs each shoot.mjs/
// browser-mount-smoke.mjs writes, and the .bench/report.json each tools/bench.mjs
// writes. A file matching none of these rules is a source file and must have a row.

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import { driftState } from './sem_registry_generate.mjs';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
const SCANNED_DIRS = ['membrane', 'shell', 'parts', 'faces', 'tools'];
const REGISTRY_AT = path.join(ROOT, 'req', '99_SEMANTICS_REGISTRY.md');

export const GATE_MESSAGES = {
  MISSING_ROW: 'a source file has no row in the registry',
  STALE_ROW: 'a registry row points at a file that no longer exists',
  DUPLICATE_ROW: 'a path has more than one row',
  JSON_DRIFT: 'docs/sem_registry.json is not FRESH against req/99_SEMANTICS_REGISTRY.md',
  CLEAN: 'every source file has exactly one row, every row points at a file that exists, and the generated JSON is FRESH',
  REGISTRY_ABSENT: 'req/99_SEMANTICS_REGISTRY.md is not in this tree, so this gate measured nothing',
};

/**
 * The four criteria this gate exists to decide. Named as data because when the corpus
 * is absent the honest output is this list under the heading UNMEASURED, and a list
 * that is written twice drifts (req/883).
 */
const CRITERIA = ['MISSING_ROW', 'STALE_ROW', 'DUPLICATE_ROW', 'JSON_DRIFT'];

// Exit codes. 0 and 1 are the two verdicts this gate could always reach; 2 says it
// reached neither. A published tree ships the source files and the generated JSON but
// NOT req/, which is internal -- so the corpus this gate diffs against is simply not
// there. Before req/883 that case was an uncaught ENOENT, and the reader of a fresh
// clone got a stack trace where a verdict belonged. Exiting 0 would have been worse:
// a gate that cannot see its corpus and reports clean is the exact fail-open this
// project refuses elsewhere. It is a third state and it is spelled as one.
const EXIT_CLEAN = 0;
const EXIT_RED = 1;
const EXIT_UNMEASURED = 2;

const EXCLUDE = [
  (p) => p.endsWith('.png'),
  (p) => /\.gen\.mjs$/.test(p),
  (p) => p.includes('/generated/'),
  (p) => /^parts\/fixtures\/[^/]+\.html$/.test(p),
  (p) => /^faces\/(ledger|notice)\/fixtures\/[^/]+\.html$/.test(p),
  (p) => /^faces\/(ledger|notice)\/fixtures\/(browser-mount-entry|mount-entry)\.mjs$/.test(p),
  (p) => /\/shots\/measurements\.json$/.test(p),
  (p) => /\/shots\/browser-mount-smoke\.json$/.test(p),
  (p) => /smoke_.*\.log$/.test(p),
  (p) => /\/\.bench\/report\.json$/.test(p),
  // Local, gitignored operator configuration -- D5's reference-corpus roots (req/883).
  // It is machine-specific and never committed, so it can have no registry row; without
  // this rule the gate reports MISSING for a file it is structurally impossible to
  // register, which is a red that no one can ever clear.
  (p) => p === 'membrane/test/reference-roots.local.json',
];
// Deliberately no exclusion rule for the self-test's own scratch file: MISSING has to
// see it to fire. It is written and removed within one round of --self-test and never
// outlives that round, so it never needs a row -- excluding it here would exempt the
// exact file the MISSING round exists to catch, which is how this gate went blind on
// its own first self-test run (2026-08-24, recorded rather than silently fixed away).

export function isExcluded(relPath) {
  return EXCLUDE.some((rule) => rule(relPath));
}

function walk(dir, out) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else out.push(full);
  }
}

/** Every file under the five scanned directories, minus the excluded categories, sorted. */
export function sourceFiles(root = ROOT) {
  const out = [];
  for (const d of SCANNED_DIRS) {
    const abs = path.join(root, d);
    if (fs.existsSync(abs)) walk(abs, out);
  }
  return out
    .map((f) => path.relative(root, f).split(path.sep).join('/'))
    .filter((rel) => !isExcluded(rel))
    .sort();
}

/** Every path named in a `| \`path\`` registry row, in file order (not de-duplicated -- a duplicate is a finding, not noise to swallow). */
export function registryPaths(text) {
  const out = [];
  for (const line of text.split('\n')) {
    const m = line.match(/^\| `([^`]+)`/);
    if (m) out.push(m[1]);
  }
  return out;
}

export function diff(files, rows) {
  const fileSet = new Set(files);
  const rowSet = new Set(rows);
  const missing = files.filter((f) => !rowSet.has(f)).sort();
  const stale = rows.filter((r) => !fileSet.has(r)).sort();
  const seen = new Set();
  const duplicated = new Set();
  for (const r of rows) { if (seen.has(r)) duplicated.add(r); seen.add(r); }
  return { missing, stale, duplicated: [...duplicated].sort() };
}

function runGate(root = ROOT, registryAt = REGISTRY_AT) {
  const files = sourceFiles(root);
  if (!fs.existsSync(registryAt)) {
    return {
      registryAbsent: true,
      registryAt: path.relative(root, registryAt).split(path.sep).join('/'),
      missing: [], stale: [], duplicated: [],
      fileCount: files.length, rowCount: 0, json: driftState(root),
    };
  }
  const text = fs.readFileSync(registryAt, 'utf8');
  const rows = registryPaths(text);
  const json = driftState(root);
  return { registryAbsent: false, ...diff(files, rows), fileCount: files.length, rowCount: rows.length, json };
}

/** @returns {'CLEAN'|'RED'|'UNMEASURED'} */
function report(result) {
  if (result.registryAbsent) {
    console.log(`sem_registry_gate: ${result.fileCount} source files scanned, but ${result.registryAt} is not in this tree`);
    console.log(`\n  ${GATE_MESSAGES.REGISTRY_ABSENT}.`);
    console.log('  the following criteria go UNMEASURED in this run -- none of them passed, and none of them failed:');
    for (const c of CRITERIA) console.log(`        - ${c}: ${GATE_MESSAGES[c]}`);
    console.log('\n  this is expected in a published clone: the registry is an internal corpus and does not ship.');
    console.log('sem_registry_gate: UNMEASURED');
    return 'UNMEASURED';
  }
  console.log(`sem_registry_gate: ${result.fileCount} source files, ${result.rowCount} registry rows, docs/sem_registry.json ${result.json.state}`);
  if (result.missing.length) {
    console.log(`\n  ${GATE_MESSAGES.MISSING_ROW} (${result.missing.length}):`);
    for (const m of result.missing) console.log(`        - ${m}`);
  }
  if (result.stale.length) {
    console.log(`\n  ${GATE_MESSAGES.STALE_ROW} (${result.stale.length}):`);
    for (const s of result.stale) console.log(`        - ${s}`);
  }
  if (result.duplicated.length) {
    console.log(`\n  ${GATE_MESSAGES.DUPLICATE_ROW} (${result.duplicated.length}):`);
    for (const d of result.duplicated) console.log(`        - ${d}`);
  }
  if (result.json.state !== 'FRESH') {
    console.log(`\n  ${GATE_MESSAGES.JSON_DRIFT}: ${result.json.state} -- ${result.json.detail}`);
    console.log('        fix: node tools/sem_registry_generate.mjs');
  }
  const clean = result.missing.length === 0 && result.stale.length === 0 && result.duplicated.length === 0 && result.json.state === 'FRESH';
  console.log(clean ? `\nsem_registry_gate: ${GATE_MESSAGES.CLEAN}` : '\nsem_registry_gate: RED');
  return clean ? 'CLEAN' : 'RED';
}

const EXIT_FOR = { CLEAN: EXIT_CLEAN, RED: EXIT_RED, UNMEASURED: EXIT_UNMEASURED };

// SELF-TEST: a gate that has never gone red is a gate nobody has evidence works. Both
// directions are fired here before the real run is believed, and neither round touches
// req/99_SEMANTICS_REGISTRY.md -- MISSING is fired by a throwaway file written under
// tools/ and removed in the same round; STALE is fired by handing diff() a synthetic
// row that names a path nothing on disk has ever held. Editing the real registry file
// to fire STALE would race any concurrent writer in this shared tree and would be the
// exact "edit a canonical file to test the gate that watches it" move the canon-rewrite
// guard exists to refuse.
function selfTest() {
  const tmpRel = 'tools/.sem_registry_selftest_tmp.mjs';
  const tmpAt = path.join(ROOT, tmpRel);
  const phantomRel = 'tools/.sem_registry_selftest_phantom.mjs';
  const registryText = fs.readFileSync(REGISTRY_AT, 'utf8');
  const registeredRows = registryPaths(registryText);

  fs.writeFileSync(tmpAt, '// SPDX-License-Identifier: Apache-2.0\n// self-test scratch file, written and removed within one round of node tools/sem_registry_gate.mjs --self-test.\n');
  let missingFired = false;
  try {
    const { missing } = diff(sourceFiles(ROOT), registeredRows);
    missingFired = missing.includes(tmpRel);
  } finally {
    fs.rmSync(tmpAt, { force: true });
  }

  const { stale: staleWithPhantom } = diff(sourceFiles(ROOT), [...registeredRows, phantomRel]);
  const staleFired = staleWithPhantom.includes(phantomRel);

  const cleaned = !fs.existsSync(tmpAt);
  const untouched = fs.readFileSync(REGISTRY_AT, 'utf8') === registryText;

  console.log(`  MISSING  ${missingFired ? 'fired' : 'DID NOT FIRE'} (planted ${tmpRel}, unregistered)`);
  console.log(`  STALE    ${staleFired ? 'fired' : 'DID NOT FIRE'} (planted a row for ${phantomRel}, which does not exist on disk)`);
  console.log(`  cleanup  ${cleaned ? 'confirmed -- temp file removed' : 'FAILED -- temp file still present'}`);
  console.log(`  registry ${untouched ? 'confirmed byte-identical -- never written' : 'FAILED -- registry file changed'}`);

  // Round 3: the JSON drift check, fired at a throwaway output path so docs/
  // sem_registry.json (if it exists) is never touched by this round.
  const scratchOut = path.join(ROOT, '.run', 'sem_registry_selftest_scratch.json');
  fs.mkdirSync(path.dirname(scratchOut), { recursive: true });
  fs.rmSync(scratchOut, { force: true });
  const absentState = driftState(ROOT, scratchOut).state;
  fs.writeFileSync(scratchOut, `${JSON.stringify({ source_sha256: 'deliberately-wrong-hash', row_count: 0, rows: [] }, null, 2)}\n`);
  const staleState = driftState(ROOT, scratchOut).state;
  fs.rmSync(scratchOut, { force: true });
  const absentFired = absentState === 'ABSENT';
  const jsonStaleFired = staleState === 'STALE';
  console.log(`  ABSENT   ${absentFired ? 'fired' : 'DID NOT FIRE'} (no file at the scratch output path)`);
  console.log(`  JSON-STALE ${jsonStaleFired ? 'fired' : 'DID NOT FIRE'} (scratch file with a deliberately wrong source_sha256)`);

  return missingFired && staleFired && cleaned && untouched && absentFired && jsonStaleFired;
}

const argv = process.argv.slice(2);
if (argv.includes('--self-test')) {
  // The self-test plants rows against the real registry, so it cannot run without it.
  // It says so and stops rather than reporting BLIND, which would accuse the gate of a
  // fault it does not have (req/883).
  if (!fs.existsSync(REGISTRY_AT)) {
    console.log(`sem_registry_gate --self-test: UNMEASURED -- ${GATE_MESSAGES.REGISTRY_ABSENT}, and the self-test plants its rounds against it.`);
    process.exitCode = EXIT_UNMEASURED;
  } else {
    const ok = selfTest();
    console.log(ok
      ? '\nsem_registry_gate --self-test: both directions fired, cleanup confirmed, registry untouched'
      : '\nsem_registry_gate --self-test: BLIND -- a round did not fire, cleanup failed, or the registry was touched');
    if (!ok) process.exitCode = EXIT_RED;
    else process.exitCode = EXIT_FOR[report(runGate())];
  }
} else {
  process.exitCode = EXIT_FOR[report(runGate())];
}
