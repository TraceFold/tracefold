<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# examples/

Copy-paste starting points. Nothing here is wired into this repository's own build — these are
files you take away and adapt.

---

| File | What it does | What it does not do |
| :--- | :--- | :--- |
| [`ci/receipt-check.yml`](ci/receipt-check.yml) | A GitHub Actions job that fails the build when a delivery carries no `gx` receipt, or carries one that does not verify offline. It runs `gx receipt verify --offline` over a receipt, a checkpoint and a trusted-key bundle. | It is not run by this repository (this repo verifies its own receipts through its test suite instead), and it assumes you supply the key bundle. The file's own header states what it assumes in full. |
| [`demo_one_screen.sh`](demo_one_screen.sh) | A one-screen, 8-stage shell reproduction of commit → receipt → tamper → verify → undo → redo against a real cloned git repository (not a fixture). Run it with `bash examples/demo_one_screen.sh`. | It does not exercise the filesystem substrate (git only), and it does not repair the known repeated-undo edge case it discloses in its own header — it just doesn't hit that case, since it only undoes each intent once. |
| [`openclaw-plugin-demo/`](openclaw-plugin-demo/README.md) | An OpenClaw `before_tool_call` plugin (~200 lines) that routes an agent's filesystem writes through the gx membrane: a write can be refused before it lands and taken back after it does, with a third party able to check it happened without the engine, network, or project. Run `pwsh -File run-demo.ps1` or `node src/demo.ts`. | Not wired into this repository's own build or CI; it is a copy-paste starting point, same as the other rows. |
| [`receipt-memory-export/`](receipt-memory-export/README.md) | Derives generic agent-memory nodes (`memory/nodes/*`, `memory/index.*`) from `gx` receipts, using CLI output only (`gx plan`/`gx commit`/`gx undo`/`gx receipt show`) — never reads `.gx/` state or imports a `gx-*` crate. Run `run_demo.sh` to drive a real `gx` project and verify the export against it. | Verified only against the `fs` substrate; does not read `.gx/` state by design, so it cannot list receipts the way `gx replay`/`gx draft list`/`gx verdict-checkpoint list` each partially can (see its own README for the exact gap). |
| `README.md` | This page. | — |

---

## Using it

Copy `ci/receipt-check.yml` into your own `.github/workflows/`, point it at the receipt and key
your delivery produces, and it will refuse a build whose evidence does not check out.
[`docs/TUTORIAL.md`](../docs/TUTORIAL.md) walks the same verification by hand first, which is the
faster way to find out what the three input files are before automating them.

Run `demo_one_screen.sh` directly (it needs a built `gx` on `PATH`, or `GX=./target/debug/gx`
set beforehand) to watch the same lifecycle end to end against a real npm-package repo.
`openclaw-plugin-demo/` and `receipt-memory-export/` each carry their own README with their own
run instructions; read those before running either.

## Scope of this folder

This folder is published whole: every file added here later is published too, with no allow-list
filtering it. That is why it holds no staging step to catch a stray file — but it is a folder of
independent starting points, not a fixed count: it held three top-level entries when that sentence
was first written, held four once `openclaw-plugin-demo/` landed without this page being updated
to match (caught 2026-09-01, GitDoc inventory pass), and holds five now that
`receipt-memory-export/` is listed above. Read the table above for the current count, not this
paragraph's history.
