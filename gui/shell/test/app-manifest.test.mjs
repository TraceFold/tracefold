// SPDX-License-Identifier: Apache-2.0
// The window that carries the real faces, asked about from outside the window.
//
// req/97's gap-list item 2 was found by grep and not by a screenshot: the shell's only
// generated manifest named seven demo placeholders, so "the machinery for AC-1 exists"
// and "a viewer can reach a second real face" had drifted apart with nothing counting
// the gap. These tests count it. They are deliberately about the generated pair and the
// folder it is generated from, not about a hand-written expectation of what faces exist
// -- the folder is the source, here as everywhere else in this package.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, existsSync, readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { readManifest } from '../kernel/manifest.mjs';
import { MANIFEST } from '../app/manifest.gen.mjs';
import { MODULES } from '../app/modules.gen.mjs';
import { standInPort, NO_MEMBRANE, STAND_IN } from '../app/port.stand-in.mjs';

const SHELL = dirname(dirname(fileURLToPath(import.meta.url)));
const APP_ROOT = dirname(SHELL);
const FACES = join(APP_ROOT, 'faces');

const facesOnDisk = readdirSync(FACES, { withFileTypes: true })
  .filter((e) => e.isDirectory() && existsSync(join(FACES, e.name, 'FACE.json')))
  .map((e) => e.name)
  .sort();

test('every face folder that declares itself is in the window, and nothing else is', () => {
  assert.ok(facesOnDisk.length >= 6, `expected the six real faces on disk, found ${facesOnDisk.length}`);
  assert.deepEqual(MANIFEST.faces.map((f) => f.id).sort(), facesOnDisk);
  assert.deepEqual([...MODULES.keys()].sort(), facesOnDisk);
});

test('each declared face is a real one: its folder is the same folder its own declaration.mjs lives in', async () => {
  for (const id of facesOnDisk) {
    const declaration = await import(`../../faces/${id}/declaration.mjs`);
    assert.equal(declaration.FACE_ID, id, `${id}/declaration.mjs calls itself ${declaration.FACE_ID}`);
    const module = MODULES.get(id);
    assert.equal(typeof module.mount, 'function', `${id} exports no mount`);
  }
});

test('no face is asked for a route it did not declare: FACE.json consumes is its own declaration, not a copy that can drift', async () => {
  for (const face of MANIFEST.faces) {
    const declaration = await import(`../../faces/${face.id}/declaration.mjs`);
    const declared = [...(declaration.DECLARATION.consumes ?? [])].sort();
    assert.deepEqual([...face.consumes].sort(), declared, `${face.id}: FACE.json and declaration.mjs disagree`);
  }
});

test('the shell accepts the real manifest: places, purposes, dock capacities and the rail ceiling all hold', () => {
  const read = readManifest(MANIFEST);
  assert.equal(read.faces.length, facesOnDisk.length);
  const railed = read.faces.filter((f) => f.rail);
  assert.ok(railed.length > 0, 'nothing reaches the rail, so nothing is reachable from the frame');
});

test('the generated pair is current: regenerating from the folder changes nothing', () => {
  execFileSync(process.execPath, [join(SHELL, 'tools', 'gen_manifest.mjs'), '--faces', FACES, '--out', join(SHELL, 'app'), '--check'], { stdio: 'pipe' });
});

test('the port in this window invents nothing and says so in every reply', async () => {
  assert.equal(STAND_IN, true);
  const port = standInPort();
  const folded = await port.fold('get_transformations');
  assert.equal(folded.outcome, 'absent', 'a face must be able to tell "not read" from "empty"');
  assert.equal(folded.reason, NO_MEMBRANE);
  assert.equal(folded.standIn, true);
  assert.equal(Array.isArray(folded.items), false, 'no invented rows');
  const invoked = await port.get_ledger_consistency();
  assert.equal(invoked.outcome, 'absent');
  assert.equal(invoked.standIn, true);
});

test('the page that carries this window loads the app boot and nothing from the demo', () => {
  const page = readFileSync(join(SHELL, 'app.html'), 'utf8');
  assert.match(page, /src="\/app\/boot\.mjs"/);
  assert.equal(page.includes('/demo/'), false, 'the app window must not reach into the demo');
});
