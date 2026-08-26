// SPDX-License-Identifier: Apache-2.0
// The rig. It owns the denominator, it owns the empty case, and it owns the record.
//
// The three things an assay is not allowed to do, and therefore cannot:
//   - build its own population (it is handed one)
//   - decide what an empty population means (this file decides)
//   - overwrite its own result (a second result for one id is refused)
// Each of those was a repeated failure in the retired instruments, twelve sites,
// eleven sites and two sites respectively. Care at the call site did not fix them.

import { VERDICT, RIG_MESSAGES } from './verdict.mjs';

const isPast = (iso) => Boolean(iso) && Date.parse(iso) < Date.now();

function judgeEmpty(assay) {
  const declared = assay.expect_empty;
  if (!declared || !declared.reason || String(declared.reason).trim() === '') {
    return { verdict: VERDICT.EMPTY, note: RIG_MESSAGES.EMPTY_UNEXPLAINED };
  }
  if (isPast(declared.expires)) {
    return { verdict: VERDICT.EMPTY, note: `${RIG_MESSAGES.EMPTY_EXPIRED}: ${declared.expires}` };
  }
  return { verdict: VERDICT.PASS, note: `${RIG_MESSAGES.EMPTY_EXPLAINED}: ${declared.reason}` };
}

export async function runCatalogue(catalogue, world, { present = new Set() } = {}) {
  const recorded = new Map();

  for (const assay of catalogue.all()) {
    if (recorded.has(assay.id)) throw new Error(`${RIG_MESSAGES.RESULT_ALREADY_RECORDED}: ${assay.id}`);
    const started = process.hrtime.bigint();

    const absent = (assay.requires ?? []).filter((r) => !present.has(r));
    if (absent.length > 0) {
      recorded.set(assay.id, freeze(assay, {
        verdict: VERDICT.SKIP, population: 0, failures: [],
        note: `${RIG_MESSAGES.REQUIREMENT_ABSENT}: ${absent.join(', ')}`, ms: 0,
      }));
      continue;
    }

    let population;
    let failures = [];
    let verdict;
    let note;
    try {
      population = await assay.population(world);
      if (!Array.isArray(population)) population = [population];
      if (population.length === 0) {
        ({ verdict, note } = judgeEmpty(assay));
      } else {
        for (const member of population) {
          const held = await assay.hold(member, world);
          if (held !== true) failures.push({ member: describe(member), why: typeof held === 'string' ? held : RIG_MESSAGES.BROKE });
        }
        verdict = failures.length === 0 ? VERDICT.PASS : VERDICT.FAIL;
        note = failures.length === 0
          ? `${RIG_MESSAGES.HELD} (n=${population.length})`
          : `${RIG_MESSAGES.BROKE} for ${failures.length} of ${population.length}`;
      }
    } catch (err) {
      population = population ?? [];
      verdict = VERDICT.FAIL;
      note = `the reading itself threw: ${err.message}`;
      failures = [{ member: '<reading>', why: err.message }];
    }

    recorded.set(assay.id, freeze(assay, {
      verdict,
      population: population.length,
      failures,
      note,
      ms: Number((process.hrtime.bigint() - started) / 1000000n),
    }));
  }

  return [...recorded.values()];
}

const describe = (member) => {
  if (member === null || member === undefined) return String(member);
  if (typeof member === 'string') return member;
  if (typeof member === 'object') return member.path ?? member.id ?? member.name ?? JSON.stringify(member).slice(0, 120);
  return String(member);
};

const freeze = (assay, result) => Object.freeze({
  id: assay.id,
  tier: assay.tier,
  title: assay.title,
  backs: assay.backs,
  armed_by: assay.armed_by ?? [],
  ...result,
});
