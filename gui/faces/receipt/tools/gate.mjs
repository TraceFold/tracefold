// SPDX-License-Identifier: Apache-2.0
// The gates this face is held to, written so that they can be fired at something that
// breaks them.
//
// Same shape as faces/ledger's and faces/held's own gate.mjs (source checks over the
// shipped files, tree checks over what was actually drawn, comments stripped before
// the source checks run, an empty population refused rather than passed). Two checks
// beyond the shared ones are this face's own: `no-boolean-sealed-claim` and
// `seal-claim-mark-matches-standing`, both existing to hold glovrex/req/405 SS5's
// binding rule (render never reaches back into a raw payload for a fact attest
// already decided) at the machine level, not only at the level of a sentence in a
// comment.
//
// This file is a Node-only audit tool -- it reads faces/receipt's shipped sources
// off disk with node:fs -- and nothing in the browser import graph (index.mjs,
// receipt.mjs, binding.mjs, declaration.mjs) ever imports it.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { DECLARATION } from '../declaration.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FACE_ROOT = path.resolve(HERE, '..');

export const SHIPPED = Object.freeze(['declaration.mjs', 'binding.mjs', 'receipt.mjs', 'index.mjs']);

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
 * rules every face in this tree is held to; the eleventh (`no-boolean-sealed-claim`)
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
    name: 'a row that has been written is never edited; there is no act on this screen that could edit one',
    pattern: /\brecord\.[A-Za-z_]+\s*=[^=]/,
  },
  {
    // Owner #348 (5). Fired red against this face's own shipped source before it was
    // written: `git show 3bae9b7:faces/receipt/receipt.mjs` carries
    // `'border-radius': '4px'` in controlToggle, which is one of the four hand-picked
    // corner numbers Owner #349 (1) named. The cure is the token, not a nicer number.
    id: 'no-raw-corner',
    name: 'no corner is a number; the three-tier scale in the roster of record owns every radius on this screen',
    pattern: /border-radius[^,}\n]*\d+\s*(px|rem|em|%)/,
  },
  {
    // The other half of the same directive. A face may reach for the shared route as a
    // class (`gx-move`) but may not respell it: five durations in an application that
    // declared two is how nobody can say what a duration means any more.
    id: 'no-raw-motion',
    name: 'no transition and no duration is written here; the one motion route belongs to the roster of record',
    pattern: /transition\s*:|\b\d+(\.\d+)?ms\b/,
  },
  {
    // Owner #348 (4)'s weight hierarchy, held as a count rather than as an intention.
    // `weight(role)` is the one place this file spells the property, so a second
    // occurrence is a number typed at a call site -- which is the drift, and which no
    // amount of looking at the screen would catch.
    id: 'weight-is-spelled-once',
    name: 'font-weight is written in exactly one place, so a call site names a role and cannot pick a number',
    pattern: /font-weight/,
    atMost: 1,
  },
  {
    id: 'no-boolean-sealed-claim',
    name: 'glovrex/req/405 SS5: render never reads .sealed off a raw payload -- only off the claim claimOf() already decided',
    // Negative lookbehind: "claim.sealed" (the decided answer, legitimate) is
    // excluded; "record.sealed", "body.sealed", "receiptBody.sealed", a bare
    // ".sealed" -- anything else -- is not.
    pattern: /(?<!claim)\.sealed\b/,
  },
]);

/**
 * `atMost` exists for one rule and is named rather than special-cased: a property that
 * must be written exactly once cannot be checked by "never appears", and a rule that
 * cannot be written is a rule that gets left out. Everything without it is a
 * zero-tolerance rule and reads the same as it did before.
 */
/**
 * Nodes this face draws but does not build, which break mid-word and are not this
 * lane's files to change. Named with the reason, and the list is asserted stale-free
 * below, so it cannot grow quietly and cannot outlive the defect it excuses -- the same
 * shape parts/test/glyph-sheet.test.mjs's own floor exception uses.
 */
