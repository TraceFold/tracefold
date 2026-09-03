// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
// Generated from schema/wire.json by tools/wire_schema.mjs. Edit the schema, not this file.

export const SCHEMA_SHA256 = "b8777f95aa4f46a3aa0a2aae527952b79213200023bb9eb7a25471d6d79c3b4c";

type SchemaNode = Record<string, unknown>;

export const SCHEMA: Record<string, unknown> = {
  "oneOf": [
    {
      "$ref": "#/$defs/page_wire"
    },
    {
      "$ref": "#/$defs/bands_wire"
    },
    {
      "$ref": "#/$defs/gate_wire"
    },
    {
      "$ref": "#/$defs/transport_refusal"
    }
  ],
  "$defs": {
    "verdict": {
      "type": "string",
      "enum": [
        "TRUE",
        "FALSE",
        "UNKNOWN"
      ]
    },
    "exit": {
      "type": "integer",
      "enum": [
        0,
        1,
        2
      ]
    },
    "source_ref": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "path",
        "anchor",
        "start_line",
        "end_line",
        "byte_start",
        "byte_end"
      ],
      "properties": {
        "path": {
          "type": "string"
        },
        "anchor": {
          "type": "string"
        },
        "start_line": {
          "type": "integer"
        },
        "end_line": {
          "type": "integer"
        },
        "byte_start": {
          "type": "integer"
        },
        "byte_end": {
          "type": "integer"
        }
      }
    },
    "relation": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "type",
        "dst"
      ],
      "properties": {
        "type": {
          "type": "string"
        },
        "dst": {
          "type": "string"
        }
      }
    },
    "atom_row": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "id",
        "band",
        "document",
        "layer",
        "kind",
        "role",
        "executor",
        "evidence",
        "line"
      ],
      "properties": {
        "id": {
          "type": "string"
        },
        "band": {
          "type": "string"
        },
        "document": {
          "type": "string"
        },
        "layer": {
          "type": "string"
        },
        "kind": {
          "type": "string"
        },
        "role": {
          "type": "string"
        },
        "executor": {
          "type": "string"
        },
        "evidence": {
          "type": "string"
        },
        "line": {
          "type": "string"
        },
        "score": {
          "type": "number"
        },
        "content": {
          "type": "string"
        },
        "provenance": {
          "$ref": "#/$defs/source_ref"
        },
        "relations": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/relation"
          }
        }
      }
    },
    "band_row": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "id",
        "title",
        "abstract",
        "documents",
        "atoms",
        "gaps"
      ],
      "properties": {
        "id": {
          "type": "string"
        },
        "title": {
          "type": "string"
        },
        "abstract": {
          "type": "string"
        },
        "documents": {
          "type": "integer"
        },
        "atoms": {
          "type": "integer"
        },
        "gaps": {
          "type": "integer"
        }
      }
    },
    "breakdown_row": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "attribute",
        "document",
        "count"
      ],
      "properties": {
        "attribute": {
          "type": "string"
        },
        "document": {
          "type": "string"
        },
        "count": {
          "type": "integer"
        }
      }
    },
    "gate_row": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "name",
        "verdict",
        "reason",
        "count",
        "denominator",
        "detail",
        "breakdown"
      ],
      "properties": {
        "name": {
          "type": "string"
        },
        "verdict": {
          "type": "string",
          "enum": [
            "pass",
            "fail",
            "unknown"
          ]
        },
        "reason": {
          "type": "string"
        },
        "count": {
          "type": "integer"
        },
        "denominator": {
          "type": "integer"
        },
        "detail": {
          "type": "string"
        },
        "breakdown": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/breakdown_row"
          }
        }
      }
    },
    "page_denominator": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "total",
        "matched",
        "returned",
        "withheld",
        "unscanned",
        "gaps_excluded"
      ],
      "properties": {
        "total": {
          "type": [
            "integer",
            "null"
          ]
        },
        "matched": {
          "type": "integer"
        },
        "returned": {
          "type": "integer"
        },
        "withheld": {
          "type": "integer"
        },
        "unscanned": {
          "type": "integer"
        },
        "gaps_excluded": {
          "type": "integer"
        }
      }
    },
    "plain_denominator": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "matched",
        "returned",
        "withheld",
        "unscanned"
      ],
      "properties": {
        "matched": {
          "type": "integer"
        },
        "returned": {
          "type": "integer"
        },
        "withheld": {
          "type": "integer"
        },
        "unscanned": {
          "type": "integer"
        }
      }
    },
    "cap": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "rows",
        "budget_tokens",
        "bytes_returned"
      ],
      "properties": {
        "rows": {
          "type": "integer"
        },
        "budget_tokens": {
          "type": "integer"
        },
        "bytes_returned": {
          "type": "integer"
        }
      }
    },
    "page_query": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "cmd",
        "band",
        "layer",
        "lod",
        "cursor"
      ],
      "properties": {
        "cmd": {
          "type": "string",
          "enum": [
            "ls",
            "show",
            "find",
            "unknown"
          ]
        },
        "band": {
          "type": [
            "string",
            "null"
          ]
        },
        "layer": {
          "type": [
            "string",
            "null"
          ]
        },
        "lod": {
          "type": "integer"
        },
        "cursor": {
          "type": [
            "string",
            "null"
          ]
        }
      }
    },
    "bands_query": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "cmd",
        "band",
        "layer",
        "lod",
        "cursor"
      ],
      "properties": {
        "cmd": {
          "const": "bands"
        },
        "band": {
          "type": "null"
        },
        "layer": {
          "type": "null"
        },
        "lod": {
          "type": "null"
        },
        "cursor": {
          "type": "null"
        }
      }
    },
    "gate_query": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "cmd",
        "band",
        "layer",
        "lod",
        "cursor"
      ],
      "properties": {
        "cmd": {
          "const": "gate"
        },
        "band": {
          "type": "null"
        },
        "layer": {
          "type": "null"
        },
        "lod": {
          "type": "null"
        },
        "cursor": {
          "type": "null"
        }
      }
    },
    "page_wire": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "schema",
        "verdict",
        "reason",
        "exit",
        "query",
        "cap",
        "denominator",
        "rows",
        "note"
      ],
      "properties": {
        "schema": {
          "const": 1
        },
        "verdict": {
          "$ref": "#/$defs/verdict"
        },
        "reason": {
          "type": "string"
        },
        "exit": {
          "$ref": "#/$defs/exit"
        },
        "query": {
          "$ref": "#/$defs/page_query"
        },
        "cap": {
          "$ref": "#/$defs/cap"
        },
        "denominator": {
          "$ref": "#/$defs/page_denominator"
        },
        "rows": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/atom_row"
          }
        },
        "note": {
          "type": "string"
        }
      }
    },
    "bands_wire": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "schema",
        "verdict",
        "reason",
        "exit",
        "query",
        "cap",
        "denominator",
        "rows",
        "note"
      ],
      "properties": {
        "schema": {
          "const": 1
        },
        "verdict": {
          "$ref": "#/$defs/verdict"
        },
        "reason": {
          "type": "string"
        },
        "exit": {
          "$ref": "#/$defs/exit"
        },
        "query": {
          "$ref": "#/$defs/bands_query"
        },
        "cap": {
          "type": "null"
        },
        "denominator": {
          "$ref": "#/$defs/page_denominator"
        },
        "rows": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/band_row"
          }
        },
        "note": {
          "type": "string"
        }
      }
    },
    "gate_wire": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "schema",
        "verdict",
        "reason",
        "exit",
        "query",
        "cap",
        "denominator",
        "rows",
        "note"
      ],
      "properties": {
        "schema": {
          "const": 1
        },
        "verdict": {
          "$ref": "#/$defs/verdict"
        },
        "reason": {
          "type": "string"
        },
        "exit": {
          "$ref": "#/$defs/exit"
        },
        "query": {
          "$ref": "#/$defs/gate_query"
        },
        "cap": {
          "type": "null"
        },
        "denominator": {
          "$ref": "#/$defs/plain_denominator"
        },
        "rows": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/gate_row"
          }
        },
        "note": {
          "type": "string"
        }
      }
    },
    "transport_refusal": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "schema",
        "verdict",
        "reason",
        "exit",
        "query",
        "cap",
        "denominator",
        "rows",
        "note"
      ],
      "properties": {
        "schema": {
          "const": 1
        },
        "verdict": {
          "$ref": "#/$defs/verdict"
        },
        "reason": {
          "type": "string"
        },
        "exit": {
          "$ref": "#/$defs/exit"
        },
        "query": {
          "type": "null"
        },
        "cap": {
          "type": "null"
        },
        "denominator": {
          "$ref": "#/$defs/plain_denominator"
        },
        "rows": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/atom_row"
          }
        },
        "note": {
          "type": "string"
        }
      }
    }
  }
};

