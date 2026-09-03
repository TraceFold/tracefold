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

/**
 * Every tool this plugin puts through the membrane.
 *
 * One list, exported, because the four call sites that used to spell `["write"]` each are four
 * places for the scope to drift apart. `register()`'s matcher is built from the same array.
 *
 * The set is every built-in OpenClaw tool that puts bytes at a path the caller names, enumerated
 * from their tree: `write` and `edit` (`src/agents/sessions/tools/`), and `apply_patch`
 * (`src/agents/apply-patch.ts:128`, one directory up -- a first sweep of the tools directory alone
 * misses it).
 *
 * `bash` is **not** here and cannot be: an arbitrary command has no locator to escrow, so there is
 * nothing to put through the membrane. That gap is declared in `docs/LIMITS.md`, which is the
 * honest place for it -- see the note on `blockFor` about why it must not be declared at runtime.
 *
 * Of these three, only `write` carries a full intended end state in its parameters. `edit` and
 * `apply_patch` describe a delta, and this plugin does not reimplement their semantics to recover
 * the end state from it -- so they are mediated by being **refused**, via the same fail-closed
 * branch that catches a `write` whose target cannot be read. Covered here means stopped, not
 * escrowed, and the README says so in those words.
 */
export const MEDIATED_TOOLS: readonly string[] = ["write", "edit", "apply_patch"];

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

/**
 * The text of a refusal.
 *
 * OpenClaw hands `blockReason` to the model verbatim as the tool result
 * (`buildBlockedToolResult`: `content: [{ type: "text", text: reason }]`), so every word here lands
 * in the agent's context. ANP2 Network's second report was about exactly that: when only `write`
 * was mediated, a courteous outage message on `write` was a usable hint that `edit` still moved
 * bytes -- the blast radius of an outage was "writes move".
 *
 * The fix is not to make this text vaguer. This project's fourth pillar is to declare what is not
 * covered, and a vague refusal would trade one honesty for another. The fix is that the set of
 * tools left uncovered no longer contains one that writes to a named path (`MEDIATED_TOOLS`), and
 * that the gap which genuinely remains -- `bash` -- is written down in `docs/LIMITS.md`, where a
 * reader deciding whether to deploy this finds it, and not in a runtime string where only the model
 * being gated finds it. Declare the hole statically; do not narrate it to the agent.
 */
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
      //
      // This is also where `edit` and `apply_patch` land, every time: they describe a delta rather
      // than an end state, and this plugin does not reimplement their semantics to recover one. It
      // covers them by stopping them. That is worse for an agent and better for the invariant, and
      // it is the whole reason they are in `MEDIATED_TOOLS` rather than left to walk through.
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
      // The effect is already in the substrate: gx has no escrow-without-apply verb, so `commit`
      // escrowed the inverse and applied, atomically, before this line was reached. Whatever the
      // native tool does now, it does *after* the membrane and outside it.
      //
      // This used to `return { params }` and let it run, defended as convergent re-application.
      // ANP2 Network's reading is the correct one: the re-application is **unconditional**.
      // `writeSchema` is `{ path, content }` and nothing else, so no parameter this hook could
      // return makes the native write conditional on the file still holding the digest gx signed
      // for. Anything landing in the window between the commit above and that write -- a later
      // hook in this same chain, a background process, a file watcher, the agent's own unmediated
      // `bash` -- is silently overwritten by bytes no receipt covers. It takes one caller; no
      // concurrency is involved.
      //
      // So the call is stopped instead. `block` is the only terminal instrument in the hook's
      // result vocabulary: `runBeforeToolCall`'s merge is `stickyTrue(acc?.block, next.block)` with
      // `shouldStop: block === true`, and the wrapper turns a plugin block into `kind: "veto"` and
      // returns `blockToolCall()` without ever calling `execute()`. Returned `params`, by contrast,
      // are `lastDefined(acc?.params, next.params)` -- a later plugin overwrites them -- which is a
      // second reason a params rewrite could not have carried this guarantee.
      //
      // The cost, stated rather than hidden: OpenClaw records this call as blocked, and emits a
      // blocked diagnostic and security event, for a write that in fact succeeded. The reason text
      // below is handed to the model verbatim as the tool result, so the agent is told the truth
      // even though the envelope says "blocked". README carries this trade in full.
      const applied = outcome.transformationId ?? "an unrecorded transformation";
      const receipt = outcome.commitReceiptPath;
      return {
        block: true,
        blockReason:
          `gx admitted this write and has already applied it, inside the membrane: ` +
          `${applied}${receipt === null ? "" : ` (receipt ${receipt})`}. ` +
          `${parsed.locator} now holds exactly the content you asked for, and an inverse was ` +
          `escrowed before it landed, so it can be taken back. ` +
          `The native write was suppressed on purpose: it would re-apply these bytes ` +
          `unconditionally, and a second write that nothing escrowed is a write nothing can take ` +
          `back. Nothing further is needed -- this change is done.`,
      };
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
 *
 * `matcher` shape: measured, not guessed, as of the first real install (the req after `1034`). The
 * first attempt passed `{ tools: [...] }`, which is what `req/1034` §5-3 flagged as unverified. A
 * real `openclaw plugins install` run failed registration with `TypeError: tool hook matcher must be
 * an array of tool names` -- so the matcher is the bare array, not an object wrapping it. The
 * handler's own tool-name guard (`readWriteParams`'s caller above) is what actually holds the scope
 * regardless of which shape the matcher takes, which is why the earlier wrong shape never mis-scoped
 * anything -- it just never installed.
 */
export function register(api: any, cfg: PluginConfig): void {
  api.on("before_tool_call", makeBeforeToolCallHandler(cfg), {
    matcher: [...cfg.tools],
  });
}
