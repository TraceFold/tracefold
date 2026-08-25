// AC-P4-4: "run the quickstart for real (execute the <= 3 commands the README lists by script =
// keep the document and the real behaviour in sync)". (sem: SEM-sdk-typescript-028)
//
// README.md's quickstart names three commands (`npm install`, setting `GX_TOKEN`/`GX_KEY_ID`,
// `node quickstart.mjs`) and shows `quickstart.mjs`'s contents inline. This test runs the real
// `scripts/quickstart.mjs` -- the same file, not a paraphrase of it -- against a real `gx serve`,
// with exactly the environment variables the README says to set. `GX_BINARY` required.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { sdkRoot } from "../testlib/support.mjs";
import { scratchProject, keyGen, startServe, warmProject } from "../testlib/gx_process.mjs";

const hasBinary = Boolean(process.env.GX_BINARY);
const skip = hasBinary ? false : "GX_BINARY is not set (see testlib/gx_process.mjs::gxBinary)";

test("AC-P4-4: README.md's quickstart.mjs matches scripts/quickstart.mjs", () => {
  const readme = readFileSync(join(sdkRoot(), "README.md"), "utf8");
  const script = readFileSync(join(sdkRoot(), "scripts", "quickstart.mjs"), "utf8");
  // The README's fenced `quickstart.mjs` block is documentation prose (imports from the package
  // name `"tracefold"`); the real script imports from `../dist/index.js` so it runs unpublished.
  // What must agree is the client calls themselves, not the import line.
  for (const call of ["createCandidate", "verifyCandidate", "commitCandidate"]) {
    assert.ok(readme.includes(call), `README's quickstart.mjs block does not call ${call}`);
    assert.ok(script.includes(call), `scripts/quickstart.mjs does not call ${call}`);
  }
});

test("AC-P4-4: the three README commands, run for real", { skip }, async (t) => {
  // Command 1, `npm install tracefold`: covered by `npm run build` having already produced
  // `dist/` (this repository's own copy, not a registry install -- `req/132` §3 keeps the
  // publish gate separate from this test; HARDRULE: a real publish is absolutely forbidden;
  // sem: SEM-sdk-typescript-029).
  const { project, home, target } = scratchProject("p4-quickstart");
  const { key_id } = await keyGen(project, home);
  await warmProject(project, home, target, key_id);
  const serving = await startServe(project, home, key_id);
  t.after(() => serving.stop());

  // Command 2, `export GX_TOKEN=... GX_KEY_ID=...`: the real values this `gx serve` printed.
  const env = {
    ...process.env,
    GX_BASE_URL: serving.baseUrl,
    GX_TOKEN: serving.token,
    GX_KEY_ID: key_id,
  };

  // Command 3, `node quickstart.mjs`.
  const result = spawnSync(process.execPath, [join(sdkRoot(), "scripts", "quickstart.mjs")], {
    cwd: sdkRoot(),
    env,
    encoding: "utf8",
    timeout: 20_000,
  });
  console.log(`QUICKSTART_STDOUT=\n${result.stdout}`);
  if (result.stderr) console.log(`QUICKSTART_STDERR=\n${result.stderr}`);
  assert.equal(result.status, 0, `quickstart.mjs exited ${result.status}`);
  assert.ok(result.stdout.includes("committed"), "quickstart.mjs did not report a commit");
});
