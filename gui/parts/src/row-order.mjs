// SPDX-License-Identifier: Apache-2.0
// M2 -- the order rows are in, and the reason they are in it.
//
// Decides; draws nothing.
//
// An order that nobody chose is still an order. Sorting by id and calling the result
// chronological works until an id stops carrying a timestamp, and then the ledger
// quietly lies in a way no test notices. So `by` is required: there is no default
// order, and asking for one raises rather than picking.
//
// Two orders here depend on comparing strings and getting time out of it, which is
// only true when the strings are fixed width, zero padded and written in one zone.
// Those three conditions are checked against the data rather than assumed, and when
// they do not hold the records are handed back in the order they arrived with the
// substitution stated. Falling back silently to a plausible-looking sort is the
// failure this guards.
//
// Rows that cannot be drawn are dropped with a word for why, not with a boolean
// (req/04a M2). A count of drops tells you something is wrong; the words tell you
// which thing.

export const ORDER_MESSAGES = {
  ORDER_REQUIRED: 'rows are put in a stated order; there is no default',
  UNKNOWN_ORDER: 'no such order',
  SUBSTITUTED: 'the requested order rests on an assumption this data breaks, so the recorded order was kept',
  ORDERED: 'rows ordered',
};

export const DROP_REASONS = Object.freeze({
  NOT_AN_OBJECT: 'not-a-record',
  NO_IDENTITY: 'no-identity',
});

export const ORDERS = Object.freeze({
  'as-recorded': {
    reason: 'the order they arrived in, which is the only order that needs no assumption',
    assumes: [],
    compare: null,
  },
  'by-sequence': {
    reason: 'ascending sequence number, because the issuer assigns it in the order it issued',
    assumes: ['every-record-has-a-sequence'],
    compare: (a, b) => (a.n - b.n) || String(a.id).localeCompare(String(b.id)),
  },
  'by-time-then-id': {
    reason: 'ascending recorded time, with identity breaking ties so the order is total',
    assumes: ['times-are-fixed-width', 'times-are-zero-padded', 'times-share-one-zone'],
    compare: (a, b) => String(a.at).localeCompare(String(b.at)) || String(a.id).localeCompare(String(b.id)),
  },
});

function dropReasonFor(record) {
  if (record === null || typeof record !== 'object' || Array.isArray(record)) return DROP_REASONS.NOT_AN_OBJECT;
  if (typeof record.id !== 'string' || record.id === '') return DROP_REASONS.NO_IDENTITY;
  return null;
}

const ZONE = /(Z|[+-]\d{2}:?\d{2})$/;

/** The three things a string comparison has to be true about before it means time. */
function timeAssumptions(records) {
  const times = records.map((r) => r.at).filter((t) => typeof t === 'string');
  const widths = new Set(times.map((t) => t.length));
  const zones = new Set(times.map((t) => (ZONE.exec(t) ?? ['(none)'])[0]));
  const padded = times.every((t) => /^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}/.test(t));
  return [
    { name: 'times-are-fixed-width', holds: times.length === records.length && widths.size <= 1, detail: `widths: ${[...widths].join(', ') || 'none'}; ${times.length} of ${records.length} records carry a time` },
    { name: 'times-are-zero-padded', holds: times.length > 0 && padded, detail: padded ? 'every time is written to a fixed field' : 'at least one time is not written to a fixed field' },
    { name: 'times-share-one-zone', holds: zones.size <= 1, detail: `zones seen: ${[...zones].join(', ') || 'none'}` },
  ];
}

function sequenceAssumptions(records) {
  const numbered = records.filter((r) => Number.isInteger(r.n));
  return [{
    name: 'every-record-has-a-sequence',
    holds: numbered.length === records.length,
    detail: `${numbered.length} of ${records.length} records carry a whole sequence number`,
  }];
}

function assumptionsFor(by, records) {
  if (by === 'by-time-then-id') return timeAssumptions(records);
  if (by === 'by-sequence') return sequenceAssumptions(records);
  return [];
}

/** Values that appear twice where they were meant to appear once. */
export function repeated(records, field) {
  const seen = new Map();
  for (const record of records) {
    const value = record?.[field];
    if (value === undefined || value === null) continue;
    seen.set(value, (seen.get(value) ?? 0) + 1);
  }
  return [...seen.entries()].filter(([, count]) => count > 1).map(([value, count]) => ({ value, count }));
}

export function order(records, { by } = {}) {
  if (by === undefined || by === null) throw new Error(ORDER_MESSAGES.ORDER_REQUIRED);
  const spec = ORDERS[by];
  if (!spec) throw new Error(`${ORDER_MESSAGES.UNKNOWN_ORDER}: ${by}`);

  const incoming = Array.isArray(records) ? records : [];
  const dropped = [];
  const kept = [];
  incoming.forEach((record, index) => {
    const why = dropReasonFor(record);
    if (why) dropped.push({ index, id: record?.id ?? null, why });
    else kept.push(record);
  });

  const assumptions = assumptionsFor(by, kept);
  const broken = assumptions.filter((a) => !a.holds);
  const substituted = broken.length > 0;
  const applied = substituted ? 'as-recorded' : by;
  const compare = ORDERS[applied].compare;

  return {
    rows: compare ? [...kept].sort(compare) : kept,
    requested: by,
    by: applied,
    substituted,
    reason: substituted ? `${ORDER_MESSAGES.SUBSTITUTED}: ${broken.map((a) => a.name).join(', ')}` : ORDERS[applied].reason,
    assumptions,
    dropped,
    repeated: { id: repeated(kept, 'id'), n: repeated(kept, 'n') },
  };
}
