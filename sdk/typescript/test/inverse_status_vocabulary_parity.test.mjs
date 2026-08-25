// 🔴 **D-34 / DR-46-13** — the SDK's `InverseStatus` union against `gx-engine`'s enum, as an
// equality rather than as a memory.
//
// The defect this file exists for: the engine's `InverseStatus` had **seven** members and
// `sdk/typescript/src/types.ts` listed **six**. `Undetermined` — the word DR-46-13 raised and
// DR-46-26 gave a writer — decoded off the wire into a value no arm of a caller's `switch` could
// name, and TypeScript could not warn them, because an incomplete union is not an error: it is a
// type that lets a caller believe they matched exhaustively when they did not.
//
// That is R11's own defect wearing the opposite face. `req/240` L-06 (i) refused `unknown` for this
// field because a caller cannot switch on it; a union missing a member is worse, because the caller
// *can* switch on it and gets a false sense that they covered the space.
//
// Nothing measured the pairing, which is why it drifted. This is the measurement: the vocabulary is
// parsed out of both languages and compared as sets, so the next word added on either side fails
// the build on the other. `endpoint_parity.test.mjs` is the same move one surface over.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readRepoFile } from "../testlib/support.mjs";

/** The `InverseStatus` variants declared in `crates/gx-engine/src/store.rs`. */
function enginesVocabulary() {
  const source = readRepoFile("crates/gx-engine/src/store.rs");
  const start = source.indexOf("pub enum InverseStatus {");
  assert.notEqual(start, -1, "the enum moved — this parser is reading the wrong file");
  const body = source.slice(start + "pub enum InverseStatus {".length);
  const end = body.indexOf("\n}");
  assert.notEqual(end, -1, "the enum has no closing brace at column 0");
  // A variant is a line at one level of indentation that is not a doc comment, not an attribute
  // and not blank. `Consumed { by: TransformationId },` and `Available,` both match; the name is
  // everything before the first `,`, `{` or `(`.
  return body
    .slice(0, end)
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("//") && !line.startsWith("#["))
    .map((line) => line.split(/[,{(]/)[0].trim())
    .filter((name) => /^[A-Z][A-Za-z0-9]*$/.test(name));
}

/** The members of the `InverseStatus` union in `sdk/typescript/src/types.ts`. */
function sdkVocabulary() {
  const source = readRepoFile("sdk/typescript/src/types.ts");
  const start = source.indexOf("export type InverseStatus =");
  assert.notEqual(start, -1, "the union moved — this parser is reading the wrong file");
  const body = source.slice(start, source.indexOf(";", start));
  const bare = [...body.matchAll(/\|\s*"([A-Za-z0-9]+)"/g)].map((m) => m[1]);
  // The one externally-tagged member: serde writes `Consumed` as an object.
  const tagged = [...body.matchAll(/\|\s*\{\s*([A-Za-z0-9]+):/g)].map((m) => m[1]);
  return [...bare, ...tagged];
}

test("42 §3.12's vocabulary is the same set in the engine and in the SDK", () => {
  const engine = enginesVocabulary().sort();
  const sdk = sdkVocabulary().sort();
  console.log(`INVERSE_STATUS_ENGINE=${engine.length} ${JSON.stringify(engine)}`);
  console.log(`INVERSE_STATUS_SDK=${sdk.length} ${JSON.stringify(sdk)}`);

  const missingFromSdk = engine.filter((word) => !sdk.includes(word));
  const missingFromEngine = sdk.filter((word) => !engine.includes(word));
  assert.deepEqual(
    missingFromSdk,
    [],
    `the engine can write these and the SDK's union cannot name them: ${missingFromSdk}. ` +
      `A caller who switches on this field would fall through every arm. (D-34)`,
  );
  assert.deepEqual(
    missingFromEngine,
    [],
    `the SDK's union names words no engine writes: ${missingFromEngine}. ` +
      `A caller would write an arm that can never run.`,
  );
  assert.equal(engine.length, sdk.length, "and the counts are equal, which the two filters imply");
});

test("the seventh word is present by name", () => {
  // Named rather than counted: a count is satisfied by any seven words, and the whole of DR-46-13
  // is about *this* one — "nobody established whether an inverse exists", which is a different
  // fact from `Unavailable` ("we asked and there is none") with a different remedy.
  const sdk = sdkVocabulary();
  assert.ok(
    sdk.includes("Undetermined"),
    `DR-46-13's word is missing from the SDK union: ${JSON.stringify(sdk)}`,
  );
  assert.ok(sdk.includes("Unavailable"), "and it has not replaced the word it is distinct from");
});
