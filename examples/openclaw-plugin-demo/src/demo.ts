// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
/**
 * The demo. One process, three scenarios, a text log, and an exit code that is 0 only if every
 * assertion held.
 *
 * What it drives is a real `gx` binary against a real filesystem. What it does NOT drive is
 * OpenClaw: the hook is called by `harness.ts`, which reproduces the firing order read out of their
 * wrapper, not their wrapper itself. Every claim below is therefore about the plugin, and the
 * README says so in the same words.
 */

import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { GxCliMembrane, runGx } from "./gx-cli-membrane.ts";
import type { GxMembrane, EscrowOutcome, ProposedEffect, UndoOutcome } from "./membrane.ts";
import { makeBeforeToolCallHandler, type PluginConfig } from "./plugin.ts";
import { runToolCall, type ToolCallEvent } from "./harness.ts";

// ---------------------------------------------------------------------------------------------
// Environment
//
// gx here is a Linux binary reached from a Windows Node process, so every invocation takes a WSL
// hop and the demo keeps two path spellings. A native deployment has one spelling and no hop; the
// translation lives in this file and in the harness, never in `plugin.ts`.
// ---------------------------------------------------------------------------------------------

const WSL_DISTRO = process.env["GX_DEMO_DISTRO"] ?? "Ubuntu-24.04";
// No default here on purpose: a fallback would have to be someone's absolute local path, and
// a demo that ships a machine-specific default is a demo that only ever ran on one machine.
// Build a binary (`cargo build -p gx-cli`) and point this at it -- see the README / run-demo.ps1.
const GX_LINUX_PATH = process.env["GX_DEMO_BIN"];
if (!GX_LINUX_PATH) {
  throw new Error(
    'GX_DEMO_BIN is not set. Point it at a Linux gx binary reachable from the configured WSL ' +
      "distro, e.g. the output of `cargo build -p gx-cli` (see run-demo.ps1's header comment).",
  );
}

/** The gx project bed and key store. Must be on a filesystem the fs adapter can snapshot: see README. */
const LINUX_ROOT = "/tmp/gx-openclaw-demo";
const LINUX_HOME = `${LINUX_ROOT}/home`;
const LINUX_PROJECT = `${LINUX_ROOT}/project`;

/** The agent's workspace, written by Node and read by gx, so it needs both spellings. */
const WIN_WORKSPACE = join(tmpdir(), "gx-openclaw-demo", "workspace");

function toWslPath(winPath: string): string {
  const m = /^([A-Za-z]):[\\/](.*)$/.exec(winPath);
  if (m === null) return winPath.replace(/\\/g, "/");
  return `/mnt/${m[1]!.toLowerCase()}/${m[2]!.replace(/\\/g, "/")}`;
}

function toWinPath(wslPath: string): string {
  const m = /^\/mnt\/([a-z])\/(.*)$/.exec(wslPath);
  if (m === null) return wslPath;
  return `${m[1]!.toUpperCase()}:\\${m[2]!.replace(/\//g, "\\")}`;
}

const gxEnv = {
  command: "wsl.exe",
  args: ["-d", WSL_DISTRO, "-e"] as const,
  binary: GX_LINUX_PATH,
  homeDir: LINUX_HOME,
};

function sha256(bytes: string): string {
  return createHash("sha256").update(bytes, "utf8").digest("hex");
}

// ---------------------------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------------------------

let failures = 0;
const out: string[] = [];

function say(line = ""): void {
  out.push(line);
  process.stdout.write(line + "\n");
}

function check(label: string, ok: boolean, detail: string): void {
  if (!ok) failures += 1;
  say(`  [${ok ? "OK  " : "FAIL"}] ${label}: ${detail}`);
}

// ---------------------------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------------------------

async function wslShell(script: string): Promise<string> {
  const { spawn } = await import("node:child_process");
  return new Promise((resolve) => {
    const c = spawn("wsl.exe", ["-d", WSL_DISTRO, "-e", "bash", "-c", script], {
      windowsHide: true,
    });
    let s = "";
    c.stdout.on("data", (d) => (s += String(d)));
    c.stderr.on("data", (d) => (s += String(d)));
    c.on("error", (e) => resolve(`spawn failed: ${String(e)}`));
    c.on("close", () => resolve(s));
  });
}