export type Verdict = "TRUE" | "FALSE" | "UNKNOWN";
export type Exit = 0 | 1 | 2;
export interface SourceRef {
  path: string;
  anchor: string;
  start_line: number;
  end_line: number;
  byte_start: number;
  byte_end: number;
}
export interface Relation {
  type: string;
  dst: string;
}
export interface AtomRow {
  id: string;
  band: string;
  document: string;
  layer: string;
  kind: string;
  role: string;
  executor: string;
  evidence: string;
  line: string;
  score?: number;
  content?: string;
  provenance?: SourceRef;
  relations?: Relation[];
}
export interface BandRow {
  id: string;
  title: string;
  abstract: string;
  documents: number;
  atoms: number;
  gaps: number;
}
export interface BreakdownRow {
  attribute: string;
  document: string;
  count: number;
}
export interface GateRow {
  name: string;
  verdict: "pass" | "fail" | "unknown";
  reason: string;
  count: number;
  denominator: number;
  detail: string;
  breakdown: BreakdownRow[];
}
export interface PageDenominator {
  total: number | null;
  matched: number;
  returned: number;
  withheld: number;
  unscanned: number;
  gaps_excluded: number;
}
export interface PlainDenominator {
  matched: number;
  returned: number;
  withheld: number;
  unscanned: number;
}
export interface Cap {
  rows: number;
  budget_tokens: number;
  bytes_returned: number;
}
export interface PageQuery {
  cmd: "ls" | "show" | "find" | "unknown";
  band: string | null;
  layer: string | null;
  lod: number;
  cursor: string | null;
}
export interface BandsQuery {
  cmd: "bands";
  band: null;
  layer: null;
  lod: null;
  cursor: null;
}
export interface GateQuery {
  cmd: "gate";
  band: null;
  layer: null;
  lod: null;
  cursor: null;
}
export interface PageWire {
  schema: 1;
  verdict: Verdict;
  reason: string;
  exit: Exit;
  query: PageQuery;
  cap: Cap;
  denominator: PageDenominator;
  rows: AtomRow[];
  note: string;
}
export interface BandsWire {
  schema: 1;
  verdict: Verdict;
  reason: string;
  exit: Exit;
  query: BandsQuery;
  cap: null;
  denominator: PageDenominator;
  rows: BandRow[];
  note: string;
}
export interface GateWire {
  schema: 1;
  verdict: Verdict;
  reason: string;
  exit: Exit;
  query: GateQuery;
  cap: null;
  denominator: PlainDenominator;
  rows: GateRow[];
  note: string;
}
export interface TransportRefusal {
  schema: 1;
  verdict: Verdict;
  reason: string;
  exit: Exit;
  query: null;
  cap: null;
  denominator: PlainDenominator;
  rows: AtomRow[];
  note: string;
}

