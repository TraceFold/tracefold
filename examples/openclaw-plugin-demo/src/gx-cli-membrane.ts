// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * A `GxMembrane` bound to the `gx` CLI surface.
 *
 * Surface discipline (`SKILL.md` §0): this file consumes `gx submit | plan | verify | commit | undo`
 * and reads their JSON. It does not link the engine, re-derive a verdict, or reconstruct a receipt.
 * Every judgement in an `EscrowOutcome` is a field the engine printed.
 *
 * Why the CLI and not the typed HTTP client (`sdk/typescript`, `GxClient`): both are shipped
 * surfaces and either would satisfy §0. The CLI is the one this demo could *measure* today -- the
 * HTTP path needs a running `gx serve` whose bed and lifecycle this demo does not manage. The
 * `GxMembrane` seam exists precisely so that swapping in a `GxHttpMembrane` touches no other file.
 * That swap is NOT written and NOT tested here; see README's untestable list.
 */

import { spawn } from "node:child_process";
import { EOL } from "node:os";
import type {
  EscrowOutcome,
  GxMembrane,
  ProposedEffect,
  UndoOutcome,
  Verdict,
} from "./membrane.ts";

export interface GxCliMembraneOptions {
  /**
   * How to reach the binary. The demo runs a Node process on Windows against a Linux `gx`, so the
   * default is a WSL hop; a native deployment passes `{ command: "gx", args: [] }` and nothing else
   * in this file changes.
   */
  readonly command: string;
  readonly args: readonly string[];
  /** The gx binary itself, which must sit after the `env` prefix and before the verb. */
  readonly binary: string;
  /** Absolute path, in the substrate's spelling, of the gx project bed. */
  readonly projectDir: string;
  /** `HOME` for the gx process, which is where it keeps `.gx/keys`. */
  readonly homeDir: string;
  /** The actor key id the plugin signs as. */
  readonly actorKeyId: string;
  /** Where diagnostics go. */
  readonly log?: (line: string) => void;
}

export interface RunResult {
  readonly code: number;
  readonly stdout: string;
  readonly stderr: string;
}

/**
 * Run one gx invocation.
 *
 * Exported because the demo's own verification steps (`log checkpoint`, `receipt verify --offline`)
 * are not plugin behaviour and must not be reachable through `GxMembrane` -- a plugin that could
 * mint its own checkpoints would be checking its own homework.
 */
export function runGx(
  opts: Pick<GxCliMembraneOptions, "command" | "args" | "homeDir" | "binary">,
  verb: readonly string[],
  stdin?: string,
): Promise<RunResult> {
  // Order matters and cost this demo one red run: `env` sets the environment *for* the binary, so
  // it precedes it. Putting the binary first made gx read "env" as a subcommand and answer
  // "unrecognized subcommand", which the membrane correctly reported as Unknown -- the assertion
  // that caught it was "the block names the shipped policy that decided", because an Unknown was
  // standing where a Deny belonged.
  const argv = [...opts.args, "env", `HOME=${opts.homeDir}`, opts.binary, ...verb];
  return new Promise((resolve) => {
    const child = spawn(opts.command, argv, { windowsHide: true });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => (stdout += String(d)));
    child.stderr.on("data", (d) => (stderr += String(d)));
    child.on("error", (e) =>
      resolve({ code: -1, stdout: "", stderr: `spawn failed: ${String(e)}` }),
    );
    child.on("close", (code) => resolve({ code: code ?? -1, stdout, stderr }));
    if (stdin !== undefined) {
      child.stdin.write(stdin);
    }
    child.stdin.end();
  });
}

/** The engine's own words, when it refused in a shape we can read. */
interface GxProblem {
  readonly title?: string;
  readonly detail?: string;
  readonly gx_code?: string;
}

export class GxCliMembrane implements GxMembrane {
  // A plain field rather than a constructor parameter property, so that every file here stays
  // erasable TypeScript and `node src/demo.ts` needs no build step and no `node_modules`.
  private readonly opts: GxCliMembraneOptions;

  constructor(opts: GxCliMembraneOptions) {
    this.opts = opts;
  }

  private log(line: string): void {
    this.opts.log?.(line);
  }

  /**
   * Run one gx verb.
   *
   * `HOME` is set through `env(1)` rather than a shell string so that no argument in this file is
   * ever parsed by a shell -- paths in this demo contain characters a shell would take an interest
   * in, and quoting them correctly across a WSL hop is a class of bug worth not having.
   */
  private run(verb: readonly string[], stdin?: string): Promise<RunResult> {
    return runGx(this.opts, verb, stdin);
  }

  /** gx prints one JSON document per invocation; a run that printed none is a run we cannot read. */
  private static parse(out: string): Record<string, unknown> | null {
    const line = out
      .split(/\r?\n/)
      .map((s) => s.trim())
      .find((s) => s.startsWith("{"));
    if (line === undefined) return null;
    try {
      return JSON.parse(line) as Record<string, unknown>;
    } catch {
      return null;
    }
  }

  private static problemOf(doc: Record<string, unknown> | null): string | null {
    if (doc === null) return null;
    const p = doc as GxProblem;
    if (typeof p.gx_code !== "string") return null;
    return `${p.gx_code}: ${p.detail ?? p.title ?? ""}`.trim();
  }

  /** Everything gx could not answer is `Unknown`, and says so with the engine's own text. */
  private static unknown(reason: string): EscrowOutcome {
    return {
      verdict: "Unknown",
      transformationId: null,
      reason,
      policyId: null,
      commitReceiptPath: null,
      applied: false,
    };
  }

