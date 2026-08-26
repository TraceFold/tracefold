// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  readTokenSource, declarations, palettes,
  parseColour, contrastRatio, contrastTable, CONSUMED, FORBIDDEN_TOKENS, DETAIL_X_CONTRACT,
  AA_NORMAL_TEXT, TOKEN_MESSAGES, INK_ON_BED, bedTable,
} from '../src/tokens.mjs';
// Node-only: resolves a real path against a real disk. Moved out of tokens.mjs so
// that module carries zero node:* imports (req/02 W15 -- a browser loads it).
import { tokenSourcePath, tokenSourceRealPath, tokenHref, TOKEN_SOURCE_MESSAGES } from '../tools/token-source.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PACKAGE = path.resolve(HERE, '..');

test('the stylesheet of record is where this package says it is', () => {
  const target = tokenSourcePath();
  assert.equal(fs.existsSync(target), true, `${TOKEN_SOURCE_MESSAGES.SOURCE_MISSING}: ${target}`);
  assert.equal(path.basename(target), 'tokens.css');
  assert.equal(tokenSourceRealPath().toLowerCase().endsWith(path.join('tokens', 'tokens.css').toLowerCase()), true);
});

test('there is no second roster: nothing inside this package declares a token', () => {
  const declaring = [];
  // 'shots' holds screenshots, not source. 'generated' holds exactly one file,
  // tokens.generated.mjs, which does carry the roster's declarations -- as a string,
  // not as a second set of --name: value; declarations of its own -- because a
  // browser has no node:fs to read them from disk at runtime (req/02 W15). That copy
  // is not exempted from scrutiny by being skipped here: it is checked by a stricter
  // gate than a grep can be, in tokens-generated.test.mjs, which re-reads the
  // canonical file and fails the moment the two stop being byte-identical. A hand
  // roster could drift silently; this one cannot.
  const walkDir = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) { if (entry.name !== 'shots' && entry.name !== 'generated') walkDir(full); continue; }
      if (!/\.(css|mjs|js|html)$/.test(entry.name)) continue;
      const body = fs.readFileSync(full, 'utf8');
      if (/--(bg|ink|line|deny|row|pad-x|spine-x)\s*:/.test(body)) declaring.push(path.relative(PACKAGE, full));
    }
  };
  walkDir(PACKAGE);
  assert.deepEqual(declaring, [], TOKEN_MESSAGES.SECOND_ROSTER);
});

test('the href a fixture writes resolves back to the same one file', () => {
  const fixtures = path.join(PACKAGE, 'fixtures');
  const href = tokenHref(fixtures);
  assert.equal(href.includes('\\'), false, 'an href uses forward slashes on every platform');
  assert.equal(path.resolve(fixtures, href), tokenSourcePath());
});

test('every name this package spells is a var() reference and none of them is forbidden', () => {
  for (const [role, value] of Object.entries(CONSUMED)) {
    assert.match(value, /^var\(--[a-z0-9-]+\)$/, `${role} must be a reference, not a value`);
    for (const forbidden of FORBIDDEN_TOKENS) assert.equal(value.includes(forbidden), false);
  }
});

test('every name this package spells is actually declared in the stylesheet of record', () => {
  const source = readTokenSource();
  const declared = new Set(declarations(source).map((d) => d.name));
  for (const value of Object.values(CONSUMED)) {
    const name = /var\((--[a-z0-9-]+)\)/.exec(value)[1];
    assert.equal(declared.has(name), true, `${name} is consumed here but not declared there`);
  }
});

