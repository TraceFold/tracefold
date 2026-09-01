// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * The seam.
 *
 * Everything above this file speaks about a *proposed effect* and a *verdict*. Everything below it
 * is bound to one way of reaching a gx deployment. `req/1032` §3 records why the seam is here and
 * not one layer up or down: a plugin that spawns a CLI in its hook body has fused the membrane to a
 * process model, and the second consumer (an HTTP deployment, a long-lived engine) cannot reuse it.
 *
 * Only one implementation ships in this demo (`gx-cli-membrane.ts`). Per the seam-first rule, the
 * second implementation is what decides whether this interface was drawn in the right place, and
 * this demo has not written a second one -- so the seam is a proposal, not a proven boundary.
 */

/**
 * The engine's three answers, kept three.
 *
 * `Unknown` is not a failure and not a denial: it is the state where the membrane could not be
 * consulted at all. Folding it into `Deny` would make this demo's own tooling break the first
 * principle the product is built on (`docs/LIMITS.md`; the audit-side statement of the same rule is
 * in the project's memory as "untestable is not failed"). The caller decides what posture to take
 * on `Unknown` -- this layer only refuses to lie about which of the three it saw.
 */
export type Verdict = "Admit" | "Deny" | "Escalate" | "Unknown";

/** A change an agent wants made, before anything has been touched. */
export interface ProposedEffect {
  /** Which substrate the change is against. Only `fs` is wired in this demo. */
  readonly substrate: "fs";
  /** The position inside it, in the adapter's own spelling. For `fs`, an absolute path. */
  readonly locator: string;
  /** The intended end state, in full. The `write` tool's `content` is exactly this. */
  readonly desiredBytes: string;
  /** The model asking, which gx requires for `--actor-kind agent` and a reviewer cannot recover from the key. */
  readonly actorModel: string;
}

export interface EscrowOutcome {
  readonly verdict: Verdict;
  /** The `gx1:` id, when the walk got far enough to fix one. */
  readonly transformationId: string | null;
  /** Why, in the engine's words. Never this layer's paraphrase of them. */
  readonly reason: string | null;
  /** Which shipped policy decided, when a policy did. */
  readonly policyId: string | null;
  /** Where the signed commit receipt landed, when one was issued. */
  readonly commitReceiptPath: string | null;
  /**
   * Whether the effect has already landed in the substrate.
   *
   * `true` on `Admit`, because gx has no escrow-without-apply verb: `commit` escrows the inverse and
   * applies, atomically. See README "What the native tool does after we return".
   */
  readonly applied: boolean;
}

export interface UndoOutcome {
  readonly ok: boolean;
  readonly receiptPath: string | null;
  readonly reason: string | null;
}

/**
 * The one thing the plugin is allowed to know about gx.
 *
 * This is a consumer of a shipped surface, not a reimplementation of one: no method here computes a
 * verdict, an inverse, a digest, or a receipt. It asks, and reports what it was told.
 */
export interface GxMembrane {
  /** Put the proposed effect through the membrane and come back with the engine's answer. */
  escrow(effect: ProposedEffect): Promise<EscrowOutcome>;
  /** Commit the inverse gx escrowed before it applied `transformationId`. */
  undo(transformationId: string): Promise<UndoOutcome>;
}
