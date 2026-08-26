// SPDX-License-Identifier: Apache-2.0
// Make each gate fail on purpose, once, and say so.
//
// A gate that has never gone red is a gate nobody has evidence about. It may be watching
// the wrong file, it may be matching nothing, it may be looking at an empty population --
// all of which look exactly like "held" from the outside. So every gate in tools/gates.mjs
// gets one fault injected into a copy of the tree, and is required to notice.
//
// The copy is the point: the tree this runs against is a temporary duplicate, and the
// working tree is never written to. A check that edited its subject would be measuring
// the edit.
//
//   node tools/negative.mjs
//
// Exit 0 when every gate went red for its own injection, 1 otherwise.

import { cpSync, mkdtempSync, readFileSync, writeFileSync, rmSync, appendFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { dirname, join } from 'node:path';
import { execFileSync } from 'node:child_process';

const HERE = dirname(fileURLToPath(import.meta.url));
const SHELL = join(HERE, '..');

/** One fault per gate, each the smallest thing that ought to be caught. */
const INJECTIONS = [
  {
    gate: 'W1 the frame names no face',
    said: 'write one face id into the frame',
    apply: (root) => append(join(root, 'kernel', 'slots.mjs'), "\nexport const ONE_FACE = 'probe-a';\n"),
  },
  {
    gate: 'W14 the frame holds no written-out list of faces',
    said: 'hand-write a strip of face names into the frame',
    apply: (root) => append(join(root, 'kernel', 'slots.mjs'), "\nexport const STRIP = ['finder-one', 'finder-two'];\n"),
  },
  {
    gate: 'W14 every declared field has a reader, and the reader reads it',
    said: 'declare a field nothing reads',
    apply: (root) => swap(join(root, 'kernel', 'manifest.mjs'),
      "  { name: 'condition', required: false, reader: 'kernel/render.mjs' },",
      "  { name: 'condition', required: false, reader: 'kernel/render.mjs' },\n  { name: 'accent', required: false, reader: 'kernel/dismiss.mjs' },"),
  },
  {
    gate: 'W14 the rail and the docks are within their stated capacities',
    said: 'declare one face past the rail\'s stated ceiling',
    apply: (root) => swap(join(root, 'kernel', 'manifest.mjs'),
      'export const RAIL = Object.freeze({ capacity: 8 });',
      'export const RAIL = Object.freeze({ capacity: 3 });'),
  },
  {
    gate: 'W3 nothing copies the whole state',
    said: 'take a snapshot',
    apply: (root) => append(join(root, 'kernel', 'state.mjs'), '\nexport const snapshot = (state) => structuredClone(state);\n'),
  },
  {
    gate: 'W3 every act declares an inverse, and the face track keeps none',
    said: 'drop one act\'s invert field',
    apply: (root) => swap(join(root, 'kernel', 'acts.mjs'),
      "  'dock:size': {\n    track: TRACK.SHELL, invert: 'captured-closure', layer: 'dock', operation: 'update',",
      "  'dock:size': {\n    track: TRACK.SHELL, layer: 'dock', operation: 'update',"),
    loadFails: true,
  },
  {
    gate: 'W4 one assignment can reach the state',
    said: 'add a second way to write the state',
    apply: (root) => swap(join(root, 'kernel', 'state.mjs'),
      '  get state() { return this.#current; }',
      '  get state() { return this.#current; }\n\n  force(next) { this.#current = next; }'),
  },
  {
    gate: 'W9 the keyboard names only verbs that exist',
    said: 'bind a chord to a verb nobody wrote',
    apply: (root) => swap(join(root, 'kernel', 'keys.mjs'),
      "  { chord: 'mod+k', verb: 'theme:set', said: 'change the theme' },",
      "  { chord: 'mod+k', verb: 'theme:set', said: 'change the theme' },\n  { chord: 'mod+p', verb: 'pane:maximise', said: 'a verb nobody wrote' },"),
    loadFails: true,
  },
  {
    gate: 'W9 no face carries a layer number, a chord or a composition check',
    said: 'let a face decide about input methods for itself',
    apply: (root) => append(join(root, 'demo', 'faces', 'sheet-a', 'index.mjs'),
      '\nexport const typing = (event) => (event.isComposing ? null : event.key);\n'),
  },
  {
    gate: 'W10 no eval, and the policy is stated on the page',
    said: 'build a function out of text',
    apply: (root) => append(join(root, 'kernel', 'slots.mjs'), '\nexport const late = (text) => new Function(text);\n'),
  },
  {
    gate: 'W11 nothing here imports an SDK, and only the membrane reaches a network',
    said: 'open a connection from the frame',
    apply: (root) => append(join(root, 'kernel', 'slots.mjs'), "\nexport const ask = () => fetch('/healthz');\n"),
  },
  {
    gate: 'W2 a face imports nothing at all',
    said: 'let a face import the frame',
    apply: (root) => prepend(join(root, 'demo', 'faces', 'probe-a', 'index.mjs'), "import { SLOTS } from '../../../kernel/slots.mjs';\n"),
  },
  {
    gate: 'the frame writes no colour, and no borrowed symbol reaches the page',
    said: 'write one colour into the frame',
    apply: (root) => append(join(root, 'kernel', 'shell.css'), '\n.strip-digest { color: #7c848c; }\n'),
  },
  {
    gate: 'every mark is drawn at a size the caller chose',
    said: 'ask for a mark without a size',
    apply: (root) => swap(join(root, 'kernel', 'render.mjs'),
      'tool.append(mark(name, MARK_SIZE.tab));',
      'tool.append(mark(name));'),
  },
];

function append(path, text) { appendFileSync(path, text, 'utf8'); }
function prepend(path, text) { writeFileSync(path, text + readFileSync(path, 'utf8'), 'utf8'); }
function swap(path, from, to) {
  const text = readFileSync(path, 'utf8');
  if (!text.includes(from)) throw new Error(`the injection point is gone from ${path}: ${from.slice(0, 50)}`);
  writeFileSync(path, text.replace(from, to), 'utf8');
}

function copy() {
  const root = mkdtempSync(join(tmpdir(), 'gx-shell-negative-'));
  const to = join(root, 'shell');
  cpSync(SHELL, to, { recursive: true, filter: (path) => !path.includes('node_modules') });
  return { root, to };
}

const results = [];
for (const injection of INJECTIONS) {
  const { root, to } = copy();
  let outcome;
  try {
    injection.apply(to);
    let json = null;
    let loadFailed = false;
    try {
      json = execFileSync(process.execPath, [join(to, 'tools', 'gates.mjs'), '--json'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
    } catch (error) {
      // A gate can also be met by the table refusing to load at all -- several of these
      // are checked twice, once when the module is read and once by the gate. Both are a
      // refusal; neither is the fault passing.
      if (error.stdout) json = error.stdout;
      loadFailed = !json || json.trim() === '';
    }
    if (loadFailed) {
      outcome = { red: injection.loadFails === true, how: 'the table refused to load' };
    } else {
      const read = JSON.parse(json);
      const gate = read.answers.find((a) => a.name === injection.gate);
      outcome = gate
        ? { red: gate.ok === false, how: gate.faults[0] ?? 'held' }
        : { red: false, how: `no gate is called "${injection.gate}"` };
    }
  } catch (error) {
    outcome = { red: false, how: `the injection itself failed: ${error.message}` };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
  results.push({ gate: injection.gate, said: injection.said, ...outcome });
}

for (const result of results) {
  process.stdout.write(`${result.red ? 'red  ' : 'MISS '} ${result.gate}\n        injected: ${result.said}\n        answer:   ${result.how}\n`);
}
const red = results.filter((r) => r.red).length;
process.stdout.write(`\n${red}/${results.length} gates went red for their own injection\n`);
process.exit(red === results.length ? 0 : 1);