test('the two palettes are read apart, and both carry every ink', () => {
  const { dark, light } = palettes(readTokenSource());
  for (const name of ['--bg', '--ink', '--ink-2', '--ink-3', '--line', '--deny']) {
    assert.match(dark[name] ?? '', /^#/, `dark ${name}`);
    assert.match(light[name] ?? '', /^#/, `light ${name}`);
  }
  assert.notEqual(dark['--bg'], light['--bg']);
});

test('the contrast function reproduces the four ratios the stylesheet states for itself', () => {
  // The dark side is the only side that states its own numbers. Reproducing them is
  // what makes the light side's numbers -- which nobody stated -- worth reading.
  // req/822_c7: the dark ground moved from neutral #0a0a0c to ink-blue #0d1016 (theme
  // identity, Owner #388). This instrument fired red on the move (all four stated
  // ratios drifted), which is exactly its job; the stated set below is the re-measured
  // set the stylesheet now states for itself.
  const stated = { '--ink': 15.5, '--ink-2': 7.2, '--ink-3': 5.0, '--deny': 4.7 };
  const { dark } = palettes(readTokenSource());
  for (const [name, claimed] of Object.entries(stated)) {
    const measured = contrastRatio(dark[name], dark['--bg']);
    assert.ok(Math.abs(measured - claimed) < 0.1, `${name}: stated ${claimed}, measured ${measured.toFixed(3)}`);
  }
});

test('parseColour takes both spellings and refuses anything else', () => {
  assert.deepEqual(parseColour('#fff'), [255, 255, 255]);
  assert.deepEqual(parseColour('#000000'), [0, 0, 0]);
  const refuses = (value) => assert.throws(() => parseColour(value), (error) => {
    assert.ok(error.message.startsWith(TOKEN_MESSAGES.NOT_A_COLOUR), error.message);
    assert.ok(error.message.endsWith(String(value)), 'the refusal names what it was given');
    return true;
  });
  refuses('white');
  refuses('#12345');
  refuses('rgb(0,0,0)');
});

test('a ratio is symmetric and bottoms out at one', () => {
  assert.equal(contrastRatio('#ffffff', '#000000').toFixed(2), contrastRatio('#000000', '#ffffff').toFixed(2));
  assert.equal(contrastRatio('#123456', '#123456'), 1);
});

test('the inks this package sets text in clear the normal-text floor on both sides', () => {
  const table = contrastTable(readTokenSource());
  const used = ['--ink', '--ink-2', '--deny'];
  const failures = table.filter((r) => used.includes(r.name) && r.ratio < AA_NORMAL_TEXT);
  assert.deepEqual(failures.map((f) => `${f.side} ${f.name} ${f.ratio.toFixed(2)}`), []);
});

test('the third ink is measured, reported, and left unused', () => {
  // Its stated 5.2:1 is the dark side's figure. Against the light page -- which is
  // what this application opens as -- it lands within a rounding step of the floor.
  // The measurement is printed rather than asserted to a hard number, because the
  // point is that it is too close to the line to set text in, not that it is any
  // particular value.
  const table = contrastTable(readTokenSource());
  const light = table.find((r) => r.side === 'light' && r.name === '--ink-3');
  const dark = table.find((r) => r.side === 'dark' && r.name === '--ink-3');
  assert.ok(light && dark);
  assert.ok(light.ratio < dark.ratio, `light ${light.ratio.toFixed(3)} should be the weaker of the two`);
  assert.ok(Math.abs(light.ratio - AA_NORMAL_TEXT) < 0.5, `light --ink-3 measured ${light.ratio.toFixed(3)}:1 against a ${AA_NORMAL_TEXT}:1 floor`);
  assert.deepEqual(FORBIDDEN_TOKENS, ['--ink-3']);
});

// ---- Owner #340: the standing hues, and the number that decides whether a chip reads --

test('red-first: the bed reading refuses a pair whose ink cannot be read on its own bed', () => {
  // Fired before the silence below is read as a pass. The planted pair is a real
  // failure shape: an ink perfectly readable on the page (this is the light --admit
  // against a white-ish page) laid on a bed of nearly its own luminance, which is
  // exactly what happens when a bed is picked by eye from the hue rather than measured.
  //
  // The block carries no --bg, and it does not need one -- this reading is ink against
  // bed and never against the page, which is the whole reason it exists as a separate
  // table from contrastTable(). Keeping it out also keeps this file honestly clean for
  // the second-roster walk above, which would otherwise count a planted fixture string
  // as a roster; the fixture is narrowed rather than the gate loosened.
  const planted = ':root{--admit:#176b3d;--admit-bed:#1d7a46;}';
  const found = bedTable(planted).find((r) => r.side === 'dark' && r.ink === '--admit');
  assert.ok(found.ratio !== null);
  assert.ok(found.ratio < AA_NORMAL_TEXT, `planted pair measured ${found.ratio.toFixed(2)}:1 and should be under the floor`);
  // And a pair that is simply absent reads as absent, never as a pass.
  const missing = bedTable(':root{}').filter((r) => r.ratio === null);
  assert.equal(missing.length, INK_ON_BED.length * 2, 'a pair nobody declared is null, not fine');
});

test('every standing ink clears the normal-text floor on its own bed, on both pages', () => {
  const table = bedTable(readTokenSource());
  assert.equal(table.length, INK_ON_BED.length * 2, 'both sides, every pair');
  const failures = table.filter((r) => r.ratio === null || r.ratio < AA_NORMAL_TEXT);
  assert.deepEqual(
    failures.map((f) => `${f.side} ${f.ink} on ${f.bed}: ${f.ratio === null ? 'not declared' : f.ratio.toFixed(2)}`),
    [],
  );
});

test('every standing ink is also readable straight off the page, so a chip is not the only place it may be used', () => {
  const { dark, light } = palettes(readTokenSource());
  const weak = [];
  for (const [side, palette] of [['dark', dark], ['light', light]]) {
    for (const [ink] of INK_ON_BED) {
      const ratio = contrastRatio(palette[ink], palette['--bg']);
      if (ratio < AA_NORMAL_TEXT) weak.push(`${side} ${ink} ${ratio.toFixed(2)}`);
    }
  }
  assert.deepEqual(weak, []);
});

test('the four standings this app can be in are four different hues, so they are told apart before a word is read', () => {
  // The point of the pass, stated as a measurement rather than as an intention: no two
  // of the standings share an ink on either page. A pair that collided would be a pair
  // a reader at arm's length cannot separate, which is the defect Owner #340 named.
  const { dark, light } = palettes(readTokenSource());
  for (const [side, palette] of [['dark', dark], ['light', light]]) {
    const standings = ['--admit', '--deny', '--escalate', '--held'].map((n) => palette[n]);
    assert.equal(new Set(standings).size, standings.length, `${side}: ${standings.join(' ')}`);
    // And the one ink that means "you may press this" is none of them.
    assert.equal(standings.includes(palette['--act']), false, `${side}: --act collides with a standing`);
  }
});

test('every ink/bed name the roster spells is declared, and the roster names both halves of every pair', () => {
  const declared = new Set(declarations(readTokenSource()).map((d) => d.name));
  for (const [ink, bed] of INK_ON_BED) {
    assert.equal(declared.has(ink), true, `${ink} is paired here but not declared there`);
    assert.equal(declared.has(bed), true, `${bed} is paired here but not declared there`);
    const spelled = Object.values(CONSUMED);
    assert.equal(spelled.includes(`var(${ink})`), true, `${ink} is measured but no part may spell it`);
    assert.equal(spelled.includes(`var(${bed})`), true, `${bed} is measured but no part may spell it`);
  }
});

test('the collapse of --detail-x is answered by a stated contract, not left to whoever consumes it', () => {
  const source = readTokenSource();
  assert.match(source, /--detail-x\s*:\s*0px/, 'the source of record still collapses it');
  assert.match(DETAIL_X_CONTRACT, /stacked under its row at every width/);
});
