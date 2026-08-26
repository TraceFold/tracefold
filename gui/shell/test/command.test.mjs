// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { commandFor } from '../kernel/command.mjs';

test('a dock object view reproduces as a dock:go line, with every argument named', () => {
  const line = commandFor('dock', {
    index: 0, side: 'left', at: 1, id: 'probe-a',
  });
  assert.equal(line, 'gx dock:go --index 0 --side left --at 1 --id probe-a');
});

test('a stage object view reproduces as a tab:go line, with the path joined by dots', () => {
  const line = commandFor('stage', {
    index: 1, path: [0, 1], at: 2, id: 'sheet-a',
  });
  assert.equal(line, 'gx tab:go --index 1 --path 0.1 --at 2 --id sheet-a');
});

test('a stage object view with the root path (empty array) still names --path', () => {
  const line = commandFor('stage', {
    index: 0, path: [], at: 0, id: 'sheet-b',
  });
  assert.ok(line.includes('--path '));
});

test('an unknown kind is refused rather than silently formatted wrong', () => {
  assert.throws(() => commandFor('nowhere', {}), RangeError);
});

test('a command with no id still formats (a bare object view has nothing to name)', () => {
  const line = commandFor('dock', { index: 0, side: 'right', at: 0 });
  assert.ok(!line.includes('--id'));
});
