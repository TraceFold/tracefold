// SPDX-License-Identifier: Apache-2.0
// The three declarations of what is not covered, computed rather than kept.
//
// The reference tree kept these as three object literals somebody edited by hand, and
// req/01a §4-3 caught the result: a count that had been rewritten three times and was
// stale again by the time it was read. A hand-kept ledger records what its author
// remembered; a derived one records what is true, and forgetting is not one of the
// moves available.
//
// So membership is a set difference and never an entry. What a person supplies is the
// *reason* a member is a member, and the gate fails in both directions: a member with
// no reason is red, and a reason for something that is no longer a member is red too,
// because a stale explanation is how a closed hole keeps looking open.
//
//   NOT_CONSUMED  routes the server serves that no face calls
//   NOT_DRAWN     fields that arrive that no face puts on a screen
//   NOT_A_ROUTE   addresses a face asks for that the server does not serve

export const REASON_TAGS = Object.freeze(['unimplemented', 'undesigned', 'backend_missing']);

export function routeKey(verb, path) {
  return `${String(verb).toUpperCase()} ${path}`;
}

function difference(domain, covered) {
  return domain.filter((key) => !covered.has(key));
}

function checkReasons(members, reasons, scope, problems) {
  const memberSet = new Set(members);
  const entries = [];
  for (const key of members) {
    const reason = reasons[key];
    if (!reason) {
      problems.push(`${scope}: "${key}" has no reason`);
      continue;
    }
    if (!REASON_TAGS.includes(reason.tag)) {
      problems.push(`${scope}: "${key}" has tag "${reason.tag}", which is not one of ${REASON_TAGS.join('|')}`);
    }
    entries.push({ key, ...reason });
  }
  for (const key of Object.keys(reasons)) {
    if (!memberSet.has(key)) problems.push(`${scope}: a reason is kept for "${key}", which is not a member`);
  }
  return entries;
}

/**
 * @param {{routes: Array, coverage: object, fields: Array}} input
 */
export function deriveLedgers({ routes, coverage, fields = [] }) {
  const problems = [];

  // NOT_CONSUMED = served \ called
  const served = routes.map((r) => r.name);
  const called = new Set(
    Object.entries(coverage.consumed ?? {})
      .filter(([, faces]) => Array.isArray(faces) && faces.length > 0)
      .map(([name]) => name),
  );
  for (const name of called) {
    if (!served.includes(name)) problems.push(`NOT_CONSUMED: a face claims to call "${name}", which the table does not have`);
  }
  const notConsumed = difference(served, called);

  // NOT_DRAWN = arriving \ drawn
  const arriving = fields.map((f) => `${f.route}.${f.field}`);
  const drawn = new Set(coverage.drawn ?? []);
  for (const key of drawn) {
    if (!arriving.includes(key)) problems.push(`NOT_DRAWN: a face claims to draw "${key}", which is not in the field domain`);
  }
  const notDrawn = difference(arriving, drawn);

  // NOT_A_ROUTE = asked \ served, matched on the pair and never on a prefix.
  // The reference tree checked a path prefix and so believed in a GET that has never
  // existed (req/01a §4-7); one whole session was green on a route the server refuses.
  const servedPairs = new Set(routes.map((r) => routeKey(r.verb, r.path)));
  const asked = (coverage.requested ?? []).map((r) => routeKey(r.verb, r.path));
  const notARoute = [...new Set(asked)].filter((key) => !servedPairs.has(key));

  const reasons = coverage.reasons ?? {};
  const ledgers = {
    NOT_CONSUMED: {
      members: notConsumed,
      n: notConsumed.length,
      N: served.length,
      entries: checkReasons(notConsumed, reasons.NOT_CONSUMED ?? {}, 'NOT_CONSUMED', problems),
    },
    NOT_DRAWN: {
      members: notDrawn,
      n: notDrawn.length,
      N: arriving.length,
      entries: checkReasons(notDrawn, reasons.NOT_DRAWN ?? {}, 'NOT_DRAWN', problems),
    },
    NOT_A_ROUTE: {
      members: notARoute,
      n: notARoute.length,
      N: asked.length,
      entries: checkReasons(notARoute, reasons.NOT_A_ROUTE ?? {}, 'NOT_A_ROUTE', problems),
    },
  };

  return { ledgers, problems, ok: problems.length === 0 };
}
