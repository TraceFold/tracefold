<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx x OpenClaw — a `before_tool_call` escrow plugin

An OpenClaw plugin that puts an agent's filesystem writes through the gx membrane, so that a write
can be **refused before it lands** and **taken back after it does**.

It is about 200 lines of plugin. The interesting part is not the size — it is that the "taken back"
is a real inverse escrowed before the effect, not a diff computed afterwards, and that a third party
can check it happened without the engine, the network, or the project.

Run it:

```
pwsh -File run-demo.ps1        # or: node src/demo.ts
```

Last recorded run: [`req/1034_artifacts/demo_run.log`](../../req/1034_artifacts/demo_run.log) — 21
assertions, `DEMO = PASS`, exit 0.

---

## Why `before_tool_call` and not somewhere else

There is exactly one point in OpenClaw's tool path where the precondition still exists. Three facts,
each read in their source (`req/1031`):

| Fact | Where |
| :--- | :--- |
| The hook result is evaluated first, and when it blocks, the real `execute()` is never called — the hook **conditions** the effect | `src/agents/agent-tools.before-tool-call.wrapper.ts:445-549` |
| `runBeforeToolCall` is sequential, may block, may rewrite params | `src/plugins/hooks.ts:1438-1509` |
| `runAfterToolCall` beside it is documented *"fire-and-forget"* — after the fact there is nothing left to hold | same file, `:1511-1520` |

An independent contributor reached the same point from the other direction: PR #125531 uses
`before_tool_call (sync): capture the step checkpoint`. There is nowhere else to stand.

## What the demo actually shows

Four scenarios, one process, against a real `gx` binary and a real filesystem.

**A — an ordinary write.** The agent asks to append a line to a file. The hook fires; gx
`submit → plan → verify → commit` runs; the shipped `fs-permit-default` policy admits it; an inverse
is escrowed and the change lands with a signed receipt. The tool is then allowed through. Afterwards
`gx undo` puts the file back **byte for byte**, and both receipts verify offline.

The load-bearing assertion is not "the file has the new content" — that would be true no matter who
wrote it. It is this one:

```
[OK] gx had already applied it -- the native tool found the change done
```

The native tool records what it saw at the instant before writing. If gx's commit is what put the
bytes there, the tool finds the work already done. That is what distinguishes a membrane from a
logger.

**B — a write the shipped policy refuses.** The agent asks to write `/etc/hostname`. `gx verify`
answers `Deny` by `fs-deny-etc` — a policy that **ships in this repo**, not one written for the demo.
The hook returns `block: true`; the tool body is never entered; the file's digest is unchanged.

**C — the membrane cannot be reached.** The verdict is `Unknown`, and the plugin says `Unknown`. It
still closes — a change nobody escrowed is a change nobody can take back — but it does not report a
denial that never happened. `Unknown` folded into `Deny` would be this demo's tooling breaking the
first principle the product is built on.

**D — a tool this plugin does not own.** A `read` call passes through untouched.

## What the native tool does after we return

gx has no escrow-without-apply verb: `commit` escrows the inverse, re-checks the precondition, and
applies, atomically. So by the time the hook returns `Admit`, **the effect is already in the
substrate**, and OpenClaw's own `write` then re-applies byte-identical content.

This is a real property of the design and is stated here rather than hidden: the native write is a
convergent re-application, not a wrapped one. It matters in two ways, one good and one a limit:

- The undo still works, and the demo proves it works *after* the native tool has written — scenario
  A's undo runs downstream of `nativeWrite`.
- A tool whose effect is not a pure function of its parameters (an append, a random name, a network
  call) would not converge this way. This plugin covers `write`, whose `content` is the full end
  state, which is exactly why `write` was chosen and `edit` / `apply_patch` / `bash` were not.

## The seam

`membrane.ts` is the only file that names gx concepts; `gx-cli-membrane.ts` is the only file bound to
a way of reaching a deployment. The plugin speaks about proposed effects and verdicts.

That boundary is a **proposal, not a proven one**. The rule is that the second implementation decides
whether a seam was drawn in the right place, and only one is written here. A `GxHttpMembrane` over
the shipped typed client (`sdk/typescript`, `GxClient.createCandidate / verifyCandidate /
commitCandidate / undoTransformation`) would be the test. It is not written.