async function setup(): Promise<{ keyId: string; pubFileLinux: string }> {
  await wslShell(
    `rm -rf ${LINUX_ROOT} && mkdir -p ${LINUX_HOME} ${LINUX_PROJECT} && echo ready`,
  );
  rmSync(WIN_WORKSPACE, { recursive: true, force: true });
  mkdirSync(WIN_WORKSPACE, { recursive: true });

  const key = await runGx(gxEnv, ["key", "gen", "--json"]);
  const line = key.stdout.split(/\r?\n/).find((l) => l.trim().startsWith("{")) ?? "";
  const parsed = JSON.parse(line) as { key_id: string; public_key: string };

  // The public half is written where the offline verifier can read it, exactly as an outside
  // reviewer would have to receive it.
  const pubFileLinux = `${LINUX_ROOT}/actor.pub.json`;
  await wslShell(`cat > ${pubFileLinux} <<'EOF'\n${line.trim()}\nEOF\necho written`);

  say(`gx binary   : ${GX_LINUX_PATH}`);
  say(`gx project  : ${LINUX_PROJECT}  (ext4; see README on why not DrvFs)`);
  say(`workspace   : ${WIN_WORKSPACE}`);
  say(`actor key   : ${parsed.key_id}`);
  say();
  return { keyId: parsed.key_id, pubFileLinux };
}

// ---------------------------------------------------------------------------------------------
// The native tool body
//
// Stands in for OpenClaw's `write`. It does what that tool does -- put `content` at `path` -- and
// nothing else. The plugin never sees this function; the harness calls it only if the hook allowed
// the call through.
// ---------------------------------------------------------------------------------------------

/**
 * What the target held at the instant the native tool was about to write.
 *
 * Recorded because the obvious assertion -- "the file holds what the agent asked for" -- is true
 * whether gx applied the change or the native tool did, so on its own it discriminates nothing.
 * This is the discriminator: if gx's commit is what put the bytes there, the native tool finds the
 * work already done.
 */
let observedByNativeTool: string | null = null;

function nativeWrite(params: Record<string, unknown>): Promise<void> {
  const target = String(params["path"] ?? params["file_path"]);
  const content = String(params["content"]);
  const win = toWinPath(target);
  observedByNativeTool = readFileSync(win, "utf8");
  writeFileSync(win, content, "utf8");
  return Promise.resolve();
}

/** The tool body that must never run. Calling it is itself the failure. */
let forbiddenExecutions = 0;
function forbiddenWrite(_params: Record<string, unknown>): Promise<void> {
  forbiddenExecutions += 1;
  return Promise.resolve();
}

// ---------------------------------------------------------------------------------------------
// A membrane that cannot be reached, for scenario C
// ---------------------------------------------------------------------------------------------

class UnreachableMembrane implements GxMembrane {
  escrow(_e: ProposedEffect): Promise<EscrowOutcome> {
    return Promise.resolve({
      verdict: "Unknown",
      transformationId: null,
      reason: "no gx deployment answered on the configured surface",
      policyId: null,
      commitReceiptPath: null,
      applied: false,
    });
  }
  undo(_t: string): Promise<UndoOutcome> {
    return Promise.resolve({ ok: false, receiptPath: null, reason: "unreachable" });
  }
}

// ---------------------------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------------------------

const ORIGINAL = "notes for the release\n- ship the thing\n";
const DESIRED = "notes for the release\n- ship the thing\n- AGENT APPENDED THIS LINE\n";

