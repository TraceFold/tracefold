// SPDX-License-Identifier: Apache-2.0
// The predicates the discipline tests ask, written once.
//
// One question, one function, every site calling it: the audits' repeated finding was
// a rule spelled separately in several places and repaired in one of them.

import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, extname } from 'node:path';

export function sourceFiles(dir, extensions = ['.mjs', '.js']) {
  if (!existsSync(dir)) return [];
  const found = [];
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry.startsWith('.')) continue;
    const path = join(dir, entry);
    const info = statSync(path);
    if (info.isDirectory()) found.push(...sourceFiles(path, extensions));
    else if (extensions.includes(extname(entry))) found.push(path);
  }
  return found;
}

/** Comments carry prose about the wire; the gates below ask about code. */
export function stripComments(text) {
  return text.replace(/\/\*[\s\S]*?\*\//g, ' ').replace(/(^|[^:])\/\/[^\n]*/g, '$1 ');
}

/**
 * Files that spell a wire address themselves instead of taking it from the table.
 * The address is assembled in exactly one module; every other module handles a row.
 */
const LITERAL = /"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`/g;

export function pathLiteralOffenders(files, paths, allow = []) {
  const needles = [...new Set([...paths.map((p) => p.split('{')[0]), '/v1'])].filter((n) => n.length > 2);
  const offenders = [];
  for (const file of files) {
    if (allow.some((a) => file.endsWith(a))) continue;
    const literals = stripComments(readFileSync(file, 'utf8')).match(LITERAL) ?? [];
    // Anywhere inside a literal, not only at its start: the first form of this gate
    // looked for a quote followed by the path and so missed `${origin}/v1/candidates`,
    // which is the exact shape it exists to catch. Its own negative control found that.
    const hits = needles.filter((n) => literals.some((literal) => literal.includes(n)));
    if (hits.length) offenders.push({ file, hits });
  }
  return offenders;
}

/** Words that would mean this layer had started stating verdicts of its own. */
export const JUDGEMENT_WORDS = [
  'isValid', 'verifyReceipt', 'verifyOffline', 'computeVerdict', 'isVerified',
  'markVerified', 'refuted', 'proofValid',
];

/** Identifiers belonging to the reference tree, which this module does not inherit. */
export const FOREIGN_IDENTIFIERS = [
  'allPages', 'PAGE_CEILING', 'RENAME', 'escalationOf', 'oneOf',
  'NO_SURFACE', 'UNDRAWN', 'NO_ROUTE', 'canVerify', 'port.desktop', 'port.web',
];

export function wordOffenders(files, words) {
  const offenders = [];
  for (const file of files) {
    const code = stripComments(readFileSync(file, 'utf8'));
    const hits = words.filter((w) => new RegExp(`(^|[^\\w$])${w.replace(/[.]/g, '\\.')}(?![\\w$])`).test(code));
    if (hits.length) offenders.push({ file, hits });
  }
  return offenders;
}

/** Anything that would let a layer above the membrane reach the network on its own. */
export const NETWORK_WORDS = ['fetch(', 'XMLHttpRequest', 'WebSocket', 'EventSource', "from 'node:http", 'require(\'http'];

export function networkOffenders(files) {
  const offenders = [];
  for (const file of files) {
    const code = stripComments(readFileSync(file, 'utf8'));
    const hits = NETWORK_WORDS.filter((w) => code.includes(w));
    if (hits.length) offenders.push({ file, hits });
  }
  return offenders;
}

// --- the backend-attestation gate --------------------------------------------
//
// The engine deliberately never returns `alg` or `verified` beside a signature
// (gx-api/tests/dr44_9_views.rs:233's two refusals: `verified`/`refuted` belong to
// gx_witness::verify_offline, run where the reader is; `alg` is permanently
// forbidden beside a signature -- NFR-011, req/38 §109/§113 -- the algorithm is a
// property of the key). req/38_ERRATA_2026-08-07.md §497(b) ruled this a negative
// gate: a face or a wire-fields declaration must never name either word as
// something this surface reads off the wire, because that is this surface grading
// its own paper. The predicate is "declared as a field this app receives from the
// backend", not "the word appears" -- a declaration that documents the withholding
// (UNDRAWN, WITHHELD, a code comment, this file's own doc block) is not an
// offender, so the scan looks only at the shapes a face or `wire-fields.json` uses
// to claim a field is present: MARKS entries and `wire-fields.json`'s `fields[]`.

/** The two fields dr44_9_views.rs:233 names. Exact-word, not substring, so a mark
 * like `structure/sealed` or a `means` of `verdict.admit` cannot collide. */
export const BACKEND_WITHHELD_FIELDS = ['alg', 'verified'];

/**
 * `declarations` is an array of face `DECLARATION` objects (one per `faces/<id>/`).
 * `wireFields` is the parsed `membrane/wire-fields.json`. Returns one row per
 * offending entry, naming the face (or `(wire-fields.json)`), the entry that named
 * the word, and which word it was -- the same shape `wordOffenders` returns, so a
 * caller can print offenders the same way for every gate in this file.
 */
export function backendAttestationOffenders(declarations, wireFields) {
  const offenders = [];
  const namesWithheldWord = (text) => BACKEND_WITHHELD_FIELDS.filter(
    (word) => new RegExp(`(^|[^\\w$])${word}(?![\\w$])`, 'i').test(text ?? ''),
  );
  for (const decl of declarations) {
    for (const mark of decl?.marks ?? []) {
      const hits = new Set([...namesWithheldWord(mark.mark), ...namesWithheldWord(mark.means)]);
      if (hits.size) offenders.push({ face: decl.id ?? '(unnamed face)', entry: mark.mark, hits: [...hits] });
    }
  }
  for (const entry of wireFields?.fields ?? []) {
    if (BACKEND_WITHHELD_FIELDS.includes(entry.field)) {
      offenders.push({ face: '(wire-fields.json)', entry: entry.field, hits: [entry.field] });
    }
  }
  return offenders;
}

// --- the copy gate ----------------------------------------------------------

const TOKEN = /[A-Za-z_$][\w$]*|\d+(?:\.\d+)?|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`|=>|===|!==|==|!=|<=|>=|\?\?|&&|\|\||\+\+|--|\S/g;

export function tokenize(text) {
  return stripComments(text).match(TOKEN) ?? [];
}

/**
 * The tokens that carry authorship: names and literals, with the language's own
 * words removed.
 *
 * Measured, not assumed. Run over every token, the gate reported 36 shared runs
 * between this module and the reference tree, and all 36 were punctuation grammar --
 * `new Set ( ) ;`, `. filter ( Boolean ) . map (`, an import line for a Node built-in.
 * None named a mechanism. A gate whose refusals are all of that kind is measuring the
 * language rather than the copying, so the question is asked of the stream where the
 * answer lives: eight consecutive *names* in common is not something two independent
 * authors produce. The raw count is still printed, so narrowing the predicate does not
 * bury the number it was narrowed away from.
 */
const LANGUAGE_WORDS = new Set([
  'const', 'let', 'var', 'function', 'return', 'if', 'else', 'for', 'of', 'in', 'while', 'break',
  'continue', 'new', 'class', 'extends', 'import', 'export', 'from', 'default', 'async', 'await',
  'try', 'catch', 'finally', 'throw', 'typeof', 'instanceof', 'delete', 'void', 'this', 'null',
  'undefined', 'true', 'false', 'switch', 'case', 'do', 'yield', 'static', 'get', 'set',
  'Object', 'Array', 'Set', 'Map', 'JSON', 'String', 'Number', 'Boolean', 'Error', 'TypeError',
  'Promise', 'Math', 'Date', 'RegExp', 'console', 'process', 'globalThis', 'length', 'push',
  'map', 'filter', 'slice', 'split', 'join', 'has', 'add', 'keys', 'values', 'entries',
  'includes', 'replace', 'trim', 'toString', 'startsWith', 'endsWith', 'test', 'exec',
]);

export function tokenizeNames(text) {
  return tokenize(text).filter((t) => /^[A-Za-z_$][\w$]*$/.test(t) && !LANGUAGE_WORDS.has(t));
}

export function nGrams(tokens, n) {
  const grams = new Set();
  for (let i = 0; i + n <= tokens.length; i += 1) grams.add(tokens.slice(i, i + n).join(' '));
  return grams;
}

/**
 * n-grams shared between the new sources and a reference tree.
 * A shared run is not proof of copying, and an absent one is not proof of originality
 * -- the same design under different names passes this. It is a floor, and it is
 * reported as one.
 */
export function sharedRuns(newFiles, referenceFiles, n = 8, stream = 'names') {
  const cut = stream === 'names' ? tokenizeNames : tokenize;
  const reference = new Set();
  for (const file of referenceFiles) for (const g of nGrams(cut(readFileSync(file, 'utf8')), n)) reference.add(g);
  const shared = [];
  for (const file of newFiles) {
    for (const g of nGrams(cut(readFileSync(file, 'utf8')), n)) {
      if (reference.has(g)) shared.push({ file, run: g });
    }
  }
  return shared;
}

// --- multi-root reference tree walk -----------------------------------------

/** Directories a reference-tree walk never enters: dependency vendoring, VCS/tool
 * metadata, and build output that is not authored source. Matched by directory
 * name at any depth, not only at the root. */
export const DEFAULT_EXCLUDE_DIRS = ['node_modules', 'vendor', 'target', '.git'];

/** Extensions a reference-tree walk treats as source worth tokenizing. Kept to code
 * ("source only"): prose/doc/data extensions (.md, .json, .txt, .lock, .toml) are
 * left out on purpose -- their tokens are prose or structured data, not authored
 * mechanism, and folding them in would make the gate noisy instead of exact. */
export const REFERENCE_CODE_EXTENSIONS = ['.mjs', '.js', '.ts', '.rs', '.html', '.css'];

/** Above this, a file is not something a person hand-wrote line by line for this
 * gate to compare against -- it is generated/bundled output (a 18MB graph.html, a
 * minified vendor bundle) and scanning it buys noise and runtime, not evidence. */
export const DEFAULT_MAX_BYTES = 3 * 1024 * 1024;

function looksBinary(buffer) {
  const probe = buffer.subarray(0, Math.min(buffer.length, 8000));
  for (const byte of probe) if (byte === 0) return true;
  return false;
}

/**
 * Walk one reference root and return the files worth comparing against, plus an
 * honest accounting of what was left out and why. A gate that silently drops files
 * is a gate whose "clean" cannot be trusted, so every skip is counted here instead
 * of folded into a bare file count.
 */
export function walkReferenceRoot(root, {
  excludeDirs = DEFAULT_EXCLUDE_DIRS,
  extensions = REFERENCE_CODE_EXTENSIONS,
  maxBytes = DEFAULT_MAX_BYTES,
} = {}) {
  const counts = {
    scanned: 0, skippedExtension: 0, skippedSize: 0, skippedBinary: 0, skippedHidden: 0, excludedDirs: 0,
    skippedUnreadable: 0,
  };
  const files = [];
  if (!existsSync(root)) return { files, counts, present: false };

  const walk = (dir) => {
    let entries;
    try { entries = readdirSync(dir); } catch { counts.skippedUnreadable += 1; return; }
    for (const entry of entries) {
      const path = join(dir, entry);
      // OneDrive placeholder files, cloud-only reparse points and permission-denied
      // spots (all real conditions in these trees) must count as skipped, not crash
      // the walk -- a gate that dies on the third root never scanned the other two.
      let info;
      try { info = statSync(path); } catch { counts.skippedUnreadable += 1; continue; }
      if (info.isDirectory()) {
        if (entry.startsWith('.') || excludeDirs.includes(entry)) { counts.excludedDirs += 1; continue; }
        walk(path);
        continue;
      }
      if (entry.startsWith('.')) { counts.skippedHidden += 1; continue; }
      if (!extensions.includes(extname(entry))) { counts.skippedExtension += 1; continue; }
      if (info.size > maxBytes) { counts.skippedSize += 1; continue; }
      let buffer;
      try { buffer = readFileSync(path); } catch { counts.skippedUnreadable += 1; continue; }
      if (looksBinary(buffer)) { counts.skippedBinary += 1; continue; }
      files.push(path);
      counts.scanned += 1;
    }
  };
  walk(root);
  return { files, counts, present: true };
}

/**
 * Walk several reference roots and merge them into one file list plus one counts
 * table keyed by label, so a multi-tree reference still reports scanned-vs-skipped
 * per root instead of one opaque total that could be hiding a root nobody scanned.
 */
export function walkReferenceTree(roots) {
  const perRoot = {};
  const files = [];
  for (const { label, path, ...opts } of roots) {
    const result = walkReferenceRoot(path, opts);
    perRoot[label] = result;
    files.push(...result.files);
  }
  return { files, perRoot };
}

// --- the standards allowlist -------------------------------------------------

/**
 * A shared run is expected when both sides implement the same public specification
 * (FIPS, W3C, ...) rather than when one copied the other. An allowlist entry says so
 * explicitly, by exact run string, and must carry a citation -- an entry without one
 * is refused rather than silently honoured, so the gate cannot be widened open by a
 * blank entry. This keeps the gate exact: only a named, cited standard clears a run,
 * nothing else does.
 */
export function requireCitation(entry) {
  if (!entry.run || typeof entry.run !== 'string') {
    throw new Error('allowlist entry has no run string to match');
  }
  if (!entry.citation || typeof entry.citation !== 'string' || entry.citation.trim().length < 8) {
    throw new Error(`allowlist entry for "${entry.run}" has no citation -- refusing to honour it`);
  }
  return entry;
}

/**
 * Partition shared runs into the ones a cited standard explains and the ones still
 * unexplained. The match is the exact run string (the full 8-token n-gram), not a
 * substring or a file, so one cited entry cannot silently clear unrelated code.
 */
export function applyAllowlist(shared, allowlist) {
  const cited = allowlist.map(requireCitation);
  const allowed = [];
  const remaining = [];
  for (const hit of shared) {
    const entry = cited.find((a) => a.run === hit.run);
    if (entry) allowed.push({ ...hit, citation: entry.citation });
    else remaining.push(hit);
  }
  return { allowed, remaining };
}
