// AC-P4-5: "a probe script that imports only the SDK's public API can obtain the information
// equivalent to submit -> commit -> undo -> receipt display (zero lower-layer imports, confirmed
// mechanically by grep)." (sem: SEM-sdk-typescript-026)
//
// Two halves: this file's first test greps `scripts/gui_probe.mjs` for any import reaching under
// this package's own `dist/` other than `dist/index.js` (the "0 deep imports" half); the second
// half — that the probe script actually *runs* the submit/commit/undo/receipt-display flow — is
// `test/gui_probe_runs.test.mjs`, which needs `GX_BINARY` and is therefore a separate file (the
// same split `test/e2e.test.mjs` takes).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { sdkRoot } from "../testlib/support.mjs";

function importSpecifiers(source) {
  const specifiers = [];
  const re = /import\s+(?:[^'"]+?\s+from\s+)?["']([^"']+)["']/g;
  let m;
  while ((m = re.exec(source))) specifiers.push(m[1]);
  return specifiers;
}

test("AC-P4-5: gui_probe.mjs imports only this package's public entry point, never a deep dist path", () => {
  const path = join(sdkRoot(), "scripts", "gui_probe.mjs");
  const source = readFileSync(path, "utf8");
  const specifiers = importSpecifiers(source);
  console.log(`GUI_PROBE_IMPORTS=${JSON.stringify(specifiers)}`);

  const deep = specifiers.filter(
    (s) => (s.includes("/dist/") || s.includes("tracefold/dist/")) && !s.endsWith("/dist/index.js"),
  );
  assert.deepEqual(
    deep,
    [],
    `gui_probe.mjs imports below this package's public surface (R-GUI-on-SDK's "zero lower-layer imports"; sem: SEM-sdk-typescript-027): ${deep}`,
  );

  const packageImports = specifiers.filter((s) => s.includes("dist/index.js"));
  assert.ok(
    packageImports.length > 0,
    "gui_probe.mjs must import the package's public entry point at least once",
  );
});

test("AC-P4-5: the public entry point is the only file gui_probe.mjs takes SDK symbols from", () => {
  // Every SDK symbol the probe uses (GxClient, verifyReceiptOffline) has to come from the one
  // import in the test above -- this is the mechanical form of "smooth to build a GUI on": a
  // symbol reachable only through a deeper path would be a symbol this test's first half missed.
  const path = join(sdkRoot(), "scripts", "gui_probe.mjs");
  const source = readFileSync(path, "utf8");
  const usedNames = ["GxClient", "verifyReceiptOffline"];
  for (const name of usedNames) {
    assert.ok(
      new RegExp(`import\\s*\\{[^}]*\\b${name}\\b[^}]*\\}\\s*from\\s*["'][^"']*dist/index\\.js["']`).test(
        source,
      ),
      `${name} must be imported from dist/index.js in gui_probe.mjs`,
    );
  }
});
