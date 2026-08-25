// A minimal, dependency-free wrapper around the real `gx` binary for this SDK's own E2E test and
// its quickstart/GUI-probe scripts (`req/132` §4 items 4/6, AC-P4-2, AC-P4-4). Spawns a real
// process; no mock server anywhere in this file.
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

/** The `gx` binary this test drives -- never guessed, always the caller's own build. */
export function gxBinary() {
  const bin = process.env.GX_BINARY;
  if (!bin) {
    throw new Error(
      "GX_BINARY is not set. Build crates/gx-cli (cargo build -p gx-cli, WSL only) and export " +
        "GX_BINARY=<CARGO_TARGET_DIR>/debug/gx before running this test (tools/verify_p4.sh does).",
    );
  }
  return bin;
}

/** A fresh project dir, home dir (for `~/.gx/keys/`), and target file -- one per test. */
export function scratchProject(prefix, before = "before\n") {
  const root = mkdtempSync(join(tmpdir(), `${prefix}-`));
  const project = join(root, "project");
  const home = join(root, "home");
  const target = join(project, "target.txt");
  mkdirSync(project, { recursive: true });
  mkdirSync(home, { recursive: true });
  writeFileSync(target, before);
  return { root, project, home, target };
}

/** Run `gx --project <project> <...args>` to completion and return `{code, stdout, stderr}`. */
export function runGx(project, home, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(gxBinary(), ["--project", project, ...args], {
      env: { ...process.env, HOME: home, USERPROFILE: home },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => (stdout += d));
    child.stderr.on("data", (d) => (stderr += d));
    child.on("error", reject);
    child.on("close", (code) => resolve({ code, stdout, stderr }));
  });
}

/** `gx key gen --json` -> `{key_id, public_key}` (44 §1.2). */
export async function keyGen(project, home) {
  const { code, stdout, stderr } = await runGx(project, home, ["key", "gen", "--json"]);
  if (code !== 0) throw new Error(`gx key gen failed (${code}): ${stderr}`);
  return JSON.parse(stdout.trim());
}

/**
 * `gx submit` once, so `.gx/` exists before `gx serve` opens it.
 *
 * `gx submit` is the one CLI verb that **creates** `.gx/` (`crates/gx-cli/src/main.rs`: "44 has no
 * `gx init`... `gx serve` **opens** a project rather than creating one" -- the same reading
 * `crates/gx-cli/tests/ac_056.rs::Serving::start`'s callers apply, one submit ahead of every
 * `serve`). `POST /candidates` over HTTP never creates `.gx/` itself, so this warm-up is required
 * even though the SDK's own E2E flow never calls `runGx("submit", ...)` again.
 */
export async function warmProject(project, home, target, keyId) {
  const goalFile = join(project, "warm-goal.txt");
  writeFileSync(goalFile, "warm\n");
  const { code, stderr } = await runGx(project, home, [
    "submit",
    "--substrate",
    "fs",
    "--locator",
    target,
    "--intent",
    goalFile,
    "--context",
    "Evidence",
    "--actor-key",
    keyId,
  ]);
  if (code !== 0) throw new Error(`gx submit (warm-up) failed (${code}): ${stderr}`);
}

/** A running `gx serve`, over `--bind 127.0.0.1:0` (the kernel picks the port, `ac_056.rs`'s own
 * choice for the identical race-avoidance reason). Returns `{child, baseUrl, token, stop}`. */
export function startServe(project, home, keyId, opts = {}) {
  return new Promise((resolve, reject) => {
    const token = `p4-sdk-e2e-token-${Math.random().toString(36).slice(2)}`;
    const tokenFile = join(project, "..", "token");
    writeFileSync(tokenFile, `${token}\n`);
    const args = [
      "serve",
      "--bind",
      "127.0.0.1:0",
      "--token-file",
      tokenFile,
      "--signing-key",
      keyId,
    ];
    if (opts.failPosture) args.push("--fail-posture", opts.failPosture);
    const child = spawn(gxBinary(), ["--project", project, ...args], {
      env: { ...process.env, HOME: home, USERPROFILE: home },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let buf = "";
    let settled = false;
    const timeout = setTimeout(() => {
      if (!settled) {
        settled = true;
        child.kill("SIGKILL");
        reject(new Error("gx serve did not print its start-up line within 20s"));
      }
    }, 20_000);
    child.stdout.on("data", (chunk) => {
      if (settled) return;
      buf += chunk.toString("utf8");
      const nl = buf.indexOf("\n");
      if (nl === -1) return;
      const line = buf.slice(0, nl).trim();
      settled = true;
      clearTimeout(timeout);
      let start;
      try {
        start = JSON.parse(line);
      } catch (e) {
        reject(new Error(`gx serve's start-up line is not JSON (44 §1.2): ${line} (${e})`));
        return;
      }
      if (start.event !== "gx.serve.started") {
        reject(new Error(`unexpected start-up event: ${JSON.stringify(start)}`));
        return;
      }
      resolve({
        child,
        baseUrl: `http://${start.bind}`,
        token,
        stop: () => stopServe(child),
      });
    });
    child.stderr.on("data", () => {}); // drained, not inspected -- 44 §1.2's diagnostics go here
    child.on("error", (e) => {
      if (!settled) {
        settled = true;
        clearTimeout(timeout);
        reject(e);
      }
    });
    child.on("close", (code) => {
      if (!settled) {
        settled = true;
        clearTimeout(timeout);
        reject(new Error(`gx serve exited (${code}) before printing its start-up line`));
      }
    });
  });
}

function stopServe(child) {
  return new Promise((resolve) => {
    if (child.exitCode !== null) {
      resolve();
      return;
    }
    child.once("close", () => resolve());
    child.kill("SIGTERM");
  });
}
