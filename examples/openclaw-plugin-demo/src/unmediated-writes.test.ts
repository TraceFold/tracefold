// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * One invariant, guarded: **no byte reaches the substrate except through the membrane.**
 *
 * Provenance. Both cases below were reported by **ANP2 Network**, in a comment on the dev.to post
 * about this plugin -- an outside reader reading the published source. Neither was found by us, and
 * both were confirmed against the code before being fixed. They are written here as negative
 * controls, so the reason they were once real is a thing this repo can re-run rather than a thing it
 * asserts.
 *
 *  1. `Admit` used to `return { params }`, letting OpenClaw's own `write` re-apply the bytes gx had
 *     already applied. The old comment defended this as convergent -- "the native write re-applies
 *     the same bytes". ANP2's sharper reading: the re-application is *unconditional*. Anything that
 *     lands between gx's commit and the native write (a later hook in the same chain, a background
 *     process, a file watcher, the agent's own unmediated `bash`) is overwritten by a write nothing
 *     escrowed. No concurrency is required; a single caller reaches this.
 *
 *  2. The plugin covered `write` only. During a membrane outage `write` was blocked *with an
 *     explanation*, and that explanation is delivered to the model as the tool result -- while
 *     `edit` and `apply_patch` passed through untouched. The block was therefore a routing hint to a
 *     tool that still moved bytes.
 *
 * This file needs no gx, no WSL, no network and no filesystem: both defects live entirely in the
 * plugin's decision logic, which is the layer under test. `demo.ts` is what exercises a real engine.
 *
 *   node src/unmediated-writes.test.ts
 */

import { MEDIATED_TOOLS, makeBeforeToolCallHandler, type PluginConfig } from "./plugin.ts";
import { runToolCall, type ToolCallEvent } from "./harness.ts";
import type { EscrowOutcome, GxMembrane, ProposedEffect, UndoOutcome } from "./membrane.ts";

// ---------------------------------------------------------------------------------------------
// Reporting -- same shape demo.ts uses, so a red line reads the same in both.
// ---------------------------------------------------------------------------------------------

let failures = 0;

function check(label: string, ok: boolean, detail: string): void {
  if (!ok) failures += 1;
  console.log(`  [${ok ? "OK  " : "FAIL"}] ${label}: ${detail}`);
}

// ---------------------------------------------------------------------------------------------
// The substrate, and the native tool that is allowed to touch it
// ---------------------------------------------------------------------------------------------

/** One file, and a count of every byte movement, tagged by who made it. */
interface Substrate {
  bytes: string;
  /** Writes the membrane made. These are escrowed and have an inverse. */
  mediated: number;
  /** Writes OpenClaw's own tool body made. These are escrowed by nobody. */
  native: number;
}

/**
 * OpenClaw's `write` tool body, in the only two behaviours it has.
 *
 * Read out of their tree rather than assumed (`../../../GitRepo/openclaw`,
 * `src/agents/sessions/tools/write.ts`): the tool takes a precheck of the file's current content,
 * and when that content is already byte-identical to `content` it returns `{ changed: false }` as a
 * terminal no-op without calling `writeFile`. In every other case it calls
 * `ops.writeFile(absolutePath, content)` **unconditionally** -- there is no parameter in
 * `writeSchema` (`{ path, content }`) by which a caller could make that write conditional on the
 * file still holding some expected digest.
 *
 * That asymmetry is the whole of defect 1: undisturbed, the re-application is a genuine no-op, which
 * is why the old comment looked true. Disturbed, it is a silent overwrite.
 */
function nativeWrite(disk: Substrate, params: Record<string, unknown>): Promise<void> {
  const content = String(params["content"]);
  if (disk.bytes === content) return Promise.resolve(); // terminal no-op: already identical
  disk.bytes = content;
  disk.native += 1;
  return Promise.resolve();
}

/** Stands in for any tool body that mutates the substrate without being `write`. */
function nativeMutate(disk: Substrate, marker: string): Promise<void> {
  disk.bytes = marker;
  disk.native += 1;
  return Promise.resolve();
}

// ---------------------------------------------------------------------------------------------
// Membranes
// ---------------------------------------------------------------------------------------------

const TRANSFORMATION = "gx1:testtransformationidforthenegativecontrols";

/**
 * A membrane that admits.
 *
 * It applies inside `escrow()`, because that is what the real one does: gx has no
 * escrow-without-apply verb, so `commit` escrows the inverse and applies, atomically. By the time
 * the hook has an `Admit` in hand, the bytes are already in the substrate.
 */
class AdmittingMembrane implements GxMembrane {
  private readonly disk: Substrate;

  constructor(disk: Substrate) {
    this.disk = disk;
  }

  escrow(effect: ProposedEffect): Promise<EscrowOutcome> {
    this.disk.bytes = effect.desiredBytes;
    this.disk.mediated += 1;
    return Promise.resolve({
      verdict: "Admit",
      transformationId: TRANSFORMATION,
      reason: null,
      policyId: "fs-permit-default",
      commitReceiptPath: "/tmp/does-not-exist/commit.json",
      applied: true,
    });
  }

