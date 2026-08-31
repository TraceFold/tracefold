// SPDX-License-Identifier: Apache-2.0
//
// What the ledger looks like. Rebuilt from nothing after the screen it replaces was
// read by eye and failed on five counts (req/893 S0).
//
// The five, and what each one is structurally rather than cosmetically:
//
//   1. Eleven act buttons stood on the screen at rest -- one `undo` on every settled row
//      and three more on every held row -- in a gutter beside the list, so they could
//      not line up with the rows they belonged to and did not. Chrome was being copied
//      once per row. Here a row at rest carries no control at all: the line itself is
//      the control, and a row's acts exist only while that row is open. Act controls on
//      screen go from a constant eleven to zero at rest and at most three when a held
//      row is open.
//
//   2. A path broke mid-word while, on the same row, a verdict was cut to `not a rec...`
//      -- wrapping and clipping mixed with no policy, and the row that wrapped grew to
//      five times the pitch. There is one policy here and it is stated once: a line
//      never wraps. Every cell is `nowrap` and clipped, every row is exactly one pitch
//      high, and the whole of every value is in the opened row. A path is cut in the
//      middle rather than at the end, because the end of a path is the part that names
//      the file.
//
//   3. A detail frame took a third of the width to say that nothing was open. A panel
//      whose only content is the news that it has no content is furniture. Detail opens
//      inside the row it describes, so it costs nothing when nothing is chosen and it
//      is next to its subject when something is.
//
//   4. Five hues. A screen that emphasises five things emphasises nothing, so the budget
//      is one and it is spent on reversibility -- whether a thing can still be taken
//      back -- because that is the claim this product exists to make. The three verdicts
//      are told apart by mark, word and weight, which survives a monochrome print and a
//      colour-blind reader; a hue does not.
//
//   5. Six disclosures with six subtitles stood in front of the data: twelve pieces of
//      text before the first record, produced by a rule that says to move explanation
//      out of the way. Building the escape hatch is not the same as escaping. There is
//      one entrance now, it is one line, and it carries the count of what is behind it
//      so that it is itself a fact rather than a door with a label on it.
//
// Nothing here names a colour, a length or a token. Every appearance is asked for by
// intent, every quantity by purpose, and both resolve through ./roles.mjs -- which is
// the rank this tree did not have and the reason a screen can now be checked against
// the vocabulary the engine speaks rather than against somebody's taste.

import { ink, metric, markOfIntent, WEIGHT } from './roles.mjs';

/**
 * How many characters a column will show before it cuts. Declared, because a budget
 * worked out from a width is a guess in front of a renderer that has not loaded its font
 * yet, and a guess that is wrong silently is how a value goes unseen. Cutting is also
 * enforced in CSS (`nowrap` plus `hidden`), so these numbers decide where the ellipsis
 * falls and never whether the line holds.
 */
const BUDGET = Object.freeze({ actor: 18, effect: 12, verdict: 16, path: 44 });

/** Cut in the middle, keeping both ends. Used where the tail is the informative half. */
function cutMiddle(value, budget) {
  const s = String(value);
  if (s.length <= budget) return { shown: s, cut: false };
  const keepEnd = Math.max(6, Math.floor((budget - 1) / 2));
  const keepStart = budget - 1 - keepEnd;
  return { shown: `${s.slice(0, keepStart)}…${s.slice(s.length - keepEnd)}`, cut: true };
}

/** Cut at the end, where what comes first is what identifies the value. */
function cutEnd(value, budget) {
  const s = String(value);
  if (s.length <= budget) return { shown: s, cut: false };
  return { shown: `${s.slice(0, budget - 1)}…`, cut: true };
}