async function scenarioA(cfg: PluginConfig, pubFileLinux: string): Promise<void> {
  say("SCENARIO A -- an ordinary write: escrowed, allowed, applied, then actually taken back");
  say();

  const winTarget = join(WIN_WORKSPACE, "notes.txt");
  const wslTarget = toWslPath(winTarget);
  writeFileSync(winTarget, ORIGINAL, "utf8");
  const shaBefore = sha256(ORIGINAL);
  say(`  file before  : ${shaBefore.slice(0, 16)}...`);

  const event: ToolCallEvent = {
    toolName: "write",
    toolCallId: "call-a-1",
    params: { path: wslTarget, content: DESIRED },
  };

  const handler = makeBeforeToolCallHandler(cfg);
  const result = await runToolCall(handler, event, nativeWrite);

  check("hook allowed the call", !result.blocked, `blocked=${result.blocked}`);
  check("native tool ran", result.executed, `executed=${result.executed}`);

  const escrowedRow = cfg.escrowed.at(-1);
  check(
    "an inverse was escrowed before the effect",
    escrowedRow !== undefined,
    escrowedRow?.transformationId ?? "none recorded",
  );
  if (escrowedRow === undefined) return;

  const afterWrite = readFileSync(winTarget, "utf8");
  check(
    "the file now holds what the agent asked for",
    sha256(afterWrite) === sha256(DESIRED),
    `${sha256(afterWrite).slice(0, 16)}...`,
  );

  // The discriminating one. The previous check would pass either way; this one fails unless the
  // membrane is what actually applied the change, before the tool was allowed to run.
  check(
    "gx had already applied it -- the native tool found the change done",
    observedByNativeTool !== null && sha256(observedByNativeTool) === sha256(DESIRED),
    observedByNativeTool === null
      ? "the native tool never ran"
      : `native tool saw ${sha256(observedByNativeTool).slice(0, 16)}... (desired=${sha256(DESIRED).slice(0, 16)}...)`,
  );

  // A checkpoint taken while the commit is the newest thing in the log, so the receipt can be
  // proved included against it later without the engine present.
  const ck1 = `${LINUX_ROOT}/head1.json`;
  const ckRes1 = await runGx(gxEnv, [
    "--project",
    LINUX_PROJECT,
    "log",
    "checkpoint",
    "--key",
    `${LINUX_HOME}/.gx/keys/${KEY_ID}.key`,
    "--out",
    ck1,
  ]);
  check("checkpoint taken after commit", ckRes1.code === 0, `exit ${ckRes1.code}`);

  say();
  say("  -- now take it back --");
  const undone = await cfg.membrane.undo(escrowedRow.transformationId);
  check("gx undo committed", undone.ok, undone.receiptPath ?? undone.reason ?? "?");

  const afterUndo = readFileSync(winTarget, "utf8");
  check(
    "the file is byte-for-byte what it was before the agent touched it",
    sha256(afterUndo) === shaBefore,
    `${sha256(afterUndo).slice(0, 16)}...`,
  );

  const ck2 = `${LINUX_ROOT}/head2.json`;
  const ckRes2 = await runGx(gxEnv, [
    "--project",
    LINUX_PROJECT,
    "log",
    "checkpoint",
    "--key",
    `${LINUX_HOME}/.gx/keys/${KEY_ID}.key`,
    "--out",
    ck2,
  ]);
  check("checkpoint taken after undo", ckRes2.code === 0, `exit ${ckRes2.code}`);

  say();
  say("  -- verify both receipts with no engine, no network, no project --");

  for (const [label, receipt, checkpoint] of [
    ["commit receipt", COMMIT_RECEIPT, ck1],
    ["undo receipt", undone.receiptPath, ck2],
  ] as const) {
    if (receipt === null) {
      check(`${label} verified offline`, false, "no receipt path was returned");
      continue;
    }
    const res = await runGx(gxEnv, [
      "receipt",
      "verify",
      receipt,
      "--offline",
      "--checkpoint",
      checkpoint,
      "--checkpoint-key",
      pubFileLinux,
      "--key",
      pubFileLinux,
    ]);
    const doc = res.stdout.split(/\r?\n/).find((l) => l.trim().startsWith("{")) ?? "{}";
    let valid = false;
    let checks = "";
    try {
      const j = JSON.parse(doc) as { valid?: boolean; checks?: Record<string, unknown> };
      valid = j.valid === true;
      checks = JSON.stringify(j.checks ?? {});
    } catch {
      /* leave valid false */
    }
    check(`${label} verified offline`, valid && res.code === 0, checks || res.stderr.trim());
  }
  say();
}

async function scenarioB(cfg: PluginConfig): Promise<void> {
  say("SCENARIO B -- a write the shipped policy refuses: blocked before anything is touched");
  say();

  const shaBefore = (await wslShell("sha256sum /etc/hostname")).trim().split(/\s+/)[0] ?? "?";
  say(`  /etc/hostname before : ${shaBefore.slice(0, 16)}...`);

  const event: ToolCallEvent = {
    toolName: "write",
    toolCallId: "call-b-1",
    params: { path: "/etc/hostname", content: "owned-by-the-agent\n" },
  };

  const before = forbiddenExecutions;
  const result = await runToolCall(makeBeforeToolCallHandler(cfg), event, forbiddenWrite);

  check("hook blocked the call", result.blocked, `blocked=${result.blocked}`);
  check("the tool body never ran", forbiddenExecutions === before, `executions=${forbiddenExecutions - before}`);
  check(
    "the block names the shipped policy that decided",
    (result.reason ?? "").includes("fs-deny-etc"),
    result.reason ?? "no reason",
  );

  const shaAfter = (await wslShell("sha256sum /etc/hostname")).trim().split(/\s+/)[0] ?? "?";
  check("/etc/hostname is untouched", shaAfter === shaBefore, `${shaAfter.slice(0, 16)}...`);
  say();
}

