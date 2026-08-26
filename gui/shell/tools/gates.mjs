// SPDX-License-Identifier: Apache-2.0
// The gates over the source itself, and the count each one is standing on.
//
// These exist because the requirements they enforce are the ones a principle cannot hold.
// The system this shell was derived from wrote down "we do not make places, so a rail
// contradicts our own rule", and on the same day added a rail, hand-wrote a list of face
// names into it, and never revised the page that had refused it. Nothing was dishonest;
// there was simply nothing that would fail. A rule can be withdrawn in silence. An exit
// code cannot.
//
// Every gate reports the size of what it looked at as well as what it found, because a
// gate over an empty population passes forever.
//
//   node tools/gates.mjs [--json]
//
// Exit 0 when every gate holds, 1 otherwise.

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, relative } from 'node:path';

import { MANIFEST } from '../demo/manifest.gen.mjs';
import { FACE_FIELDS, RAIL, DOCK_RULES } from '../kernel/manifest.mjs';
import { ACTS, VERBS, TRACK } from '../kernel/acts.mjs';
import { KEYMAP, HISTORY_VERBS, VIEW_VERBS } from '../kernel/keys.mjs';

/** How many verbs may claim to change only what is shown, before that is the rule. */
const VIEW_VERB_CEILING = 3;

const HERE = dirname(fileURLToPath(import.meta.url));
export const SHELL = join(HERE, '..');

const walk = (dir, out = []) => {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) walk(path, out);
    else out.push(path);
  }
  return out;
};

const named = (path) => relative(SHELL, path).split('\\').join('/');

/** The shell's own source: the frame and its assembly. Not the demo, not the tools. */
export const kernelFiles = () => walk(join(SHELL, 'kernel')).filter((p) => /\.(mjs|css)$/.test(p));
export const faceFiles = () => walk(join(SHELL, 'demo', 'faces')).filter((p) => p.endsWith('.mjs'));
export const shippedFiles = () => [
  ...kernelFiles(),
  ...walk(join(SHELL, 'demo')).filter((p) => /\.(mjs|css)$/.test(p) && !p.includes('.gen.')),
  join(SHELL, 'index.html'),
];

const read = (path) => readFileSync(path, 'utf8');

