// SPDX-License-Identifier: Apache-2.0
// The gates that watch the parts' own shape.
//
// Every one of them is fired red at least once in test/boundary.test.mjs. A gate
// that has never refused anything is a gate for which nobody has evidence.
//
// The line these hold is the one in req/04 §1: a drawing part draws and does not
// judge, a deciding part decides and never reaches a document. The population is
// checked for being non-empty first, because a rule over nothing passes (req/04 Q4).

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { FORBIDDEN_TOKENS as FORBIDDEN } from '../src/tokens.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const SRC_DIR = path.resolve(HERE, '../src');

export const BOUNDARY_MESSAGES = {
  EMPTY_POPULATION: 'the rule was applied to nothing, which is not the same as the rule holding',
  CROSSED: 'a part crossed the line between drawing and deciding',
  CLEAN: 'no offenders',
};

/**
 * Which module is which kind. Written out rather than inferred from a filename,
 * because a rule that reads its own population off a naming convention stops holding
 * the day somebody names a file badly.
 */
export const KINDS = Object.freeze({
  'element.mjs': 'C',
  'tokens.mjs': 'C',
  'glyph-sheet.mjs': 'C',
  'verdict-badge.mjs': 'C',
  'receipt-row.mjs': 'C',
  'surface.mjs': 'C',
  'provenance-fold.mjs': 'C',
  'serial.mjs': 'C',
  'seal-claim.mjs': 'M',
  'row-order.mjs': 'M',
  'checkable.mjs': 'M',
  'reversibility.mjs': 'M',
});

export function sourceFiles(dir = SRC_DIR) {
  return fs.readdirSync(dir).filter((f) => f.endsWith('.mjs')).sort()
    .map((name) => {
      const text = fs.readFileSync(path.join(dir, name), 'utf8');
      return { name, path: path.join(dir, name), text, code: codeOf(text) };
    });
}

/**
 * The file with its prose blanked out, line numbers kept.
 *
 * Two kinds of gate live in this file and they want different inputs. A gate about
 * what the code does -- does this module reach a document, does it compare a verdict,
 * does it spell a colour -- has to read code, because a comment saying "there is no
 * document in this file" is the module telling the truth and would otherwise be
 * counted as the offence. A gate about what ships in the bytes -- mixed script,
 * borrowed symbols -- reads the whole file, comments included, because those matter
 * wherever they are. Blanking rather than deleting keeps the line numbers, so an
 * offender is still reported at the line a person would open.
 */
export function codeOf(text) {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, (block) => block.replace(/[^\n]/g, ' '))
    .split(/\n/)
    .map((line) => line.replace(/(^|[^:])\/\/.*$/, (_, lead) => lead))
    .join('\n');
}

export const filesOfKind = (kind, files = sourceFiles()) => files.filter((f) => KINDS[f.name] === kind);

function offendersByLine(text, pattern) {
  const hits = [];
  text.split(/\r?\n/).forEach((line, i) => {
    for (const match of line.matchAll(pattern)) hits.push({ line: i + 1, text: line.trim(), match: match[0] });
  });
  return hits;
}

/**
 * A deciding part must not be able to reach a document. The names below are the ones
 * that would let it.
 */
const DOM_REACH = /\b(document|window|HTMLElement|createElement|createElementNS|innerHTML|outerHTML|appendChild|querySelector|getElementById|setAttribute)\b/g;

export const domReach = (text) => offendersByLine(text, DOM_REACH);

/**
 * A drawing part must not take a different route because of what a verdict says. It
 * may look a mark up by name -- that is a table, and a table is data. What it may not
 * do is compare.
 */
const VERDICT_WORDS = 'Admit|Deny|Escalate';
const VERDICT_BRANCH = new RegExp(
  `(?:(?:===|!==|==|!=)\\s*['"\`](?:${VERDICT_WORDS})['"\`])`
  + `|(?:['"\`](?:${VERDICT_WORDS})['"\`]\\s*(?:===|!==|==|!=))`
  + `|(?:case\\s*['"\`](?:${VERDICT_WORDS})['"\`])`
  + '|(?:switch\\s*\\([^)]*verdict)',
  'g',
);

export const verdictBranches = (text) => offendersByLine(text, VERDICT_BRANCH);

