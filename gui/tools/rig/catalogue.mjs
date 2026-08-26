// SPDX-License-Identifier: Apache-2.0
// Registration, and what it refuses.
//
// A tier is declared here or the assay does not exist. Nothing in this file infers a
// tier from what an assay's source looks like: that inference is exactly how a
// reading came to be quietly skipped while the count still read full.

import { TIERS, RIG_MESSAGES } from './verdict.mjs';

export function createCatalogue() {
  const assays = new Map();
  const retired = new Map();

  return {
    register(definition) {
      const { id, tier, title, backs, population, hold } = definition;
      if (!id) throw new Error(RIG_MESSAGES.ID_DUPLICATE);
      if (assays.has(id)) throw new Error(`${RIG_MESSAGES.ID_DUPLICATE}: ${id}`);
      if (!tier) throw new Error(`${RIG_MESSAGES.TIER_MISSING}: ${id}`);
      if (!TIERS.includes(tier)) throw new Error(`${RIG_MESSAGES.TIER_UNKNOWN}: ${id} declared ${tier}`);
      if (!Array.isArray(backs) || backs.length === 0) throw new Error(`${RIG_MESSAGES.BACKS_MISSING}: ${id}`);
      if (typeof population !== 'function') throw new Error(`${RIG_MESSAGES.POPULATION_MISSING}: ${id}`);
      if (typeof hold !== 'function') throw new Error(`${RIG_MESSAGES.POPULATION_MISSING}: ${id}`);
      assays.set(id, {
        armed_by: [],
        requires: [],
        expect_empty: null,
        ...definition,
        title: title ?? id,
      });
      return id;
    },

    // Deleting an assay silently is how coverage falls without anybody deciding it
    // should. Retirement is a row, not an absence.
    retire(id, { reason, replacedBy = null }) {
      const existing = assays.get(id);
      if (!existing) return false;
      assays.delete(id);
      retired.set(id, { id, reason, replacedBy, tier: existing.tier, backs: existing.backs });
      return true;
    },

    get: (id) => assays.get(id) ?? null,
    all: () => [...assays.values()],
    retiredRows: () => [...retired.values()],
    size: () => assays.size,
  };
}
