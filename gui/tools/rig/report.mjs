// SPDX-License-Identifier: Apache-2.0
// The report, in the order it has to be built: counts first, then the word.
//
// The word at the bottom is computed from the counts above it. There is no branch in
// this file where an author picks it. That is the whole point -- the retired
// instrument printed a headline it had chosen and a body that disagreed with it.

import crypto from 'node:crypto';
import { VERDICT, EXIT, OUTCOME, COUNTED_TIERS, RIG_MESSAGES } from './verdict.mjs';

export function assemble({ results, lint = [], acLedger = { total: 0, backed: 0, unbacked: [] }, missingAcSources = [], tree, environment, wire = false, window = false, treeStable = true, selfBlind = false, timings = {}, budgets = {} }) {
  const counted = results.filter((r) => COUNTED_TIERS.includes(r.tier));
  const tally = {
    pass: counted.filter((r) => r.verdict === VERDICT.PASS).length,
    fail: counted.filter((r) => r.verdict === VERDICT.FAIL).length,
    empty: counted.filter((r) => r.verdict === VERDICT.EMPTY).length,
    skip: counted.filter((r) => r.verdict === VERDICT.SKIP).length,
    flaky: counted.filter((r) => r.verdict === VERDICT.FLAKY).length,
    total: counted.length,
  };
  const lintTally = {
    pass: lint.filter((r) => r.verdict === VERDICT.PASS).length,
    fail: lint.filter((r) => r.verdict === VERDICT.FAIL).length + lint.filter((r) => r.verdict === VERDICT.EMPTY).length,
    total: lint.length,
  };
  const unarmed = results.filter((r) => (r.armed_by ?? []).length === 0).map((r) => r.id);
  const neverExercised = results.filter((r) => r.population === 0 && r.verdict !== VERDICT.SKIP).map((r) => r.id);
  const overBudget = Object.entries(budgets)
    .filter(([stage, allowed]) => (timings[stage] ?? 0) > allowed)
    .map(([stage, allowed]) => ({ stage, ms: timings[stage], allowed }));

  // The order here is the order of severity, and the first one that fires wins.
  //
  // SOURCES_ABSENT is first because it is not a claim about the tree at all, it is a
  // statement that one axis of this report has no denominator. Every branch below it
  // reads acLedger.unbacked, and when the corpus is missing that list is empty for the
  // one reason that must never be mistaken for agreement: nothing was compared. Ranked
  // above selfBlind deliberately -- a harness that cannot see its criteria is in a
  // worse epistemic position than one whose self-check merely went dark (req/883).
  let outcome;
  let exit;
  if (missingAcSources.length > 0) { outcome = OUTCOME.SOURCES_ABSENT; exit = EXIT.SOURCES_ABSENT; }
  else if (selfBlind) { outcome = OUTCOME.SELF_BLIND; exit = EXIT.SELF_BLIND; }
  else if (!treeStable) { outcome = OUTCOME.NON_CANONICAL; exit = EXIT.NON_CANONICAL; }
  else if (tally.flaky > 0) { outcome = OUTCOME.FLAKY; exit = EXIT.FLAKY; }
  else if (tally.fail > 0 || tally.empty > 0 || lintTally.fail > 0 || unarmed.length > 0
           || acLedger.unbacked.length > 0 || overBudget.length > 0) { outcome = OUTCOME.RED; exit = EXIT.RED; }
  else if (tally.skip > 0 || !wire) { outcome = OUTCOME.PARTIAL; exit = EXIT.PARTIAL; }
  else { outcome = OUTCOME.GREEN; exit = EXIT.GREEN; }

  const body = {
    tree, environment, wire, window,
    assays: tally,
    lint: lintTally,
    // `measured` travels inside coverage rather than beside it, so a consumer that
    // reads `backed / total` cannot reach the numbers without the flag that says
    // whether they mean anything.
    coverage: {
      total: acLedger.total,
      backed: acLedger.backed,
      unbacked: acLedger.unbacked,
      measured: missingAcSources.length === 0,
    },
    missingAcSources,
    unarmed,
    neverExercised,
    overBudget,
    timings,
    // Persisted, not only used internally: a reader of report.json could previously see
    // overBudget entries (a stage that failed its budget) but never the budget itself
    // unless it happened to be exceeded. A bench declaration that only shows the number
    // when it is red is not a declaration (app req/98 V-14 conformance gate needs this).
    budgets,
    outcome,
    // A report that cannot stand behind an acceptance criterion says so in the file,
    // not only on the screen, so the flag travels with the evidence.
    citableAsEvidence: outcome === OUTCOME.GREEN,
    readings: results.map((r) => ({
      id: r.id, tier: r.tier, verdict: r.verdict, population: r.population,
      backs: r.backs, armed_by: r.armed_by, note: r.note, ms: r.ms,
      failures: r.failures.slice(0, 8),
    })),
    lintReadings: lint.map((r) => ({ id: r.id, verdict: r.verdict, population: r.population, note: r.note })),
  };

  // Digest over the shape of the outcome, not over the clock, so two runs of one
  // tree are comparable and a stopwatch cannot make them differ.
  const canonical = JSON.stringify({
    tree, environment, wire, window, outcome,
    readings: body.readings.map(({ id, verdict, population }) => ({ id, verdict, population })),
    lint: body.lintReadings.map(({ id, verdict, population }) => ({ id, verdict, population })),
    coverage: body.coverage, unarmed, neverExercised,
  });
  body.digest = crypto.createHash('sha256').update(canonical).digest('hex').slice(0, 16);
  return { body, exit };
}

