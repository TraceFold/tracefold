// SPDX-License-Identifier: Apache-2.0
// The gates, run as part of the suite so that a change which breaks one is a failing test
// rather than a thing somebody remembers to check.
//
// The negative controls are not run from here: they copy the tree and shell out fourteen
// times, which is a slow thing to put in front of every run. They are a separate command
// (tools/negative.mjs) and their result belongs in the report.

import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { cpSync, mkdtempSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { dirname, join } from 'node:path';

import { runGates, SHELL } from '../tools/gates.mjs';
import { MANIFEST } from '../demo/manifest.gen.mjs';

test('every gate holds, and none of them is looking at nothing', () => {
  const outcome = runGates();
  const fell = outcome.answers.filter((a) => !a.ok);
  assert.deepEqual(fell.map((a) => `${a.name}: ${a.faults.join('; ')}`), [], 'a gate fell');
  assert.deepEqual(outcome.emptyPopulations, [], 'a gate passed over an empty population');
  assert.ok(outcome.total >= 12, `only ${outcome.total} gates exist`);
  process.stdout.write(`# gates: ${outcome.held}/${outcome.total} held\n`);
});

test('the generated manifest is what the folder says, and the folder is the source', () => {
  // The one requirement that cannot be checked by reading: delete a face's folder and it
  // has to leave the rail, the docks and the tabs with nothing in the frame edited.
  const going = MANIFEST.faces[0].id;
  const root = mkdtempSync(join(tmpdir(), 'gx-shell-drop-'));
  try {
    const to = join(root, 'shell');
    cpSync(SHELL, to, { recursive: true });
    execFileSync(process.execPath, [join(to, 'tools', 'gen_manifest.mjs'), '--drop', going], { stdio: 'pipe' });
    const after = readFileSync(join(to, 'demo', 'manifest.gen.mjs'), 'utf8');
    assert.ok(!after.includes(`"${going}"`), `${going} is still in the manifest after its folder was dropped`);

    const modules = readFileSync(join(to, 'demo', 'modules.gen.mjs'), 'utf8');
    assert.ok(!modules.includes(`'${going}'`), `${going} is still in the module map`);

    // And the frame is untouched: byte-identical to the one in the working tree.
    for (const file of ['kernel/shell.mjs', 'kernel/render.mjs', 'kernel/manifest.mjs']) {
      assert.equal(readFileSync(join(to, file), 'utf8'), readFileSync(join(SHELL, file), 'utf8'), `${file} was edited to drop a face`);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('the checked-in manifest is current with the faces folder', () => {
  execFileSync(process.execPath, [join(SHELL, 'tools', 'gen_manifest.mjs'), '--check'], { stdio: 'pipe' });
  execFileSync(process.execPath, [join(SHELL, 'tools', 'gen_routes.mjs'), '--check'], { stdio: 'pipe' });
});

test('the stand-in port offers only methods the membrane actually serves', async () => {
  const { standInPort } = await import(pathToFileURL(join(SHELL, 'demo', 'port.mock.mjs')).href);
  const { tableRows } = await import(pathToFileURL(join(SHELL, '..', 'membrane', 'src', 'index.mjs')).href);
  const served = new Set(tableRows().map((row) => row.name));
  const offered = Object.keys(standInPort()).filter((name) => !['routes', 'ledgers'].includes(name));
  assert.ok(offered.length >= 20, `the stand-in offers ${offered.length} methods`);
  for (const name of offered) assert.ok(served.has(name), `the stand-in offers "${name}", which the membrane does not serve`);
  assert.equal(offered.length, served.size, 'the stand-in and the membrane disagree about how many routes there are');
});
