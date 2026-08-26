// SPDX-License-Identifier: Apache-2.0
// The atlas face: what subjects this window has read, and what was last said about
// each -- one folded line per distinct path, oldest-to-newest history collapsed
// underneath it until a reader asks to see it.
//
// req/03 F-6 (SS2): "主面。stage col分割の主葉に置く「見る面」(drill-down起点)" -- the
// entry-point screen a reader consults before opening any one of the other five
// questions. Three facts follow directly and are held structurally below, the same
// discipline every other face in this tree already carries:
//
//   1. Every distinct path this window read becomes a subject line -- not only the
//      ones touched twice or more (that filter is faces/graph's own question, F-4).
//      A path touched exactly once still gets a line here; there is no undrawn
//      class for "touched once" on this screen the way there is on faces/graph's.
//   2. This screen draws no chain edges. `childOf`/`structure/child`/
//      `structure/outside` are faces/graph's vocabulary for a different question
//      (does touch B name touch A as its predecessor); atlas states a count and the
//      most recent touch's own facts, never a resolved link between two touches.
//   3. Every subject's grouping, ordering, and "most recent touch" decision is
//      computed once, in `toRecord()`/`buildAtlas()` (the attest step), and read as
//      already-decided data everywhere under `view()` (the render step) -- the same
//      split req/100 SS6 states for faces/receipt and req/101 SS6 names for MODO/
//      JIN's enabled/disabled decision: nothing under `view()` re-derives which
//      touch is "latest" by a second pass, and nothing mutates an already-frozen
//      subject object. Every subject and touch object this file builds is frozen at
//      construction; a subject's `latest`/`earliest` pointers are set once, in
//      `buildAtlas()`, never reassigned afterward.
//
// Owner eye-judgment corrections applied structurally here, not retrofitted onto
// the five earlier faces (that retrofit is a separate, later lane's scope):
//   - every subject's own touch history is a native <details> constructed CLOSED
//     unless it genuinely needs to start open (a hole, or a path/verdict word long
//     enough that its summary-row copy would be clipped with no full copy visible
//     elsewhere on the page while closed -- `needsOpen()` below, and see the two
//     genuine-negative-control-adjacent tests in test/atlas.test.mjs that prove
//     both directions).
//   - the screen's two disclosures ("why", "legend") are drawn as bordered,
//     compact controls sitting in one flex row together, each carrying a 2-3 word
//     plain-language hint next to its own label (`controlToggle()` below) --
//     not two full-width empty bands.
//   - a single compact header line states the face name and both denominators
//     before anything else on the screen.
//   - no face-switcher is drawn inside this face's own source; see declaration.mjs
//     UNDRAWN for why that is a structural non-goal here, not an oversight.
//
// Owner directive #340 -- "monotone, hard to grasp at a glance, hard to operate" --
// adds three shapes to that list and removes one sentence:
//   - the two denominators are no longer a clause in the header line. They are the
//     first two columns of a band of figures directly under it, beside this screen's
//     complete split by standing, each standing carrying its own hue and its own
//     mark. `ROWS.reports_denominator` is still discharged, in figures rather than
//     in prose, and a count this screen cannot know is drawn as a dash and never as
//     a zero.
//   - every subject is drawn inside its own bordered box (parts/src/surface.mjs
//     box()), whose head states the subject, how many changes were read for it, and
//     the standing of the most recent one. A group of records is an object on the
//     screen rather than rows separated by spacing alone -- and an empty or unread
//     population keeps its border and states `0` or a dash rather than vanishing.
//   - the last thing this screen draws is what this draw cost, measured here around
//     the tree build itself rather than estimated.
//   - the sentence that opened every subject's own history ("every touch this window
//     read for X, oldest to newest") is gone. The box head already names the subject
//     and counts it, and the order those changes are in is stated once, in the
//     legend, which is the same rule req/97's gap 3 applied to the four faces that
//     repeated a sentence per row.
//
// Owner directives #348 and #349 add four more, and each one is held by a machine
// rather than by this paragraph:
//   - every mark on this screen is drawn at the sheet's own readable floor, taken
//     from the sheet (binding.mjs `minReadable`) rather than retyped here.
//   - a right-click anywhere a value is drawn opens this screen's own menu, built by
//     mapping `declaration.mjs` OFFERS -- so the menu cannot offer something this
//     face has not declared, and a face that declares no ACTS says so in the menu
//     rather than letting the browser's page menu answer instead.
//   - the type carries an explicit weight in all three tiers (figure, label, body),
//     and prose breaks between words rather than inside them.
//   - the three builders that were one builder written three times (a heading, an
//     attendant line, an ink line) are one `typed()` call each, over one TYPE table.

import {
  DECLARATION, CONSUMES, READS, ORDER, ROWS, UNDRAWN, QUESTION, FACE_ID, ACTS, ACTS_REASON, OFFERS,
} from './declaration.mjs';
import { parts as defaultParts } from './binding.mjs';
import { order as orderRows } from '../../parts/src/row-order.mjs';

export const ATLAS_MESSAGES = {
  NO_HOST: 'a face is mounted into a host element, and none was given',
  NO_PORT: 'a face is mounted with the port it is to speak through, and none was given',
  UNDECLARED: 'this face may not call a method it did not declare',
  READING: 'reading what subjects this window has touched',
  // Owner #348 (4), the redundant-word half, and a leak this face had not noticed:
  // every other sentence on this screen calls one of these a "change" (`changeNoun`,
  // `READ_SOURCE`, the band's second column). This one said "transformations", which
  // is the wire's noun for the route -- a second vocabulary for one thing, on the one
  // screen state a reader only ever reaches when something has already gone wrong.
  LIST_UNREAD: 'the list of changes could not be read',
  // req/822_c7 (Owner #387/#388 冗長文字全掃): this used to read "this member was
  // looked for and was not there: <member>", built with the member's own name
  // stitched onto the end of it in normalizeTouch() below. Drawn as a note line
  // (touchNoteLines()), that put the member's name on the screen twice -- once as
  // the row label beside the sentence, once again inside the sentence itself. The
  // member is still the row label; the sentence carries no second copy of it.
  MEMBER_ABSENT: 'not in this record',
  MEMBER_NOT_SCALAR: 'this member arrived as a structure this face does not read',
  NOTE_SUMMARY: 'what this touch holds in full, and what is missing from it',
  NO_SUBJECTS: 'nothing has been read yet',
  // The same panel, for the state that is NOT that one. Measured against a real gx
  // 0.1.0 bed holding three Admitted rows: every row came back with `scope: null`, so
  // no row carried a path, so no subject could be formed -- and this panel drew
  // "nothing has been read yet" two hundred pixels under a figure reading "3 CHANGES".
  // Three changes were read. What failed was the filing, not the reading, and the face
  // already knew which: `notDrawn.unidentifiable` has held the count and the reason per
  // row since this file was written, and nothing drew it on the one screen state where
  // it is the whole story. "Asked and empty" and "asked, answered, and unfilable" are
  // opposite facts about an engine and this face was drawing them with one sentence.
  SUBJECTS_UNFILED: (touches, why) =>
    `${touches} ${touches === 1 ? 'change was' : 'changes were'} read and none of them could be filed under a subject (${why}), so the three standings beside the figure above count none of them`,
  UNFILED_WHY: 'no usable path',
  MENU: 'what can be taken from what is under the pointer',
  COPIED: 'copied',
  COPY_FAILED: 'this window would not let that be copied',
};

const ANSWERED = 'answered';

/** The five members this face reads off each transformation item, the same member
 * names every other reading face in this tree reads off its own list items. */
const TOUCH_MEMBERS = Object.freeze([
  { key: 'at', member: 'at' },
  { key: 'actor', member: 'actor' },
  { key: 'effect', member: 'effect' },
  { key: 'verdict', member: 'verdict' },
  { key: 'path', member: 'path' },
]);

