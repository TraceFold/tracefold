// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
// 🔴 **E-SDK-9** (`req/38` §285, `req/503` §1) -- the shipped `.wasm` is a build artefact of a
// crate this suite cannot compile, and nothing in `npm run build` rebuilds it.
//
// `package.json`'s `"build"` is `tsc -p tsconfig.json && node scripts/copy-wasm.mjs`. `tsc` compiles
// the TypeScript and `copy-wasm.mjs` copies `src/wasm-gen/wasm_verify_bg.wasm` into `dist/`
// **if it exists**; neither runs `sdk/wasm-verify/build.sh`, which is the only thing that produces
// it. So the binary in `dist/` -- the one `files: ["dist"]` publishes to npm -- is whatever was on
// the packager's disk. Measured on 2026-08-22: it was five months of Rust behind, missing the fifth
// `InclusionCheck` word (`unbridged`, H-09) that `sdk/wasm-verify/src/lib.rs` had implemented and
// that `src/verify.ts`'s own `InclusionCheck` union already promised callers.
//
// `tools/e2e.sh` stage 5b *does* run `build.sh` before `npm ci` (R12 / req/242 M-04 added it), so a
// full floor run is not exposed. A developer running `npm test`, and `npm publish` from any
// machine, are.
//
// # Why a census and not five literals
//
// The same argument `test/gx_code_census.test.mjs` makes for `gx_code`: a hardcoded copy of the
// five words is a sixth place the vocabulary lives, and the day a sixth word is added this file
// would keep passing while the shipped binary stayed one word short -- the exact failure it exists
// to catch. So the words are **read out of the Rust** at test time, from both of the two places
// that spell them, and the count is whatever those files say rather than the number 5.
//
// # What this test does not claim
//
// A byte scan proves the string is *in* the binary, not that the branch producing it is reachable.
// That is what `sdk/wasm-verify/src/lib.rs`'s own `#[cfg(test)] mod tests` measures, on the host
// target, by driving the branches. This suite's job is narrower and is the one nothing else does:
// **the artefact on the disk was built from the source in this tree**.
//
// # Argument-count freshness (added 2026-08-23, `req/38` §370, E-SDK-1+9 repair lane)
//
// A vocabulary word can be present in the binary while the *glue*'s exported function still only
// takes the old, shorter argument list -- wasm-bindgen's `--target nodejs` glue is generated at the
// same stale moment as the `.wasm` and is a second place staleness hides. `req/607`'s NC-3 measured
// this directly: a shipped glue with `verify_receipt_offline.length === 4` silently drops a caller's
// 5th/6th JS arguments (JS does not error on extra arguments), so a checkpoint's key material never
// reaches the Rust side and a receipt that should authenticate does not. The count comes from
// `lib.rs`'s own signature, not a literal `6`, for the same reason the vocabulary is not five
// literals above.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { verifyReceiptOffline } from "../dist/index.js";
import { verify_receipt_offline as wasmGlueVerifyReceiptOffline } from "../dist/wasm-gen/wasm_verify.js";

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = dirname(here);
const repoRoot = dirname(dirname(pkgRoot));

const WASM_SOURCE = join(repoRoot, "sdk", "wasm-verify", "src", "lib.rs");
const CLI_SOURCE = join(repoRoot, "crates", "gx-cli", "src", "receipt.rs");
const SHIPPED_WASM = join(pkgRoot, "dist", "wasm-gen", "wasm_verify_bg.wasm");

/** The arms of `sdk/wasm-verify/src/lib.rs::inclusion_word` -- the producing side, and therefore
 * the side whose words have to be inside the binary. Read as `InclusionCheck::X => "word",`. */
function wordsFromWasmCrate() {
  const src = readFileSync(WASM_SOURCE, "utf8");
  const body = src.slice(src.indexOf("fn inclusion_word("));
  const arms = body.slice(0, body.indexOf("\n}"));
  return [...arms.matchAll(/InclusionCheck::\w+\s*=>\s*"([a-z_]+)"/g)].map((m) => m[1]);
}

/** `crates/gx-cli/src/receipt.rs::INCLUSION_JSON`'s first column -- the second spelling, which
 * `sdk/wasm-verify`'s own test already asserts equal to the first. Read here too so that this file
 * fails rather than silently agreeing if the two ever drift while the binary is fresh. */
