// SPDX-License-Identifier: Apache-2.0
// V-14 (app req/98, ranked #1): no gate anywhere in this tree checked template
// conformance against the five principles (~/.claude/skills/glovrex-app-scaffold/
// INHERITED_PRINCIPLES.md §3c) a module is supposed to hold at all times -- template
// form is emergent style, not a checked contract. Every other violation in req/98 was
// a drift this gate could have caught. This is the gate.
//
// Checks the four machine-checkable slices per module:
//   (a) bench declaration present        -- principle 2 (lightweight+fast, runtime-measured)
//   (b) SPDX + English                   -- principle 1 (template shape) + principle 3 (English)
//   (c) generated-file rebuildable header -- principle 5 (store=source, derived declared)
//   (d) CRUD/ACTS declaration present    -- principle 4 (always-CRUD, declared even when N/A)
//
// What this gate does NOT check: design quality, whether a bench number is *good*,
// whether a CRUD declaration is *correct* -- only whether the declaration exists and is
// readable. Passing this gate is the floor req/98's own audit describes: "declaration
// and measurement present", not "the module is finished".
//
//   node tools/conformance.mjs [--root <path>] [--json]
//
// Exit 0 when every check holds for every module. Exit 1 otherwise, listing every miss.
// --root lets tools/conformance_negative.mjs point this file at a scratch copy of the
// tree without duplicating the check logic (the same reuse shape shell/tools/negative.mjs
// gets by importing tools/gates.mjs's runGates rather than re-implementing it).

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import { buildManifest } from './rig/manifest.mjs';

const HERE = path.dirname(url.fileURLToPath(import.meta.url));
const DEFAULT_ROOT = path.resolve(HERE, '..');

const CJK = /[぀-ヿ㐀-鿿]/;

function stripComments(text) {
  return text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');
}

/** SPDX in the opening lines, and no CJK outside comments (principle 1 header shape +
 * principle 3 English), for a given file population. `exemptions` names files known to
 * carry CJK for a stated reason (data the code parses, not prose it prints) so the
 * population stays honest -- exempted files are reported, never silently dropped. */
function checkSpdxEnglish(manifest, files, exemptions = []) {
  const misses = [];
  const exempt = [];
  for (const f of files) {
    const entry = manifest.at(f);
    if (!entry) { misses.push(`${f}: not found in tree`); continue; }
    const known = exemptions.find((e) => e.file === f);
    if (!entry.text.slice(0, 400).includes('SPDX-License-Identifier')) misses.push(`${f}: no SPDX in opening lines`);
    const cjkHit = CJK.test(stripComments(entry.text));
    if (cjkHit) {
      if (known) exempt.push(`${f}: CJK outside comments, exempted -- ${known.reason}`);
      else misses.push(`${f}: CJK outside comments (not exempted)`);
    }
  }
  return { misses, exempt };
}

/** A module's own bench declaration: `.bench/report.json` (or, for tools/ itself,
 * `.run/report.json`) parses and carries a numeric median and budget. Does not require
 * `ok:true` -- a bench that ran and is over budget is still a bench declaration; an
 * absent or unparsable one is not. */
function checkBench(root, relativeReport) {
  const at = path.join(root, relativeReport);
  if (!fs.existsSync(at)) return { misses: [`${relativeReport}: absent -- run the module's bench script`] };
  let parsed;
  try { parsed = JSON.parse(fs.readFileSync(at, 'utf8')); } catch (error) { return { misses: [`${relativeReport}: does not parse -- ${error.message}`] }; }
  const median = parsed.medianMs ?? parsed.timings?.total;
  const budget = parsed.budgetMs ?? parsed.budgets?.total;
  const misses = [];
  if (typeof median !== 'number') misses.push(`${relativeReport}: no numeric median (medianMs or timings.total)`);
  if (typeof budget !== 'number') misses.push(`${relativeReport}: no numeric budget (budgetMs or budgets.total)`);
  return { misses, medianMs: median, budgetMs: budget };
}

/** A generated artifact's rebuildable-header declaration. Self-declaring files (`.mjs`
 * banners) are checked in place; files that cannot carry a comment (JSON, or a
 * hand-maintained non-generated file declared as such on purpose) are checked against
 * the nearest README instead -- both are "declared", neither is silently accepted. */
function checkGenerated(manifest, entries) {
  const misses = [];
  for (const entry of entries) {
    const target = manifest.at(entry.selfDeclaring ?? entry.declaredIn);
    if (!target) { misses.push(`${entry.file}: declaration file ${entry.selfDeclaring ?? entry.declaredIn} not found`); continue; }
    const hit = entry.markers.some((m) => target.text.includes(m));
    if (!hit) misses.push(`${entry.file}: none of [${entry.markers.join(', ')}] found in ${entry.selfDeclaring ?? entry.declaredIn}`);
  }
  return { misses };
}

/** A CRUD/ACTS position, present somewhere machine-checkable: either an exported ACTS
 * table, or a stated N-A declaration in the module's reqdef. Both count -- the
 * principle asks that the position be declared, not that every module offer acts. */
