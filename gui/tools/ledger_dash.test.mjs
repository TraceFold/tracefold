// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { parseLedgerText, renderHTML, buildReport } from './ledger_dash.mjs';

// A fixture ledger shaped like req/90 (state token is the FIRST cell of the row,
// followed by a row number) and req/07 (state token IS the first cell, no number).
const FIXTURE_90_STYLE = `
| # | path | category | dest | 備考 |
|---|---|---|---|---|
| [] 1 | a.js | impl | shell | one |
| [◐] 2 | b.js | impl | shell | two |
| [●] 3 | c.js | impl | shell | closed via §12 lane rerun |
| [退役●] 4 | d.js | impl | 不要 | retired |
`;

// A fixture shaped like req/01/02/06's AC tables (state token is the LAST cell).
const FIXTURE_AC_STYLE = `
| ID | 定義 | 検知 | 負対照 | pointer | state |
|---|---|---|---|---|---|
| **AC-1** | rule one | detect one | inject one | — | [ ] |
| **AC-2** | rule two | detect two | inject two | — | [●] |
`;

// A fixture where a [●] row has no §/lane/一次 pointer anywhere in the row: this
// must WARN. A companion row with a pointer must NOT warn -- the negative control.
const FIXTURE_WARN = `
| ID | note | state |
|---|---|---|
| W-1 | no pointer anywhere in this row | [●] |
| W-2 | closed after independent rerun via lane 4, see §9 | [●] |
`;

// An unrecognized bracket token (checkbox-style [x], as used by req/07's own
// selection column) must be counted as parse-unable, not silently folded into
// one of the four canonical states.
const FIXTURE_UNRECOGNIZED = `
| sel | repo | verdict |
|---|---|---|
| [x] | some/repo | observed |
| [ ] | other/repo | rejected |
`;

// Two bracket-looking cells in one row is ambiguous and must not be guessed at.
const FIXTURE_AMBIGUOUS = `
| a | b |
|---|---|
| [] | [●] |
`;

// A table with no bracket-leading cells at all (e.g. req/05's real-value table)
// must contribute zero tracked rows and zero parse-unable rows -- it is simply
// not a state ledger, not a parsing failure.
const FIXTURE_NO_STATE_TABLE = `
| ID | 極性 | 真値 |
|---|---|---|
| RT-01 | 正 | S-WEB desktop 28/28 PASS |
`;

test('counts the four canonical states across two different column layouts', () => {
  const r1 = parseLedgerText(FIXTURE_90_STYLE);
  assert.equal(r1.counts.EMPTY, 1);
  assert.equal(r1.counts.HALF, 1);
  assert.equal(r1.counts.DONE, 1);
  assert.equal(r1.counts.RETIRED_DONE, 1);
  assert.equal(r1.parseUnableCount, 0);

  const r2 = parseLedgerText(FIXTURE_AC_STYLE);
  assert.equal(r2.counts.EMPTY, 1); // "[ ]" (with a space) normalizes to EMPTY
  assert.equal(r2.counts.DONE, 1);
  assert.equal(r2.parseUnableCount, 0);
});

test('a [●] row with no §/lane/一次 pointer in its own row WARNs; a row with one does not', () => {
  const r = parseLedgerText(FIXTURE_WARN);
  assert.equal(r.counts.DONE, 2);
  assert.equal(r.doneWarnings.length, 1);
  assert.equal(r.doneWarnings[0].rowId, 'W-1');
});

test('a bracket token outside the 4 canonical states is parse-unable, not silently absorbed', () => {
  const r = parseLedgerText(FIXTURE_UNRECOGNIZED);
  assert.equal(r.counts.EMPTY, 1); // "[ ]" is still literally the empty token
  assert.equal(r.unrecognized.length, 1);
  assert.equal(r.unrecognized[0].token, 'x');
  assert.equal(r.parseUnableCount, 1);
});

test('two bracket-leading cells in one row is ambiguous, not guessed', () => {
  const r = parseLedgerText(FIXTURE_AMBIGUOUS);
  assert.equal(r.ambiguous.length, 1);
  assert.equal(r.parseUnableCount, 1);
  assert.equal(r.counts.EMPTY + r.counts.HALF + r.counts.DONE + r.counts.RETIRED_DONE, 0);
});

test('a table with no bracket-leading cells contributes zero tracked and zero parse-unable rows', () => {
  const r = parseLedgerText(FIXTURE_NO_STATE_TABLE);
  assert.equal(r.totalTracked, 0);
  assert.equal(r.parseUnableCount, 0);
});

test('separator rows (---|---|) never get counted as data', () => {
  const r = parseLedgerText('| a | b |\n|---|---|\n| [] | x |\n');
  assert.equal(r.counts.EMPTY, 1);
  assert.equal(r.totalTracked, 1);
});

test('rows mentioning 凌駕 are counted mechanically as a keyword count', () => {
  const text = '| id | note | state |\n|---|---|---|\n| A | 凌駕の言い方が成立する | [●] |\n| B | 単なる観察 | [◐] |\n';
  const r = parseLedgerText(text);
  assert.equal(r.chogaMentions, 1);
});

test('buildReport resolves all configured ledgers without throwing, and renderHTML produces a self-contained page', () => {
  const report = buildReport();
  assert.ok(report.ledgers.length >= 10);
  for (const l of report.ledgers) assert.ok(l.status === 'OK' || l.status === 'MISSING' || l.status === 'AMBIGUOUS');
  const html = renderHTML(report);
  assert.match(html, /<!doctype html>/i);
  assert.match(html, /ABSORPTION_STATUS|吸収状況/);
  // forbidden decorative glyphs must not appear as bare bullets/dividers -- only
  // inside <code> tokens that quote the ledgers' own notation.
  assert.equal(/[◆■]/.test(html), false); // ◆ ■ never appear at all
});
