// req/841 P1-2 -- SDK parity for the attach-source registry (`req/824` A4's three live routes).
//
// Red-first: this file was written and run BEFORE the client methods existed, and failed on the
// roster assertion. The data source is the committed wire bed
// `req/wire/fixtures/attach_source.jsonl` (w824-attach_source-00000..00009) -- the same vectors
// `crates/gx-api`'s own tests drive -- not invented request/response pairs, so a drift between
// what the SDK sends and what the fixture bed says the route takes is red here rather than
// discoverable only against a live server (`feedback_ground_truth_over_digest`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { GxClient, EXTENSION_METHODS, GxApiError } from "../dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const FIXTURES = join(here, "..", "..", "..", "req", "wire", "fixtures", "attach_source.jsonl");

/** The bed, one parsed vector per line. */
function vectors() {
  return readFileSync(FIXTURES, "utf8")
    .split("\n")
    .filter((l) => l.trim().length > 0)
    .map((l) => JSON.parse(l));
}

/** A vector by id -- throws if the bed no longer carries it, so a renumbered bed is loud. */
function vector(id) {
  const v = vectors().find((x) => x.vector_id === id);
  assert.ok(v, `fixture bed no longer carries ${id}`);
  return v;
}

/**
 * A `fetchImpl` that records the one request it receives and answers with a canned response.
 * Returns `{ calls, impl }`; `calls[0]` is `{ url, method, headers, body }`.
 */
function recordingFetch(status, responseBody) {
  const calls = [];
  const impl = async (url, init) => {
    calls.push({
      url: String(url),
      method: init?.method,
      headers: init?.headers ?? {},
      body: init?.body === undefined ? undefined : JSON.parse(init.body),
    });
    return new Response(JSON.stringify(responseBody), {
      status,
      headers: { "content-type": "application/json" },
    });
  };
  return { calls, impl };
}

/** Builds a plausible full response body from a vector's `body_contains` skeleton: `null` means
 * "present, value unchecked" in the bed's own convention, so each null gets a stand-in. */
function bodyFromContains(contains) {
  const standIns = {
    id: "src_0000000000000001",
    registered_at: "2026-08-26T00:00:00Z",
    limits: ["coverage is attach-source-declared, rendered as numerator/denominator, never as all"],
  };
  const body = {};
  for (const [k, v] of Object.entries(contains)) {
    body[k] = v === null ? (standIns[k] ?? `stand-in:${k}`) : v;
  }
  return body;
}

test("the fixture bed exists and carries the ten w824-attach_source vectors", () => {
  const all = vectors();
  console.log(`ATTACH_SOURCE_VECTORS=${all.length}`);
  assert.equal(all.length, 10, "the bed's vector count moved -- re-read req/wire/fixtures");
  assert.ok(all.every((v) => v.kind === "attach_source"));
});

test("req/841 P1-2: EXTENSION_METHODS names the three attach-source methods", () => {
  for (const name of ["registerAttachSource", "listAttachSources", "getAttachSource"]) {
    assert.ok(
      EXTENSION_METHODS.includes(name),
      `EXTENSION_METHODS does not name "${name}" -- the live route it projects ` +
        `(crates/gx-api/src/attach_sources.rs ATTACH_SOURCE_ENDPOINTS) has no SDK surface`,
    );
  }
});

test("w824-attach_source-00000: registerAttachSource sends the vector's verb/path/key/body and parses the row", async () => {
  const v = vector("w824-attach_source-00000");
  const { calls, impl } = recordingFetch(v.expected.status, bodyFromContains(v.expected.body_contains));
  const client = new GxClient({ baseUrl: "http://127.0.0.1:8787", token: "t", fetchImpl: impl });
  const row = await client.registerAttachSource(v.request.body, v.request.headers["idempotency-key"]);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, "http://127.0.0.1:8787/v1/attach-sources");
  assert.equal(calls[0].headers["idempotency-key"], v.request.headers["idempotency-key"]);
  assert.deepEqual(calls[0].body, v.request.body);
  // The honesty field the whole schema exists for: constantly false and present anyway.
  assert.equal(row.coverage_verified, false);
  assert.equal(row.kind, v.expected.body_contains.kind);
});

