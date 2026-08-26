// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { textOf, findByAttr } from '../src/element.mjs';
import { serial, serialOf, cutOf, notAProof, refusalFor, DEFAULT_TAKE, SERIAL_MESSAGES } from '../src/serial.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const SHORT = 'a73e393d8a97b86c';
const LONG = 'a73e393d8a97b86c1122334455667788990011223344556677889900aabbccdd';

test('the cut is the first characters, upper cased', () => {
  assert.equal(serialOf(SHORT), 'A73E39');
  assert.equal(serialOf(SHORT, { take: 4 }), 'A73E');
  assert.equal(serialOf(LONG), 'A73E39');
});

test('the denominator is read off the digest, so the same code says two different true things', () => {
  // This is the defect being closed: the shipped page said "six characters out of
  // sixty-four" about a sixteen character digest, and the check that guarded the
  // sentence was looking for a different phrase.
  assert.ok(notAProof(SHORT).includes(`out of ${SHORT.length}`));
  assert.ok(notAProof(LONG).includes(`out of ${LONG.length}`));
  assert.notEqual(notAProof(SHORT), notAProof(LONG));
  assert.equal(/sixty-four/i.test(notAProof(SHORT) + notAProof(LONG)), false);
});

test('the module carries no typed denominator at all', () => {
  const source = fs.readFileSync(path.resolve(HERE, '../src/serial.mjs'), 'utf8');
  const code = source.split('\n').filter((line) => !line.trim().startsWith('//')).join('\n');
  assert.equal(/sixty-four|sixty four/i.test(code), false);
});

test('how it was cut is stated in full, so the cut can be redone by hand', () => {
  assert.equal(cutOf(SHORT), `the first ${DEFAULT_TAKE} of ${SHORT.length} hexadecimal characters, in upper case`);
  assert.equal(cutOf(LONG, { take: 8 }), `the first 8 of ${LONG.length} hexadecimal characters, in upper case`);
});

test('a digest that cannot be cut is refused with the reason, and each reason is its own', () => {
  assert.equal(refusalFor('not-a-digest'), SERIAL_MESSAGES.NOT_HEX);
  assert.equal(refusalFor(''), SERIAL_MESSAGES.NOT_HEX);
  assert.equal(refusalFor(null), SERIAL_MESSAGES.NOT_HEX);
  assert.equal(refusalFor(123456), SERIAL_MESSAGES.NOT_HEX);
  assert.equal(refusalFor('abc'), SERIAL_MESSAGES.TOO_SHORT);
  assert.equal(refusalFor(SHORT, 0), SERIAL_MESSAGES.BAD_TAKE);
  assert.equal(refusalFor(SHORT, 2.5), SERIAL_MESSAGES.BAD_TAKE);
  assert.equal(refusalFor(SHORT), null);
  assert.equal(serialOf('abc'), null);
});

test('drawing a serial always draws the cut and the caveat with it', () => {
  const node = serial(SHORT);
  const roles = ['cut-value', 'how-cut', 'not-a-proof'].map((r) => findByAttr(node, 'data-role', r));
  for (const [i, hit] of roles.entries()) assert.equal(hit.length, 1, ['cut-value', 'how-cut', 'not-a-proof'][i]);
  const drawn = textOf(node);
  assert.ok(drawn.includes('A73E39'));
  assert.ok(drawn.includes(String(SHORT.length)));
  assert.ok(drawn.includes('not a proof'));
});

test('there is no argument that draws the characters on their own', () => {
  // Getting the six characters without the two sentences means calling serialOf,
  // which returns a string and puts nothing on a screen.
  const drawn = textOf(serial(SHORT));
  assert.ok(drawn.length > 'A73E39'.length + 60);
  assert.equal(typeof serialOf(SHORT), 'string');
});

test('an uncuttable digest is drawn as a refusal, not as an empty space', () => {
  const node = serial('not-a-digest');
  assert.equal(node.attrs['data-usable'], 'false');
  const drawn = textOf(node);
  assert.ok(drawn.includes(SERIAL_MESSAGES.NOT_HEX));
  assert.ok(drawn.includes('nothing was cut'));
  assert.equal(drawn.includes('undefined'), false);
  assert.equal(drawn.includes('null'), false);
});

test('the caveat says what the reader has to do instead', () => {
  assert.ok(notAProof(SHORT).includes('Compare the whole digest'));
  assert.ok(notAProof(SHORT).includes('hint'));
});

test('a usable serial is marked usable, so a caller can tell the two apart by machine', () => {
  assert.equal(serial(SHORT).attrs['data-usable'], 'true');
  assert.equal(serial('zz').attrs['data-usable'], 'false');
});