  undo(_transformationId: string): Promise<UndoOutcome> {
    return Promise.resolve({ ok: true, receiptPath: null, reason: null });
  }
}

/** A membrane nothing answers on. Its verdict is `Unknown`, and stays `Unknown`. */
class UnreachableMembrane implements GxMembrane {
  escrow(_effect: ProposedEffect): Promise<EscrowOutcome> {
    return Promise.resolve({
      verdict: "Unknown",
      transformationId: null,
      reason: "no gx deployment answered on the configured surface",
      policyId: null,
      commitReceiptPath: null,
      applied: false,
    });
  }
  undo(_transformationId: string): Promise<UndoOutcome> {
    return Promise.resolve({ ok: false, receiptPath: null, reason: "unreachable" });
  }
}

// ---------------------------------------------------------------------------------------------

/**
 * What this test *demands* be mediated, fixed here rather than read from the plugin.
 *
 * Every built-in OpenClaw tool that puts bytes at a path the caller names. Enumerated from their
 * tree twice, in two spellings, because the first sweep got it wrong: `name: "..."` across
 * `src/agents/sessions/tools/*.ts` yields seven tools (`bash`, `edit`, `find`, `grep`, `ls`,
 * `read`, `write`) and no `apply_patch` -- which lives one directory up, in
 * `src/agents/apply-patch.ts:128`. A test that took the first count would have demanded coverage of
 * two tools and passed while `apply_patch` walked through.
 *
 * `bash` is deliberately absent: an arbitrary command has no locator to escrow, so it cannot be put
 * through the membrane at all. That gap is declared in `docs/LIMITS.md` -- statically, where a
 * reader finds it, and not at runtime where a model would.
 */
const MUST_BE_MEDIATED = ["write", "edit", "apply_patch"] as const;

const ORIGINAL = "notes for the release\n- ship the thing\n";
const DESIRED = "notes for the release\n- ship the thing\n- AGENT APPENDED THIS LINE\n";
const INTERLOPER = "notes for the release\n- ship the thing\n- SOMEONE ELSE WROTE THIS\n";

function configFor(membrane: GxMembrane): PluginConfig {
  return {
    membrane,
    tools: MEDIATED_TOOLS,
    actorModel: "unmediated-writes-test",
    escrowed: [],
  };
}

function writeEvent(content: string): ToolCallEvent {
  return {
    toolName: "write",
    toolCallId: "call-1",
    params: { path: "/workspace/notes.txt", content },
  };
}

// ---------------------------------------------------------------------------------------------
// DEFECT 1 -- the window between gx's commit and OpenClaw's own write
// ---------------------------------------------------------------------------------------------

/**
 * The control that shows why the old comment looked true: undisturbed, the native re-application
 * moves no bytes, because the tool's own precheck finds the file already identical.
 *
 * This one passes both before and after the fix. It is here so the failure below cannot be read as
 * "the native tool is always destructive" -- it is destructive exactly when something intervened.
 */
async function undisturbedWindow(): Promise<void> {
  console.log("DEFECT 1 control -- nothing intervenes: the native re-application is a real no-op");
  const disk: Substrate = { bytes: ORIGINAL, mediated: 0, native: 0 };
  const cfg = configFor(new AdmittingMembrane(disk));

  await runToolCall(makeBeforeToolCallHandler(cfg), writeEvent(DESIRED), (p) =>
    nativeWrite(disk, p),
  );

  check("the membrane applied the change", disk.mediated === 1, `mediated=${disk.mediated}`);
  check("the native tool moved no bytes", disk.native === 0, `native writes=${disk.native}`);
  check("the file holds what the agent asked for", disk.bytes === DESIRED, "as asked");
  console.log("");
}

/**
 * The negative control for defect 1.
 *
 * A third party lands a change in the window between gx's commit and the native tool. One caller,
 * no concurrency. Before the fix the hook returned `{ params }`, the native tool ran, found bytes
 * that were not what it was told to write, and overwrote them -- a byte movement no receipt covers
 * and no inverse can take back.
 */
async function driftInTheWindow(): Promise<void> {
  console.log("DEFECT 1 -- a change lands between gx's commit and OpenClaw's own write");
  const disk: Substrate = { bytes: ORIGINAL, mediated: 0, native: 0 };
  const cfg = configFor(new AdmittingMembrane(disk));

  const handler = makeBeforeToolCallHandler(cfg);
  // The third party writes in the window between the hook returning and the tool body running, and
  // it writes *whatever the hook decided* -- it is a background process, not something the plugin
  // gates. Modelling it inside the tool body instead would have made it fire only when the plugin
  // allowed the call, which is the one arrangement under which this defect cannot be seen.
  const result = await runToolCall(
    async (event, ctx) => {
      const decision = await handler(event, ctx);
      disk.bytes = INTERLOPER;
      return decision;
    },
    writeEvent(DESIRED),
    (p) => nativeWrite(disk, p),
  );

  check(
    "the membrane escrowed and applied the agent's change",
    disk.mediated === 1 && cfg.escrowed.length === 1,
    `mediated=${disk.mediated} escrowed=${cfg.escrowed.length}`,
  );
  check(
    "the native tool moved no bytes",
    disk.native === 0,
    disk.native === 0
      ? "the redundant re-application never ran"
      : `native writes=${disk.native} -- it overwrote what landed in the window`,
  );
  check(
    "the change that landed in the window was not silently destroyed",
    disk.bytes === INTERLOPER,
    disk.bytes === INTERLOPER
      ? "still there, and still visible to whoever has to reconcile it"
      : "gone, with no receipt naming its removal and no inverse able to restore it",
  );
  check(
    "the agent is told its write landed, and by which transformation",
    result.reason !== null && result.reason.includes(TRANSFORMATION),
    result.reason ?? "no reason given to the model",
  );
  console.log("");
}

