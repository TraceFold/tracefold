// SPDX-License-Identifier: Apache-2.0
// The gates that watch this module's own shape.
//
// Every one of them is fired red at least once below. A gate that has never refused
// anything is a gate nobody has evidence works.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { writeFileSync, readFileSync, mkdtempSync, readdirSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname, basename, delimiter, relative, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  sourceFiles, pathLiteralOffenders, wordOffenders, networkOffenders, sharedRuns, tokenizeNames,
  JUDGEMENT_WORDS, FOREIGN_IDENTIFIERS, walkReferenceTree, applyAllowlist,
  BACKEND_WITHHELD_FIELDS, backendAttestationOffenders,
} from '../tools/discipline.mjs';
import { TABLE } from '../src/membrane.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const MEMBRANE = join(HERE, '..');
const APP = join(MEMBRANE, '..');

// D5 compares this module against a REFERENCE CORPUS: the older asset trees whose code
// new code here must not have inherited from. Which trees those are is local
// configuration and is deliberately not written down in this file.
//
// Until req/883 the three roots were absolute paths spelled out right here. That is two
// faults in one line. It leaks -- the paths disclose one machine's directory layout and,
// worse, the *names* disclose a roster of unpublished projects to anyone who reads a
// published clone. And it is wrong everywhere but that one machine, so the gate silently
// measured nothing for every other reader. Configure it instead, by either:
//
//   GX_REFERENCE_ROOTS=/path/one<delim>/path/two    (delim = ; on Windows, : elsewhere)
//   membrane/test/reference-roots.local.json        ["/path/one", "/path/two"]
//
// The JSON file is gitignored. Labels are derived from the basename of whatever is
// configured, because a label was only ever report text -- it never selected a path or
// decided an assertion, so nothing is lost by not naming anything here.
const LOCAL_ROOTS_AT = join(HERE, 'reference-roots.local.json');

function referenceRoots() {
  const fromEnv = (process.env.GX_REFERENCE_ROOTS ?? '').split(delimiter).map((s) => s.trim()).filter(Boolean);
  const configured = fromEnv.length > 0
    ? fromEnv
    : (existsSync(LOCAL_ROOTS_AT) ? JSON.parse(readFileSync(LOCAL_ROOTS_AT, 'utf8')) : []);
  return configured.map((path) => ({ label: basename(path.replace(/[/\\]+$/, '')) || path, path }));
}

const REFERENCE_ROOTS = referenceRoots();
const REFERENCE = walkReferenceTree(REFERENCE_ROOTS);

// A corpus that is not there is not a corpus that agrees with us. Every D5 assertion
// below is of the form "no shared run was found", and an empty corpus satisfies all of
// them for the one reason that must never be read as agreement: nothing was compared.
// So the reason is computed once, printed once, and handed to node:test as a skip --
// which prints the tests as skipped rather than as 3 more passes on the tally.
const CORPUS_ABSENT = REFERENCE.files.length === 0
  ? 'D5 UNMEASURED: no reference corpus is configured or reachable, so the copy-lineage gate compared this tree against nothing. '
    + 'Set GX_REFERENCE_ROOTS, or write membrane/test/reference-roots.local.json. '
    + 'This is expected in a published clone: the corpus is local and does not ship.'
  : false;
if (CORPUS_ABSENT) console.log(`      ${CORPUS_ABSENT}`);

/**
 * Standards-derived constants both sides are expected to carry because both
 * implement the same public specification, not because one copied the other. Each
 * `run` is the exact 8-name n-gram tokenizeNames produces from the real files --
 * verified against shell/kernel/digest.mjs and the corresponding hash implementation
 * in the reference corpus before being written here, not guessed. An entry with no
 * live hit in a given run of this suite is not dead weight: it stays cited and ready.
 *
 * (Reference-corpus files are referred to by their role, never by path. The corpus is
 * local configuration -- see referenceRoots() above -- and naming its internals here
 * would put back the roster disclosure req/883 took out.)
 *
 * The WCAG relative-luminance/contrast-ratio coefficients (0.2126/0.7152/0.0722/
 * 0.03928/12.92/0.055/1.055/2.4, shared with a vendored terminal emulator in the
 * corpus) are deliberately NOT listed here: they are decimal-literal tokens, and
 * tokenizeNames's TOKEN grammar (`\d+(?:\.\d+)?`) keeps every numeric literal out of
 * the name stream entirely -- there is no 8-name run for a run of numbers to form,
 * so D5's 'names' assertion is structurally incapable of ever flagging it (confirmed
 * empirically: shared 8-grams between parts/src/tokens.mjs and that file, over the
 * name stream, is the empty set). This is a property of what the gate measures
 * (identifiers, not literals), not a coverage gap left open by this widening.
 */
