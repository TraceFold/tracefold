// AC-P4-5's second half: `scripts/gui_probe.mjs` actually completes submit -> verify -> commit ->
// undo -> a verified receipt card, using only the public entry point (`test/
// gui_probe_import_boundary.test.mjs` checks the import shape; this file checks that running it
// against a real `gx serve` succeeds). `GX_BINARY` required, same as `test/e2e.test.mjs`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { sdkRoot } from "../testlib/support.mjs";

const hasBinary = Boolean(process.env.GX_BINARY);
const skip = hasBinary ? false : "GX_BINARY is not set (see testlib/gx_process.mjs::gxBinary)";

test("AC-P4-5: scripts/gui_probe.mjs runs submit/verify/commit/undo/receipt-card SDK-only", { skip }, () => {
  const result = spawnSync(process.execPath, [join(sdkRoot(), "scripts", "gui_probe.mjs")], {
    cwd: sdkRoot(),
    env: process.env,
    encoding: "utf8",
    timeout: 30_000,
  });
  console.log(`GUI_PROBE_STDOUT=\n${result.stdout}`);
  if (result.stderr) console.log(`GUI_PROBE_STDERR=\n${result.stderr}`);
  assert.equal(result.status, 0, `gui_probe.mjs exited ${result.status}`);
  assert.ok(result.stdout.includes("gui_probe: PASS"), "gui_probe.mjs did not print its own PASS line");
});
