// SPDX-License-Identifier: Apache-2.0
// C1 -- the token part, which does not make tokens.
//
// The contract is consumption: there is one physical stylesheet of record, this
// package points at it by path, and this module holds no colour of its own. Not one
// hex literal appears below, on purpose -- a second spelling of a colour is a second
// source of truth even when today the two agree byte for byte (req/04 T-1).
//
// It also does one thing the source of record does not do for itself. The dark side
// of that file states a measured contrast ratio beside every ink; the light side
// states none (req/04a C1, defect 1). So the ratios are computed here, from the
// file, at test time -- a number nobody has to trust.
//
// This module used to read that file with node:fs at import time, which is exactly
// what a real browser cannot do: this is the module every face's drawing parts pull
// their inks and type scale from, and a face is loaded straight into a window with
// no build step standing between the two (req/02 W15). The read still happens in
// exactly one place -- it moved from "every time this module loads" to "once, at
// build time, in parts/tools/generate-tokens.mjs" -- and what that produces is
// parts/generated/tokens.generated.mjs: the stylesheet's text, carried here as a
// plain string, with a SHA-256 of the bytes it was made from stamped beside it.
// parts/test/tokens-generated.test.mjs re-reads the canonical file itself and fails
// the moment that digest stops matching, which is what keeps this from quietly
// becoming a second roster with a timestamp attached. Path resolution against the
// real disk -- proving a junction or a copy is not standing in for the one file, and
// building the href a fixture writes into its <link> -- is Node-only work and lives
// in parts/tools/token-source.mjs instead, which nothing a browser loads may import.

import {
  TOKEN_SOURCE_RELATIVE, TOKEN_SOURCE_SHA256, TOKEN_GENERATED_AT, TOKEN_SOURCE_TEXT,
} from '../generated/tokens.generated.mjs';

export const TOKEN_MESSAGES = {
  SECOND_ROSTER: 'a second token roster exists inside this package',
  NOT_A_COLOUR: 'not a colour this module can measure (expected #rgb or #rrggbb)',
  READ: 'tokens read from the source of record',
};

/** Re-exported so a caller that only has tokens.mjs can still name and date the
 * mirror it is reading, without importing the Node-only generator or accessor. */
export { TOKEN_SOURCE_RELATIVE, TOKEN_SOURCE_SHA256, TOKEN_GENERATED_AT };

/** The stylesheet's text. No disk touched here -- the generated module already
 * carries it, and tokens-generated.test.mjs is what proves that copy is current. */
export function readTokenSource() {
  return TOKEN_SOURCE_TEXT;
}

/**
 * The names this package is allowed to spell. Z-2 makes token variable names the
 * standing exception to inherited identifiers: T-1 requires consuming one roster, so
 * these names belong to that roster and not to any implementation that also reads it.
 */
export const CONSUMED = Object.freeze({
  ink: 'var(--ink)',
  attendant: 'var(--ink-2)',
  rule: 'var(--line)',
  page: 'var(--bg)',
  // req/822_c7 (Owner #388 theme pass): one elevation step off the page, for the stat
  // band and the detail pane -- the two organs the reference set draws as panels.
  // Spelled raised-first (like bedDeny) so a substring count of `CONSUMED.page`
  // cannot mistake this for a second page site.
  raisedPage: 'var(--bg-raised)',
  deny: 'var(--deny)',
  // Owner #340's anti-monotone pass. Two roles, and the names say which is which: an
  // `ink` is set text (and stroke) in; a `bed` is only ever a background a matching ink
  // sits on. They are declared here in pairs so a caller cannot reach for one without
  // the other being one line away, which is the whole of how a filled chip stays
  // legible -- the stylesheet of record states the measured ratio for every pair and
  // parts/test/tokens.test.mjs recomputes all ten at test time.
  //
  // `bedDeny` is spelled with the bed first on purpose. tools/boundary.mjs counts the
  // sites that spend the deny colour by looking for `CONSUMED.deny`, and a name of the
  // form `CONSUMED.denyBed` would be counted as a second site by a substring match --
  // which would either weaken the gate or force the gate to be loosened to accommodate
  // a name. The order of two words is cheaper than either.
  act: 'var(--act)',
  bedAct: 'var(--act-bed)',
  admit: 'var(--admit)',
  bedAdmit: 'var(--admit-bed)',
  bedDeny: 'var(--deny-bed)',
  escalate: 'var(--escalate)',
  bedEscalate: 'var(--escalate-bed)',
  held: 'var(--held)',
  bedHeld: 'var(--held-bed)',
  // Owner #349's corner scale and #348's motion route. Named by what the thing is
  // rather than by its number, so a call site says `radiusControl` and cannot pick a
  // tier by eye: `CONSUMED.radiusChip` on something that is not a chip reads wrong in
  // the source, which is the only place a wrong tier can be caught.
  radiusChip: 'var(--radius-1)',
  radiusControl: 'var(--radius-2)',
  radiusContainer: 'var(--radius-3)',
  motionQuick: 'var(--motion-quick)',
  motionSettle: 'var(--motion-settle)',
  motionEase: 'var(--motion-ease)',
  pitch: 'var(--row)',
  padX: 'var(--pad-x)',
  spineX: 'var(--spine-x)',
  mono: 'var(--mono)',
  sans: 'var(--sans)',
  meta: 'var(--t-meta)',
  metaLine: 'var(--lh-meta)',
  time: 'var(--t-time)',
  timeLine: 'var(--lh-time)',
  record: 'var(--t-record)',
  recordLine: 'var(--lh-record)',
  head: 'var(--t-head)',
  headLine: 'var(--lh-head)',
  // req/822_c7: the stat figure's own step. The figure was 22px written as a literal
  // inside surface.mjs's rules -- a size nobody could retune from the roster. The
  // caption under it keeps the record size (SS693: the floor is hard; the figure is
  // what carries the hierarchy).
  stat: 'var(--t-stat)',
  statLine: 'var(--lh-stat)',
});

