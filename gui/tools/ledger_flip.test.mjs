// SPDX-License-Identifier: Apache-2.0
//
// ledger_flip.test.mjs -- red-first negative controls for the [●]-flip mechanization (SS548/SS558):
//   1. a FAILING verification command must NOT flip the row and must NOT touch the file.
//   2. a PASSING verification command MUST flip the row and append a correct receipt line.
//   3. re-running on an already-flipped row refreshes the receipt in place (no duplication).
//   4. a `[退役●]` (retired) row refuses to flip without --force, and accepts it with --force.
//   5. an ambiguous --row match refuses to flip (does not guess).
//   6. ledger_dash.mjs's receipt-WARN check: a receiptless [●] WARNs, a receipted [●] does not
//      (the companion negative control for the WARN itself), and the real ledgers' 27
//      pre-existing [●] rows are all correctly reported as legacy/grandfathered, never as new.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { parseLedgerText, buildReport } from './ledger_dash.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FLIP_SCRIPT = path.join(HERE, 'ledger_flip.mjs');

const FIXTURE = `| ID | note | pointer | state |
|---|---|---|---|
| R-1 | plain row | (none yet) | [] |
| R-DUP | duplicate id A | (none) | [] |
| R-DUP | duplicate id B | (none) | [] |
| R-RETIRED | already retired | (none) | [退役●] |
`;

function writeFixture(dir) {
  const p = path.join(dir, 'fixture.md');
  writeFileSync(p, FIXTURE, 'utf8');
  return p;
}

function runFlip(args) {
  return spawnSync(process.execPath, [FLIP_SCRIPT, ...args], { encoding: 'utf8' });
}

