# @mahirhir/tracefold

Typed HTTP client and offline receipt verification for a [Glovrex](https://github.com/TraceFold/tracefold) (`gx`) deployment.

**You can check a receipt with this package alone.** Verification runs in Node against a bundled
WebAssembly build of the engine's own verifier, so a JavaScript or TypeScript developer needs no
Rust toolchain, no WSL and no running server to decide whether a receipt someone handed them is
genuine. Producing a receipt is the other half and does need `gx`.

This package is a thin projection of the 44 API spec (internal to the development tree this
package is cut from -- not part of this public repository; see `req/` in
[README.md](https://github.com/TraceFold/tracefold#readme) for the ruling):
one method per HTTP endpoint, named error types for every `gx_code`, and offline verification of
the DSSE receipts a `gx` server issues. It holds no policy logic and no retry logic. It does not
decide anything a `gx` server has not already decided.

## Install

```sh
npm install @mahirhir/tracefold
```

Requires Node 18+. No runtime dependencies: HTTP goes through the global `fetch`, and offline
verification runs through a bundled WebAssembly module compiled from the same signature-checking
code path `gx`'s own CLI (`gx receipt verify --offline`) uses.

## Quickstart

Three commands, assuming a `gx serve` is already running (its own quickstart is `gx`'s, not this
package's, and the main repository's README covers it) and you have a token and a key id from it:

```sh
npm install @mahirhir/tracefold
export GX_TOKEN=<the token gx serve was started with> GX_KEY_ID=<from `gx key gen`>
node quickstart.mjs
```

`quickstart.mjs`:

```js
import { GxClient } from "@mahirhir/tracefold";

const client = new GxClient({
  baseUrl: process.env.GX_BASE_URL ?? "http://127.0.0.1:8842",
  token: process.env.GX_TOKEN,
});

const created = await client.createCandidate({
  substrate: "fs",
  locator: "/path/to/a/file",
  goal: "new content\n",
  context: "Evidence",
  actor: { Human: { key: process.env.GX_KEY_ID } },
});

const verified = await client.verifyCandidate(created.id);
console.log(`verdict: ${verified.verdict}`);

if (verified.verdict === "Admit") {
  const receipt = await client.commitCandidate(created.id);
  console.log(`committed, signed by ${receipt.envelope.signatures[0]?.keyid}`);
}
```

## Offline receipt verification

```js
import { verifyReceiptOffline } from "@mahirhir/tracefold";

const result = verifyReceiptOffline(
  JSON.stringify(receipt), // whatever `getReceipt`/`commitCandidate` returned
  keyId, // `gx key gen`'s `key_id`
  publicKeyBase64, // `gx key gen`'s `public_key`
  JSON.stringify(checkpoint), // `ledgerCheckpoint()`, taken at (or after) the commit -- see below
);

// { valid: true, checks: { signature: true, canonical_cid: true, inclusion: "verified", key_id },
//   anchor_authenticated: false, error: null }
```

No network call, no ledger read. This runs the same
`gx_witness::receipt::verify_offline` the `gx` binary itself runs, compiled to WebAssembly (see
"WASM, not a CLI shell-out" below).

### Run it now, against a real receipt

The block above names its inputs rather than fetching them. If you want to watch it work before
you have a server, three signed fixtures live in the public repository:

```sh
npm install @mahirhir/tracefold
F=https://raw.githubusercontent.com/TraceFold/tracefold/main/crates/gx-cli/tests/fixtures/attach_face_frozen/issued_2026_08_22
curl -sO $F/commit_receipt.json -O $F/key.pub.json -O $F/checkpoint.json
```

```js
import { readFileSync } from "node:fs";
import { verifyReceiptOffline } from "@mahirhir/tracefold";

const key = JSON.parse(readFileSync("key.pub.json", "utf8"));
const result = verifyReceiptOffline(
  readFileSync("commit_receipt.json", "utf8"),
  key.key_id,
  key.public_key,
  readFileSync("checkpoint.json", "utf8"),
  key.key_id,
  key.public_key,
);

console.log(result.valid, result.checks.inclusion, result.anchor_authenticated);
// true verified true
```

Change one character inside the receipt's `payload` and the same call returns `valid: false` with
`error: 'no valid signature under key "ed25519-703643751f18a688" (34 AC-019)'`. Delete the last
three arguments and it returns `valid: false` with `inclusion: "unanchored"`, which is a refusal to
call a receipt verified rather than a claim that anything was tampered with.

### `anchor_authenticated`, or who vouched for the checkpoint

The verification above reads the checkpoint's `tree_size` and `root_hash` and **never asks who
signed them**. That is deliberate, because the log's key may differ from the receipt's key and a verifier
holding one public key cannot assume it verifies both. It is also exactly what a forger wants,
because a third party normally receives the receipt *and* the checkpoint from the same hand: hand
over a head signed by nobody, or one belonging to a different log, and the arithmetic still holds.

So every answer carries `anchor_authenticated`, and it is `false` until something checks. Pass the
checkpoint's own key to change that:

```js
const result = verifyReceiptOffline(
  JSON.stringify(receipt),
  keyId,
  publicKeyBase64,
  JSON.stringify(checkpoint),
  ledgerKeyId, // the key the checkpoint was signed with -- `gx log checkpoint`'s signer
  ledgerPublicKeyBase64,
);
// -> anchor_authenticated: true, or a refusal naming `checkpoint_key:` if the head does not verify
```

The two key arguments move together: one without the other is refused rather than half-checked.
This is the same opt-in the CLI offers as `gx receipt verify --checkpoint-key <FILE>`, with the
same field on the wire.

`anchor_authenticated: false` is **not** the same as "there was no anchor". Read
`checks.inclusion` for that (`unanchored` / `not_applicable`). `false` beside
`inclusion: "verified"` means the proof reached a root nobody vouched for.

### Every argument is a string

All six parameters are JSON *text*, not parsed values, and the types that say so are erased at
runtime. `getReceipt()` returns an object, so the natural mistake is to forget one
`JSON.stringify`. That is refused with `{ valid: false, checks: null, error: "receiptJson: expected
a string, received object..." }` rather than throwing. This function never throws, for any
argument shape.

**Pin the checkpoint to the receipt's own tree size.** An `InclusionProof` is computed against the
ledger's size at commit time (42 §3.11). A checkpoint fetched *later*, after other commits have
appended more leaves, has a different root and will not reconstruct an older proof. That reads as
`inclusion: "refuted"`, and it is not a sign of tampering, only of the wrong checkpoint. Fetch the
checkpoint right after the commit whose receipt you are about to verify, not at some later point.

For a `VerdictReceipt` (a verdict before commit, or after a `Deny`/`Escalate`), pass no checkpoint
at all, because ASM-14 says there is nothing in the ledger yet, and the result reads
`inclusion: "not_applicable"`.

## Errors

Every non-2xx response from `gx-api` becomes a `GxApiError` carrying 44 §2.3's five
`problem+json` fields (`type`, `title`, `status`, `detail`, `gx_code`), never a bare string:

```js
import { GxApiError } from "@mahirhir/tracefold";

try {
  await client.commitCandidate(id);
} catch (e) {
  if (e instanceof GxApiError && e.code === "NOT_ADMITTED") {
    // the gate refused this transformation; e.detail says why
  } else {
    throw e;
  }
}
```

A response whose body was not `problem+json` (a proxy in front of `gx-api`, a load balancer) is a
`GxTransportError` instead, so a caller can tell "the engine refused" from "the network refused"
without inspecting a string.

## Endpoint coverage (v0.1)

Every row of 44 §2.1's table has a method: `createCandidate`, `getCandidate`, `verifyCandidate`,
`commitCandidate`, `escalateCandidate`, `cancelCandidate`, `undoTransformation`,
`replayTransformation`, `getTransformation`, `getReceipt`, `ledgerProof`, `ledgerCheckpoint`,
`issueVerdictCheckpoint`, `listVerdictCheckpoints`, `getVerdictCheckpoint`, `stream`, `healthz`.
`GxClient.SPECIFIED_METHODS` names them in this order and `test/endpoint_parity.test.mjs` checks
the count against a parse of the spec itself, not against this paragraph.

`GET /candidates`, `GET /escalations`, `GET /transformations` and `GET /ledger/consistency` are
44 §2.6/§2.7 additions rather than rows of §2.1's own table. `ledgerConsistency` has a method;
the other three are reachable through `client.raw("GET", "/candidates")` and are not named methods
in v0.1 (declared, not silently dropped; see `GxClient.EXTENSION_METHODS`).

## Not in v0.1 (declared, not silently dropped)

- **Retry, a connection pool, a cache.** This client makes one request per call and does not
  retry a failed one. `gx-api`'s `Idempotency-Key` support is exposed as an optional argument to
  `commitCandidate`/`undoTransformation`; retrying on top of it is a caller's decision.
- **A parsed `GET /stream` client.** `client.stream()` returns the raw `fetch` `Response`; a
  caller reads `response.body` (a `ReadableStream` of NDJSON) directly. An SSE/JSONL parsing
  client is a v0.2 addition.
- **A Python SDK.** Not written yet.
- **A GUI.** `scripts/gui_probe.mjs` in this package's source tree is a machine check that a GUI
  can be built on this client's public exports alone (submit, verify, commit, undo, and a
  verified receipt card) without reaching into anything this README does not document. The GUI
  itself is a separate, later piece of work.

## WASM, not a CLI shell-out

Offline verification bundles a small `wasm-bindgen` module (`sdk/wasm-verify` in the source
repository) rather than shelling out to `gx receipt verify --offline`, because the verification
path, which is a signature check plus deterministic hashing, never touches the operating system's entropy
source (only key *generation* does, and this SDK never generates a key). Measured, not assumed:
the compiled module's only WebAssembly imports are `wasm-bindgen`'s own plumbing, none of them
related to randomness. Full measurement in the source repository's `req/133` §1.