const STANDARDS_ALLOWLIST = [
  {
    run: 'x6a09e667 xbb67ae85 x3c6ef372 xa54ff53a x510e527f x9b05688c x1f83d9ab x5be0cd19',
    citation: 'FIPS 180-4 §5.3.3, SHA-256 initial hash value H(0): eight public 32-bit words every conformant SHA-256 implementation carries verbatim (BLAKE3 reuses the same IV) -- not an expression copied from the hash implementation in the reference corpus.',
  },
];

const SRC = sourceFiles(join(MEMBRANE, 'src'));
const ALL = [...SRC, ...sourceFiles(join(MEMBRANE, 'test')), ...sourceFiles(join(MEMBRANE, 'tools'))];

/** True for a test file or anything under a test/ directory. D5 has always scanned
 * SRC (membrane/src), never membrane/test -- test files reuse the same assertion
 * idiom (`assert.equal(fnOf(...), ...)`) call after call, and that repetition is
 * itself an 8-name run, so a test file shares runs with any other test file that
 * exercises a same-named function this way regardless of whether either copied the
 * other (parts/test/seal-claim.test.mjs vs ui_proto/ui/faces/receipt/index.test.mjs
 * is exactly this: req/92 already reviewed that pair -- same Z-1 count, 7 -- and
 * judged it "own-lineage naming, no code-body copy", not a fresh finding). Widening
 * D5's own-code scan app-wide keeps that same narrowing, so the widening surfaces
 * genuinely new ground (shell/parts/faces implementation) without re-flagging a case
 * already reviewed. */
const isTestPath = (path) => /[\\/]test[\\/]/.test(path) || /\.test\.mjs$/.test(path);

/** The whole app's implementation, not just this one module: D5 asks whether
 * *anything* new here shares a run with the reference tree, and "anything" silently
 * meant "membrane/src only" before this widening. Mirrors D4's existing
 * shell/faces/parts sweep below, plus the app-level tools/ the independent
 * collation (req/92) also covered. D3 is deliberately left scanning `ALL`
 * (membrane only, unchanged): its FOREIGN_IDENTIFIERS list is a membrane-specific
 * boundary contract ("no identifier of the reference tree in THIS module" --
 * membrane, the wire-address layer), not a general app-wide copy check, and
 * widening it to faces/ledger flagged UNDRAWN as a false regression against code
 * that -- per req/92's own precedent for sibling identifiers in that same module --
 * is continuing ui_proto's ledger-domain vocabulary by design, not inheriting it by
 * accident. */
const APP_SOURCE_D5 = [
  ...ALL,
  ...sourceFiles(join(APP, 'shell')),
  ...sourceFiles(join(APP, 'parts')),
  ...sourceFiles(join(APP, 'faces')),
  ...sourceFiles(join(APP, 'tools')),
].filter((f) => !isTestPath(f));

const PATHS = TABLE.routes.map((r) => r.path);

function scratch(contents) {
  const dir = mkdtempSync(join(tmpdir(), 'membrane-gate-'));
  const file = join(dir, 'offender.mjs');
  writeFileSync(file, contents);
  return [file];
}

// --- D1: the address is assembled in one module -----------------------------

/**
 * `tables.gen.mjs` is exempt, and the exemption is narrower than it looks.
 *
 * What this gate forbids is a module SPELLING a path -- a hand-typed `/candidates` that
 * can disagree with the table and be wrong on its own. The generated module is the table:
 * it is `route-table.json` in the one form a browser can import, written by
 * `tools/gen_tables.mjs`, and `test/tables_gen.test.mjs` G1/G2 fail if a single character
 * of it differs from the JSON. It cannot drift, because nothing hand-writes it.
 *
 * The JSON has always been exempt for exactly this reason -- this gate reads `src/` and
 * the table lives a directory up. Binding the membrane into a window moved the table's
 * MODULE form into `src/`, and the exemption follows the reason rather than the folder.
 * D1-neg below still proves the gate catches a module that builds a path itself.
 */
