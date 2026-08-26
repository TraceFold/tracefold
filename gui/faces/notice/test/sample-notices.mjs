// SPDX-License-Identifier: Apache-2.0
// Entries in the two shapes this face is actually handed, spelled out rather than
// built with the membrane or the shell -- a test that passes here and disagrees with
// what those two modules actually push says the two disagree, instead of silently
// agreeing with whatever they happen to produce today.
//
// Shape one (`membrane/src/membrane.mjs` `note`): no `through` field, carries `verb`
// and `path`, and a `result` holding the full envelope wire.mjs's four outcome
// builders produce.
//
// Shape two (`shell/kernel/shell.mjs` `watch`/`act`): `through: 'shell'`, no `verb`
// or `path`, and for a refused or elsewhere act a `said` string instead of a
// `result`.

let seq = 0;
const nextSeq = () => { seq += 1; return seq; };
export const resetSeq = () => { seq = 0; };

export function asked(method) {
  return { seq: nextSeq(), at: '2026-08-24T10:00:00.000Z', through: 'shell', method, outcome: 'asked' };
}

export function answered(method, verb, path, status = 200, body = {}) {
  return {
    seq: nextSeq(), at: '2026-08-24T10:00:01.000Z', method, verb, path, outcome: 'answered', status, result: { outcome: 'answered', status, body },
  };
}

export function refused(method, verb, path, problem, status = 409) {
  return {
    seq: nextSeq(),
    at: '2026-08-24T10:00:02.000Z',
    method,
    verb,
    path,
    outcome: 'refused',
    status,
    result: { outcome: 'refused', status, gx_code: problem.gx_code ?? null, problem },
  };
}

export function failed(method, verb, path, reason = 'transport', detail = 'the socket was closed before an answer arrived') {
  return {
    seq: nextSeq(), at: '2026-08-24T10:00:03.000Z', method, verb, path, outcome: 'failed', status: null, result: { outcome: 'failed', reason, status: null, detail },
  };
}

export function absent(method, requested) {
  return {
    seq: nextSeq(), at: '2026-08-24T10:00:04.000Z', method, verb: null, path: null, outcome: 'absent', status: null, result: { outcome: 'absent', reason: 'no_such_route', requested },
  };
}

export function shellRefused(verb, said) {
  return {
    seq: nextSeq(), at: '2026-08-24T10:00:05.000Z', through: 'shell', method: verb, outcome: 'refused', said,
  };
}

export function shellElsewhere(verb, said) {
  return {
    seq: nextSeq(), at: '2026-08-24T10:00:06.000Z', through: 'shell', method: verb, outcome: 'elsewhere', said,
  };
}

/** A word the wire has never produced. Every screen must draw this without judging
 * whether it should exist -- this face is not the layer that checks the vocabulary. */
export function unrecognised(method) {
  return {
    seq: nextSeq(), at: '2026-08-24T10:00:07.000Z', method, verb: 'PATCH', path: '/v1/nowhere', outcome: 'partially_answered', status: 207,
  };
}

/** A representative window: one of every shape this face has to tell apart. */
export function representative() {
  resetSeq();
  return [
    asked('get_transformations'),
    answered('get_transformations', 'GET', '/v1/transformations', 200, { items: [] }),
    refused('post_candidates_id_commit', 'POST', '/v1/candidates/{id}/commit', {
      type: 'about:blank', title: 'conflict', status: 409, detail: 'this candidate was already committed', gx_code: 'IDEMPOTENCY_CONFLICT',
    }),
    failed('get_candidates', 'GET', '/v1/candidates'),
    absent('get_everything_i_wish_for', { name: 'get_everything_i_wish_for' }),
    shellRefused('pane:divide', 'there is no act called "pane:divide" in this space'),
    shellElsewhere('theme:set', 'theme:set belongs to a different screen'),
    unrecognised('get_transformations'),
    'this entry is not a record',
    42,
  ];
}