/** A colour spelled anywhere but in the stylesheet of record (req/04 T-2, T-3). */
const COLOUR_LITERAL = /#[0-9a-fA-F]{3,8}\b|\brgba?\s*\(|\bhsla?\s*\(/g;

export const colourLiterals = (text) => offendersByLine(text, COLOUR_LITERAL);

/**
 * Borrowed marks. A general-purpose symbol is somebody else's drawing with somebody
 * else's meaning, and it arrives at a size nobody chose. The gate is wider than the
 * named list: any character outside ASCII, which also catches the mixed-script text
 * that must not reach shipped source.
 */
const NAMED_SYMBOLS = /[●◆◇◈▾▴★■⏺]/g;
const NON_ASCII = /[^\x00-\x7F]/g;

export const namedSymbols = (text) => offendersByLine(text, NAMED_SYMBOLS);
export const nonAscii = (text) => offendersByLine(text, NON_ASCII);

/**
 * Tokens this package has measured and refused. The gate looks for the reference
 * form -- var(--name) -- and not for the bare name, so the roster that declares which
 * tokens are refused does not report itself as the offender.
 */
export const forbiddenTokens = (text, forbidden) => offendersByLine(
  text,
  new RegExp(`var\\(\\s*(?:${forbidden.map((t) => t.replace(/-/g, '\\-')).join('|')})\\s*\\)`, 'g'),
);

/**
 * A denominator typed by hand. The shipped page said "sixty-four" about a sixteen
 * character digest and the check that guarded the sentence looked for a different
 * phrase (req/04a C6). Numbers about the data come from the data.
 */
const TYPED_DENOMINATOR = /\b(sixty-four|thirty-two|sixty four)\b/gi;

export const typedDenominators = (text) => offendersByLine(text, TYPED_DENOMINATOR);

/** Red is spent in one place (req/04 T-5). This counts the places. */
const RED_SPEND = /CONSUMED\.deny/g;

export const redSpendSites = (files = sourceFiles()) => files
  .flatMap((f) => offendersByLine(f.code ?? codeOf(f.text), RED_SPEND).map((hit) => ({ file: f.name, ...hit })));

/**
 * Every element is built in one place. The tree this replaces had a palette that
 * assembled its own <svg> and <use> by hand rather than calling the shared glyph, and
 * that second builder is where the unsized glyph came from -- the sizing rule was
 * written for the first builder's container (req/04a, palette note). So there is one
 * door, element.mjs, and this counts anybody else who opens their own.
 */
export const ELEMENT_DOOR = 'element.mjs';
const RAW_ELEMENT_BUILD = /createElementNS|createElement\b/g;

export const rawSvgBuilders = (files = sourceFiles()) => files
  .filter((f) => f.name !== ELEMENT_DOOR)
  .flatMap((f) => offendersByLine(f.code ?? codeOf(f.text), RAW_ELEMENT_BUILD).map((hit) => ({ file: f.name, ...hit })));

const code = (f) => f.code ?? codeOf(f.text);

/** The whole sweep, as one report. Code gates read code; byte gates read bytes. */
export function survey(files = sourceFiles()) {
  const drawing = filesOfKind('C', files);
  const deciding = filesOfKind('M', files);
  return {
    counted: files.length,
    drawing: drawing.map((f) => f.name),
    deciding: deciding.map((f) => f.name),
    populationsNonEmpty: drawing.length > 0 && deciding.length > 0,
    domInDeciding: deciding.flatMap((f) => domReach(code(f)).map((h) => ({ file: f.name, ...h }))),
    verdictBranchesInDrawing: drawing.flatMap((f) => verdictBranches(code(f)).map((h) => ({ file: f.name, ...h }))),
    colourLiterals: files.flatMap((f) => colourLiterals(code(f)).map((h) => ({ file: f.name, ...h }))),
    forbiddenTokens: files.flatMap((f) => forbiddenTokens(code(f), FORBIDDEN).map((h) => ({ file: f.name, ...h }))),
    typedDenominators: files.flatMap((f) => typedDenominators(code(f)).map((h) => ({ file: f.name, ...h }))),
    nonAscii: files.flatMap((f) => nonAscii(f.text).map((h) => ({ file: f.name, ...h }))),
    namedSymbols: files.flatMap((f) => namedSymbols(f.text).map((h) => ({ file: f.name, ...h }))),
    redSpendSites: redSpendSites(files),
    rawSvgBuilders: rawSvgBuilders(files),
  };
}