const GENERATED_TABLE = 'tables.gen.mjs';

test('D1 no module but address.mjs spells a wire path (AC-M3)', () => {
  const offenders = pathLiteralOffenders(SRC, PATHS, ['address.mjs', GENERATED_TABLE]);
  assert.deepEqual(offenders, [], `path literals outside address.mjs: ${JSON.stringify(offenders)}`);
});

test('D1 the exempt generated table is generated, and is the JSON it claims to be', () => {
  // The exemption above is only honest while this holds, so it is asserted beside it
  // rather than left to another file nobody reads at the same time.
  const generated = readFileSync(join(MEMBRANE, 'src', GENERATED_TABLE), 'utf8');
  assert.match(generated, /Generated by tools\/gen_tables\.mjs/, 'the exempt file no longer says it is generated');
  const held = JSON.parse(readFileSync(join(MEMBRANE, 'route-table.json'), 'utf8'));
  for (const path of held.routes.map((r) => r.path)) {
    assert.ok(generated.includes(`"${path}"`), `${path} is in the table and not in the module generated from it`);
  }
});

test('D1-neg the same gate refuses a module that builds a path itself', () => {
  const offenders = pathLiteralOffenders(scratch('const u = `${o}/v1/candidates/${id}`;\n'), PATHS, ['address.mjs']);
  assert.equal(offenders.length, 1);
});

// --- D2: the membrane states no verdict of its own --------------------------

test('D2 no judgement of validity is computed here (AC-M10)', () => {
  assert.deepEqual(wordOffenders(SRC, JUDGEMENT_WORDS), []);
});

test('D2-neg the same gate refuses a module that grades a receipt', () => {
  const offenders = wordOffenders(scratch('export function verifyReceipt(r) { return true; }\n'), JUDGEMENT_WORDS);
  assert.equal(offenders.length, 1);
});

// --- D3: no identifier is inherited from the reference tree (Z-2) -----------

test('D3 no identifier of the reference tree appears in this module', () => {
  // The two files that have to name the forbidden words in order to forbid them are
  // the list and the test that fires it; everything else is scanned.
  const scanned = ALL.filter((f) => !f.endsWith('discipline.mjs') && !f.endsWith('discipline.test.mjs'));
  console.log(`      D3 scanned ${scanned.length} of ${ALL.length} files (2 excluded: the list and this file)`);
  assert.deepEqual(wordOffenders(scanned, FOREIGN_IDENTIFIERS), []);
});

test('D3-neg the same gate refuses an inherited identifier', () => {
  const offenders = wordOffenders(scratch('const NO_SURFACE = {};\n'), FOREIGN_IDENTIFIERS);
  assert.equal(offenders.length, 1);
});

