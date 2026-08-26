// SPDX-License-Identifier: Apache-2.0
// The gates this face is held to, written so that they can be fired at something
// that breaks them.
//
// Two populations. Source checks read the shipped files and ask what the face is
// capable of doing -- a line that could reach a network is a hole whether or not it
// ran today. Tree checks read what a mounted screen actually drew and ask what a
// reader was shown. The second kind refuses to pass on an empty population, because
// a rule that is green when nothing was drawn has not been tested by anything.
//
// Comments are stripped before a source check runs, so that describing a rule in
// prose does not trip the rule it describes. The stripper keeps quoted text intact.
//
// This file is Node-only tooling -- it reads faces/notice's shipped sources off disk
// with node:fs -- and nothing in the browser import graph (index.mjs, notice.mjs,
// binding.mjs, declaration.mjs) ever reaches it.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { DECLARATION } from '../declaration.mjs';
// The tree checks below need something drawn to read. What they read is the same
// population the shipped pages are photographed from (tools/fixture.mjs's own STATES)
// rather than a test helper -- for the reason tools/browser-mount-smoke.mjs states
// about itself: test/ exists to serve `node --test`, and a tool that leans on it
// starts failing for reasons that have nothing to do with what it checks.
import { face } from '../notice.mjs';
import { STATES } from './fixture.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FACE_ROOT = path.resolve(HERE, '..');

/** What ships. Tools and tests are not scanned. */
export const SHIPPED = Object.freeze(['declaration.mjs', 'binding.mjs', 'notice.mjs', 'index.mjs']);

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
 * Source rules. Each is a thing this face must be structurally unable to do, and
 * each one carries a planted string in the test that proves it goes red.
 */
