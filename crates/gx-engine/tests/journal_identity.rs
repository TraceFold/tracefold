// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **I-1** applied to the one thing this hand canonically encodes: [`EngineJournalRecord`].
//!
//! req/78 §6.0-5: "place an I-1-shaped defense in the same turn as any new IdentityView
//! projection … A-10-shaped (an assert on the canonical encode's map key count) plus "two values
//! differing in only one field have different digests" for every field," and
//! §6.2 hand 1 ⑤ makes it conditional on there being an encode at all — "if
//! `EngineJournalRecord` gets a canonical encode, an I-1-shaped defense goes in the same turn"
//! (sem: SEM-gx-engine-748). There is one: a write-ahead journal is
//! bytes on a device, and 41 §6 allows exactly one encoder for them.
//!
//! # What is being defended, and against what
//!
//! A journal record is not a CID'd object — 42 gives it no identity view and nothing in the system
//! quotes a record's digest. So the failure this suite exists to catch is not a collision between
//! two objects; it is a **field that never reaches the bytes**. A record whose `at` is dropped by
//! the encoder writes a journal in which two events an hour apart are the same line, and a recovery
//! reading it back cannot tell them apart. The two questions are the same two A-10 asks — how many
//! keys does the encoder declare, and does each field move the digest — and the second is the one
//! that catches the B-3 shape (every key present, one of them filled with a constant).
//!
//! # The shape of an encoded record
//!
//! serde writes a struct variant as a **one-key map** whose single value is the map of fields
//! (`{ "Planned": { "at": .., "delta_cid": .., .. } }`). So A-10 splits in two here: the outer map
//! declares one key and it is the variant name, and the inner map declares as many keys as the
//! variant has fields. Both counts are read from the head byte — the encoder's own statement of how
//! many pairs follow (42 §2.1, RFC 8949 §3) — and then from a decode that names them, which is
//! `gx-gate/tests/verdict_identity.rs`'s helper and its reason: a test that only counted would pass
//! on a struct that swapped one field for another.

mod support;

use std::collections::BTreeMap;

use gx_canon::{cbor, cid};
use gx_core::{Cid, Fingerprint, SubstrateKind};
use gx_engine::store::FingerprintRecord;
use gx_engine::EngineJournalRecord;
use support::{cid as cid_of, every_variant, one_field_changed};

/// The bytes the journal writes for a record. Through `gx_canon::cbor::encode`, which is the point
/// rather than a convenience: a record that could be encoded some other way would not be the thing
/// on the device.
fn bytes_of(record: &EngineJournalRecord) -> Vec<u8> {
    cbor::encode(record).expect("a journal record must have a canonical form")
}

/// The digest of those bytes.
///
/// `mint_leaf` is `BLAKE3(0x00 || canonical_dagcbor(value))` (42 §3.11). The domain byte is
/// constant on both sides of every comparison below, so what these assertions measure is whether a
/// field reached the canonical form at all — which is I-1's question. The alternative,
/// `cid::compute`, needs an `IdentityView`, and a journal record has none because nothing quotes
/// its identity.
fn digest_of(record: &EngineJournalRecord) -> Cid {
    cid::mint_leaf(record).expect("a journal record must have a canonical form")
}

/// The keys of a canonical DAG-CBOR map, and the count its head byte declares.
///
/// Two readings of the same bytes on purpose (A-10). Lifted from `gx-gate/tests/verdict_identity.rs`
/// in the crate that cannot import it.
fn map_keys(bytes: &[u8]) -> (u8, Vec<String>) {
    let head = bytes[0];
    assert_eq!(
        head & 0b1110_0000,
        0b1010_0000,
        "an encoded record is a CBOR map (major type 5); head byte was {head:#04x}"
    );
    let declared = head & 0b0001_1111;
    assert!(
        declared < 24,
        "a map of {declared} or more pairs spells its count in following bytes; this helper reads \
         the short form only, which is all any record here needs"
    );
    let decoded: BTreeMap<String, serde::de::IgnoredAny> =
        cbor::decode(bytes).expect("a canonical map of text keys");
    (declared, decoded.into_keys().collect())
}

