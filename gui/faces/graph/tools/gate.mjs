// SPDX-License-Identifier: Apache-2.0
// The gates this face is held to, written so that they can be fired at something that
// breaks them.
//
// Same shape as faces/ledger's, faces/held's and faces/receipt's own gate.mjs
// (source checks over the shipped files, tree checks over what was actually drawn,
// comments stripped before the source checks run, an empty population refused
// rather than passed). Two checks beyond the ten shared ones are this face's own:
// `no-hardcoded-childof` (a source check) and `edge-state-is-not-contradictory` (a
// tree check), both existing to hold req/03 §5's binding rule -- an edge is drawn
// only from a resolved lookup against what this window actually read, never from a
// literal id and never in visible disagreement with the outside-window annotation
// that would say the opposite -- at the machine level, not only at the level of a
// sentence in a comment.
//
// This file is a Node-only audit tool -- it reads faces/graph's shipped sources off
// disk with node:fs -- and nothing in the browser import graph (index.mjs,
// graph.mjs, binding.mjs, declaration.mjs) ever imports it.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { DECLARATION } from '../declaration.mjs';
// The floor is read through the same seam the face draws through, so this tool and the
// face cannot hold two different numbers for one rule (Owner #348 (3)).
import { parts } from '../binding.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FACE_ROOT = path.resolve(HERE, '..');

export const SHIPPED = Object.freeze(['declaration.mjs', 'binding.mjs', 'graph.mjs', 'index.mjs']);

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
 * one has a planted string in the test that makes it red. The first ten are the same
 * rules every face in this tree is held to; the eleventh (`no-hardcoded-childof`) is
 * this face's own.
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
    name: 'the face does not verify; drawing an unchecked chain as checked is not this screen\'s job',
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
    name: 'a row that has been read is never edited; a chained row is a new frozen object, not a mutation of the one before it',
    pattern: /\bnode\.[A-Za-z_]+\s*=[^=]|\brecord\.[A-Za-z_]+\s*=[^=]/,
  },
  {
    id: 'no-hardcoded-childof',
    name: 'req/03 §5: an edge is drawn only from a resolved lookup against what this window read -- childOf is never a literal string, only ever the id of an already-found predecessor node',
    pattern: /childOf\s*:\s*['"`]/,
  },
  /**
   * Owner #348 (5): four things this face is now structurally unable to decide for
   * itself, because each of them is a scale the application declares once and a face
   * that picks its own value is where a scale of two becomes a scale of five.
   *
   * All four are written as source rules rather than as readings of the drawn tree, on
   * purpose: a drawn tree only ever shows the numbers that were reached on that paint,
   * so a fifth duration sitting in a branch no fixture takes would pass a tree check
   * every time and still be in the shipped file.
   */
  {
    id: 'no-raw-motion',
    name: 'Owner #348 (1): how long a thing takes belongs to the motion route in parts/src/surface.mjs, so this face writes no transition and no duration of its own',
    pattern: /transition\s*:|(?<![\w.])\d+(?:\.\d+)?ms\b/,
  },
  {
    id: 'no-raw-corner',
    name: 'Owner #349 (1): a corner is one of three declared tiers reached by name (radiusChip, radiusControl, radiusContainer), never a number chosen at a call site',
    pattern: /border-radius[^;]*\d+\s*(?:px|rem|em|%)/,
  },
  {
    id: 'no-raw-weight',
    name: 'Owner #348 (4): every weight comes from this face\'s own three-step scale, so a fourth weight cannot arrive as a number written at a call site',
    pattern: /font-weight'?\s*:\s*['"]?\d/,
  },
  {
    id: 'no-raw-mark-size',
    name: 'Owner #348 (3): a mark is drawn at a floor this face asks the glyph sheet for by name, never at a number typed beside it',
    pattern: /(?<![-\w])size\s*:\s*\d/,
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

  /**
   * Owner #348 (3), read off what was actually drawn rather than off the source.
   *
   * The source rule above (`no-raw-mark-size`) stops a number being typed beside a mark;
   * this one stops the other half, which is a size arriving correctly named and still
   * being under the floor -- a mark handed a size from a variable, or a shared part
   * drawing at its own number. It is the reading that would have caught the state this
   * face shipped in an hour ago, where two marks were at 14 with the floor at 16.
   */
  const floor = parts.minReadable;
  const belowFloor = glyphs
    .filter((n) => Number(n.attrs.width) < floor || Number(n.attrs.height) < floor)
    .map((n) => `${n.attrs['data-mark']} at ${n.attrs.width}`);
  checks.push({
    id: 'marks-at-or-above-the-floor',
    name: `every mark this face draws is at or above the readable floor the glyph sheet declares (${floor})`,
    holds: glyphs.length > 0 && belowFloor.length === 0,
    detail: glyphs.length === 0
      ? 'no glyphs were drawn, so this rule was applied to an empty population and cannot pass'
      : `${glyphs.length} marks, under ${floor}: ${belowFloor.join(', ') || 'none'}`,
  });

  /**
   * This face's own tree check. A row that carries `data-child-of` (the edge was
   * resolved and is drawn as a chain) must never also be the target of a
   * `data-to`-bearing outside-window annotation (the edge was declared not drawn) --
   * the two are computed from the same `buildGraph()` pass in graph.mjs and can
   * never honestly disagree about the same row. This is what would catch a future
   * edit that computed the two independently and let them drift.
   */
  const childOfRows = attrs(trees, 'data-child-of').filter((n) => n.attrs['data-child-of']);
  const outsideAnnotations = attrs(trees, 'data-to');
  const outsideTargets = new Set(outsideAnnotations.map((n) => n.attrs['data-to']));
  const contradictions = childOfRows
    .map((n) => n.attrs['data-row'])
    .filter((id) => outsideTargets.has(id));
  checks.push({
    id: 'edge-state-is-not-contradictory',
    name: 'a row is never simultaneously drawn as chained (structure/child) and flagged as the target of an edge that was declared not drawn',
    holds: (childOfRows.length + outsideAnnotations.length) > 0 && contradictions.length === 0,
    detail: (childOfRows.length + outsideAnnotations.length) === 0
      ? 'no edge (drawn or declared-outside) was drawn, so this rule was applied to an empty population and cannot pass'
      : `${childOfRows.length} chained rows, ${outsideAnnotations.length} outside annotations, contradictions: ${contradictions.join(', ') || 'none'}`,
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
 * The gates, run from a command line.
 *
 * This file could be imported and its report read (test/gate.test.mjs does exactly
 * that) but until this round it could not be *run*: `node tools/gate.mjs` loaded the
 * module, evaluated nothing, and exited 0 -- an exit code that said "every check
 * held" while no check had been applied to anything. Silence that looks like a pass
 * is the failure mode this whole file exists to refuse, so it is closed here.
 *
 * The trees the tree checks are applied to are the fixture states, imported at call
 * time rather than at the top of the file: tools/fixture.mjs reaches a real disk, and
 * a test that only wants the source checks should not have to load a disk reader to
 * get them.
 */
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { STATES } = await import('./fixture.mjs');
  const { face } = await import('../graph.mjs');
  const result = report({ trees: Object.values(STATES).map((state) => face.view(state)) });
  for (const check of result.checks) {
    process.stdout.write(`${check.holds ? 'held' : 'FELL'}  ${check.id}: ${check.detail}\n`);
  }
  process.stdout.write(`${result.checks.filter((c) => c.holds).length}/${result.checks.length} held, over ${Object.keys(STATES).length} drawn states\n`);
  if (!result.holds) process.exitCode = 1;
}