  async escrow(effect: ProposedEffect): Promise<EscrowOutcome> {
    const project = ["--project", this.opts.projectDir];

    // T-1 submit. The intended end state travels on stdin so that this demo never has to place a
    // temporary file inside a filesystem the engine may not be able to snapshot.
    const submit = await this.run(
      [
        ...project,
        "submit",
        "--substrate",
        effect.substrate,
        "--locator",
        effect.locator,
        "--intent",
        "-",
        "--context",
        "Model",
        "--actor-key",
        this.opts.actorKeyId,
        "--actor-kind",
        "agent",
        "--actor-model",
        effect.actorModel,
      ],
      effect.desiredBytes,
    );
    const submitDoc = GxCliMembrane.parse(submit.stdout);
    const intentId = submitDoc?.["intent_id"];
    if (typeof intentId !== "string") {
      return GxCliMembrane.unknown(
        GxCliMembrane.problemOf(submitDoc) ??
          `submit produced no intent_id (exit ${submit.code}) ${submit.stderr.trim()}`,
      );
    }
    this.log(`  gx submit    -> intent ${intentId}`);

    // T-2 plan. This is where the precondition is snapshotted, which is the whole reason the hook
    // has to sit before the effect rather than after it.
    const plan = await this.run([...project, "plan", intentId]);
    const planDoc = GxCliMembrane.parse(plan.stdout);
    const transformation = (planDoc?.["transformation"] ?? {}) as Record<string, unknown>;
    const tid = transformation["id"];
    if (typeof tid !== "string") {
      return GxCliMembrane.unknown(
        GxCliMembrane.problemOf(planDoc) ??
          `plan produced no transformation id (exit ${plan.code}) ${plan.stderr.trim()}`,
      );
    }
    const pre = (planDoc?.["precondition_fingerprint"] ?? {}) as Record<string, unknown>;
    this.log(
      `  gx plan      -> ${tid}${
        typeof pre["digest"] === "string" ? `  precondition ${String(pre["digest"]).slice(0, 24)}...` : ""
      }`,
    );

    // T-3/T-4 verify. The gate answers; this file does not.
    const verify = await this.run([...project, "verify", tid]);
    const verifyDoc = GxCliMembrane.parse(verify.stdout);
    const kind = verifyDoc?.["kind"];
    if (typeof kind !== "string") {
      return GxCliMembrane.unknown(
        GxCliMembrane.problemOf(verifyDoc) ??
          `verify produced no verdict (exit ${verify.code}) ${verify.stderr.trim()}`,
      );
    }

    const reasons = verifyDoc?.["reasons"];
    const firstReason = Array.isArray(reasons) && reasons.length > 0
      ? (reasons[0] as Record<string, unknown>)
      : null;
    const reasonText = typeof firstReason?.["message"] === "string"
      ? (firstReason["message"] as string)
      : null;

    const proof = (verifyDoc?.["proof"] ?? null) as Record<string, unknown> | null;
    const decisions = proof?.["policy_decisions"];
    const policyId = Array.isArray(decisions) && decisions.length > 0 &&
        typeof (decisions[0] as Record<string, unknown>)["policy_id"] === "string"
      ? ((decisions[0] as Record<string, unknown>)["policy_id"] as string)
      : (() => {
          const src = firstReason?.["source"] as Record<string, unknown> | undefined;
          const pol = src?.["Policy"] as Record<string, unknown> | undefined;
          return typeof pol?.["policy_id"] === "string" ? (pol["policy_id"] as string) : null;
        })();

    this.log(`  gx verify    -> ${kind}${policyId ? `  by ${policyId}` : ""}`);

    // The engine's vocabulary is passed through, not widened. An answer this demo has never seen
    // stays `Unknown` rather than being guessed into one of the two it knows.
    if (kind === "Deny" || kind === "Escalate") {
      return {
        verdict: kind as Verdict,
        transformationId: tid,
        reason: reasonText,
        policyId,
        commitReceiptPath: null,
        applied: false,
      };
    }
    if (kind !== "Admit") {
      return {
        ...GxCliMembrane.unknown(`verify answered "${kind}", which this plugin does not model`),
        transformationId: tid,
      };
    }

    // T-8..T-11 commit: escrow the inverse, re-check the precondition, apply, issue the receipt.
    const commit = await this.run([...project, "commit", tid]);
    const commitDoc = GxCliMembrane.parse(commit.stdout);
    const state = commitDoc?.["state"];
    if (state !== "Committed") {
      return {
        ...GxCliMembrane.unknown(
          GxCliMembrane.problemOf(commitDoc) ??
            `commit did not reach Committed (exit ${commit.code}) ${commit.stderr.trim()}`,
        ),
        transformationId: tid,
      };
    }
    const storedAt = commitDoc?.["stored_at"];
    this.log(`  gx commit    -> Committed  receipt ${typeof storedAt === "string" ? storedAt : "?"}`);

    return {
      verdict: "Admit",
      transformationId: tid,
      reason: null,
      policyId,
      commitReceiptPath: typeof storedAt === "string" ? storedAt : null,
      applied: true,
    };
  }

  async undo(transformationId: string): Promise<UndoOutcome> {
    // `--settle 0` because the fs adapter is read-after-write consistent in this demo's bed; a
    // deployment against a substrate that is not should leave the default pre-flight alone.
    const res = await this.run([
      "--project",
      this.opts.projectDir,
      "undo",
      transformationId,
      "--settle",
      "0",
    ]);
    const doc = GxCliMembrane.parse(res.stdout);
    if (doc?.["state"] !== "Committed") {
      return {
        ok: false,
        receiptPath: null,
        reason: GxCliMembrane.problemOf(doc) ?? `undo did not commit (exit ${res.code})${EOL}${res.stderr}`,
      };
    }
    const storedAt = doc["stored_at"];
    return {
      ok: true,
      receiptPath: typeof storedAt === "string" ? storedAt : null,
      reason: null,
    };
  }
}
