// SPDX-License-Identifier: Apache-2.0
// AC-P0. A drawing part draws and does not judge; a deciding part decides and never
// reaches a document.
//
// Each gate is fired red here before its silence is read as a pass, and two of them
// are fired against files written to disk rather than against strings, because the
// gate that ships walks a directory and that walk is part of what is being tested.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';

import {
  sourceFiles, filesOfKind, survey, domReach, verdictBranches, colourLiterals, namedSymbols,
  nonAscii, typedDenominators, redSpendSites, rawSvgBuilders, forbiddenTokens, codeOf,
  KINDS, SRC_DIR, ELEMENT_DOOR, BOUNDARY_MESSAGES,
} from '../tools/boundary.mjs';
import { FORBIDDEN_TOKENS } from '../src/tokens.mjs';

const files = sourceFiles();

test('the population is not empty, because a rule over nothing passes', () => {
  assert.ok(files.length > 0, BOUNDARY_MESSAGES.EMPTY_POPULATION);
  assert.ok(filesOfKind('C', files).length > 0, BOUNDARY_MESSAGES.EMPTY_POPULATION);
  assert.ok(filesOfKind('M', files).length > 0, BOUNDARY_MESSAGES.EMPTY_POPULATION);
});

test('every module on disk is classified, and every classification is on disk', () => {
  const onDisk = files.map((f) => f.name).sort();
  const classified = Object.keys(KINDS).sort();
  assert.deepEqual(onDisk, classified, `${SRC_DIR} and the roster in tools/boundary.mjs disagree`);
});

test('no deciding part can reach a document', () => {
  assert.deepEqual(survey(files).domInDeciding, []);
});

test('the document gate refuses a deciding part that reaches for one, on disk', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'gxapp-boundary-'));
  const planted = path.join(dir, 'seal-claim.mjs');
  fs.writeFileSync(planted, 'export const claim = () => document.createElement("span");\n', 'utf8');
  const found = survey(sourceFiles(dir));
  assert.equal(found.domInDeciding.length > 0, true, BOUNDARY_MESSAGES.CROSSED);
  assert.equal(found.domInDeciding[0].file, 'seal-claim.mjs');
  fs.rmSync(dir, { recursive: true, force: true });
});

test('no drawing part takes a different route because of what a verdict says', () => {
  assert.deepEqual(survey(files).verdictBranchesInDrawing, []);
});

test('the verdict gate refuses each shape a comparison can take', () => {
  const planted = [
    "if (record.verdict === 'Deny') { return red(); }",
    'if (v !== "Admit") { return null; }',
    "switch (verdict) { case 'Escalate': break; }",
    "case 'Deny':",
    "if ('Admit' === value) {}",
  ];
  for (const line of planted) assert.equal(verdictBranches(line).length > 0, true, line);
  const allowed = [
    "const mark = MARKS.verdict[key];",
    "MARKS.verdict = { Admit: {}, Deny: {}, Escalate: {} };",
    "const word = String(verdict);",
    "RED_MEANINGS.includes(mark.means)",
  ];
  for (const line of allowed) assert.deepEqual(verdictBranches(line), [], line);
});

test('no colour is spelled anywhere but in the stylesheet of record', () => {
  assert.deepEqual(survey(files).colourLiterals, []);
});

test('the colour gate refuses each way a colour can be written', () => {
  for (const line of ['color: #E2493B;', 'background:#fff', 'rgb(226, 73, 59)', 'rgba(0,0,0,.5)', 'hsl(4 74% 56%)']) {
    assert.equal(colourLiterals(line).length > 0, true, line);
  }
  assert.deepEqual(colourLiterals("href: '#gx-verdict-Admit'"), [], 'an id reference is not a colour');
  assert.deepEqual(colourLiterals('color: var(--deny)'), []);
});

test('red is spent in exactly one place', () => {
  const sites = redSpendSites(files);
  assert.equal(sites.length, 1, `red is spent at: ${sites.map((s) => `${s.file}:${s.line}`).join(', ')}`);
  assert.equal(sites[0].file, 'verdict-badge.mjs');
});

test('no borrowed symbol and no character outside ascii reaches shipped source', () => {
  assert.deepEqual(survey(files).nonAscii, []);
  for (const file of files) assert.deepEqual(namedSymbols(file.text), [], file.name);
});

