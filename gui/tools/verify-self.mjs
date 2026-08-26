// SPDX-License-Identifier: Apache-2.0
// The harness fired at itself, which runs before anything else it says can be read.
//
// For each round: write down what is about to be edited, edit it, put the fixed
// situation through the edited rig in a fresh process, put the file back, and check
// it came back byte for byte. A round whose edit matched nothing is a failure of the
// round, not a pass for the harness -- the retired instrument had a substitution
// silently miss and read the result as success.
//
//   node tools/verify-self.mjs
//
// Exit 0 when every round changed the answer. Exit 5 when one did not: the harness
// is blind there, and nothing else it prints today means anything.

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { EXIT } from './rig/verdict.mjs';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
const PROBE = path.join(HERE, 'rig', 'self_probe.mjs');
const PENDING_AT = path.join(ROOT, '.run', 'pending-meta-breach.json');

export const SELF_MESSAGES = {
  ROUND_LANDED: 'the answer changed, so this point is not blind',
  ROUND_MISSED: 'the edit matched nothing, so this round fired blank',
  ROUND_INERT: 'the edit applied and the answer did not change, so the harness is blind here',
  RESTORED: 'the file came back byte for byte',
  NOT_RESTORED: 'the file did not come back byte for byte',
  SELF_BLIND: 'the harness did not notice being broken; every other number in this run is void',
  SELF_SIGHTED: 'every round changed the answer',
  DEBT_SETTLED: 'an interrupted round from an earlier run was rolled back before starting',
};

const digestOf = (text) => crypto.createHash('sha256').update(text).digest('hex').slice(0, 16);
const readProbe = () => {
  const run = spawnSync(process.execPath, [PROBE], { encoding: 'utf8' });
  return { stdout: run.stdout.trim(), status: run.status };
};

// An interrupted run leaves the tree edited. The note is written before the edit, so
// the next start can put it back rather than measure a tree somebody broke on purpose.
if (fs.existsSync(PENDING_AT)) {
  const pending = JSON.parse(fs.readFileSync(PENDING_AT, 'utf8'));
  for (const [relative, original] of Object.entries(pending.originals)) {
    fs.writeFileSync(path.join(ROOT, relative), original);
  }
  fs.rmSync(PENDING_AT);
  console.log(`${SELF_MESSAGES.DEBT_SETTLED}: ${pending.id}`);
}

const rounds = JSON.parse(fs.readFileSync(path.join(HERE, 'meta-breaches.json'), 'utf8')).meta_breaches;
const before = readProbe();
console.log(`unbroken : exit ${before.status}  ${before.stdout}\n`);

const outcomes = [];
for (const round of rounds) {
  const touched = [...new Set(round.edits.map((e) => e.file))];
  const originals = Object.fromEntries(touched.map((f) => [f, fs.readFileSync(path.join(ROOT, f), 'utf8')]));
  fs.mkdirSync(path.dirname(PENDING_AT), { recursive: true });
  fs.writeFileSync(PENDING_AT, JSON.stringify({ id: round.id, originals }, null, 2));

  let missed = null;
  for (const edit of round.edits) {
    const at = path.join(ROOT, edit.file);
    const text = fs.readFileSync(at, 'utf8');
    const hits = text.split(edit.find).length - 1;
    if (hits !== 1) { missed = `${edit.file} matched ${hits} times, wanted exactly 1`; break; }
    fs.writeFileSync(at, text.replace(edit.find, edit.replace));
  }

  let after = null;
  if (!missed) {
    // Applied means the bytes moved. A round that did not move any bytes is not a
    // round, whatever its result says.
    const moved = touched.every((f) => digestOf(fs.readFileSync(path.join(ROOT, f), 'utf8')) !== digestOf(originals[f]));
    if (!moved) missed = 'the edits left every file byte identical';
    else after = readProbe();
  }

  for (const [relative, original] of Object.entries(originals)) fs.writeFileSync(path.join(ROOT, relative), original);
  fs.rmSync(PENDING_AT, { force: true });
  const restored = touched.every((f) => fs.readFileSync(path.join(ROOT, f), 'utf8') === originals[f]);

  const changed = Boolean(after) && (after.stdout !== before.stdout || after.status !== before.status);
  const verdict = missed ? 'BLANK' : changed ? 'LANDED' : 'INERT';
  outcomes.push({ id: round.id, verdict, restored, edits: round.edits.length });

  console.log(`${round.id}  ${verdict.padEnd(6)} edits ${round.edits.length}  restored ${restored ? 'yes' : 'NO'}  -- ${round.breaks}`);
  if (missed) console.log(`        ${SELF_MESSAGES.ROUND_MISSED}: ${missed}`);
  else if (!changed) console.log(`        ${SELF_MESSAGES.ROUND_INERT}`);
  else console.log(`        broken: exit ${after.status}  ${after.stdout}`);
  if (!restored) console.log(`        ${SELF_MESSAGES.NOT_RESTORED}`);
}

const blind = outcomes.filter((o) => o.verdict !== 'LANDED');
const unrestored = outcomes.filter((o) => !o.restored);
console.log(`\nrounds ${outcomes.length}  landed ${outcomes.length - blind.length}  inert-or-blank ${blind.length}  not-restored ${unrestored.length}`);
console.log(blind.length === 0 && unrestored.length === 0
  ? `verify-self : ${SELF_MESSAGES.SELF_SIGHTED}`
  : `verify-self : ${SELF_MESSAGES.SELF_BLIND}`);
process.exitCode = blind.length === 0 && unrestored.length === 0 ? EXIT.GREEN : EXIT.SELF_BLIND;
