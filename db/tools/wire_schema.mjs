// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const SCHEMA = path.join(ROOT, 'schema', 'wire.json');
const TARGET = path.join(ROOT, 'face', 'src', 'wire.generated.ts');

const KEYWORDS = new Set([
  '$schema',
  '$id',
  '$ref',
  '$defs',
  'title',
  'description',
  'type',
  'properties',
  'required',
  'additionalProperties',
  'items',
  'enum',
  'const',
  'oneOf'
]);
const ANNOTATIONS = new Set(['$schema', '$id', 'title', 'description']);
const WRITE = process.argv.includes('--write');
const CHECK = process.argv.includes('--check');

function refuse(reason, detail) {
  process.stderr.write(`wire_schema: ${reason}\n${detail}\n`);
  process.exit(2);
}

function visit(node, where, out) {
  if (node === null || typeof node !== 'object' || Array.isArray(node)) return;
  for (const key of Object.keys(node)) {
    if (!KEYWORDS.has(key)) out.push(`${where}.${key}`);
  }
  for (const collection of ['properties', '$defs']) {
    if (node[collection] === undefined) continue;
    for (const [name, child] of Object.entries(node[collection])) {
      visit(child, `${where}.${collection}.${name}`, out);
    }
  }
  if (node.items !== undefined) visit(node.items, `${where}.items`, out);
  if (Array.isArray(node.oneOf)) node.oneOf.forEach((child, index) => visit(child, `${where}.oneOf[${index}]`, out));
}

function unsupportedKeywords(schema) {
  const out = [];
  visit(schema, '#', out);
  return out;
}

function pascal(name) {
  return name
    .split('_')
    .map((piece) => piece.charAt(0).toUpperCase() + piece.slice(1))
    .join('');
}

function refName(ref) {
  const prefix = '#/$defs/';
  if (!ref.startsWith(prefix)) refuse('REF_UNSUPPORTED', `${ref} is not a #/$defs/ reference; this generator resolves nothing else`);
  return pascal(ref.slice(prefix.length));
}

function scalar(type) {
  if (type === 'string') return 'string';
  if (type === 'integer' || type === 'number') return 'number';
  if (type === 'boolean') return 'boolean';
  if (type === 'null') return 'null';
  return null;
}

function typeOf(node, where) {
  if (node.$ref) return refName(node.$ref);
  if (node.const !== undefined) return JSON.stringify(node.const);
  if (node.enum && node.type === 'string') return node.enum.map((value) => JSON.stringify(value)).join(' | ');
  if (node.enum && node.type === 'integer') return node.enum.join(' | ');
  if (Array.isArray(node.type)) {
    const parts = node.type.map((one) => scalar(one));
    if (parts.some((part) => part === null)) refuse('TYPE_UNSUPPORTED', `${where}: ${JSON.stringify(node.type)}`);
    return parts.join(' | ');
  }
  if (node.type === 'array') {
    if (!node.items) refuse('ITEMS_ABSENT', `${where} is an array with no items; the element type cannot be generated`);
    const inner = typeOf(node.items, `${where}.items`);
    return inner.includes(' ') ? `Array<${inner}>` : `${inner}[]`;
  }
  if (node.type === 'object') refuse('INLINE_OBJECT', `${where} is an inline object; give it a name under $defs so both sides can refer to it`);
  const one = scalar(node.type);
  if (one === null) refuse('TYPE_UNSUPPORTED', `${where}: ${JSON.stringify(node.type)}`);
  return one;
}

function declaration(name, node) {
  const label = pascal(name);
  if (node.type === 'object' && node.properties) {
    const required = new Set(node.required || []);
    const lines = [`export interface ${label} {`];
    for (const [key, child] of Object.entries(node.properties)) {
      const mark = required.has(key) ? '' : '?';
      const safe = /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(key) ? key : JSON.stringify(key);
      lines.push(`  ${safe}${mark}: ${typeOf(child, `${name}.${key}`)};`);
    }
    lines.push('}');
    return lines.join('\n');
  }
  return `export type ${label} = ${typeOf(node, name)};`;
}

function constraints(node) {
  if (Array.isArray(node)) return node.map(constraints);
  if (node === null || typeof node !== 'object') return node;
  const out = {};
  for (const [key, value] of Object.entries(node)) {
    if (ANNOTATIONS.has(key)) continue;
    if (key === 'properties' || key === '$defs') {
      const inner = {};
      for (const [name, child] of Object.entries(value)) inner[name] = constraints(child);
      out[key] = inner;
      continue;
    }
    out[key] = constraints(value);
  }
  return out;
}