/**
 * The standings this screen counts its own population by, in the order a reader
 * meets them. The engine's own word is the key and the meaning behind it is
 * whatever the shared sheet says it is, so the counting below is a table read on
 * both sides -- this file never compares a record against a word it spells, and a
 * word the sheet does not hold falls into no column instead of being guessed into
 * one (see `standingCounts()`).
 *
 * The word is also the label under the figure, unchanged. Two reasons, and the
 * second was read off a photograph: parts/src/verdict-badge.mjs's own rule is that
 * the frozen words are printed as they arrive and no friendlier synonym is invented
 * for them, so a band that said "admitted" over a screen whose pills say "Admit"
 * would be a second vocabulary for one fact -- and "ESCALATED" is nine characters,
 * which is one more than a fifth of a 720px band can draw, so the first version of
 * this label reached the screen cut to "ESCALAT...".
 */
const STANDINGS = Object.freeze(['Admit', 'Deny', 'Escalate']);

/**
 * What one touch is called on this screen, in one place, and in the number it
 * actually is. `1 changes` was on the screen until the shot was looked at.
 */
const changeNoun = (count) => (count === 1 ? 'change' : 'changes');

/** What this face read, in its own words, for the strip at the foot of the screen. */
const READ_SOURCE = 'the list of changes';

const isScalar = (value) => typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean';

/** A caller that cannot reach a method the declaration does not hold. */
export function callerFor(port, allowed = CONSUMES) {
  const allow = new Set(allowed);
  const guard = (name) => {
    if (!allow.has(name)) throw new Error(`${ATLAS_MESSAGES.UNDECLARED}: ${name}`);
  };
  return {
    async fold(name, input) {
      guard(name);
      return port.fold(name, input);
    },
    async invoke(name, input) {
      guard(name);
      const method = port[name];
      if (typeof method !== 'function') return { outcome: 'absent', reason: 'no_such_method', requested: { name } };
      return method(input);
    },
  };
}

/**
 * One touch, read verbatim. Every member that is absent or arrives as something
 * other than a scalar becomes a named hole rather than a silent omission --
 * glovrex/req/405 SS5 conditions 1/2, the same conformance discipline every other
 * face's toRecord() already holds.
 */
function normalizeTouch(item, index) {
  const holes = {};
  const cells = {};
  for (const { key, member } of TOUCH_MEMBERS) {
    const value = item ? item[member] : undefined;
    if (value === undefined || value === null || value === '') holes[key] = ATLAS_MESSAGES.MEMBER_ABSENT;
    else if (!isScalar(value)) holes[key] = `${ATLAS_MESSAGES.MEMBER_NOT_SCALAR}: ${member}`;
    else cells[key] = String(value);
  }

  let id = null;
  if (typeof item?.id === 'string' && item.id !== '') id = item.id;
  else holes.id = ATLAS_MESSAGES.MEMBER_ABSENT;

  let digest = null;
  if (isScalar(item?.digest) && String(item.digest) !== '') digest = String(item.digest);
  else holes.digest = ATLAS_MESSAGES.MEMBER_ABSENT;

  const n = Number.isInteger(item?.sequence) ? item.sequence : undefined;

  return Object.freeze({
    // req/97 gap-list item 3: the sentence that was the same on every open touch is
    // stated once, in the legend, and is not carried on the record any more.
    index, id, n, digest, ...cells, lifecycle: 'settled', holes: Object.freeze(holes),
  });
}

/**
 * Attest. Reads the one list envelope verbatim and groups the touches it names by
 * `path`. No subject is filtered out here for having only one touch -- that
 * filter belongs to faces/graph's different question, not this one.
 */
export function toRecord({ transformations } = {}) {
  const listRead = transformations?.outcome === ANSWERED;
  const items = listRead && Array.isArray(transformations.items) ? transformations.items : [];

  if (!listRead) {
    return Object.freeze({
      listOutcome: transformations?.outcome ?? 'absent',
      rawTransformations: transformations ?? null,
      touches: Object.freeze([]),
      subjects: Object.freeze([]),
      distinctPaths: null,
      totalTouches: null,
      notDrawn: Object.freeze({ unidentifiable: Object.freeze({ count: null, entries: Object.freeze([]) }) }),
      holes: Object.freeze({ list: `${ATLAS_MESSAGES.LIST_UNREAD}: ${transformations?.outcome ?? 'absent'}` }),
    });
  }

  const touches = items.map((item, index) => normalizeTouch(item, index));
  return buildAtlas(touches, transformations);
}

/**
 * Decide (face-local, the same kind of helper faces/graph's buildGraph() and
 * faces/receipt's digestAgreement() already are). Groups the read touches by
 * path, orders each group by sequence (parts/src/row-order.mjs), and records the
 * earliest/latest touch this window read for that path. A touch with no usable
 * path or identity is never silently folded into a group -- it is named and
 * counted instead (C-3).
 */
function buildAtlas(touches, transformations) {
  const byPath = new Map();
  const unidentifiable = [];
  for (const touch of touches) {
    if (!touch.path) { unidentifiable.push({ index: touch.index, id: touch.id, why: 'no usable path' }); continue; }
    if (!byPath.has(touch.path)) byPath.set(touch.path, []);
    byPath.get(touch.path).push(touch);
  }

  const subjects = [...byPath.entries()].map(([path, groupTouches]) => {
    const ordered = orderRows(groupTouches, { by: 'by-sequence' });
    for (const drop of ordered.dropped) unidentifiable.push({ path, index: drop.index, why: drop.why });
    const rows = ordered.rows;
    return Object.freeze({
      path,
      touchCount: groupTouches.length,
      rows: Object.freeze(rows),
      latest: rows[rows.length - 1] ?? null,
      earliest: rows[0] ?? null,
      orderRequested: ordered.requested,
      orderApplied: ordered.by,
      orderSubstituted: ordered.substituted,
      orderReason: ordered.reason,
    });
  }).sort((a, b) => (b.touchCount - a.touchCount) || a.path.localeCompare(b.path));

  return Object.freeze({
    listOutcome: transformations.outcome,
    rawTransformations: transformations,
    touches: Object.freeze(touches),
    subjects: Object.freeze(subjects),
    distinctPaths: byPath.size,
    totalTouches: touches.length,
    notDrawn: Object.freeze({ unidentifiable: Object.freeze({ count: unidentifiable.length, entries: Object.freeze(unidentifiable) }) }),
    holes: Object.freeze({}),
  });
}

