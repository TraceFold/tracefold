// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { toHtml, textOf, findByAttr, find } from '../src/element.mjs';
import {
  glyph, markOf, everyMark, sheetMarks, meaningCollisions, sheet, symbolId, MARKS, NAMESPACES,
  UNDEFINED_MARK, SHARED_MEANINGS, RED_RULE, SHEET_ID, BOX, STROKE, GLYPH_MESSAGES,
  MIN_READABLE, MIN_ACT,
} from '../src/glyph-sheet.mjs';
import { verdictFor, strokeReach } from '../tools/glyph-bounds.mjs';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

test('a glyph without a size throws rather than drawing at whatever size an svg defaults to', () => {
  // This is the whole of AC-F3. N-2 was two glyphs at the SVG default size because
  // the rule meant to size them was written for another container.
  assert.throws(() => glyph('verdict', 'Admit'), new RegExp(GLYPH_MESSAGES.SIZE_REQUIRED));
  assert.throws(() => glyph('verdict', 'Admit', {}), new RegExp(GLYPH_MESSAGES.SIZE_REQUIRED));
  assert.throws(() => glyph('verdict', 'Admit', { size: null }), new RegExp(GLYPH_MESSAGES.SIZE_REQUIRED));
  assert.throws(() => glyph('verdict', 'Admit', { size: 0 }), new RegExp(GLYPH_MESSAGES.SIZE_REQUIRED));
  assert.throws(() => glyph('verdict', 'Admit', { size: -8 }), new RegExp(GLYPH_MESSAGES.SIZE_REQUIRED));
  assert.throws(() => glyph('verdict', 'Admit', { size: '14' }), new RegExp(GLYPH_MESSAGES.SIZE_REQUIRED));
  assert.throws(() => glyph('verdict', 'Admit', { size: Number.NaN }), new RegExp(GLYPH_MESSAGES.SIZE_REQUIRED));
});

test('the size is written twice, so losing every stylesheet cannot change how big it is', () => {
  const node = glyph('verdict', 'Deny', { size: 18 });
  assert.equal(node.attrs.width, '18');
  assert.equal(node.attrs.height, '18');
  assert.match(node.attrs.style, /width:18px/);
  assert.match(node.attrs.style, /height:18px/);
  assert.equal(node.attrs.viewBox, `0 0 ${BOX} ${BOX}`);
});

test('an unknown name is drawn and labelled, never returned as nothing', () => {
  const node = glyph('standing', 'invented-word', { size: 16 });
  assert.equal(node.attrs['data-mark'], 'undefined');
  assert.match(node.attrs['aria-label'], /undefined mark: standing\/invented-word/);
  assert.equal(node.children.length, 1, 'it still points at a drawing');
  assert.equal(markOf('standing', 'invented-word').defined, false);
  assert.deepEqual(markOf('standing', 'invented-word').strokes, UNDEFINED_MARK.strokes);
});

test('an unknown namespace is a mistake in code and raises, unlike an unknown name', () => {
  assert.throws(() => markOf('nonsense', 'Admit'), new RegExp(GLYPH_MESSAGES.UNKNOWN_NAMESPACE));
  assert.throws(() => glyph('nonsense', 'Admit', { size: 14 }), new RegExp(GLYPH_MESSAGES.UNKNOWN_NAMESPACE));
});

test('the verdict namespace holds the engine spelling and nothing invented beside it', () => {
  assert.deepEqual(Object.keys(MARKS.verdict), ['Admit', 'Deny', 'Escalate']);
  for (const key of Object.keys(MARKS.verdict)) assert.match(key, /^[A-Z]/, 'the wire spelling is PascalCase');
});

test('the namespaces are separate: a standing word is not a verdict and does not answer as one', () => {
  assert.equal(markOf('verdict', 'held').defined, false);
  assert.equal(markOf('standing', 'held').defined, true);
  assert.equal(markOf('verdict', 'Admit').id, symbolId('verdict', 'Admit'));
  assert.notEqual(markOf('standing', 'held').id, markOf('verdict', 'Admit').id);
});

test('no two marks carry one meaning unless the sharing is declared', () => {
  assert.deepEqual(meaningCollisions(), []);
  assert.deepEqual(SHARED_MEANINGS, []);
});

test('the collision gate refuses a planted collision', () => {
  const planted = [
    { namespace: 'verdict', key: 'Admit', means: 'verdict.admit' },
    { namespace: 'standing', key: 'fine', means: 'verdict.admit' },
  ];
  assert.deepEqual(meaningCollisions(planted, []), [{ means: 'verdict.admit', names: ['verdict/Admit', 'standing/fine'] }]);
  assert.deepEqual(meaningCollisions(planted, [{ means: 'verdict.admit', why: 'declared' }]), []);
});

test('every mark carries an origin line and a meaning, the stand-in included', () => {
  for (const mark of sheetMarks()) {
    assert.equal(typeof mark.source, 'string');
    assert.ok(mark.source.length > 20, `${mark.namespace}/${mark.key} needs an origin worth reading`);
    assert.match(mark.means, /^[a-z][a-z.-]+$/);
    assert.ok(mark.strokes.length > 0);
    for (const stroke of mark.strokes) assert.match(stroke.d, /^M/, 'a stroke starts by moving somewhere');
  }
});

test('the sheet carries the red rule as a node, not as a footnote in a picture', () => {
  const html = toHtml(sheet());
  assert.equal(html.includes(RED_RULE), true);
  assert.match(textOf(sheet()), /deny mark is the only thing/);
  const rule = findByAttr(sheet(), 'id', `${SHEET_ID}-rule`);
  assert.equal(rule.length, 1);
});