export function headline(body) {
  const a = body.assays;
  const lines = [
    `verify-all  tree=${body.tree}  env=${body.environment}  wire=${body.wire ? 'yes' : 'no'}  window=${body.window ? 'yes' : 'no'}  report=${body.digest}`,
    `  assays     : pass ${a.pass}  fail ${a.fail}  empty ${a.empty}  skip ${a.skip}  flaky ${a.flaky}   (total ${a.total})`,
    `  lint       : pass ${body.lint.pass}  fail ${body.lint.fail}                              [not counted as assays]`,
    // The coverage line above is only as honest as this one: a denominator nobody can
    // see is a denominator nobody can doubt. Printed every run, not only in the JSON.
    `  ac sources : ${(body.acSources ?? []).join(', ') || 'none declared'}`,
    body.coverage.measured === false
      ? `  coverage   : UNMEASURED -- ${(body.missingAcSources ?? []).length} of ${(body.acSources ?? []).length} ac sources absent: ${(body.missingAcSources ?? []).join(', ')}`
      : `  coverage   : AC backed ${body.coverage.backed} / ${body.coverage.total}   unbacked ${body.coverage.unbacked.length}`,
    `  unarmed    : ${body.unarmed.length}${body.unarmed.length ? `  (${body.unarmed.slice(0, 6).join(', ')})` : ''}`,
    `  unexercised: ${body.neverExercised.length}${body.neverExercised.length ? `  (${body.neverExercised.slice(0, 6).join(', ')})` : ''}`,
    `  bench      : ${Object.entries(body.timings).map(([k, v]) => `${k} ${v}ms`).join('  ') || 'not measured'}${body.overBudget.length ? `   OVER BUDGET: ${body.overBudget.map((o) => `${o.stage} ${o.ms}>${o.allowed}`).join(', ')}` : ''}`,
    `  self       : ${body.selfCheck ? `${body.selfCheck.rounds} rounds, ${body.selfCheck.inert} inert, ${body.selfCheck.blind ? 'BLIND' : 'sighted'} (${body.selfCheck.ms}ms)` : 'not run'}`,
    `  verdict    : ${body.outcome}`,
  ];
  if (body.coverage.measured === false) lines.push(`               ${RIG_MESSAGES.AC_SOURCES_ABSENT}`);
  if (body.outcome !== 'GREEN' && !body.wire) lines.push(`               ${RIG_MESSAGES.GREEN_WITHOUT_WIRE}`);
  return lines.join('\n');
}