/// The inner map of a struct variant: its declared key count and its key names.
fn variant_body(bytes: &[u8]) -> (u8, Vec<String>) {
    let (outer, keys) = map_keys(bytes);
    assert_eq!(outer, 1, "a struct variant is a one-key map; got {keys:?}");
    let decoded: BTreeMap<String, BTreeMap<String, serde::de::IgnoredAny>> =
        cbor::decode(bytes).expect("a variant name mapping to a map of fields");
    let (_, fields) = decoded
        .into_iter()
        .next()
        .expect("the one-key map has its one key");
    // The head byte of the inner map, found by skipping the outer head, the key string and its
    // header. Read from the bytes rather than recomputed, so that the count is still the encoder's
    // statement and not this test's.
    let name_len = usize::from(bytes[1] & 0b0001_1111);
    let inner_head = bytes[2 + name_len];
    assert_eq!(
        inner_head & 0b1110_0000,
        0b1010_0000,
        "the body of a struct variant is a map; head byte was {inner_head:#04x}"
    );
    (inner_head & 0b0001_1111, fields.into_keys().collect())
}

/// The field names a derived `Debug` prints for a value, one indent level down.
///
/// The mirror A-10 exists to hold: `#[derive(Debug)]` is written over **every** field, and the
/// encoder's key list is a second declaration of the same list. Lifted from
/// `gx-canon/tests/intent_identity.rs`, where the shape and its soundness note are written out.
fn debug_field_names(text: &str) -> Vec<String> {
    let mut names: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("    ")?;
            if rest.starts_with(' ') {
                return None;
            }
            let (name, _) = rest.split_once(": ")?;
            // Digits allowed: `fp0` is a field name, and gx-canon's copy of this helper -- written
            // for structs whose fields happen to be alphabetic -- would silently drop it, which is
            // a mirror check that stops seeing the field it exists to check.
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                .then(|| name.to_string())
        })
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------------------
// A-10: the key counts
// ---------------------------------------------------------------------------

/// Every record encodes as a one-key map whose key is the variant name `kind()` reports.
///
/// The name is the wire form. A `kind()` that drifted from it would make a journal unreadable by
/// the code that wrote it, and nothing else in the system would notice until a recovery ran.
#[test]
fn every_record_encodes_under_the_name_its_kind_reports() {
    let mut checked = 0;
    for record in every_variant() {
        let (declared, keys) = map_keys(&bytes_of(&record));
        assert_eq!(declared, 1, "one key: {keys:?}");
        assert_eq!(
            keys,
            vec![record.kind().to_string()],
            "the encoded key is not `kind()`"
        );
        checked += 1;
    }
    println!("RECORDS_NAMED_ON_THE_WIRE={checked}");
    assert_eq!(checked, 15, "fifteen variants");
}

/// The declared key count of each variant's body is its field count, and the names are its fields.
///
/// The arithmetic is printed per variant so that a reader sees the shape of the whole enum in one
/// place, which is what makes "43 fields over thirteen records" (sem: SEM-gx-engine-749) a
/// statement about the type rather
/// than a literal somebody can update in one place.
#[test]
fn every_variant_declares_one_key_per_field() {
    let mut total_fields = 0;
    for record in every_variant() {
        let bytes = bytes_of(&record);
        let (declared, keys) = variant_body(&bytes);
        assert_eq!(
            usize::from(declared),
            keys.len(),
            "{}: head byte declares {declared} pairs, decode names {}",
            record.kind(),
            keys.len()
        );
        let debug = debug_field_names(&format!("{record:#?}"));
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(
            sorted,
            debug,
            "{}: the encoder's keys are not the struct's fields",
            record.kind()
        );
        println!("VARIANT_KEYS {} = {} {keys:?}", record.kind(), keys.len());
        total_fields += keys.len();
    }
    println!("JOURNAL_FIELDS_TOTAL={total_fields}");
    assert_eq!(
        total_fields, 60,
        "the fifteen variants carry sixty fields between them (**DR-46-45** added \
         `Planned.undo_witness`, `req/973` §B-1 -- the compare-and-swap answer the undo road \
         reached, journalled because `Engine::undo` builds no receipt and 43 §7-3b's rebuild \
         cannot re-derive a comparison made against a live world; **DR-46-33** added \
         `Planned.input_generation`, `req/38` §413 -- the input-generation stage joined at plan \
         time and journalled as its result so 43 §7-3b's rebuild reproduces the boundary without \
         the actor; **DR-46-34** added \
         `InverseEscrowed.reads_attested`, `req/38` §268 ruling 5; 43 T-4e added one in M5 \
         hand 2, hand 4 added four -- M5-25's record and AC-038's rollback outcome -- and \
         **E-M5-13** added two to `Planned` in M6 hand 1; and \
         two-phase escrow added `pending` plus the six fields of its two records, \
         req/38 §98 ruling 1, sem: SEM-gx-engine-750; and **DR-46-26** added \
         `InverseEscrowed.reads` and `InverseEscrowed.undetermined`, `req/38` §258 -- the escrow's \
         read-set, journalled because 43 §7-3b's rebuild cannot obtain it any other way, and the \
         discriminator that tells E-M5-9's `Unavailable` from DR-46-13's `Undetermined`; and \
         **DR-46-31** added `HumanDecision.verdict_digest`, `req/38` §261 ruling 2b -- the digest \
         of the ruling the receipt was issued under, without which Σ named the human's verdict \
         beside T-4c's escalation proof and no escalated commit could be re-issued)"
    );
}

