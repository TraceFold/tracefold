// SPDX-License-Identifier: Apache-2.0
// Reading a list to its end, and saying how much reading that took.
//
// The count of items is not the interesting number. "Two hundred rows" is the same
// sentence whether the server had two hundred or whether the walk stopped at a budget
// with more behind it, so the walk reports its own denominator: how many requests, and
// whether it stopped because the list ended, because a cursor repeated, or because the
// budget ran out. A caller that is shown a truncated list without being told is a
// caller that will believe it has seen everything.
//
// Two cursor flavours, because the server has two: an opaque string on the three list
// endpoints (gx-api/src/list.rs) and an integer chain position on the verdict
// checkpoints (gx-api/src/verdict_checkpoints.rs:38). Both are carried, neither is
// interpreted.

import { OUTCOME } from './wire.mjs';

/** Enough pages to read a large ledger, few enough that a looping server stops. */
export const PAGE_BUDGET = 64;

/**
 * @param {(cursor: string|number|undefined) => Promise<object>} readPage
 * @param {{budget?: number}} options
 */
export async function foldPages(readPage, { budget = PAGE_BUDGET } = {}) {
  const items = [];
  const seen = new Set();
  let cursor;
  let requests = 0;
  let repeated = false;
  let stopped = false;
  let total;

  for (;;) {
    if (requests >= budget) {
      stopped = true;
      break;
    }
    const page = await readPage(cursor);
    requests += 1;
    if (page.outcome !== OUTCOME.ANSWERED) return page;

    const body = page.body ?? {};
    if (Array.isArray(body.items)) items.push(...body.items);
    if (typeof body.total === 'number') total = body.total;

    const next = body.next_cursor;
    if (next === undefined || next === null) break;
    const mark = `${typeof next}:${next}`;
    if (seen.has(mark)) {
      repeated = true;
      break;
    }
    seen.add(mark);
    cursor = next;
  }

  return {
    outcome: OUTCOME.ANSWERED,
    items,
    requests,
    pages: requests,
    stopped_at_budget: stopped,
    repeated_cursor: repeated,
    budget,
    ...(total === undefined ? {} : { total }),
  };
}
