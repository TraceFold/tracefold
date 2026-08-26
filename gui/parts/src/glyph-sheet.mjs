// SPDX-License-Identifier: Apache-2.0
// C2 -- the one sheet of marks, and the only door through which a mark is drawn.
//
// Two defects are closed by construction here rather than by discipline.
//
// A size is an argument, not a stylesheet's opinion (req/03 AC-F3). N-2 put two
// glyphs on screen at the SVG default size because the rule that would have sized
// them was written for a different container and simply never landed. So `glyph`
// refuses to draw without a size, and it writes that size twice -- as attributes and
// as inline width/height -- which leaves no path from "the CSS did not apply" to
// "something enormous was drawn".
//
// And the safety sentence lives in the sheet (req/04a C2, defect 1). In the tree
// this replaces, the sentence that says red means one thing existed only in a PNG's
// footer: nothing in the source could be searched for it, so nothing could notice it
// go missing. Here it is a <desc> node inside the sprite -- greppable, shipped, and
// read out by assistive technology.

import { el, style } from './element.mjs';

export const GLYPH_MESSAGES = {
  SIZE_REQUIRED: 'a glyph is drawn at a stated size; there is no default size',
  UNKNOWN_NAMESPACE: 'no such mark namespace',
  UNDEFINED_MARK: 'undefined mark',
  SHEET_INSTALLED: 'glyph sheet installed',
  SHEET_ALREADY: 'glyph sheet was already installed; the sheet is injected once',
};

export const BOX = 24;
export const SHEET_ID = 'gx-glyph-sheet';

/**
 * The sizes a mark may be drawn at, and why there are two numbers rather than one.
 *
 * Owner #348 (3): the Owner is seeing cut-off icons. `parts/tools/glyph-bounds.mjs`
 * measured every mark's real drawn geometry in a renderer -- all 26 fit inside the box
 * with their stroke, the tightest by exactly one unit -- so the marks are not clipped by
 * their own drawing. What was wrong is the scale they were drawn AT: a 24-unit design
 * with a 2-unit stroke, rendered into 12 or 14 pixels, puts that stroke under a pixel
 * and the detail between two strokes under half of one. It does not read as small; it
 * reads as broken.
 *
 * MIN_READABLE is the floor for a mark sitting in a line of text beside its own word.
 * MIN_ACT is the floor for a mark on a control a hand is meant to land on, where the
 * mark is doing more of the work of saying what the control does.
 *
 * These are floors, not sizes. A caller still states its own number -- `glyph()` refuses
 * to draw without one -- and parts/test/glyph-sheet.test.mjs holds every shipped call
 * site at or above the floor with a named, reasoned exception list, because a floor
 * enforced inside `glyph()` would forbid the one legitimate sub-floor mark this
 * application draws (a sash's grip, which is furniture and not an icon).
 */
export const MIN_READABLE = 16;
export const MIN_ACT = 20;

export const RED_RULE = 'one-point red: the deny mark is the only thing in this package that is ever drawn in the deny colour. Nothing else is red, and red never means emphasis.';

/**
 * Four namespaces, because a flat table let a caller ask the wrong table and be
 * answered with silence (req/04a C3). A verdict is a word the engine says; a
 * standing is a word this window says about a record; an effect is the kind of
 * change a row names (a verb); a structure mark is furniture. Asking an unknown
 * namespace raises -- that is a mistake in code, not in data.
 */
export const NAMESPACES = Object.freeze(['verdict', 'standing', 'effect', 'act', 'structure']);

/**
 * The verdict namespace holds the engine's three words and spells them the way the
 * engine writes them. Source: membrane/src/wire.mjs VERDICT_KINDS, which read them
 * out of gx-core/src/verdict.rs and ruled the wire spelling PascalCase.
 *
 * The three drawings follow the convention survey this project ran across the major
 * icon sets (research_bus/tracefold_ui_refs/inbox/04_glyph_convention.md) rather than
 * this project's own invention: a check for admitted, a cross for denied, a person
 * for raised to a human (the survey found no single standard glyph for "escalate" and
 * named "person + up" as the closest cross-set convention). The strokes below are
 * this package's own coordinates -- authored to the same silhouette, not lifted from
 * any sheet (COPY HARD BAN); the geometry that is shared with the reference sheet at
 * research_bus/tracefold_design/mocks_v4/glyph_sheet.html is the convention itself
 * (24-unit box, 1.7 stroke, round joins), which is the part a convention is for.
 */