function wordsFromCli() {
  const src = readFileSync(CLI_SOURCE, "utf8");
  const decl = src.slice(src.indexOf("pub const INCLUSION_JSON"));
  const table = decl.slice(0, decl.indexOf("\n];"));
  return [...table.matchAll(/\(\s*"([a-z_]+)"\s*,/g)].map((m) => m[1]);
}

/** The parameter count of `pub fn verify_receipt_offline(...)` itself -- paren-depth-tracked so an
 * `Option<...>` type never confuses the split (none of this signature's types nest parens, but the
 * count must not depend on that staying true). */
function rustVerifyReceiptOfflineArity() {
  const src = readFileSync(WASM_SOURCE, "utf8");
  const sigStart = src.indexOf("pub fn verify_receipt_offline(");
  assert.ok(sigStart >= 0, `could not find "pub fn verify_receipt_offline(" in ${WASM_SOURCE}`);
  const openParen = src.indexOf("(", sigStart);
  let depth = 1;
  let i = openParen + 1;
  while (depth > 0) {
    if (src[i] === "(") depth++;
    else if (src[i] === ")") depth--;
    i++;
  }
  const paramsText = src.slice(openParen + 1, i - 1);
  return paramsText
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean).length;
}

test("the two Rust spellings of checks.inclusion agree, and there is a countable number of them", () => {
  const wasmWords = wordsFromWasmCrate();
  const cliWords = wordsFromCli();
  console.log(`WASM_VOCAB_RUST=${JSON.stringify(wasmWords)}`);
  console.log(`WASM_VOCAB_CLI=${JSON.stringify(cliWords)}`);
  // A parse that returned nothing would make every assertion below vacuous -- the shape
  // `test/gx_code_census.test.mjs` calls "a non-trivial denominator".
  assert.ok(wasmWords.length >= 4, `parsed too few words out of ${WASM_SOURCE}: ${wasmWords}`);
  assert.deepEqual(
    wasmWords,
    cliWords,
    "sdk/wasm-verify and gx-cli must spell checks.inclusion identically",
  );
});

test("E-SDK-9: every word the Rust produces is inside the .wasm that ships", () => {
  const words = wordsFromWasmCrate();
  const bytes = readFileSync(SHIPPED_WASM);
  // `latin1` maps every byte to one code unit, so a substring search over it is a byte search --
  // no UTF-8 decoding, and no lost bytes on a binary that is not text.
  const asBytes = bytes.toString("latin1");
  const found = words.filter((w) => asBytes.includes(w));
  const missing = words.filter((w) => !asBytes.includes(w));
  console.log(
    `WASM_VOCAB_CENSUS file=${SHIPPED_WASM} bytes=${bytes.length} words=${words.length} found=${found.length} missing=${JSON.stringify(missing)}`,
  );
  assert.deepEqual(
    missing,
    [],
    `dist/wasm-gen/wasm_verify_bg.wasm does not contain ${JSON.stringify(missing)}. It was built ` +
      `from older Rust than this tree holds. Rebuild it: bash sdk/wasm-verify/build.sh (WSL: cargo ` +
      `+ wasm-bindgen), then npm run build.`,
  );
  assert.equal(found.length, words.length);
});

test("E-SDK-9/E-SDK-10: the shipped .wasm answers with the fields this tree's Rust renders", () => {
  // A byte scan can be satisfied by a string sitting in a data section. This drives the boundary
  // instead, on the one input that needs no fixture: a refusal. `anchor_authenticated` is rendered
  // on **every** answer (M6H8-11 adopted (a)), so its absence here means the binary predates
  // E-SDK-10 no matter what the byte scan said.
  const answer = verifyReceiptOffline("not json at all", "some-key-id", "AAAA");
  console.log(`WASM_FRESHNESS_ANSWER=${JSON.stringify(answer)}`);
  assert.ok(
    Object.prototype.hasOwnProperty.call(answer, "anchor_authenticated"),
    `the shipped .wasm renders no anchor_authenticated -- it predates E-SDK-10. Rebuild it: bash sdk/wasm-verify/build.sh, then npm run build. Got: ${JSON.stringify(answer)}`,
  );
  assert.equal(answer.anchor_authenticated, false);
  assert.equal(answer.valid, false);
  assert.equal(answer.checks, null);
});

test("E-SDK-9: the shipped glue's verify_receipt_offline takes as many arguments as lib.rs declares", () => {
  const rustArity = rustVerifyReceiptOfflineArity();
  const glueArity = wasmGlueVerifyReceiptOffline.length;
  console.log(`WASM_ARITY_RUST=${rustArity} WASM_ARITY_GLUE=${glueArity}`);
  assert.ok(rustArity >= 4, `parsed too few parameters out of ${WASM_SOURCE}: ${rustArity}`);
  assert.equal(
    glueArity,
    rustArity,
    `dist/wasm-gen/wasm_verify.js's verify_receipt_offline takes ${glueArity} argument(s) but ` +
      `lib.rs declares ${rustArity}. A stale glue silently drops the caller's trailing arguments ` +
      `(req/607 NC-3: checkpoint key material never reaches the check). Rebuild it: bash ` +
      `sdk/wasm-verify/build.sh (WSL: cargo + wasm-bindgen), then npm run build.`,
  );
});
