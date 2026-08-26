#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Put three real fs candidates on a running gx bed, so a window bound to it has rows --
# without them, `bound_smoke --expect bound` honestly fails B4 ("an answered read carries
# the engine's own row") against a freshly attached project, which is how req/822_c5
# first ran it. The body shape is `gx-api/src/handlers.rs` CreateCandidate; `context` is
# a fieldless variant so its wire form is its name (the same note membrane
# tools/smoke_serve.mjs S8 carries).
#
#   bash seed_bed.sh [bed-dir] [origin]
set -u
BED="${1:-/tmp/gxapp_smoke}"
ORIGIN="${2:-http://127.0.0.1:8791}"
TOK=$(cat "$BED/token")
for f in alpha beta gamma; do
  printf 'seed %s\n' "$f" > "$BED/$f.txt"
  BODY=$(printf '{"substrate":"fs","locator":"%s/%s.txt","goal":"seed %s","context":"Representation","actor":{"Human":{"key":"window"}}}' "$BED" "$f" "$f")
  curl -s -m 10 -X POST "$ORIGIN/v1/candidates" \
    -H "authorization: Bearer $TOK" -H 'content-type: application/json' \
    -d "$BODY" | head -c 120
  echo
done
curl -s "$ORIGIN/v1/healthz"
echo
