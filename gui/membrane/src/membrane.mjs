// SPDX-License-Identifier: Apache-2.0
// The membrane. The only place in this app that touches a network.
//
// There is no hand-written method here. Every callable comes from a row of
// route-table.json, whose wire half is extracted from the crate that serves it, so a
// route the server does not have cannot be called and a route it gains cannot be
// forgotten. That is also why the names look mechanical: a name is a function of the
// verb and the path, so no hand gets to decide one, and nothing about a route can be
// asserted here that the table does not say.
//
// The membrane states nothing of its own. It carries the server's words up and the
// caller's words down, attaches identity, and classifies what came back. It does not
// verify, does not repair a spelling, and does not drop a member.

import { buildUrl } from './address.mjs';
import { send } from './transport.mjs';
import { stableKey } from './idempotency.mjs';
import { foldPages } from './pages.mjs';
import { translateFold } from './vocabulary.mjs';
import { deriveLedgers } from './ledgers.mjs';
import {
  OUTCOME, ABSENCE, AUTH_HEADER, AUTH_SCHEME, IDEMPOTENCY_HEADER, JSON_MEDIA_TYPE, absent,
} from './wire.mjs';

// The three tables arrive as a module rather than as three `readFileSync` calls, and the
// reason is not tidiness. A module that reads a file at import time cannot be imported by
// a browser, and this is the one module a window must import in order to draw a row that
// came from the server -- so that single `node:fs` line was, by itself, the whole of
// req/803's C3 = 0/16. The JSON files beside this one are still the source; `tools/
// gen_tables.mjs` writes them into `tables.gen.mjs` and `test/tables_gen.test.mjs` fails
// when the two disagree, so nothing here can drift away from the crate's own route table.
import { TABLE, COVERAGE, WIRE_FIELDS } from './tables.gen.mjs';

export { TABLE, COVERAGE, WIRE_FIELDS };

/** A name is derived, never chosen: verb, then the path's segments, parameters included. */
export function methodName(verb, path) {
  const parts = path
    .split('/')
    .filter(Boolean)
    .map((segment) => {
      const parameter = /^\{(.+)\}$/.exec(segment);
      return (parameter ? parameter[1] : segment).replace(/-/g, '_');
    });
  return [String(verb).toLowerCase(), ...parts].join('_');
}

export function tableRows(table = TABLE) {
  return table.routes.map((row) => ({ ...row, name: methodName(row.verb, row.path) }));
}

/**
 * @param {object} options
 * @param {string} options.origin           e.g. http://127.0.0.1:8787
 * @param {string} [options.token]          the server's bearer token
 * @param {object} [options.actor]          gx-core Actor, externally tagged
 * @param {Function} [options.fetchImpl]
 * @param {Array} [options.notices]         the ledger every call is written into
 */
export function createMembrane({
  origin,
  token = '',
  actor = null,
  fetchImpl = globalThis.fetch,
  table = TABLE,
  coverage = COVERAGE,
  fields = WIRE_FIELDS.fields,
  notices = [],
} = {}) {
  if (!origin) throw new TypeError('origin is required');
  const rows = tableRows(table);
  const byName = new Map(rows.map((row) => [row.name, row]));
  const byPair = new Map(rows.map((row) => [`${row.verb} ${row.path}`, row]));

  function note(entry) {
    notices.push({ seq: notices.length + 1, at: Date.now(), ...entry });
    return entry.result;
  }

  async function perform(row, input = {}) {
    const { params = {}, query = {}, body: given } = input;
    const headers = { accept: JSON_MEDIA_TYPE };
    if (row.auth === 'bearer') headers[AUTH_HEADER] = `${AUTH_SCHEME}${token}`;

    let payload = null;
    if (row.accepts_body) {
      const draft = { ...(given ?? {}) };
      if (row.actor !== 'none') {
        // A caller cannot name who is acting. An identity a face may state is an
        // identity a face may state falsely, and nothing downstream could tell.
        if (given && 'actor' in given) {
          throw new TypeError(`${row.name}: the actor is attached by the membrane; a caller may not name one`);
        }
        if (row.actor === 'required' && !actor) {
          throw new TypeError(`${row.name}: this route requires an actor and the membrane was built without one`);
        }
        if (actor) draft.actor = actor;
      }
      payload = JSON.stringify(draft);
      headers['content-type'] = JSON_MEDIA_TYPE;
    } else if (given !== undefined) {
      throw new TypeError(`${row.name}: this route takes no body`);
    }

    if (row.idempotency === 'accepts') {
      headers[IDEMPOTENCY_HEADER] = await stableKey(row.name, params.id ?? params.tid ?? null);
    }

    const url = buildUrl(origin, row.path, params, query);
    const result = await send(fetchImpl, {
      url,
      verb: row.verb,
      headers,
      body: payload,
      expects: row.kind === 'stream' ? 'stream' : 'json',
    });
    return note({ method: row.name, verb: row.verb, path: row.path, outcome: result.outcome, status: result.status ?? null, result });
  }

  const port = {};
  for (const row of rows) {
    port[row.name] = (input) => perform(row, input);
  }

  /**
   * The one way to reach a route by its address rather than by its name, and the one
   * place `absent` is produced. An address the table does not hold is answered here,
   * before any socket is opened -- which is how a method for a route that has never
   * existed becomes impossible rather than merely discouraged.
   */
  port.direct = async ({ verb, path, ...input }) => {
    const row = byPair.get(`${String(verb).toUpperCase()} ${path}`);
    if (!row) {
      const result = absent(ABSENCE.NO_SUCH_ROUTE, { verb, path });
      return note({ method: null, verb, path, outcome: result.outcome, status: null, result });
    }
    return perform(row, input);
  };

  /** Read a list to its end, carrying the walk's own denominator back up. */
  port.fold = async (name, input = {}, options = {}) => {
    const row = byName.get(name);
    if (!row) {
      const result = absent(ABSENCE.NO_SUCH_ROUTE, { name });
      return note({ method: name, verb: null, path: null, outcome: result.outcome, status: null, result });
    }
    if (row.kind !== 'list') throw new TypeError(`${name} is not a list route`);
    const folded = await foldPages(
      (cursor) => perform(row, {
        ...input,
        query: { ...(input.query ?? {}), ...(cursor === undefined ? {} : { cursor }) },
      }),
      options,
    );
    // req/822_c6: the declared row-vocabulary projection (vocabulary.mjs — additive,
    // observed routes only). This is the one line between "the engine answered" and
    // "a face drew zero of its rows" that c5 §4 measured.
    return translateFold(row.name, folded);
  };

  /** The three declarations of what is not covered, computed on the spot. */
  port.ledgers = () => deriveLedgers({ routes: rows, coverage, fields });

  port.routes = () => rows.map((row) => ({ ...row }));

  return { port, notices, rows };
}

export { OUTCOME };
