// 🔴 **R11 / `req/240` L-06 (ii) — the SDK's `gx_code` vocabulary, counted against the server's.**
//
// The finding this is the gate for: `GX_CODES` held thirteen words and said so in its own doc
// comment ("Thirteen codes on the wire, not twelve") while `crates/gx-api/src/gx_code.rs` had grown
// `RULED_ADDITIONS` from two entries to nine. Twenty-one codes can arrive at a TypeScript caller
// and eight of them had no name here — `BUSY`, `LEDGER_DISAGREES`, `DECLARATION_UNREADABLE`,
// `DECLARATION_ABSENT`, `CONFIG_ABSENT`, `UNAVAILABLE`, `PAYLOAD_TOO_LARGE`,
// `UNSUPPORTED_MEDIA_TYPE` — so `GxApiError.code` widened them to `string` and 44 §2.6's
// forward-compatibility rule did the work of a table nobody had updated.
//
// A census and not a copy: the two lists are read out of the Rust source, so this file fails the
// day a tenth ruled addition lands, which is the only kind of test that can keep two vocabularies
// in one place from drifting again. Same shape as `probes/doubt/tests/m6_gx_code.rs` (which
// compares 44 §2.3's markdown with the Rust `GX_CODES`) and `exit_map.rs`, one language over.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readRepoFile } from "../testlib/support.mjs";

// 🔴 **R12 / `req/242` M-04** — read from `src/`, not from `dist/`.
//
// This line was `import { GX_CODES } from "../dist/index.js"`, and `dist/` is a build output
// `.gitignore` excludes. So the census compared the server's **source** with the SDK's
// **artifact**: a `src/errors.ts` that went backwards while a stale `dist/` stayed correct passed
// this file green, which is the exact fail-open shape the probe exists to prevent one language
// over. `req/242` M-04 measured it on this very tree — `src/` held R11's twenty-one words and the
// `dist/` on the disk held thirteen.
//
// Parsed rather than imported, because a TypeScript source cannot be `import`ed by `node --test`
// without the build this probe must not depend on.
function sdkCodes() {
  const source = readRepoFile("sdk/typescript/src/errors.ts");
  const start = source.indexOf("export const GX_CODES = [");
  assert.notEqual(start, -1, "`src/errors.ts` no longer declares `GX_CODES`");
  const body = source.slice(start, source.indexOf("] as const;", start));
  return [...body.matchAll(/"([A-Z_]+)"/g)].map((m) => m[1]);
}

const GX_CODES = sdkCodes();

/** `gx_code.rs`'s `GX_CODES` — 44 §2.3's twelve, transcribed. */
function serverTableCodes() {
  const source = readRepoFile("crates/gx-api/src/gx_code.rs");
  const start = source.indexOf("pub const GX_CODES:");
  assert.notEqual(start, -1, "`gx_code.rs` no longer declares `GX_CODES`");
  const body = source.slice(start, source.indexOf("];", start));
  return [...body.matchAll(/code:\s*"?([A-Z_]+)"?\s*,/g)].map((m) => m[1]);
}

/** `gx_code.rs`'s `RULED_ADDITIONS` — the codes 44 §2.3 has no row for, each ruled in `req/38`. */
function serverRuledAdditions() {
  const source = readRepoFile("crates/gx-api/src/gx_code.rs");
  const start = source.indexOf("pub const RULED_ADDITIONS:");
  assert.notEqual(start, -1, "`gx_code.rs` no longer declares `RULED_ADDITIONS`");
  const body = source.slice(start, source.indexOf("\n];", start));
  return [...body.matchAll(/code:\s*"?([A-Z_]+)"?\s*,/g)].map((m) => m[1]);
}

test("the SDK names every gx_code this server can send, and no others", () => {
  const table = serverTableCodes();
  const ruled = serverRuledAdditions();
  const onTheWire = [...table, ...ruled];
  console.log(
    `GX_CODE_CENSUS table=${table.length} ruled=${ruled.length} wire=${onTheWire.length} ` +
      `sdk=${GX_CODES.length}`,
  );
  console.log(`GX_CODE_CENSUS wire=${JSON.stringify(onTheWire)}`);
  console.log(`GX_CODE_CENSUS sdk=${JSON.stringify([...GX_CODES])}`);

  const missing = onTheWire.filter((code) => !GX_CODES.includes(code));
  assert.deepEqual(
    missing,
    [],
    `these codes are on the wire and the SDK cannot name them: ${missing.join(", ")}. ` +
      "A caller switching on `GxApiError.code` gets `string` for each of them (req/240 L-06).",
  );

  const invented = [...GX_CODES].filter((code) => !onTheWire.includes(code));
  assert.deepEqual(
    invented,
    [],
    `these are in the SDK's table and no longer on the wire: ${invented.join(", ")}. ` +
      "A vocabulary that only grows is one that stops being a census.",
  );

  assert.equal(
    GX_CODES.length,
    onTheWire.length,
    "the census is an equality, not a subset — duplicates would satisfy both filters above",
  );
});

test("the count is thirty-two, and the doc comment says so", () => {
  const source = readRepoFile("sdk/typescript/src/errors.ts");
  assert.equal(GX_CODES.length, 32, `GX_CODES holds ${GX_CODES.length} words`);
  assert.ok(
    source.includes("Thirty-two"),
    "the prose above `GX_CODES` is what a reader trusts before they count; `req/240` L-06 (ii) " +
      "is the measurement of it being wrong for four releases",
  );
});

// 🔴 **R12 / `req/242` M-04** — and when a build **is** present, it has to agree with the
// source it was built from.
//
// The census above deliberately reads `src/`. That closes the fail-open the audit measured and
// opens the mirror of it: a `dist/` older than `src/` is what a consumer of this package actually
// receives (`package.json`'s `main`, `types` and `files` all point at `dist`). So the artifact is
// compared here when it exists, and skipped — out loud — when it does not, because `dist/` is
// git-ignored and a fresh clone has none until `npm run build` has run.
test("a built dist/ agrees with the source the census reads", async () => {
  let built;
  try {
    built = await import("../dist/index.js");
  } catch {
    console.log(
      "GX_CODE_CENSUS dist=absent (git-ignored build output; `npm run build` makes one). " +
        "The census above read src/errors.ts and is unaffected.",
    );
    return;
  }
  console.log(`GX_CODE_CENSUS dist=${JSON.stringify([...built.GX_CODES])}`);
  assert.deepEqual(
    [...built.GX_CODES],
    GX_CODES,
    "the package a consumer installs ships `dist/`; a `dist/` that disagrees with `src/` is the " +
      "vocabulary they actually get (req/242 M-04)",
  );
});