async function scenarioC(): Promise<void> {
  say("SCENARIO C -- the membrane cannot be reached: closed, and reported as Unknown, not as Deny");
  say();

  const winTarget = join(WIN_WORKSPACE, "unreachable.txt");
  writeFileSync(winTarget, ORIGINAL, "utf8");

  const cfg: PluginConfig = {
    membrane: new UnreachableMembrane(),
    tools: ["write"],
    actorModel: "openclaw-demo",
    escrowed: [],
  };

  const event: ToolCallEvent = {
    toolName: "write",
    toolCallId: "call-c-1",
    params: { path: toWslPath(winTarget), content: DESIRED },
  };

  const before = forbiddenExecutions;
  const result = await runToolCall(makeBeforeToolCallHandler(cfg), event, forbiddenWrite);

  check("hook blocked the call", result.blocked, `blocked=${result.blocked}`);
  check("the tool body never ran", forbiddenExecutions === before, `executions=${forbiddenExecutions - before}`);
  check(
    "the reason says unknown, and does not claim a denial",
    (result.reason ?? "").includes("could not be consulted") &&
      !/\bDeny\b/.test(result.reason ?? ""),
    result.reason ?? "no reason",
  );
  check(
    "the file was left alone",
    sha256(readFileSync(winTarget, "utf8")) === sha256(ORIGINAL),
    "unchanged",
  );
  say();
}

async function scenarioD(cfg: PluginConfig): Promise<void> {
  say("SCENARIO D -- a tool this plugin does not own is not touched by it");
  say();
  const event: ToolCallEvent = {
    toolName: "read",
    toolCallId: "call-d-1",
    params: { path: "/etc/hostname" },
  };
  let ran = false;
  const result = await runToolCall(makeBeforeToolCallHandler(cfg), event, () => {
    ran = true;
    return Promise.resolve();
  });
  check("the call went through untouched", result.executed && !result.blocked, `executed=${result.executed}`);
  check("the tool body ran", ran, `ran=${ran}`);
  say();
}

// ---------------------------------------------------------------------------------------------

let KEY_ID = "";
let COMMIT_RECEIPT: string | null = null;

async function main(): Promise<void> {
  say("=== gx x OpenClaw -- before_tool_call escrow demo ===");
  say(`run at ${new Date().toISOString()}`);
  say();

  const { keyId, pubFileLinux } = await setup();
  KEY_ID = keyId;

  const membrane = new GxCliMembrane({
    command: gxEnv.command,
    args: [...gxEnv.args],
    binary: gxEnv.binary,
    projectDir: LINUX_PROJECT,
    homeDir: LINUX_HOME,
    actorKeyId: keyId,
    log: (l) => say(l),
  });

  // Wrap so the demo can keep the commit receipt path, which the plugin has no reason to expose.
  const recording: GxMembrane = {
    escrow: async (e) => {
      const o = await membrane.escrow(e);
      if (o.commitReceiptPath !== null) COMMIT_RECEIPT = o.commitReceiptPath;
      return o;
    },
    undo: (t) => membrane.undo(t),
  };

  const cfg: PluginConfig = {
    membrane: recording,
    tools: ["write"],
    actorModel: "openclaw-demo",
    escrowed: [],
    log: (l) => say(l),
  };

  await scenarioA(cfg, pubFileLinux);
  await scenarioB(cfg);
  await scenarioC();
  await scenarioD(cfg);

  say("=== result ===");
  say(`failures = ${failures}`);
  say(`DEMO = ${failures === 0 ? "PASS" : "FAIL"}`);
  say();
  say("Not tested here: OpenClaw itself never ran. See README, 'What this demo does not show'.");

  process.exitCode = failures === 0 ? 0 : 1;
}

main().catch((e: unknown) => {
  say(`fatal: ${String(e)}`);
  process.exitCode = 2;
});
