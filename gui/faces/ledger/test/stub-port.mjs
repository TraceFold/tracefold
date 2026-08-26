// SPDX-License-Identifier: Apache-2.0
// A port that answers without a network, and writes down what it was asked.
//
// The face is handed a port and never builds one, so a stub is the whole of what a
// unit test needs. The shapes here are the membrane's four outcomes and its folded
// list envelope, spelled out rather than imported, so that a test which passes here
// and fails against the real membrane says the two disagree instead of silently
// agreeing with whatever the membrane happens to return today. The declaration test
// checks the method names against the membrane's own route table, which is where
// that agreement is actually enforced.

export const ANSWERED = 'answered';
export const REFUSED = 'refused';
export const FAILED = 'failed';
export const ABSENT = 'absent';

export function page(items, { pages = 1, stopped = false, repeated = false, budget = 64 } = {}) {
  return {
    outcome: ANSWERED,
    items,
    requests: pages,
    pages,
    stopped_at_budget: stopped,
    repeated_cursor: repeated,
    budget,
  };
}

export function answered(body, status = 200) {
  return { outcome: ANSWERED, status, body };
}

export function refused(problem, status = 409) {
  return { outcome: REFUSED, status, gx_code: problem.gx_code ?? null, problem };
}

export function failed(reason = 'transport', detail = 'the socket was closed before an answer arrived') {
  return { outcome: FAILED, reason, status: null, detail };
}

export function absent(requested) {
  return { outcome: ABSENT, reason: 'no_such_route', requested };
}

/**
 * @param {object} answers  method name -> result, or (input, call) -> result
 * @param {object} options
 * @param {string[]} [options.methods] names the port carries at all
 */
export function stubPort(answers = {}, { methods = null } = {}) {
  const calls = [];
  const names = methods ?? Object.keys(answers);
  const port = {};

  const reply = (name, input) => {
    calls.push({ name, input });
    const answer = answers[name];
    if (typeof answer === 'function') return Promise.resolve(answer(input, calls.length));
    if (answer === undefined) return Promise.resolve(absent({ name }));
    return Promise.resolve(answer);
  };

  for (const name of names) port[name] = (input) => reply(name, input);
  port.fold = (name, input) => reply(name, input);
  port.routes = () => names.map((name) => ({ name }));
  port.ledgers = () => ({ NOT_CONSUMED: [], NOT_DRAWN: [], NOT_A_ROUTE: [] });
  port.calls = calls;
  return port;
}

/** Names used by more than one test, kept in one place so a rename is one edit. */
export const SAMPLE = Object.freeze({
  transformation: (n, extra = {}) => ({
    id: `t-${String(n).padStart(3, '0')}`,
    sequence: n,
    prev: n > 1 ? `t-${String(n - 1).padStart(3, '0')}` : null,
    at: `2026-08-24T09:${String(n).padStart(2, '0')}:00Z`,
    actor: 'agent:packer',
    effect: 'write',
    verdict: 'Admit',
    path: `/work/report-${n}.md`,
    digest: `a1b2c3d4e5f6${String(n).padStart(4, '0')}`,
    basis: 'exact',
    ...extra,
  }),
  candidate: (n, extra = {}) => ({
    id: `c-${String(n).padStart(3, '0')}`,
    sequence: n,
    at: `2026-08-24T10:${String(n).padStart(2, '0')}:00Z`,
    actor: 'agent:packer',
    effect: 'write',
    verdict: 'Escalate',
    path: `/work/pending-${n}.md`,
    digest: `f6e5d4c3b2a1${String(n).padStart(4, '0')}`,
    ...extra,
  }),
});
