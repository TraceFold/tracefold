// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * tracefold -- typed HTTP client + offline receipt verification for a Glovrex (gx) deployment.
 *
 * Thin projection (`req/132` §1): every export below is a direct projection of 44's API spec or
 * 42's data model. This module is the **only** import path a consumer (or a future GUI, R-GUI-on-SDK
 * / AC-P4-5) needs -- `test/gui_probe_import_boundary.test.ts` greps this package's own `dist/` for
 * any deep import reaching under a consumer's `node_modules/tracefold/dist/*` other than this file,
 * so that "smooth to build a GUI on" has a machine definition rather than a claim.
 */

export { GxClient, SPECIFIED_METHODS, EXTENSION_METHODS } from "./client.js";
export type { GxClientOptions } from "./client.js";

export { GxApiError, GxTransportError, GX_CODES } from "./errors.js";
export type { GxCode, ProblemDetail } from "./errors.js";

export { verifyReceiptOffline } from "./verify.js";
export type { OfflineVerifyResult, OfflineVerifyChecks, InclusionCheck } from "./verify.js";

export type {
  Cid,
  TransformationId,
  IntentId,
  KeyId,
  Rfc3339,
  SubstrateKind,
  ChangeContext,
  Actor,
  Lifecycle,
  AbortReason,
  VerdictKind,
  Fingerprint,
  DsseSignature,
  Receipt,
  InclusionProof,
  ConsistencyProof,
  Checkpoint,
  VerdictTally,
  VerdictCheckpoint,
  Page,
  CreateCandidateRequest,
  VerifyRequest,
  CommitRequest,
  EscalationRequest,
  CancelRequest,
  UndoRequest,
  ReplayRequest,
  IssueVerdictCheckpointRequest,
  CandidateCreated,
  CandidateView,
  TransformationView,
  VerifyOutcome,
  EscalationOutcome,
  CancelOutcome,
  UndoOutcome,
  ReplayOutcome,
  VerdictCheckpointPage,
  // 🔴 **R11 / `req/240` L-06 (i)** — 42 §3.12's six words, exported so that a caller can switch
  // on them rather than widen them.
  InverseStatus,
  // 44 §2.7 list rows -- return element of `listCandidates`/`listEscalations`/
  // `listTransformations` (`client.ts`), not just a `raw()` reader's shape anymore.
  CandidateRow,
  EscalationRow,
  TransformationRow,
} from "./types.js";