export const BREAK_EXCEPTIONS = new Map([
  ['receipt-note', 'the opened row\'s note, drawn by the shared row part. It holds a path and a timestamp, which have no spaces to break at, in the same block as its own opening sentence, which does -- so one rule is applied to both and the sentence can break mid-word. Splitting that block is a change to parts/src/receipt-row.mjs, which this lane may not make.'],
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
  const allowed = check.atMost ?? 0;
  return {
    id: check.id,
    name: check.name,
    holds: hits.length <= allowed,
    detail: hits.length <= allowed
      ? `${hits.length} in ${sources.length} files, at most ${allowed} allowed`
      : hits.join(' | '),
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

function textOf(node) {
  if (!node) return '';
  if (node.text !== undefined) return String(node.text);
  return (node.children ?? []).map(textOf).join('');
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
   * This face's own tree check. The row's seal cell draws a glyph whose data-mark is
   * either structure/seal or structure/unsealed (parts/src/receipt-row.mjs, reading
   * claimOf()'s own mark); the verify section separately carries the same claim's
   * standing. The two are computed from the same claimOf() call in receipt.mjs's
   * view(), so they cannot honestly disagree -- this check is what would catch it if a
   * future edit computed them twice and let the two calls drift.
   *
   * It used to read the standing out of the sentence `seal claim: unsealed -- ...`
   * with a regular expression. That sentence has gone (Owner #348 (4): the box head
   * already wears the standing as a pill, so the line was the head said twice), and a
   * prose match was the weaker instrument anyway -- it went quiet rather than red when
   * the words changed, which is a gate that stops testing without saying so. The
   * standing is now on the node as `data-standing`, and a missing attribute fails
   * here rather than passing over an empty set.
   */
  const sealCells = attrs(trees, 'data-cell').filter((n) => n.attrs['data-cell'] === 'seal');
  const sealGlyphs = sealCells.flatMap((cell) => attrs([cell], 'data-mark'));
  const claimLines = attrs(trees, 'data-role').filter((n) => n.attrs['data-role'] === 'seal-claim');
  const MARK_FOR = { sealed: 'structure/seal', unsealed: 'structure/unsealed' };
  const disagreements = [];
  const unstated = claimLines.filter((line) => !MARK_FOR[line.attrs['data-standing']]);
  for (const line of unstated) disagreements.push(`a claim line states no standing this package knows: ${JSON.stringify(line.attrs['data-standing'] ?? null)}`);
  for (const glyph of sealGlyphs) {
    const mark = glyph.attrs['data-mark'];
    if (mark !== MARK_FOR.sealed && mark !== MARK_FOR.unsealed) continue;
    for (const line of claimLines) {
      const wanted = MARK_FOR[line.attrs['data-standing']];
      if (wanted && wanted !== mark) disagreements.push(`row draws ${mark} but the claim line stands ${line.attrs['data-standing']}`);
    }
  }
  checks.push({
    id: 'seal-claim-mark-matches-standing',
    name: 'the row\'s seal mark and the verify section\'s stated standing never disagree, because both are read off the one claimOf() answer',
    holds: sealGlyphs.length > 0 && claimLines.length > 0 && disagreements.length === 0,
    detail: sealGlyphs.length === 0 || claimLines.length === 0
      ? 'no seal mark or no claim line was drawn, so this rule was applied to an empty population and cannot pass'
      : `${sealGlyphs.length} seal marks against ${claimLines.length} claim lines, disagreements: ${disagreements.join(' | ') || 'none'}`,
  });

  /**
   * Owner #348 (4)'s line-breaking half, as a population rather than as an intention.
   *
   * `overflow-wrap:anywhere` is the rule that breaks a word in the middle and can
   * leave one character alone on a line. It is right for a digest or an anchor, which
   * have no spaces to break at, and wrong for every sentence on this screen. So the
   * nodes allowed to carry it are exactly the ones that say they are opaque, and a
   * prose node that reaches for it fails here.
   */
  const styled = attrs(trees, 'style');
  const breaking = styled.filter((n) => /overflow-wrap:anywhere/.test(n.attrs.style));
  const wrongly = breaking.filter((n) => n.attrs['data-text'] !== 'opaque' && !BREAK_EXCEPTIONS.has(n.attrs['data-part']));
  const stale = [...BREAK_EXCEPTIONS.keys()].filter((part) => !breaking.some((n) => n.attrs['data-part'] === part));
  checks.push({
    id: 'only-opaque-values-break-mid-word',
    name: 'only a value with no spaces in it may break mid-word; a sentence breaks between words',
    holds: breaking.length > 0 && wrongly.length === 0 && stale.length === 0,
    detail: breaking.length === 0
      ? 'nothing on this screen breaks mid-word, so this rule was applied to an empty population and cannot pass'
      : `${breaking.length} nodes break mid-word, ${wrongly.length} not declared opaque, ${BREAK_EXCEPTIONS.size} named exception(s), stale exceptions: ${stale.join(', ') || 'none'}`,
  });

  /**
   * Owner #348 (2). Every cell offering a value to take states the whole value, and a
   * cell whose drawn form is a shortening says which -- an entry that handed over six
   * characters of a digest on the one screen built for checking a receipt elsewhere is
   * the worst copy this application could offer.
   */
  const copyCells = attrs(trees, 'data-copy');
  const empty = copyCells.filter((n) => typeof n.attrs['data-copy'] !== 'string' || n.attrs['data-copy'] === '');
  const shortened = copyCells.filter((n) => n.attrs['data-copy-whole'] === 'true');
  const drawnShorter = shortened.filter((n) => textOf(n).trim().length >= n.attrs['data-copy'].length);
  checks.push({
    id: 'copy-hands-over-the-whole-value',
    name: 'a cell offers the whole value, and a cell whose drawn form is shorter says so',
    holds: copyCells.length > 0 && empty.length === 0 && shortened.length > 0 && drawnShorter.length === 0,
    detail: copyCells.length === 0
      ? 'no cell offers a value, so this rule was applied to an empty population and cannot pass'
      : `${copyCells.length} cells offer a value, ${shortened.length} declared shortened, empty: ${empty.length}, declared shortened but not shorter: ${drawnShorter.length}`,
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
 * Run from a command line, this file used to print nothing and exit 0 -- the shape of
 * a gate that has been run and has said nothing, which is indistinguishable from a
 * gate that passed and is worse than one that fails, because somebody reads the exit
 * code and believes it. It now draws every state this face has a fixture for, checks
 * the trees together with the shipped sources, prints one line per check and exits
 * non-zero on the first one that does not hold.
 *
 * The tree checks refuse an empty population by design, so they are applied to the
 * three real states rather than to nothing.
 */
async function main() {
  const { face } = await import('../receipt.mjs');
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
