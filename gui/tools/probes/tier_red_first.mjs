// SPDX-License-Identifier: Apache-2.0
// Live rounds for the tiers that are not the pixel tier, fired before any of their
// readings are believed. This is the shape the breach runner takes when it is
// built (req/06 §8-2 stage I5); until then it is run by hand and its results are
// written into breaches.json.
//
// Every round: record what is about to change, change it, run the whole harness in a
// fresh process, put it back, and check the bytes came back. A round whose edit
// matched nothing is a failure of the round.
//
//   node tools/probes/tier_red_first.mjs

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');
const VERIFY_ALL = path.join(ROOT, 'tools', 'verify-all.mjs');
const PENDING_AT = path.join(ROOT, '.run', 'pending-tier-breach.json');

const ROUNDS = [
  {
    id: 'TB-LOAD',
    targets: ['LD-EVALUATES'],
    breaks: 'a module in the tree stops parsing, which a text reading cannot see',
    edits: [{
      file: 'membrane/src/address.mjs',
      find: 'export',
      replace: 'const = ; export',
    }],
  },
  {
    id: 'TB-MOUNT',
    targets: ['MT-FACE'],
    breaks: 'a face produces fewer glyphs than it declares, so the host is not what was asked for',
    edits: [{
      file: 'tools/faces.json',
      find: '"expects": { "minElements": 12, "glyphs": 3 },\n      "baseline": true',
      replace: '"expects": { "minElements": 12, "glyphs": 9 },\n      "baseline": true',
    }],
  },
  {
    id: 'TB-PIXEL',
    targets: ['PX-INK-OVERLAP', 'PX-BASELINE'],
    breaks: 'the positive control acquires the RT-07 defect, so the tier must stop calling it clean',
    edits: [{
      file: 'tools/fixtures/clean-baseline.html',
      find: '.row .note { width: 200px; color: #5d666e; font-size: 12px; }',
      replace: '.row .note { width: 200px; color: #5d666e; font-size: 12px; margin-left: -180px; }',
    }],
  },
];

const digestOf = (text) => crypto.createHash('sha256').update(text).digest('hex').slice(0, 16);

function harness() {
  const run = spawnSync(process.execPath, [VERIFY_ALL], { encoding: 'utf8', cwd: ROOT });
  const report = JSON.parse(fs.readFileSync(path.join(ROOT, '.run', 'report.json'), 'utf8'));
  return {
    exit: run.status,
    verdicts: Object.fromEntries(report.readings.map((r) => [r.id, r.verdict])),
    notes: Object.fromEntries(report.readings.map((r) => [r.id, r.note])),
  };
}

if (fs.existsSync(PENDING_AT)) {
  const pending = JSON.parse(fs.readFileSync(PENDING_AT, 'utf8'));
  for (const [relative, original] of Object.entries(pending.originals)) fs.writeFileSync(path.join(ROOT, relative), original);
  fs.rmSync(PENDING_AT);
  console.log(`rolled back an interrupted round: ${pending.id}`);
}

const unbroken = harness();
console.log(`unbroken : exit ${unbroken.exit}  ${JSON.stringify(unbroken.verdicts)}\n`);

const outcomes = [];
for (const round of ROUNDS) {
  const touched = [...new Set(round.edits.map((e) => e.file))];
  const originals = Object.fromEntries(touched.map((f) => [f, fs.readFileSync(path.join(ROOT, f), 'utf8')]));
  fs.mkdirSync(path.dirname(PENDING_AT), { recursive: true });
  fs.writeFileSync(PENDING_AT, JSON.stringify({ id: round.id, originals }, null, 2));

  let blank = null;
  for (const edit of round.edits) {
    const at = path.join(ROOT, edit.file);
    const text = fs.readFileSync(at, 'utf8');
    const hits = text.split(edit.find).length - 1;
    if (hits < 1) { blank = `${edit.file}: the pattern matched ${hits} times`; break; }
    fs.writeFileSync(at, text.replace(edit.find, edit.replace));
  }
  const moved = !blank && touched.every((f) => digestOf(fs.readFileSync(path.join(ROOT, f), 'utf8')) !== digestOf(originals[f]));
  if (!blank && !moved) blank = 'the edits left every byte identical';

  const broken = blank ? null : harness();
  for (const [relative, original] of Object.entries(originals)) fs.writeFileSync(path.join(ROOT, relative), original);
  fs.rmSync(PENDING_AT, { force: true });
  const restored = touched.every((f) => fs.readFileSync(path.join(ROOT, f), 'utf8') === originals[f]);

  const turned = broken ? round.targets.filter((t) => unbroken.verdicts[t] === 'PASS' && broken.verdicts[t] === 'FAIL') : [];
  const collateral = broken
    ? Object.keys(unbroken.verdicts).filter((id) => !round.targets.includes(id) && unbroken.verdicts[id] !== broken.verdicts[id])
    : [];
  const verdict = blank ? 'BLANK' : turned.length === round.targets.length ? 'LANDED' : 'INERT';
  outcomes.push({ id: round.id, verdict, restored, collateral });

  console.log(`${round.id}  ${verdict.padEnd(6)} restored ${restored ? 'yes' : 'NO'}  collateral ${collateral.length}  -- ${round.breaks}`);
  if (blank) console.log(`        the round fired blank: ${blank}`);
  else {
    for (const target of round.targets) console.log(`        ${target}: ${unbroken.verdicts[target]} -> ${broken.verdicts[target]}   ${broken.notes[target]}`);
    if (collateral.length) console.log(`        also moved: ${collateral.join(', ')}`);
  }
}

const failed = outcomes.filter((o) => o.verdict !== 'LANDED' || !o.restored || o.collateral.length > 0);
console.log(`\nrounds ${outcomes.length}  landed ${outcomes.filter((o) => o.verdict === 'LANDED').length}  not-restored ${outcomes.filter((o) => !o.restored).length}  with-collateral ${outcomes.filter((o) => o.collateral.length).length}`);
console.log(failed.length === 0
  ? 'tier live rounds : every round turned its target and nothing else'
  : 'tier live rounds : a round did not turn its target, or moved something it was not aimed at');
process.exitCode = failed.length === 0 ? 0 : 1;
