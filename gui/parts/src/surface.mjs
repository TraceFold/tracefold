// SPDX-License-Identifier: Apache-2.0
// The rules a surface needs that an inline style cannot carry, and the one pane a
// screen's details are stored in.
//
// Owner directive #335, three of its five points land here.
//
// (2) The scrollbar. A native Windows scrollbar is 17 device pixels of grey chrome
// with a raised thumb and two arrow buttons, drawn in the platform's colours and not
// in this application's, and it appears on four of the six faces. It cannot be
// touched from an inline style -- ::-webkit-scrollbar and scrollbar-width are
// pseudo-element and property rules and need a rule set. So this module owns one, and
// only one: everything in it is either a token this application already declares or a
// geometric number, and there is no colour literal anywhere in it, so the stylesheet
// of record is still the only thing that owns colour (parts/src/tokens.mjs's own
// contract, unchanged). Firefox gets `scrollbar-width: thin` and the same two colours;
// WebKit/Blink gets an 8px track with a rounded thumb and no buttons.
//
// (4) The type. `--t-record` is 14px and a face draws almost everything at it, so a
// screen reads as one undifferentiated grey field. The rules here give a face's own
// numbers a size of their own (`.gx-figure`) against a small label (`.gx-label`), which
// is the Docker Scout card idiom req/768 names: the number is what the eye lands on and
// the word next to it is support, not the other way round.
//
// (3) The pane. See `detailFrame` below.
//
// Two consumers, one source: `installSurface()` puts these rules into a live document
// beside the glyph sprite, and each face's tools/fixture.mjs writes `SURFACE_CSS` into
// the static page it photographs. A hand-copied second stylesheet is the thing that
// drifts, so there is not one.

import { el, style, installOnce } from './element.mjs';
import { CONSUMED } from './tokens.mjs';
import { BUILD } from '../generated/build.generated.mjs';

export const SURFACE_ID = 'gx-surface-rules';

export const SURFACE_MESSAGES = {
  INSTALLED: 'surface rules installed',
  ALREADY: 'surface rules were already installed here',
  NO_NOUN: 'a stat segment states a number with no noun beside it, so the number is about nothing',
  NO_NAME: 'a box without a name is a border around an unnamed thing',
  NO_COUNT: 'a box states how many things are in it, and zero is a count',
};

/**
 * One surface step, derived rather than declared.
 *
 * Nothing in this application's palette declares "the page, but slightly not the page",
 * and four things below need exactly that: a row under a pointer, a box's head strip, a
 * band's own field, and the footer. Deriving it from two tokens this package already
 * spells keeps the stylesheet of record the only place a colour is chosen -- the
 * mixture has no value of its own, it has whatever those two have, on whichever page
 * the reader is on. tools/boundary.mjs's colour gate reads this file and finds nothing,
 * correctly: there is no colour here.
 */
const STEP = `color-mix(in srgb, ${CONSUMED.ink} 5%, ${CONSUMED.page})`;
const STEP_HARD = `color-mix(in srgb, ${CONSUMED.ink} 9%, ${CONSUMED.page})`;

