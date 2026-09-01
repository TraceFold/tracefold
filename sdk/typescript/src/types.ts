// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * The wire vocabulary of 42 (data model) and 44 (API spec), as TypeScript types.
 *
 * Rule 1 (`req/132` §1; sem: SEM-sdk-typescript-009): this SDK holds no semantic authority, so
 * nothing here is invented structure —
 * every type is either a leaf identifier 42 names (`TransformationId`, `Cid`, `KeyId`, ...), a
 * request body 44 §2.2 documents field-for-field, or a response shape read off the real handler
 * (`crates/gx-api/src/handlers.rs`) rather than guessed from 44's prose, where the two differ (44
 * §2.2 is the *intent*; the handler is what a caller actually receives, and this SDK is a projection
 * of the second, exactly `crates/gx-cli` is one crate up).
 *
 * # Why some fields are `Record<string, unknown>` rather than a fully-typed nested shape
 *
 * `Transformation`, `AdmitProof`, `EscalationTicket` and the DSSE `Receipt` envelope's `payload` are
 * assembled by `serde_json::json!()` in the handler rather than derived from one `#[derive(Serialize)]`
 * struct end to end (see `crates/gx-api/src/handlers.rs`'s own header: "no canonical encode, no
 * `Verdict` constructed"). A TypeScript type that pinned every nested field would be *this SDK*
 * inventing a schema the source itself does not commit to in one place -- the opposite of "turn the
 * engine's vocabulary into TS types as it is" (`req/132` §1; sem: SEM-sdk-typescript-010). So this
 * file types what 44 documents as a field-level
 * contract precisely, and leaves what 44 calls "adapter-defined" or what the handler assembles ad hoc
 * as a loosely-typed JSON value a caller can read with their own narrowing.
 */

/** 42 §1.2: `gx1:<base32>`, textual form of every content-addressed id. */
export type Cid = string;

/** 42 §3.2's `TransformationId` (`Cid`-shaped) and `IntentId` (also `Cid`-shaped, ASM-11). */
export type TransformationId = string;
export type IntentId = string;

/** 42 §3.2: `KeyId = String`, the same namespace as a DSSE `keyid`. */
export type KeyId = string;

/** 44 §0: RFC 3339, the API layer's own responsibility to convert from `Timestamp` (ns epoch). */
export type Rfc3339 = string;

/** 42 §3.1's three shipped substrates plus 42 §3.1's `Custom(String)` escape hatch. */
export type SubstrateKind = "fs" | "git" | "mcp" | `custom:${string}`;

/** 42 §3.2's `ChangeContext`: six unit variants as bare strings, `Custom(String)` as `{Custom}`. */
export type ChangeContext =
  | "Time"
  | "Evidence"
  | "Policy"
  | "Model"
  | "Representation"
  | "Substrate"
  | { Custom: string };

/**
 * 42 §3.2's `Actor`, externally tagged (serde's default for a fieldful enum) -- verified against
 * `crates/gx-api/tests/endpoints.rs`'s own request bodies (`{"Human":{"key":...}}`).
 */
export type Actor =
  | { Human: { key: KeyId } }
  | { Agent: { key: KeyId; model: string } }
  | { Process: { key: KeyId } };

/** 43's state names, one-for-one (44 §0: "43 defines the state names ... used verbatim, character
 * for character"; sem: SEM-sdk-typescript-011). */
export type Lifecycle =
  | "Draft"
  | "Candidate"
  | "Verifying"
  | "Admitted"
  | "Denied"
  | "Escalated"
  | "Canonicalized"
  | "Committing"
  | "Committed"
  | { Aborted: AbortReason }
  | "Superseded";

/**
 * 41 §3 / ASM-15's six reasons a `Committing` pipeline can end in `Aborted`, plus M5-11's seventh.
 *
 * `PostconditionMismatch` (`req/919` A1) is the plan's promised post-state digest disagreeing with
 * the one `apply` observed. It can only reach a client from an engine whose adapter fills
 * `PlannedDelta.promised_target`; no adapter shipped with gx does yet, so a client that has never
 * seen this string is reading a true picture of today rather than a stale one.
 */
export type AbortReason =
  | "PreconditionChanged"
  | "ApplyFailed"
  | "VerifierUnavailable"
  | "Expired"
  | "OwnerCancelled"
  | "InternalError"
  | "PostconditionMismatch";

/** The three-valued verdict kind (`GET /candidates/{id}`'s `verdict` field: a bare string or `null`). */
export type VerdictKind = "Admit" | "Deny" | "Escalate";

/** 42 §3.5's `Fingerprint`, as the `FingerprintRecord` JSON `fingerprint_json` (handlers.rs) emits. */
export interface Fingerprint {
  substrate: SubstrateKind;
  scope: string;
  digest: Cid;
}

/** 42 §3.10's `DsseSignature`. Permanently these two fields: the algorithm is a property of the
 * verifier's pinned key, never a wire field, and a wire-carried algorithm name must never drive
 * crypto dispatch (req/38 §109; DSSE issue #35; RFC 8725 §3.1; 33 NFR-011 closing note; sem:
 * SEM-sdk-typescript-012). */
export interface DsseSignature {
  keyid: KeyId;
  /** base64 in JSON (44 §2.2), 42 §3.10. */
  sig: string;
}

/** 42 §3.10's `Receipt = DsseEnvelope` (three fields) plus `issued_at`, unsigned (E-M2-6). */
export interface Receipt {
  envelope: {
    payload_type: string;
    /** base64 of `canonical_dagcbor(ReceiptPayload)` (44 §2.2). Decode only if you need the payload;
     * offline verification (`verifyReceiptOffline`, `req/132` §2 item 2) reads this envelope itself. */
    payload: string;
    signatures: DsseSignature[];
  };
  issued_at: number;
  /** DR-44-9 (`req/38` §168 ruling 1, `req/187` §5): the signed payload's readable half, decoded by
   * the API layer so a window needs no DAG-CBOR decoder. Present on `GET /receipts/{tid}` **only**
   * -- the commit/undo composites and `GET /transformations/{id}`'s `receipt` interior are still
   * the bare pair. `null` when the payload is not canonical DAG-CBOR (the document is still served:
   * refusing to hand over a receipt because this server could not read it would withhold the
   * artefact a stranger needs in order to establish that it is malformed). */
  receipt_view?: ReceiptView | null;
  /** L-02 (`req/38` §369 item 1, R44-E): the fourth key `GET /receipts/{tid}` mounts beside the
   * document -- what the **server** is at read time, never a grade of the receipt itself. Present on
   * `GET /receipts/{tid}` **only**, on the same terms as `receipt_view`: added under 44 §2.6's
   * backward-compatible rule, so `envelope` and `issued_at` are byte-for-byte what an offline
   * verifier always read and it ignores this third member (DSSE: consumers ignore unrecognized
   * fields). It lets a reader tell a good document from a deployment currently in dispute from a
   * good document from a well deployment -- the distinction this SDK could not draw while the field
   * fell off `[extra: string]: unknown` into `unknown`. See `ServerHealth` for what each word means. */
  server_health?: ServerHealth;
  /** Fields the real commit/undo handlers add beside the envelope (not in 42 §3.10's own table --
   * see this file's header). Present on a `POST .../commit` or `POST .../undo` response. */
  transformation?: TransformationId;
  state?: string;
  enforced?: boolean;
  at?: Rfc3339;
  [extra: string]: unknown;
}

/** DR-44-9's decoded receipt view (`GET /receipts/{tid}`, `req/38` §168 ruling 1).
 *
 * Every member is read off the **signed payload** (`gx_witness::ReceiptPayload`) or off
 * `Receipt.issued_at`; nothing is read from the server's live table or ledger, which
 * `crates/gx-api/tests/dr44_9_views.rs` measures by rewriting the world and requiring the object
 * not to move.
 *
 * **Two things this deliberately does not carry.** There is no `verified` / `refuted` /
 * `inclusion`: HTTP carries the proof and does not grade it, and a signature check plus an
 * inclusion check is `gx receipt verify` (44 §1.2, DR-44-4) run where the reader is -- a desktop
 * shell proxies to the CLI rather than asking the server to mark its own paper. And there is **no
 * `alg`**, permanently: 33 NFR-011's closing note (`req/38` §109 = DR-46-5 (b)) makes the algorithm
 * a property of the **key**, and `req/38` §113 extended the "nothing alg-like beside a signature"
 * census from `DsseSignature` to its carriers. A reader who wants the algorithm resolves `key_id`
 * against the key they pinned. */
export interface ReceiptView {
  /** `payload.transformation` -- the receipt's own subject, not the path segment that was asked
   * for. */
  subject: TransformationId;
  /** `payload.inclusion_proof.tree_size`; `null` on a receipt with no inclusion proof, which ASM-14
   * makes every `VerdictReceipt`. */
  tree_size: number | null;
  /** `payload.inclusion_proof.leaf_index`; `null` for the same reason. */
  leaf_index: number | null;
  /** The root **this receipt's own audit path reconstructs** from its own leaf -- a restatement of
   * the proof, not an attestation that the server's tree has that root today. Same spelling as
   * `Checkpoint.root_hash` (42 §1.2 gives a `Cid` one readable form), so the comparison a reader
   * wants is string equality. `null` when there is no inclusion proof, or when the path does not
   * land on a root. */
  root: Cid | null;
  /** `payload.key_id`, from inside the signed bytes. */
  key_id: KeyId;
  /** `payload.postcondition_fingerprint`, base64 (M2H1-4's spelling for raw bytes); `null` when
   * nothing was applied (42 §3.10). */
  postcondition_fingerprint: string | null;
  /** The same instant as `Receipt.issued_at`, as 44 §0's RFC 3339. Two spellings, one value, and
   * **neither is signed** (E-M2-6, CM-5). */
  issued_at: Rfc3339;
}

/** The four words `Receipt.server_health.status` can carry (`crates/gx-api/src/handlers.rs`'s
 * `server_health`, L-02 / `req/38` §369 item 1).
 *
 * - `"ok"` -- the journal and the ledger describe one tree and this server's writer is open.
 * - `"degraded"` -- a `Nature::Meta` file is gone, so writes are refused; `GET /healthz` still
 *   answers `200`, and the receipt beside this object is unaffected.
 * - `"unhealthy"` -- `ledger_agrees` is false (`GET /healthz` answers `500 LEDGER_DISAGREES` for the
 *   same condition): the two files describe different trees, this server signs no checkpoint and
 *   refuses every write while it holds, so an inclusion proof compared against the head it publishes
 *   now proves nothing. The signed receipt is still served and still worth verifying offline.
 * - `"unknown"` -- this server could not read its own journal or ledger to answer, so it does not
 *   claim to be well. It is **not** folded into `"unhealthy"`: "the two files disagree" is a
 *   finding, and "I could not look" is the absence of one -- the same distinction `receipt_view`
 *   being `null` draws for a payload that will not decode. */
export type HealthStatus = "ok" | "degraded" | "unhealthy" | "unknown";

/** `Receipt.server_health`: the object `crates/gx-api/src/handlers.rs`'s `server_health` mounts
 * beside a receipt on `GET /receipts/{tid}` (L-02 / `req/38` §369 item 1, R44-E).
 *
 * Two fields, by construction. There is deliberately **no** `verified` / `refuted` / `inclusion`:
 * for DR-44-9's reason a server does not grade its own receipts, and this object says what the
 * server *is*, not whether the paper beside it is good. The reading is bounded-stale (the same
 * `250 ms` cache `GET /healthz` reads, invalidated by any write), which is a property of the
 * server, not of the signed bytes -- this field is outside the signature and cannot reach it. */
export interface ServerHealth {
  status: HealthStatus;
  /** The engine's own sentence for a `"degraded"`, `"unhealthy"` or `"unknown"` status -- the same
   * clause `GET /healthz` prints, chosen rather than composed (R32 / `req/392` M-02). `null` for
   * `"ok"`. */
  status_reason: string | null;
}

/** 42 §3.11's `InclusionProof`. */
export interface InclusionProof {
  leaf_index: number;
  tree_size: number;
  audit_path: Cid[];
}

/** 42 §3.11's `ConsistencyProof`. */
export interface ConsistencyProof {
  old_size: number;
  new_size: number;
  path: Cid[];
}

/** `GET /ledger/consistency?from=&to=` -> `200` (DR-44-1 (a) + DR-44-9).
 *
 * The proof itself is still the body -- `old_size` / `new_size` / `path` at the top level, agreeing
 * with the CLI twin (`gx log consistency`), with `GET /ledger/proof`'s bare `InclusionProof` and
 * with `ConsistencyProof` above, which is what DR-44-1 (a) settled. DR-44-9 adds the judgement
 * beside it, and this endpoint is the **single stated exception** to "this surface carries proofs
 * and does not grade them" (`req/38` §168 ruling 1 (3); the reason is in 44 §2.7's v0.4-s addendum
 * and in `crates/gx-api/src/list.rs`): without it a ledger face's one alarm read
 * `consistent === false` against a wire that had no such key, so a genuinely broken chain raised
 * nothing -- unreachable rather than wrong.
 *
 * **`consistent` is not third-party verification.** It means this deployment rebuilt both roots
 * from the path it just produced and they matched the roots it holds. A reader who does not trust
 * the deployment runs the CLI twin or feeds `path` to their own verifier. A face may say "the
 * operator's own server reports its log grew rather than being rewritten"; it may not spell this
 * `verified` or `refuted`, which are `gx_witness::InclusionCheck`'s words for a check the reader
 * ran. */
export interface LedgerConsistencyAnswer extends ConsistencyProof {
  /** `gx_log::proof::verify_consistency` over this proof and the two roots -- see the caveat
   * above. */
  consistent: boolean;
  /** The tree sizes the judgement is about. Equal to `old_size` / `new_size` **by construction**
   * (the roots are taken at the proof's own sizes), pinned by
   * `crates/gx-api/tests/dr44_9_views.rs` so the pair can never become a second account of which
   * trees were compared. */
  checked_from: number;
  checked_to: number;
}

/** 42 §3.11's `Checkpoint` (signed tree head). `timestamp` is nanoseconds, unconverted (M6H5-5). */
export interface Checkpoint {
  origin: string;
  tree_size: number;
  root_hash: Cid;
  timestamp: number;
  signature: DsseSignature;
}

/** 42 §3.14's `VerdictTally` -- four counts, `unverdicted` being T-4e's fail-open-admitted bucket. */
export interface VerdictTally {
  deny: number;
  admit: number;
  escalate: number;
  unverdicted: number;
}

/** 42 §3.14's `VerdictCheckpoint`. */
export interface VerdictCheckpoint {
  origin: string;
  tally: VerdictTally;
  window_start: number;
  window_end: number;
  ledger_root_hash: Cid | null;
  ledger_tree_size: number;
  timestamp: number;
  signature: DsseSignature;
  /** DR-44-9 (`req/38` §168 ruling 1 (4), `req/187` §5): `window_start` / `window_end` are **verdict
   * sequence numbers** (42 §3.14), not clock readings and not ledger indices -- a reader took them
   * for times, derived a range from them and got a confidently wrong answer. These two are the API
   * layer resolving those coordinates against the journal: the time of the **first** verdict the
   * window speaks for (`window_start`, inclusive) and of the **last** (`window_end - 1`, since the
   * upper bound is exclusive).
   *
   * `null` for an empty window (`window_start === window_end`, the document a second `POST` with
   * nothing between produces) and for a boundary this journal cannot resolve. **Outside the signed
   * core** -- present only on the three HTTP answers, absent from a checkpoint read from a file or
   * from the CLI, and invisible to a verifier that rebuilds the signing bytes from the typed
   * fields. */
  window_start_at?: Rfc3339 | null;
  window_end_at?: Rfc3339 | null;
}

/** A page of list results (44 §2.7's `{items, next_cursor}` shape, cursor opaque). */
export interface Page<T> {
  items: T[];
  next_cursor: string | null;
}

// ---------------------------------------------------------------------------
// Request bodies -- 44 §2.2's field tables, one type per endpoint that takes one
// ---------------------------------------------------------------------------

/** `POST /candidates` (44 §2.2's table, `CreateCandidate` in `handlers.rs`). */
export interface CreateCandidateRequest {
  substrate: SubstrateKind;
  locator: string;
  /** 44 §2.2: "adapter-defined" (sem: SEM-sdk-typescript-013). A string is taken as UTF-8 bytes
   * (M6H5-7); anything else is compact
   * JSON -- see `CreateCandidate::goal_bytes` in `crates/gx-api/src/handlers.rs`. */
  goal: unknown;
  context: ChangeContext;
  actor: Actor;
  /** 44 §2.2: "default 0" (sem: SEM-sdk-typescript-014). v0.1 accepts only 0 (M6H3-5: refused
   * rather than silently dropped). */
  order?: 0;
  /** 44 §2.2: "default `[]`" (sem: SEM-sdk-typescript-015). v0.1 accepts only `[]` (M6H3-5, same
   * reason as `order`). */
  parents?: [];
}

/** `POST /candidates/{id}/verify`'s optional body (44 §2.2). */
export interface VerifyRequest {
  evidence?: unknown[];
  record_only?: boolean;
}

/** `POST /candidates/{id}/commit`'s optional body (E-M6-20, `req/38` §52). */
export interface CommitRequest {
  record_only?: boolean;
}

/** `POST /candidates/{id}/escalation` (44 §2.2, DR-11). */
export interface EscalationRequest {
  decision: "approve" | "reject";
  reason: string;
  actor: Actor;
}

/** `POST /candidates/{id}/cancel` (44 §2.2, DR-11). Echoed and unchecked in v0.1 (M5H6-4, adopted
 * (a); sem: SEM-sdk-typescript-016). */
export interface CancelRequest {
  actor: Actor;
}

/** `POST /transformations/{id}/undo`'s body (44 §2.2). `actor` is echoed, never selected (P-5). */
export interface UndoRequest {
  actor?: Actor;
}

/** `POST /transformations/{id}/replay`'s optional body (44 §2.2, E-M6-6: journal-indexed). */
export interface ReplayRequest {
  from?: number;
  to?: number;
  dry_run?: boolean;
}

/** `POST /verdict-checkpoints`'s optional body (`req/119` §4). */
export interface IssueVerdictCheckpointRequest {
  origin?: string;
}

// ---------------------------------------------------------------------------
// Response bodies, read off the handlers (`crates/gx-api/src/handlers.rs`)
// ---------------------------------------------------------------------------

/** `POST /candidates` -> `201` (handler: `id` + `transformation` both present). */
export interface CandidateCreated {
  transformation: Record<string, unknown>;
  id: TransformationId;
  precondition_fingerprint: Fingerprint;
  state: Lifecycle;
  created_at: Rfc3339;
}

/** `GET /candidates/{id}` -> `{ transformation, state, verdict, fingerprint }` (44 §2.2). */
export interface CandidateView {
  transformation: Record<string, unknown>;
  state: Lifecycle | null;
  verdict: VerdictKind | null;
  fingerprint: Fingerprint | null;
}

/** 🔴 **R29** (`req/38` §238 ruling 1, from the twenty-eighth adversarial audit `req/361` H-01) --
 * what became of 43 T-10c's roll-back, as the engine measured it.
 *
 * Four words, and the fourth is new in this window. Until R29 the engine wrote `"Succeeded"` the
 * moment the adapter answered `Ok` to the compensating inverse, without reading the object back --
 * so the word meant *the adapter accepted the call*, and clients read it as *the object is home*.
 * The audit drove a contract-conforming adapter whose `apply` fails halfway and got
 * `"Succeeded"` printed over a world with a record duplicated in it.
 *
 * - `"Succeeded"` -- accepted **and** the object was read again and found at the fingerprint it
 *   started from. Since R29 this is a statement about the object, not about the call.
 * - `"Diverged"` -- accepted, and the object is **not** back where it started. Do not treat the
 *   change as taken back. It does not say the roll-back moved it: a writer of your own inside the
 *   window lands here too (`docs/LIMITS.md` v0.5-p).
 * - `"Failed"` -- the adapter refused the inverse, **or** the read-back that would confirm the
 *   homecoming could not be taken at all.
 * - `"NotAttempted"` -- there was no executable inverse to send.
 */
export type Rollback = "NotAttempted" | "Succeeded" | "Failed" | "Diverged";

/** `GET /transformations/{id}` -> `{ transformation, state, receipt, superseded_by, rollback,
 * inverse_status }` (44 §2.2; `rollback` added by R29 and `inverse_status` by Owner #260, both
 * under 44 §2.6's backward-compatible addition). */
export interface TransformationView {
  transformation: Record<string, unknown>;
  state: Lifecycle | null;
  receipt: Receipt | null;
  superseded_by: TransformationId | null;
  /** 🔴 **R29 / `req/361` §3-1** -- `null` where no roll-back was in question. */
  rollback: Rollback | null;
  /** 🔴 **Owner #260 / `req/987`** -- 42 §3.12's status of this row's escrowed inverse, the same
   * value and the same spelling `TransformationRow` carries on the list. `null` for a
   * transformation with no escrow row at all, which is the *only* thing `null` means here.
   *
   * It says **whether** an inverse can still be run and never **what would come back**: no
   * locator, no substrate, no digest of the escrowed bytes. A consent screen that must show the
   * loss before it asks needs the descriptor `req/987` §4-2 specifies, which is not on this face. */
  inverse_status: InverseStatus | null;
}

/** `POST /candidates/{id}/verify` -> `200` (handler's actual body, wider than 44 §2.2's summary). */
export interface VerifyOutcome {
  transformation: TransformationId;
  state: Lifecycle;
  verdict: VerdictKind | null;
  proof: Record<string, unknown> | null;
  reasons: unknown[] | null;
  ticket: Record<string, unknown> | null;
  enforced: boolean;
  fail_posture_engaged: boolean;
  held_by: TransformationId | null;
  record_only: boolean;
  at: Rfc3339;
}

/** `POST /candidates/{id}/escalation` -> `200` (handler's seven keys, censused by
 * `crates/gx-api/tests/wire_census.rs`). v0.4-l (`req/189` M-06): `reason` / `ruled_by` /
 * `signed_by` / `at` are named rather than left to an index signature -- `signed_by` is the key
 * that signed the ruling receipt (INV-S6: without it a ruling signed by the wrong key is invisible
 * at this surface), `null` only if the receipt could not be read back. */
export interface EscalationOutcome {
  transformation: TransformationId;
  state: Lifecycle;
  decision: VerdictKind;
  reason: string;
  ruled_by: Actor;
  signed_by: string | null;
  at: Rfc3339;
}

/** `POST /candidates/{id}/cancel` -> `200` (44 §2.2's own shape: state is a bare string, not
 * `Lifecycle`'s tagged form -- see the handler's own comment). */
export interface CancelOutcome {
  transformation: TransformationId;
  state: "Aborted";
  reason: AbortReason | null;
  actor_unchecked: Actor;
  at: Rfc3339;
}

/** `POST /transformations/{id}/undo` -> `200` (the new `Receipt` plus what it superseded). */
export interface UndoOutcome extends Receipt {
  undone: TransformationId;
  superseded_state: Lifecycle | null;
}

/** `POST /transformations/{id}/replay` -> `200` (44 §2.2, M6-26, adopted (a); sem:
 * SEM-sdk-typescript-017). */
export interface ReplayOutcome {
  matches: boolean;
  /** v0.4-l (`req/189` M-06): the wire's `diffs` entries are objects, not strings (the handler's
   * `json!({component, transformation, journal_ledger_seq, ledger_index})`). */
  diffs: ReplayDiff[];
  /** The denominator M6-26 asks not to be dropped: which components this replay did *not* check. */
  unchecked: string[];
  /** How many journal records the replay covered (44 §2.2; the `from`/`to` window, or the
   * transformation's own records). */
  records_replayed: number;
  /** Echo of the request's `dry_run` (default `false`). */
  dry_run: boolean;
}

/** One `ReplayOutcome.diffs` entry: the one component with a second copy (the ledger) disagreeing
 * with the journal about a transformation's leaf. `ledger_index` is `null` when the ledger holds
 * no leaf for it at all. */
export interface ReplayDiff {
  component: "ledger";
  transformation: TransformationId;
  journal_ledger_seq: number;
  ledger_index: number | null;
}

/** `GET /verdict-checkpoints` -> `200`. v0.4-l (`req/189` M-06): **not** `Page<T>` -- this
 * endpoint's cursor is the chain's own 0-based position (an integer, `crates/gx-api/src/
 * verdict_checkpoints.rs`'s module header says why `window_end` will not do) and the page carries
 * a `total` 44 §2.7's regulation does not have (44 §2.2's own declaration for this endpoint). */
export interface VerdictCheckpointPage {
  items: VerdictCheckpoint[];
  next_cursor: number | null;
  total: number;
}

// ---------------------------------------------------------------------------
// List rows -- 44 §2.7 v0.4-k/v0.4-l addenda (`crates/gx-api/src/list.rs`). Named methods
// (`listCandidates`, `listEscalations`, `listTransformations`, `client.ts`) return `Page<T>` of
// these directly; a `raw()` caller reading the same paths gets the same shape untyped.
// ---------------------------------------------------------------------------

/** One `GET /candidates` row (44 §2.7 v0.4-k 4 keys + v0.4-l M-15's three, `req/189`).
 * `created_at` / `actor` / `scope` are the engine's own answers off the live row and are `null`
 * for a row this process no longer holds the body of (after a restart: every row, M5H3-5). */
export interface CandidateRow {
  transformation: TransformationId;
  state: Lifecycle | null;
  verdict: VerdictKind | null;
  enforced: boolean | null;
  created_at: Rfc3339 | null;
  actor: Actor | null;
  /** 42 §3.5's `Fingerprint.scope` -- the locator half (ASM-69-1), and **the "target" a person
   * reads on a row** (DR-44-9, `req/38` §168 ruling 1 (5)). `Transformation` also has a field
   * spelled `target` (`Cid | null`, the *expected post-state digest*); it is not this column, and
   * wiring `target -> target` -- the most natural line anybody would write -- puts a content hash
   * where a human reads a name. A graph face draws its lanes on `scope` for the same reason: an
   * axis that is a digest has one row per lane and says nothing. */
  scope: string | null;
}

/**
 * 42 §3.12's `InverseStatus`, as `gx-engine` serialises it (serde's external tagging: six of the
 * seven are bare strings and `Consumed` is an object naming the transformation that used the
 * inverse).
 *
 * 🔴 **R11 / `req/240` L-06 (i), audit 9 L-02** — six words, not `unknown`. The type said
 * `unknown | null`, which is the one shape a caller cannot switch on: DR-1(a)'s wedge is verified
 * undo and "which of these can I still undo" is a question answered by exactly this field, so an
 * SDK that widened it to `unknown` handed the question back. `BodyMissing` is R8's sixth
 * (`req/234` B-5): the escrow row names an inverse whose body is no longer in
 * `.gx/ledger/journal.blobs/` -- the receipt still proves the commit and the undo can no longer be
 * run, which is a different fact from `Unavailable` ("`invert()` answered `None`").
 *
 * 🔴 **D-34 / DR-46-13** — and the **seventh**, which this type was missing. `gx-engine`'s
 * `InverseStatus` has seven members; the union above listed six, so `Undetermined` decoded off the
 * wire into a value no arm of a caller's `switch` could name and TypeScript would not warn them.
 * That is R11's own defect wearing the opposite face: R11 refused `unknown` because a caller
 * cannot switch on it, and an incomplete union is a type that lets a caller *believe* they
 * switched exhaustively when they did not.
 *
 * The word is the half of `req/38` §198 ruling (b) that DR-46-26 gave a writer: `invert()` was
 * **not asked and could not find out** — the prior that would not be read under
 * `OnReadFailure::Unknown`. It is not `Unavailable`: "there is no undo" and "nobody established
 * whether there is an undo" have different remedies, and the second one is a posture a deployment
 * chose and can unchoose.
 */
export type InverseStatus =
  | "Available"
  | "Expired"
  | "Unavailable"
  | "Pending"
  | "BodyMissing"
  | "Undetermined"
  | { Consumed: { by: TransformationId } };

/** One `GET /transformations` row: `CandidateRow`'s seven plus `superseded_by` and
 * `inverse_status` (M6H6-15; `null` for a transformation with no escrow row at all). */
export interface TransformationRow extends CandidateRow {
  superseded_by: TransformationId | null;
  inverse_status: InverseStatus | null;
  /** 🔴 **R29 / `req/361` §3-1** -- what became of 43 T-10c's roll-back on this row, `null` where
   * none was in question. On the list as well as on the row for `inverse_status`'s reason: "which
   * of these left my object somewhere it should not be" is a question about a set. */
  rollback: Rollback | null;
}

/** One `GET /escalations` row (44 §2.7 v0.4-k, 7 keys). v0.4-l (`req/189` H-04): only rows whose
 * `state` is `Escalated` are listed -- a ruled, cancelled or expired row leaves the queue. */
export interface EscalationRow {
  transformation: TransformationId;
  ticket_id: string;
  state: Lifecycle | null;
  reasons: unknown[];
  required_approval: unknown;
  created_at: Rfc3339;
  deadline: Rfc3339 | null;
}

/** What an attach-source CLAIMS it reports (`req/wire/schema/attach_source.schema.json`,
 * `registerRequest.declared_coverage`). Glovrex does not verify the claim -- the response's
 * `coverage_verified: false` says so, and that marking is the whole honesty of the row
 * (`req/824` A4 LIMITS; `req/841` §2-4a). */
export interface AttachSourceDeclaredCoverage {
  envset?: boolean;
  deploys?: boolean;
  logs?: boolean;
  config?: boolean;
}

/** `POST /attach-sources` body (`req/812` §3-R1 / `req/824` A4). An attach-source is an external
 * executor we can only *receive reports from* -- never a SubstrateAdapter (SS273 forbids the
 * write half permanently; the schema's own header sentence). A `kind` outside the union is
 * refused at decode server-side (`422 VALIDATION_ERROR`), never defaulted. */
export interface AttachSourceRegisterRequest {
  kind: "vercel" | "github-actions" | "generic-ci";
  name: string;
  declared_coverage: AttachSourceDeclaredCoverage;
  /** Optional. Absent means observations from this source are authenticated by the membrane's
   * Bearer token only -- and the registered row says so in its `limits`. */
  pubkey?: string;
}

/** One registered attach-source, as `POST /attach-sources`, `GET /attach-sources` items and
 * `GET /attach-sources/{id}` all render it (`registerResponse` in the wire schema). */
export interface AttachSourceView {
  id: string;
  kind: string;
  registered_at: Rfc3339;
  declared_coverage: AttachSourceDeclaredCoverage;
  /** Always `false` in this phase, and present ANYWAY: omitting it would let a reader take
   * `declared_coverage` for a measured fact -- the single most likely misreading of this whole
   * surface (wire schema, `coverage_verified`'s own description). */
  coverage_verified: boolean;
  /** What this attachment cannot observe, per P-12 (`req/805` P-19). Never empty. */
  limits: string[];
  pubkey?: string;
}

/** `GET /attach-sources` page. `total` is a zero-inclusive census (F-B: an empty registry answers
 * `total: 0` explicitly), and constitutionally never a billing meter (F4/F6). The cursor is the
 * numeric chain-index kind (`verdict_checkpoints` precedent), unlike the opaque-string cursor of
 * the `Page<T>` lists. */
export interface AttachSourcePage {
  items: AttachSourceView[];
  next_cursor: number | null;
  total: number;
}
