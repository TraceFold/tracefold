// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * `GxClient` -- the typed HTTP half of P4's SDK (`req/132` §2 item 1).
 *
 * Thin projection (`req/132` §1): one method per 44 §2.1 endpoint, no retry, no connection pool, no
 * cache, no state beyond the base URL and the bearer token a caller hands in (principle ⑤ of the
 * five: "the SDK holds no persistence; keeping the token is the caller's responsibility" (sem:
 * SEM-sdk-typescript-003) -- this class holds the token in memory for the life of the
 * object and persists nothing to disk). `fetch` is the only I/O primitive (global, Node 18+); no
 * runtime dependency is declared for it.
 *
 * # Surface parity denominator (AC-P4-1)
 *
 * `ENDPOINT_COUNT` below is asserted equal to a machine parse of `req/spec/40-architecture/44-api-spec.md`
 * §2.1's table by `test/endpoint_parity.test.ts` -- the same doc-conformance shape
 * `crates/gx-cli/tests/exit_map.rs` uses on the Rust side of this repository, one language over.
 */

import {
  CancelOutcome,
  CancelRequest,
  CandidateCreated,
  CandidateRow,
  CandidateView,
  Checkpoint,
  CommitRequest,
  ConsistencyProof,
  CreateCandidateRequest,
  EscalationOutcome,
  EscalationRequest,
  EscalationRow,
  InclusionProof,
  Page,
  Receipt,
  ReplayOutcome,
  ReplayRequest,
  TransformationId,
  TransformationRow,
  TransformationView,
  UndoOutcome,
  UndoRequest,
  VerdictCheckpoint,
  VerdictCheckpointPage,
  VerifyOutcome,
  VerifyRequest,
  IssueVerdictCheckpointRequest,
  AttachSourcePage,
  AttachSourceRegisterRequest,
  AttachSourceView,
} from "./types.js";
import { GxApiError, GxTransportError, ProblemDetail } from "./errors.js";

/** 44 §2's base path. */
const BASE_PATH = "/v1";

/**
 * The seventeen methods that project 44 §2.1's table, one row each, **in the table's own order**.
 *
 * Named rather than counted, the same shape `crates/gx-api/src/lib.rs`'s `SPECIFIED_ENDPOINTS`
 * takes one language over: `test/endpoint_parity.test.ts` parses 44 §2.1's markdown table
 * independently and asserts this array's length equals its row count *and* that every name here is
 * a real, callable method of {@link GxClient} -- so a method renamed here and not there, or added
 * to the class and not to this list, is red rather than a passing reflection over "whatever the
 * class happens to have" (which `raw`/`ledgerConsistency` below would otherwise inflate).
 */
export const SPECIFIED_METHODS = [
  "createCandidate", // POST /candidates
  "getCandidate", // GET /candidates/{id}
  "verifyCandidate", // POST /candidates/{id}/verify
  "commitCandidate", // POST /candidates/{id}/commit
  "escalateCandidate", // POST /candidates/{id}/escalation
  "cancelCandidate", // POST /candidates/{id}/cancel
  "undoTransformation", // POST /transformations/{id}/undo
  "replayTransformation", // POST /transformations/{id}/replay
  "getTransformation", // GET /transformations/{id}
  "getReceipt", // GET /receipts/{tid}
  "ledgerProof", // GET /ledger/proof?leaf=
  "ledgerCheckpoint", // GET /ledger/checkpoint
  "issueVerdictCheckpoint", // POST /verdict-checkpoints
  "listVerdictCheckpoints", // GET /verdict-checkpoints
  "getVerdictCheckpoint", // GET /verdict-checkpoints/{window_end}
  "stream", // GET /stream
  "healthz", // GET /healthz
] as const;

/** The methods this class serves beyond 44 §2.1's seventeen -- 44 §2.6/§2.7 extensions and the
 * escape hatch, named for the same reason `crates/gx-api/src/list.rs`'s `EXTENSION_ENDPOINTS` is:
 * a count that did not name its extras would look like a bigger surface than 44 §2.1 specifies. */
export const EXTENSION_METHODS = [
  "ledgerConsistency",
  "listCandidates",
  "listEscalations",
  "listTransformations",
  // `req/841` P1-2: the attach-source registry (`req/824` A4) -- three live routes
  // (`crates/gx-api/src/attach_sources.rs::ATTACH_SOURCE_ENDPOINTS`), 44 §2.6 extensions the same
  // way the list family is; measured absent from this roster on 2026-08-26 while the routes were
  // already guarded and serving, which is exactly the drift this named roster exists to make red.
  "registerAttachSource",
  "listAttachSources",
  "getAttachSource",
  "raw",
] as const;