/**
 * Every declaration below is a token this application owns or a number. No colour
 * literal appears here; `parts/test/boundary.test.mjs` colourLiterals holds that at
 * zero across this package and would fire on one.
 *
 * The rule set carries no comments of its own. It is a string that ships into every
 * face's fixture page and into every live document, so a paragraph in it is a paragraph
 * downloaded on every screen -- and a directive number written in it (a hash followed
 * by three digits) is indistinguishable from a colour to a gate that reads the shipped
 * bytes, which is a real reading this file's own test performs. The commentary lives
 * here instead, in a place a reader looking for it will find it and a browser will not.
 *
 * The operability rules, in the order they appear, and what each one answers:
 *
 *  - `cursor:pointer` on rows, live act buttons and disclosures. Every control on these
 *    screens declared `cursor:default`, on purpose, as this codebase's own convention
 *    for a clickable label -- and the result Owner #340 read was a screen on which
 *    nothing announced itself as pressable. The convention is overturned here for the
 *    three things that genuinely act. `cursor:not-allowed` on a disabled act says the
 *    opposite thing in the same language, rather than saying nothing.
 *  - a hover step on a row, a disclosure and a band segment: one surface step, so a
 *    pointer moving down a list is tracked by the list.
 *  - the selected row takes the accent bed and an accent left edge. It already carried
 *    a 3px marker in the ink colour, which is the same marker an unselected row carries
 *    in transparent, so the only difference was a dark line on a dark ground.
 *  - the act gutter's live buttons take the accent as ink and border, brighten to the
 *    accent bed under a pointer, and invert on press. A disabled one keeps the neutral
 *    ink it already had, so available and unavailable are two different colours rather
 *    than two opacities of one.
 *  - `:focus-visible` draws a 2px accent outline, inset so it cannot be clipped by a
 *    parent's own overflow. There was no visible focus anywhere on any face; a reader
 *    working by keyboard had no way to know where they were.
 *  - the disclosure marker is hidden because every disclosure in this app draws its own
 *    fold glyph from the canon sheet, and the platform's triangle beside it is a second
 *    mark for the same meaning at a size nobody chose.
 *
 * The accent ink is spent on exactly one thing -- that a hand may act here. It is never
 * a standing (parts/src/verdict-badge.mjs owns those, and its own test holds that the
 * accent collides with none of them), so a coloured area on these screens is either a
 * standing or an invitation, and there is no third meaning to learn.
 *
 * `[data-role="act"]` sits beside `[data-part="act-gutter"] button` in every act rule, and
 * that pairing is the point rather than a convenience. The gutter selector names a PLACE,
 * so when all six faces grew a right-click menu under Owner #348 (2) they had a second
 * surface full of acts that this route could not reach -- leaving each of them to spell
 * the accent inline, which is exactly the inert-operability defect this set was rewritten
 * to cure one round ago, arriving by a different door. A role is a claim about what a
 * thing IS, and any face can make it. Reported by the faces/ledger lane, which noticed
 * that its menu could only be styled by copying colours or by lying about its part name.
 *
 * MOTION (Owner #348, confirmed as the priority by #349). One route, and it is the four
 * rules at the top of the set rather than a transition written at each call site.
 *
 * What moves is what changes when a hand arrives: ground, ink and edge. Nothing here
 * transitions a geometric property -- no width, no height, no margin -- because those are
 * the properties whose animation costs a layout pass per frame, and a ledger that reflows
 * while it is being read is the jank the Owner would see rather than the smoothness asked
 * for. The one exception is a 1px `translateY` under a press, which is a compositor
 * transform and moves no other element; a press that produces no physical answer is the
 * thing that reads as unresponsive however fast it is.
 *
 * Two durations, from the one token pair. A state a finger is holding right now takes
 * `--motion-quick`; a container settling takes `--motion-settle`. A reader who has asked
 * their system for less motion gets none of it: the reduced-motion block turns off every
 * transition AND the press transform, rather than leaving a "small" amount of movement
 * that the setting was specifically asked to remove.
 *
 * `.gx-move` is the same route as a class, for a caller drawing something this set does
 * not name. It exists so that the answer to "how do I make my thing move like the others"
 * is a class rather than a copied duration, which is how five durations get into a system
 * that declared two.
 */
