// SPDX-License-Identifier: Apache-2.0
// W12 -- "extension is additive only" -- read as two counted sets that a schema change is
// not allowed to move in one direction:
//
//   requiredFields  a face declaration schema field that is required and was not
//                    required in the committed baseline is a breaking change: every
//                    face folder that built against the old schema now fails to load.
//   portMethods     a method the baseline's port offered and the current one does not
//                    is a breaking change: a face written against the old port throws
//                    at call time instead of at manifest time.
//
// Growing requiredFields or shrinking portMethods is the fault this file exists to catch.
// The reverse directions (a field stops being required, a method is added) are additive
// and are not faults -- W12 says "additive only", not "unchanged".
//
// The baseline is a committed fact, not a live computation: `tools/w12_baseline.json` is
// read by the test and is not regenerated as part of running it. To move the baseline
// forward after a reviewed, intentional schema change:
//
//   node tools/w12_baseline.mjs --write
//
// Never run that from a test, a gate, or CI. A baseline that regenerates itself on every
// run cannot ever report a diff -- it would be comparing the schema to itself.

import { writeFileSync, readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { FACE_FIELDS } from '../kernel/manifest.mjs';
import { standInPort } from '../demo/port.mock.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
export const BASELINE_PATH = join(HERE, 'w12_baseline.json');

/** @returns {{requiredFields: string[], portMethods: string[]}} the schema's current shape */
export function computeCurrent() {
  const requiredFields = FACE_FIELDS.filter((f) => f.required).map((f) => f.name).sort();
  const portMethods = Object.keys(standInPort()).sort();
  return { requiredFields, portMethods };
}

/**
 * @param {{requiredFields: string[], portMethods: string[]}} baseline the committed shape
 * @param {{requiredFields: string[], portMethods: string[]}} current the shape now
 * @returns {{grewRequired: string[], shrankPort: string[]}} empty arrays mean the gate holds
 */
export function diffAgainstBaseline(baseline, current) {
  const grewRequired = current.requiredFields.filter((f) => !baseline.requiredFields.includes(f));
  const shrankPort = baseline.portMethods.filter((m) => !current.portMethods.includes(m));
  return { grewRequired, shrankPort };
}

export function readBaseline() {
  if (!existsSync(BASELINE_PATH)) throw new Error(`no baseline committed at ${BASELINE_PATH}; run --write once, deliberately, and commit it`);
  return JSON.parse(readFileSync(BASELINE_PATH, 'utf8'));
}

if (process.argv[1] && process.argv[1].endsWith('w12_baseline.mjs')) {
  if (!process.argv.includes('--write')) {
    process.stderr.write('this only writes the baseline, and only when told to: node tools/w12_baseline.mjs --write\n');
    process.exit(1);
  }
  const current = computeCurrent();
  writeFileSync(BASELINE_PATH, `${JSON.stringify(current, null, 2)}\n`, 'utf8');
  process.stdout.write(`wrote ${BASELINE_PATH}: ${current.requiredFields.length} required fields, ${current.portMethods.length} port methods\n`);
}
