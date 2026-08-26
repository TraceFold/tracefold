// SPDX-License-Identifier: Apache-2.0
// Live round for AC-I40 (L-LEDGER-JSON), fired before the reading is believed.
//
// This one does not touch a real file, breach-style: the reading tests one string
// against JSON.parse, so the fixture is a string, not a checked-out edit with a
// rollback. A corrupted copy in memory proves the same thing an edited-and-restored
// file would, with zero chance of leaving a real ledger malformed if the process dies
// mid-round.
//
//   node tools/probes/ledger_json_red_first.mjs

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import { ledgerParses, LEDGER_JSON_MESSAGES } from '../tiers/lint.mjs';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');

const outcomes = [];

// Positive control: the real ledger, as it stands on disk right now, must read PASS.
// A round that only ever proves the broken case can be broken can prove nothing.
for (const relative of ['tools/breaches.json', 'tools/faces.json', 'tools/meta-breaches.json']) {
  const text = fs.readFileSync(path.join(ROOT, relative), 'utf8');
  const clean = ledgerParses({ path: relative, text });
  outcomes.push({ id: `clean:${relative}`, ok: clean === true, detail: clean });
}

// Negative control: the exact defect this AC exists for -- a missing comma between two
// array members, the shape tools/breaches.json actually carried at line 42->43.
const broken = '{ "a": [ {"id": 1} {"id": 2} ] }';
const dirty = ledgerParses({ path: 'tools/fixtures/broken-ledger.json', text: broken });
outcomes.push({
  id: 'dirty:missing-comma',
  ok: typeof dirty === 'string' && dirty.startsWith(LEDGER_JSON_MESSAGES.MALFORMED),
  detail: dirty,
});

// A second negative shape: truncated file (the other way a hand-edit breaks a ledger).
const truncated = '{ "breaches": [ {"id": "RT-07"';
const dirty2 = ledgerParses({ path: 'tools/fixtures/truncated-ledger.json', text: truncated });
outcomes.push({
  id: 'dirty:truncated',
  ok: typeof dirty2 === 'string' && dirty2.startsWith(LEDGER_JSON_MESSAGES.MALFORMED),
  detail: dirty2,
});

for (const o of outcomes) console.log(`${o.ok ? 'held ' : 'FELL '} ${o.id}  -- ${o.detail}`);
const failed = outcomes.filter((o) => !o.ok);
console.log(failed.length === 0
  ? '\nAC-I40 live round: clean ledgers pass, broken shapes turn red -- the reading is not decorative'
  : '\nAC-I40 live round: a control did not answer as expected');
process.exitCode = failed.length === 0 ? 0 : 1;
