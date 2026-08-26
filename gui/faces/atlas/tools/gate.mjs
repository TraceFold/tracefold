// SPDX-License-Identifier: Apache-2.0
// The gates this face is held to, written so that they can be fired at something that
// breaks them.
//
// Same shape as every other face's own gate.mjs (source checks over the shipped
// files, tree checks over what was actually drawn, comments stripped before the
// source checks run, an empty population refused rather than passed). Four checks
// beyond the ten shared ones are this face's own: `no-hardcoded-subject-open` (a
// source check) and `fold-mark-agrees-with-open-state` (a tree check), both
// existing to hold this face's own binding rule -- a subject's open/closed state is
// always a computed decision (`needsOpen()`), never a hardcoded literal, and the
// fold mark drawn always agrees with the state actually constructed, at the machine
// level, not only at the level of a sentence in a comment -- and two added in the
// Owner #340 round: `no-scrolling-container` (this screen is composed to fit, so it
// declares no scroller and no height bound) and `no-inline-cursor` (the shared rule
// set is the only thing allowed to say what can be pressed, and an inline cursor is
// the one thing that can outrank it -- which is what this face's own controls were
// doing until this round).
//
// Owner #348 (5) adds three more of exactly that kind, deliberately built to the
// shape the two above already have rather than to a second one: a face may not spell
// a duration (`no-raw-motion`), may not pick a corner by eye (`no-raw-corner`), and
// may not write a font weight by hand (`weights-come-from-the-scale`). Each of the
// three names a thing that has one owner elsewhere -- the motion route, the corner
// scale, the type scale -- and each was fired red before it was believed: two on a
// planted string in test/gate.test.mjs, and `no-raw-corner` on shipped code, which
// was carrying `'border-radius': '4px'` on a control.
//
// This file is a Node-only audit tool -- it reads faces/atlas's shipped sources off
// disk with node:fs -- and nothing in the browser import graph (index.mjs,
// atlas.mjs, binding.mjs, declaration.mjs) ever imports it.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { DECLARATION } from '../declaration.mjs';
import { parts } from '../binding.mjs';

/** The floor a mark may be drawn at, from the sheet that declares it -- reached the
 * same way the face reaches it, so this gate and the face cannot hold two numbers. */
const { minReadable: MIN_READABLE } = parts;

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FACE_ROOT = path.resolve(HERE, '..');