export const SURFACE_CSS = `
*{scrollbar-width:thin;scrollbar-color:var(--scrollbar-thumb) var(--scrollbar-track)}
*::-webkit-scrollbar{width:var(--scrollbar-w);height:var(--scrollbar-w)}
*::-webkit-scrollbar-track{background:var(--scrollbar-track)}
*::-webkit-scrollbar-thumb{background:var(--scrollbar-thumb);border-radius:var(--scrollbar-radius)}
*::-webkit-scrollbar-thumb:hover{background:var(--scrollbar-thumb-hover)}
*::-webkit-scrollbar-corner{background:transparent}
*::-webkit-scrollbar-button{display:none;width:0;height:0}
.gx-figure{font-family:${CONSUMED.mono};font-size:${CONSUMED.stat};line-height:${CONSUMED.statLine};font-weight:700;color:${CONSUMED.ink};white-space:nowrap}
.gx-label{font-family:${CONSUMED.sans};font-size:${CONSUMED.record};line-height:${CONSUMED.recordLine};letter-spacing:0.04em;text-transform:uppercase;color:${CONSUMED.attendant}}
.gx-row-line{font-size:15px;line-height:22px}
.gx-move{transition:background-color var(--motion-quick) var(--motion-ease),color var(--motion-quick) var(--motion-ease),border-color var(--motion-quick) var(--motion-ease),opacity var(--motion-quick) var(--motion-ease)}
[data-part="selectable-row"],[data-part="act-gutter"] button,summary,[data-part="stat-band"] [data-role="segment"]{transition:background-color var(--motion-quick) var(--motion-ease),color var(--motion-quick) var(--motion-ease),border-color var(--motion-quick) var(--motion-ease)}
[data-part="detail-pane"],[data-part="box"]{transition:border-color var(--motion-settle) var(--motion-ease)}
[data-part="act-gutter"] button:not([disabled]):active{transform:translateY(var(--press-y))}
@media (prefers-reduced-motion:reduce){.gx-move,[data-part="selectable-row"],[data-part="act-gutter"] button,summary,[data-part="stat-band"] [data-role="segment"],[data-part="detail-pane"],[data-part="box"]{transition:none}[data-part="act-gutter"] button:not([disabled]):active{transform:none}}
[data-part="selectable-row"],[data-part="act-gutter"] button:not([disabled]),summary{cursor:var(--cursor-act)}
[data-part="selectable-row"]{background:${CONSUMED.page};border-left-color:transparent}
[data-part="selectable-row"]:hover{background:${STEP}}
[data-part="selectable-row"][data-selected="true"]{background:${CONSUMED.bedAct};border-left-color:${CONSUMED.act}}
[data-part="act-gutter"] button,[data-role="act"]{background:${CONSUMED.page};border:1px solid ${CONSUMED.rule};color:${CONSUMED.attendant}}
[data-part="act-gutter"] button:not([disabled]),[data-role="act"]:not([disabled]){color:${CONSUMED.act};border-color:${CONSUMED.act}}
[data-part="act-gutter"] button:not([disabled]):hover,[data-role="act"]:not([disabled]):hover{background:${CONSUMED.bedAct}}
[data-part="act-gutter"] button:not([disabled]):active,[data-role="act"]:not([disabled]):active{background:${CONSUMED.act};color:${CONSUMED.page}}
[data-part="act-gutter"] button[disabled],[data-role="act"][disabled]{cursor:var(--cursor-refuse)}
[data-role="act"]{cursor:var(--cursor-act)}
summary:hover{background:${STEP}}
summary::-webkit-details-marker{display:none}
[data-part="stat-band"] [data-role="segment"]:hover{background:${STEP}}
:focus-visible{outline:var(--focus-w) solid var(--focus-ink);outline-offset:var(--focus-offset)}
[data-part="act-gutter"] button:focus-visible{outline-offset:var(--focus-offset-in)}
`.trim();

/** The rules as a node, so the one builder in this package still builds every element. */
export function surfaceStyle() {
  return el('style', { id: SURFACE_ID, 'data-part': 'surface-rules' }, [SURFACE_CSS]);
}

/**
 * The rules in a live document, once. Idempotent per document for the same reason
 * installSheet() is: a second face mounted into the same page must not install a
 * second copy.
 */
export function installSurface(doc, render) {
  const put = installOnce(doc, render, surfaceStyle(), SURFACE_ID, { into: 'head' });
  return { ...put, why: put.installed ? SURFACE_MESSAGES.INSTALLED : SURFACE_MESSAGES.ALREADY };
}

/**
 * Owner directive #335 (3): a row's detail is stored, never inlined.
 *
 * What this replaces is the shape req/97 measured: a note drawn underneath the row it
 * belongs to, so opening one record pushed every record below it down a screen and two
 * records filled the window. A detail that lives in one pane cannot do that -- the list
 * keeps its geometry whatever is selected, and exactly one object is ever described,
 * which is the property Studio's own Code / declaration / log panes have and ours did
 * not.
 *
 * It is a wrapping flex row and not a media query on purpose. `flex-wrap` puts the
 * pane beside the list when there is room for both stated minimums and underneath it
 * when there is not, and it does that from the width the box actually gets rather than
 * from the width of the viewport -- which matters here, because a face is mounted
 * inside a shell dock whose width is a shell act and has nothing to do with the
 * window's. A viewport media query would put a detail sidebar next to a list in a
 * 300px dock.
 *
 * The pane is height-bounded and scrolls inside itself, so a long detail cannot make
 * the page taller than the screen; that is the other half of directive #335 (2),
 * "design so scrollbars rarely appear" -- the one that does appear is inside the pane,
 * it is the slim one declared above, and the list never grows one of its own.
 */