/// The projection lands on the wire face rather than beside it (AC-014, 42 §2.1-6).
#[test]
fn every_record_lands_on_the_canonical_wire_face() {
    for record in every_variant() {
        assert!(
            cbor::is_canonical(&bytes_of(&record)),
            "{} does not encode canonically",
            record.kind()
        );
    }
}

// ---------------------------------------------------------------------------
// I-1: every field reaches the digest
// ---------------------------------------------------------------------------

/// 🔴 Each of the fifty-eight fields moves the record's digest.
///
/// The half a key count cannot see (the B-3 shape, req/67 §2.1: every key declared, one of them
/// filled with a constant). At least one mutant per field, each differing from its baseline in that
/// field alone, and **the coverage is asserted against the field list** rather than against a
/// number -- a field added without a mutant is a failing probe rather than a silent gap.
///
/// Fifty-eight fields, sixty-two mutants. `Verdict.verdict_digest` has two, because M5 hand 2 made it
/// an `Option` and "a different digest" and "no digest at all" (sem: SEM-gx-engine-751) are
/// different records: T-4e writes
/// the second one, and a journal that hashed it like the first would lose INV-S5's distinction.
/// 🔴 **DR-46-31** gives `HumanDecision.verdict_digest` two for the same reason, and the second
/// case is load-bearing here: "no digest at all" is a journal written before the field existed,
/// which `replay.rs` degrades to the old blocked re-issue rather than repairing.
///
/// 🔴 The three prose numbers above were stale before this lane (they read "forty-three" and
/// "forty-four" while the asserts read 56/59) and are corrected here rather than left, because a
/// doc that contradicts the assert three lines below it is where the next hand's count comes from.
#[test]
fn every_field_of_every_record_reaches_its_digest() {
    let baselines: BTreeMap<String, Cid> = every_variant()
        .into_iter()
        .map(|r| (r.kind().to_string(), digest_of(&r)))
        .collect();

    let mutants = one_field_changed();
    println!("FIELD_MUTANTS={}", mutants.len());

    // Every (variant, field) the wire declares has a mutant. `verdict_digest(None)` is the one
    // mutant whose label is not a bare field name, and it is stripped so that the coverage set is
    // about fields.
    let covered: std::collections::BTreeSet<(String, String)> = mutants
        .iter()
        .map(|(variant, field, _)| {
            (
                (*variant).to_string(),
                field.split('(').next().unwrap_or(field).to_string(),
            )
        })
        .collect();
    let mut missing: Vec<String> = Vec::new();
    let mut declared = 0usize;
    for record in every_variant() {
        let variant = record.kind().to_string();
        for key in variant_body(&bytes_of(&record)).1 {
            declared += 1;
            if !covered.contains(&(variant.clone(), key.clone())) {
                missing.push(format!("{variant}.{key}"));
            }
        }
    }
    println!("JOURNAL_FIELDS_COVERED={declared} MISSING={missing:?}");
    assert!(
        missing.is_empty(),
        "these fields reach the wire and have no mutant, so nothing says they reach the digest: \
         {missing:?}"
    );
    assert_eq!(
        declared, 60,
        "the fifteen records declare sixty fields (**DR-46-45** added `Planned.undo_witness`, \
         `req/973` §B-1; **DR-46-33** added \
         `Planned.input_generation`, `req/38` §413; **E-M5-13** added `Planned.locator` and \
         `Planned.parents` in M6 hand 1; two-phase escrow added `InverseEscrowed.pending` and the \
         `ApplyObserved`/`InverseCompleted` pair, req/38 §98 ruling 1, sem: SEM-gx-engine-752; \
         **DR-46-26** added `InverseEscrowed.reads` and `InverseEscrowed.undetermined`, \
         `req/38` §258; **DR-46-31** added `HumanDecision.verdict_digest`, `req/38` §261 \
         ruling 2b; and **DR-46-34** added `InverseEscrowed.reads_attested`, `req/38` §268 \
         ruling 5 — the flag that separates a recorded empty read-set from a journal that \
         predates the field, which is the one thing between two records `reads` alone spells \
         identically)"
    );
    assert_eq!(
        mutants.len(),
        64,
        "one mutant per field (**DR-46-45** brings its own, `Planned.undo_witness`, in the same \
         commit as the field, and its mutant is the other arm of the disposition rather than an \
         absence -- the absence is `None`, which is \"not an undo's plan\" and not a third value; \
         **DR-46-33** brings its own, `Planned.input_generation`, and \
         **DR-46-34** brings `reads_attested`, in the same \
         commit as the field), plus four absent cases: `Verdict.verdict_digest` (43 T-4e), \
         `InverseEscrowed.inverse_cid` (E-M5-9), `InverseCompleted.inverse_cid` (the \
         fold of req/38 §99 ruling 2-④, sem: SEM-gx-engine-753) and **DR-46-31**'s \
         `HumanDecision.verdict_digest`, whose absence is a pre-DR-46-31 journal and not a \
         ruling without a proof. **DR-46-26** adds two of each -- \
         each field and its mutant in the same commit, which is what `missing` above enforces"
    );

    for (variant, field, mutant) in mutants {
        let baseline = baselines
            .get(variant)
            .unwrap_or_else(|| panic!("{variant} has a baseline"));
        assert_eq!(
            mutant.kind(),
            variant,
            "the mutant for {variant}.{field} is a different variant"
        );
        assert_ne!(
            *baseline,
            digest_of(&mutant),
            "{variant}.{field} does not reach the digest -- a journal written with it would lose \
             the field"
        );
    }
}

