<!-- SPDX-License-Identifier: Apache-2.0 -->

# TraceFold GUI

A desktop-shaped window over a TraceFold engine. It draws six faces — atlas, graph, held,
ledger, notice, receipt — onto a shell that is built from a declaration rather than from a
list of screen names, so that changing the layout is the same kind of operation as the thing
the product is about: a transform that carries its own inverse.

It is a **client**. It computes nothing about your data. Every claim it puts on screen was
either read from an engine over HTTP, or is marked as not-read.

## What you need

- **Node.js 20+** (measured on 24.15.0). There are **no npm dependencies** — not one. The
  tree runs on node builtins and plain DOM. There is no bundler, no framework, no install step.
- **A local `gx` binary** for real data — the engine from
  [TraceFold/tracefold](https://github.com/TraceFold/tracefold). See "Without an engine" below
  for what happens when you don't have one.
- **Chrome or Chromium** only if you want to run the two instruments that drive a real window.

## Running it

```sh
# the window, with no engine behind it
node shell/tools/serve.mjs --port 8807

# the window, against a running engine ("bed")
node shell/tools/serve.mjs --port 8807 --bed http://127.0.0.1:8795 --bed-token <hex>
```

Then open `http://127.0.0.1:8807/`.

## Without an engine

This is stated plainly because a window that draws an empty table looks the same as a window
that drew a table of nothing:

- `/v1/*` answers **503**, and the server says so on startup.
- Every face reports that it **was not read** — it does not render a blank panel and let you
  assume the data is empty.
- `node tools/verify-all.mjs` does not print green. With no engine it carries the reason
  `a run that never reached a real server is not green`; in this published checkout it stops
  earlier still, at `SOURCES-ABSENT` (see "A note on this checkout"). Either way the refusal is
  the intended answer, not a failure of setup.

## Checking it

```sh
node --test                                            # 1085 tests: 1079 pass, 6 skip, 0 fail
node shell/tools/btn_verify.mjs  --origin http://127.0.0.1:8807
node shell/tools/bound_smoke.mjs --origin http://127.0.0.1:8807 --expect unbound
```

Both instruments carry a negative control and will tell you when they are the thing that is
broken. `btn_verify --plant` puts a dead button into the window and **requires itself to go red
for it**; `bound_smoke --expect unbound` asserts that a window with no engine behind it
genuinely reports no engine, so that a passing "bound" run means something.

The counts above are what this published tree actually answers, measured in a clean checkout
before publication. The 6 skips are readings that name what they could not reach rather than
passing over an empty set, and each prints its reason:

- **4** need the private corpus described below.
- **2** compare this tree's route table against the Rust crate that serves those routes. In a
  clone of this repository they do **not** skip — the crate is a sibling directory
  (`../crates/gx-api`), the comparison runs, and it holds: every route the router registers is
  declared here and no others. They skip only in a copy of `gui/` taken out of the repository,
  where the crate is gone; point `GX_CRATE_LIB` at one to restore the check. The 6 above is the
  count for a standalone copy.

`tools/verify-all.mjs` is the only command allowed to state a verdict about the whole tree. Its
exit codes are a state machine, not a boolean: `0` green, `1` red, `2` non-canonical, `3`
partial, `4` flaky, `5` self-blind (the harness failed its own rounds, so every other number in
that run is void), `6` sources-absent (the acceptance-criteria corpus is not in this tree, so
coverage has no denominator — distinct from red, which means measured and broken).

## What this does not do

Kept here rather than in an issue tracker, because the gap between what a UI draws and what it
has actually verified is the failure this project exists to make visible.

- **The wire tier is not built.** `verify-all` prints `wire=no`; no run can be green through it.
- **Acceptance criteria are mostly unbacked**, and this checkout cannot tell you by how much:
  the criteria live in the private corpus, so `verify-all` here reports coverage as
  `UNMEASURED` rather than quoting a ratio. A ratio measured upstream is not a fact about the
  tree you are holding, so it is not printed here.
- **The breach runner is not built.** Rows in `tools/breaches.json` are fired by hand, so a row's
  result can go stale with nothing saying so.
- **Determinism quarantine, the evidence floor, and the exemption ledger are unwritten.**
- **This is not a packaged desktop application.** There is no installer and no signed binary; it
  is a local server plus a browser window.
- **A drawn row cannot yet name the engine build that produced it.** The window records which
  calls it made, but nothing on screen binds a row to the engine version that answered it. Since
  the product's whole claim is that a receipt proves what happened, and a screen can draw
  anything, this is a real gap and it is named here rather than left to be discovered.

## A note on this checkout

Parts of this tree were written against a private planning corpus (`req/`) that is not
published. Comments throughout the source cite it as `req/NNN §N`; those citations are
provenance, not links you can follow, and nothing in the running product reads them.

Two instruments read that corpus, and in this checkout they **refuse rather than pretend**:

- `tools/sem_registry_gate.mjs` exits **2 `UNMEASURED`** and names all four criteria it did not
  decide. It does not exit 0.
- `tools/verify-all.mjs` exits **6 `SOURCES-ABSENT`**, prints `coverage: UNMEASURED` with the
  absent sources listed, and sets `citableAsEvidence: false` in `.run/report.json`.

Both previously did something worse — one crashed with an unhandled `ENOENT`, the other printed
`AC backed 0 / 0`, which reads as *no gap* when it means *no data*, and could still let a run
finish green. Treat both as unavailable here; that is now what they say about themselves.

## Related

- **[The browser verifier](https://tracefold.github.io/tracefold/verify.html)** — checks a
  receipt in your own browser, with no install and no network request, and answers with the
  same three-valued verdict this window draws. It is the shortest way to see what the engine
  asserts without running either the engine or this GUI.

## License

Apache-2.0, under the licence at the root of this repository. Every source file in this tree
carries its own `SPDX-License-Identifier: Apache-2.0` header.
