// SPDX-License-Identifier: Apache-2.0
// Formatting for the strip's LIVE measured numbers -- SS551: "suite counts from
// .run/report.json, bench medians from .bench/report.json, serve state; fake
// numbers forbidden -- unreachable numbers get an honest 'not wired' slot".
//
// This module does no fetching and touches no disk: it is handed whatever JSON
// was reachable (or null, if it was not) and turns that into the three strings
// the strip draws. Keeping the fetch out of here is what makes the formatting
// itself testable with plain objects, the same way every other kernel module in
// this package is tested without a real document or a real network.

export const NOT_WIRED = 'not wired';

/** An age in words a strip has room for. Coarse on purpose: the sentence this serves is
 * "this reading is old", not a timestamp -- the report file itself carries the exact one. */
function ageText(ms) {
  const s = Math.round(ms / 1000);
  if (s < 120) return `${Math.max(s, 0)}s`;
  const m = Math.round(s / 60);
  if (m < 120) return `${m}m`;
  const h = Math.round(m / 60);
  if (h < 48) return `${h}h`;
  return `${Math.round(h / 24)}d`;
}

/**
 * req/822_c4 §2's open row, closed: `.run/report.json` recorded the INIT tree's two T3
 * failures and was drawn as a current reading for three critique cycles, because this
 * formatter compared the report against nothing. The report already carries its own tree
 * digest (`tools/verify-all.mjs` writes `body.tree` from the same manifest its assays were
 * handed); the bed computes the digest of the tree it is actually serving with the same
 * shipped derivation and hands both here as `now`. Three honest states, not two:
 *
 *   stale: false  the report describes the served tree; its verdict stands.
 *   stale: true   the report describes ANOTHER tree; its counts are still printed (they
 *                 are real measurements of a real tree) but its verdict is withdrawn --
 *                 `ok: null`, neither green nor red, because a failure that was measured
 *                 against a tree that no longer exists is not a failure of this one, and
 *                 a pass is not a pass of this one either. That second half is the sharp
 *                 edge: the stale GREEN is the false-green this row was opened over.
 *   stale: null   no basis to compare (an old bed, or a report from before the digest was
 *                 recorded). Claiming "fresh" here would be the same invention.
 *
 * @param {object|null} run the parsed contents of `.run/report.json`, or null if
 *   it could not be reached
 * @param {{tree: string|null, atMs: number, reportMtimeMs: number|null}|null} [now] what
 *   the bed measured about the tree it serves, or null when it measured nothing
 * @returns {{text: string, ok: boolean|null, stale: boolean|null}}
 */
export function suiteMeasure(run, now = null) {
  if (!run || typeof run !== 'object' || !run.assays || !run.lint) {
    return { text: `suite: ${NOT_WIRED}`, ok: null, stale: null };
  }
  const { assays, lint } = run;
  // req/822_c7 S5 (Owner #388): `suite N/N assays, N/N lint` opened with a word the
  // numbers already imply and said `suite` before saying anything measured. The
  // counts keep their nouns; the frame word goes.
  const counts = `assays ${assays.pass}/${assays.total}, lint ${lint.pass}/${lint.total}`;
  const comparable = typeof run.tree === 'string' && typeof now?.tree === 'string';
  if (!comparable) {
    return { text: counts, ok: assays.fail === 0 && lint.fail === 0, stale: null };
  }
  if (run.tree === now.tree) {
    return { text: counts, ok: assays.fail === 0 && lint.fail === 0, stale: false };
  }
  const aged = typeof now.reportMtimeMs === 'number' && typeof now.atMs === 'number';
  const since = aged ? `, ${ageText(now.atMs - now.reportMtimeMs)} ago` : '';
  return { text: `${counts} (another tree${since})`, ok: null, stale: true };
}

/**
 * @param {object|null} bench the parsed contents of a `.bench/report.json`
 * @param {string} [label] what the median is a median of
 * @returns {{text: string, ok: boolean|null}}
 */
export function benchMeasure(bench, label = 'shell mount') {
  if (!bench || typeof bench !== 'object' || typeof bench.medianMs !== 'number') {
    return { text: `bench: ${NOT_WIRED}`, ok: null };
  }
  const ok = bench.ok !== false;
  // req/822_c7 S5: the budget rides the hover layer -- the strip states the reading,
  // the title states what it is judged against.
  return {
    text: `${label} ${bench.medianMs}ms`,
    ok,
    title: `median ${bench.medianMs}ms against a ${bench.budgetMs}ms budget`,
  };
}

/**
 * @param {string|null} origin `window.location.origin`, or null off-window
 * @returns {{text: string, ok: boolean|null}}
 */
export function serveMeasure(origin) {
  if (typeof origin !== 'string' || origin === '') return { text: `served: ${NOT_WIRED}`, ok: null };
  return { text: `served ${origin}`, ok: true };
}

/**
 * @param {{run?: object|null, bench?: object|null, benchLabel?: string, origin?: string|null,
 *   now?: {tree: string|null, atMs: number, reportMtimeMs: number|null}|null}} sources
 * @returns {{suite: object, bench: object, serve: object}}
 */
export function formatMeasures({
  run = null, bench = null, benchLabel = 'shell mount', origin = null, now = null,
} = {}) {
  return {
    suite: suiteMeasure(run, now),
    bench: benchMeasure(bench, benchLabel),
    serve: serveMeasure(origin),
  };
}
