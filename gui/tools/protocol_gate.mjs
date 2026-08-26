// SPDX-License-Identifier: Apache-2.0
// req/884 (Owner #423/#424, seat=Opus, 2026-08-26, 暫定 -- 再審査可)
//
// THE CONSUMPTION CHECK.
//
// One source of record declares the design protocol (tokens/tokens.css). This walks
// every file that DRAWS and fails if any of them states an interaction value of its own
// instead of reading a token: a colour, a radius, a spacing step, a duration or easing,
// a cursor, a focus ring, a disabled or idle opacity, a press transform, a shadow, or a
// scrollbar treatment.
//
// Why this exists rather than a review convention: the tree already had two token
// rosters and did not know it. shell/kernel/shell.css:26-30 declared --surface-0/1/2,
// --crease and --tension on `.shell`, under a file header that says in its first line
// "Not one colour is written here" -- and that header was TRUE about hex literals while
// being false about the thing that mattered, because a color-mix() of two tokens under a
// new name is a new value that only one file can spell. That is precisely why the shell
// chrome and the face interiors disagreed about what a hover looks like. No reviewer
// caught it across seven req cycles. A machine does.
//
// Red-first (req/884 section 5): this gate was run against the tree BEFORE the repairs
// landed and reported its offenders; the count is recorded in the ledger. A gate whose
// first run is green is a gate for which nobody has evidence (parts/tools/boundary.mjs
// states the same rule and is the model this follows).
//
//   node tools/protocol_gate.mjs              # report, exit 1 if any offender
//   node tools/protocol_gate.mjs --json       # machine-readable
//   node tools/protocol_gate.mjs --index      # write tokens/protocol.index.json
//
// The generated index is what req/885's landing pages consume so they can adopt the
// scrollbar and state treatments without parsing CSS or, worse, inventing a local fix.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const ROOT = path.resolve(HERE, '..');
export const TOKENS_CSS = path.join(ROOT, 'tokens', 'tokens.css');
export const INDEX_PATH = path.join(ROOT, 'tokens', 'protocol.index.json');

export const GATE_MESSAGES = {
  EMPTY_POPULATION: 'the rule was applied to nothing, which is not the same as the rule holding',
  HARDCODED: 'a drawing file states an interaction value the roster already owns',
  CLEAN: 'every drawing file reads the roster',
};

/**
 * Which token belongs to which family. Written out by PREFIX rather than inferred from
 * a value's shape, because a rule that reads its own taxonomy off a regex over values
 * stops holding the day somebody declares a duration that happens to parse as a length.
 * The families are the ones Owner #424 enumerated, plus the two #423追記1 added.
 */
export const FAMILIES = Object.freeze({
  colour: [
    '--bg', '--bg-raised', '--bg-inset', '--ink', '--ink-2', '--ink-3', '--line',
    '--deny', '--act', '--act-bed', '--admit', '--admit-bed', '--deny-bed',
    '--escalate', '--escalate-bed', '--held', '--held-bed',
    '--surface-0', '--surface-1', '--surface-2', '--crease', '--tension',
    '--alarm', '--alarm-bed', '--alarm-edge',
  ],
  radius: ['--radius-1', '--radius-2', '--radius-3'],
  spacing: ['--space-1', '--space-2', '--space-3', '--space-4', '--pad-x', '--spine-x', '--row'],
  type: [
    '--t-meta', '--t-time', '--t-record', '--t-head', '--t-stat', '--t-label',
    '--lh-meta', '--lh-time', '--lh-record', '--lh-head', '--lh-stat', '--lh-label',
    '--mono', '--sans', '--track-label',
  ],
  elevation: ['--shadow-menu', '--shadow-palette'],
  motion: ['--motion-quick', '--motion-settle', '--motion-ease'],
  interaction: [
    '--press-y', '--focus-w', '--focus-offset', '--focus-offset-in', '--focus-ink',
    '--disabled-opacity', '--idle-opacity',
    '--cursor-act', '--cursor-refuse', '--cursor-inert', '--cursor-text',
    '--cursor-size-x', '--cursor-size-y',
    '--drag-bar-scale', '--drag-ghost-opacity', '--drop-bed', '--drop-edge',
  ],
  scrollbar: [
    '--scrollbar-w', '--scrollbar-track', '--scrollbar-thumb',
    '--scrollbar-thumb-hover', '--scrollbar-radius',
  ],
  glyph: ['--glyph-1', '--glyph-2', '--glyph-stroke', '--glyph-opacity'],
});