test('the symbol gate refuses each banned mark and any mixed script', () => {
  for (const line of ['const bullet = "●";', 'label: "★ starred"', 'const box = "■";']) {
    assert.equal(namedSymbols(line).length > 0, true, line);
    assert.equal(nonAscii(line).length > 0, true, line);
  }
  assert.equal(nonAscii('// a comment with 日本語 in it').length > 0, true);
  assert.deepEqual(nonAscii('plain ascii only'), []);
});

test('no denominator about the data is typed by hand', () => {
  assert.deepEqual(survey(files).typedDenominators, []);
  // The word does appear in serial.mjs, in the comment that explains the defect. A
  // gate that counted that would be a gate that punishes writing down what went
  // wrong, so the code gates read code.
  const serial = files.find((f) => f.name === 'serial.mjs');
  assert.ok(typedDenominators(serial.text).length > 0, 'the prose says it');
  assert.deepEqual(typedDenominators(serial.code), [], 'the code does not');
});

test('blanking the prose does not blind the gate: an offender beside a comment is still caught', () => {
  const planted = [
    '// there is no document in this file',
    'export const reach = () => document.body;',
    '/* nothing here spells a colour */',
    'const c = "#E2493B";',
  ].join('\n');
  const stripped = codeOf(planted);
  assert.equal(domReach(stripped).length, 1);
  assert.equal(domReach(stripped)[0].line, 2, 'line numbers survive the blanking');
  assert.equal(colourLiterals(stripped).length, 1);
  assert.equal(colourLiterals(stripped)[0].line, 4);
  assert.ok(colourLiterals(planted).length >= 1);
});

test('blanking leaves a url alone, because the slashes in one are not a comment', () => {
  assert.match(codeOf("const NS = 'http://www.w3.org/2000/svg';"), /www\.w3\.org/);
});

test('the denominator gate refuses the sentence that shipped', () => {
  assert.equal(typedDenominators('six characters out of sixty-four, so two records').length > 0, true);
  assert.deepEqual(typedDenominators('six characters out of ${digest.length}'), []);
});

test('the ink this package measured and refused is spelled in no code, only in the note saying why', () => {
  assert.deepEqual(survey(files).forbiddenTokens, []);
  for (const file of files) assert.deepEqual(forbiddenTokens(file.code, FORBIDDEN_TOKENS), [], file.name);
  assert.equal(forbiddenTokens('color: var(--ink-3)', FORBIDDEN_TOKENS).length > 0, true);
  assert.equal(forbiddenTokens('style: `${CONSUMED.ink}`', FORBIDDEN_TOKENS).length, 0);
  const tokens = files.find((f) => f.name === 'tokens.mjs');
  assert.ok(tokens.text.includes('FORBIDDEN_TOKENS'), 'the roster naming it is not itself the offender');
  assert.ok(/within a rounding step of the floor|rounding error of the 4\.5/.test(tokens.text), 'the measured reason is written down');
});

test('elements are built in one place, so a second builder cannot invent its own sizing', () => {
  const offenders = rawSvgBuilders(files);
  assert.deepEqual(offenders, [], `only ${ELEMENT_DOOR} builds elements`);
});

test('the one-door gate refuses a second builder, on disk', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'gxapp-door-'));
  fs.writeFileSync(path.join(dir, 'palette.mjs'), 'const s = doc.createElementNS(NS, "svg");\n', 'utf8');
  assert.equal(rawSvgBuilders(sourceFiles(dir)).length > 0, true);
  fs.rmSync(dir, { recursive: true, force: true });
});

test('a document is reached from two drawing modules and nowhere else, in the whole package', () => {
  // The rule is one-directional: deciding parts may not reach a document, drawing
  // parts may. Naming which two do keeps that permission from spreading unnoticed.
  const reaching = files.filter((f) => domReach(f.code).length > 0).map((f) => f.name).sort();
  assert.deepEqual(reaching, [ELEMENT_DOOR, 'glyph-sheet.mjs'].sort());
  assert.equal(filesOfKind('M', files).some((f) => reaching.includes(f.name)), false);
});

test('the whole survey is clean and says what it counted', () => {
  const report = survey(files);
  assert.equal(report.populationsNonEmpty, true);
  assert.equal(report.counted, Object.keys(KINDS).length);
  assert.equal(report.drawing.length + report.deciding.length, report.counted);
  for (const key of ['domInDeciding', 'verdictBranchesInDrawing', 'colourLiterals', 'forbiddenTokens', 'nonAscii', 'namedSymbols', 'typedDenominators', 'rawSvgBuilders']) {
    assert.deepEqual(report[key], [], `${key}: ${JSON.stringify(report[key])}`);
  }
  assert.equal(report.redSpendSites.length, 1);
});