export function createScreen(P, {
  MESSAGES, QUESTION, FACE_ID, ORDER, UNDRAWN, ACTS, MEMBER_KEYS,
}) {
  const { el, style, find } = P.element;

  /**
   * A member's value, or undefined.
   *
   * `toRecord` spreads the members flat onto the record and keeps only the reasons for
   * the missing ones in a nested `holes`. The first draft of this file assumed a nested
   * `cells` instead, and the result was a screen that drew every value as a hole while
   * every instrument stayed green -- seven rows, thirty-two glyphs, no overflow, no
   * clipping, and not one fact on it. Read the shape; do not assume it.
   */
  const valueOf = (record, key) => record[key];
  const holeOf = (record, key) => record.holes?.[key];

  // -- type, asked for by purpose ---------------------------------------------

  const reading = (intent, words, extra = {}) => el('span', {
    'data-intent': intent,
    style: style({
      ...ink(intent),
      'font-family': metric('family.reading'),
      'font-size': metric('type.record'),
      'line-height': metric('type.record.line'),
      ...extra,
    }),
  }, [words]);

  const exact = (intent, words, extra = {}) => el('span', {
    'data-intent': intent,
    style: style({
      ...ink(intent),
      'font-family': metric('family.exact'),
      'font-size': metric('type.time'),
      'line-height': metric('type.time.line'),
      ...extra,
    }),
  }, [words]);

  const prose = (intent, words) => el('p', {
    'data-intent': intent,
    style: style({
      ...ink(intent),
      margin: `0 0 ${metric('gap.line')}`,
      'font-family': metric('family.reading'),
      'font-size': metric('type.record'),
      'line-height': metric('type.record.line'),
      'max-width': '68ch',
    }),
  }, [words]);

  /** A mark, always at a named floor and always carrying the word it stands for. */
  const mark = (intent, label, { act = false } = {}) => {
    const named = markOfIntent(intent);
    if (!named) return null;
    return P.glyph(named[0], named[1], { size: act ? P.minAct : P.minReadable, label });
  };

  // -- the head: one line, and the figures are the line ------------------------

  /**
   * The figures, inline. The screen this replaces drew five bordered tiles and then, on
   * the line above them, a sentence that said the same numbers again. The tiles were
   * furniture and the sentence was a second copy; what a reader needed was the numbers,
   * large, once. A half that was not read states neither a number nor a zero -- the
   * distinction the whole face is built around applies first to its own head.
   */
  function figure(noun, count, said) {
    const measured = typeof count === 'number';
    return el('span', {
      'data-role': 'figure',
      'data-noun': noun,
      'data-value': measured ? String(count) : null,
      title: said,
      style: style({
        display: 'inline-flex',
        'align-items': 'baseline',
        gap: metric('gap.hairline'),
        'margin-inline-end': metric('gap.block'),
        'white-space': 'nowrap',
      }),
    }, [
      el('span', {
        style: style({
          ...ink(measured ? 'measure.figure' : 'nothing.unknown'),
          'font-family': metric('family.exact'),
          'font-size': metric('type.figure'),
          'line-height': metric('type.figure.line'),
        }),
      }, [measured ? String(count) : P.statDash]),
      el('span', {
        style: style({
          ...ink('measure.label'),
          'font-family': metric('family.reading'),
          'font-size': metric('type.meta'),
          'line-height': metric('type.meta.line'),
          'letter-spacing': metric('track.label'),
        }),
      }, [noun]),
    ]);
  }

  function head(figures) {
    return el('header', {
      'data-part': 'ledger-head',
      style: style({
        display: 'flex',
        'align-items': 'baseline',
        'flex-wrap': 'wrap',
        gap: metric('gap.block'),
        padding: `${metric('gap.line')} 0 ${metric('gap.block')}`,
      }),
    }, [
      el('h1', {
        style: style({
          ...ink('measure.label'),
          margin: '0',
          'margin-inline-end': metric('gap.block'),
          'font-family': metric('family.reading'),
          'font-size': metric('type.head'),
          'line-height': metric('type.head.line'),
          'font-weight': WEIGHT.figure,
        }),
      }, [FACE_ID]),
      reading('measure.label', QUESTION, { 'margin-inline-end': 'auto' }),
      el('div', { 'data-role': 'figures' }, figures),
    ]);
  }

  // -- the one entrance --------------------------------------------------------

  /**
   * Everything explanatory, behind one door, with the count of what is behind it on the
   * door. Six doors each carrying a label and a subtitle put twelve pieces of text in
   * front of the first record; the rule that put them there says explanation lives
   * behind a click, and six clicks in a row is not behind a click, it is a second
   * screen standing in front of the first.
   */
  function aside(notes, open = false) {
    const kept = notes.filter((n) => n && n.body);
    if (kept.length === 0) return null;
    return el('details', {
      'data-part': 'ledger-aside',
      // Whether it is open is this window's decision and is carried in this window's
      // state, not left to the element: every repaint destroys the element, and a
      // reader who opened this and then chose a row used to watch it shut for no
      // reason they could see.
      'data-peripheral': 'about this screen',
      'data-open': String(Boolean(open)),
      open: open || null,
      'data-count': String(kept.length),
      style: style({
        'border-top': HAIRLINE,
        'border-bottom': HAIRLINE,
      }),
    }, [
      el('summary', {
        style: style({
          display: 'flex',
          'align-items': 'center',
          gap: metric('gap.line'),
          'min-height': metric('pitch.row'),
          ...ink('measure.label'),
          'font-family': metric('family.reading'),
          'font-size': metric('type.record'),
          'line-height': metric('type.record.line'),
        }),
      }, [
        mark('nothing.loading', 'about this screen'),
        el('span', {}, ['about this screen']),
        // The count sits in its own box with its own gap. Loose text beside a sibling
        // in a flex row was measured overlapping it by 161 square pixels -- a defect
        // that is invisible to read and that the shot probe found on the first run.
        el('span', {
          style: style({ ...ink('measure.figure'), 'margin-inline-start': metric('gap.line') }),
        }, [String(kept.length)]),
      ]),
      el('div', {
        'data-role': 'aside-body',
        style: style({ padding: `${metric('gap.block')} 0` }),
      }, kept.map((n) => el('section', {
        'data-note': n.key,
        style: style({ 'margin-block-end': metric('gap.block') }),
      }, [
        el('h2', {
          style: style({
            ...ink('measure.label'),
            margin: `0 0 ${metric('gap.hairline')}`,
            'font-family': metric('family.reading'),
            'font-size': metric('type.meta'),
            'line-height': metric('type.meta.line'),
            'letter-spacing': metric('track.label'),
          }),
        }, [n.key]),
        n.body,
      ]))),
    ]);
  }

  // -- a row -------------------------------------------------------------------

  /**
   * The track. Seven columns, all bounded, none of them able to grow a row taller: the
   * pitch is fixed and the overflow is hidden, so the tallest thing a row can be is one
   * line and there is no value anywhere that can argue with that.
   */
  /**
   * The track, and why the path is last.
   *
   * A clipped run of text keeps its full inline width in layout even when the paint is
   * hidden, so any column standing to the right of a long value is measured sitting
   * underneath it. That is not a cosmetic quibble: it is almost certainly why the screen
   * this replaces wrapped its paths, because wrapping is the only way to make a
   * left-hand column stop overlapping its neighbour, and wrapping is what made one row
   * five times the height of the others. The instrument was rewarding the defect.
   *
   * Putting the one unbounded value last removes the conflict rather than trading one
   * defect for the other: a long path can only run into the row's own clipped edge,
   * because there is nothing to its right to run into.
   */
  const TRACK = '0.9rem 4.75rem 4.5rem minmax(0,7rem) 6.5rem minmax(0,1fr)';

  /** One hairline, asked for by purpose, so no length is spelled at a call site. */
  const HAIRLINE = `${metric('edge.hairline')} solid ${ink('boundary.rule')['border-color']}`;

  // req/109: a cell names which column it is (`data-cell="at"`), the idiom the parts'
  // own slotCell already uses. A bare boolean here rendered as `data-cell=""`, which is
  // an address nothing can ask for -- and the copy affordance has to ask "which member
  // is under the pointer" of exactly this attribute (ledger.mjs onContextMenu).
  function cellOf(key, intent, words, { exactType = false, title = null } = {}) {
    const draw = exactType ? exact : reading;
    return el('span', {
      'data-cell': key,
      title,
      style: style({
        'min-width': '0',
        overflow: 'hidden',
        'white-space': 'nowrap',
        // Ellipsis rather than a bare cut. `clip` leaves the text box its full width
        // even though the paint is hidden, so the renderer reported five values sitting
        // under their neighbours; `ellipsis` shortens the run to the box, which is the
        // same policy honestly implemented. The declared character budget above decides
        // where the cut reads best; this decides that it always fits.
        'text-overflow': 'ellipsis',
      }),
    }, [draw(intent, words)]);
  }

  /** A member that is not there is drawn as a hole with its reason, never as blank. */
  function holeCell(key, why) {
    return el('span', {
      'data-cell': key,
      'data-hole': true,
      title: why,
      style: style({ 'min-width': '0', overflow: 'hidden', 'white-space': 'nowrap' }),
    }, [mark('nothing.absent', why)]);
  }

  function verdictCell(record) {
    const word = valueOf(record, 'verdict');
    if (word === undefined) return holeCell('verdict', holeOf(record, 'verdict') ?? MESSAGES.MEMBER_ABSENT);
    const known = ['Admit', 'Deny', 'Escalate'].includes(word);
    const intent = known ? `verdict.${word.toLowerCase()}` : 'verdict.unrecognised';
    const { shown } = cutEnd(word, BUDGET.verdict);
    return el('span', {
      'data-cell': 'verdict',
      title: known ? null : MESSAGES.VERDICT_UNRECOGNISED,
      style: style({
        display: 'flex',
        'align-items': 'center',
        gap: metric('gap.hairline'),
        'min-width': '0',
        overflow: 'hidden',
        'white-space': 'nowrap',
      }),
    }, [mark(intent, word), reading(intent, shown)]);
  }

  /**
   * The one hue on the screen, and it is only ever drawn when there is something to say.
   *
   * A row that has not been reversed draws nothing here rather than a chip saying so.
   * The face this replaces put a chip on every row -- `unknown` on most of them -- which
   * is a column of noise that says "we did not find out" over and over. What is unknown
   * is stated once, in the opened row, where it can also say why.
   */
  function standingCell(record, reversal, holes) {
    const marks = [];
    if (reversal && reversal.state === 'reversed') {
      marks.push(mark('reversal.reversed', 'reversed'), reading('reversal.reversed', 'reversed'));
    }
    // Holes, counted, and only when there are any.
    //
    // This column used to read `5 fields` on every settled row and `6 fields` on every
    // held one, which is not a measurement of anything: the member list is fixed, so the
    // number is a constant wearing a figure's clothes. What is worth a column is the
    // count of members that were looked for and not found -- which varies, which is a
    // fact about this record, and which is silent on the rows that have none.
    //
    // The seal hole is not counted. Nothing on the held half has happened, so "there is
    // nothing here to check" is true of every held row by construction: it is a property
    // of the half, and it is stated once on the half's own line.
    if (holes > 0) {
      marks.push(mark('nothing.absent', `${holes} looked for, not found`), reading('nothing.absent', String(holes)));
    }
    if (marks.length === 0) {
      return el('span', { 'data-cell': 'standing', 'data-standing': 'nothing to say' }, []);
    }
    return el('span', {
      'data-cell': 'standing',
      'data-standing': reversal?.state ?? 'none',
      title: reversal?.why ?? null,
      style: style({
        display: 'flex',
        'align-items': 'center',
        gap: metric('gap.hairline'),
        'min-width': '0',
        overflow: 'hidden',
        'white-space': 'nowrap',
      }),
    }, marks);
  }

  /**
   * The line. It is a button, because choosing a row is the only thing a row does at
   * rest, and a control that is the whole of the thing it acts on cannot be misaimed --
   * which is exactly what the gutter of `undo` buttons beside the old list was.
   */
  function rowLine(record, { open, reversal, holes }) {
    const path = valueOf(record, 'path');
    const pathCut = path === undefined ? null : cutMiddle(path, BUDGET.path);
    const effect = valueOf(record, 'effect');
    return el('button', {
      type: 'button',
      'data-part': 'ledger-row',
      // Two names for one identity, and both are load-bearing. `data-select-row` is
      // what the window's press handler reaches for; `data-row` is what the shot probe
      // counts rows by. Removing the second made the probe report zero rows on a screen
      // holding seven -- an instrument measuring nothing, which reads exactly like an
      // instrument finding nothing. Both live here and on nothing else in the row: the
      // acts carry `data-target` and the detail carries `data-detail-for`, because a
      // second element wearing the row's identity made one row count as two.
      'data-select-row': record.id,
      'data-row': record.id,
      'data-open': String(Boolean(open)),
      'data-clipped': pathCut?.cut ? 'true' : 'false',
      'aria-expanded': String(Boolean(open)),
      style: style({
        display: 'grid',
        'grid-template-columns': TRACK,
        'align-items': 'center',
        gap: metric('gap.line'),
        width: '100%',
        height: metric('pitch.row'),
        'box-sizing': 'border-box',
        padding: `0 ${metric('pad.side')}`,
        border: '0',
        'border-bottom': HAIRLINE,
        'border-radius': '0',
        'text-align': 'left',
        cursor: metric('cursor.act'),
        overflow: 'hidden',
        ...ink(open ? 'selection.open' : 'ground.page'),
      }),
    }, [
      // The spine: a child row says so here and nowhere else.
      record.childOf
        ? el('span', { 'data-cell': 'spine', title: MESSAGES.CHILD_ROW }, [mark('record.child', MESSAGES.CHILD_ROW)])
        : el('span', { 'data-cell': 'spine' }, []),
      valueOf(record, 'at') === undefined
        ? holeCell('at', holeOf(record, 'at') ?? MESSAGES.MEMBER_ABSENT)
        : cellOf('at', 'value.clipped', P.drawnTextFor('at', valueOf(record, 'at')), { exactType: true }),
      effect === undefined
        ? holeCell('effect', holeOf(record, 'effect') ?? MESSAGES.MEMBER_ABSENT)
        : cellOf('effect', 'value.clipped', cutEnd(effect, BUDGET.effect).shown),
      verdictCell(record),
      standingCell(record, reversal, holes),
      path === undefined
        ? holeCell('path', holeOf(record, 'path') ?? MESSAGES.MEMBER_ABSENT)
        // The whole value travels with the cut one. A value that is only ever shown cut
        // off is a record that has gone quiet, and this face exists to keep records from
        // going quiet -- so the full path is on the line as well as in the opened row,
        // and the line says which policy cut it.
        : cellOf('path', 'value.clipped', pathCut.shown, {
          exactType: true,
          title: pathCut.cut ? `${path}\n\n${MESSAGES.CLIP_ONE_POLICY}` : path,
        }),
    ]);
  }

  // -- what opens inside a row -------------------------------------------------

  /** The same two-column line, but the value is a drawing rather than a run of text. */
  function pairNode(name, node) {
    return el('div', {
      'data-role': 'detail-line',
      'data-name': name,
      style: style({
        display: 'grid',
        'grid-template-columns': 'minmax(0,8rem) minmax(0,1fr)',
        gap: metric('gap.line'),
        padding: `${metric('gap.hairline')} 0`,
      }),
    }, [reading('measure.label', name), node]);
  }

  function pair(name, value, intent = 'value.full') {
    return el('div', {
      'data-role': 'detail-line',
      'data-name': name,
      style: style({
        display: 'grid',
        'grid-template-columns': 'minmax(0,8rem) minmax(0,1fr)',
        gap: metric('gap.line'),
        padding: `${metric('gap.hairline')} 0`,
      }),
    }, [
      reading('measure.label', name),
      el('span', {
        style: style({ ...ink(intent), 'font-family': metric('family.exact'), 'font-size': metric('type.record'), 'line-height': metric('type.record.line'), 'overflow-wrap': 'break-word' }),
      }, [value]),
    ]);
  }

  /**
   * The acts, inside the row they act on, and nowhere else.
   *
   * A withheld act is drawn and dimmed with its reason rather than hidden, because "the
   * button does nothing" and "the button is not offered" are both worse than "the button
   * is dimmed and says why". That was already true of the face this replaces and is the
   * one thing about its acts worth keeping.
   */
  function actBar(record, half, sending) {
    const offered = ACTS.filter((a) => a.half === half);
    if (offered.length === 0) return null;
    return el('div', {
      'data-part': 'act-bar',
      'data-count': String(offered.length),
      style: style({
        display: 'flex',
        'flex-wrap': 'wrap',
        gap: metric('gap.line'),
        'padding-block-start': metric('gap.block'),
      }),
    }, offered.map((spec) => {
      const inFlight = sending?.has?.(`${spec.act}:${record.id}`) ?? false;
      const intent = spec.sends ? (inFlight ? 'act.inflight' : 'act.offered') : 'act.withheld';
      return el('button', {
        type: 'button',
        'data-role': 'act',
        'data-act': spec.act,
        'data-target': record.id,
        'data-sends': String(spec.sends),
        disabled: spec.sends && !inFlight ? null : '',
        title: spec.sends ? (inFlight ? MESSAGES.IN_FLIGHT : null) : spec.why,
        style: style({
          display: 'inline-flex',
          'align-items': 'center',
          gap: metric('gap.hairline'),
          'min-height': metric('pitch.row'),
          padding: `0 ${metric('pad.side')}`,
          'border-style': 'solid',
          'border-width': metric('edge.hairline'),
          'border-radius': metric('corner.control'),
          background: 'transparent',
          cursor: spec.sends ? metric('cursor.act') : metric('cursor.refuse'),
          'font-family': metric('family.reading'),
          'font-size': metric('type.record'),
          ...ink(intent),
        }),
      }, [P.glyph('act', spec.act, { size: P.minAct, label: spec.label }), spec.label]);
    }));
  }

  /**
   * The copy affordance, reimplemented for the rebuilt screen (req/109; req/893 D-8:
   * "copy was a real capability and its loss is a regression, not a simplification").
   *
   * The row menu the old screen drew was two things at once: a second act surface,
   * which D-8 rules was correct to remove, and the one place a reader could take a
   * member's whole value, which no ruling revoked. This is the second thing alone. It
   * is drawn under the row a reader right-clicked, only while a copyable cell was under
   * the pointer, and it carries exactly one control -- so acts still exist in exactly
   * one place (the opened row) and the D-8 tests that hold that at one keep holding it.
   *
   * The value offered is the member off the record and never the text the cell drew:
   * two of the columns draw a declared cut (the time of day of a timestamp, the middle
   * of a path), and a copy of what the cell says would hand back something that is not
   * the value, quietly. A member that is a declared hole keeps its control, dimmed,
   * wearing the hole's own reason -- a control that vanishes when it cannot be used is
   * indistinguishable from one that was never offered.
   */
  function rowMenu(record, offer) {
    const usable = offer.value !== null;
    return el('div', {
      'data-role': 'row-menu',
      'data-menu-row': record.id,
      style: style({
        display: 'flex',
        'align-items': 'center',
        gap: metric('gap.line'),
        padding: `${metric('gap.hairline')} ${metric('pad.side')}`,
        'border-bottom': HAIRLINE,
        ...ink('ground.opened'),
      }),
    }, [
      el('button', {
        type: 'button',
        'data-menu-item': 'copy',
        'data-copy-from': offer.from,
        'data-target': record.id,
        disabled: usable ? null : '',
        title: usable ? MESSAGES.COPY_WHOLE : offer.why,
        style: style({
          display: 'inline-flex',
          'align-items': 'center',
          gap: metric('gap.hairline'),
          'min-height': metric('pitch.row'),
          padding: `0 ${metric('pad.side')}`,
          'border-style': 'solid',
          'border-width': metric('edge.hairline'),
          'border-radius': metric('corner.control'),
          background: 'transparent',
          cursor: usable ? metric('cursor.act') : metric('cursor.refuse'),
          'font-family': metric('family.reading'),
          'font-size': metric('type.record'),
          ...ink(usable ? 'act.offered' : 'act.withheld'),
        }),
      }, [`copy ${offer.from}`]),
    ]);
  }

  /**
   * What the last copy did, stated rather than left to inference. A copy that reached
   * a clipboard and one that found no clipboard to write to end in the same silence,
   * and a reader who cannot tell them apart has been told the second one worked.
   */
  function copyReport(copied) {
    if (!copied) return null;
    const done = copied.state === 'copied';
    const failed = copied.state === 'refused';
    return el('div', {
      'data-role': 'copy-report',
      'data-copy-state': copied.state,
      'data-copied': done ? copied.from : null,
      'data-copy-failed': failed ? copied.from : null,
      style: style({
        padding: `${metric('gap.hairline')} 0`,
        'border-bottom': HAIRLINE,
      }),
    }, [
      reading('measure.label', done ? `${MESSAGES.COPIED}: ${copied.from}` : copied.why),
    ]);
  }

  /**
   * Everything the line could not hold, under the line that could not hold it.
   *
   * This is where the panel went. It exists only while a row is open, it is beside its
   * subject rather than across the screen from it, and when nothing is open it occupies
   * nothing at all -- which is the whole of the difference between this and a frame that
   * spent a third of the width announcing that it was empty.
   */
  function rowDetail(record, { half, reversal, sending }) {
    const lines = [];
    for (const name of MEMBER_KEYS) {
      const value = valueOf(record, name);
      if (value !== undefined) lines.push(pair(name, value));
    }
    for (const [name, why] of Object.entries(record.holes ?? {})) {
      lines.push(el('div', {
        'data-role': 'detail-line',
        'data-name': name,
        'data-hole': 'true',
        style: style({
          display: 'grid',
          'grid-template-columns': 'minmax(0,8rem) minmax(0,1fr)',
          gap: metric('gap.line'),
          padding: `${metric('gap.hairline')} 0`,
        }),
      }, [
        reading('measure.label', name),
        el('span', { style: style({ display: 'flex', 'align-items': 'center', gap: metric('gap.hairline') }) }, [
          mark('nothing.absent', why),
          reading('nothing.absent', why),
        ]),
      ]));
    }
    if (record.digest) {
      // The part that cannot let a serial travel alone: it draws the cut, how it was
      // cut, and the sentence saying a shared prefix is not a shared record, together or
      // not at all. The first draft called `cutOf` and printed the description of the
      // cut without the cut itself -- a line that explained a value it had not shown.
      lines.push(pairNode('serial', P.serial(record.digest, { size: P.minReadable })));
    }
    // The membrane's own boundary, drawn. This window worked the reversal out from the
    // rows it happened to be holding; the engine never said it. A screen that showed
    // that conclusion without saying whose it was would be claiming an engine's word
    // for a window's guess, which is the failure this product is built to make visible.
    if (reversal) {
      lines.push(el('div', {
        'data-role': 'detail-line',
        'data-name': 'reversal',
        'data-provenance': 'inferred-here',
        style: style({
          display: 'grid',
          'grid-template-columns': 'minmax(0,8rem) minmax(0,1fr)',
          gap: metric('gap.line'),
          padding: `${metric('gap.hairline')} 0`,
        }),
      }, [
        reading('measure.label', 'reversal'),
        el('span', { style: style({ display: 'flex', 'align-items': 'flex-start', gap: metric('gap.hairline') }) }, [
          mark('provenance.inferred-here', MESSAGES.INFERRED_HERE),
          reading('provenance.inferred-here', `${reversal.why} -- ${MESSAGES.INFERRED_HERE}`),
        ]),
      ]));
    }
    return el('div', {
      'data-part': 'row-detail',
      'data-detail-for': record.id,
      style: style({
        padding: `${metric('gap.block')} ${metric('pad.side')}`,
        'border-bottom': HAIRLINE,
        ...ink('ground.opened'),
      }),
    }, [...lines, actBar(record, half, sending)].filter(Boolean));
  }

  // -- a half ------------------------------------------------------------------

  function halfSection(half) {
    const children = [];
    children.push(el('div', {
      'data-part': 'half-head',
      style: style({
        display: 'flex',
        'align-items': 'baseline',
        'flex-wrap': 'wrap',
        gap: metric('gap.line'),
        padding: `${metric('gap.block')} ${metric('pad.side')} ${metric('gap.line')}`,
      }),
    }, [
      reading('measure.label', half.key),
      reading('value.clipped', half.denominator),
    ]));

    if (!half.read) {
      children.push(el('div', {
        'data-part': 'unread',
        style: style({ padding: `${metric('gap.block')} ${metric('pad.side')}`, display: 'flex', gap: metric('gap.line') }),
      }, [
        mark('reading.unread', MESSAGES.UNREAD),
        prose('reading.unread', half.why),
      ]));
    } else if (half.rows.length === 0) {
      children.push(el('div', {
        'data-part': 'empty',
        style: style({ padding: `${metric('gap.block')} ${metric('pad.side')}` }),
      }, [prose('nothing.false', half.empty)]));
    } else {
      for (const entry of half.rows) {
        children.push(rowLine(entry.record, entry));
        // req/109: the take strip, under its row, only while this window's menu state
        // names this row and a copyable cell (the model resolved the offer; a row with
        // no menu carries nothing extra, so a screen at rest is byte-identical).
        if (entry.menu) children.push(rowMenu(entry.record, entry.menu));
        if (entry.open) children.push(rowDetail(entry.record, { half: half.key, reversal: entry.reversal, sending: half.sending }));
      }
    }

    for (const dropped of half.dropped ?? []) {
      children.push(el('div', {
        'data-part': 'dropped',
        style: style({ padding: `${metric('gap.hairline')} ${metric('pad.side')}`, display: 'flex', gap: metric('gap.hairline') }),
      }, [mark('nothing.absent', dropped.why), reading('nothing.absent', dropped.why)]));
    }

    return el('section', { 'data-part': 'half', 'data-half': half.key }, children);
  }

  // -- the screen --------------------------------------------------------------

  function view(model) {
    return el('main', {
      'data-face': FACE_ID,
      style: style({
        display: 'block',
        padding: `0 ${metric('pad.spine')}`,
        ...ink('ground.page'),
      }),
    }, [
      head(model.figures.map((f) => figure(f.noun, f.count, f.said))),
      copyReport(model.copied),
      aside(model.notes, model.asideOpen),
      ...model.halves.map(halfSection),
      model.actLog,
    ].filter(Boolean));
  }
  // The strip at the foot is appended by the caller, after it has measured. A figure
  // that included the drawing of itself would be written before the thing it measured
  // had finished happening, and this face states a cost it actually took.

  return { view, figure, rowLine, rowDetail, rowMenu, copyReport, halfSection, aside, head, prose, reading, mark, pair, BUDGET, cutMiddle, cutEnd, TRACK, find };
}
