// SPDX-License-Identifier: Apache-2.0
// The gates this face is held to, written so that they can be fired at something that
// breaks them.
//
// Two populations, because the two failures are different in kind. The source checks
// read the shipped files and ask what the face is capable of -- a line that could
// reach a network is a hole whether or not it ran today. The tree checks read what was
// actually drawn and ask what a reader was shown. A rule of the second kind that
// passes when nothing was drawn is not a rule, so the mark check refuses an empty
// population rather than reporting green on it.
//
// Comments are stripped before the source checks run, because a rule that a comment
// can trip is a rule that punishes explaining yourself. The stripper keeps quoted text
// intact, so a string that happens to hold two slashes does not eat the rest of a line.
//
// This file is a Node-only audit tool -- it reads faces/ledger's shipped sources off
// disk with node:fs -- and nothing in the browser import graph (index.mjs, ledger.mjs,
// binding.mjs, declaration.mjs) ever imports it. It moved here from the face root
// (req/02 W15's census: a node:fs file sitting beside the browser-loaded modules reads
// as though it might be reachable even when it is not) so that everything left at the
// face root is, in fact, part of what a shell mounts.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { DECLARATION } from '../declaration.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
/** The face root: this tool reads the face's shipped sources and named tests, both of
 * which live one directory up from tools/, not beside this file. */
const FACE_ROOT = path.resolve(HERE, '..');

/** What ships. The instruments and the tests are not in this list and are not scanned. */
export const SHIPPED = Object.freeze(['declaration.mjs', 'binding.mjs', 'ledger.mjs', 'index.mjs']);

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

/**
 * Source rules. Each is a thing the face must be structurally unable to do, and each
 * one has a planted string in the test that makes it red.
 */
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
    name: 'nothing is positioned; the note that overlapped a row was an absolutely positioned element',
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
    name: 'a row that has been written is never edited; undo appends a child row instead',
    pattern: /\brecord\.[A-Za-z_]+\s*=[^=]/,
  },
  // Owner #348 (5): three things that were decided once for the whole application and
  // must therefore not be re-decided in a face. Each is a number a face could pick by
  // eye, and each has a name to ask for instead, so the check is for the number.
  {
    id: 'no-raw-motion',
    name: 'the motion route owns durations; a face writes neither a transition nor a millisecond',
    pattern: /transition\s*:|\b\d+\s*ms\b/,
  },
  {
    id: 'no-raw-corner',
    name: 'the corner scale owns radii; a face asks for a tier by name and never spells a length',
    // Bounded repetition, not `[\s'":]+`: the quoted-key form is exactly four
    // non-word characters (`': '`) and an unbounded class that also matches
    // whitespace backtracks for a very long time on the long comment-free lines
    // this file scans. A gate that hangs is a gate nobody runs.
    pattern: /border-radius\W{1,4}[\d.]+\s*(px|rem|em|%)/,
  },
  // Owner #348 (4), the line-breaking half. `anywhere` breaks a word at any character
  // it likes, which is what put "repor / t.md" and "cl / ipped" on two lines of the
  // same capture; `break-word` breaks only a word that cannot fit on a line of its
  // own, which is the case this face actually has to survive. The two words are one
  // character apart in the source and a mile apart on the page.
  {
    id: 'no-mid-word-breaking',
    name: 'no text on this face breaks mid-word: a long value takes a line, it does not take half a syllable',
    pattern: /overflow-wrap\W{1,4}anywhere/,
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
    const mark = node.attrs['data-mark'];
    // The rule is that no meaning is carried by two drawings, so the population is
    // drawings. A node that names the meaning it is about and draws no mark of its own
    // is a container around the one that does, and reading its absent mark as a second
    // mark for the meaning fires this rule on a standing drawn exactly once.
    //
    // Found by this gate going red on the box-head pill, which is the only way to learn
    // that a rule counts the wrong thing. The instance that found it has since been
    // fixed on the other side as well (parts/src/verdict-badge.mjs's chip() no longer
    // puts the meaning on its wrapper -- 4b09865), so this line is not load-bearing for
    // that one part any more. It stays because the population is the thing this rule is
    // about: the next container to name a meaning must not turn a gate red either.
    if (mark === undefined || mark === null) continue;
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
 * The same rules, from a command line, against the same three states the fixtures are
 * photographed from -- so a person can run this and read what held, rather than reading
 * an exit code alone.
 *
 * Run bare, this file printed nothing and exited 0, which is exactly what a gate that
 * had run and held would do and exactly what a gate that had done nothing would do.
 * The two are now different on the screen. The tree rules need something drawn to be
 * applied to, and they refuse an empty population by design, so the states are the
 * fixtures' own rather than an empty list.
 */
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { face } = await import('../ledger.mjs');
  const { STATES } = await import('./fixture.mjs');
  const trees = Object.values(STATES).map((state) => face.view(state));
  const result = report({ trees });
  for (const check of result.checks) {
    process.stdout.write(`${check.holds ? 'held  ' : 'FELL  '}${check.id}: ${check.detail}\n`);
  }
  process.stdout.write(`${result.checks.length} checks, ${result.failing.length} fell, over ${trees.length} states\n`);
  process.exitCode = result.holds ? 0 : 1;
}