function checkCrud(manifest, entries) {
  const misses = [];
  for (const entry of entries) {
    const target = manifest.at(entry.file);
    if (!target) { misses.push(`${entry.file}: not found`); continue; }
    const hit = entry.markers.some((m) => target.text.includes(m));
    if (!hit) misses.push(`${entry.file}: none of [${entry.markers.join(', ')}] found`);
  }
  return { misses };
}

// ---------------------------------------------------------------- module descriptors

function buildModules(manifest) {
  return [
    {
      name: 'membrane',
      spdxEnglish: {
        files: manifest.under('membrane/src/').concat(manifest.under('membrane/tools/')).filter((f) => f.path.endsWith('.mjs')).map((f) => f.path),
        exemptions: [],
      },
      bench: 'membrane/.bench/report.json',
      generated: [
        { file: 'membrane/route-table.json', declaredIn: 'membrane/README.md', markers: ['route_table_from_crate'] },
        { file: 'membrane/coverage.json', declaredIn: 'membrane/README.md', markers: ['hand-maintained'] },
        { file: 'membrane/wire-fields.json', declaredIn: 'membrane/README.md', markers: ['hand-maintained'] },
      ],
      crud: [
        { file: 'req/01_MEMBRANE.md', markers: ['後から粒度を増やせる'] },
      ],
    },
    {
      name: 'shell',
      spdxEnglish: {
        files: manifest.under('shell/kernel/').filter((f) => f.path.endsWith('.mjs')).map((f) => f.path)
          .concat(manifest.under('shell/tools/').filter((f) => f.path.endsWith('.mjs')).map((f) => f.path)),
        exemptions: [],
      },
      bench: 'shell/.bench/report.json',
      generated: [
        { file: 'shell/demo/manifest.gen.mjs', selfDeclaring: 'shell/demo/manifest.gen.mjs', markers: ['Generated by'] },
        { file: 'shell/demo/modules.gen.mjs', selfDeclaring: 'shell/demo/modules.gen.mjs', markers: ['Generated by'] },
        { file: 'shell/demo/routes.gen.mjs', selfDeclaring: 'shell/demo/routes.gen.mjs', markers: ['Generated by'] },
      ],
      crud: [
        { file: 'req/02_SHELL_WLAYER.md', markers: ['slots'] },
        { file: 'req/02_SHELL_WLAYER.md', markers: ['marks'] },
      ],
    },
    {
      name: 'parts',
      spdxEnglish: {
        files: manifest.under('parts/src/').filter((f) => f.path.endsWith('.mjs')).map((f) => f.path)
          .concat(manifest.under('parts/generated/').filter((f) => f.path.endsWith('.mjs')).map((f) => f.path)),
        exemptions: [],
      },
      bench: 'parts/.bench/report.json',
      generated: [
        { file: 'parts/generated/tokens.generated.mjs', selfDeclaring: 'parts/generated/tokens.generated.mjs', markers: ['AUTO-GENERATED'] },
        { file: 'parts/fixtures/*.html', declaredIn: 'parts/README.md', markers: ['tools/fixtures.mjs'] },
        { file: 'parts/fixtures/shots/*.png', declaredIn: 'parts/README.md', markers: ['tools/shoot.mjs'] },
      ],
      crud: [
        { file: 'req/04_PARTS_REBUILD.md', markers: ['CRUDの対象でない'] },
      ],
    },
    {
      name: 'faces/ledger',
      spdxEnglish: {
        files: ['faces/ledger/ledger.mjs', 'faces/ledger/declaration.mjs', 'faces/ledger/binding.mjs', 'faces/ledger/index.mjs'],
        exemptions: [],
      },
      bench: 'faces/ledger/.bench/report.json',
      generated: [
        { file: 'faces/ledger/record/*.png', declaredIn: 'faces/ledger/README.md', markers: ['CopyFromScreen'] },
      ],
      crud: [
        { file: 'faces/ledger/declaration.mjs', markers: ['export const ACTS'] },
      ],
    },
    {
      name: 'faces/notice',
      spdxEnglish: {
        files: ['faces/notice/notice.mjs', 'faces/notice/declaration.mjs', 'faces/notice/binding.mjs', 'faces/notice/index.mjs'],
        exemptions: [],
      },
      bench: 'faces/notice/.bench/report.json',
      generated: [
        { file: 'faces/notice/record/*.png', declaredIn: 'faces/notice/README.md', markers: ['CopyFromScreen'] },
      ],
      crud: [
        { file: 'faces/notice/declaration.mjs', markers: ['export const ACTS'] },
      ],
    },
    {
      name: 'tools',
      spdxEnglish: {
        files: manifest.under('tools/').filter((f) => f.path.endsWith('.mjs') && !f.path.startsWith('tools/fixtures/')).map((f) => f.path),
        // ledger_dash.mjs parses Japanese-language req/*.md ledgers (POINTER_RE matches
        // the literal word "一次" as it appears in those source docs); its state labels
        // are also Japanese. req/98 V-11 records this as an open violation, not a
        // silently-accepted one -- exempted here with the same reasoning, not swept
        // under the check.
        exemptions: [
          { file: 'tools/ledger_dash.mjs', reason: "app req/98 V-11 (open): parses Japanese-language req/*.md ledger tables and mirrors the source documents' own vocabulary in its state labels and pointer-detection regex; a mechanical translation would desynchronise the parser from the documents it reads. Left open, not fixed, per this gate's own report." },
          { file: 'tools/ledger_dash.test.mjs', reason: 'fixtures for the file above; same reasoning.' },
          { file: 'tools/conformance.mjs', reason: 'self-referential: the CJK detection range itself is written as a Unicode range literal (data, not prose), and the generated/CRUD marker strings for req/01, req/02 and req/04 are search patterns matching this Japanese-language req corpus verbatim -- the same non-shipped-output justification as ledger_dash.mjs above.' },
          { file: 'tools/ledger_flip.mjs', reason: "SS548/SS558 flip mechanization: this file detects and protects the retired ledger state, whose canonical token IS the literal text that appears verbatim in the req/*.md ledgers it writes to (STATES.RETIRED_DONE.token in ledger_dash.mjs); translating that one comparison string would desynchronise the flip tool's retired-row guard from the real files it operates on -- same non-shipped-output justification as ledger_dash.mjs above. All prose/comments/user-facing messages in this file are English." },
          { file: 'tools/ledger_flip.test.mjs', reason: "fixture for the retired-row guard above; same reasoning -- the negative-control fixture must use the real retired token to prove the guard fires on it." },
        ],
      },
      bench: '.run/report.json',
      generated: [
        { file: 'docs/ABSORPTION_STATUS.html', selfDeclaring: 'docs/ABSORPTION_STATUS.html', markers: ['sha256'] },
      ],
      // CRUD signal for tools/ is structural (baseline.mjs's export shape), not a text
      // marker -- computed in runConformance() below rather than declared here.
      crud: [],
    },
  ];
}

