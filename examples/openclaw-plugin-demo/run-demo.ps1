# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 Glovrex
#
# One script. Runs the demo against a real gx binary and writes a transcript next to itself.
#
#   pwsh -File run-demo.ps1
#
# Requires: Node >= 23.6 (runs the .ts sources directly -- no build, no node_modules), and a `gx`
# binary reachable from the configured WSL distro. Build one with:
#
#   wsl -d Ubuntu-24.04 -e bash -lc 'cd /mnt/c/.../glovrex && cargo build -p gx-cli'
#
# Override either with GX_DEMO_BIN / GX_DEMO_DISTRO.

$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$log = Join-Path $here "demo-run.log"

# No machine-specific default for GX_DEMO_BIN on purpose (a demo that only runs against one
# person's absolute path is a demo that only ever ran on that machine) -- demo.ts throws a clear
# error naming this variable if it is unset. Build one with the command in the header above.
if (-not $env:GX_DEMO_DISTRO) {
  $env:GX_DEMO_DISTRO = "Ubuntu-24.04"
}

node (Join-Path $here "src/demo.ts") 2>&1 | Tee-Object -FilePath $log
$rc = $LASTEXITCODE

Write-Host ""
Write-Host "transcript: $log"
exit $rc
