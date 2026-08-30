<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# .devcontainer/

A container definition so that opening this repository lands you on a built workspace instead of
on a cold cargo build.

---

| File | What it declares |
| :--- | :--- |
| [`devcontainer.json`](devcontainer.json) | Base image `mcr.microsoft.com/devcontainers/rust:1`; `postCreateCommand` runs `cargo build --workspace`; the `rust-analyzer` extension is installed for VS Code. Works with GitHub Codespaces and VS Code Remote Containers. |

---

## The toolchain version is not declared here

The image ships `rustup`, and [`rust-toolchain.toml`](../rust-toolchain.toml) at the repository
root is the single place the channel is pinned. `rustup` resolves it on the first cargo
invocation. Nothing in this folder restates a version, because a version written in two places is
two versions.

## What to expect on first open

`postCreateCommand` builds the whole workspace, so the first open is as slow as a cold build and
every open after it is not. The workspace member list in the root
[`Cargo.toml`](../Cargo.toml) matches the crates this repository carries, which is what
`cargo build --workspace` needs in order to load at all — it is worth checking that first if the
build stops before it reaches any Rust.

If you would rather verify from outside a container, [`tools/e2e.sh`](../tools/e2e.sh) does the
same build against a fresh clone.