export function detailFrame(listNode, paneNode) {
  return el('div', {
    'data-part': 'list-with-detail',
    style: style({ display: 'flex', 'flex-wrap': 'wrap', 'align-items': 'flex-start', gap: '16px' }),
  }, [
    el('div', { 'data-role': 'list', style: style({ flex: '1 1 620px', 'min-width': '0' }) }, [listNode]),
    paneNode,
  ].filter(Boolean));
}

export const PANE_MESSAGES = {
  // "in the list", not "on the left". detailFrame() is a wrapping flex row on purpose --
  // when there is not room for both stated minimums the pane wraps BELOW the list, which
  // is the normal case at this application's own narrow viewport. So the sentence told a
  // reader to look left at exactly the widths where the thing it names is above them.
  // Naming the list instead is true at every width, which is the property the layout was
  // chosen for in the first place.
  NOTHING: 'no row is open. Choose one in the list and everything held about it is drawn here, in full.',
};

/**
 * The pane itself. One object at a time, named at the top, with its own facts under
 * it as label/value pairs -- and nothing else, ever: the sentences that are the same
 * whichever row is chosen belong in the legend, which is why no caller passes prose
 * through here.
 */
export function detailPane({ subject = null, lines = null, said = null } = {}) {
  const entries = Array.isArray(lines) ? lines : [];
  return el('aside', {
    'data-part': 'detail-pane',
    'data-subject': subject ?? null,
    'data-count': String(entries.length),
    style: style({
      flex: '1 1 320px',
      'min-width': '0',
      'max-height': '520px',
      'overflow-y': 'auto',
      'box-sizing': 'border-box',
      border: `1px solid ${CONSUMED.rule}`,
      'border-radius': CONSUMED.radiusContainer,
      // req/822_c7: one elevation step -- the pane is a panel, not more page.
      background: CONSUMED.raisedPage,
      padding: `10px ${CONSUMED.padX}`,
    }),
  }, [
    el('div', { 'data-role': 'pane-head', class: 'gx-label' }, [subject ? 'open row' : 'detail']),
    subject
      ? el('div', {
        'data-role': 'pane-subject',
        class: 'gx-figure',
        style: style({ 'overflow-wrap': 'anywhere', margin: '2px 0 8px' }),
      }, [subject])
      : el('p', {
        'data-role': 'pane-empty',
        style: style({
          margin: '6px 0 0', color: CONSUMED.attendant, 'font-family': CONSUMED.sans,
          'font-size': CONSUMED.record, 'line-height': CONSUMED.recordLine,
        }),
      }, [said ?? PANE_MESSAGES.NOTHING]),
    ...entries.map((entry) => el('div', {
      'data-role': 'pane-line',
      'data-name': String(entry.name),
      style: style({
        display: 'grid', 'grid-template-columns': 'minmax(0,9rem) minmax(0,1fr)', gap: '10px',
        padding: '3px 0', 'border-top': `1px solid ${CONSUMED.rule}`,
        'font-family': CONSUMED.sans, 'font-size': CONSUMED.record, 'line-height': CONSUMED.recordLine,
        'overflow-wrap': 'anywhere',
      }),
    }, [
      el('span', { style: style({ color: CONSUMED.attendant }) }, [String(entry.name)]),
      el('span', { style: style({ color: CONSUMED.ink }) }, [String(entry.value)]),
    ])),
  ].filter(Boolean));
}

// ---- the stat band (req/784 A-01, ranked #1 of the ten adoptions) --------------------

/**
 * A bordered band of equal columns at the head of a screen, each column carrying one
 * mark, one bold number, and the noun that number counts.
 *
 * This is the single element in the whole reference round that is literally what Owner
 * directive #335 asked for -- the number is what the eye lands on and the word beside
 * it is support -- and it is the fastest answer available to Owner #340's own test,
 * that a screen be readable at a glance:
 * before reading anything a viewer knows how many things this screen holds and how they
 * are divided. It replaces prose with facts at the exact place every one of these faces
 * previously opened with a sentence.
 *
 * Three rules it enforces rather than requests:
 *
 *  1. **A number with no noun is refused.** The reference set's own best instance of
 *     this band breaks its own pattern on its fourth segment, which states a size with
 *     no noun; the reader is left to infer what is being counted. Throwing here is the
 *     `stat-band-gate` req/784's fifth section ranks first, applied at the one place a band can be
 *     built rather than as a lint pass over the faces that build one.
 *  2. **Zero is drawn.** A caller passes 0 and 0 appears, because "none of these" is a
 *     measurement and a missing segment is not. A count that is not known passes null
 *     and gets a dash (req/784 L-04) -- the two are different facts and are drawn
 *     differently.
 *  3. **The columns are equal.** `repeat(N, minmax(0,1fr))` rather than content-sized
 *     tracks, so the band's own geometry does not shift when a number goes from 9 to
 *     1000 and a reader's eye can return to the same place on every face.
 *
 * `tone` is an ink the caller has already chosen (a standing's, from
 * parts/src/verdict-badge.mjs's table). This function places it and decides nothing.
 */