export const MARKS = Object.freeze({
  verdict: Object.freeze({
    Admit: {
      means: 'verdict.admit',
      source: 'a checkmark: the cross-icon-set convention for admitted / passed / done',
      strokes: [{ d: 'M4.6 12.4l4.6 4.6L19.4 6.8' }],
    },
    Deny: {
      means: 'verdict.deny',
      source: 'a cross: the cross-icon-set convention for denied / refused, drawn as two strokes meeting in the middle rather than one continuous zigzag so it reads at small sizes',
      strokes: [{ d: 'M6.4 6.4l11.2 11.2' }, { d: 'M17.6 6.4L6.4 17.6' }],
    },
    Escalate: {
      means: 'verdict.escalate',
      source: 'a person, raised to whom a decision is escalated: no icon set has a single standard mark for this meaning, and the convention survey named "person" as the closest shared element across sets -- drawn as a head and shoulders rather than the tray-and-arrow shape that reads as a share/upload silhouette',
      strokes: [{ d: 'M12 5.4a3.1 3.1 0 1 1 0 6.2 3.1 3.1 0 0 1 0-6.2z' }, { d: 'M5.6 19.6a6.4 6.4 0 0 1 12.8 0' }],
    },
  }),
  standing: Object.freeze({
    held: {
      means: 'standing.held',
      source: 'two uprights holding a line still: the line arrives and leaves, and is stopped between them',
      strokes: [{ d: 'M2.5 12h5' }, { d: 'M16.5 12h5' }, { d: 'M10 6.5v11' }, { d: 'M14 6.5v11' }],
    },
    cancelled: {
      means: 'standing.cancelled',
      source: 'a stroke through, which is what a clerk does to an entry that is not to be read as standing',
      strokes: [{ d: 'M5.5 5.5 18.5 18.5' }, { d: 'M18.5 5.5 5.5 18.5' }],
    },
    conflict: {
      means: 'standing.conflict',
      source: 'two movements meeting head on, neither yielding',
      strokes: [{ d: 'M2 12h7.5' }, { d: 'M6 8.5 9.5 12 6 15.5' }, { d: 'M22 12h-7.5' }, { d: 'M18 8.5 14.5 12 18 15.5' }],
    },
    none: {
      means: 'standing.none',
      source: 'an empty ring: a place where a standing would be written, with nothing written in it',
      strokes: [{ d: 'M12 4.5a7.5 7.5 0 0 1 0 15 7.5 7.5 0 0 1 0-15z' }],
    },
    reversed: {
      means: 'standing.reversed',
      source: 'req/768 F-I (the reversibility chip, retrofit round 2): a closed ring with one break, re-entered by an arrowhead -- deliberately not effect.undo/act.undo\'s open returning hook (a line that ends mid-motion, meaning "a row was appended against"). This mark is a different meaning: a row\'s own story has closed because a later row is on record as its reversal, so the ring closes on itself rather than trailing off. Own coordinates, not lifted from any icon set (COPY HARD BAN), same convention (24-unit box, round joins) every other mark here follows.',
      strokes: [{ d: 'M18.4 8.6a7.5 7.5 0 1 1 -2.9-4.9' }, { d: 'M19.4 3.4v5.6h-5.6' }],
    },
  }),
  /**
   * The kind of change a row names -- a verb, drawn as a glyph so the row does not
   * carry the word alone. Own coordinates, conventional silhouettes (a pencil-on-page
   * for a write, a can for a delete, a returning arrow for an undo), and two that
   * stand for how an entry reached this window rather than what changed: a globe for
   * a call that crossed the network, a bubble for one answered inside this window
   * without leaving it. Naming the layer that carried a call is not this package's
   * job (req/09 -- product words, not layer words); the glyph carries the fact and
   * the word next to it is written by the caller.
   */
  effect: Object.freeze({
    write: {
      means: 'effect.write',
      source: 'a page with a pencil at its corner: something was written',
      strokes: [{ d: 'M6 4h8.4l4 4v11.6H6z' }, { d: 'M14.4 4v4h4' }, { d: 'M9 13.4l6.6-6.6 2 2-6.6 6.6H9z' }],
    },
    delete: {
      means: 'effect.delete',
      source: 'a can with a lid: something was removed',
      strokes: [{ d: 'M5 7h14' }, { d: 'M9.5 7V5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v2' }, { d: 'M7 7l1 13h8l1-13' }],
    },
    undo: {
      means: 'effect.undo',
      source: 'an arrow returning along its own line: a prior row was appended against, not edited',
      strokes: [{ d: 'M5 10h10.5a4.5 4.5 0 0 1 0 9H10' }, { d: 'M9 6l-4 4 4 4' }],
    },
    network: {
      means: 'effect.network',
      source: 'a globe: this call crossed the network to be answered',
      strokes: [{ d: 'M12 3.6a8.4 8.4 0 1 0 0 16.8 8.4 8.4 0 0 0 0-16.8z' }, { d: 'M3.8 12h16.4' }, { d: 'M12 3.6c2.2 2.3 3.4 5.3 3.4 8.4S14.2 18.1 12 20.4c-2.2-2.3-3.4-5.3-3.4-8.4S9.8 5.9 12 3.6z' }],
    },
    message: {
      means: 'effect.message',
      source: 'a speech bubble: this was answered inside this window, without a call leaving it',
      strokes: [{ d: 'M4.6 6.4a2 2 0 0 1 2-2h10.8a2 2 0 0 1 2 2v7.2a2 2 0 0 1-2 2h-7l-3.8 3v-3h-0.2a2 2 0 0 1-2-2z' }],
    },
  }),
  /**
   * The three acts a row can offer (declaration.mjs ACTS), coined here because no
   * icon set has a standard mark for a domain verb like "commit a candidate" -- own
   * geometry, distinct from the verdict marks they sit beside on the same row so a
   * check does not read as an Admit and a slashed circle does not read as a Deny.
   */
  act: Object.freeze({
    commit: {
      means: 'act.commit',
      source: 'a check inside a held box: closing what was open, distinct from the plain verdict check beside it',
      strokes: [{ d: 'M4.5 4.5h15v15h-15z' }, { d: 'M8 12.2l2.8 2.8 5.2-5.6' }],
    },
    cancel: {
      means: 'act.cancel',
      source: 'a slashed circle: the standard prohibition/withdraw silhouette, not the two-diagonal cross the deny verdict already owns',
      strokes: [{ d: 'M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16z' }, { d: 'M6.8 6.8l10.4 10.4' }],
    },
    undo: {
      means: 'act.undo',
      source: 'the same returning-arrow silhouette as effect.undo, because pressing this button and a row later reading as an undo are the same fact seen from before and after',
      strokes: [{ d: 'M5 10h10.5a4.5 4.5 0 0 1 0 9H10' }, { d: 'M9 6l-4 4 4 4' }],
    },
    escalate: {
      means: 'act.escalate',
      source: 'the same person silhouette as verdict.Escalate, because pressing this button and the engine answering Escalate are the same fact from either side of it',
      strokes: [{ d: 'M12 5.4a3.1 3.1 0 1 1 0 6.2 3.1 3.1 0 0 1 0-6.2z' }, { d: 'M5.6 19.6a6.4 6.4 0 0 1 12.8 0' }],
    },
  }),
  structure: Object.freeze({
    child: {
      means: 'structure.child',
      source: 'an elbow descending from a line above and turning into this one: an entry written under an earlier entry, never over it',
      strokes: [{ d: 'M7 4.5v9a4 4 0 0 0 4 4h6.5' }, { d: 'M14 14l3.5 3.5L14 21' }],
    },
    hole: {
      means: 'structure.hole',
      source: 'a dashed enclosure: a space that was declared and left empty, as against a space nobody mentioned',
      strokes: [{ d: 'M4.5 6.5h15v11h-15z', dasharray: '3 3' }],
    },
    'fold-shut': {
      means: 'structure.fold.shut',
      source: 'a wedge pointing along the reading direction: there is more here, and it is folded away',
      strokes: [{ d: 'M9.5 5.5 16 12l-6.5 6.5' }],
    },
    'fold-open': {
      means: 'structure.fold.open',
      source: 'the same wedge turned down: the fold is open and what it holds is below',
      strokes: [{ d: 'M5.5 9.5 12 16l6.5-6.5' }],
    },
    seal: {
      means: 'structure.seal',
      source: 'a closed ring with a bar across it: a thing shut in a way that shows whether it has been opened',
      strokes: [{ d: 'M12 4.5a7.5 7.5 0 0 1 0 15 7.5 7.5 0 0 1 0-15z' }, { d: 'M8 12h8' }],
    },
    unsealed: {
      means: 'structure.unsealed',
      source: 'the same ring with the bar left out and the circle left open at the top',
      strokes: [{ d: 'M16.9 7.1a7.5 7.5 0 1 0 2.4 6.4' }],
    },
    outside: {
      means: 'structure.outside',
      source: 'faces/graph (req/03 F-4): a line reaching the edge of a bounded box and stopping there, broken rather than continuing past it -- the box is what this window read, and the line is a declared edge whose other end is not inside it. Deliberately not an arrow (an arrow reads as "goes somewhere"; this mark has to read as "stops here, undrawn").',
      strokes: [{ d: 'M4.5 4.5h15v15h-15z' }, { d: 'M9 15l4-4' }, { d: 'M14.6 8.4l1.6 1.6' }],
    },
    subject: {
      means: 'structure.subject',
      source: 'faces/atlas (req/03 F-6): three squares stacked back to front, offset diagonally -- every touch this window read for one path, folded into the single summary line this mark sits on. Deliberately not structure.child\'s elbow (which marks one row descending from one specific earlier row, a "this one, after that one" fact): this mark says "many, folded", a fact about a count, not about a sequence.',
      strokes: [{ d: 'M4.5 10.5h9v9h-9z' }, { d: 'M7.5 7.5h9v9' }, { d: 'M10.5 4.5h9v9' }],
    },
  }),
});