/**
 * The files that DRAW. A gate over a population it chose by glob would silently shrink
 * the day a file moves, so the roots are named and the walk under them is explicit.
 * tokens/ is excluded on purpose: it is the source of record, and the one place a real
 * value is supposed to appear.
 */
export const DRAWING_ROOTS = Object.freeze([
  'shell/kernel',
  'shell/demo',
  'parts/src',
  'faces',
]);

const DRAWS = /\.(css|mjs)$/;
const SKIP_DIR = /(^|[\\/])(test|tests|node_modules|\.git|generated|fixtures|record|\.bench|\.run)([\\/]|$)/;
/**
 * Instruments reason ABOUT literals, so they are allowed to contain them. A check that
 * asserts "this element must not be rgba()" has to be able to write rgba(), and counting
 * that as the offence would make the gate forbid the gates.
 */
const SKIP_FILE = /(^|[\\/])(tools|probes)[\\/]|(^|[\\/])(checks|gate|negative|assays)\.mjs$/;

export function drawingFiles(root = ROOT) {
  const out = [];
  const walk = (dir) => {
    if (SKIP_DIR.test(dir)) return;
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const at = path.join(dir, entry.name);
      const rel = path.relative(root, at);
      if (entry.isDirectory()) { if (!SKIP_DIR.test(rel)) walk(at); continue; }
      if (!DRAWS.test(entry.name)) continue;
      if (SKIP_FILE.test(rel) || SKIP_DIR.test(rel)) continue;
      out.push({ rel: rel.split(path.sep).join('/'), path: at, text: fs.readFileSync(at, 'utf8') });
    }
  };
  for (const r of DRAWING_ROOTS) {
    const at = path.join(root, r);
    if (fs.existsSync(at)) walk(at);
  }
  return out.sort((a, b) => a.rel.localeCompare(b.rel));
}

/**
 * Prose blanked, line numbers kept -- the same device parts/tools/boundary.mjs uses and
 * for the same reason: a comment that NAMES the literal it replaced is the file telling
 * the truth, and counting it as the offence would punish the documentation this project
 * runs on. Blanking rather than deleting keeps the reported line openable.
 */
export function codeOf(text) {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, (b) => b.replace(/[^\n]/g, ' '))
    .split(/\n/)
    .map((l) => l.replace(/(^|[^:])\/\/.*$/, (_, lead) => lead))
    .join('\n');
}

/**
 * The rules. Each names the family it protects, so an offender can be told which token
 * to reach for instead of merely being told it is wrong -- a gate that reports a defect
 * without naming the cure gets worked around rather than obeyed.
 */