export const STAT_DASH = '--';

export function statBand(segments, { said = null } = {}) {
  const cells = Array.isArray(segments) ? segments : [];
  for (const cell of cells) {
    if (!cell || typeof cell.noun !== 'string' || cell.noun.trim() === '') {
      throw new Error(`${SURFACE_MESSAGES.NO_NOUN}: ${JSON.stringify(cell?.noun ?? null)}`);
    }
  }
  return el('div', {
    'data-part': 'stat-band',
    'data-count': String(cells.length),
    title: said,
    style: style({
      display: 'grid',
      'grid-template-columns': `repeat(${Math.max(cells.length, 1)}, minmax(0,1fr))`,
      'align-items': 'center',
      'box-sizing': 'border-box',
      border: `1px solid ${CONSUMED.rule}`,
      'border-radius': CONSUMED.radiusContainer,
      // req/822_c7: the band is the screen's head panel -- raised, like the reference
      // set's KPI strips, so the figures stand off the page before a word is read.
      background: CONSUMED.raisedPage,
      margin: '0 0 10px',
      overflow: 'hidden',
    }),
  }, cells.map((cell, index) => el('div', {
    'data-role': 'segment',
    'data-noun': cell.noun,
    'data-value': cell.count === null || cell.count === undefined ? 'unread' : String(cell.count),
    title: cell.said ?? null,
    style: style({
      display: 'flex',
      'align-items': 'center',
      gap: '8px',
      'min-width': '0',
      padding: `6px ${CONSUMED.padX}`,
      ...(index === 0 ? {} : { 'border-left': `1px solid ${CONSUMED.rule}` }),
    }),
  }, [
    cell.mark ?? null,
    el('div', { style: style({ 'min-width': '0' }) }, [
      el('div', {
        'data-role': 'figure',
        class: 'gx-figure',
        style: style({ ...(cell.tone ? { color: cell.tone } : {}) }),
      }, [cell.count === null || cell.count === undefined ? STAT_DASH : String(cell.count)]),
      el('div', {
        'data-role': 'noun',
        class: 'gx-label',
        style: style({ overflow: 'hidden', 'text-overflow': 'ellipsis', 'white-space': 'nowrap' }),
      }, [cell.noun]),
    ]),
  ].filter(Boolean))));
}

// ---- the box (Owner #340: Studio's Box/Package container, in this app's own terms) ---

/**
 * A bordered, named container for a group of records, with its own count and its own
 * standing.
 *
 * Owner #340 named this as missing by pointing at the reference tool's package/box
 * idiom, and the reference round had already found the same shape twice under other
 * names (req/784 A-26's asset card, A-27's section-header row): a group of things is
 * drawn as an area with an edge, and the edge's head states what the group is, how many
 * are in it, and what condition it is in. What these faces drew instead was a heading,
 * a rule, and then rows -- so the reader had to work out where one group ended and the
 * next began from spacing alone, which is an encoding with no legend.
 *
 * Three properties, each of which is one of the reference round's own findings:
 *
 *  - **The count is in the head, and zero is in the head.** A container that holds N
 *    things states N (req/784 R-16 is the failure this refuses -- a card whose header
 *    omits its own count). An empty box says `0` and keeps its border; it does not
 *    vanish, because a group that is empty and a group that was never read are
 *    different facts (R-04).
 *  - **The standing is a pill, not a colour.** `pill` is a node the caller already
 *    built (verdict-badge's chip()), so this function places a standing and never
 *    decides one.
 *  - **It does not scroll.** There is no max-height and no overflow here. A box grows
 *    to its contents and the screen is composed so that it fits -- Owner #335 (2), and
 *    the reason the only scrolling container this package owns is the detail pane.
 */