/**
 * What is drawn when a name is asked for that the sheet does not hold. It is drawn,
 * and it is labelled -- the thing not to do is return nothing, because nothing looks
 * exactly like a mark that means "nothing" (req/04 G-3).
 */
export const UNDEFINED_MARK = Object.freeze({
  means: 'mark.undefined',
  source: 'a dashed square around a single dot: a name was asked for and this sheet does not hold it',
  strokes: [{ d: 'M4.5 4.5h15v15h-15z', dasharray: '2 3' }, { d: 'M12 12h0.01' }],
});

/**
 * Two marks may share one drawing only where the sharing is written down and given a
 * reason, because a shared drawing edited for one meaning silently edits the other
 * (req/04a C2, defect 2). The ledger is empty: in this sheet every meaning has its
 * own drawing. `meaningCollisions` holds it that way.
 */
export const SHARED_MEANINGS = Object.freeze([]);

export function symbolId(namespace, key) {
  return `gx-${namespace}-${String(key).replace(/[^a-z0-9-]/gi, '-')}`;
}

/** Resolve a mark. Unknown namespace raises; unknown key resolves to the drawn "undefined". */
export function markOf(namespace, key) {
  const table = MARKS[namespace];
  if (!table) throw new Error(`${GLYPH_MESSAGES.UNKNOWN_NAMESPACE}: ${namespace}`);
  const mark = table[key];
  if (!mark) return { namespace, key, defined: false, id: symbolId('undefined', 'mark'), ...UNDEFINED_MARK };
  return { namespace, key, defined: true, id: symbolId(namespace, key), ...mark };
}