export const CHECKS = Object.freeze([
  {
    id: 'no-network',
    name: 'this face touches no network; the membrane is the only place that does, and this face does not even hold a caller into it',
    pattern: /\bfetch\s*\(|XMLHttpRequest|WebSocket|EventSource|node:http/,
  },
  {
    id: 'no-foreign-import',
    name: 'this face imports no membrane, no shell, no port and no other face',
    pattern: /from\s+['"][^'"]*(membrane|shell|ports?\/|faces\/)[^'"]*['"]/,
  },
  {
    id: 'no-verification',
    name: 'this face does not verify; drawing an unchecked entry as checked is not a mistake this screen can make, because it never claims anything is checked',
    pattern: /\b(verify|verified|isValid|validateSignature|blake3|sha256)\s*\(/,
  },
  {
    id: 'no-actor-named',
    name: 'this face never names who acted; nothing here states an identity',
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
    name: 'nothing is positioned except the one overlay this face draws on purpose, and there is only ever one of it',
    // The pattern was `position\s*:\s*(absolute|fixed|sticky)` and it could only ever
    // have fired on a CSS string -- `'position:absolute;left:0'`, which is exactly the
    // shape of the planted line in the negative control that proved it worked. Nothing
    // in this face is written that way. Every style here is an object whose values are
    // quoted (`position: 'fixed'`), and the quote between the colon and the word is
    // what the old pattern could not cross. So the rule had a negative control that
    // passed, no positive hits ever, and no ability to see the only spelling this file
    // uses; the first line of source it was supposed to catch was written today and it
    // did not notice. Both spellings now.
    pattern: /position['"]?\s*:\s*['"]?(absolute|fixed|sticky)/,
    // A right-click menu is out of flow by definition -- it is drawn over the row a
    // hand is pointing at, at the coordinates of that hand. The rule this replaces
    // was a blanket ban, which is a rule that cannot be obeyed by a screen that has
    // one, so it is a bounded allowance instead: `fixed`, once, and nothing else.
    //
    // `fixed` rather than `absolute` on purpose. An absolutely-positioned node is
    // placed against whichever ancestor happens to be positioned, so it drifts the
    // moment anything above it gains a position of its own; a fixed one is placed
    // against the viewport, which is where the pointer's own coordinates are. And
    // the count is what stops this from becoming a licence: two of them is a screen
    // with two overlays nobody arranged, which is the defect the old ban was aiming
    // at. The tree check `one-overlay-and-it-is-the-menu` below is the other half --
    // it reads what was drawn rather than what was written.
    allow: {
      pattern: /position['"]?\s*:\s*['"]fixed['"]/,
      max: 1,
      why: 'the right-click menu, which is drawn at a pointer and therefore cannot be in flow',
    },
  },
  {
    id: 'no-raw-transition',
    name: 'no movement is written here; there is one motion route for the whole application and a face spends it by name, never by writing a duration of its own',
    pattern: /transition\s*:|[^A-Za-z_$\d]\d+(\.\d+)?ms\b/,
  },
  {
    id: 'no-raw-corner',
    name: 'no corner is chosen by eye; the corner scale owns those and a face names a tier',
    pattern: /border-radius['"]?\s*:\s*['"]?[\d.]/,
  },
  {
    id: 'no-hand-picked-mark-size',
    name: 'no mark is drawn at a number typed here; every size on this face comes from the floors the sheet declares',
    pattern: /\bsize\s*:\s*[\d.]/,
  },
  {
    id: 'no-method-literals-outside-the-declaration',
    name: 'the names of server methods live in a declaration and nowhere else, even in a face that declares none',
    pattern: /['"](?:get|post)_[a-z0-9_]+['"]/,
    exclude: ['declaration.mjs'],
  },
  {
    id: 'no-dynamic-code',
    name: 'no code is built at run time',
    pattern: /\beval\s*\(|new\s+Function\s*\(/,
  },
  {
    id: 'entries-are-not-edited',
    name: 'an entry that has been drawn is never edited; every paint rebuilds every entry fresh',
    pattern: /\brecord\.[A-Za-z_]+\s*=[^=]/,
  },
]);

/**
 * A rule's population, split into what it forbids outright and what it allows a
 * bounded number of.
 *
 * The second half exists for exactly one rule (`nothing-out-of-flow`) and is written
 * generically rather than as a special case inside it, so that the next bounded
 * allowance is a declaration rather than a second branch in this function. An
 * allowance that is over its own bound is reported as a failure naming the bound --
 * the count is the rule, so the number has to be in the sentence.
 */
export function checkSource(check, sources) {
  const hits = [];
  const allowed = [];
  for (const source of sources) {
    if (check.exclude?.includes(source.file)) continue;
    const code = stripComments(source.text);
    for (const line of code.split('\n')) {
      if (!check.pattern.test(line)) continue;
      const where = `${source.file}: ${line.trim().slice(0, 90)}`;
      if (check.allow && check.allow.pattern.test(line)) allowed.push(where);
      else hits.push(where);
    }
  }
  const over = check.allow ? allowed.length - check.allow.max : 0;
  const holds = hits.length === 0 && over <= 0;
  const counted = check.allow ? `, ${allowed.length}/${check.allow.max} allowed (${check.allow.why})` : '';
  return {
    id: check.id,
    name: check.name,
    holds,
    detail: holds
      ? `0 in ${sources.length} files${counted}`
      : [...hits, ...(over > 0 ? [`${allowed.length} of an allowed ${check.allow.max}: ${allowed.join(' | ')}`] : [])].join(' | '),
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
    // Only a node that carries a mark can be a second mark for a meaning. A node that
    // names a meaning and draws no mark of its own is a wrapper around one, and
    // reading it as "(no mark)" made this check go red on a screen drawing exactly one
    // mark for that meaning -- found by running this file from a command line over all
    // three drawn states for the first time, the tree check having only ever been
    // fired at one state (the one that draws no standing chip).
    //
    // The shared part that made it reachable was fixed the same day and independently
    // (parts/src/verdict-badge.mjs no longer puts `data-means` on the chip's wrapper),
    // so nothing in this face reaches this line today. It stays because the sentence
    // it enforces is true whether or not anything currently breaks it, and because the
    // next part to wrap a mark in a labelled span should not redden a face's gate.
    if (!(node.attrs && 'data-mark' in node.attrs)) continue;
    const means = node.attrs['data-means'];
    const mark = node.attrs['data-mark'];
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
   * The other half of `nothing-out-of-flow`, read off what was drawn rather than off
   * what was written.
   *
   * A source count can say "one line writes a position"; only the tree can say "the
   * node that carries it is the menu, and there is one of it on the screen". Both are
   * needed: source alone would pass a face that wrote one positioned style and drew it
   * on forty rows, and a tree alone would pass a face that wrote forty and happened to
   * draw one. The population is per drawn state, so a state that draws no menu is
   * required to draw no positioned node at all.
   */
  const positioned = trees.map((tree) => {
    const hits = [];
    const visit = (node) => {
      if (!node || typeof node.tag !== 'string') return;
      if (/position['"]?\s*:/.test(node.attrs?.style ?? '')) hits.push(node);
      for (const child of node.children ?? []) visit(child);
    };
    visit(tree);
    return hits;
  });
  const menusDrawn = positioned.filter((hits) => hits.length > 0);
  const wrongNode = menusDrawn.flat().filter((node) => node.attrs['data-role'] !== 'menu');
  const stacked = positioned.filter((hits) => hits.length > 1);
  checks.push({
    id: 'one-overlay-and-it-is-the-menu',
    name: 'a state draws at most one positioned node, it is the menu, and at least one of the states drawn here has one',
    holds: menusDrawn.length > 0 && wrongNode.length === 0 && stacked.length === 0,
    detail: menusDrawn.length === 0
      ? 'no state drawn here opened a menu, so this rule was applied to an empty population and cannot pass'
      : `${menusDrawn.length} of ${trees.length} states carry one, wrong node: ${wrongNode.length}, stacked: ${stacked.length}`,
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
 * Run from a command line, this file printed nothing and exited 0 -- which is what a
 * gate that has never been run looks like from the outside, and is indistinguishable
 * from one that passed. Every check above was only ever reached through
 * `test/gate.test.mjs`. The retrofit lane's own verification list says
 * `node faces/notice/tools/gate.mjs` "must print all checks held, exit 0", and until
 * this block existed that sentence was true of a file that checked nothing.
 *
 * The tree checks refuse an empty population by design, so this draws every state the
 * shipped pages carry before asking anything -- a run against nothing drawn would go
 * red here, correctly, rather than quietly pass.
 */
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const trees = Object.values(STATES).map((notices) => face.view(face.read(notices)));
  // A fourth state the shipped pages do not photograph: a row with its menu open.
  // The overlay rule cannot be checked against a screen nobody right-clicked, and a
  // rule applied to an empty population is a rule that has not been applied.
  trees.push(face.view(face.read(STATES.notice, [], { menu: { entry: '3', x: 40, y: 80 } })));
  const result = report({ trees });
  for (const check of result.checks) {
    process.stdout.write(`${check.holds ? 'held' : 'FELL'}  ${check.id}  ${check.detail}\n`);
  }
  process.stdout.write(`${result.checks.length} checks, ${result.failing.length} failing, over ${trees.length} drawn states\n`);
  if (!result.holds) process.exitCode = 1;
}