export function box({
  name, count, noun = 'records', pill = null, said = null, children = null, open = null,
} = {}) {
  if (typeof name !== 'string' || name.trim() === '') throw new Error(SURFACE_MESSAGES.NO_NAME);
  if (count === undefined) throw new Error(`${SURFACE_MESSAGES.NO_COUNT}: ${name}`);
  const kids = Array.isArray(children) ? children.filter(Boolean) : [children].filter(Boolean);
  const shown = count === null ? STAT_DASH : String(count);
  return el('section', {
    'data-part': 'box',
    'data-box': name,
    'data-count': shown,
    'data-noun': noun,
    'data-open': open === null ? null : String(open),
    style: style({
      'box-sizing': 'border-box',
      border: `1px solid ${CONSUMED.rule}`,
      'border-radius': CONSUMED.radiusContainer,
      background: CONSUMED.page,
      margin: '0 0 10px',
    }),
  }, [
    el('div', {
      'data-role': 'box-head',
      title: said,
      style: style({
        display: 'flex',
        'align-items': 'center',
        gap: '10px',
        'min-width': '0',
        // The head is allowed a second line rather than a crushed first one. In the
        // 320px right dock the gate ladder's head asks for a name, a count and a pill
        // on one row and does not get it, and because the pill is `flex:none` (below)
        // and the name is the only shrinking item, the name took the entire shortfall:
        // measured in the real window, `readiness` drew as one character followed by an
        // ellipsis, as the title of the block holding this product's central act.
        // Wrapping spends a row of height and keeps the word.
        'flex-wrap': 'wrap',
        'row-gap': '4px',
        padding: `5px ${CONSUMED.padX}`,
        background: STEP_HARD,
        'border-bottom': kids.length > 0 ? `1px solid ${CONSUMED.rule}` : 'none',
        // The head sits inside the box's own 1px edge, so its corners are the
        // container's radius less that pixel -- `calc()` rather than a second number,
        // because a hand-picked "3px beside an 8px" is exactly the drift the scale
        // exists to stop, and it goes wrong the moment either value changes.
        'border-radius': `calc(${CONSUMED.radiusContainer} - 1px) calc(${CONSUMED.radiusContainer} - 1px) 0 0`,
      }),
    }, [
      el('span', {
        'data-role': 'box-name',
        style: style({
          'font-family': CONSUMED.sans,
          'font-size': CONSUMED.record,
          'line-height': CONSUMED.recordLine,
          'font-weight': '600',
          color: CONSUMED.ink,
          overflow: 'hidden',
          'text-overflow': 'ellipsis',
          'white-space': 'nowrap',
          // A floor under the name, for the same reason the pill has `flex:none`: an
          // item with `overflow:hidden` resolves its automatic minimum width to zero,
          // and zero is what it went to. An ellipsis is a promise that the reader can
          // still tell what was abbreviated, and one character keeps none of it.
          'min-width': '6rem',
        }),
      }, [name]),
      el('span', {
        'data-role': 'box-count',
        style: style({
          'font-family': CONSUMED.mono,
          'font-size': CONSUMED.record,
          'line-height': CONSUMED.recordLine,
          color: CONSUMED.attendant,
          'white-space': 'nowrap',
          flex: 'none',
        }),
      }, [`${shown} ${noun}`]),
      el('span', { style: style({ flex: '1' }) }, []),
      // `flex:none`, because the head's own name and its pill are both flex items with
      // `overflow:hidden`, which resolves each one's automatic minimum width to zero --
      // so at a narrow width BOTH shrink, and the standing is the one that loses. The
      // faces/atlas lane measured it at 720px: the name drew 478px of the 543 it wanted
      // and the pill drew 30 of 37, landing `Admit` on the screen as `Ad...`.
      //
      // The pill is a fixed, short, closed vocabulary and the name is arbitrary-length
      // text, so the name is the one that should give way. This makes that explicit
      // instead of leaving two shrinking things to fight.
      pill ? el('span', { 'data-role': 'box-pill', style: style({ flex: 'none', 'min-width': 'auto' }) }, [pill]) : null,
    ].filter(Boolean)),
    ...kids,
  ]);
}

// ---- the runtime footer (req/784 A-11, adoption #5) ----------------------------------

