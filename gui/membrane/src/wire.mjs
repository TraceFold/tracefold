// SPDX-License-Identifier: Apache-2.0
// The words the server owns, and the four shapes an answer can take.
//
// Only vocabulary read out of the crate is frozen here. What was not read is named
// in HOLES rather than guessed, because a list that looks complete and is not is
// worse than a short one that says so.

/** The four faces an answer can wear. Three of them are not success, and they differ. */
export const OUTCOME = Object.freeze({
  ANSWERED: 'answered',
  REFUSED: 'refused',
  FAILED: 'failed',
  ABSENT: 'absent',
});

/** Why a call failed. Never a refusal: a refusal is the engine speaking. */
export const FAILURE = Object.freeze({
  TRANSPORT: 'transport',
  UNDECODABLE: 'undecodable',
  UNEXPECTED_MEDIA_TYPE: 'unexpected_media_type',
});

/** Why a call was absent. */
export const ABSENCE = Object.freeze({
  NO_SUCH_ROUTE: 'no_such_route',
});

export const PROBLEM_MEDIA_TYPE = 'application/problem+json';
export const JSON_MEDIA_TYPE = 'application/json';

/** gx-api/src/auth.rs:166-169 */
export const AUTH_HEADER = 'authorization';
export const AUTH_SCHEME = 'Bearer ';

/** gx-api/src/handlers.rs:323-326 */
export const IDEMPOTENCY_HEADER = 'idempotency-key';

/**
 * gx-core/src/verdict.rs:46-54 and :69-75.
 *
 * Ruled here, because it was open: the fieldless variants carry no serde rename in
 * the workspace, so serde writes the variant name and `as_str` writes the same three
 * strings. The wire spelling is therefore PascalCase, and a fixture holding "admit"
 * is describing a server that does not exist. req/01a §4-2 found the disagreement and
 * left it undecided; this is the decision, with the line that decides it.
 */
export const VERDICT_KINDS = Object.freeze(['Admit', 'Deny', 'Escalate']);

/** gx-core/src/context.rs:52-71 — externally tagged, so an actor is `{Variant:{...}}`. */
export const ACTOR_VARIANTS = Object.freeze(['Human', 'Agent', 'Process']);

/** gx-api/src/handlers.rs:1123,1156-1157 */
export const ESCALATION_DECISIONS = Object.freeze(['approve', 'reject']);

/** gx-api/src/problem.rs:335-342 — every refusal carries these five. */
export const PROBLEM_MEMBERS = Object.freeze(['type', 'title', 'status', 'detail', 'gx_code']);

/** gx-api/src/list.rs:57-60 */
export const PAGE_LIMIT_DEFAULT = 50;
export const PAGE_LIMIT_MAX = 200;

/**
 * Vocabulary the reference documents name but this module has not read in the crate.
 * Declared rather than copied: an unread word frozen from a second-hand source is a
 * second source of truth, and the two drift without anyone being told.
 */
export const HOLES = Object.freeze({
  Lifecycle: 'gx-engine; not read here',
  Rollback: 'gx-core; not read here',
  InverseStatus: 'gx-core; not read here',
  AbortReason: 'gx-core; not read here',
  GxCode: 'gx-api/src/gx_code.rs holds the set; only VALIDATION_ERROR, UNAUTHORIZED and IDEMPOTENCY_CONFLICT were read',
  response_bodies: 'gx-api/src/handlers.rs bodies were not read; the membrane carries them without naming their members',
});

export function answered(status, body, extra = {}) {
  return { outcome: OUTCOME.ANSWERED, status, body, ...extra };
}

export function refused(status, problem) {
  return {
    outcome: OUTCOME.REFUSED,
    status,
    gx_code: typeof problem?.gx_code === 'string' ? problem.gx_code : null,
    problem,
  };
}

export function failed(reason, status, detail) {
  return { outcome: OUTCOME.FAILED, reason, status, detail };
}

export function absent(reason, requested) {
  return { outcome: OUTCOME.ABSENT, reason, requested };
}
