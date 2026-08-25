#!/usr/bin/env node
// The README's quickstart, as a runnable script (AC-P4-4: "keep the document and the real
// behaviour in sync"; sem: SEM-sdk-typescript-002).
//
// Reads the same two environment variables the README's three commands set up (`gx serve` and
// `GX_TOKEN`), and does the one thing a first-time reader wants to see: submit a change, watch it
// get judged, commit it, and hold a receipt nobody but this SDK and the server touched.
import { GxClient } from "../dist/index.js";
import { writeFileSync } from "node:fs";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const baseUrl = process.env.GX_BASE_URL ?? "http://127.0.0.1:8787";
const token = process.env.GX_TOKEN;
const keyId = process.env.GX_KEY_ID;
if (!token || !keyId) {
  console.error(
    "quickstart: set GX_TOKEN and GX_KEY_ID (the values `gx serve` and `gx key gen` printed) " +
      "and GX_BASE_URL if not the default http://127.0.0.1:8787",
  );
  process.exit(1);
}

const client = new GxClient({ baseUrl, token });

const scratchDir = mkdtempSync(join(tmpdir(), "tracefold-quickstart-"));
const target = join(scratchDir, "hello.txt");
writeFileSync(target, "hello\n");

const created = await client.createCandidate({
  substrate: "fs",
  locator: target,
  goal: "hello, gx\n",
  context: "Evidence",
  actor: { Human: { key: keyId } },
});
console.log(`submitted ${created.id} (${created.state})`);

const verified = await client.verifyCandidate(created.id);
console.log(`verdict: ${verified.verdict}`);

if (verified.verdict !== "Admit") {
  console.log("not admitted -- stopping here rather than forcing a commit that would be refused");
  process.exit(0);
}

const receipt = await client.commitCandidate(created.id);
console.log(`committed. receipt signed by ${receipt.envelope.signatures[0]?.keyid}`);
console.log("done.");