export function runConformance(root = DEFAULT_ROOT) {
  const manifest = buildManifest(root);
  const modules = buildModules(manifest);
  const results = [];

  for (const mod of modules) {
    const spdxEnglish = checkSpdxEnglish(manifest, mod.spdxEnglish.files, mod.spdxEnglish.exemptions);
    const bench = checkBench(root, mod.bench);
    const generated = checkGenerated(manifest, mod.generated);
    // tools/ CRUD signal is the ledger command surface baseline.mjs is the working
    // template for (req/98 V-8): a CLI with list/commit/retire branches, not an
    // exported library function -- checked against the shape the file actually has.
    let crud;
    if (mod.name === 'tools') {
      const baselineEntry = manifest.at('tools/baseline.mjs');
      const hasAll = baselineEntry && ["action === 'list'", "action === 'commit'", "action === 'retire'"].every((m) => baselineEntry.text.includes(m));
      crud = { misses: hasAll ? [] : ['tools/baseline.mjs: no list/commit/retire command branches -- no CRUD command surface for the ledger it governs'] };
    } else {
      crud = checkCrud(manifest, mod.crud);
    }

    const checks = {
      'bench declaration present': bench.misses,
      'SPDX + English': [...spdxEnglish.misses],
      'generated-file rebuildable header': generated.misses,
      'CRUD/ACTS declaration present': crud.misses,
    };
    const ok = Object.values(checks).every((m) => m.length === 0);
    results.push({
      module: mod.name, ok, checks, exempt: spdxEnglish.exempt, benchFigures: { medianMs: bench.medianMs, budgetMs: bench.budgetMs },
    });
  }

  const held = results.filter((r) => r.ok).length;
  return { results, held, total: results.length };
}

if (process.argv[1] && process.argv[1].endsWith('conformance.mjs')) {
  const rootArgIndex = process.argv.indexOf('--root');
  const root = rootArgIndex >= 0 ? path.resolve(process.argv[rootArgIndex + 1]) : DEFAULT_ROOT;
  const outcome = runConformance(root);
  if (process.argv.includes('--json')) {
    process.stdout.write(`${JSON.stringify(outcome, null, 2)}\n`);
  } else {
    for (const r of outcome.results) {
      process.stdout.write(`${r.ok ? 'held ' : 'FELL '} ${r.module}\n`);
      for (const [name, misses] of Object.entries(r.checks)) {
        if (misses.length === 0) { process.stdout.write(`        ok    ${name}\n`); continue; }
        process.stdout.write(`        MISS  ${name}\n`);
        for (const m of misses) process.stdout.write(`              - ${m}\n`);
      }
      for (const e of r.exempt) process.stdout.write(`        exempt  ${e}\n`);
      if (r.benchFigures.medianMs !== undefined) process.stdout.write(`        bench   median=${Number(r.benchFigures.medianMs).toFixed(2)}ms  budget=${r.benchFigures.budgetMs}ms\n`);
    }
    process.stdout.write(`\n${outcome.held}/${outcome.total} modules conform\n`);
  }
  process.exit(outcome.held === outcome.total ? 0 : 1);
}
