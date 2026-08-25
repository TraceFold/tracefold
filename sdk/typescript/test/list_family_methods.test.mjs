// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
// req/727 §2-4: `GET /candidates`, `GET /escalations`, `GET /transformations` (the list-family, 44
// §2.7 extensions, `crates/gx-api/src/list.rs`) had no dedicated named method -- reachable only
// through `raw()`. This file is the shape/URL-construction test for the three methods that closed
// that gap (`listCandidates`, `listEscalations`, `listTransformations`, `client.ts`), following the
// same zero-dependency `node:http` stub pattern `audit_m9_p4_tamper_and_errors.test.mjs` uses for
// its non-`GX_BINARY` coverage -- no real `gx serve` process, so these run in every environment.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { GxClient } from "../dist/index.js";

/** Same helper as `audit_m9_p4_tamper_and_errors.test.mjs::startStub` -- a minimal local server
 * that lets a test inspect the request `GxClient` actually sent, not just the response it parsed. */
function startStub(responder) {
  return new Promise((resolve) => {
    const server = createServer((req, res) => {
      responder(req, res);
    });
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      resolve({ baseUrl: `http://127.0.0.1:${port}`, stop: () => server.close() });
    });
  });
}

/** 44 §2.7's envelope, one canned row per list -- the shape `crates/gx-api/src/list.rs::page_of`
 * answers with, echoing back the request's method+URL as `_seen` so a test can assert both the wire
 * request GxClient made and the parsed value it returned in one round trip. */
function stubPage(req, res, item) {
  res.writeHead(200, { "content-type": "application/json" });
  res.end(JSON.stringify({ items: [item], next_cursor: null, _seen: `${req.method} ${req.url}` }));
}

test("listCandidates: bare GET /v1/candidates (no params), and the page shape round-trips", async (t) => {
  const stub = await startStub((req, res) =>
    stubPage(req, res, {
      transformation: "gx1:tid",
      state: "Candidate",
      verdict: null,
      enforced: null,
      created_at: null,
      actor: null,
      scope: null,
    }),
  );
  t.after(() => stub.stop());
  const client = new GxClient({ baseUrl: stub.baseUrl, token: "irrelevant" });
  const page = await client.listCandidates();
  console.log(`LIST_CANDIDATES_RESULT=${JSON.stringify(page)}`);
  assert.equal(page._seen, "GET /v1/candidates", "listCandidates() with no args must not append a query string");
  assert.equal(page.items.length, 1);
  assert.equal(page.items[0].transformation, "gx1:tid");
  assert.equal(page.next_cursor, null);
});

test("listCandidates: limit/cursor become 44 §2.7's query string", async (t) => {
  const stub = await startStub((req, res) => stubPage(req, res, { transformation: "gx1:tid" }));
  t.after(() => stub.stop());
  const client = new GxClient({ baseUrl: stub.baseUrl, token: "irrelevant" });
  const page = await client.listCandidates(10, "opaque-cursor-1");
  assert.equal(page._seen, "GET /v1/candidates?limit=10&cursor=opaque-cursor-1");
});

test("listEscalations: bare GET /v1/escalations, and the ticket-shaped row round-trips", async (t) => {
  const stub = await startStub((req, res) =>
    stubPage(req, res, {
      transformation: "gx1:tid",
      ticket_id: "gx1:ticket",
      state: "Escalated",
      reasons: [],
      required_approval: null,
      created_at: "2026-08-24T00:00:00Z",
      deadline: null,
    }),
  );
  t.after(() => stub.stop());
  const client = new GxClient({ baseUrl: stub.baseUrl, token: "irrelevant" });
  const page = await client.listEscalations();
  console.log(`LIST_ESCALATIONS_RESULT=${JSON.stringify(page)}`);
  assert.equal(page._seen, "GET /v1/escalations", "listEscalations() with no args must not append a query string");
  assert.equal(page.items[0].ticket_id, "gx1:ticket");
});

test("listEscalations: limit/cursor become 44 §2.7's query string", async (t) => {
  const stub = await startStub((req, res) => stubPage(req, res, { transformation: "gx1:tid" }));
  t.after(() => stub.stop());
  const client = new GxClient({ baseUrl: stub.baseUrl, token: "irrelevant" });
  const page = await client.listEscalations(25, "opaque-cursor-2");
  assert.equal(page._seen, "GET /v1/escalations?limit=25&cursor=opaque-cursor-2");
});

test("listTransformations: bare GET /v1/transformations, and the audit-row shape round-trips", async (t) => {
  const stub = await startStub((req, res) =>
    stubPage(req, res, {
      transformation: "gx1:tid",
      state: "Committed",
      verdict: "Admit",
      enforced: true,
      created_at: null,
      actor: null,
      scope: null,
      superseded_by: null,
      inverse_status: "Available",
      rollback: null,
    }),
  );
  t.after(() => stub.stop());
  const client = new GxClient({ baseUrl: stub.baseUrl, token: "irrelevant" });
  const page = await client.listTransformations();
  console.log(`LIST_TRANSFORMATIONS_RESULT=${JSON.stringify(page)}`);
  assert.equal(
    page._seen,
    "GET /v1/transformations",
    "listTransformations() with no args must not append a query string",
  );
  assert.equal(page.items[0].inverse_status, "Available");
});

test("listTransformations: limit/cursor become 44 §2.7's query string", async (t) => {
  const stub = await startStub((req, res) => stubPage(req, res, { transformation: "gx1:tid" }));
  t.after(() => stub.stop());
  const client = new GxClient({ baseUrl: stub.baseUrl, token: "irrelevant" });
  const page = await client.listTransformations(1, "opaque-cursor-3");
  assert.equal(page._seen, "GET /v1/transformations?limit=1&cursor=opaque-cursor-3");
});