/**
 * `--ink-3` is deliberately absent from CONSUMED and no part may spell it.
 *
 * Reason, measured rather than assumed: the source of record states 5.2:1 for that
 * ink, and that figure is the dark side's. Computed against the light page the same
 * ink sits within a rounding error of the 4.5:1 floor (see contrastTable, run at test
 * time), and light is this application's default. An ink that may or may not clear
 * the floor depending on how a ratio is rounded is not an ink to set text in, so the
 * aside role takes `--ink-2` here and the third ink goes unused. The gate in
 * tools/boundary.mjs holds the count at zero.
 */
export const FORBIDDEN_TOKENS = Object.freeze(['--ink-3']);

/**
 * `--detail-x` collapses to 0 below 1000px, and the shipping window is narrower than
 * that, so the collapsed case is the normal case rather than the exception (req/04a
 * C1, defect 2). The source of record sets the number and says nothing about what
 * happens next; the answer this package gives is that nothing happens next, because
 * no part of it places a note beside a row at any width. See receipt-row.
 */
export const DETAIL_X_CONTRACT = 'no part positions by --detail-x; a note is stacked under its row at every width, so there is no width at which "beside" stops working';

const ROOT_BLOCK = /:root\s*(\[[^\]]*\])?\s*\{([^}]*)\}/g;
const DECLARATION = /(--[a-z0-9-]+)\s*:\s*([^;]+);/gi;

/** Every declaration in the file, in order, with the selector that carried it. */
export function declarations(cssText) {
  const out = [];
  for (const block of cssText.matchAll(ROOT_BLOCK)) {
    const qualifier = block[1] ?? '';
    for (const decl of block[2].matchAll(DECLARATION)) {
      out.push({ name: decl[1], value: decl[2].trim(), qualifier });
    }
  }
  return out;
}

/**
 * The two palettes, kept apart. Bare `:root` carries the dark values; the light ones
 * are declared once under their own names and then aliased by the theme rules, so
 * reading `--l-*` reads light without having to evaluate a cascade.
 */
export function palettes(cssText) {
  const dark = {};
  const light = {};
  for (const { name, value, qualifier } of declarations(cssText)) {
    if (value.startsWith('var(')) continue;
    if (name.startsWith('--l-')) light[name.replace('--l-', '--')] = value;
    else if (qualifier === '') dark[name] = value;
  }
  return { dark, light };
}

export function parseColour(value) {
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(String(value).trim());
  if (!hex) throw new Error(`${TOKEN_MESSAGES.NOT_A_COLOUR}: ${value}`);
  const digits = hex[1].length === 3 ? hex[1].split('').map((c) => c + c).join('') : hex[1];
  return [0, 2, 4].map((i) => parseInt(digits.slice(i, i + 2), 16));
}

const channel = (v) => {
  const s = v / 255;
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
};

export function relativeLuminance(value) {
  const [r, g, b] = parseColour(value).map(channel);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function contrastRatio(a, b) {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  const [hi, lo] = la >= lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}

export const AA_NORMAL_TEXT = 4.5;
export const AA_LARGE_TEXT = 3;

/**
 * The five ink/bed pairs Owner #340's pass introduced, named rather than measured
 * here: a filled chip draws its ink on its own bed, so the ratio that decides whether
 * the chip is readable is the ink against the BED, not the ink against the page. The
 * page-relative figure -- which is what contrastTable() computes and what a palette
 * file conventionally states -- says nothing about a chip at all.
 *
 * This is a table of names. bedTable() below is what turns it into numbers, from the
 * stylesheet of record, at test time.
 */
export const INK_ON_BED = Object.freeze([
  Object.freeze(['--act', '--act-bed']),
  Object.freeze(['--admit', '--admit-bed']),
  Object.freeze(['--deny', '--deny-bed']),
  Object.freeze(['--escalate', '--escalate-bed']),
  Object.freeze(['--held', '--held-bed']),
]);

/** Every pair above, measured on both pages, in one table. */
export function bedTable(cssText) {
  const { dark, light } = palettes(cssText);
  const rows = [];
  for (const [side, palette] of [['dark', dark], ['light', light]]) {
    for (const [ink, bed] of INK_ON_BED) {
      const on = palette[ink];
      const under = palette[bed];
      if (!on || !under) { rows.push({ side, ink, bed, ratio: null }); continue; }
      rows.push({ side, ink, bed, ratio: contrastRatio(on, under) });
    }
  }
  return rows;
}

/** Every ink measured against its own page, both sides, in one table. */
export function contrastTable(cssText) {
  const { dark, light } = palettes(cssText);
  const rows = [];
  for (const [side, palette] of [['dark', dark], ['light', light]]) {
    for (const [name, value] of Object.entries(palette)) {
      if (name === '--bg' || !/^#/.test(value)) continue;
      rows.push({ side, name, value, ratio: contrastRatio(value, palette['--bg']) });
    }
  }
  return rows;
}