export const RULES = Object.freeze([
  {
    family: 'colour', name: 'colour-literal',
    // Two spellings, because the false-positive risk is not the same in the two file
    // kinds and one pattern for both would have to be wrong in one of them. In CSS a
    // three-digit hex is a real colour, so the net is wide. In JS it is far more often
    // an Owner reference number -- `#340`, `#387`, `#335` all parse as valid CSS hex,
    // and the first run of this gate reported twenty-odd of them as colour defects.
    // A gate that cries wolf gets worked around rather than obeyed, so in JS the net
    // takes six- and eight-digit hex only, which is how a real colour is written there.
    pattern: /#[0-9a-fA-F]{3,8}\b|\brgba?\s*\(|\bhsla?\s*\(/g,
    jsPattern: /#[0-9a-fA-F]{6}\b|#[0-9a-fA-F]{8}\b|\brgba?\s*\(|\bhsla?\s*\(/g,
    cure: 'a token from the roster (--ink, --bg, --act, --surface-*, ...)',
  },
  {
    family: 'motion', name: 'duration-literal',
    pattern: /\b(?:transition|animation)(?:-duration)?\s*:[^;]*?\b\d+(?:\.\d+)?m?s\b/g,
    cure: 'var(--motion-quick) or var(--motion-settle)',
  },
  {
    family: 'motion', name: 'easing-literal',
    pattern: /\b(?:transition|animation)[^;]*?\b(?:ease-in-out|ease-out|ease-in|linear|cubic-bezier\s*\()/g,
    cure: 'var(--motion-ease)',
  },
  {
    family: 'radius', name: 'radius-literal',
    pattern: /border-radius\s*:\s*(?![^;]*var\()[^;]*\d/g,
    cure: 'var(--radius-1|2|3)',
  },
  {
    family: 'interaction', name: 'cursor-literal',
    pattern: /cursor\s*:\s*(?!\s*var\()\s*(?:pointer|not-allowed|default|text|col-resize|row-resize|grab|grabbing|move)\b/g,
    cure: 'var(--cursor-act|refuse|inert|text|size-x|size-y)',
  },
  {
    family: 'interaction', name: 'focus-ring-literal',
    pattern: /outline(?:-width|-offset)?\s*:\s*(?![^;]*var\()[^;]*\d/g,
    cure: 'var(--focus-w) / var(--focus-offset) / var(--focus-ink)',
  },
  {
    family: 'interaction', name: 'press-transform-literal',
    pattern: /transform\s*:\s*translateY\(\s*(?!\s*var\()-?[\d.]+px/g,
    cure: 'var(--press-y)',
  },
  {
    family: 'interaction', name: 'state-opacity-literal',
    // Fractional only. `opacity: 1` is the identity value -- the absence of a treatment,
    // written where a rule returns a thing to normal -- and demanding a token for it
    // would mean inventing `--full-opacity: 1`, which is a name for nothing. A FRACTION
    // is always somebody's judgement about how faded a state should read, and that is
    // exactly the judgement the roster is supposed to hold once.
    pattern: /opacity\s*:\s*(?!\s*var\()\s*0?\.\d+\s*;/g,
    cure: 'var(--disabled-opacity) or var(--idle-opacity)',
  },
  {
    family: 'elevation', name: 'shadow-literal',
    pattern: /box-shadow\s*:\s*(?![^;]*var\()[^;]*\d/g,
    cure: 'var(--shadow-menu) or var(--shadow-palette)',
  },
  {
    family: 'scrollbar', name: 'scrollbar-literal',
    // `scrollbar-width` accepts auto|thin|none and CANNOT take a length, so flagging it
    // was asking for a token it is not legal to spend there. Only two things in a
    // scrollbar rule are genuinely a call site's invention: a colour, and the webkit
    // track/thumb geometry in px.
    pattern: /scrollbar-color\s*:\s*(?![^;]*var\()[^;]+;|::-webkit-scrollbar[^{]*\{[^}]*\b\d+px/g,
    cure: 'var(--scrollbar-w|track|thumb|thumb-hover|radius)',
  },
]);

/** Values the roster owns, so a spacing literal can be told from an arbitrary length. */
export function rosterValues(cssText = fs.readFileSync(TOKENS_CSS, 'utf8')) {
  const byName = new Map();
  for (const m of cssText.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/gi)) {
    if (!byName.has(m[1])) byName.set(m[1], m[2].trim());
  }
  return byName;
}

export function familyOf(token) {
  for (const [family, names] of Object.entries(FAMILIES)) if (names.includes(token)) return family;
  return null;
}

function hitsIn(text, pattern) {
  const out = [];
  text.split(/\r?\n/).forEach((line, i) => {
    for (const m of line.matchAll(pattern)) out.push({ line: i + 1, match: m[0].trim(), text: line.trim() });
  });
  return out;
}

export function survey({ root = ROOT, files = null } = {}) {
  const population = files ?? drawingFiles(root);
  const offenders = [];
  for (const file of population) {
    const code = codeOf(file.text);
    const isCss = file.rel.endsWith('.css');
    for (const rule of RULES) {
      const pattern = (!isCss && rule.jsPattern) ? rule.jsPattern : rule.pattern;
      for (const hit of hitsIn(code, pattern)) {
        offenders.push({ file: file.rel, rule: rule.name, family: rule.family, cure: rule.cure, ...hit });
      }
    }
  }
  return {
    counted: population.length,
    populationNonEmpty: population.length > 0,
    offenders,
    byFamily: offenders.reduce((acc, o) => { acc[o.family] = (acc[o.family] ?? 0) + 1; return acc; }, {}),
    byFile: offenders.reduce((acc, o) => { acc[o.file] = (acc[o.file] ?? 0) + 1; return acc; }, {}),
  };
}

/** The generated index. A build artifact of the one source, never a second source. */
export function buildIndex(cssText = fs.readFileSync(TOKENS_CSS, 'utf8')) {
  const values = rosterValues(cssText);
  const families = {};
  for (const [family, names] of Object.entries(FAMILIES)) {
    families[family] = Object.fromEntries(
      names.filter((n) => values.has(n)).map((n) => [n, values.get(n)]),
    );
  }
  const declared = [...values.keys()];
  const classified = Object.values(FAMILIES).flat();
  return {
    note: 'GENERATED by tools/protocol_gate.mjs --index from tokens/tokens.css. Do not hand-edit. The CSS is the source of record; this is a machine-readable face of it so tools and other repos (req/885) can consume the protocol without parsing CSS.',
    source: 'tokens/tokens.css',
    generatedAt: new Date().toISOString(),
    families,
    unclassified: declared.filter((n) => !classified.includes(n) && !n.startsWith('--l-')),
  };
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));
if (isMain) {
  if (process.argv.includes('--index')) {
    const index = buildIndex();
    fs.writeFileSync(INDEX_PATH, `${JSON.stringify(index, null, 2)}\n`, 'utf8');
    process.stdout.write(`${INDEX_PATH}\n`);
    if (index.unclassified.length) {
      process.stdout.write(`unclassified tokens (a family is missing a name): ${index.unclassified.join(', ')}\n`);
    }
    process.exit(0);
  }
  const report = survey();
  if (!report.populationNonEmpty) {
    process.stdout.write(`${GATE_MESSAGES.EMPTY_POPULATION}\n`);
    process.exit(1);
  }
  if (process.argv.includes('--json')) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
    process.exit(report.offenders.length ? 1 : 0);
  }
  process.stdout.write(`protocol gate: ${report.counted} drawing files\n`);
  if (!report.offenders.length) {
    process.stdout.write(`${GATE_MESSAGES.CLEAN}\n`);
    process.exit(0);
  }
  for (const [file, n] of Object.entries(report.byFile).sort((a, b) => b[1] - a[1])) {
    process.stdout.write(`\n  ${file}  (${n})\n`);
    for (const o of report.offenders.filter((x) => x.file === file)) {
      process.stdout.write(`    ${String(o.line).padStart(5)}  ${o.rule.padEnd(24)} ${o.match}\n`);
      process.stdout.write(`           -> ${o.cure}\n`);
    }
  }
  process.stdout.write(`\n${GATE_MESSAGES.HARDCODED}: ${report.offenders.length} in ${Object.keys(report.byFile).length} files\n`);
  process.stdout.write(`by family: ${JSON.stringify(report.byFamily)}\n`);
  process.exit(1);
}
