<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# .devcontainer/

One file: `devcontainer.json` — a Codespaces / VS Code Remote container definition so a
contributor can open the repo and land on an already-built workspace instead of hitting
the cargo-build activation wall cold (req/818 ATOM 1).

The image ships `rustup`; `rust-toolchain.toml` (channel pinned there) is the single
source of the toolchain version — nothing here re-declares it.

## Not yet live

As of the file's own header note (req/818 F1, 2026-08-25), this file is staged for the
public sync set but has **not** been pushed to `TraceFold/tracefold`. The live public
tree currently declares workspace members it does not carry (`probes/doubt`,
`gx-adapter-postgres`, `gx-adapter-mysql`, `gx-mcp-wire`, `gx-confine`), so
`cargo build --workspace` — this file's `postCreateCommand` — would fail at workspace
load on a fresh public clone today. Ship this file publicly only after that manifest gap
closes.

---
Derived from: `.devcontainer/devcontainer.json` (1 file, 2026-08-30). req/968 P-968-4.