// --- D4: nothing above the membrane touches a network (AC-M0) ---------------
//
// Widened 2026-08-31 (glovrex_app req/104 boundary ledger B2/B6): the population used
// to be shell/faces/parts only, on the strength of a comment calling monitor/terminal/
// wire/demo/tools "never in a shipped browser bundle" without anywhere a reader could
// check that claim. It wasn't checked -- req/104's census found a direct-fetch near-miss
// in terminal/wire (B2) and a second, self-declared network surface in monitor/ that
// this gate's old population never saw (B6). Below is every file the widened population
// (this census, run against the same networkOffenders() D4 always used) actually finds
// with a network word in it, named with why it is not an offender. An undeclared new
// `fetch(`/`WebSocket`/... anywhere in the widened population is still red -- this is a
// list, not a directory-shaped exemption, so a ninth file added tomorrow gets no pass it
// was not given by name.
const D4_DECLARED = [
  // (a) fetchImpl injection (Owner-named shape): the function is defined here and handed
  // to createMembrane({ fetchImpl }); the one call site is membrane/src/transport.mjs.
  // Verified by reading the call site, not assumed (req/104 §1②).
  { file: 'terminal/tui.mjs', reason: 'fetchImpl injected into createMembrane(); call site is transport.mjs (req/104 §1②)' },
  { file: 'terminal/check.mjs', reason: 'same fetchImpl-injection shape as terminal/tui.mjs' },
  { file: 'wire/probe.mjs', reason: 'fetchImpl injected into createMembrane(); same shape as terminal/tui.mjs (req/104 §1②)' },
  // (b) the monitor's self-declared surface (Owner-named shape, monitor/serve.mjs; the
  // other three are the same surface by the same header claim and the same gate):
  // "a monitor that could change the thing it watches is not a monitor" -- GET-only,
  // never imports membrane/src, gated by monitor/check.mjs's own "the monitor writes
  // nothing" regex (req/104 §1⑥/B6).
  { file: 'monitor/serve.mjs', reason: "monitor's own GET-only engine mirror; gated by monitor/check.mjs (req/104 §1⑥)" },
  { file: 'monitor/shoot.mjs', reason: 'monitor family: GET-only liveness probe before a screenshot, same surface as serve.mjs' },
  { file: 'monitor/check.mjs', reason: "monitor's own live() health probe, and the file that implements the monitor's self-gate" },
  { file: 'monitor/face.mjs', reason: 'dev-only live-reload <script> text emitted for a browser tab (EventSource sits inside a template-literal string, not a Node call); off by ?live=0' },
  // found by this widening, outside the two Owner-named shapes: dev/CLI tooling that
  // self-declares a narrow network touch and ships nothing to a browser bundle.
  { file: 'demo/serve.mjs', reason: "local static file server only (node:http for inbound serving); header states 'issues no request of its own'" },
  { file: 'demo/check.mjs', reason: "the demo's own gate: 'fetch(' appears only inside the regex asserting the rendered page calls out to nothing -- same self-naming exemption D2/D3 already give discipline.mjs/discipline.test.mjs" },
  { file: 'tools/verify-all.mjs', reason: 'top-level verify orchestrator: one GET to /v1/healthz to ask whether a real engine is reachable before grading against it' },
  { file: 'tools/rig/wire_ws.mjs', reason: 'dev rig driving the pixel tier over a raw WebSocket for testing; not shipped app code' },
];
const D4_DIRS = ['shell', 'faces', 'parts', 'monitor', 'terminal', 'wire', 'demo', 'tools'];
const relPosix = (f) => relative(APP, f).split(sep).join('/');

/** shell/faces/parts keep their pre-widening rule (their own nested tools/ -- self-audit
 * gates, dev-only local static servers -- is out of population); that rule would also eat
 * the newly-added top-level tools/ itself if applied unconditionally, so it is scoped to
 * the three directories it was written for. */
function d4Population(dirs) {
  return dirs
    .flatMap((dir) => sourceFiles(join(APP, dir)).map((f) => ({ f, dir })))
    .filter(({ f, dir }) => !isTestPath(f) && (dir === 'tools' || !/[\\/]tools[\\/]/.test(f)))
    .map(({ f }) => f);
}

test('D4 shell/faces/parts/monitor/terminal/wire/demo/tools reach no undeclared network (AC-M0)', () => {
  const above = d4Population(D4_DIRS);
  const declaredPaths = new Set(D4_DECLARED.map((d) => d.file));
  for (const path of declaredPaths) {
    assert.ok(above.some((f) => relPosix(f) === path), `D4_DECLARED names a file the widened population no longer contains: ${path}`);
  }
  const scanned = above.filter((f) => !declaredPaths.has(relPosix(f)));
  const offenders = networkOffenders(scanned);
  assert.deepEqual(offenders, [], `undeclared network touch above the membrane: ${JSON.stringify(offenders)}`);
  // Reported rather than implied, same as before the widening.
  console.log(`      D4 denominator: ${above.length} source files above the membrane`
    + ` (${declaredPaths.size} declared network-touch exceptions, ${scanned.length} scanned for undeclared network)`);
});

test('D4-neg the same gate refuses a face that calls fetch', () => {
  assert.equal(networkOffenders(scratch('const r = await fetch("/v1/candidates");\n')).length, 1);
});

