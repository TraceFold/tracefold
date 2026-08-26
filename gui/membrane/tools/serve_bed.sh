#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Stand a real gx server up in WSL so the membrane can be pointed at something that
# is not a fixture. Fixtures agreeing with each other is the failure req/01a §4-1
# measured; this script is what makes the contract tests answerable.
#
#   bash serve_bed.sh <gx-binary> <port> <log-file> [bed-dir]
#
# req/822_c5: the bed directory is an argument now. Two sessions sharing this worktree
# ran two beds the same day, and a hard-coded /tmp/gxapp_smoke means the second
# session's `rm -rf` below erases the first session's live ledger -- c5 came within one
# mangled path of doing exactly that. Name your own directory (e.g. /tmp/gxapp_c5) and
# the two beds cannot touch each other.
set -u

GX="${1:-/home/testuser2/gx_p1_target/debug/gx}"
PORT="${2:-8791}"
LOG="${3:-/tmp/gxapp_smoke/serve.log}"
BED="${4:-/tmp/gxapp_smoke}"

rm -rf "$BED"
mkdir -p "$BED"
cd "$BED" || exit 1
mkdir -p "$(dirname "$LOG")"

: > "$LOG"
{
  echo "== gx: $GX"
  "$GX" --version 2>&1 | head -1
  echo "== attach"
  "$GX" attach 2>&1 | tail -5
  echo "== key gen"
  "$GX" key gen 2>&1 | tail -3
  echo "== token"
  head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' > "$BED/token"
  cat "$BED/token"; echo
  echo "== keys"
  "$GX" key list 2>&1 | tail -3
} >> "$LOG" 2>&1

KEY=$("$GX" key list 2>/dev/null | grep -o '"key_id":"[^"]*"' | head -1 | cut -d'"' -f4)
echo "== signing key: ${KEY:-<none>}" >> "$LOG"
echo "== serve on 127.0.0.1:$PORT" >> "$LOG"

if [ -n "${KEY:-}" ]; then
  exec "$GX" serve --bind "127.0.0.1:$PORT" --token-file "$BED/token" --signing-key "$KEY" >> "$LOG" 2>&1
fi
exec "$GX" serve --bind "127.0.0.1:$PORT" --token-file "$BED/token" >> "$LOG" 2>&1
