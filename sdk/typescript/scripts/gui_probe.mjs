#!/usr/bin/env node
// R-GUI-on-SDK's "vessel of inspection" (req/132 §1, AC-P4-5; sem: SEM-sdk-typescript-001): a
// probe script that imports **only** this
// package's public surface (`tracefold`'s `index.js` -- never a deep path like
// `tracefold/dist/client.js`) and still manages submit -> commit -> undo -> "receipt display"
// (a client reading everything an L1-L2 disclosure needs, `48 §3.1`'s shape one crate over).
//
// If this script needs anything `../dist/index.js` (or an installed `tracefold`) does not export,
// that is SDK API surface / GUI-readiness gap and is the finding R-GUI-on-SDK exists to produce --
// not a reason to reach one level deeper.
import { GxClient, verifyReceiptOffline } from "../dist/index.js";
import { scratchProject, keyGen, startServe, warmProject } from "../testlib/gx_process.mjs";

async function main() {
  const { project, home, target } = scratchProject("p4-gui-probe");
  const { key_id, public_key } = await keyGen(project, home);
  await warmProject(project, home, target, key_id);
  const serving = await startServe(project, home, key_id);

  try {
    const client = new GxClient({ baseUrl: serving.baseUrl, token: serving.token });

    // submit (+plan, 44 §2.1's one call)
    const created = await client.createCandidate({
      substrate: "fs",
      locator: target,
      goal: "gui-probe-after\n",
      context: "Evidence",
      actor: { Human: { key: key_id } },
    });
    console.log(`gui_probe: created ${created.id} (state=${created.state})`);

    // verify
    const verified = await client.verifyCandidate(created.id);
    console.log(`gui_probe: verified verdict=${verified.verdict} state=${verified.state}`);

    // commit
    const receipt = await client.commitCandidate(created.id);
    console.log(`gui_probe: committed, receipt payload_type=${receipt.envelope.payload_type}`);

    // "receipt display" (48 §3.1's L1-L2): everything a GUI's summary card needs, off the client
    // alone -- no deep import, no second HTTP library. The checkpoint is read **here**, right
    // after the commit its own inclusion proof was built against: an `InclusionProof` carries the
    // `tree_size` it was computed at (42 §3.11), and a later checkpoint (a bigger tree, after the
    // undo below appends its own leaf) has a different root that this proof was never built to
    // reconstruct -- `checks.inclusion` would read "refuted" for a reason that is the wrong
    // checkpoint, not a bad receipt (verified against this repo's own gx-log arithmetic, not
    // guessed: `gx_log::proof::verify_inclusion_of` compares root hashes at one tree size).
    const finalReceipt = await client.getReceipt(created.id);
    const checkpoint = await client.ledgerCheckpoint();
    const offline = verifyReceiptOffline(
      JSON.stringify(finalReceipt),
      key_id,
      public_key,
      JSON.stringify(checkpoint),
    );

    // undo (43 §5-2, a whole second pipeline through the same client) -- after the receipt card is
    // built, so the card above is pinned to the checkpoint its own proof belongs to.
    const undone = await client.undoTransformation(created.id, { actor: { Human: { key: key_id } } });
    console.log(
      `gui_probe: undone -> new transformation, original superseded_state=${undone.superseded_state}`,
    );
    console.log(
      `gui_probe: receipt card -- valid=${offline.valid} inclusion=${offline.checks?.inclusion} ` +
        `key_id=${offline.checks?.key_id}`,
    );

    if (!offline.valid) {
      throw new Error(`gui_probe: the final commit receipt did not verify offline: ${JSON.stringify(offline)}`);
    }
    console.log("gui_probe: PASS -- submit, verify, commit, undo and a verified receipt card, SDK-only");
  } finally {
    await serving.stop();
  }
}

main().catch((e) => {
  console.error("gui_probe: FAIL", e);
  process.exitCode = 1;
});