test('D4-widen-neg a fetch planted inside the newly-widened tools/ population is caught, then removed', () => {
  // scratch() above proves networkOffenders() itself catches a fetch; it does not prove
  // the widened *population* actually walks the new directories, because scratch() writes
  // outside APP entirely (os.tmpdir()). This plants inside tools/ -- one of the five
  // directories this widening added -- so the assertion can only pass if d4Population()
  // truly reaches it, then removes the plant in a finally so the tree is unchanged after.
  const dir = mkdtempSync(join(APP, 'tools', 'd4-widen-negctl-'));
  const planted = join(dir, 'offender.mjs');
  try {
    writeFileSync(planted, 'const r = await fetch("/v1/candidates");\n');
    const above = d4Population(D4_DIRS);
    assert.ok(above.includes(planted), 'a file freshly written inside tools/ must appear in the widened D4 population');
    const declaredPaths = new Set(D4_DECLARED.map((d) => d.file));
    const offenders = networkOffenders(above.filter((f) => !declaredPaths.has(relPosix(f))));
    assert.ok(offenders.some((o) => o.file === planted), 'D4 must flag a planted fetch inside the newly-included tools/ directory');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// --- D5: the copy gate (Z-1), a floor and reported as one -------------------

test('D5 no run of 8 names is shared with the reference tree, uncited', { skip: CORPUS_ABSENT }, () => {
  // Scanned-vs-skipped per root, always -- a widened reference tree that silently
  // dropped a root or a file would be a gate whose "clean" cannot be trusted.
  for (const [label, { counts, present }] of Object.entries(REFERENCE.perRoot)) {
    if (!present) { console.log(`      D5 root ${label}: ABSENT`); continue; }
    console.log(`      D5 root ${label}: scanned ${counts.scanned}`
      + ` (skipped: ${counts.skippedExtension} non-source ext, ${counts.skippedSize} oversize,`
      + ` ${counts.skippedBinary} binary, ${counts.skippedHidden} hidden, ${counts.skippedUnreadable} unreadable,`
      + ` ${counts.excludedDirs} dirs excluded)`);
  }
  const shared = sharedRuns(APP_SOURCE_D5, REFERENCE.files, 8, 'names');
  const raw = sharedRuns(APP_SOURCE_D5, REFERENCE.files, 8, 'all');
  const { allowed, remaining } = applyAllowlist(shared, STANDARDS_ALLOWLIST);
  // Both numbers, always. The second is the wider measurement the gate was narrowed
  // away from, and every one of its hits is language grammar rather than mechanism.
  console.log(`      D5 ${APP_SOURCE_D5.length} files vs ${REFERENCE.files.length} reference files:`
    + ` ${shared.length} shared name-runs (${allowed.length} allowlisted with citation, ${remaining.length} uncited),`
    + ` ${raw.length} shared token-runs (grammar included)`);
  if (allowed.length) console.log(`      D5 allowlisted: ${JSON.stringify(allowed, null, 2)}`);
  assert.deepEqual(remaining, [], `uncited shared runs: ${JSON.stringify(remaining.slice(0, 5), null, 2)}`);
});

test('D5-neg the same gate catches a pasted run from each reference root', { skip: CORPUS_ABSENT }, () => {
  for (const [label, { files, present }] of Object.entries(REFERENCE.perRoot)) {
    if (!present || files.length === 0) { assert.fail(`reference root ${label} produced no files to prove the gate against`); }
    // Lifted at run time into a temporary file and never into this module: the gate
    // has to be shown refusing something, and the something must not be committed
    // here. One donor per root, so a root being unreachable cannot hide behind
    // another root's donor passing.
    const donor = files
      .map((file) => ({ file, names: tokenizeNames(readFileSync(file, 'utf8')) }))
      .filter((d) => d.names.length >= 120)
      .sort((a, b) => b.names.length - a.names.length)[0];
    assert.ok(donor, `reference root ${label} has no file with 120+ names to donate a run from`);
    const run = donor.names.slice(100, 120).join(' ');
    const pasted = sharedRuns(scratch(`// donor: ${label}\n${run}\n`), REFERENCE.files, 8, 'names');
    assert.ok(pasted.length > 0, `a run of names taken from reference root ${label} (${donor.file}) must be caught`);
  }
});

// --- D5's allowlist: exact, cited, fail-closed -------------------------------

test('D5 allowlist explains a cited standards run and leaves everything else caught', { skip: CORPUS_ABSENT }, () => {
  // A generic proof of the mechanism, independent of which real standard is cited
  // today: donate one run from the widened reference tree, allowlist exactly that
  // run with a citation, and confirm it moves from "remaining" to "allowed" while
  // an unrelated pasted run is still caught.
  const donor = REFERENCE.files
    .map((file) => ({ file, names: tokenizeNames(readFileSync(file, 'utf8')) }))
    .filter((d) => d.names.length >= 140)
    .sort((a, b) => b.names.length - a.names.length)[0];
  const citedRun = donor.names.slice(10, 18).join(' ');
  const uncitedRun = donor.names.slice(30, 38).join(' ');
  const pasted = scratch(`// donor: cited\n${citedRun}\n// donor: uncited\n${uncitedRun}\n`);
  const shared = sharedRuns(pasted, REFERENCE.files, 8, 'names');
  const allowlist = [{ run: citedRun, citation: 'test fixture citation: standing in for a real specification reference' }];
  const { allowed, remaining } = applyAllowlist(shared, allowlist);
  assert.ok(allowed.some((a) => a.run === citedRun), 'the cited run must be marked allowed');
  assert.ok(remaining.some((r) => r.run === uncitedRun), 'the uncited run must still be reported');
  assert.equal(allowed.every((a) => a.run !== uncitedRun), true, 'the uncited run must not slip into allowed');
});

test('D5-neg allowlist refuses an entry with no citation (fail-closed)', () => {
  assert.throws(() => applyAllowlist([{ file: 'x', run: 'a b c d e f g h' }], [{ run: 'a b c d e f g h' }]));
  assert.throws(() => applyAllowlist([{ file: 'x', run: 'a b c d e f g h' }], [{ run: 'a b c d e f g h', citation: '' }]));
  assert.throws(() => applyAllowlist([{ file: 'x', run: 'a b c d e f g h' }], [{ run: 'a b c d e f g h', citation: 'short' }]));
});

// --- D6: the engine's two withheld fields never wear the face of an attested read --
//
// req/38_ERRATA_2026-08-07.md §497(b) (glovrex_app/req/08 §6-2 8-A's `dr44_9_views.rs`
// citation): `alg` and `verified` are fields the engine deliberately never returns
// beside a signature (dr44_9_views.rs:233's two refusals). This face/wire-fields scan
// is the machine gate that ruling asked for.

async function loadFaceDeclarations() {
  const facesDir = join(APP, 'faces');
  if (!existsSync(facesDir)) return [];
  const out = [];
  for (const entry of readdirSync(facesDir)) {
    const declPath = join(facesDir, entry, 'declaration.mjs');
    if (!existsSync(declPath)) continue;
    const mod = await import(pathToFileURL(declPath).href);
    out.push(mod.DECLARATION);
  }
  return out;
}

function loadWireFields() {
  return JSON.parse(readFileSync(join(MEMBRANE, 'wire-fields.json'), 'utf8'));
}

test('D6 no face declares alg/verified as a field it draws from the backend (req/38 §497(b))', async () => {
  const declarations = await loadFaceDeclarations();
  assert.ok(declarations.length >= 1, 'at least one real face must be scanned, or this gate is proving nothing');
  const offenders = backendAttestationOffenders(declarations, loadWireFields());
  assert.deepEqual(offenders, []);
});

test('D6-neg the same gate catches a face that names alg as an engine-sourced mark', () => {
  const rigged = [{
    id: 'rigged-face',
    marks: [{ mark: 'receipt/alg', means: 'receipt.alg', from: 'the engine returned the signature algorithm' }],
  }];
  const offenders = backendAttestationOffenders(rigged, { fields: [] });
  assert.equal(offenders.length, 1);
  assert.equal(offenders[0].face, 'rigged-face');
  assert.deepEqual(offenders[0].hits, ['alg']);
});

test('D6-neg the same gate catches a wire-fields.json entry naming verified', () => {
  const offenders = backendAttestationOffenders([], { fields: [{ route: 'get_x', field: 'verified', source: 'made-up.rs:1' }] });
  assert.equal(offenders.length, 1);
  assert.deepEqual(offenders[0].hits, ['verified']);
});

test('D6-neg a mark that only shares a substring (e.g. `algorithm-note`) is not an offender', () => {
  const rigged = [{ id: 'x', marks: [{ mark: 'note/algorithm-note', means: 'note.algorithm_note' }] }];
  assert.deepEqual(backendAttestationOffenders(rigged, { fields: [] }), []);
});

test('D6 the withheld-field list is exactly the two dr44_9_views.rs:233 names', () => {
  assert.deepEqual(BACKEND_WITHHELD_FIELDS, ['alg', 'verified']);
});