/// Two different variants never share a digest, even carrying the same components.
///
/// `ApplyStarted` and `InverseEscrowed` are the pair this is about: both are
/// `{ transformation, <a Cid>, at }`, and an encoding that dropped the variant name would identify
/// "the inverse was escrowed" with "the adapter was asked to apply" (sem:
/// SEM-gx-engine-754). Those are the two records
/// **E-M5-1** exists to keep apart — the whole of Λ4 is that a recovery must be able to tell them
/// apart — so a probe that says so is a probe on the erratum itself.
#[test]
fn the_variant_name_is_part_of_the_digest() {
    let at = gx_core::Timestamp(500);
    let transformation = support::tid(3);
    let payload = cid_of(21);
    let escrowed = EngineJournalRecord::InverseEscrowed {
        transformation,
        inverse_cid: Some(payload),
        pending: false,
        reads: Vec::new(),
        undetermined: false,
        // 🔴 **DR-46-34** — this probe is about the variant *name* reaching the digest, so the
        // flag takes its default; `reads_attested` has its own row in `one_field_changed`.
        reads_attested: false,
        at,
    };
    let applying = EngineJournalRecord::ApplyStarted {
        transformation,
        delta_cid: payload,
        at,
    };
    assert_ne!(
        digest_of(&escrowed),
        digest_of(&applying),
        "T-10b's record and E-M5-1's record are distinguishable"
    );
}

/// Each of the three components of a written-down fingerprint reaches the record's digest.
///
/// `Planned.fp0` is one field of one variant above, so the mutant list moves it as a unit. This is
/// the same question one level down, and it is the one that matters most: 42 §3.5 lets an adapter
/// widen a scope past the object itself, so two fingerprints over the same digest and different
/// scopes are two different statements about the world (`gx-canon/tests/fingerprint_identity.rs`
/// makes the same point about `Fingerprint`'s own projection).
#[test]
fn every_component_of_a_written_fingerprint_reaches_the_digest() {
    let base = |substrate: SubstrateKind, scope: &str, digest: Cid| EngineJournalRecord::Planned {
        transformation: support::tid(1),
        intent_id: support::iid(1),
        locator: "/tmp/one".to_string(),
        delta_cid: cid_of(11),
        fp0: FingerprintRecord::of(
            &Fingerprint::new(substrate, scope.to_string(), digest)
                .expect("a short scope is inside MAX_SCOPE_BYTES"),
        ),
        parents: Vec::new(),
        input_generation: gx_core::BoundaryStage::Unknown,
        // DR-46-45 (`req/973` B-1): not an undo's plan, so no witness was compared.
        undo_witness: None,
        at: gx_core::Timestamp(101),
    };

    let baseline = digest_of(&base(SubstrateKind::Fs, "/tmp/x", cid_of(1)));
    let mutants = [
        ("substrate", base(SubstrateKind::Git, "/tmp/x", cid_of(1))),
        ("scope", base(SubstrateKind::Fs, "/tmp/y", cid_of(1))),
        ("digest", base(SubstrateKind::Fs, "/tmp/x", cid_of(2))),
    ];
    assert_eq!(mutants.len(), 3, "one mutant per component of 42 §3.5");
    for (name, mutant) in mutants {
        assert_ne!(
            baseline,
            digest_of(&mutant),
            "fp0.{name} does not reach the digest"
        );
    }
}
