// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * 44 §2.3's `gx_code` vocabulary, named rather than swallowed (`req/132` §2 item 1: "map gx_code
 * onto named error types (swallowing forbidden)"; sem: SEM-sdk-typescript-007).
 *
 * ~~Thirteen~~ ~~**Twenty-one**~~ ~~**Twenty-two**~~ ~~**Twenty-four**~~ ~~**Twenty-five**~~
 * **Twenty-six** codes on
 * the wire, not twelve:
 * 44 §2.3's table lists twelve, and `crates/gx-api/src/gx_code.rs`'s `RULED_ADDITIONS` carries
 * **fourteen** more, each one ruled in `req/38` and each one reachable by a client of this SDK.
 *
 * 🔴 **R11 / `req/240` L-06 (ii)** — the struck sentence is kept because it was this file's claim
 * for four releases and a reader who trusted it would look for thirteen. What it cost, measured:
 * `RULED_ADDITIONS` grew from two to nine while this list stood still, so `BUSY`,
 * `LEDGER_DISAGREES`, `DECLARATION_UNREADABLE`, `DECLARATION_ABSENT`, `CONFIG_ABSENT`,
 * `PAYLOAD_TOO_LARGE`, `UNSUPPORTED_MEDIA_TYPE` and `UNAVAILABLE` were refusals a TypeScript
 * caller could receive and could not name -- `GxApiError.code` widened them to `string`, which is
 * 44 §2.6's forward-compatibility rule doing the work of a table that had simply gone stale.
 * `sdk/typescript/test/gx_code_census.test.mjs` now reads the Rust source and fails when the two
 * disagree, so this list cannot go stale in silence again.
 *
 * 🔴 **R12 / `req/242` M-04 + H-01 (d)** — two things moved here.
 *
 * `JOURNAL_ABSENT` is the twenty-second: a project whose `.gx/ledger/journal` is gone is refused by
 * every door that appends, rather than being handed an empty log by the next writer (`req/242`
 * H-01 (d) measured one `gx submit` creating eight bytes of `GXJRNL01` over a loss `gx repair` had
 * correctly reported).
 *
 * And the census that keeps this list honest was itself fail-open. It imported `GX_CODES` from
 * `../dist/index.js` — a build output `.gitignore` excludes — so it compared the server's source
 * with an **artifact**: a `src/` that went backwards while a stale `dist/` stayed right passed it
 * green. It reads `src/errors.ts` now. `tools/e2e.sh` runs the SDK suite as its own stage, so the
 * probe is inside the floor rather than beside it (`req/242` M-04).
 *
 * 🔴 **R13 / `req/244` H-01 + M-04** — two more, and the first of them is about this face's own
 * plumbing. `OUTPUT_FAILED` is the twenty-third: a command whose answer was composed and could not
 * be written to its destination now says so in 44 §1.3's shape, where until R13 it panicked with
 * exit **101** and a Rust string (`req/244` H-01, three arms, three runs each). `HISTORY_LOST` is
 * the twenty-fourth: a project holding entries that say it used gx and holding no witness of any
 * commit is refused by the writer's door rather than being given a second history (`req/244`
 * M-04).
 *
 * 🔴 **R14 / `req/246` M-04** — `LAYOUT_BLOCKED` is the twenty-fifth: a path that is not a directory
 * sitting where one of `.gx/`'s declared directories belongs. One byte at `.gx/repair` locked a
 * project out of every verb that writes with `INTERNAL` "File exists (os error 17)" — 44 §2.3's word
 * for what cannot be classified, over a state the operating system had classified completely — while
 * `gx repair` reported the project healthy at exit 0. The word is about the **predicate** and not
 * about that path: `Layout::create` checks the shape of every declared directory before it makes any
 * of them.
 *
 * 🔴 **DR-B / `req/38` §337, `req/565` §3** — `JOURNAL_UNREADABLE` is the twenty-sixth: this
 * project's journal is present, is the regular file `req/56` §2 declares, and the process could
 * not open it (`EACCES` or similar). `req/38` §328 ruling 2 ③④ measured the same condition
 * (`gx log proof`/`gx log consistency`/`gx log checkpoint` all answering `INTERNAL` on a journal
 * `chmod 0000`'d) and deliberately did not mint a word for it at the time, filing it as a DR
 * instead — not `JOURNAL_ABSENT` ("is not there" would be false) and not `LAYOUT_BLOCKED` ("is
 * not what the declaration says" would be false of a regular file that is one). `req/38` §337
 * ruled the DR.
 */

/** 44 §2.3's twelve, then `gx_code.rs`'s fourteen `RULED_ADDITIONS`, in that file's order. */
export const GX_CODES = [
  // 44 §2.3's table, verbatim.
  "VALIDATION_ERROR",
  "NOT_FOUND",
  "NOT_ADMITTED",
  "PRECONDITION_CHANGED",
  "APPLY_FAILED",
  "ESCALATION_PENDING",
  "INVERSE_UNAVAILABLE",
  "IDEMPOTENCY_CONFLICT",
  "ADAPTER_ERROR",
  "POLICY_ERROR",
  "UNAUTHORIZED",
  "INTERNAL",
  // `gx_code.rs`'s `RULED_ADDITIONS`: codes 44 §2.3 has no row for, each ruled in `req/38`.
  "INVALID_STATE",
  "UNAVAILABLE",
  "PAYLOAD_TOO_LARGE",
  "UNSUPPORTED_MEDIA_TYPE",
  "BUSY",
  "LEDGER_DISAGREES",
  "DECLARATION_UNREADABLE",
  "DECLARATION_ABSENT",
  "CONFIG_ABSENT",
  "JOURNAL_ABSENT",
  "OUTPUT_FAILED",
  "HISTORY_LOST",
  "LAYOUT_BLOCKED",
  "JOURNAL_UNREADABLE",
] as const;

export type GxCode = (typeof GX_CODES)[number];

/**
 * 44 §2.3's `problem+json` body (`type`, `title`, `status`, `detail`, `gx_code`), and the one
 * member a refusal may carry beyond them.
 *
 * 🔴 **R11 / `req/240` L-06 (iii)** — `retry_after_ms` is DR-43-2's, put on the wire "for machines
 * to read", and `crates/gx-api/tests/wire_census.rs`'s
 * `a_busy_refusal_carries_a_sixth_member_and_nothing_else_does` fixes it to exactly one code:
 * `BUSY`. This SDK does not retry (`req/132` §1 declares that, and it is a design and not an
 * omission) -- which is precisely why the number has to be **typed**: a caller writing their own
 * retry is reading a field the SDK's own types said did not exist.
 */
export interface ProblemDetail {
  type: string;
  title: string;
  status: number;
  detail: string;
  gx_code: string;
  /** Milliseconds to wait before sending the same request again. Present on `BUSY` and on nothing
   * else -- `undefined` everywhere else, which is what the census pins. */
  retry_after_ms?: number;
}

/**
 * A refusal from `gx-api`, carrying 44 §2.3's five fields and never a bare string.
 *
 * `code` is typed as {@link GxCode} for every value this SDK's own vocabulary knows, and widens to
 * `string` for a code the server sent that predates this SDK's own table -- 44 §2.6's
 * forward-compatibility rule ("clients are required to be implemented so that unknown fields ...
 * can be ignored"; sem: SEM-sdk-typescript-008) applied
 * to a code the same way `crates/gx-api`'s own `_` fold applies to a refusal kind: named, not
 * dropped, not thrown as an opaque `Error` a caller cannot switch on.
 */
export class GxApiError extends Error {
  readonly code: GxCode | (string & {});
  readonly status: number;
  readonly type: string;
  readonly detail: string;

  constructor(problem: ProblemDetail) {
    super(`${problem.gx_code}: ${problem.detail}`);
    this.name = "GxApiError";
    this.code = problem.gx_code as GxCode | (string & {});
    this.status = problem.status;
    this.type = problem.type;
    this.detail = problem.detail;
  }
}

/** A response whose body was not `application/problem+json`-shaped -- the server refused to answer
 * about something other than a `gx_code` (a proxy's `502`, a load balancer's `503` HTML page, ...).
 * Named apart from {@link GxApiError} so a caller's `catch` can tell "the engine refused" from "the
 * network/host refused" without inspecting a string. */
export class GxTransportError extends Error {
  readonly status: number;
  readonly body: string;

  constructor(status: number, body: string) {
    super(`HTTP ${status} with a non-problem+json body: ${body.slice(0, 200)}`);
    this.name = "GxTransportError";
    this.status = status;
    this.body = body;
  }
}
