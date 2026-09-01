// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * The plugin: an OpenClaw `before_tool_call` handler that puts a filesystem write through the gx
 * membrane before OpenClaw's own tool is allowed to reach the disk.
 *
 * ## Why this hook and not another
 *
 * `before_tool_call` is the only point in the tool path where the *precondition still exists*.
 * `req/1031` established the three facts this design rests on, each read in OpenClaw's own source:
 *
 *  - `src/agents/agent-tools.before-tool-call.wrapper.ts:445-549` -- the hook result is evaluated
 *    first, and when it blocks, the real `execute()` is never called. The hook conditions the
 *    effect; it does not merely observe it.
 *  - `src/plugins/hooks.ts:1438-1509` -- `runBeforeToolCall` is sequential, may block, and may
 *    rewrite params. `runAfterToolCall` next to it is documented "fire-and-forget": after the fact
 *    there is nothing left to hold.
 *  - `src/plugins/hook-before-tool-call-result.ts` -- the result type carries `block`,
 *    `blockReason`, `params` and `requireApproval`, which is the entire vocabulary used below.
 *
 * ## What this file is not
 *
 * No line of OpenClaw source is reproduced here. The `api` and the event are typed `unknown`/`any`
 * and read defensively by field name, which is the amount of coupling calling a public API
 * requires and no more. The one thing taken from their tree as fact rather than assumption is the
 * `write` tool's parameter spelling (`src/agents/sessions/tools/write.ts:42-45` declares `path` and
 * `content`; `:259` shows `file_path` accepted as an alias) -- read so that this demo would not
 * ship a guessed schema wearing the clothes of a measured one.
 */

import type { GxMembrane, ProposedEffect, Verdict } from "./membrane.ts";

/**
 * The subset of OpenClaw's `PluginHookBeforeToolCallResult` this plugin ever produces.
 *
 * Declared locally and structurally: the field names are the wire contract that has to match for
 * the call to mean anything, and nothing beyond them is modelled.
 */
export interface GateDecision {
  params?: Record<string, unknown>;
  block?: boolean;
  blockReason?: string;
  requireApproval?: { title: string; description: string };
}

export interface PluginConfig {
  readonly membrane: GxMembrane;
  /** Which tool names go through the membrane. */
  readonly tools: readonly string[];
  /** Reported to gx as the actor's model. */
  readonly actorModel: string;
  /** Records every transformation this plugin escrowed, newest last, so an undo can find it. */
  readonly escrowed: { toolCallId: string | null; locator: string; transformationId: string }[];
  readonly log?: (line: string) => void;
}

/** The `write` tool's arguments, in the spelling its own schema declares. */
function readWriteParams(
  params: Record<string, unknown>,
): { locator: string; content: string } | null {
  const raw = params["file_path"] ?? params["path"];
  const content = params["content"];
  if (typeof raw !== "string" || raw.length === 0) return null;
  if (typeof content !== "string") return null;
  return { locator: raw, content };
}

function blockFor(verdict: Verdict, reason: string | null, policyId: string | null): GateDecision {
  const because = reason ?? "no reason given";
  const by = policyId ? ` (${policyId})` : "";
  return {
    block: true,
    blockReason:
      verdict === "Unknown"
        ? `gx could not be consulted, so this write was not allowed to land: ${because}. ` +
          `This is not a policy denial -- the membrane's answer is unknown, and the posture on ` +
          `unknown is closed.`
        : `gx ${verdict}${by}: ${because}`,
  };
}

/**
 * The handler.
 *
 * Returning `undefined` means "nothing to say about this call", which is how a hook declines a tool
 * it does not own.
 */
export function makeBeforeToolCallHandler(cfg: PluginConfig) {
  return async function beforeToolCall(event: any, _ctx?: unknown): Promise<GateDecision | undefined> {
    const toolName = typeof event?.toolName === "string" ? event.toolName : null;

    // Guarded here as well as by the registration matcher. The matcher's accepted shape
    // (`normalizePluginToolMatcher`) was not read, so this plugin does not depend on having got it
    // right: if the matcher admits more than intended, this line still declines.
    if (toolName === null || !cfg.tools.includes(toolName)) return undefined;

    const params = (event?.params ?? {}) as Record<string, unknown>;
    const parsed = readWriteParams(params);
    if (parsed === null) {
      // Fail closed. gx's stated posture is that a rule it cannot evaluate stops rather than
      // passes; a write whose target this plugin cannot name is exactly that case.
      return {
        block: true,
        blockReason:
          `gx-escrow could not read a target path and content out of this ${toolName} call, ` +
          `so it could not put the change through the membrane. Blocking rather than passing, ` +
          `because a change nobody escrowed is a change nobody can take back.`,
      };
    }

    cfg.log?.(`  hook fires on ${toolName} -> ${parsed.locator}`);

    const effect: ProposedEffect = {
      substrate: "fs",
      locator: parsed.locator,
      desiredBytes: parsed.content,
      actorModel: cfg.actorModel,
    };

    const outcome = await cfg.membrane.escrow(effect);

    if (outcome.verdict === "Admit") {
      if (outcome.transformationId !== null) {
        cfg.escrowed.push({
          toolCallId: typeof event?.toolCallId === "string" ? event.toolCallId : null,
          locator: parsed.locator,
          transformationId: outcome.transformationId,
        });
      }
      // Pass the call through unchanged. See README "What the native tool does after we return" --
      // the effect is already in the substrate at this point, and the native write re-applies the
      // same bytes. That is a real property of this design, not an omission.
      return { params };
    }

    if (outcome.verdict === "Escalate") {
      // The engine asked for a human. OpenClaw already owns that conversation, on whichever
      // channel the operator is using, so hand it back rather than inventing a second one.
      return {
        requireApproval: {
          title: `gx escalated a write to ${parsed.locator}`,
          description:
            `The gate did not refuse this change, but it did not admit it either: ` +
            `${outcome.reason ?? "no reason given"}. ` +
            `Approving lets it through the membrane, where an inverse is escrowed before it lands.`,
        },
      };
    }

    return blockFor(outcome.verdict, outcome.reason, outcome.policyId);
  };
}

/**
 * Registration, in the shape `extensions/onepassword/index.ts:125` uses: `api.on("before_tool_call",
 * handler)` inside `register(api)`.
 *
 * `api` is `any` on purpose. Importing `openclaw/plugin-sdk` would give a checked signature, and a
 * real plugin should; this demo does not take the dependency because it has not been run inside
 * OpenClaw, and a type-checked call is not evidence of a working one.
 */
export function register(api: any, cfg: PluginConfig): void {
  api.on("before_tool_call", makeBeforeToolCallHandler(cfg), {
    // Shape unverified -- see README's untestable list. The handler's own guard is what actually
    // holds the scope.
    matcher: { tools: [...cfg.tools] },
  });
}
