// SPDX-License-Identifier: Apache-2.0
// The gates this face is held to. Same two populations faces/ledger's gate reads --
// shipped source (what the face is capable of) and the drawn tree (what a reader was
// shown) -- because the failures they catch are different in kind and a rule of the
// second kind that passes on an empty population is not a rule.
//
// This file is a Node-only audit tool. Nothing in the browser import graph
// (index.mjs, held.mjs, binding.mjs, declaration.mjs) ever imports it.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { DECLARATION } from '../declaration.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FACE_ROOT = path.resolve(HERE, '..');

export const SHIPPED = Object.freeze(['declaration.mjs', 'binding.mjs', 'held.mjs', 'index.mjs']);

export function stripComments(text) {
  let out = '';
  let i = 0;
  let quote = null;
  while (i < text.length) {
    const c = text[i];
    const next = text[i + 1];
    if (quote) {
      out += c;
      if (c === '\\') { out += next ?? ''; i += 2; continue; }
      if (c === quote) quote = null;
      i += 1;
      continue;
    }
    if (c === '"' || c === "'" || c === '`') { quote = c; out += c; i += 1; continue; }
    if (c === '/' && next === '/') {
      while (i < text.length && text[i] !== '\n') i += 1;
      continue;
    }
    if (c === '/' && next === '*') {
      i += 2;
      while (i < text.length && !(text[i] === '*' && text[i + 1] === '/')) i += 1;
      i += 2;
      continue;
    }
    out += c;
    i += 1;
  }
  return out;
}

export function shippedSources(dir = FACE_ROOT) {
  return SHIPPED
    .map((file) => ({ file, full: path.join(dir, file) }))
    .filter((entry) => fs.existsSync(entry.full))
    .map((entry) => ({ file: entry.file, text: fs.readFileSync(entry.full, 'utf8') }));
}