export interface GxClientOptions {
  /** e.g. `http://127.0.0.1:8787` (`gx serve`'s default bind, 44 §2.5). No trailing slash. */
  baseUrl: string;
  /** 44 §2.5's Bearer token. Required for every endpoint but `/healthz`; the caller supplies it
   * (principle ⑤ of the five (sem: SEM-sdk-typescript-004) -- this SDK never reads a token file,
   * env var, or config on its own). */
  token?: string;
  /** Injected for tests; defaults to `globalThis.fetch`. Never a dependency, always the platform's. */
  fetchImpl?: typeof fetch;
}

/** `GET /candidates`, `GET /escalations`, `GET /transformations`, `GET /ledger/consistency` are 44
 * extensions (§2.6's "backward-compatible additions"; sem: SEM-sdk-typescript-005), not rows of
 * 44 §2.1's own table -- `req/132` §2 item 1 scopes
 * this SDK to 44 §2.1's seventeen and declares the four extensions out of v0.1 (scope §3, not silently
 * dropped: `ledgerConsistency`, `listCandidates`, `listEscalations` and `listTransformations` below
 * name all four; `GxClient.raw` remains for anything this SDK has not named a method for). */
export class GxClient {
  private readonly baseUrl: string;
  private readonly token: string | undefined;
  private readonly fetchImpl: typeof fetch;

