// SPDX-License-Identifier: Apache-2.0
// A fixed situation put through the shipped rig, so that breaking the rig by one
// line changes the answer.
//
// The corpus is chosen so that every decision the rig owns is exercised once and in
// a direction where failing open is visible: a population that is empty with no
// reason must come out EMPTY, a requirement that is absent must come out SKIP, a
// tier that was not declared must be refused at registration. The probe is expected
// to be red. If it goes green, something stopped catching what it must.
//
// This calls the real modules. A hand-written copy of the rig's logic would pass
// while the shipped rig rotted, which is the failure mode this file exists to avoid.

import { createCatalogue } from './catalogue.mjs';
import { runCatalogue } from './runner.mjs';
import { assemble } from './report.mjs';

export async function runSelfProbe() {
  const catalogue = createCatalogue();

  catalogue.register({
    id: 'SP-HOLDS', tier: 'T1', title: 'a reading that holds over three members',
    backs: ['SP'], armed_by: ['self'],
    population: () => ['a', 'b', 'c'], hold: () => true,
  });

  catalogue.register({
    id: 'SP-BREAKS', tier: 'T1', title: 'a reading that does not hold',
    backs: ['SP'], armed_by: ['self'],
    population: () => ['a'], hold: () => 'this member was planted to break',
  });

  catalogue.register({
    id: 'SP-EMPTY-UNEXPLAINED', tier: 'T1', title: 'an empty population with nothing recorded about why',
    backs: ['SP'], armed_by: ['self'],
    population: () => [], hold: () => true,
  });

  catalogue.register({
    id: 'SP-EMPTY-EXPLAINED', tier: 'T1', title: 'an empty population with a live recorded reason',
    backs: ['SP'], armed_by: ['self'],
    expect_empty: { reason: 'planted: the reason is present and has not expired', expires: '2099-01-01' },
    population: () => [], hold: () => true,
  });

  catalogue.register({
    id: 'SP-EMPTY-EXPIRED', tier: 'T1', title: 'an empty population whose recorded reason has run out',
    backs: ['SP'], armed_by: ['self'],
    expect_empty: { reason: 'planted: the reason expired long ago', expires: '2000-01-01' },
    population: () => [], hold: () => true,
  });

  catalogue.register({
    id: 'SP-REQUIREMENT-ABSENT', tier: 'T1', title: 'a reading whose requirement is not here',
    backs: ['SP'], armed_by: ['self'],
    requires: ['a-thing-that-is-not-present'],
    population: () => ['a'], hold: () => true,
  });

  catalogue.register({
    id: 'SP-LINT', tier: 'T0', title: 'a text reading, which is not counted as an assay',
    backs: ['SP'], armed_by: ['self'],
    population: () => ['a'], hold: () => true,
  });

  // Registration is part of the rig, so its refusals belong in the probe.
  let tierlessWas;
  try {
    catalogue.register({ id: 'SP-NO-TIER', title: 'no tier declared', backs: ['SP'], population: () => ['a'], hold: () => true });
    tierlessWas = 'accepted';
  } catch { tierlessWas = 'refused'; }

  let unbackedWas;
  try {
    catalogue.register({ id: 'SP-NO-BACKS', tier: 'T1', title: 'no criteria declared', population: () => ['a'], hold: () => true });
    unbackedWas = 'accepted';
  } catch { unbackedWas = 'refused'; }

  const results = await runCatalogue(catalogue, {}, { present: new Set() });
  const { body, exit } = assemble({
    results: results.filter((r) => r.tier !== 'T0'),
    lint: results.filter((r) => r.tier === 'T0'),
    acLedger: { total: 1, backed: 1, unbacked: [] },
    tree: 'probe', environment: 'probe', wire: true, window: true, treeStable: true,
    timings: {}, budgets: {},
  });

  // Two gates that the corpus above cannot reach, because they are decisions about a
  // run that is otherwise clean. Each gets its own minimal assembly so that removing
  // it changes something visible here.
  const clean = results.filter((r) => r.id === 'SP-HOLDS');
  const shell = { acLedger: { total: 1, backed: 1, unbacked: [] }, tree: 'probe', environment: 'probe', window: false, treeStable: true, timings: {}, budgets: {} };
  const wireGate = assemble({ ...shell, results: clean, lint: [], wire: false }).body.outcome;
  const unarmedGate = assemble({
    ...shell, lint: [], wire: true,
    results: [{ ...clean[0], id: 'SP-UNARMED', armed_by: [] }],
  }).body.outcome;

  return {
    registration: { tierless: tierlessWas, unbacked: unbackedWas },
    gates: { wireless: wireGate, unarmed: unarmedGate },
    verdicts: Object.fromEntries(results.map((r) => [r.id, r.verdict])),
    outcome: body.outcome,
    exit,
    digest: body.digest,
  };
}

// Printed in a shape a parent process can compare without parsing prose.
if (import.meta.url === `file:///${process.argv[1].split('\\').join('/')}`) {
  const probe = await runSelfProbe();
  console.log(JSON.stringify(probe));
}