// ---------------------------------------------------------------------------------------------
// DEFECT 2 -- a block that is a routing hint
// ---------------------------------------------------------------------------------------------

/**
 * The negative control for defect 2.
 *
 * With the membrane unreachable, every tool this plugin claims to mediate must stop. Before the fix
 * only `write` stopped, and its block reason -- which OpenClaw hands to the model verbatim as the
 * tool result (`buildBlockedToolResult`, `content: [{ type: "text", text: reason }]`) -- was
 * therefore an instruction the model could follow to `edit`, which still worked.
 */
async function outageLeavesNothingMoving(): Promise<void> {
  console.log("DEFECT 2 -- the membrane is unreachable: what can still move bytes?");

  const reasons: string[] = [];
  let moved = 0;

  for (const toolName of MUST_BE_MEDIATED) {
    const disk: Substrate = { bytes: ORIGINAL, mediated: 0, native: 0 };
    const cfg = configFor(new UnreachableMembrane());
    // Each tool in the spelling its own schema uses. Only `write` carries a full end state; the
    // other two describe a change the plugin cannot turn into one, which is the point.
    const params: Record<string, unknown> =
      toolName === "write"
        ? { path: "/workspace/notes.txt", content: DESIRED }
        : toolName === "edit"
          ? {
              path: "/workspace/notes.txt",
              old_string: "ship the thing",
              new_string: "ship something else",
            }
          : { input: "*** Begin Patch\n*** Update File: /workspace/notes.txt\n" };

    const result = await runToolCall(
      makeBeforeToolCallHandler(cfg),
      { toolName, toolCallId: `call-${toolName}`, params },
      toolName === "write" ? (p) => nativeWrite(disk, p) : () => nativeMutate(disk, INTERLOPER),
    );

    if (result.reason !== null) reasons.push(result.reason);
    moved += disk.native;

    check(
      `${toolName} stopped`,
      result.blocked && disk.native === 0,
      result.blocked ? "blocked before the tool body" : "PASSED THROUGH -- it still moves bytes",
    );
  }

  check(
    "nothing that writes bytes to a named path moved one during the outage",
    moved === 0,
    `native writes across ${MUST_BE_MEDIATED.length} tools = ${moved}`,
  );
  check(
    "the shipped tool list covers every tool this test demands",
    MUST_BE_MEDIATED.every((t) => MEDIATED_TOOLS.includes(t)),
    `shipped = [${MEDIATED_TOOLS.join(", ")}]`,
  );

  // The ethos rule (pillar 4) is to declare what is not covered -- statically, in docs/LIMITS.md,
  // not at runtime in an error the model reads. A block reason that names an uncovered tool is a
  // signpost pointing at the hole.
  const leaks = reasons.filter((r) => /\bbash\b/i.test(r));
  check(
    "no block reason names a tool that is still uncovered",
    leaks.length === 0,
    leaks.length === 0 ? "the gap is declared in docs/LIMITS.md, not in the model's context" : leaks[0]!,
  );

  // A tool the plugin has never claimed is still not touched by it.
  const disk: Substrate = { bytes: ORIGINAL, mediated: 0, native: 0 };
  const readResult = await runToolCall(
    makeBeforeToolCallHandler(configFor(new UnreachableMembrane())),
    { toolName: "read", toolCallId: "call-read", params: { path: "/workspace/notes.txt" } },
    () => Promise.resolve(),
  );
  check(
    "a tool this plugin does not own is still declined, not blocked",
    readResult.executed && !readResult.blocked,
    `executed=${readResult.executed}`,
  );
  console.log("");
}

// ---------------------------------------------------------------------------------------------

async function main(): Promise<void> {
  console.log("=== unmediated writes: negative controls for the two defects ANP2 Network found ===");
  console.log("");
  await undisturbedWindow();
  await driftInTheWindow();
  await outageLeavesNothingMoving();
  console.log("=== result ===");
  console.log(`failures = ${failures}`);
  console.log(`TESTS = ${failures === 0 ? "PASS" : "FAIL"}`);
  process.exitCode = failures === 0 ? 0 : 1;
}

main().catch((e: unknown) => {
  console.log(`fatal: ${String(e)}`);
  process.exitCode = 2;
});