export const CHECKS = Object.freeze([
  {
    id: 'no-network',
    name: 'the face does not touch a network; the membrane is the only place that does',
    pattern: /\bfetch\s*\(|XMLHttpRequest|WebSocket|EventSource|node:http/,
  },
  {
    id: 'no-foreign-import',
    name: 'the face imports no membrane, no shell, no port and no other face',
    pattern: /from\s+['"][^'"]*(membrane|shell|ports?\/|faces\/)[^'"]*['"]/,
  },
  {
    id: 'no-verification',
    name: 'the face does not verify; drawing an unchecked record as checked is the worst failure available here',
    pattern: /\b(verify|verified|isValid|validateSignature|blake3|sha256)\s*\(/,
  },
  {
    id: 'no-actor-named',
    name: 'the face never names who is acting; an identity a face may state is one it may state falsely',
    pattern: /\bactor\s*:/,
  },
  {
    id: 'no-colour-literal',
    name: 'no colour is spelled out; the roster of record is the only place a colour is written',
    pattern: /#[0-9a-fA-F]{3,8}\b/,
  },
  {
    id: 'no-borrowed-symbol',
    name: 'every mark is bespoke, so no borrowed symbol and no non-ascii character is in the source',
    // eslint-disable-next-line no-control-regex
    pattern: /[^\x00-\x7F]/,
  },
  {
    id: 'nothing-out-of-flow',
    name: 'nothing is positioned; a note that overlapped its row was an absolutely positioned element once, in this project, and must not be again',
    pattern: /position\s*:\s*(absolute|fixed|sticky)/,
  },
  {
    id: 'no-method-literals-outside-the-declaration',
    name: 'the names of server methods live in the declaration and nowhere else',
    pattern: /['"](?:get|post)_[a-z0-9_]+['"]/,
    exclude: ['declaration.mjs'],
  },
  {
    id: 'no-dynamic-code',
    name: 'no code is built at run time',
    pattern: /\beval\s*\(|new\s+Function\s*\(/,
  },
  {
    id: 'rows-are-not-edited',
    name: 'a row that has been acted on is never edited; committing or cancelling re-reads the list instead',
    pattern: /\brecord\.[A-Za-z_]+\s*=[^=]/,
  },
  // Owner #348 (5): the rules a reader would otherwise have to hold in their head,
  // held by a grep instead. Each of the four is a shared decision this face is a
  // consumer of and not an author of, and each one was actually broken here -- the
  // control toggle drew a 4px corner of its own while a three-tier scale existed,
  // and every paragraph on the face carried its own weight and its own break rule.
  {
    id: 'no-face-motion',
    name: 'a face writes no transition and no duration; there is one motion route for the whole application and a face is not it',
    pattern: /transition\s*:|\b\d+\s*ms\b/,
  },
  {
    id: 'no-raw-corner',
    name: 'a face picks no corner by eye; a radius is one of the three tiers the scale declares, named',
    // The lookahead sits before the whitespace, not after it: with `\s*` outside it,
    // a greedy match backtracks to zero spaces and the lookahead then reads the space
    // rather than the token -- which passes every line, including the ones that name
    // a tier correctly, and fails every line that does. Fired red on this face's own
    // source in exactly that shape before it was written this way round.
    pattern: /'border-radius':(?!\s*T\.radius)/,
  },
  {
    id: 'no-raw-weight',
    name: 'a face spells no font weight; the three this file declares are the only ones it can draw',
    pattern: /'font-weight':\s*['"]/,
  },
  {
    id: 'no-mid-word-break',
    name: 'prose does not come apart mid-word: a word breaks only where it cannot fit a line of its own',
    pattern: /'overflow-wrap':\s*'anywhere'/,
  },
  {
    id: 'no-unconditional-seal',
    name: 'every row states a seal hole -- this face draws nothing settled, so no row cell may set data-state to a value other than hole on the seal column',
    pattern: /'data-cell':\s*'seal'[\s\S]{0,120}'data-state':\s*'value'/,
  },
]);

export function checkSource(check, sources) {
  const hits = [];
  for (const source of sources) {
    if (check.exclude?.includes(source.file)) continue;
    const code = stripComments(source.text);
    for (const line of code.split('\n')) {
      if (check.pattern.test(line)) hits.push(`${source.file}: ${line.trim().slice(0, 90)}`);
    }
  }
  return {
    id: check.id,
    name: check.name,
    holds: hits.length === 0,
    detail: hits.length === 0 ? `0 in ${sources.length} files` : hits.join(' | '),
  };
}

function attrs(trees, name) {
  const found = [];
  const visit = (node) => {
    if (!node || typeof node.tag !== 'string') return;
    if (node.attrs && name in node.attrs) found.push(node);
    for (const child of node.children ?? []) visit(child);
  };
  for (const tree of trees) visit(tree);
  return found;
}

export function report({ trees = [], declaration = DECLARATION, sources = shippedSources(), dir = FACE_ROOT } = {}) {
  const checks = CHECKS.map((check) => checkSource(check, sources));

  const marked = attrs(trees, 'data-mark');
  const declared = new Set(declaration.marks.map((m) => m.mark));
  const undeclared = [...new Set(marked.map((n) => n.attrs['data-mark']))].filter((mark) => !declared.has(mark));
  checks.push({
    id: 'declared-marks-only',
    name: 'every mark on the screen was declared, and something was drawn',
    holds: marked.length > 0 && undeclared.length === 0,
    detail: marked.length === 0
      ? 'nothing was drawn, so this rule was applied to an empty population and cannot pass'
      : `${marked.length} marks drawn, undeclared: ${undeclared.join(', ') || 'none'}`,
  });

  const meanings = new Map();
  const collisions = [];
  for (const node of attrs(trees, 'data-means')) {
    const means = node.attrs['data-means'];
    const mark = node.attrs['data-mark'] ?? '(no mark)';
    if (meanings.has(means) && meanings.get(means) !== mark) collisions.push(`${means}: ${meanings.get(means)} and ${mark}`);
    meanings.set(means, mark);
  }
  checks.push({
    id: 'one-meaning-one-mark',
    name: 'no meaning is carried by two marks',
    holds: meanings.size > 0 && collisions.length === 0,
    detail: meanings.size === 0 ? 'no meanings were drawn' : `${meanings.size} meanings, collisions: ${collisions.join(' | ') || 'none'}`,
  });

  const glyphs = attrs(trees, 'data-mark').filter((n) => n.tag === 'svg');
  const unsized = glyphs.filter((n) => !/^\d+$/.test(n.attrs.width ?? '') || !/^\d+$/.test(n.attrs.height ?? ''));
  checks.push({
    id: 'every-glyph-states-its-size',
    name: 'every glyph states a width and a height, so none can fall to a default size',
    holds: glyphs.length > 0 && unsized.length === 0,
    detail: glyphs.length === 0 ? 'no glyphs were drawn' : `${glyphs.length} glyphs, unsized: ${unsized.length}`,
  });

  const sealCells = attrs(trees, 'data-cell').filter((n) => n.attrs['data-cell'] === 'seal');
  const notAHole = sealCells.filter((n) => n.attrs['data-state'] !== 'hole');
  checks.push({
    id: 'seal-column-is-always-a-hole',
    name: 'this is the held screen: no row may ever draw a seal value, only a declared hole',
    holds: sealCells.length > 0 && notAHole.length === 0,
    detail: sealCells.length === 0 ? 'no rows were drawn, so this rule was applied to an empty population and cannot pass' : `${sealCells.length} seal cells, drawn as something other than a hole: ${notAHole.length}`,
  });

  // Owner #348 (2). Two properties of the menu that cannot be read off the source,
  // because both are about what a whole drawn screen holds at one moment.
  const menusPerTree = trees.map((tree) => attrs([tree], 'data-menu').length);
  checks.push({
    id: 'one-menu-at-most',
    name: 'a screen holds one menu or none -- a second right-click replaces the first and can never stack on it',
    holds: menusPerTree.some((n) => n === 1) && menusPerTree.every((n) => n <= 1),
    detail: menusPerTree.length === 0
      ? 'no states were drawn, so this rule was applied to an empty population and cannot pass'
      : `menus drawn per state: ${menusPerTree.join(', ')}`,
  });

  const menuActs = attrs(trees, 'data-role').filter((n) => n.attrs['data-role'] === 'menu-act');
  const declaredActs = new Set(declaration.acts.map((a) => a.act));
  const invented = [...new Set(menuActs.map((n) => n.attrs['data-act']))].filter((act) => !declaredActs.has(act));
  const armedDead = menuActs.filter((n) => n.attrs['data-state'] !== 'open' && (n.attrs['data-target'] ?? null) !== null);
  checks.push({
    id: 'the-menu-offers-what-the-row-offers',
    name: 'every act a menu offers is one this face declares, and one whose gate is not open names no row to send against',
    holds: menuActs.length > 0 && invented.length === 0 && armedDead.length === 0,
    detail: menuActs.length === 0
      ? 'no menu was drawn, so this rule was applied to an empty population and cannot pass'
      : `${menuActs.length} offers, invented: ${invented.join(', ') || 'none'}, dead but armed: ${armedDead.length}`,
  });

  const missing = declaration.tests.filter((named) => !fs.existsSync(path.join(dir, named)));
  checks.push({
    id: 'named-tests-exist',
    name: 'every test the declaration names is on disk, so a test cannot vanish quietly',
    holds: declaration.tests.length > 0 && missing.length === 0,
    detail: missing.length === 0 ? `${declaration.tests.length} named, all present` : `missing: ${missing.join(', ')}`,
  });

  return { checks, holds: checks.every((c) => c.holds), failing: checks.filter((c) => !c.holds) };
}

/**
 * The gate, run as a command.
 *
 * It did not have one. `node faces/held/tools/gate.mjs` loaded this module, declared
 * two constants and a table, printed nothing and exited 0 -- and would have exited 0
 * on a face that failed every check in it, because nothing here was ever called. The
 * checks were real and were only ever run from test/gate.test.mjs; the command that
 * four other faces in this tree already carry, and that this one was verified with,
 * was a command that could not fail. Same shape as theirs, so a reader learns one
 * form: every check printed with its detail, a count, and a non-zero exit if any of
 * them fell.
 *
 * The tree checks refuse an empty population by design, so they are applied to the
 * face's real states rather than to nothing.
 */
async function main() {
  const { face } = await import('../held.mjs');
  const { STATES } = await import('./fixture.mjs');
  const trees = Object.values(STATES).map((state) => face.view(state));
  const result = report({ trees });
  for (const check of result.checks) {
    process.stdout.write(`${check.holds ? 'held' : 'FELL'}  ${check.id}: ${check.detail}\n`);
  }
  process.stdout.write(`${result.checks.length} checks over ${trees.length} states, ${result.failing.length} failing\n`);
  if (!result.holds) process.exitCode = 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`${error.stack ?? error.message}\n`);
    process.exitCode = 1;
  });
}