function kindOf(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  if (typeof value === "number") return Number.isInteger(value) ? "integer" : "number";
  return typeof value;
}

function accepts(wanted: string, found: string): boolean {
  if (wanted === found) return true;
  return wanted === "number" && found === "integer";
}

function resolve(ref: string): SchemaNode {
  const defs = SCHEMA["$defs"] as Record<string, SchemaNode>;
  const held = defs[ref.slice("#/$defs/".length)];
  if (held === undefined) throw new Error(`the schema names ${ref} and does not define it`);
  return held;
}

function check(node: SchemaNode, value: unknown, at: string, bad: string[]): void {
  if (typeof node["$ref"] === "string") {
    check(resolve(node["$ref"] as string), value, at, bad);
    return;
  }
  if (Array.isArray(node["oneOf"])) {
    const branches = node["oneOf"] as SchemaNode[];
    const held: string[][] = branches.map((branch) => {
      const errors: string[] = [];
      check(branch, value, at, errors);
      return errors;
    });
    const clean = held.filter((errors) => errors.length === 0).length;
    if (clean !== 1) {
      bad.push(`${at}: ${clean} of ${branches.length} shapes accept this body, not exactly one`);
    }
    return;
  }
  if (node["const"] !== undefined) {
    if (value !== node["const"]) bad.push(`${at} is ${JSON.stringify(value)}, not ${JSON.stringify(node["const"])}`);
    return;
  }
  const wanted = node["type"];
  const found = kindOf(value);
  if (typeof wanted === "string" && !accepts(wanted, found)) {
    bad.push(`${at} is ${found}, not ${wanted}`);
    return;
  }
  if (Array.isArray(wanted) && !wanted.some((one) => accepts(String(one), found))) {
    bad.push(`${at} is ${found}, not one of ${wanted.join(" ")}`);
    return;
  }
  if (Array.isArray(node["enum"]) && !(node["enum"] as unknown[]).includes(value)) {
    bad.push(`${at} is ${JSON.stringify(value)}, not one of ${(node["enum"] as unknown[]).map((one) => JSON.stringify(one)).join(" ")}`);
    return;
  }
  if (found === "array" && node["items"] !== undefined) {
    (value as unknown[]).forEach((item, index) => check(node["items"] as SchemaNode, item, `${at}[${index}]`, bad));
    return;
  }
  if (found !== "object") return;
  const held = value as Record<string, unknown>;
  const properties = (node["properties"] as Record<string, SchemaNode>) ?? {};
  for (const key of (node["required"] as string[]) ?? []) {
    if (!(key in held)) bad.push(`${at}.${key} is absent`);
  }
  if (node["additionalProperties"] === false) {
    for (const key of Object.keys(held)) {
      if (!(key in properties)) bad.push(`${at}.${key} is not a field this schema declares`);
    }
  }
  for (const [key, child] of Object.entries(properties)) {
    if (key in held) check(child, held[key], `${at}.${key}`, bad);
  }
}

export function errorsAgainst(profile: string, value: unknown): string[] {
  const bad: string[] = [];
  check({ $ref: `#/$defs/${profile}` }, value, "body", bad);
  return bad;
}

export function shapeErrors(value: unknown): string[] {
  return errorsAgainst("page_wire", value);
}