export const SHIPPED = Object.freeze(['declaration.mjs', 'binding.mjs', 'atlas.mjs', 'index.mjs']);

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
 * rules every face in this tree is held to; the eleventh (`no-hardcoded-subject-open`)
 * is this face's own.
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
    name: 'the face does not verify; drawing an unchecked record as checked is not this screen\'s job',
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
    name: 'a row that has been read is never edited; a folded subject is a new frozen object, not a mutation of a touch record',
    pattern: /\btouch\.[A-Za-z_]+\s*=[^=]|\brecord\.[A-Za-z_]+\s*=[^=]|\bsubject\.[A-Za-z_]+\s*=[^=]/,
  },
  {
    id: 'no-hardcoded-subject-open',
    name: 'a subject\'s open/closed state is always the result of calling needsOpen(); it is never a hardcoded boolean literal assigned in its place',
    pattern: /\bopen\s*=\s*(true|false)\s*[;,)]/,
    exclude: ['declaration.mjs'],
  },
  {
    // Owner directive #335 (2), held at the source rather than in a screenshot. This
    // face draws no scrolling container and no height-bounded one: it is composed to
    // fit, and the one surface in this application that scrolls inside itself is the
    // detail pane, which this face does not draw. `overflow:hidden` is deliberately
    // not caught -- clipping a cell is not scrolling, and this face clips two.
    id: 'no-scrolling-container',
    name: 'nothing here scrolls inside itself and nothing here is height-bounded; a screen is composed to fit',
    pattern: /overflow(-[xy])?'?\s*:\s*'?(auto|scroll)|max-?height/i,
  },
  {
    // Found by reading the shared rule set against this face's own source: every
    // control here declared `cursor:default` inline, which outranks the rule that
    // draws a pointer over a summary, so the three elements a reader is meant to
    // press were the three saying they could not be. The cure is structural -- no
    // face spells a cursor at all, the way no face spells a colour.
    id: 'no-inline-cursor',
    name: 'no cursor is spelled inline; the shared rule set is the only thing that says what can be pressed',
    pattern: /\bcursor\s*:/,
  },
  {
    // Owner #348 (5), and the same shape as the two rules above it rather than a
    // second one: a thing this face must be structurally unable to spell, with a
    // planted string in test/gate.test.mjs that makes it red.
    //
    // Owner #348 (1) put the whole application's motion on one route -- two durations
    // on one curve, declared in parts/src/surface.mjs and in the frame's stylesheet
    // from the same token pair. A face that writes its own `transition:` is a face
    // that has quietly added a third duration, and the way a system that declared two
    // ends up with five is that each addition looked reasonable where it was written.
    // The bare `ms` half catches the other spelling of the same mistake, a duration
    // typed as a number beside a property rather than taken from `T.motionQuick`.
    id: 'no-raw-motion',
    name: 'no transition and no duration is spelled here; the motion route owns both',
    pattern: /\btransition[a-z-]*\s*:|(?<![\w.])\d+(\.\d+)?\s*ms\b/i,
  },
  {
    // Owner #349 (1)/(5). The corner scale has three tiers named by what a thing IS
    // (chip, control, container), and it exists because this application was drawing
    // square, 2px, 3px, 4px and 8px -- four numbers nobody chose once. A face may
    // still say that something HAS a corner; what it may not do is pick which one by
    // eye. This fired red on shipped code the moment it was written: controlToggle()
    // carried `'border-radius': '4px'`, tier 1's number on a control.
    // The exemption is written as a lookahead that consumes nothing and swallows its
    // own whitespace. Spelled the other way round -- `:\s*(?!T\.radius)` -- the `\s*`
    // backtracks to zero and the lookahead is tested against a space, which is not
    // `T.radius`, so the rule fires on the correct code and cannot be trusted. It did
    // exactly that on first run, on the line this round had just fixed.
    id: 'no-raw-corner',
    name: 'a corner is taken from the scale by name, never written as a number',
    pattern: /border-radius['"]?\s*:(?!\s*T\.radius)/,
  },
  {
    // Owner #348 (3), the hierarchy half, made mechanical rather than incidental.
    // Three weights are declared in atlas.mjs TYPE and every piece of text takes one
    // of them through `typed()`. A hand-written weight anywhere in this face is a
    // fourth tier nobody named -- which is what `'600'`, `'700'` and eleven omissions
    // already were before this round.
    id: 'weights-come-from-the-scale',
    name: 'no font weight is written by hand; the three the type scale declares are the three there are',
    pattern: /'font-weight'\s*:(?!\s*TYPE[.[])/,
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
   * Owner #348 (3), asked of what was drawn rather than of what was typed.
   *
   * parts/test/glyph-sheet.test.mjs holds the floor over the source, which is the
   * right place for it -- it catches a literal in any of the six faces at once. It
   * cannot catch a size this face computes: `size: MARK` is a name, and a name can be
   * bound to anything. This reads the width that actually reached the tree, on all
   * three of the states this face has, and refuses to pass on a population of none.
   */
  const floor = MIN_READABLE;
  const small = glyphs
    .map((n) => ({ mark: n.attrs['data-mark'], size: Number(n.attrs.width) }))
    .filter((g) => !Number.isFinite(g.size) || g.size < floor);
  checks.push({
    id: 'marks-are-at-or-above-the-floor',
    name: `every mark drawn is at or above the readable floor (${floor})`,
    holds: glyphs.length > 0 && small.length === 0,
    detail: glyphs.length === 0
      ? 'no glyphs were drawn, so this rule was applied to an empty population and cannot pass'
      : `${glyphs.length} marks, under ${floor}: ${small.map((g) => `${g.mark} at ${g.size}`).join(', ') || 'none'}`,
  });

  /**
   * This face's own tree check. Every element carrying `data-role="subject"` (or
   * `data-role="control"`) draws exactly one fold mark, and that mark must agree
   * with whether the element's own `open` attribute is present -- the two are
   * computed from the same `needsOpen()` call in atlas.mjs and can never honestly
   * disagree about the same element. This is what would catch a future edit that
   * computed the glyph choice and the `open` attribute independently and let them
   * drift, the same class of check `edge-state-is-not-contradictory` (faces/graph)
   * holds for that face's own two independently-drawn facts.
   */
  const foldables = [...attrs(trees, 'data-role').filter((n) => n.attrs['data-role'] === 'subject' || n.attrs['data-role'] === 'control')];
  const contradictions = [];
  for (const node of foldables) {
    const isOpen = node.attrs['data-open'] === 'true';
    const foldMarks = attrs([node], 'data-mark').filter((n) => n.attrs['data-mark'] === 'structure/fold-shut' || n.attrs['data-mark'] === 'structure/fold-open');
    if (foldMarks.length === 0) { contradictions.push(`${node.attrs['data-path'] ?? node.attrs['data-control'] ?? '(unnamed)'}: no fold mark drawn`); continue; }
    const drawnOpen = foldMarks[0].attrs['data-mark'] === 'structure/fold-open';
    if (drawnOpen !== isOpen) contradictions.push(`${node.attrs['data-path'] ?? node.attrs['data-control'] ?? '(unnamed)'}: open=${isOpen} but drew ${foldMarks[0].attrs['data-mark']}`);
  }
  checks.push({
    id: 'fold-mark-agrees-with-open-state',
    name: 'the fold mark drawn on a subject or control always agrees with whether it was constructed open',
    holds: foldables.length > 0 && contradictions.length === 0,
    detail: foldables.length === 0
      ? 'no foldable subject or control was drawn, so this rule was applied to an empty population and cannot pass'
      : `${foldables.length} foldables, contradictions: ${contradictions.join(' | ') || 'none'}`,
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
 * Run it, and say so.
 *
 * This file had every check in it and no way to run it: `node faces/atlas/tools/gate.mjs`
 * imported the module, ran nothing, printed nothing and exited 0, so a green exit code
 * from this path meant only that the file parsed. The checks were reached from
 * test/gate.test.mjs alone. Found by running the command the lane brief names and
 * reading its (empty) output rather than its exit code.
 *
 * The tree checks need something drawn, so this draws all three of the states the
 * fixture writer already declares -- the answered one, the empty one and the unread one
 * -- and holds the rules over the set of them together. An empty population is refused
 * by the rules themselves (`declared-marks-only`, `fold-mark-agrees-with-open-state`),
 * so a run that drew nothing cannot pass here either.
 */
export async function runGate() {
  const [{ face }, { STATES }] = await Promise.all([import('../atlas.mjs'), import('./fixture.mjs')]);
  return report({ trees: Object.values(STATES).map((state) => face.view(state)) });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const result = await runGate();
  for (const check of result.checks) {
    process.stdout.write(`${check.holds ? 'held' : 'FELL'}  ${check.id}: ${check.detail}\n`);
  }
  process.stdout.write(`${result.checks.length} checks, ${result.failing.length} fell\n`);
  process.exitCode = result.holds ? 0 : 1;
}
