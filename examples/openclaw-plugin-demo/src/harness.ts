// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * A stand-in for OpenClaw's tool wrapper.
 *
 * This is NOT OpenClaw. It is a driver that calls a `before_tool_call` handler in the order
 * `req/1031` §1-4 recorded from `src/agents/agent-tools.before-tool-call.wrapper.ts:445-549`:
 *
 *   1. run the hook
 *   2. if it blocked, return the blocked result -- `execute()` is never called
 *   3. otherwise merge the hook's params over the prepared ones
 *   4. then, and only then, invoke the tool
 *
 * The ordering is the only thing being reproduced, and it is reproduced from a written description
 * of the sequence rather than from their code. Everything this harness proves is therefore a
 * statement about *the plugin*, conditional on OpenClaw firing hooks in the order its source was
 * observed to fire them. It is not a statement about OpenClaw running this plugin -- that has not
 * happened. See README's untestable list.
 */

import type { GateDecision } from "./plugin.ts";

export interface ToolCallEvent {
  readonly toolName: string;
  readonly toolCallId: string;
  readonly params: Record<string, unknown>;
}

export interface ToolCallResult {
  readonly executed: boolean;
  readonly blocked: boolean;
  readonly escalated: boolean;
  readonly reason: string | null;
}

export type HookHandler = (
  event: ToolCallEvent,
  ctx?: unknown,
) => Promise<GateDecision | undefined>;

/** The tool body: what would actually touch the world. */
export type ToolExecute = (params: Record<string, unknown>) => Promise<void>;

export async function runToolCall(
  handler: HookHandler,
  event: ToolCallEvent,
  execute: ToolExecute,
): Promise<ToolCallResult> {
  const decision = await handler(event);

  if (decision?.block === true) {
    return {
      executed: false,
      blocked: true,
      escalated: false,
      reason: decision.blockReason ?? null,
    };
  }

  if (decision?.requireApproval !== undefined) {
    // A real deployment routes this to a human on an approval channel. This harness has no human,
    // and treating "waiting for approval" as "approved" would be the demo lying to itself, so it
    // stops here and reports that it stopped.
    return {
      executed: false,
      blocked: true,
      escalated: true,
      reason: decision.requireApproval.description,
    };
  }

  const finalParams = decision?.params ?? event.params;
  await execute(finalParams);
  return { executed: true, blocked: false, escalated: false, reason: null };
}