test('the sheet holds each mark once, under an id nothing else uses', () => {
  const symbols = find(sheet(), (n) => n.tag === 'symbol');
  const ids = symbols.map((s) => s.attrs.id);
  assert.equal(new Set(ids).size, ids.length);
  assert.equal(ids.length, sheetMarks().length);
  assert.equal(sheetMarks().length, everyMark().length + 1, 'the stand-in ships but cannot be asked for by name');
  assert.equal(everyMark().some((m) => m.namespace === 'undefined'), false);
  for (const namespace of NAMESPACES) {
    for (const key of Object.keys(MARKS[namespace])) assert.ok(ids.includes(symbolId(namespace, key)));
  }
  assert.ok(ids.includes(symbolId('undefined', 'mark')), 'the undefined mark ships too, or it cannot be drawn');
});

test('the sheet takes no space and announces itself as decoration', () => {
  const node = sheet();
  assert.equal(node.attrs['aria-hidden'], 'true');
  assert.match(node.attrs.style, /width:0/);
  assert.match(node.attrs.style, /height:0/);
  assert.equal(node.attrs.id, SHEET_ID);
});

test('every instance carries its own stroke, because instanced content inherits from where it is used', () => {
  // Found by looking at a picture: with these only on the sprite, every mark drew as
  // a filled black shape while every measurement stayed correct.
  for (const [name, value] of Object.entries(STROKE)) {
    assert.equal(glyph('verdict', 'Admit', { size: 14 }).attrs[name], value, name);
    assert.equal(sheet().attrs[name], value, `sprite ${name}`);
  }
  assert.equal(glyph('structure', 'seal', { size: 16 }).attrs.fill, 'none', 'a ring must not fill into a disc');
});

test('a glyph points at the sheet by id and carries its meaning for anything that reads the tree', () => {
  const node = glyph('structure', 'child', { size: 14 });
  assert.equal(node.children[0].attrs.href, `#${symbolId('structure', 'child')}`);
  assert.equal(node.attrs['data-means'], 'structure.child');
  assert.equal(node.attrs.role, 'img');
});

// ---- Owner #348 (3) + (5): icon integrity, held mechanically ------------------------

test('red-first: the clipping detector fires on a mark that reaches past its own box', () => {
  // The detector's verdict is a pure function of a measured bounding box, so it can be
  // fired here without a renderer -- the renderer's only job is producing the box, and
  // `parts/tools/glyph-bounds.mjs` run as a command is what does that against the real
  // marks. These are the four ways a mark can cross an edge, plus the one that draws
  // nothing at all.
  const fits = { id: 'ok', paths: 1, minX: 4, minY: 4, maxX: 20, maxY: 20 };
  assert.equal(verdictFor(fits).fits, true);
  assert.deepEqual(verdictFor(fits).over, {});

  assert.deepEqual(verdictFor({ ...fits, minX: 0.5 }).over, { left: 0.5 });
  assert.deepEqual(verdictFor({ ...fits, minY: 0 }).over, { top: 1 });
  assert.deepEqual(verdictFor({ ...fits, maxX: 23.5 }).over, { right: 0.5 });
  assert.deepEqual(verdictFor({ ...fits, maxY: 24 }).over, { bottom: 1 });
  // A mark that draws no path is not a mark that fits.
  assert.equal(verdictFor({ id: 'empty', paths: 0 }).fits, false);

  // The edge case that makes the stroke matter: geometry exactly on the boundary is
  // clipped by half its stroke, and a detector that only compared geometry to the box
  // would call it fine.
  assert.equal(verdictFor({ ...fits, minX: 0, maxX: 24 }).fits, false);
});

test('the stroke reach is derived from the sheet it is measuring, not assumed', () => {
  assert.equal(strokeReach(), Number(STROKE['stroke-width']) / 2);
  // And it refuses to report a number for a join style it was not computed for, rather
  // than returning a half-width that a mitre would exceed.
  assert.throws(() => strokeReach({ ...STROKE, 'stroke-linejoin': 'miter' }), /round caps and joins/);
});

test('every shipped call site draws a mark at or above the readable floor', () => {
  // Owner #348 (3): 16px at density, 20px on an act. Enforced over the source rather
  // than inside glyph(), because this application draws exactly one mark below the
  // floor on purpose and a throw would forbid it. The exception is named with its
  // reason; an unnamed one fails.
  const EXCEPTIONS = new Map([
    ['sash', 'the grip on a divider, not an icon -- it carries no meaning to decode and must not read as content'],
  ]);
  const roots = ['parts/src', 'shell/kernel', 'faces'];
  const offenders = [];
  let counted = 0;
  const walkDir = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (['fixtures', 'record', 'shots', 'test', 'node_modules', '.bench', '.run'].includes(entry.name)) continue;
        walkDir(full);
        continue;
      }
      if (!entry.name.endsWith('.mjs')) continue;
      const text = fs.readFileSync(full, 'utf8');
      for (const [, name, value] of text.matchAll(/(\w+)\s*:\s*(\d+)(?=\s*[,}])/g)) {
        if (!/^(size|rail|tab|sash)$/.test(name)) continue;
        counted += 1;
        const n = Number(value);
        if (n >= MIN_READABLE) continue;
        if (EXCEPTIONS.has(name)) continue;
        offenders.push(`${path.relative(REPO, full)}: ${name}: ${n}`);
      }
    }
  };
  for (const root of roots) walkDir(path.join(REPO, root));
  assert.ok(counted > 0, 'the rule was applied to nothing, which is not the rule holding');
  assert.deepEqual(offenders, [], `marks drawn under the ${MIN_READABLE}px floor`);
  // The exception list is not allowed to grow silently either.
  assert.deepEqual([...EXCEPTIONS.keys()], ['sash']);
  for (const reason of EXCEPTIONS.values()) assert.ok(reason.length > 20, 'an exception states why');
});