test('a FAILING verification command does not flip the row and does not touch the file', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'ledger-flip-'));
  try {
    const file = writeFixture(dir);
    const before = readFileSync(file, 'utf8');
    const r = runFlip(['--file', file, '--line', '3', '--cmd', `${JSON.stringify(process.execPath)} -e "process.exit(1)"`]);
    assert.notEqual(r.status, 0);
    const after = readFileSync(file, 'utf8');
    assert.equal(after, before, 'file must be byte-identical after a failed verification');
    assert.match(r.stderr, /FAILED/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a PASSING verification command flips the row and appends a correct receipt', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'ledger-flip-'));
  try {
    const file = writeFixture(dir);
    const r = runFlip(['--file', file, '--line', '3', '--cmd', `${JSON.stringify(process.execPath)} -e "process.exit(0)"`]);
    assert.equal(r.status, 0, r.stderr);
    const after = readFileSync(file, 'utf8');
    const lines = after.split(/\r\n|\n/);
    assert.match(lines[2], /\[●\]\s*\|\s*$/); // line 3 (index 2) now ends in the state cell "[●] |"
    assert.match(lines[3], /^<!-- ledger_flip receipt: row_line=3 cmd=".*" exit=0 ts=\S+ -->$/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('re-flipping refreshes the receipt in place, never duplicates it', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'ledger-flip-'));
  try {
    const file = writeFixture(dir);
    const cmd = `${JSON.stringify(process.execPath)} -e "process.exit(0)"`;
    const r1 = runFlip(['--file', file, '--line', '3', '--cmd', cmd]);
    assert.equal(r1.status, 0);
    const firstReceipt = readFileSync(file, 'utf8').split(/\r\n|\n/)[3];
    // sleep is unnecessary for correctness; a second run must still produce exactly one receipt line.
    const r2 = runFlip(['--file', file, '--line', '3', '--cmd', cmd]);
    assert.equal(r2.status, 0);
    const linesAfter = readFileSync(file, 'utf8').split(/\r\n|\n/);
    const receiptLines = linesAfter.filter((l) => l.startsWith('<!-- ledger_flip receipt:'));
    assert.equal(receiptLines.length, 1, 'exactly one receipt line must remain after re-running');
    assert.match(linesAfter[3], /row_line=3/);
    void firstReceipt; // both runs' receipts reference the same row; only count/shape are asserted
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a retired ([退役●]) row refuses to flip without --force, and accepts --force', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'ledger-flip-'));
  try {
    const file = writeFixture(dir);
    const before = readFileSync(file, 'utf8');
    const cmd = `${JSON.stringify(process.execPath)} -e "process.exit(0)"`;
    const r1 = runFlip(['--file', file, '--line', '6', '--cmd', cmd]);
    assert.equal(r1.status, 5);
    assert.equal(readFileSync(file, 'utf8'), before, 'refusing a retired row must not touch the file');

    const r2 = runFlip(['--file', file, '--line', '6', '--cmd', cmd, '--force']);
    assert.equal(r2.status, 0, r2.stderr);
    const line6 = readFileSync(file, 'utf8').split(/\r\n|\n/)[5];
    assert.match(line6, /\[●\]\s*\|\s*$/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('an ambiguous --row match refuses to flip (does not guess)', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'ledger-flip-'));
  try {
    const file = writeFixture(dir);
    const before = readFileSync(file, 'utf8');
    const r = runFlip(['--file', file, '--row', 'R-DUP', '--cmd', `${JSON.stringify(process.execPath)} -e "process.exit(0)"`]);
    assert.equal(r.status, 4);
    assert.equal(readFileSync(file, 'utf8'), before);
    assert.match(r.stderr, /ambiguous/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// -- ledger_dash.mjs receipt-WARN companion controls (additive extension, SS548/SS558) ---------

test('a receiptless [●] row WARNs; the same row with a correct receipt beneath it does not', () => {
  const withoutReceipt = '| ID | note | state |\n|---|---|---|\n| W-1 | no receipt below | [●] |\n';
  const r1 = parseLedgerText(withoutReceipt);
  assert.equal(r1.receiptWarnings.length, 1);
  assert.equal(r1.receiptWarnings[0].rowId, 'W-1');
  assert.equal(r1.receiptWarnings[0].legacy, false); // no ledgerId given -> never matched against the grandfather list

  const withReceipt = `| ID | note | state |\n|---|---|---|\n| W-1 | has receipt below | [●] |\n<!-- ledger_flip receipt: row_line=3 cmd="x" exit=0 ts=2026-08-24T00:00:00.000Z -->\n`;
  const r2 = parseLedgerText(withReceipt);
  assert.equal(r2.receiptWarnings.length, 0);
});

test('a receipt line pointing at the WRONG row_line does not suppress the WARN', () => {
  const text = `| ID | note | state |\n|---|---|---|\n| W-1 | receipt below claims a different row | [●] |\n<!-- ledger_flip receipt: row_line=99 cmd="x" exit=0 ts=2026-08-24T00:00:00.000Z -->\n`;
  const r = parseLedgerText(text);
  assert.equal(r.receiptWarnings.length, 1, 'a receipt that does not name this exact line must not validate it');
});

// This one reading is the only one in this file that leaves the fixtures and reads the
// REAL ledgers, which live under req/ -- an internal corpus that does not ship. In a
// published clone it therefore had nothing to read and failed, which said "the ledger
// discipline is broken" when the truth was "the ledgers are not here" (req/855 §5
// defect 3, req/883). Every other test above builds its own text and is unaffected.
const LEDGERS_ABSENT = (() => {
  const membrane = buildReport().ledgers.find((l) => l.id === '01_membrane');
  return membrane && membrane.status === 'OK'
    ? false
    : 'UNMEASURED: the real req/ ledgers are not in this tree, so the grandfathering of '
      + 'pre-existing [●] rows was not checked. Expected in a published clone -- the '
      + 'fixture-driven readings above cover the mechanism itself.';
})();

test('the real app ledgers: every pre-existing receiptless [●] row is grandfathered (legacy), none appear as NEW', { skip: LEDGERS_ABSENT }, () => {
  const report = buildReport();
  assert.equal(report.totals.receiptWarn, report.totals.receiptWarnLegacy, 'no NEW (non-grandfathered) hand-written [●] rows should exist yet');
  assert.equal(report.totals.receiptWarnNew, 0);
  const membrane = report.ledgers.find((l) => l.id === '01_membrane');
  assert.ok(membrane && membrane.status === 'OK');
  const acM1 = membrane.receiptWarnings.find((w) => w.rowId === '**AC-M1**');
  assert.ok(acM1, 'AC-M1 should be present in the receipt-WARN list (no receipt exists for it)');
  assert.equal(acM1.legacy, true, 'AC-M1 predates ledger_flip.mjs and must be reported as legacy, not a fresh violation');
});
