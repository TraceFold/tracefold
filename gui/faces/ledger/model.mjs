// SPDX-License-Identifier: Apache-2.0
//
// The facts a screen is drawn from, worked out once and handed over.
//
// This exists so that the two halves of "what is true" and "what it looks like" can be
// wrong separately. The face this replaces computed both in the same functions, which is
// why a defect in the picture -- eleven buttons in a gutter -- could not be discussed
// without reading the code that decides what a row is. Here the model holds no colour,
// no length and no node except the ones it is handed for prose, and ./screen.mjs holds
// no decision about what is true.
//
// Nothing here concludes anything the engine is the authority on. It counts rows the
// engine returned, it reads words the engine wrote, and where it does draw a conclusion
// of its own -- exactly once, for reversal -- it marks it as this window's and says so
// on the screen.

export function createModel(P, screen, {
  MESSAGES, ORDER, ROWS, UNDRAWN, ACTS, HALF, ANSWERED, toRecord, MEMBER_KEYS, MARKS = [],
  // req/109: what a copy of a cell would actually take, or null for a cell that holds
  // no member. Resolved by the face (ledger.mjs copyOffer, the same function the
  // clipboard write reads), so the strip that offers a value and the queue that takes
  // it cannot disagree about what the value is.
  copyOfferOf = () => null,
}) {
  const { el, style, find } = P.element;
  const { prose, reading, pair } = screen;

  const nounFor = (count, one, many) => (count === 1 ? one : many);

  /**
   * One act entry, in words. req/108: the engine's own code (`gx_code`) travels with
   * the outcome -- describeAct() had been carrying it in `entry.code` and both places
   * that write an act to the screen were dropping it, so a refused act reached a
   * reader without the one machine word the engine gave for the refusal.
   */
  const actWords = (entry) => `${entry.outcome}${entry.code ? ` ${entry.code}` : ''}: ${entry.detail ?? ''}`;

  /**
   * A half's denominator, in one line.
   *
   * Never a bare count. "five records" reads the same whether the list ended or the walk
   * stopped at its budget, and those are different facts about whether anything is
   * hidden. Drawn, received, requests and whether the walk stopped, always together.
   */
  function denominatorOf(ordered, envelope, key) {
    const drawn = ordered.rows.length;
    const received = Array.isArray(envelope.items) ? envelope.items.length : 0;
    const noun = key === HALF.settled ? nounFor(drawn, 'record', 'records') : nounFor(drawn, 'candidate', 'candidates');
    const bits = [`${drawn} of ${received} ${noun}`];
    if (typeof envelope.pages === 'number') bits.push(`${envelope.pages} ${nounFor(envelope.pages, 'request', 'requests')}`);
    // req/108: the wire's names, read off the fold envelope the membrane actually
    // returns (membrane/src/pages.mjs: `stopped_at_budget`, `repeated_cursor`). This
    // used to read `envelope.stopped` and `envelope.repeated` -- fields no envelope
    // carries -- so a truncated walk was never announced on the live path. Asking the
    // wire for a field it does not have is the defect class req/893's own AC-8 exists
    // to keep visible.
    if (envelope.stopped_at_budget) bits.push(MESSAGES.TRUNCATED);
    if (envelope.repeated_cursor) bits.push(MESSAGES.REPEATED);
    return bits.join(' · ');
  }

  function halfModel(state, key, selected, sending) {
    const envelope = state[key];
    const empty = key === HALF.settled ? MESSAGES.EMPTY_SETTLED : MESSAGES.EMPTY_HELD;

    if (!envelope || envelope.outcome !== ANSWERED) {
      const why = [
        `${MESSAGES.UNREAD}.`,
        `outcome: ${envelope?.outcome ?? 'nothing came back at all'}`,
        envelope?.outcome === 'refused' ? `${envelope.problem?.title ?? ''}: ${envelope.problem?.detail ?? ''} (${envelope.gx_code ?? envelope.problem?.gx_code ?? 'no code'}, status ${envelope.status ?? 'none'})` : null,
        envelope?.outcome === 'failed' ? `${envelope.reason}: ${envelope.detail ?? ''}` : null,
        envelope?.outcome === 'absent' ? `${envelope.reason}: ${JSON.stringify(envelope.requested ?? null)}` : null,
      ].filter(Boolean).join(' ');
      return {
        key, read: false, why, empty, rows: [], dropped: [], sending,
        denominator: MESSAGES.UNREAD_DENOMINATOR, drawn: null, ordered: null,
      };
    }

    const items = Array.isArray(envelope.items) ? envelope.items : [];
    const ordered = P.order(items.map((item) => toRecord(item, key)), { by: ROWS.order });
    // req/109: this window's menu decision, resolved to an offer only for the row it
    // names and only when a cell was under the pointer. A right-press on the row
    // itself carries `cell: null` and resolves to nothing -- which is what keeps the
    // req/893 D-8 tests true: a right-click on a row opens no second act surface.
    const menuState = state.menu && state.menu.cell ? state.menu : null;
    const rows = ordered.rows.map((record) => {
      // Members looked for and not found. The seal hole is excluded: it is true of every
      // held row by construction, so counting it would put the same number on every row
      // of a half and call it a measurement.
      const holes = Object.keys(record.holes ?? {}).filter((k) => k !== 'seal').length;
      return {
        record,
        open: selected === record.id,
        // The one conclusion this window draws for itself. Only kept when it is
        // something -- an "unknown" on every row is a column of noise, and what is
        // unknown is said once, inside the row, where there is room to say why.
        reversal: P.reversalOf(record, ordered.rows),
        holes,
        menu: menuState && menuState.id === record.id ? copyOfferOf(record, menuState.cell) : null,
      };
    });
    return {
      key,
      read: true,
      why: null,
      empty,
      rows,
      dropped: (ordered.dropped ?? []).map((d) => ({ why: typeof d === 'string' ? d : (d.why ?? d.reason ?? String(d)) })),
      sending,
      denominator: denominatorOf(ordered, envelope, key),
      drawn: ordered.rows.length,
      ordered,
    };
  }

  /** The five figures. A half that was not read states a dash, never a zero. */
  function figuresOf(halves) {
    const settled = halves.find((h) => h.key === HALF.settled);
    const held = halves.find((h) => h.key === HALF.held);
    const countVerdict = (word) => (settled.read
      ? settled.rows.filter((entry) => entry.record.verdict === word).length
      : null);
    return [
      { noun: 'settled', count: settled.read ? settled.drawn : null, said: MESSAGES.BOX_SETTLED },
      { noun: 'admit', count: countVerdict('Admit'), said: 'settled rows the engine answered Admit for' },
      { noun: 'deny', count: countVerdict('Deny'), said: 'settled rows the engine answered Deny for' },
      { noun: 'escalate', count: countVerdict('Escalate'), said: 'settled rows the engine answered Escalate for' },
      { noun: 'held', count: held.read ? held.drawn : null, said: MESSAGES.BOX_HELD },
    ];
  }

  /**
   * Everything explanatory, as bodies for the one entrance.
   *
   * All six are still here and all six still say what they said. What changed is that
   * they are behind one door rather than six, and none of them carries a subtitle
   * advertising itself in front of the data.
   */
  function notesOf(state, halves, drawnMarks, cuts) {
    const settled = halves.find((h) => h.key === HALF.settled);
    const claims = settled.read && settled.ordered
      ? P.checkable(settled.ordered.rows, [])
      : [];
    const consistency = state.consistency;
    return [
      { key: 'why', body: prose('value.full', ORDER.reason) },
      {
        key: 'order',
        body: el('div', {}, [
          prose('value.full', ROWS.order_reason),
          settled.ordered?.substituted ? prose('nothing.unknown', settled.ordered.reason) : null,
        ].filter(Boolean)),
      },
      {
        key: 'legend',
        // req/108: zero-inclusive over the declared marks (req/768 F-B). A declared
        // mark this render drew none of still gets its line, with a zero -- a legend
        // that only lists what happened to be drawn cannot say "this mark exists and
        // is absent here", which is the absent/false distinction the face itself draws.
        body: el('div', {}, MARKS.map((m) => pair(m.mark, String(drawnMarks.get(m.mark) ?? 0), 'measure.figure'))),
      },
      {
        key: 'claims',
        body: el('div', {}, claims.length === 0
          ? [prose('nothing.unknown', MESSAGES.NO_CLAIMS)]
          : claims.map((c) => pair(c.holds ? 'holds' : 'does not hold', `${c.claim} (${c.detail})`))),
      },
      {
        key: 'consistency',
        body: el('div', {}, [
          prose('value.full', MESSAGES.NOT_VERIFICATION),
          consistency?.outcome === ANSWERED
            ? pair('engine says', `consistent: ${consistency.body?.consistent}, checked ${consistency.body?.checked_from} to ${consistency.body?.checked_to}`)
            : pair('engine says', `${MESSAGES.UNREAD} (${consistency?.outcome ?? 'nothing came back at all'})`, 'nothing.unknown'),
          prose('value.full', MESSAGES.NO_VERIFIER_HERE),
        ]),
      },
      {
        key: 'omitted',
        body: el('div', {}, UNDRAWN.map((u) => pair(u.what, u.why))),
      },
      // Every value the line cut, written out whole.
      //
      // The line clips and the opened row holds the whole value, but a reader who has
      // opened nothing can still be looking at a path they cannot finish reading. A
      // value that is only ever shown cut off is a record that has gone quiet, so the
      // full text of every cut value is on this screen whether or not anything is open.
      // It is a list that is empty on most reads, which is the difference between this
      // and the sentence the panel used to spend a third of the width on.
      ...(cuts.length === 0 ? [] : [{
        key: 'cut on this line',
        body: el('div', {}, cuts.map((c) => pair(c.id, c.value))),
      }]),
      {
        key: 'where from',
        body: el('div', {}, [
          pair('read', state.source ?? MESSAGES.SOURCE_ENGINE),
          ...(state.acts ?? []).map((a) => pair(`${a.act} ${a.id}`, actWords(a))),
        ]),
      },
    ];
  }

  /**
   * The values the line will cut, asked of the screen rather than guessed at here.
   *
   * The screen owns the budget, so the model asks it what it is going to do rather than
   * keeping a second copy of the number. A second copy is how the two drift, and a list
   * of "what was cut" that disagreed with what was cut would be worse than no list.
   */
  function cutsOf(halves) {
    const out = [];
    for (const half of halves) {
      for (const entry of half.rows) {
        const path = entry.record.path;
        if (path === undefined) continue;
        if (screen.cutMiddle(path, screen.BUDGET.path).cut) out.push({ id: entry.record.id, value: path });
      }
    }
    return out;
  }

  /** What this window has sent, kept out of the list because it is not a record. */
  function actLogOf(acts) {
    if (!acts || acts.length === 0) return null;
    return el('section', { 'data-part': 'act-log', 'data-count': String(acts.length) },
      acts.map((entry) => pair(`${entry.act} ${entry.id}`, actWords(entry),
        entry.outcome === ANSWERED ? 'value.full' : 'nothing.unknown')));
  }

  function build(state, renderMs) {
    const selected = state.selected ?? null;
    const sending = state.sending instanceof Set ? state.sending : new Set();
    const halves = [
      halfModel(state, HALF.settled, selected, sending),
      halfModel(state, HALF.held, selected, sending),
    ];
    const figures = figuresOf(halves);

    // The legend counts what this render is about to draw, so it is built from the
    // drawn tree rather than from a table of what could be drawn. A legend that
    // reported a mark nobody can see, or missed one they can, would be a second
    // vocabulary standing beside the real one.
    const drawnMarks = new Map();
    const provisional = [
      ...halves.map((h) => screen.halfSection(h)),
    ];
    for (const node of provisional) {
      for (const marked of find(node, (n) => n.attrs && 'data-mark' in n.attrs)) {
        const name = marked.attrs['data-mark'];
        drawnMarks.set(name, (drawnMarks.get(name) ?? 0) + 1);
      }
    }

    return {
      figures,
      halves,
      notes: notesOf(state, halves, drawnMarks, cutsOf(halves)),
      asideOpen: (Array.isArray(state.opened) ? state.opened : []).includes('about this screen'),
      actLog: actLogOf(state.acts),
      // req/109: what the last copy did. Carried whole; the screen decides the words.
      copied: state.copied ?? null,
      source: state.source ?? null,
      renderMs,
    };
  }

  return { build, halfModel, figuresOf, notesOf, denominatorOf };
}