  constructor(options: GxClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/+$/, "");
    this.token = options.token;
    this.fetchImpl = options.fetchImpl ?? globalThis.fetch;
    if (!this.fetchImpl) {
      throw new Error(
        "no global fetch found. GxClient targets Node 18+, which has one; pass fetchImpl for another runtime.",
      );
    }
  }

  // -- 1. POST /candidates -----------------------------------------------------------------
  async createCandidate(body: CreateCandidateRequest): Promise<CandidateCreated> {
    return this.request<CandidateCreated>("POST", "/candidates", body);
  }

  // -- 2. GET /candidates/{id} --------------------------------------------------------------
  async getCandidate(id: TransformationId): Promise<CandidateView> {
    return this.request<CandidateView>("GET", `/candidates/${encodeURIComponent(id)}`);
  }

  // -- 3. POST /candidates/{id}/verify ------------------------------------------------------
  async verifyCandidate(id: TransformationId, body?: VerifyRequest): Promise<VerifyOutcome> {
    return this.request<VerifyOutcome>("POST", `/candidates/${encodeURIComponent(id)}/verify`, body);
  }

  // -- 4. POST /candidates/{id}/commit ------------------------------------------------------
  async commitCandidate(
    id: TransformationId,
    body?: CommitRequest,
    idempotencyKey?: string,
  ): Promise<Receipt> {
    return this.request<Receipt>(
      "POST",
      `/candidates/${encodeURIComponent(id)}/commit`,
      body,
      idempotencyKey,
    );
  }

  // -- 5. POST /candidates/{id}/escalation --------------------------------------------------
  async escalateCandidate(id: TransformationId, body: EscalationRequest): Promise<EscalationOutcome> {
    return this.request<EscalationOutcome>(
      "POST",
      `/candidates/${encodeURIComponent(id)}/escalation`,
      body,
    );
  }

  // -- 6. POST /candidates/{id}/cancel ------------------------------------------------------
  async cancelCandidate(id: TransformationId, body: CancelRequest): Promise<CancelOutcome> {
    return this.request<CancelOutcome>("POST", `/candidates/${encodeURIComponent(id)}/cancel`, body);
  }

  // -- 7. POST /transformations/{id}/undo ---------------------------------------------------
  async undoTransformation(
    id: TransformationId,
    body?: UndoRequest,
    idempotencyKey?: string,
  ): Promise<UndoOutcome> {
    return this.request<UndoOutcome>(
      "POST",
      `/transformations/${encodeURIComponent(id)}/undo`,
      body ?? {},
      idempotencyKey,
    );
  }

  // -- 8. POST /transformations/{id}/replay -------------------------------------------------
  async replayTransformation(id: TransformationId, body?: ReplayRequest): Promise<ReplayOutcome> {
    return this.request<ReplayOutcome>(
      "POST",
      `/transformations/${encodeURIComponent(id)}/replay`,
      body,
    );
  }

  // -- 9. GET /transformations/{id} ---------------------------------------------------------
  async getTransformation(id: TransformationId): Promise<TransformationView> {
    return this.request<TransformationView>("GET", `/transformations/${encodeURIComponent(id)}`);
  }

  // -- 10. GET /receipts/{tid} --------------------------------------------------------------
  async getReceipt(id: TransformationId): Promise<Receipt> {
    return this.request<Receipt>("GET", `/receipts/${encodeURIComponent(id)}`);
  }

  // -- 11. GET /ledger/proof?leaf= ----------------------------------------------------------
  async ledgerProof(leaf: string | number): Promise<InclusionProof> {
    return this.request<InclusionProof>("GET", `/ledger/proof?leaf=${encodeURIComponent(String(leaf))}`);
  }

  // -- 12. GET /ledger/checkpoint -----------------------------------------------------------
  async ledgerCheckpoint(): Promise<Checkpoint> {
    return this.request<Checkpoint>("GET", "/ledger/checkpoint");
  }

  /** `GET /ledger/consistency?from=&to=` is a 44 §2.7 extension, not §2.1's own row (see this class's
   * header) -- exposed all the same because it costs one method and a caller checkpoint-pinning
   * across two reads needs it. Declared here rather than silently reachable only via `raw`. */
  async ledgerConsistency(from: number, to: number): Promise<ConsistencyProof> {
    return this.request<ConsistencyProof>("GET", `/ledger/consistency?from=${from}&to=${to}`);
  }

  /** `GET /candidates` (44 §2.7 extension, `crates/gx-api/src/list.rs::candidates`) -- every row
   * that has not reached a terminal state (43 §0's broad-sense Candidate; `Denied` is included, per
   * the handler's own doc). Cursor is 44 §2.7's opaque string, unlike `listVerdictCheckpoints`'s
   * integer one -- declared here rather than silently reachable only via `raw`. */
  async listCandidates(limit?: number, cursor?: string): Promise<Page<CandidateRow>> {
    const q = new URLSearchParams();
    if (limit !== undefined) q.set("limit", String(limit));
    if (cursor !== undefined) q.set("cursor", cursor);
    const qs = q.toString();
    return this.request<Page<CandidateRow>>("GET", `/candidates${qs ? `?${qs}` : ""}`);
  }

  /** `GET /escalations` (44 §2.7 extension, `crates/gx-api/src/list.rs::escalations`) -- the
   * tickets waiting for a person (rows whose state is `Escalated`). Declared here rather than
   * silently reachable only via `raw`. */
  async listEscalations(limit?: number, cursor?: string): Promise<Page<EscalationRow>> {
    const q = new URLSearchParams();
    if (limit !== undefined) q.set("limit", String(limit));
    if (cursor !== undefined) q.set("cursor", cursor);
    const qs = q.toString();
    return this.request<Page<EscalationRow>>("GET", `/escalations${qs ? `?${qs}` : ""}`);
  }

  /** `GET /transformations` (44 §2.7 extension, `crates/gx-api/src/list.rs::transformations`) --
   * every row, terminal states included; the audit list. Declared here rather than silently
   * reachable only via `raw`. */
  async listTransformations(limit?: number, cursor?: string): Promise<Page<TransformationRow>> {
    const q = new URLSearchParams();
    if (limit !== undefined) q.set("limit", String(limit));
    if (cursor !== undefined) q.set("cursor", cursor);
    const qs = q.toString();
    return this.request<Page<TransformationRow>>("GET", `/transformations${qs ? `?${qs}` : ""}`);
  }

  // -- 13. POST /verdict-checkpoints --------------------------------------------------------
  async issueVerdictCheckpoint(body?: IssueVerdictCheckpointRequest): Promise<VerdictCheckpoint> {
    return this.request<VerdictCheckpoint>("POST", "/verdict-checkpoints", body);
  }

  // -- 14. GET /verdict-checkpoints ---------------------------------------------------------
  async listVerdictCheckpoints(limit?: number, cursor?: number): Promise<VerdictCheckpointPage> {
    const q = new URLSearchParams();
    if (limit !== undefined) q.set("limit", String(limit));
    if (cursor !== undefined) q.set("cursor", String(cursor));
    const qs = q.toString();
    return this.request<VerdictCheckpointPage>("GET", `/verdict-checkpoints${qs ? `?${qs}` : ""}`);
  }

  // -- 15. GET /verdict-checkpoints/{window_end} --------------------------------------------
  async getVerdictCheckpoint(windowEnd: number): Promise<VerdictCheckpoint> {
    return this.request<VerdictCheckpoint>("GET", `/verdict-checkpoints/${windowEnd}`);
  }

  /** `POST /attach-sources` (44 §2.6 extension; `req/812` §3-R1, landed as `req/824` A4) --
   * register an external executor (Vercel / GitHub Actions / generic CI) whose *reports* the
   * pipeline will admit as observations. SS273: registering a source never grants Glovrex any
   * write toward it; the returned row's `coverage_verified` is `false` and its `limits` are
   * non-empty by construction. Retrying with the same `idempotencyKey` replays the SAME
   * registration (w824-attach_source-00003), never a second source. */
  async registerAttachSource(
    body: AttachSourceRegisterRequest,
    idempotencyKey?: string,
  ): Promise<AttachSourceView> {
    return this.request<AttachSourceView>("POST", "/attach-sources", body, idempotencyKey);
  }

  /** `GET /attach-sources` (44 §2.6 extension, `req/824` A4) -- the registry as a zero-inclusive
   * census: an empty registry answers `total: 0` explicitly (F-B), and the number is never a
   * billing input (F4/F6). Cursor is the numeric chain-index kind, as `listVerdictCheckpoints`. */
  async listAttachSources(limit?: number, cursor?: number): Promise<AttachSourcePage> {
    const q = new URLSearchParams();
    if (limit !== undefined) q.set("limit", String(limit));
    if (cursor !== undefined) q.set("cursor", String(cursor));
    const qs = q.toString();
    return this.request<AttachSourcePage>("GET", `/attach-sources${qs ? `?${qs}` : ""}`);
  }

  /** `GET /attach-sources/{id}` (44 §2.6 extension, `req/824` A4) -- inspect one registration. An
   * id the registry never held is `404 SOURCE_UNKNOWN` -- deliberately a different word from
   * `NOT_FOUND`, so "your source is unregistered" and "that deploy does not exist" are never the
   * same sentence (44 §2.3 v0.5-s). */
  async getAttachSource(id: string): Promise<AttachSourceView> {
    return this.request<AttachSourceView>("GET", `/attach-sources/${encodeURIComponent(id)}`);
  }

  // -- 16. GET /stream -----------------------------------------------------------------------
  /**
   * The JSONL event stream (44 §2.2). Returns the raw `Response` rather than a parsed iterator:
   * 44 names ten event shapes and no envelope-parsing library is declared (principle ② of the
   * five, zero runtime dependencies; sem: SEM-sdk-typescript-006),
   * so a caller reads `response.body` (a `ReadableStream`) and splits on newlines themselves --
   * or scope §3 files an SSE/JSONL client as v0.2.
   */
  async stream(since?: string): Promise<Response> {
    const path = since ? `/stream?since=${encodeURIComponent(since)}` : "/stream";
    return this.fetchRaw("GET", path);
  }

  // -- 17. GET /healthz -----------------------------------------------------------------------
  async healthz(): Promise<{ status: string; engine_version: string }> {
    return this.request<{ status: string; engine_version: string }>("GET", "/healthz", undefined, undefined, true);
  }

  /**
   * Escape hatch for anything this SDK's v0.1 has not named a method for -- declared rather than
   * hidden, per this class's header. All four 44 §2.6 extension endpoints now have a dedicated
   * method (`ledgerConsistency`, `listCandidates`, `listEscalations`, `listTransformations`
   * above, per the class header); `raw` remains for the general case.
   */
  async raw<T = unknown>(
    method: "GET" | "POST",
    path: string,
    body?: unknown,
  ): Promise<T> {
    return this.request<T>(method, path, body);
  }

  private async request<T>(
    method: "GET" | "POST",
    path: string,
    body?: unknown,
    idempotencyKey?: string,
    skipAuth = false,
  ): Promise<T> {
    const response = await this.fetchRaw(method, path, body, idempotencyKey, skipAuth);
    const text = await response.text();
    if (response.ok) {
      return text.length === 0 ? (undefined as T) : (JSON.parse(text) as T);
    }
    let problem: ProblemDetail | undefined;
    try {
      const parsed = JSON.parse(text) as Partial<ProblemDetail>;
      if (typeof parsed.gx_code === "string") {
        problem = parsed as ProblemDetail;
      }
    } catch {
      // not JSON at all -- falls through to GxTransportError below
    }
    if (problem) {
      throw new GxApiError(problem);
    }
    throw new GxTransportError(response.status, text);
  }

  private async fetchRaw(
    method: "GET" | "POST",
    path: string,
    body?: unknown,
    idempotencyKey?: string,
    skipAuth = false,
  ): Promise<Response> {
    const headers: Record<string, string> = {};
    if (body !== undefined) headers["content-type"] = "application/json";
    if (!skipAuth && this.token !== undefined) {
      headers.authorization = `Bearer ${this.token}`;
    }
    if (idempotencyKey !== undefined) headers["idempotency-key"] = idempotencyKey;
    const init: RequestInit = { method, headers };
    if (body !== undefined) init.body = JSON.stringify(body);
    return this.fetchImpl(`${this.baseUrl}${BASE_PATH}${path}`, init);
  }
}
