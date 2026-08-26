// SPDX-License-Identifier: Apache-2.0
// The row vocabulary: the engine's list-row members, projected into the names the
// row grammar reads — declared, additive, and refused anywhere the item already
// speaks for itself.
//
// Why this module exists (req/822_c6 §2, the measured gap): the engine's list rows
// carry `transformation / state / verdict / enforced / created_at / actor / scope`
// (gx-api/src/list.rs row_json(), dumped live from a bound bed 2026-08-26), and the
// five row-drawing faces share one MEMBERS map reading `id / at / actor / effect /
// verdict / path`. Of the overlap, `actor` arrived as a structure and everything
// else arrived under another name — so a real window bound to a real engine drew
// zero rows (req/822_c5 §4), with every screen honestly saying so. This table is
// the one declared translation that closes that, at the one layer every face reads
// through.
//
// Why these mappings are the engine's own reading and not this window's invention:
// M-15 (gx-api/src/list.rs, over row_json(); req/182 §1-2, 44 §2.7 v0.4-l) added
// `created_at` / `actor` / `scope` to list rows as, in the crate's own words,
// "`time / who / target` for a GUI's list". The projection below is that sentence,
// spelled in the row grammar's names.
//
// The rules, in the order they defend:
//
//  1. ADDITIVE, NEVER DESTRUCTIVE. Every wire member stays on the item at its wire
//     name, and the raw row survives whole under `wire` — the membrane "does not
//     repair a spelling, and does not drop a member" (membrane.mjs header), and this
//     module keeps that true by only ever adding.
//  2. AN ITEM THAT SPEAKS FOR ITSELF IS WORN AS STATED. A face member is added only
//     where the item does not already carry one — the same discipline held's
//     toRecord applies to `lifecycle` (req/822_c5 B1).
//  3. NO GUESSED MAPPING. Only routes whose items were read off a live engine are in
//     the translated set. A route whose rows were never observed passes through
//     untouched; a mapping for members never seen would be a guess wearing a table's
//     clothes.
//  4. THE NAME TRAP IS REFUSED BY NAME (DR-44-9, req/38 §168 ruling 1 ⑤): the target
//     column a person reads is `Fingerprint.scope`. `gx_core::Transformation` also
//     has a field spelled `target` — the expected post-state digest — and it is NOT
//     this column. `path` maps from `scope`, and from nothing else, ever.

/**
 * The declared mapping, one row per projected member. `wire` names what the engine
 * sends (source cited per row); `face` names what the shared row grammar reads.
 */
export const ROW_VOCABULARY = Object.freeze([
  {
    face: 'id',
    wire: 'transformation',
    source: 'gx-api/src/list.rs row_json(): "transformation": id.0.to_text()',
    note: 'the identity the act routes take: the commit and cancel methods address a row by this value',
  },
  {
    face: 'at',
    wire: 'created_at',
    source: 'gx-api/src/list.rs row_json() / M-15 "time"',
    note: 'RFC3339; null when the row is not on the table (M5H3-5), and null is not projected',
  },
  {
    face: 'actor',
    wire: 'actor',
    source: 'gx-core/src/context.rs Actor (externally tagged) / M-15 "who"',
    note: 'flattened to "Variant:key" ("Agent" appends " (model)"); the raw structure stays at wire.actor',
  },
  {
    face: 'path',
    wire: 'scope',
    source: 'gx-api/src/list.rs row_json(): Fingerprint.scope / M-15 "target" / DR-44-9',
    note: 'never Transformation.target — that is a post-state digest, not the column a person reads',
  },
]);

/** The routes whose item members were dumped from a live engine (req/822_c6 §1). */
export const TRANSLATED_ROUTES = Object.freeze(['get_candidates', 'get_transformations']);

/**
 * The scalar drawn form of a gx-core Actor: `Variant:key`, with the model appended
 * for an Agent because the crate names it as "the one fact about an agent a human
 * reviewer needs and cannot recover from the key" (context.rs).
 *
 * Anything that is not an externally-tagged single-variant object carrying a string
 * `key` answers null — the caller keeps the structure, and the face draws its
 * declared MEMBER_NOT_SCALAR hole over it rather than this module guessing who
 * acted.
 */
export function actorWord(actor) {
  if (actor === null || typeof actor !== 'object' || Array.isArray(actor)) return null;
  const variants = Object.keys(actor);
  if (variants.length !== 1) return null;
  const [variant] = variants;
  const body = actor[variant];
  if (body === null || typeof body !== 'object' || typeof body.key !== 'string') return null;
  const word = `${variant}:${body.key}`;
  return typeof body.model === 'string' && body.model !== '' ? `${word} (${body.model})` : word;
}

/** One item, projected. Additive only; the raw row is preserved whole under `wire`. */
export function translateItem(item) {
  if (item === null || typeof item !== 'object' || Array.isArray(item)) return item;
  const out = { ...item };
  if (!('wire' in item)) out.wire = item;
  for (const { face, wire } of ROW_VOCABULARY) {
    if (face in item) continue; // rule 2: the item's own word wins
    if (face === 'actor') continue; // same-name member; handled below
    const value = item[wire];
    if (typeof value === 'string' && value !== '') out[face] = value;
  }
  // `actor` shares its name across the two vocabularies, so "already carries one"
  // cannot mean presence — it means the item's actor is already the scalar the row
  // grammar reads. A structure is flattened where the flatten rule holds and left
  // standing where it does not.
  if ('actor' in item && typeof item.actor === 'object') {
    const word = actorWord(item.actor);
    if (word !== null) out.actor = word;
  }
  return out;
}

/** A folded list envelope, translated when its route is in the observed set. */
export function translateFold(name, envelope) {
  if (!TRANSLATED_ROUTES.includes(name)) return envelope;
  if (!envelope || !Array.isArray(envelope.items)) return envelope;
  return { ...envelope, items: envelope.items.map(translateItem), vocabulary: 'row' };
}