/** Comments and strings are still shell source; a face id in either is still a face id. */
const GATES = [
  {
    name: 'W1 the frame names no face',
    over: () => kernelFiles(),
    run(files) {
      const ids = MANIFEST.faces.map((f) => f.id);
      const found = [];
      for (const path of files) {
        const text = read(path);
        for (const id of ids) if (text.includes(id)) found.push(`${named(path)} says "${id}"`);
      }
      return { population: `${files.length} kernel files against ${ids.length} declared faces`, faults: found };
    },
  },
  {
    name: 'W14 the frame holds no written-out list of faces',
    over: () => kernelFiles(),
    run(files) {
      const found = [];
      for (const path of files) {
        const text = read(path);
        // Two or more quoted, hyphen-shaped names side by side in an array is what a
        // hand-written strip of faces looks like before anyone calls it that.
        const suspicious = text.match(/\[\s*'[a-z][a-z0-9-]*'\s*,\s*'[a-z][a-z0-9-]*'/g) ?? [];
        for (const hit of suspicious) {
          // The frame's own vocabulary. Every word here is a word the shell defines, and
          // none of them is a face; the point of listing them is that anything *not*
          // listed is a name from outside, which is exactly the thing being looked for.
          if (/'(left|right|bottom|stage|row|col|light|dark|find|read|plain|held|space|dock|pane|tab|create|update|delete|verify|inspect|undo|redo|declaration|state|shell|focus|scroll|hover)'/.test(hit)) continue;
          found.push(`${named(path)} carries ${hit}`);
        }
      }
      return { population: `${files.length} kernel files`, faults: found };
    },
  },
  {
    name: 'W14 every declared field has a reader, and the reader reads it',
    over: () => FACE_FIELDS,
    run(fields) {
      const found = [];
      for (const field of fields) {
        const path = join(SHELL, field.reader);
        let text;
        try { text = read(path); } catch { found.push(`${field.name} names ${field.reader}, which does not exist`); continue; }
        if (!text.includes(field.name)) found.push(`${field.name} is declared and ${field.reader} never mentions it`);
      }
      const declared = new Set(fields.map((f) => f.name));
      for (const face of MANIFEST.faces) {
        for (const key of Object.keys(face)) if (!declared.has(key)) found.push(`a face declares "${key}", which the schema does not hold`);
      }
      return { population: `${fields.length} fields over ${MANIFEST.faces.length} faces`, faults: found };
    },
  },
  {
    name: 'W14 the rail and the docks are within their stated capacities',
    over: () => MANIFEST.faces,
    run(faces) {
      const found = [];
      const onRail = faces.filter((f) => f.rail === true).length;
      if (onRail > RAIL.capacity) found.push(`${onRail} faces declared the rail and it holds ${RAIL.capacity}`);
      for (const [side, rule] of Object.entries(DOCK_RULES)) {
        const here = faces.filter((f) => f.place === side);
        if (here.length > rule.capacity) found.push(`${here.length} faces declared the ${side} dock and it holds ${rule.capacity}`);
        for (const face of here) {
          if (!rule.accepts.includes(face.purpose)) found.push(`the ${side} dock takes ${rule.accepts.join(' or ')} and a face states "${face.purpose}"`);
        }
      }
      return { population: `${faces.length} faces, rail ceiling ${RAIL.capacity}`, faults: found };
    },
  },
  {
    name: 'W3 nothing copies the whole state',
    over: () => shippedFiles(),
    run(files) {
      const found = [];
      for (const path of files) {
        const text = read(path);
        for (const shape of ['structuredClone', 'JSON.parse(JSON.stringify', 'JSON.stringify(state', 'JSON.stringify(this.#current']) {
          if (text.includes(shape)) found.push(`${named(path)} carries ${shape}`);
        }
      }
      return { population: `${files.length} shipped files`, faults: found };
    },
  },
  {
    name: 'W3 every act declares an inverse, and the face track keeps none',
    over: () => VERBS,
    run(verbs) {
      const found = [];
      for (const verb of verbs) {
        const act = ACTS[verb];
        if (!('invert' in act)) { found.push(`${verb} declares no invert`); continue; }
        if (act.track === TRACK.FACE && act.invert !== null) found.push(`${verb} is delegated and also keeps an inverse here`);
        if (act.track === TRACK.SHELL && act.invert !== 'captured-closure') found.push(`${verb} does not say what its inverse is`);
        if (act.track === TRACK.FACE && typeof act.delegate !== 'string') found.push(`${verb} names no membrane method`);
      }
      return { population: `${verbs.length} acts`, faults: found };
    },
  },
  {
    name: 'W4 one assignment can reach the state',
    over: () => shippedFiles(),
    run(files) {
      const found = [];
      let sites = 0;
      for (const path of files) {
        const hits = (read(path).match(/this\.#current\s*=/g) ?? []).length;
        sites += hits;
        if (hits > 0 && !path.endsWith('state.mjs')) found.push(`${named(path)} writes the state`);
      }
      if (sites !== 1) found.push(`${sites} assignments reach the state; the act path is the road only while there is one`);
      return { population: `${files.length} shipped files, ${sites} assignment sites`, faults: found };
    },
  },
  {
    name: 'W9 the keyboard names only verbs that exist',
    over: () => KEYMAP,
    run(entries) {
      const found = [];
      for (const entry of entries) {
        const known = entry.verb in ACTS
          || HISTORY_VERBS.includes(entry.verb)
          || VIEW_VERBS.includes(entry.verb);
        if (!known) found.push(`the keyboard names "${entry.verb}"`);
      }
      if (HISTORY_VERBS.length !== 2) found.push(`${HISTORY_VERBS.length} verbs claim to be history; the two exceptions to this gate are undo and redo`);
      // The third kind of verb (a view verb: it changes what is shown and nothing about
      // what is true, req/811 §8-5) is an enumeration and not a door. It is held to a
      // stated ceiling for the same reason history is: an exception list nobody bounds
      // stops being an exception list and becomes the rule.
      if (VIEW_VERBS.length > VIEW_VERB_CEILING) {
        found.push(`${VIEW_VERBS.length} verbs claim to change only what is shown; the ceiling on that exception is ${VIEW_VERB_CEILING}`);
      }
      return {
        population: `${entries.length} chords against ${VERBS.length} acts, ${HISTORY_VERBS.length} history verbs and ${VIEW_VERBS.length} view verbs`,
        faults: found,
      };
    },
  },
  {
    name: 'W9 no face carries a layer number, a chord or a composition check',
    over: () => faceFiles(),
    run(files) {
      const found = [];
      for (const path of files) {
        const text = read(path);
        if (/isComposing|keyCode\s*===?\s*229/.test(text)) found.push(`${named(path)} checks for composition`);
        if (/LAYERS\.|layer:\s*\d/.test(text)) found.push(`${named(path)} carries a dismiss layer`);
        if (/addEventListener\(\s*'keydown'/.test(text)) found.push(`${named(path)} binds the keyboard`);
      }
      return { population: `${files.length} face files`, faults: found };
    },
  },
  {
    name: 'W10 no eval, and the policy is stated on the page',
    over: () => shippedFiles(),
    run(files) {
      const found = [];
      for (const path of files) {
        const text = read(path);
        if (/\beval\s*\(/.test(text) && !path.endsWith('checks.mjs')) found.push(`${named(path)} calls eval`);
        if (/new\s+Function\s*\(/.test(text)) found.push(`${named(path)} builds a function from text`);
      }
      // Both pages, not one. app.html carries the six real faces and index.html carries
      // the demo set, and until this round only the second was ever checked -- so the
      // window a person actually opens was outside the gate that exists to hold its
      // policy.
      const pages = ['index.html', 'app.html'];
      for (const name of pages) {
        const page = read(join(SHELL, name));
        // The policy itself, not the page that carries it.
        //
        // This gate used to read the whole file, and the first version of the clauses
        // below went red on its own documentation: the comment beside the policy
        // explains that script-src carries no 'unsafe-inline', and a grep over the page
        // found those two words next to each other and called it a fault. A rule that
        // fires on the sentence describing it is a rule that punishes writing down why.
        // So the policy is extracted first and every clause is read from the policy.
        const policy = (/http-equiv="Content-Security-Policy"[\s\S]*?content="([^"]*)"/.exec(page) ?? [])[1];
        if (!policy) { found.push(`${name} states no content-security-policy at all`); continue; }
        for (const clause of ["default-src 'none'", "script-src 'self'", "connect-src 'self'"]) {
          if (!policy.includes(clause)) found.push(`${name} does not state ${clause}`);
        }
        // The half that carries the risk, held explicitly rather than by the absence of
        // a check. Relaxing style-src is a decision this project made with a measurement
        // behind it (see app.html); relaxing either of these would be a different thing
        // entirely and must not be able to ride in beside it.
        if (/script-src[^;]*'unsafe-inline'/.test(policy)) found.push(`${name} lets script run from an attribute`);
        if (/'unsafe-eval'/.test(policy)) found.push(`${name} lets script be built from text`);
        // And the half this round fixed: a page that forbids its own styling ships a
        // product with no design on it. Measured through CDP before this clause existed:
        // every face in app.html rendered with 0 applied rules and no inline style in
        // effect, while its fixtures -- which carry no CSP -- rendered correctly, so the
        // tested surface was strictly better than the shipped one and nothing said so.
        if (!/style-src[^;]*'unsafe-inline'/.test(policy)) found.push(`${name} forbids the styling its own faces draw with`);
      }
      return { population: `${files.length} shipped files and ${pages.length} pages`, faults: found };
    },
  },
  {
    name: 'W11 nothing here imports an SDK, and only the membrane reaches a network',
    over: () => shippedFiles(),
    run(files) {
      const found = [];
      for (const path of files) {
        const text = read(path);
        if (/from\s+'@?[a-z0-9@/._-]*(sdk|tracefold|glovrex)[a-z0-9@/._-]*'/i.test(text)) found.push(`${named(path)} imports a package`);
        if (/\bfetch\s*\(|XMLHttpRequest|new\s+WebSocket/.test(text)) found.push(`${named(path)} opens its own connection`);
      }
      return { population: `${files.length} shipped files`, faults: found };
    },
  },
  {
    name: 'W2 a face imports nothing at all',
    over: () => faceFiles(),
    run(files) {
      const found = [];
      for (const path of files) {
        const text = read(path);
        if (/^\s*import\s/m.test(text)) found.push(`${named(path)} imports something`);
        if (/mount\s*=\s*\([^)]*,[^)]*,[^)]*,/.test(text)) found.push(`${named(path)} takes a fourth argument`);
      }
      return { population: `${files.length} face files`, faults: found };
    },
  },
  {
    name: 'the frame writes no colour, and no borrowed symbol reaches the page',
    over: () => shippedFiles(),
    run(files) {
      const found = [];
      const banned = ['●', '◆', '◇', '◈', '▾', '▴', '★', '■', '⏺'];
      for (const path of files) {
        const text = read(path);
        const uncommented = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');
        if (path.endsWith('.css')) {
          const hexes = uncommented.match(/#[0-9a-fA-F]{3,8}\b/g) ?? [];
          const functions = uncommented.match(/\b(rgba?|hsla?|oklch|lab)\s*\(/g) ?? [];
          for (const hit of [...hexes, ...functions]) found.push(`${named(path)} writes ${hit}`);
        }
        if (path.endsWith('checks.mjs') || path.endsWith('gates.mjs')) continue;
        for (const ch of banned) if (text.includes(ch)) found.push(`${named(path)} carries ${ch}`);
      }
      return { population: `${files.length} shipped files`, faults: found };
    },
  },
  {
    name: 'every mark is drawn at a size the caller chose',
    over: () => [join(SHELL, 'kernel', 'marks.mjs'), join(SHELL, 'kernel', 'render.mjs')],
    run(files) {
      const found = [];
      const marks = read(files[0]);
      if (!/setAttribute\('width'/.test(marks) || !/setAttribute\('height'/.test(marks)) found.push('marks.mjs does not set width and height');
      if (!/needs a size in px/.test(marks)) found.push('marks.mjs has a default size');
      const frame = read(files[1]);
      const calls = frame.match(/mark\([^)]*\)/g) ?? [];
      for (const call of calls) if (!/,/.test(call)) found.push(`render.mjs asks for ${call} without a size`);
      return { population: `${calls.length} calls for a mark`, faults: found };
    },
  },
];

export function runGates() {
  const answers = GATES.map((gate) => {
    const population = gate.over();
    const { population: said, faults } = gate.run(population);
    return { name: gate.name, ok: faults.length === 0, population: said, faults };
  });
  const empty = answers.filter((a) => /^0 /.test(a.population));
  return { answers, held: answers.filter((a) => a.ok).length, total: answers.length, emptyPopulations: empty.map((a) => a.name) };
}

if (process.argv[1] && process.argv[1].endsWith('gates.mjs')) {
  const outcome = runGates();
  if (process.argv.includes('--json')) {
    process.stdout.write(`${JSON.stringify(outcome, null, 2)}\n`);
  } else {
    for (const answer of outcome.answers) {
      process.stdout.write(`${answer.ok ? 'held ' : 'FELL '} ${answer.name}  [${answer.population}]\n`);
      for (const fault of answer.faults) process.stdout.write(`        ${fault}\n`);
    }
    process.stdout.write(`\n${outcome.held}/${outcome.total} gates held\n`);
    for (const name of outcome.emptyPopulations) process.stdout.write(`WARNING: ${name} looked at nothing\n`);
  }
  process.exit(outcome.held === outcome.total && outcome.emptyPopulations.length === 0 ? 0 : 1);
}