export function createFace({ parts = defaultParts } = {}) {
  const P = parts;
  const { el, style } = P.element;
  const T = P.tokens;

  /**
   * Owner #348 (3). Every mark on this screen is drawn at this number, and this
   * number is the sheet's own declared floor rather than a literal typed here.
   *
   * The eleven call sites in this file all said `size: 14`. A 24-unit design with a
   * 2-unit stroke rendered into 14 pixels puts that stroke at 1.17 device pixels and
   * the gap between two strokes under one, which does not read as a small mark -- it
   * reads as a broken one, which is what the Owner saw. parts/tools/glyph-bounds.mjs
   * had already settled that the drawings themselves fit their box, so the scale was
   * the only remaining candidate and raising it is the whole fix.
   *
   * There is no second number here. `minAct` is the floor for a mark sitting on a
   * control that sends something, and this face sends nothing (declaration.mjs ACTS),
   * so nothing on this screen is entitled to it -- the fold marks sit on disclosures,
   * which is what the shell's own tab marks are and what it draws at `minReadable`.
   */
  const MARK = P.minReadable;

  // -- small pieces of type ---------------------------------------------------

  /**
   * Owner #348 (3), the hierarchy half. Three weights, and which one a piece of text
   * takes is decided by what the text IS rather than by whichever number the line was
   * written with.
   *
   * A number is the thing an eye lands on and is bold; the word naming one is medium,
   * so it reads as support without becoming a second heading; body prose is regular.
   * Before this the file had `font-weight: '600'` written at two call sites, `'700'`
   * at one, and nothing at all at the other eleven -- so most of this screen's weight
   * was whatever the document happened to inherit, which is the "incidental" this atom
   * replaces. tools/gate.mjs `weights-come-from-the-scale` now refuses a weight this
   * table does not hold.
   *
   * There is no `figure` tier here and its absence is deliberate: every number on this
   * screen is drawn by the shared band and the shared box head, which carry
   * `.gx-figure` (600, mono) from parts/src/surface.mjs. A fourth entry in this table
   * would be a second declaration of one thing, which is what the whole atom is about.
   * tools/shoot.mjs reads the weights that actually reached the page: 400, 500 and 700
   * from here, 600 from there.
   */
  const TYPE = Object.freeze({
    head: { weight: '700', family: T.sans },
    label: { weight: '500', family: T.sans },
    body: { weight: '400', family: T.sans },
  });

  /**
   * Owner #348 (3), the breaking half.
   *
   * `overflow-wrap: anywhere` was on five of the text styles in this file. It is an
   * instruction to break inside a word at any letter the moment a line is tight,
   * which is the mid-word break this atom forbids, written into the source as the
   * default for prose. `break-word` breaks inside a word only when the word cannot
   * fit its column at all -- the one case where the alternative is a word drawn over
   * the column beside it -- and `text-wrap: pretty` is what stops the last line of a
   * wrapped sentence being a single stranded character. Both are measured rather than
   * claimed: tools/shoot.mjs now reads every drawn line's real box for a break with a
   * letter on both sides of it, and for a last line of one character.
   *
   * It is deliberately not applied to the two cells that hold a machine value rather
   * than a sentence (a path, a digest). Those have no spaces to break at, so `anywhere`
   * is the only thing standing between them and a horizontal scrollbar -- and a path
   * broken across two lines is still the whole path, where a sentence broken inside a
   * word is a word nobody wrote.
   */
  const WRAP = Object.freeze({ 'overflow-wrap': 'break-word', 'text-wrap': 'pretty' });

  /** A machine value that has no spaces in it, and so has to be allowed to break. */
  const WRAP_VALUE = Object.freeze({ 'overflow-wrap': 'anywhere' });

  /**
   * Owner #349 (3). One builder, called four ways, instead of the four
   * near-identical `el(tag, { style: style({ ...the same six declarations... }) })`
   * blocks this file opened with. Every one of them stated family, size and line
   * height again; three of them disagreed about weight by omission.
   */
  const typed = (tag, role, attrs, extra, children) => el(tag, {
    'data-type': role,
    ...attrs,
    style: style({
      'font-family': TYPE[role].family,
      'font-size': T.record,
      'line-height': T.recordLine,
      'font-weight': TYPE[role].weight,
      ...extra,
    }),
  }, children);

  // There is no `heading()` here any more, and its absence is the point rather than
  // an omission: both of the two <h2>s this file drew were the name of the control
  // they were drawn inside, repeated (Owner #348 (4)). A builder with no callers is a
  // builder that invites the next one.

  const aside = (words, role = 'aside') => typed('p', 'body', { 'data-role': role }, {
    margin: '0 0 6px', color: T.attendant, ...WRAP,
  }, [words]);

  const plain = (words, role = 'line') => typed('p', 'body', { 'data-role': role }, {
    margin: '0 0 4px', color: T.ink, ...WRAP,
  }, [words]);

  /** The word that names a value. Medium, attendant, never a second heading. */
  const label = (words, extra = {}) => typed('span', 'label', {}, { color: T.attendant, ...extra }, [words]);

  const section = (name, state, children, extraAttrs = {}) => el('section', {
    'data-section': name,
    'data-state': state,
    ...extraAttrs,
    style: style({ padding: '14px 0', 'border-top': `1px solid ${T.rule}`, background: T.page }),
  }, children.filter(Boolean));

  /**
   * A name and what it names, in two columns.
   *
   * One builder for what were two: this shape and the omission row in
   * `notDrawnSection()` were the same grid written twice with a different first
   * column width, so the width is an argument now and there is one of them.
   */
  const kvLine = (name, value, { nameWidth = '14rem', wrap = WRAP, attrs = {} } = {}) => el('div', {
    'data-role': 'kv-line',
    ...attrs,
    style: style({ display: 'grid', 'grid-template-columns': `minmax(0,${nameWidth}) minmax(0,1fr)`, gap: '10px', padding: '3px 0' }),
  }, [
    label(name),
    typed('span', 'body', {}, { color: T.ink, ...wrap }, [value]),
  ]);

  // -- header: face name + what this screen answers, one compact line ----------

  function headerLine(record) {
    const known = record.listOutcome === ANSWERED;
    return el('div', {
      'data-role': 'face-header',
      // Wrapping, because the third span only exists on an unread screen: without it
      // the unread header is wider than a narrow window and the page grows a
      // horizontal scrollbar for one clause.
      style: style({ display: 'flex', 'flex-wrap': 'wrap', 'align-items': 'baseline', gap: '10px', padding: '10px 0 6px', 'font-family': T.sans }),
    }, [
      // req/822_c7 (Owner #387/#388 冗長文字全掃): the declared question used to be
      // drawn as its own always-visible span beside the face name -- a full sentence
      // on the one line every screen state shares, ahead of the stat band that is
      // this screen's real head now (Owner #340). It is still verbatim and still
      // sourced from one place (declaration.mjs QUESTION, never respelled here); it
      // now rides the face name's own title (a hover) and a `data-question`
      // attribute, so a reader who wants it still reaches it and one who does not is
      // not shown it by default.
      typed('span', 'head', { title: QUESTION, 'data-question': QUESTION }, { 'font-size': T.head, 'line-height': T.headLine, color: T.ink }, ['atlas']),
      // "not yet read" was a word wrong as well as a word too many: `yet` promises a
      // later reading, and two of the three states that reach this branch (refused,
      // no such method) are not waiting for anything.
      known ? null : typed('span', 'label', { 'data-role': 'unread' }, { color: T.ink }, ['not read']),
    ].filter(Boolean));
  }

  // -- the figures: how big this screen's population is, and its whole shape ---

  const NO_STANDING = 'the most recent change on these carried no verdict this screen knows, so they are counted under none of the three standings beside this figure';

  /**
   * Every subject placed under the standing of its own most recent change, and the
   * ones that could not be placed counted rather than dropped.
   *
   * The three counts and `unplaced` always sum to the number of subjects -- that is
   * the property that makes this band readable as a whole rather than as three
   * unrelated numbers, and test/atlas.test.mjs holds it on a population built to
   * contain one of each, including a word the sheet does not hold.
   */
  function standingCounts(record) {
    const counted = new Map(STANDINGS.map((standing) => [standing, 0]));
    const placement = new Map(STANDINGS.map((standing) => [P.markOf('verdict', standing).means, standing]));
    let unplaced = 0;
    for (const subject of record.subjects) {
      const where = placement.get(P.markOf('verdict', subject.latest?.verdict).means);
      if (where === undefined) { unplaced += 1; continue; }
      counted.set(where, counted.get(where) + 1);
    }
    return { counted, unplaced };
  }

  function figures(record) {
    const known = record.listOutcome === ANSWERED;
    const { counted, unplaced } = standingCounts(record);
    return P.statBand([
      {
        noun: 'subjects',
        count: known ? record.subjects.length : null,
        // Drawn only when it has something to say. A subject whose latest change
        // carries no standing is in none of the three columns to the right, so the
        // three would silently fail to add up to this figure; the mark says so on
        // the figure itself, and the subject's own row is forced open underneath.
        mark: unplaced > 0 ? P.glyph('structure', 'hole', { size: MARK, label: NO_STANDING }) : null,
        said: unplaced > 0 ? `${unplaced} of these: ${NO_STANDING}` : null,
      },
      {
        noun: changeNoun(known ? record.totalTouches : null),
        count: known ? record.totalTouches : null,
        said: 'every change this window read, including the ones it could not file under any subject',
      },
      ...STANDINGS.map((standing) => ({
        noun: standing,
        count: known ? counted.get(standing) : null,
        tone: P.inkFor(P.markOf('verdict', standing)),
        mark: P.glyph('verdict', standing, { size: MARK, label: standing }),
        said: `subjects whose most recent change the engine answered with its own word ${standing}`,
      })),
    ]);
  }

  // -- controls: bordered, compact, side by side, self-evident label + hint ---

  function controlToggle(name, hint, body, { open = false } = {}) {
    return el('details', {
      'data-role': 'control', 'data-control': name, 'data-open': String(Boolean(open)), open: open || null,
      // Owner #349 (1): the corner comes from the scale, by what this thing IS. A
      // control takes tier 2. The `4px` that was written here was tier 1's number,
      // picked by eye before the scale existed, and it was right by accident on one
      // page and wrong the moment anybody moved a tier -- tools/gate.mjs
      // `no-raw-corner` now refuses a literal here at all.
      style: style({ border: `1px solid ${T.rule}`, 'border-radius': T.radiusControl, background: T.page }),
    }, [
      el('summary', {
        // Owner #348 (4). A bare word is not a label, and a hint has to earn the
        // room it takes: `omitted -- what is not drawn` and `legend -- symbols
        // used` were each the label said a second time in more letters.
        // test/atlas.test.mjs still refuses a hint that shares a word with the
        // name it sits beside, which is the machine form of "every word earns its
        // place".
        //
        // req/822_c7 (Owner #387/#388 冗長文字全掃) goes further: the hint used to
        // be drawn as its own visible span next to the name, which is the
        // "control word + its own explainer, both always on" shape this round
        // removes everywhere on this screen. The word `name` stays the
        // default-visible surface; `hint` rides the summary's own title (a hover)
        // and a `data-hint` attribute now, so the refused-shared-word property
        // above is held of the control, not of a rendered span.
        title: hint, 'data-hint': hint,
        // No cursor is declared here either, for the reason summaryRow() states: the
        // shared rule set draws a pointer over a summary, and an inline cursor on the
        // control itself is the one thing that can outrank it.
        style: style({
          display: 'flex', 'align-items': 'center', gap: '6px', 'min-height': T.pitch, 'box-sizing': 'border-box',
          padding: `0 ${T.padX}`, color: T.ink, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'list-style': 'none',
        }),
      }, [
        P.glyph('structure', open ? 'fold-open' : 'fold-shut', { size: MARK, label: open ? 'open' : 'closed' }),
        typed('span', 'label', { 'data-role': 'control-name' }, { color: T.ink }, [name]),
      ]),
      el('div', { style: style({ padding: `0 ${T.padX} 10px` }) }, [body]),
    ]);
  }

  function controlsRow(children) {
    return el('div', {
      'data-role': 'control-row',
      style: style({ display: 'flex', gap: '8px', 'flex-wrap': 'wrap', padding: '0 0 10px' }),
    }, children);
  }

  function legendBody() {
    return el('div', { 'data-role': 'legend' }, [
      // Owner #348 (4). Every name in this column used to open with "the ... shown"
      // or repeat "on a subject line" -- five of the eight said where the thing is,
      // which is what the reader is looking at while they read the legend.
      kvLine('a subject', 'every distinct thing this screen read about, once each -- including something touched only once. This is a count of touches, not a chain between them.'),
      kvLine('the fold mark', 'shut: this subject\'s own history is folded away below it. Open: it is drawn, because one of its values needed the room to be shown whole.'),
      kvLine('the verdict and effect', 'the most recent touch this window read for that subject -- not every touch\'s verdict, only the latest one\'s.'),
      kvLine('the gap mark', 'something this screen looked for on the most recent change and did not find. A stated gap, never a blank.'),
      kvLine('the time', 'date and hour only (e.g. "2026-08-24T09") -- a fixed, complete substring, not a clipped one. The full timestamp is in this cell\'s own title attribute and in the touch\'s own note once the subject is opened. The per-touch rows underneath a subject use the shared row grid, whose narrower time column declares its own shorter cut (the time of day); both are declared forms, and neither is a value shown cut off.'),
      kvLine('an opened subject', ATLAS_MESSAGES.NOTE_SUMMARY),
      kvLine('subject order', ROWS.groups_order_reason_plain),
      kvLine('order inside a subject', ROWS.order_reason_plain),
      kvLine('a right-click', `${ACTS_REASON} ${OFFERS.map((offer) => offer.label).join(', ')} is what a menu here can do, on anything drawn as a value.`),
      internalNote(`${ROWS.groups_order_reason} | ${ROWS.order_reason}`),
    ]);
  }

  // -- subject summary row: bespoke grid, not receipt-row's 8-column grid -----

  // Owner #340: the subject and its own standing moved out of this row and into the
  // head of the box the row now sits in, so both budgets below now describe the box
  // head rather than a column of this grid. Neither is a data-loss guard: the head's
  // name cell ends in an ellipsis and the badge's word span clamps at 4.5rem, so what
  // these two numbers buy is that a subject whose name or whose standing cannot be
  // drawn whole up there is opened, and the whole of it is drawn underneath (N-4).
  const PATH_COLUMN_REM = 14;
  const PATH_BUDGET = Math.floor(PATH_COLUMN_REM * 2); // same Math.floor(rem*2) convention req/100 SS1's budgetFor() states for a flexible column.
  const VERDICT_BUDGET = 9; // parts/src/verdict-badge.mjs's own word span clamps to max-width:4.5rem -- the real, measured clip boundary, and the box head draws that badge directly.

  // The same date+hour truncation req/100 SS1 already states for the shared
  // receipt-row.mjs `at` column ("an ISO-8601 timestamp truncated to date+hour"),
  // applied here to atlas's own bespoke `at` cell for the identical reason: a
  // fixed, complete, non-overflowing substring rather than a CSS-ellipsis clip of
  // the full value -- real-renderer measurement (tools/shoot.mjs's
  // `clippedWithoutFull` reading) caught the full-ISO-string version of this cell
  // being clipped with no full copy visible on a closed (by design) subject line,
  // which a bare character-count budget on a mono-font flexible column did not
  // predict. The full value is still carried in the cell's own `title` and in the
  // per-touch note once a subject's detail is opened.
  function dateHourOf(at) {
    const value = String(at ?? '');
    return value.length > 13 ? `${value.slice(0, 13)}...` : value;
  }

  function needsOpen(subject) {
    const latest = subject.latest;
    if (!latest) return true;
    const holeKeys = Object.keys(latest.holes ?? {});
    if (holeKeys.length > 0) return true;
    if (subject.path.length > PATH_BUDGET) return true;
    if (typeof latest.verdict === 'string' && latest.verdict.length > VERDICT_BUDGET) return true;
    return false;
  }

  function effectMark(value) {
    if (value === 'write' || value === 'delete') return P.glyph('effect', value, { size: MARK, label: value });
    return null;
  }

  const NO_IDENTIFIABLE_TOUCH = 'no touch for this path could be identified (no usable identity), so there is no latest touch to state';

  /**
   * The standing that stands over a whole subject: the one its most recent change
   * was answered with. It is the box head's pill, which is where Owner #340 puts a
   * group's own condition -- drawn through the shared badge so it takes the filled
   * bed every standing on these screens now takes, and drawn as a stated hole when
   * this screen has no verdict to show rather than as an empty corner.
   */
  function standingPill(subject) {
    const latest = subject.latest;
    if (!latest) return P.glyph('structure', 'hole', { size: MARK, label: NO_IDENTIFIABLE_TOUCH });
    const why = latest.holes?.verdict;
    if (why) return P.glyph('structure', 'hole', { size: MARK, label: why });
    return P.badge(latest.verdict, { size: MARK });
  }

  /**
   * The disclosure line under a box head. It carries what the head does not: the
   * kind of the most recent change and when it was, and the fold mark for the whole
   * history underneath. The subject, its count and its standing are in the head
   * above it and are deliberately not repeated here -- a row that restates its own
   * container is the density the reference tool beats us on.
   *
   * No `cursor` is declared. This is a real control, and the rule set in
   * parts/src/surface.mjs draws a pointer over every `summary` on these screens; the
   * `cursor:default` this row used to carry was an inline style, which outranks that
   * rule, so the one line that was supposed to say "this can be pressed" said the
   * opposite on the one element it mattered most on.
   */
  /** A cell drawn as a stated gap: the mark, and the sentence saying what is missing
   * carried where a menu can read it back out. */
  const holeCell = (key, why) => el('span', {
    'data-cell': key, 'data-state': 'hole', title: why,
    style: style({ display: 'flex' }),
  }, [P.glyph('structure', 'hole', { size: MARK, label: why })]);

  /**
   * A cell drawn as a value.
   *
   * `data-menu-value` is what a right-click takes, and it is the FULL value rather
   * than the drawn one: the time cell draws a declared cut (`dateHourOf`) and a
   * reader who asks for it wants the timestamp, not the eleven characters of it that
   * fitted. That is the one thing a menu can do here that no amount of column width
   * can (declaration.mjs OFFERS).
   */
  const valueCell = (key, drawn, full, extra = {}, children = null) => el('span', {
    'data-cell': key, 'data-state': 'value', 'data-menu-value': String(full ?? drawn ?? ''), title: String(full ?? ''),
    style: style({ display: 'flex', 'align-items': 'center', gap: '6px', 'min-width': '0', overflow: 'hidden', ...extra }),
  }, children ?? [el('span', { style: style({ overflow: 'hidden', 'text-overflow': 'ellipsis', 'white-space': 'nowrap' }) }, [String(drawn ?? '')])]);

  function summaryRow(subject, open) {
    const latest = subject.latest;
    // One question asked three times, asked once: is this member a stated gap on the
    // most recent change, or is the whole change missing. Written as three ternaries
    // it also put `.actor :` on a line, which the `no-actor-named` rule reads as this
    // face naming who is acting -- the rule is right to be that blunt and the code
    // was the thing to change.
    const gapFor = (key) => (latest ? latest.holes?.[key] : NO_IDENTIFIABLE_TOUCH);
    const effectHole = gapFor('effect');
    const actorHole = gapFor('actor');
    const atHole = gapFor('at');
    return el('summary', {
      'data-role': 'subject-summary',
      // A right-click that lands on the line rather than on one of its cells still
      // has something to take: the subject this line belongs to.
      'data-menu-value': subject.path,
      style: style({
        display: 'grid',
        // Owner #348 (3) raised the two mark columns from 14 to 16, and the r4 report
        // named the rest of this row as its own worst shape: three cells packed into
        // the left half and a flexible fourth column with a short value in it, so
        // every fold line on the screen was blank from about 40% across. The actor is
        // read off every touch, was drawn nowhere until a subject was opened, and is
        // the fact a reader of a change list asks for after "what" and before "when".
        // It takes the flexible column; the time is pushed to the right edge, where a
        // column of timestamps lines up down the screen instead of floating.
        'grid-template-columns': `${MARK}px ${MARK}px minmax(0,6.5rem) minmax(0,1fr) auto`,
        'align-items': 'center', gap: '10px', 'min-height': T.pitch, 'box-sizing': 'border-box',
        padding: `0 ${T.padX}`,
        'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'font-weight': TYPE.body.weight,
        color: T.ink, 'list-style': 'none', overflow: 'hidden',
      }),
    }, [
      P.glyph('structure', open ? 'fold-open' : 'fold-shut', { size: MARK, label: open ? 'open' : 'closed' }),
      P.glyph('structure', 'subject', { size: MARK, label: 'every change this window read for this subject, folded into one line' }),
      effectHole
        ? holeCell('effect', effectHole)
        : valueCell('effect', latest.effect, latest.effect, {}, [effectMark(latest.effect), el('span', {}, [latest.effect ?? ''])]),
      actorHole
        ? holeCell('actor', actorHole)
        : valueCell('actor', latest.actor, latest.actor, { color: T.attendant }),
      atHole
        ? holeCell('at', atHole)
        : valueCell('at', dateHourOf(latest.at), latest.at, {
          'font-family': T.mono, 'font-size': T.time, color: T.attendant, 'white-space': 'nowrap', 'justify-content': 'flex-end',
        }),
    ]);
  }

  // -- per-touch detail: real delta rows, receipt-row.mjs reused (meaning matches) --

  const CHARACTER_PX = 7;
  const touchBudgetFor = (column) => {
    const fixed = /^(\d+)px$/.exec(column.width);
    if (fixed) return Math.floor(Number(fixed[1]) / CHARACTER_PX);
    const rem = /minmax\(0,\s*([\d.]+)rem\)/.exec(column.width);
    if (rem) return Math.floor(Number(rem[1]) * 2);
    return 40;
  };
  const TOUCH_BUDGETS = Object.fromEntries(P.columns.map((column) => [column.key, touchBudgetFor(column)]));

  function touchNoteLines(record) {
    const lines = Object.entries(record.holes ?? {}).map(([key, why]) => ({ name: key, value: why }));
    for (const { key } of TOUCH_MEMBERS) {
      if (record[key] !== undefined) lines.push({ name: `${key} in full`, value: record[key] });
    }
    if (record.n !== undefined) lines.push({ name: 'sequence', value: String(record.n) });
    return lines;
  }

  function touchNeedsOpen(record) {
    const holeKeys = Object.keys(record.holes ?? {});
    if (holeKeys.length > 0) return true;
    // Against the drawn form, not the arrived value (req/97 gap-list item gap 1): the shared
    // grid's `at` column draws a declared cut, so an ISO-8601 timestamp is not a
    // clip in a per-touch row either.
    return TOUCH_MEMBERS.some(({ key }) => P.drawnTextFor(key, record[key]).length > (TOUCH_BUDGETS[key] ?? 40));
  }

  /**
   * A subject's own history, oldest to newest.
   *
   * The sentence that used to open this block ("every touch this window read for
   * <path>, oldest to newest") is not here any more. It named the subject the box
   * head names two lines above it, it was the same sentence on every subject on the
   * screen, and the order it stated is stated once in the legend -- which is exactly
   * the shape req/97's gap 3 removed from the four faces that repeated a sentence on
   * every row. `orderSubstituted` still speaks, because that one is not the same
   * sentence twice: it fires only when this subject's own asked-for order could not
   * be applied, and it says which order was used instead.
   */
  function subjectDetail(subject) {
    return el('div', { 'data-role': 'subject-detail' }, [
      subject.orderSubstituted ? aside(subject.orderReason, 'order-substituted') : null,
      ...subject.rows.map((touch) => P.receiptRow(touch, { note: touchNoteLines(touch), open: touchNeedsOpen(touch) })),
    ].filter(Boolean));
  }

  /**
   * One subject, as an object on the screen: a bordered box whose head states what
   * was touched, how many changes this window read for it, and the standing of the
   * most recent one, holding the fold that opens the whole history underneath.
   *
   * Owner #340's Box idiom, and the cost of it is stated rather than hidden: a shut
   * subject is two lines now (a 30px head and a 36px disclosure) where it used to be
   * one 36px row, so this screen fits fewer subjects in a window than it did. What it
   * buys is that the boundary between one subject and the next is a drawn edge rather
   * than a gap the reader has to infer, and that each group's own count and standing
   * are read off the group itself.
   */
  function subjectBox(subject) {
    const open = needsOpen(subject);
    return P.box({
      name: subject.path,
      count: subject.touchCount,
      noun: changeNoun(subject.touchCount),
      pill: standingPill(subject),
      children: el('details', {
        'data-role': 'subject', 'data-path': subject.path, 'data-touch-count': String(subject.touchCount), 'data-open': String(Boolean(open)), open: open || null,
        style: style({ margin: '0' }),
      }, [
        summaryRow(subject, open),
        subjectDetail(subject),
      ]),
    });
  }

  // -- claims: parts/src/checkable.mjs's two population-wide claims -----------

  function claimsSection(record) {
    // Owner #348 (4): no heading here and none in `notDrawnSection` either. Both of
    // these are drawn inside a control whose own name is already on the screen a line
    // above -- `claims -- what you can check` sat directly over an <h2> reading "what
    // you can check here yourself", which is one fact wearing two hats and 32
    // characters of the second one.
    if (record.subjects.length === 0) {
      return section('claims', 'empty', [
        aside('there is no subject population yet, so there is nothing to check.', 'claims-empty'),
      ]);
    }
    const identifiable = record.touches.filter((t) => t.id);
    const claims = P.checkable(identifiable, []);
    const wanted = new Set(['serial-can-be-cut', 'identities-appear-once']);
    const shown = claims.filter((c) => wanted.has(c.id));
    return section('claims', 'stated', [
      // req/97 gap-list item 4: the surface says this in words a first viewer can
      // read. The version naming this codebase's own files and the three claims it
      // does not compute is one press away, under "internal reference".
      aside('both of these hold across every change this screen read, whichever subject it was filed under. Three more things could be checked about a single subject\'s own order and chain; those belong to the screens that answer that question, and are not claimed here.', 'claims-why'),
      internalNote('computed by parts/src/checkable.mjs over the identifiable touches only. The three not shown (sequence-appears-once, gaps-are-the-withheld-records, prev-names-the-record-before) each assume one subject\'s own issuance order or chain, which is faces/graph\'s and faces/receipt\'s question, not this face\'s.'),
      el('div', { 'data-role': 'checkable-claims' }, shown.map((c) => el('div', {
        'data-claim-id': c.id, 'data-holds': String(c.holds),
        style: style({
          display: 'grid', 'grid-template-columns': 'minmax(0,10rem) minmax(0,1fr)', gap: '10px', padding: '4px 0',
          'border-bottom': `1px solid ${T.rule}`, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
        }),
      }, [
        typed('span', 'label', {}, { color: c.holds ? T.ink : T.deny }, [c.holds ? 'holds' : 'does not hold']),
        el('div', {}, [
          typed('div', 'body', {}, { color: T.ink, ...WRAP }, [c.claim]),
          typed('div', 'body', {}, { color: T.attendant, ...WRAP }, [c.detail]),
        ]),
      ]))),
    ]);
  }

  // -- not drawn ----------------------------------------------------------------

  /**
   * req/97 gap-list item 4. The accurate internal account of a decision -- which file
   * decided it, which requirement names it, which route it concerns -- is worth
   * keeping and is not worth putting on a product surface, where it reads as
   * undecodable to anyone who does not already work in this repository. It goes here
   * instead: behind its own control, labelled as what it is, so it is one press from
   * the plain-language line it explains and zero presses from nobody.
   */
  function internalNote(words) {
    return el('details', {
      'data-role': 'internal-reference',
      style: style({ margin: '2px 0 6px' }),
    }, [
      el('summary', {
        style: style({
          display: 'flex', 'align-items': 'center', 'min-height': '36px', 'box-sizing': 'border-box',
          color: T.attendant, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine,
        }),
      }, ['internal reference -- names inside this codebase']),
      el('p', {
        'data-role': 'internal-reference-body',
        style: style({
          margin: '0', color: T.attendant, 'font-family': T.mono, 'font-size': T.time,
          'line-height': T.recordLine, 'overflow-wrap': 'anywhere',
        }),
      }, [words]),
    ]);
  }

  function notDrawnSection(record) {
    const unidentifiable = record.notDrawn.unidentifiable;
    return section('not-drawn', 'stated', [
      // No `omitted` heading: the control this section is drawn inside is named
      // `omitted`, one line above, and an <h2> repeating it was the word twice.
      // The census sentence lost four words to the same rule -- "changes this screen
      // could not file under any subject" said "this screen" on a screen.
      kvLine('changes filed under no subject', unidentifiable.count === null ? '(not counted: the list was not read)' : String(unidentifiable.count)),
      // Owner #349 (3): this was the kvLine grid written a second time with a wider
      // name column. The width is an argument now and there is one builder.
      el('div', { 'data-role': 'omissions' }, UNDRAWN.map((entry) => kvLine(
        entry.label ?? (entry.plain ? entry.what.replace(/\s*\([^)]*\)\s*$/, '') : entry.what),
        entry.plain ?? entry.why,
        { nameWidth: '16rem', attrs: { 'data-omission': entry.what } },
      ))),
      internalNote(UNDRAWN.map((entry) => `${entry.what}: ${entry.why}`).join(' | ')),
    ]);
  }

  // -- the whole screen --------------------------------------------------------

  function frame(children) {
    return el('div', {
      'data-face': FACE_ID, 'data-question': QUESTION,
      style: style({ display: 'block', background: T.page, color: T.ink, 'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, padding: `0 ${T.padX} 40px` }),
    }, children.filter(Boolean));
  }

  /**
   * A clock, or an honest absence of one. `performance.now()` is in every browser and
   * in the Node this face's own tools run under, but a face may not assume the host it
   * is mounted into has it; where it does not, the footer draws a dash rather than a
   * figure nobody measured.
   */
  const clockNow = () => (typeof performance === 'object' && performance !== null && typeof performance.now === 'function'
    ? performance.now()
    : null);

  function waitingView() {
    const started = clockNow();
    const record = toRecord({});
    const drawn = [headerLine(record), figures(record), plain(ATLAS_MESSAGES.READING, 'reading')];
    const ended = clockNow();
    return frame([
      ...drawn,
      P.runtimeFooter({ renderMs: started === null || ended === null ? null : ended - started, source: null }),
    ]);
  }

  /**
   * Why the list is not here, in the plainest words this screen has, with the
   * machine's own account of it one press away.
   *
   * req/97 gap-list item 4, applied to the state nobody photographed: this screen was
   * drawing `code: UNAUTHORIZED` and, on the other branch, the raw JSON of what it
   * had asked for -- an all-caps wire constant and a route name on a product surface,
   * the two shapes req/96 axis B scores zero for and the two this face had already
   * removed from every other part of itself. The words that say what happened stay on
   * the screen; the words that only mean something inside this codebase go where the
   * rest of them are, behind the labelled reference.
   */
  /**
   * Owner #348 (4), and one candidate this round looked at and put back.
   *
   * `outcome: refused` sits above `forbidden: this token may not list transformations`
   * and reads like the same fact twice. It is not: the four outcomes (refused, failed,
   * absent, and the answered one that never reaches here) are the four different ways
   * this screen can have no list, and the sentence underneath is whatever the other
   * side happened to send -- which on a transport failure carries no word for which of
   * the four it was. Removing the line cost three fail-closed tests the property they
   * exist to hold, which is how the removal was caught. Sixteen characters is not
   * worth a reader being unable to tell a refusal from a broken connection.
   */
  function listOutcomeLines(record) {
    const raw = record.rawTransformations;
    const lines = [plain(`outcome: ${record.listOutcome}`, 'outcome')];
    const internal = [];
    if (record.listOutcome === 'refused' && raw) {
      lines.push(plain(`${raw.problem?.title ?? ''}: ${raw.problem?.detail ?? ''}`, 'refusal'));
      internal.push(`gx_code: ${raw.gx_code ?? raw.problem?.gx_code ?? 'none'}`);
    }
    if (record.listOutcome === 'failed' && raw) lines.push(plain(`${raw.reason}: ${raw.detail ?? ''}`, 'failure'));
    if (record.listOutcome === 'absent' && raw) {
      lines.push(plain(`${raw.reason}`, 'absence'));
      internal.push(`requested: ${JSON.stringify(raw.requested ?? null)}`);
    }
    if (internal.length > 0) lines.push(internalNote(internal.join(' | ')));
    return lines;
  }

  /** What a box says when there is nothing to put in it: the words, with the room
   * around them the box's own head has, so a stated absence is not printed against
   * the border. */
  const boxWords = (children) => el('div', { style: style({ padding: `8px ${T.padX}` }) }, children.filter(Boolean));

  // -- the menu a right-click opens (Owner #348 (2)) -----------------------------

  /**
   * What this face puts under a right-click, and why there is one at all on a screen
   * that sends nothing.
   *
   * The entries are built by mapping `declaration.mjs` OFFERS. Nothing here names an
   * offer inline, so the menu cannot grow one this face has not declared -- the same
   * "one declaration, drawn twice, never disagreeing" discipline the gutter faces
   * hold between their gutter and their menu, applied to a face whose second consumer
   * is the legend rather than a gutter.
   *
   * `ACTS` is mapped too, and on this face it is empty, so that loop contributes
   * nothing and the disabled line underneath states `ACTS_REASON` instead. That line
   * is the atom rather than a consolation for missing it: a reader who right-clicks a
   * row on a face with acts and then right-clicks a row here has to be told why the
   * second menu is shorter, and the browser's own page menu tells them nothing.
   *
   * An entry with nothing to give is drawn disabled carrying its reason, which is the
   * rule an unavailable act already follows, rather than being left out.
   */
  const MENU_PART = 'row-menu';

  function menuEntry(entry, ruled) {
    return el('button', {
      type: 'button',
      'data-menu-entry': entry.id,
      'data-enabled': String(Boolean(entry.enabled)),
      'data-menu-value': entry.value ?? null,
      disabled: entry.enabled ? null : 'disabled',
      title: entry.why ?? null,
      style: style({
        display: 'flex', 'align-items': 'center', gap: '8px',
        width: '100%', 'min-height': T.pitch, 'box-sizing': 'border-box',
        padding: `6px ${T.padX}`, 'text-align': 'left',
        border: 'none', background: 'transparent',
        // What a hand can do, and what it cannot, are two blocks with a line between
        // them rather than three sentences of the same size in a column. Without it
        // the menu read as a paragraph, which is not what a menu is.
        'border-top': ruled ? `1px solid ${T.rule}` : null,
        'margin-top': ruled ? '4px' : null,
        'padding-top': ruled ? '10px' : null,
        color: entry.enabled ? T.ink : T.attendant,
        'font-family': T.sans, 'font-size': T.record, 'line-height': T.recordLine, 'font-weight': TYPE.body.weight,
        ...WRAP,
      }),
    }, [el('span', {}, [entry.words])]);
  }

  /**
   * The menu itself.
   *
   * No `position` is written here, and that is not a way around this face's own
   * `nothing-out-of-flow` gate -- it is the reason the gate can stay at zero and this
   * menu can still escape the `overflow:hidden` every row cell declares. `popover`
   * hands the node to the browser's top layer, which is where a menu belongs, and the
   * two coordinates that place it at the pointer are set on the node when it opens
   * rather than declared in this tree. It also buys the three properties this atom
   * asks to be pinned, from the platform instead of from a handler: Escape and a
   * click away both close it, and one node cannot be shown twice.
   */
  function menuTree(entries) {
    return el('div', {
      'data-part': MENU_PART,
      'data-entries': String(entries.length),
      popover: 'auto',
      role: 'menu',
      'aria-label': ATLAS_MESSAGES.MENU,
      style: style({
        // A width band rather than a width. Without the upper bound the menu was as
        // wide as its longest sentence -- 475px, measured on the first shot of it,
        // which is a paragraph with a border and not a menu.
        inset: 'auto', margin: '0', padding: '4px 0', 'min-width': '12rem', 'max-width': '22rem', overflow: 'visible',
        'box-sizing': 'border-box',
        border: `1px solid ${T.rule}`,
        'border-radius': T.radiusControl,
        background: T.page,
      }),
    }, entries.map((entry, index) => menuEntry(entry, !entry.enabled && (entries[index - 1]?.enabled ?? false))));
  }

  /**
   * The entries for one thing under the pointer.
   *
   * `value` is null when the thing under the pointer is a stated gap rather than a
   * value; the offer is then drawn disabled with the offer's own declared `why`.
   * `outcome` is what the last press of this menu actually did, drawn as its own
   * line, because a control that looks identical whether or not it worked is the
   * pretending this tree refuses everywhere else.
   */
  function menuEntriesFor(value, outcome = null) {
    const entries = OFFERS.map((offer) => ({
      id: offer.offer,
      words: offer.label,
      enabled: value !== null,
      value,
      why: value === null ? offer.why : value,
    }));
    for (const act of ACTS) entries.push({ id: act.act, words: act.label, enabled: false, why: ACTS_REASON });
    if (ACTS.length === 0) entries.push({ id: 'no-acts', words: ACTS_REASON, enabled: false, why: ACTS_REASON });
    if (outcome !== null) entries.push({ id: 'outcome', words: outcome, enabled: false, why: outcome });
    return entries;
  }

  /**
   * The subjects, always inside a box, in all three of the states this screen has.
   *
   * A population that was read and is empty keeps its border and states `0`; a
   * population that could not be read keeps its border and states a dash. Those are
   * two different facts and this screen has always refused to draw them the same way
   * -- the box is what makes the refusal visible from across the room instead of only
   * in the sentence underneath.
   */
  function subjectsSection(record) {
    if (record.listOutcome !== ANSWERED) {
      return section('list', 'unread', [
        P.box({
          name: 'subjects',
          count: null,
          // Not "subjects" twice. The head is `<name> <count> <noun>`, so naming the
          // box and its noun the same word put "subjects -- subjects" on the screen.
          noun: 'found',
          children: boxWords([aside(ATLAS_MESSAGES.LIST_UNREAD, 'unread'), ...listOutcomeLines(record)]),
        }),
      ]);
    }
    if (record.subjects.length === 0) {
      // Two states, two sentences. `unfiled` is the case where the engine answered and
      // the rows could not be grouped; the reasons are the ones this face recorded per
      // row while grouping, counted here rather than restated, and the rows themselves
      // are named underneath so the reader is not asked to take the count on trust.
      const unfiled = record.notDrawn.unidentifiable;
      const arrived = record.totalTouches ?? 0;
      if (arrived > 0 && (unfiled.count ?? 0) > 0) {
        const reasons = [...new Set(unfiled.entries.map((entry) => entry.why))].join('; ');
        return section('list', 'unfiled', [
          P.box({
            name: 'subjects',
            count: 0,
            noun: 'found',
            children: boxWords([
              aside(ATLAS_MESSAGES.SUBJECTS_UNFILED(arrived, reasons || ATLAS_MESSAGES.UNFILED_WHY), 'unfiled'),
              ...unfiled.entries.map((entry) => kvLine(
                `change at position ${entry.index}`,
                `${entry.why}${entry.id ? ` (id ${entry.id})` : ''}`,
              )),
            ]),
          }),
        ]);
      }
      return section('list', 'no-subjects', [
        P.box({
          name: 'subjects',
          count: 0,
          noun: 'found',
          children: boxWords([aside(ATLAS_MESSAGES.NO_SUBJECTS, 'no-subjects')]),
        }),
      ]);
    }
    return section('list', 'drawn', [
      el('div', { 'data-role': 'subjects' }, record.subjects.map(subjectBox)),
    ]);
  }

  function view(state) {
    // Owner #340 asks the second of this project's five design principles for a
    // measured figure rather than a claimed one, so the clock is read around the work
    // this function already does -- the whole tree, from the attest step to the last
    // box -- and the figure that reaches the footer is that subtraction and nothing
    // else. Only the footer and the frame it is placed in fall outside the reading,
    // because neither can be built until the number exists.
    const started = clockNow();
    const record = toRecord(state);
    // Owner directive #335 (1): the claims and the omitted census are behind a click,
    // in the same one row as why and legend. Before this they were two always-open
    // bands taking about two thirds of the window, ahead of the second subject line.
    const drawn = [
      headerLine(record),
      figures(record),
      controlsRow([
        // Owner #348 (4). Each hint now says something its own name does not:
        // `about this screen` was true of the entire screen, `symbols used` is what
        // the word legend means, and `what is not drawn` is what the word omitted
        // means. What is behind each control decided the replacement.
        controlToggle('why', 'which screen to open first', el('div', {}, [aside(ORDER.reason_plain, 'why-body'), internalNote(ORDER.reason)])),
        controlToggle('legend', 'how to read a line', legendBody()),
        record.listOutcome === ANSWERED ? controlToggle('claims', 'what you can check', claimsSection(record)) : null,
        controlToggle('omitted', 'and why', notDrawnSection(record)),
      ].filter(Boolean)),
      subjectsSection(record),
    ];
    const ended = clockNow();
    return frame([
      ...drawn,
      P.runtimeFooter({
        renderMs: started === null || ended === null ? null : ended - started,
        source: record.listOutcome === ANSWERED ? READ_SOURCE : null,
      }),
    ]);
  }

  // -- reading -------------------------------------------------------------------

  async function read(port) {
    const caller = callerFor(port);
    const transformations = await caller.fold(READS.transformations);
    return { transformations };
  }

  // -- mount ---------------------------------------------------------------------

  function mount(host, port, notices = []) {
    if (!host || typeof host.appendChild !== 'function') throw new TypeError(ATLAS_MESSAGES.NO_HOST);
    if (!port || typeof port !== 'object') throw new TypeError(ATLAS_MESSAGES.NO_PORT);
    void notices;

    const doc = host.ownerDocument ?? globalThis.document;
    if (typeof doc.getElementById === 'function' && doc.body) P.installSheet(doc, P.element.render);
    // Owner directive #335 (2) and (4): the slim scrollbar and the figure/label type
    // scale are rules, not inline styles, so they arrive the same way the glyph
    // sprite does -- once per document, from the one module that owns them
    // (parts/src/surface.mjs). Same guard, same reason: the structural stand-in in
    // test/dom-stand-in.mjs is not a document and proves nothing about drawing.
    if (typeof doc.getElementById === 'function' && doc.body) P.installSurface(doc, P.element.render);
    let live = true;

    // Owner #348 (2). One menu node at a time, held here rather than looked up, and
    // `at` is where the last right-click landed so a copy can redraw the menu in
    // place with what it actually did.
    let menu = null;
    let at = { x: 0, y: 0 };

    const shutMenu = () => {
      if (menu === null) return;
      const node = menu;
      menu = null;
      if (typeof node.hidePopover === 'function') {
        try { node.hidePopover(); } catch { /* it was not open */ }
      }
      if (node.parentNode && typeof node.parentNode.removeChild === 'function') node.parentNode.removeChild(node);
    };

    const clear = () => { while (host.firstChild) host.removeChild(host.firstChild); };
    const paint = (tree) => {
      if (!live) return;
      // A menu left behind by a repaint would be a menu about a row that is no longer
      // the row it was opened on. It is taken down first, and `clear()` would remove
      // the node anyway -- taking it down here is what stops `menu` pointing at a node
      // nothing is holding.
      shutMenu();
      clear();
      host.appendChild(P.element.render(doc, tree));
    };

    /**
     * Open the menu, or reopen it in the same place carrying what a press just did.
     *
     * The second right-click cannot stack a second menu because the first thing this
     * does is take the current one down -- the node is replaced rather than added to.
     */
    const openMenu = (value, outcome = null) => {
      if (!live) return null;
      shutMenu();
      const node = P.element.render(doc, menuTree(menuEntriesFor(value, outcome)));
      host.appendChild(node);
      menu = node;
      if (typeof node.showPopover === 'function') {
        try { node.showPopover(); } catch { /* a document that will not take a popover */ }
        // Placed after it is shown, because a node in the top layer has no measurable
        // box until it is in it, and a menu that hangs off the bottom of the window is
        // a menu whose last entry nobody can reach.
        if (typeof node.getBoundingClientRect === 'function') {
          const own = node.getBoundingClientRect();
          const width = typeof globalThis.innerWidth === 'number' ? globalThis.innerWidth : own.width;
          const height = typeof globalThis.innerHeight === 'number' ? globalThis.innerHeight : own.height;
          node.style.left = `${Math.max(0, Math.min(at.x, width - own.width))}px`;
          node.style.top = `${Math.max(0, Math.min(at.y, height - own.height))}px`;
        }
      }
      return node;
    };

    /**
     * Take a value, and say whether it was taken.
     *
     * The same shape the shell's own copy control already holds: `data-copied` and
     * `data-copy-failed` on the node, never a control that looks the same either way.
     * The menu is redrawn rather than closed, so the sentence is on the screen and not
     * only in an attribute.
     */
    const takeValue = (value) => {
      const node = menu;
      if (node === null) return;
      if (typeof node.removeAttribute === 'function') {
        node.removeAttribute('data-copied');
        node.removeAttribute('data-copy-failed');
      }
      const board = typeof navigator === 'object' && navigator !== null ? navigator.clipboard : undefined;
      if (!board || typeof board.writeText !== 'function') {
        node.setAttribute('data-copy-failed', 'true');
        const redrawn = openMenu(value, ATLAS_MESSAGES.COPY_FAILED);
        if (redrawn) redrawn.setAttribute('data-copy-failed', 'true');
        return;
      }
      board.writeText(value).then(
        () => { const redrawn = openMenu(value, ATLAS_MESSAGES.COPIED); if (redrawn) redrawn.setAttribute('data-copied', 'true'); },
        () => { const redrawn = openMenu(value, ATLAS_MESSAGES.COPY_FAILED); if (redrawn) redrawn.setAttribute('data-copy-failed', 'true'); },
      );
    };

    /**
     * A right-click, on anything this screen drew a value or a gap into.
     *
     * The keyboard is untouched by this: every disclosure on this screen opens with
     * Enter and Space exactly as it did, and nothing here is the only way to reach
     * anything. `data-menu-value` is on every cell drawn as a value and on the fold
     * line itself; a cell drawn as a stated gap carries `data-cell` and no value, and
     * gets the same menu with the offer disabled and the reason in it.
     */
    const onContextMenu = (event) => {
      const hit = event?.target;
      if (!hit || typeof hit.closest !== 'function') return;
      // A right-click landing inside the menu is not a request for a menu about the
      // menu. The entries carry `data-menu-value` because a press reads it back off
      // them, and without this line that attribute would make the menu its own target.
      if (hit.closest('[data-menu-entry]')) return;
      // A cell first, and the line it sits in only when the pointer is not on a cell.
      // The other way round, the fold line's own `data-menu-value` is found first from
      // inside every cell in it -- so a right-click on a stated gap would have offered
      // the subject's path, which is not the thing under the pointer. A menu that is
      // about something other than what was clicked is worse than no menu.
      const carrier = hit.closest('[data-cell]') ?? hit.closest('[data-menu-value]');
      if (!carrier) return;
      if (typeof event.preventDefault === 'function') event.preventDefault();
      at = {
        x: typeof event.clientX === 'number' ? event.clientX : 0,
        y: typeof event.clientY === 'number' ? event.clientY : 0,
      };
      const value = typeof carrier.getAttribute === 'function' ? carrier.getAttribute('data-menu-value') : null;
      openMenu(value === null || value === '' ? null : value);
    };

    const onMenuPress = (event) => {
      const hit = event?.target;
      if (!hit || typeof hit.closest !== 'function' || menu === null) return;
      const entry = hit.closest('[data-menu-entry]');
      if (!entry) { shutMenu(); return; }
      if (entry.getAttribute('data-enabled') !== 'true') return;
      if (typeof event.preventDefault === 'function') event.preventDefault();
      takeValue(entry.getAttribute('data-menu-value') ?? '');
    };

    // Escape and a click away are what a popover already does, and these two are the
    // same two for a host that has no popover to give (the structural stand-in the
    // unit tests mount into). Shutting twice is shutting once.
    const onKey = (event) => { if (event?.key === 'Escape') shutMenu(); };
    const onAway = (event) => {
      if (menu === null) return;
      const hit = event?.target;
      if (hit && typeof hit.closest === 'function' && hit.closest('[data-menu-entry]')) return;
      shutMenu();
    };

    if (typeof host.addEventListener === 'function') {
      host.addEventListener('contextmenu', onContextMenu);
      host.addEventListener('click', onMenuPress);
    }
    if (typeof doc.addEventListener === 'function') {
      doc.addEventListener('keydown', onKey);
      doc.addEventListener('pointerdown', onAway);
    }

    paint(waitingView());

    const ready = read(port)
      .then((state) => {
        paint(view(state));
        return state;
      })
      .catch((error) => {
        paint(frame([plain(`${ATLAS_MESSAGES.LIST_UNREAD}. ${error.message}`, 'unread')]));
        return null;
      });

    const unmount = () => {
      live = false;
      shutMenu();
      if (typeof host.removeEventListener === 'function') {
        host.removeEventListener('contextmenu', onContextMenu);
        host.removeEventListener('click', onMenuPress);
      }
      if (typeof doc.removeEventListener === 'function') {
        doc.removeEventListener('keydown', onKey);
        doc.removeEventListener('pointerdown', onAway);
      }
      clear();
    };
    unmount.ready = ready;
    return unmount;
  }

  return {
    DECLARATION, mount, read, view, waitingView, toRecord, callerFor, toHtml: P.element.toHtml,
  };
}

export const face = createFace();
export const mount = face.mount;
export { DECLARATION };
