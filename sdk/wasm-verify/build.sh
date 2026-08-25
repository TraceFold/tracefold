#!/usr/bin/env bash
# Builds sdk/wasm-verify for wasm32-unknown-unknown and runs wasm-bindgen (Node target) to produce
# the JS glue + .wasm + .d.ts the TypeScript SDK bundles (req/132 §5 item 1 / §2 item 2).
#
# Requires: rustup target wasm32-unknown-unknown, wasm-bindgen-cli matching the `wasm-bindgen`
# crate version pinned in Cargo.toml (0.2.x). Run from WSL (HARDRULE: cargo/node execution=WSL
# only, via a script file) (sem: SEM-sdk-wasm-verify-003).
set -euo pipefail
here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$here/../.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.sg/target}"
export PATH="$HOME/.cargo/bin:$PATH"

cd "$repo_root"
echo "build.sh: cargo build --release --target wasm32-unknown-unknown -p wasm-verify"
cargo build --release --target wasm32-unknown-unknown -p wasm-verify

wasm="$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/wasm_verify.wasm"
out="$here/../typescript/src/wasm-gen"
mkdir -p "$out"
echo "build.sh: wasm-bindgen --target nodejs --out-dir $out $wasm"
wasm-bindgen --target nodejs --out-dir "$out" "$wasm"
echo "build.sh: done -> $out"
ls -la "$out"