const VALIDATOR = `function kindOf(value: unknown): string {
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
  if (held === undefined) throw new Error(\`the schema names \${ref} and does not define it\`);
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
      bad.push(\`\${at}: \${clean} of \${branches.length} shapes accept this body, not exactly one\`);
    }
    return;
  }
  if (node["const"] !== undefined) {
    if (value !== node["const"]) bad.push(\`\${at} is \${JSON.stringify(value)}, not \${JSON.stringify(node["const"])}\`);
    return;
  }
  const wanted = node["type"];
  const found = kindOf(value);
  if (typeof wanted === "string" && !accepts(wanted, found)) {
    bad.push(\`\${at} is \${found}, not \${wanted}\`);
    return;
  }
  if (Array.isArray(wanted) && !wanted.some((one) => accepts(String(one), found))) {
    bad.push(\`\${at} is \${found}, not one of \${wanted.join(" ")}\`);
    return;
  }
  if (Array.isArray(node["enum"]) && !(node["enum"] as unknown[]).includes(value)) {
    bad.push(\`\${at} is \${JSON.stringify(value)}, not one of \${(node["enum"] as unknown[]).map((one) => JSON.stringify(one)).join(" ")}\`);
    return;
  }
  if (found === "array" && node["items"] !== undefined) {
    (value as unknown[]).forEach((item, index) => check(node["items"] as SchemaNode, item, \`\${at}[\${index}]\`, bad));
    return;
  }
  if (found !== "object") return;
  const held = value as Record<string, unknown>;
  const properties = (node["properties"] as Record<string, SchemaNode>) ?? {};
  for (const key of (node["required"] as string[]) ?? []) {
    if (!(key in held)) bad.push(\`\${at}.\${key} is absent\`);
  }
  if (node["additionalProperties"] === false) {
    for (const key of Object.keys(held)) {
      if (!(key in properties)) bad.push(\`\${at}.\${key} is not a field this schema declares\`);
    }
  }
  for (const [key, child] of Object.entries(properties)) {
    if (key in held) check(child, held[key], \`\${at}.\${key}\`, bad);
  }
}

export function errorsAgainst(profile: string, value: unknown): string[] {
  const bad: string[] = [];
  check({ $ref: \`#/$defs/\${profile}\` }, value, "body", bad);
  return bad;
}

export function shapeErrors(value: unknown): string[] {
  return errorsAgainst("page_wire", value);
}
`;

let raw;
try {
  raw = readFileSync(SCHEMA);
} catch (error) {
  refuse('SCHEMA_UNREADABLE', `${SCHEMA}: ${error.message}`);
}

let schema;
try {
  schema = JSON.parse(raw.toString('utf8'));
} catch (error) {
  refuse('SCHEMA_NOT_JSON', `${SCHEMA}: ${error.message}`);
}

const unsupported = unsupportedKeywords(schema);
if (unsupported.length > 0) {
  refuse(
    'KEYWORD_UNSUPPORTED',
    `${unsupported.join(', ')}\nthis generator and the rust conformance test implement one subset of json schema between them.\n` +
      'a keyword neither of them reads would be a constraint nobody checks, so it refuses rather than skipping it.'
  );
}

const digest = createHash('sha256').update(raw).digest('hex');
const defs = Object.entries(schema.$defs);
const body = [
  '// SPDX-License-Identifier: Apache-2.0',
  '// Copyright (c) 2026 Glovrex',
  '// Generated from schema/wire.json by tools/wire_schema.mjs. Edit the schema, not this file.',
  '',
  `export const SCHEMA_SHA256 = ${JSON.stringify(digest)};`,
  '',
  'type SchemaNode = Record<string, unknown>;',
  '',
  `export const SCHEMA: Record<string, unknown> = ${JSON.stringify(constraints(schema), null, 2)};`,
  '',
  ...defs.map(([name, node]) => declaration(name, node)),
  '',
  VALIDATOR
].join('\n');

if (CHECK) {
  let current;
  try {
    current = readFileSync(TARGET, 'utf8');
  } catch (error) {
    refuse('GENERATED_ABSENT', `${TARGET}: ${error.message}\nnothing was compared, and a comparison that did not happen is not a comparison that found no drift`);
  }
  if (current === body) {
    process.stdout.write(`wire_schema: no drift; ${defs.length} definition(s) in schema/wire.json agree with face/src/wire.generated.ts (sha256 ${digest.slice(0, 12)})\n`);
    process.exit(0);
  }
  const before = current.split('\n');
  const after = body.split('\n');
  process.stderr.write('wire_schema: DRIFT between schema/wire.json and face/src/wire.generated.ts\n');
  let shown = 0;
  for (let index = 0; index < Math.max(before.length, after.length); index += 1) {
    if (before[index] === after[index]) continue;
    if (shown >= 8) {
      process.stderr.write('  ... further differences not shown\n');
      break;
    }
    process.stderr.write(`  line ${index + 1}\n    generated: ${before[index] ?? '(absent)'}\n    schema:    ${after[index] ?? '(absent)'}\n`);
    shown += 1;
  }
  process.stderr.write('run with --write to take the schema as the source\n');
  process.exit(1);
}

if (!WRITE) {
  refuse('NOTHING_ASKED', 'wire_schema needs --write to generate face/src/wire.generated.ts, or --check to compare it against schema/wire.json');
}

writeFileSync(TARGET, body);
process.stdout.write(`wire_schema: wrote face/src/wire.generated.ts from ${defs.length} definition(s) (sha256 ${digest.slice(0, 12)})\n`);