## What this demo does **not** show

Stated plainly because a demo's limits rot faster than its claims.

1. **This harness never ran inside OpenClaw.** `harness.ts` reproduces the firing order read out of
   their wrapper; it is not their wrapper. Every scenario A-D result above is a statement about *the
   plugin's decision logic*, conditional on OpenClaw firing hooks in the order its source was
   observed to fire them. That conditional itself is no longer untested at the install layer: a real
   `openclaw` gateway (`OpenClaw 2026.8.1`) has loaded `openclaw-install/` and called `register(api)`
   at boot (`req/1036`, `openclaw plugins doctor` = `ok: true`, live log line `[gx-escrow]
   register(api) called -- before_tool_call handler registered` inside `[gateway] http server
   listening (... gx-escrow ...)`). What is **still** untested is a real tool call actually reaching
   `before_tool_call` end to end — that needs an agent turn, which needs a model, which this project's
   constraints do not let this lane spend. **UNTESTED: real tool-call firing. TESTED: real plugin
   load and hook registration.**
2. **The `matcher` shape was guessed, then corrected against the real SDK.** The first version passed
   `{ tools: [...] }` to `register()`'s third argument; a real `openclaw plugins install` run failed
   that registration with `TypeError: tool hook matcher must be an array of tool names` (req/1036).
   `register()` now passes the bare array. The handler's own tool-name guard held the scope either
   way, which is why the wrong shape never mis-scoped anything — it simply never installed.
3. **No genuine third-party plugin is known to use `before_tool_call`.** `registerTypedHook` applies
   no origin gate to it (`req/1031` §2), and `extensions/onepassword` uses the same public API — but
   that extension is `origin: "bundled"`. ClawHub was never searched.
4. **Only the `fs` substrate, only the `write` tool.** gx ships `git`, `mcp` and `postgres` adapters
   and none of them is wired here.
5. **Escalation is not carried to a human.** The plugin returns `requireApproval`; the harness stops
   there rather than pretending an approval arrived. No `Escalate` verdict was produced by a real gx
   run in this demo, so that branch is **untested, not passing**.
6. **Multi-plugin composition is untested.** How `mergeResults` combines this hook's result with
   another plugin's was not exercised.
7. **Concurrency is untested.** Two tool calls against the same locator in flight at once were not
   tried.

## Environment notes (measured, not assumed)

- The gx **project bed and `HOME` must be on a Linux filesystem.** With the bed on `/mnt/c` (DrvFs)
  the walk fails at `verify`. The **target file may live on DrvFs** — that combination was measured
  working, and is what the demo uses.
- `gx submit --actor-kind agent` requires `--actor-model`. The plugin passes the model name, which is
  the one fact about an agent a reviewer cannot recover from the key.
- The intent travels on **stdin** (`--intent -`), so the plugin never has to place a temp file in a
  filesystem the engine may not be able to snapshot.
- Sources run directly on Node's type stripping (Node >= 23.6): no build step, no `node_modules`.
  `npm run typecheck` is the optional stricter pass and is the only script needing dependencies.

## Files

| File | What it is |
| :--- | :--- |
| `src/membrane.ts` | The seam: proposed effect in, three-valued verdict out |
| `src/gx-cli-membrane.ts` | The seam bound to the `gx` CLI surface |
| `src/plugin.ts` | The `before_tool_call` handler and its registration |
| `src/harness.ts` | A stand-in for OpenClaw's tool wrapper — **not** OpenClaw |
| `src/demo.ts` | The four scenarios and their assertions |
| `src/openclaw-entry.ts` | A `definePluginEntry` wrapper around `register()`, loadable by a real `openclaw` process via `plugins.load.paths` on a single file (no manifest, so it is discoverable but not auto-activated at boot — see `openclaw-install/`) |
| `openclaw-install/` | The manifest form (`openclaw.plugin.json` with `activation.onStartup: true`, `package.json`, `src/index.ts`) that a real `openclaw plugins install <dir>` accepts and that actually loads at gateway boot. Confirmed working against `OpenClaw 2026.8.1` in `req/1036`. |

No line of OpenClaw source is reproduced in any of them.