/** Every mark that can be asked for by name. */
export function everyMark() {
  const out = [];
  for (const namespace of NAMESPACES) {
    for (const key of Object.keys(MARKS[namespace])) out.push(markOf(namespace, key));
  }
  return out;
}

/**
 * Everything the sprite has to carry, which is one more than can be asked for: the
 * drawing that stands in for a name the sheet does not hold has to ship, or the
 * answer to an unknown name would be an empty box after all. It is deliberately not
 * addressable -- there is no namespace it lives in, because nothing should be able to
 * ask for "undefined" on purpose.
 */
export function sheetMarks() {
  return [...everyMark(), { namespace: 'undefined', key: 'mark', defined: true, id: symbolId('undefined', 'mark'), ...UNDEFINED_MARK }];
}

/** Meanings carried by more than one drawing, with the declared sharings removed. */
export function meaningCollisions(marks = sheetMarks(), declared = SHARED_MEANINGS) {
  const byMeaning = new Map();
  for (const mark of marks) {
    if (!byMeaning.has(mark.means)) byMeaning.set(mark.means, []);
    byMeaning.get(mark.means).push(`${mark.namespace}/${mark.key}`);
  }
  const excused = new Set(declared.map((d) => d.means));
  return [...byMeaning.entries()]
    .filter(([means, names]) => names.length > 1 && !excused.has(means))
    .map(([means, names]) => ({ means, names }));
}