test("w824-attach_source-00002: listAttachSources surfaces the zero-inclusive census (total: 0 explicit)", async () => {
  const v = vector("w824-attach_source-00002");
  const { calls, impl } = recordingFetch(v.expected.status, {
    items: [],
    next_cursor: null,
    total: 0,
  });
  const client = new GxClient({ baseUrl: "http://127.0.0.1:8787", token: "t", fetchImpl: impl });
  const page = await client.listAttachSources();
  assert.equal(calls[0].method, "GET");
  assert.equal(calls[0].url, "http://127.0.0.1:8787/v1/attach-sources");
  assert.equal(page.total, 0, "an absent denominator reads as unknown; the census answers 0 explicitly");
  assert.deepEqual(page.items, []);
});

test("listAttachSources forwards limit and the numeric cursor as 44 §2.7 query params", async () => {
  const { calls, impl } = recordingFetch(200, { items: [], next_cursor: null, total: 0 });
  const client = new GxClient({ baseUrl: "http://127.0.0.1:8787", token: "t", fetchImpl: impl });
  await client.listAttachSources(5, 42);
  assert.equal(calls[0].url, "http://127.0.0.1:8787/v1/attach-sources?limit=5&cursor=42");
});

test("w824-attach_source-00005: an unknown source family surfaces as GxApiError VALIDATION_ERROR, not a silent default", async () => {
  const v = vector("w824-attach_source-00005");
  const { impl } = recordingFetch(v.expected.status, {
    type: "about:blank",
    title: "validation error",
    status: v.expected.status,
    detail: "kind outside the declared families",
    gx_code: v.expected.gx_code,
    verdict: v.expected.verdict,
  });
  const client = new GxClient({ baseUrl: "http://127.0.0.1:8787", token: "t", fetchImpl: impl });
  await assert.rejects(
    () => client.registerAttachSource(v.request.body, v.request.headers["idempotency-key"]),
    (err) => {
      assert.ok(err instanceof GxApiError, `expected GxApiError, got ${err?.constructor?.name}`);
      assert.equal(err.code, v.expected.gx_code);
      return true;
    },
  );
});

test("w824-attach_source-00007: getAttachSource on a never-registered id surfaces SOURCE_UNKNOWN", async () => {
  const v = vector("w824-attach_source-00007");
  const { calls, impl } = recordingFetch(v.expected.status, {
    type: "about:blank",
    title: "source unknown",
    status: v.expected.status,
    detail: "attach-source id not in the registry",
    gx_code: v.expected.gx_code,
    verdict: v.expected.verdict,
  });
  const client = new GxClient({ baseUrl: "http://127.0.0.1:8787", token: "t", fetchImpl: impl });
  await assert.rejects(
    () => client.getAttachSource("never-registered"),
    (err) => {
      assert.ok(err instanceof GxApiError);
      assert.equal(err.code, "SOURCE_UNKNOWN");
      return true;
    },
  );
  assert.equal(calls[0].method, "GET");
  assert.equal(calls[0].url, "http://127.0.0.1:8787/v1/attach-sources/never-registered");
});

test("getAttachSource URI-encodes the id segment (an id is data, never path syntax)", async () => {
  const { calls, impl } = recordingFetch(200, bodyFromContains({ id: null, kind: "vercel", registered_at: null, declared_coverage: {}, coverage_verified: false, limits: null }));
  const client = new GxClient({ baseUrl: "http://127.0.0.1:8787", token: "t", fetchImpl: impl });
  await client.getAttachSource("a/b?c");
  assert.equal(calls[0].url, "http://127.0.0.1:8787/v1/attach-sources/a%2Fb%3Fc");
});
