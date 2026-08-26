// SPDX-License-Identifier: Apache-2.0
// W12 -- "extension is additive only", machine-checked as a diff against a committed
// baseline (tools/w12_baseline.json). See tools/w12_baseline.mjs for what "grew" and
// "shrank" mean and the (manual, explicit) regeneration procedure.
//
// Red-first: `diffAgainstBaseline` is exercised first against synthetic, deliberately
// broken pairs -- a required field the baseline never saw, a port method the baseline
// had and the current set has lost -- and required to report them. Only then is it run
// against the real committed baseline and the real current schema, and required to
// report nothing.

import test from 'node:test';
import assert from 'node:assert/strict';

import { computeCurrent, diffAgainstBaseline, readBaseline, BASELINE_PATH } from '../tools/w12_baseline.mjs';

test('red-first: the diff function catches a synthetic grown-required and a synthetic shrunk-port', () => {
  const baseline = { requiredFields: ['id', 'title'], portMethods: ['get_healthz', 'get_stream'] };

  const grew = diffAgainstBaseline(baseline, { requiredFields: ['id', 'title', 'newRequiredField'], portMethods: ['get_healthz', 'get_stream'] });
  assert.deepEqual(grew.grewRequired, ['newRequiredField']);
  assert.deepEqual(grew.shrankPort, []);

  const shrank = diffAgainstBaseline(baseline, { requiredFields: ['id', 'title'], portMethods: ['get_healthz'] });
  assert.deepEqual(shrank.grewRequired, []);
  assert.deepEqual(shrank.shrankPort, ['get_stream']);

  // The directions W12 does not forbid: a field stops being required, a method is added.
  const additive = diffAgainstBaseline(baseline, { requiredFields: ['id'], portMethods: ['get_healthz', 'get_stream', 'get_new_thing'] });
  assert.deepEqual(additive, { grewRequired: [], shrankPort: [] }, 'a shrinking required-set or a growing port-set is additive, not a fault');
});

test('the baseline file is committed and readable', () => {
  const baseline = readBaseline();
  assert.ok(Array.isArray(baseline.requiredFields) && baseline.requiredFields.length > 0, `no required fields in ${BASELINE_PATH}`);
  assert.ok(Array.isArray(baseline.portMethods) && baseline.portMethods.length > 0, `no port methods in ${BASELINE_PATH}`);
});

test('the current schema has not grown a required field or shrunk a port method versus the committed baseline', () => {
  const baseline = readBaseline();
  const current = computeCurrent();
  const diff = diffAgainstBaseline(baseline, current);
  assert.deepEqual(diff.grewRequired, [], `required fields grew: ${diff.grewRequired.join(', ')} -- if this is an intentional, reviewed schema change, run "node tools/w12_baseline.mjs --write" and commit the result`);
  assert.deepEqual(diff.shrankPort, [], `port methods shrank: ${diff.shrankPort.join(', ')} -- a face written against the old port would now throw at call time`);
  process.stdout.write(`# W12 baseline: ${current.requiredFields.length} required fields, ${current.portMethods.length} port methods, 0 grown, 0 shrunk\n`);
});
