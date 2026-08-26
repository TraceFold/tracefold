// SPDX-License-Identifier: Apache-2.0
// The generated tables against the JSON that owns them, and the property that made
// generating them worth doing: nothing under `src/` reaches for node.
//
// req/803 measured C3 -- "wired to a real backend" -- at 0/16 across every GUI surface,
// and the cause was not that the membrane was unfinished. It was built, and it passed a
// live smoke against a real gx bed (`tools/smoke_2026-08-24.log`). It could not be
// imported by a window, because it read its route table off the disk at import time. The
// fix moves the read to build time; these tests are what stop it moving back.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { render, TARGET, SOURCES } from '../tools/gen_tables.mjs';
import { TABLE, COVERAGE, WIRE_FIELDS } from '../src/membrane.mjs';
import { stableKey, KEY_SCHEME } from '../src/idempotency.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const SRC = join(HERE, '..', 'src');
const MEMBRANE = join(HERE, '..');

test('G1 the generated tables are what the JSON beside them says', () => {
  assert.equal(existsSync(TARGET), true, 'src/tables.gen.mjs has not been generated');
  assert.equal(readFileSync(TARGET, 'utf8'), render(), 'stale: run `node tools/gen_tables.mjs`');
});

test('G2 what the membrane exports is what is in the JSON, member for member', () => {
  const held = (file) => JSON.parse(readFileSync(join(MEMBRANE, file), 'utf8'));
  assert.deepEqual(TABLE, held(SOURCES.TABLE));
  assert.deepEqual(COVERAGE, held(SOURCES.COVERAGE));
  assert.deepEqual(WIRE_FIELDS, held(SOURCES.WIRE_FIELDS));
});

/**
 * The one that matters. A browser has no `node:fs`, so a single such import anywhere in
 * the reachable graph means no window can draw a server's row -- which is the exact
 * shape of the defect this test was written after, not a hypothetical.
 */
test('G3 no module under src/ imports a node builtin, so a window can load all of them', () => {
  const offenders = [];
  for (const name of readdirSync(SRC)) {
    if (!name.endsWith('.mjs')) continue;
    const text = readFileSync(join(SRC, name), 'utf8');
    for (const hit of text.matchAll(/from\s+['"]node:([^'"]+)['"]/g)) {
      offenders.push(`${name}: node:${hit[1]}`);
    }
  }
  assert.deepEqual(offenders, [], `these keep the membrane out of every window: ${offenders.join(', ')}`);
});

test('G3-neg the check above is able to see a node import', () => {
  const planted = "import { readFileSync } from 'node:fs';\nexport const x = 1;\n";
  const found = [...planted.matchAll(/from\s+['"]node:([^'"]+)['"]/g)].map((h) => h[1]);
  assert.deepEqual(found, ['fs']);
});

/**
 * The instrument changed; the value must not have. Computed here the old way, with the
 * node builtin the membrane no longer imports, and compared against what ships.
 */
test('G5 the idempotency key through WebCrypto is the key node:crypto produced', async () => {
  const { createHash } = await import('node:crypto');
  const wasComputedBy = (methodName, row) => createHash('sha256')
    .update(JSON.stringify([KEY_SCHEME, methodName, row ?? null]))
    .digest('hex')
    .slice(0, 32);
  for (const [name, row] of [['post_candidates', null], ['post_candidates_id_verify', 't-001'], ['post_transformations_id_undo', 'gx1:zzz']]) {
    assert.equal(await stableKey(name, row), wasComputedBy(name, row), `${name}/${row} changed on the wire`);
  }
});

test('G5-neg the comparison would notice a changed derivation', async () => {
  assert.notEqual(await stableKey('post_candidates', 't-001'), await stableKey('post_candidates', 't-002'));
});

test('G4 every source the generator names is on disk and parses', () => {
  for (const file of Object.values(SOURCES)) {
    assert.equal(existsSync(join(MEMBRANE, file)), true, `${file} is named by the generator and is not there`);
    assert.doesNotThrow(() => JSON.parse(readFileSync(join(MEMBRANE, file), 'utf8')));
  }
});