/**
 * How a stroke is drawn, written on every instance rather than once on the sprite.
 *
 * This was found by looking at a photograph, not by measuring anything. The first
 * build put these on the sprite root, which is correct for the sprite's own tree and
 * useless for a <use>: instanced content inherits down the tree the <use> sits in,
 * not down the tree the symbol was defined in. Every mark came out as a filled black
 * shape -- a ring became a disc, a dashed enclosure became a solid block -- while the
 * measurements said the right number of glyphs at the right sizes with no overlap.
 * The rectangles were never wrong. The picture was.
 */
export const STROKE = Object.freeze({
  fill: 'none',
  stroke: 'currentColor',
  'stroke-width': '2',
  'stroke-linecap': 'round',
  'stroke-linejoin': 'round',
});

function symbolNode(mark) {
  return el('symbol', { id: mark.id, viewBox: `0 0 ${BOX} ${BOX}` }, [
    el('desc', {}, [mark.source]),
    ...mark.strokes.map((stroke) => el('path', { d: stroke.d, 'stroke-dasharray': stroke.dasharray ?? null })),
  ]);
}

/**
 * The sprite. Injected once per document (G-5) and carrying the red rule as a node
 * that can be searched for and read aloud.
 */
export function sheet() {
  return el('svg', {
    id: SHEET_ID,
    'aria-hidden': 'true',
    ...STROKE,
    style: style({ position: 'absolute', width: '0', height: '0', overflow: 'hidden' }),
  }, [
    el('desc', { id: `${SHEET_ID}-rule` }, [RED_RULE]),
    ...sheetMarks().map(symbolNode),
  ]);
}

/**
 * Install the sprite once. The second call reports that it did nothing rather than
 * appending a second sheet, because two sprites holding the same ids is exactly the
 * double-drawing shape N-2 wore.
 */
export function installSheet(doc, render) {
  const existing = doc.getElementById(SHEET_ID);
  if (existing) return { installed: false, why: GLYPH_MESSAGES.SHEET_ALREADY, node: existing };
  const node = render(doc, sheet());
  doc.body.insertBefore(node, doc.body.firstChild);
  return { installed: true, why: GLYPH_MESSAGES.SHEET_INSTALLED, node };
}

function requireSize(size) {
  if (typeof size !== 'number' || !Number.isFinite(size) || size <= 0) {
    throw new Error(`${GLYPH_MESSAGES.SIZE_REQUIRED} (received ${JSON.stringify(size)})`);
  }
  return size;
}

/**
 * One glyph, at a stated size. The size is written into width/height attributes and
 * into inline width/height, so a document that loses every stylesheet still draws it
 * at the size the caller asked for.
 */
export function glyph(namespace, key, { size, label } = {}) {
  requireSize(size);
  const mark = markOf(namespace, key);
  const name = label ?? `${namespace} ${key}`;
  return el('svg', {
    width: size,
    height: size,
    viewBox: `0 0 ${BOX} ${BOX}`,
    ...STROKE,
    role: 'img',
    'aria-label': mark.defined ? name : `${GLYPH_MESSAGES.UNDEFINED_MARK}: ${namespace}/${key}`,
    'data-mark': mark.defined ? `${namespace}/${key}` : 'undefined',
    'data-means': mark.means,
    style: style({
      width: `${size}px`, height: `${size}px`, display: 'block', flex: 'none',
    }),
  }, [el('use', { href: `#${mark.id}` })]);
}