/**
 * What this screen cost and what it is, on the screen.
 *
 * Two of the four fields are measured in this window at draw time and passed in
 * (`renderMs`, and the source a face read); two are stamped at build time by
 * parts/tools/generate-build.mjs, because a face has no disk and may not invent a
 * number. Anything null is drawn as a dash and never as a zero.
 *
 * It discharges two standing requirements at once in one strip: the second of this
 * project's five design principles (lightweight and fast) asks for a
 * runtime figure that is measured rather than claimed, and a figure printed on every
 * screenshot cannot go stale the way a figure in a document does; req/768 F-K asks that
 * a capture be able to date itself, and a shot of any face now carries the commit it
 * was taken against.
 *
 * `dirty` is drawn, not hidden. A build taken from a tree with uncommitted changes is
 * not exactly its commit, and a footer that printed the hash alone would be claiming it
 * was.
 */
export const FOOTER_MESSAGES = {
  UNREAD: 'this field was not measured, so it is drawn as a dash rather than as a zero',
  DIRTY: 'the tree carried uncommitted changes when this build was generated, so it is not exactly this commit',
};

/**
 * A measured duration, printed at a precision that cannot round it away.
 *
 * This function exists because the first version of the footer printed `toFixed(1)`, and
 * the faces on this tree build their trees in tens of microseconds -- so a real, measured,
 * non-zero reading came out as `render 0.0 ms`, which is the fabricated zero the doc
 * comment two paragraphs up forbids in as many words. The footer was doing the exact
 * thing it was written to prevent, and it took a lane measuring an actual face to see it
 * (`faces/graph`, whose three states paint in 7.3, 0.9 and 0.6 ms; the shorter two are
 * the ones that would have been misdrawn on a faster machine).
 *
 * The rule: below one millisecond, two significant figures, so 0.032 stays 0.032 and
 * nothing non-zero can ever reach a zero. At or above one millisecond, one decimal, which
 * is more precision than a reader of a render cost needs. An exact zero prints as `0`
 * with no decimal at all -- a bare zero reads as the measurement it is, where `0.0` reads
 * as something that was rounded.
 */
export function figureFor(ms) {
  if (ms === 0) return '0';
  return ms >= 1 ? ms.toFixed(1) : Number(ms.toPrecision(2)).toString();
}

export function runtimeFooter({
  renderMs = null, source = null, build = BUILD,
} = {}) {
  const held = build ?? {};
  const ms = typeof renderMs === 'number' && Number.isFinite(renderMs)
    ? `${figureFor(renderMs)} ms`
    : STAT_DASH;
  const suite = typeof held.suiteTests === 'number'
    ? `${held.suiteTests} tests / ${held.suiteFailed ?? STAT_DASH} failed`
    : STAT_DASH;
  const commit = held.commit ? `${held.commit}${held.dirty ? ' +changes' : ''}` : STAT_DASH;
  const fields = [
    { name: 'render', value: ms, said: renderMs === null ? FOOTER_MESSAGES.UNREAD : null },
    { name: 'read', value: source === null ? STAT_DASH : String(source), said: source === null ? FOOTER_MESSAGES.UNREAD : null },
    { name: 'suite', value: suite, said: held.suiteAt ? `measured ${held.suiteAt}` : FOOTER_MESSAGES.UNREAD },
    { name: 'build', value: commit, said: held.dirty ? FOOTER_MESSAGES.DIRTY : held.at },
  ];
  return el('footer', {
    'data-part': 'runtime-footer',
    'data-build': held.commit ?? null,
    'data-render-ms': renderMs === null ? null : String(renderMs),
    style: style({
      display: 'flex',
      'flex-wrap': 'wrap',
      'align-items': 'center',
      gap: '14px',
      'min-height': '20px',
      'box-sizing': 'border-box',
      padding: `1px ${CONSUMED.padX}`,
      'margin-top': '10px',
      'border-top': `1px solid ${CONSUMED.rule}`,
      background: STEP,
      'font-family': CONSUMED.mono,
      'font-size': CONSUMED.record,
      'line-height': CONSUMED.recordLine,
      color: CONSUMED.attendant,
    }),
  }, fields.map((field) => el('span', {
    'data-role': 'footer-field',
    'data-name': field.name,
    title: field.said,
    style: style({ 'white-space': 'nowrap' }),
  }, [`${field.name} ${field.value}`])));
}
