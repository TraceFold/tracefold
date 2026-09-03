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
pwsh -File run-demo.ps1                  # or: node src/demo.ts   (needs a real gx binary)
node src/unmediated-writes.test.ts       # negative controls; needs nothing at all
```

Last recorded run: [`req/1034_artifacts/demo_run.log`](../../req/1034_artifacts/demo_run.log) — 21
assertions, `DEMO = PASS`, exit 0. Two of the claims that run was making were wrong, and were
reported from outside by **ANP2 Network**; see *What the native tool does after we return* below.

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
is escrowed and the change lands with a signed receipt. The native tool is then stopped, because the
change is already made. Afterwards `gx undo` puts the file back **byte for byte**, and both receipts
verify offline.

The load-bearing assertion is not "the file has the new content" — that would be true no matter who
wrote it. It is this one:

```
[OK] gx is the only thing that moved a byte: native writes=0, tool body executed=false
```

The file holds the new content and no tool body ran, so the membrane is the only thing that can have
put it there. That is what distinguishes a membrane from a logger.

**B — a write the shipped policy refuses.** The agent asks to write `/etc/hostname`. `gx verify`
answers `Deny` by `fs-deny-etc` — a policy that **ships in this repo**, not one written for the demo.
The hook returns `block: true`; the tool body is never entered; the file's digest is unchanged.

**C — the membrane cannot be reached.** The verdict is `Unknown`, and the plugin says `Unknown`. It
still closes — a change nobody escrowed is a change nobody can take back — but it does not report a
denial that never happened. `Unknown` folded into `Deny` would be this demo's tooling breaking the
first principle the product is built on.

**D — a tool this plugin does not own.** A `read` call passes through untouched.

### The negative controls

```
node src/unmediated-writes.test.ts
```

Separate from the four scenarios, and needing **no gx, no WSL, no network and no filesystem**:
`src/unmediated-writes.test.ts` guards one invariant — *no byte reaches the substrate except through
the membrane* — against the two defects **ANP2 Network** reported. Each case was watched to fail
against the unfixed plugin before the fix went in, and to fail again when the fix was reverted:
**7 failures before, 0 after, 7 again on revert.**

It carries its own control: with nothing intervening, the old pass-through really was a no-op, which
is why the wrong comment looked right for as long as it did. The failure appears only once something
lands in the window.

## What the native tool does after we return — nothing, and why that changed

gx has no escrow-without-apply verb: `commit` escrows the inverse, re-checks the precondition, and
applies, atomically. So by the time the hook has an `Admit` in hand, **the effect is already in the
substrate**. On an admitted write the hook therefore returns `block: true` and OpenClaw's own
`write` never runs. The block reason tells the agent the write landed and names the transformation
and receipt; OpenClaw hands that text to the model verbatim as the tool result.

**This used to pass the call through**, and the pass-through was defended here as a convergent
re-application — the native write puts back the same bytes, so nothing moves. **ANP2 Network**, a
reader of the dev.to post, read the source and pointed out the sharper fact: the re-application is
*unconditional*. Anything that lands between gx's commit and that write — a later hook in the same
chain, a background process, a file watcher, the agent's own unmediated `bash` — is overwritten by
bytes no receipt covers and no inverse can restore. **A single caller reaches this; no concurrency
is required.** The receipt proves the gx transformation was valid. It never proved that what is on
disk afterwards descends from the post-image it signed.

Their suggested repairs were (a) rewrite the params so the native call becomes a verified no-op, or
(b) at minimum re-hash after it. Neither is what shipped, for reasons read out of OpenClaw's tree
rather than guessed:

- **(a) is not expressible.** `writeSchema` is `{ path, content }` and nothing else
  (`src/agents/sessions/tools/write.ts`), so no returned parameter can make the native write
  conditional on the file still holding the digest gx signed for. Worse, returned params are not
  final: `runBeforeToolCall` merges them as `lastDefined(acc?.params, next.params)`, so a later
  plugin in the chain overwrites ours.
- **(b) detects; it does not prevent.** The hook next door, `after_tool_call`, is documented
  fire-and-forget — by then the bytes have moved and there is nothing left to hold.
- **What shipped is the stronger form of (a):** stop the call. `block` is the only terminal
  instrument in the result vocabulary — `stickyTrue(acc?.block, next.block)` with
  `shouldStop: block === true`, and the wrapper turns a plugin block into `kind: "veto"` and returns
  a blocked result **without calling `execute()`**. Zero unmediated byte movement, guaranteed by
  their control flow rather than by ours.

Two costs, stated rather than hidden:

- **OpenClaw records an admitted write as blocked.** It emits a blocked diagnostic and a blocked
  security event for a write that in fact succeeded. The envelope says blocked; the reason text
  inside it says the change landed and names the transformation. A deployment reading that telemetry
  needs to know this, which is why it is here.
- **`edit` and `apply_patch` are covered by refusal, not by escrow.** They describe a delta, and
  this plugin does not reimplement their semantics to recover an end state from it. Only `write`
  carries the full end state in its parameters, which is why only `write` is escrowed. Covered here
  means **stopped**, which is worse for the agent and better for the invariant.

The undo still works and the demo still proves it, now against a file the native tool never touched.

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
4. **Only the `fs` substrate; `bash` is not covered and cannot be by this plugin.** gx ships `git`,
   `mcp` and `postgres` adapters and none of them is wired here. Of OpenClaw's own tools, the plugin
   mediates `write`, `edit` and `apply_patch` — every built-in tool that puts bytes at a path the
   caller names — but **`bash` is not among them and cannot be**: an arbitrary command has no
   locator to escrow, so nothing about it can go through the membrane. An agent that has `bash` has
   an unmediated path to the filesystem no matter what the other three do. Closing that needs the
   tool taken away or the process confined (`gx confine`, whose own limits are in `docs/LIMITS.md`),
   not a change here. The gap is declared in [`docs/LIMITS.md`](../../docs/LIMITS.md) and
   deliberately never named in a runtime message the gated model reads.
5. **The drift case is proved by the unit test, not by the gx-backed demo.** The window between gx's
   commit and the native tool is exercised in `unmediated-writes.test.ts` against a fake membrane.
   No scenario in `demo.ts` injects a third-party write into that window against the real engine —
   the plugin's decision is identical either way, but that is an argument, not a measurement.
6. **Escalation is not carried to a human.** The plugin returns `requireApproval`; the harness stops
   there rather than pretending an approval arrived. No `Escalate` verdict was produced by a real gx
   run in this demo, so that branch is **untested, not passing**.
7. **Multi-plugin composition is untested.** How `mergeResults` combines this hook's result with
   another plugin's was not exercised. Its behaviour was **read** while fixing the first defect —
   later plugins overwrite returned `params`, and `block` is sticky and stops the chain — but read is
   not run.
8. **Concurrency is still untested, and that is no longer the interesting gap.** Two tool calls
   against the same locator in flight at once were not tried. ANP2 Network's report was that the
   drift defect never needed them: one caller and one intervening write is enough, and that is what
   `unmediated-writes.test.ts` exercises. Treating this as a concurrency question is what let the
   defect stand.
9. **An admitted write is recorded by OpenClaw as blocked.** Because the plugin stops the redundant
   native call, OpenClaw emits a blocked diagnostic and a blocked security event for a write that
   succeeded. Nothing reports the write as failed to the model — the reason text says it landed and
   names the transformation — but telemetry built on those events will miscount.

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
| `src/unmediated-writes.test.ts` | The negative controls for the two defects ANP2 Network reported — no engine needed |
| `src/openclaw-entry.ts` | A `definePluginEntry` wrapper around `register()`, loadable by a real `openclaw` process via `plugins.load.paths` on a single file (no manifest, so it is discoverable but not auto-activated at boot — see `openclaw-install/`) |
| `openclaw-install/` | The manifest form (`openclaw.plugin.json` with `activation.onStartup: true`, `package.json`, `src/index.ts`) that a real `openclaw plugins install <dir>` accepts and that actually loads at gateway boot. Confirmed working against `OpenClaw 2026.8.1` in `req/1036`. |

No line of OpenClaw source is reproduced in any of them.
