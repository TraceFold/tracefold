// SPDX-License-Identifier: Apache-2.0
// The membrane's single entry. A shell imports this and nothing else; a face imports
// nothing at all and is handed the port it is mounted with.

export { createMembrane, methodName, tableRows, TABLE, COVERAGE, WIRE_FIELDS } from './membrane.mjs';
export { deriveLedgers, routeKey, REASON_TAGS } from './ledgers.mjs';
export { foldPages, PAGE_BUDGET } from './pages.mjs';
export { stableKey, KEY_SCHEME } from './idempotency.mjs';
export { buildUrl, fillPath, BASE_PATH } from './address.mjs';
export {
  OUTCOME, FAILURE, ABSENCE, VERDICT_KINDS, ACTOR_VARIANTS, ESCALATION_DECISIONS,
  PROBLEM_MEMBERS, PROBLEM_MEDIA_TYPE, JSON_MEDIA_TYPE, PAGE_LIMIT_DEFAULT, PAGE_LIMIT_MAX, HOLES,
} from './wire.mjs';
