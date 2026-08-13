//! T-30 / T-21 — the decode side refuses every vector that the spec says is not canonical.
//!
//! The vectors live as raw JSON in `tests/vectors/negative/`, one file per vector, readable
//! without running anything (T-31: 「生成 script に依存せず読める」). Their provenance is the
//! table `req/33_NEGVECTOR_TABLE_A_2026-08-07.md` ruled on 2026-08-07: A-1〜A-8 authored from the
//! IPLD DAG-CBOR Strictness section, A-9 taken as data from `ipld/codec-fixtures`. P-1〜P-3 are
//! outside that table and are marked as such in their files -- T-30 asks for at least one
//! violating input per rule of 42 §2.1, and rules 3, 4 and 5 have no row in req/33 because
//! req/33 was drawn from IPLD's MUST tokens and those three are places where gx is *stricter*
//! than IPLD (no floats at all, no tags at all, no indefinite lengths).
//!
//! Two things are asserted per vector, and they are not the same thing:
//!
//! 1. `cbor::decode` refuses it. That is the requirement.
//! 2. `cbor::scan_strict` refuses it on its own. That is the gx layer, an independent
//!    implementation written from the spec text, and it is checked separately so that "the
//!    dependency happened to catch it" is never mistaken for "gx checks it".
//!
//! Whether the underlying codec would also have refused is recorded and printed rather than
//! asserted, because it is a fact about `serde_ipld_dagcbor` and not a requirement on gx.
//!
//! Each vector carries a positive control (req/29 §2: 「正例の対照を必ず対で持つ」). Without one,
//! a scanner that refused everything would pass this file.
//!
//! # The six S-AI vectors, and why 「refused」 was not enough (**H6-4**)
//!
//! `req/38_ERRATA_2026-08-07.md` §16 逐語: 「🔴**H6-4（採用・fix 批へ載荷）**: additional-info 境界の
//! 判別 vector（24/25/26/27・28-30・31）を負例表へ追加する。mutation survivor 22 件は「負例 suite が
//! strict scanner の境界を区別していない」という検査の穴の具体形であり、数学的厳密の第一原則から放置
//! 不可」.
//!
//! The hole had two shapes and the six vectors close both.
//!
//! * **The boundary was never visited.** `scan_strict` decides shortest-form with four comparisons
//!   -- `v >= 24`, `v > u8::MAX`, `v > u16::MAX`, `v > u32::MAX` -- and req/33's A-2 and A-3 use the
//!   values 1 and 1, which are nowhere near any of them. Swapping a `>` for a `>=` left every vector
//!   in the table passing. `S-AI24`..`S-AI27` each sit on one side of one comparison and each carry a
//!   control on the other side, so a change in either direction moves a vector across it.
//! * **「it was refused」 did not say *why*.** `simple_or_float` answers major type 7 with five arms,
//!   four of which are refusals; deleting any one of them lets its inputs fall to the fifth, which
//!   also refuses. A suite that only asks `is_err()` cannot see the difference between 「the float
//!   rule caught it」 and 「the catch-all caught it」. So a vector may declare `expected_error`, and
//!   `negative_the_declared_error_is_the_one_the_scanner_returns` asserts the variant rather than the
//!   refusal. The twelve older vectors do not declare one and are unchanged; the field is optional
//!   and its absence is not a failure (extending it to them is raised in req/57 §4).
//!
//! # M3 hand 4: the payload, and the two vectors that reach it (**F-3**, **A-8**, **A-9**)
//!
//! Until this hand the sentence above read 「the survivors that live in the error *payload*
//! arithmetic -- `missing: end - self.b.len()`, `extra: bytes.len() - scan.i` -- are still open,
//! because no assertion here reads those numbers」. req/38 §17 shelved that as F-3 and §18 shelved
//! A-8 and A-9 beside it, all three into M3's error-vocabulary window, 「error の値をどこまで検査
//! するか」 being one design decision rather than three.
//!
//! The decision taken is to **widen `expected_error` rather than build a second suite**: a vector
//! may now also declare `expected_error_payload`, a table of the numeric fields the refusal must
//! carry. The reason is that those numbers are facts about the byte string -- 「eight bytes are
//! missing at offset one」 -- and a separate suite would have to restate the byte string to assert
//! them, which is two places to keep one input.
//!
//! It is a **subset** check: a vector declares the fields it means, and a field it does not name is
//! not asserted. A vector that had to name every field would make adding a field to an `Error`
//! variant a change to every vector that mentions it.
//!
//! Three vectors changed and two are new:
//!
//! * **A-8** (trailing bytes) declares `TrailingBytes { at: 1, extra: 1 }`. `extra` is
//!   `bytes.len() - scan.i` = 2 - 1; the two survivors turn that `-` into `+` (3) and `/` (2).
//! * **TR-1** is new, and exists for `missing`. No vector reached `Truncated` at all before it.
//!   Its declared `missing` is 8, against 14 for `+` and 3 for `/` -- a short truncation would not
//!   have separated them, since integer division of 3 by 2 is the correct answer 1.
//! * **D-65K** is new and is A-9's, whose recipe req/58 §4 wrote out byte for byte: 64 nested maps
//!   and a 65th whose key is a broken text-string header. The depth ceiling is reached while
//!   reading the *key*, so this is the one vector that separates `map`'s key-side `depth + 1` from
//!   its value-side one; with the addition gone the key is read and the answer becomes `Truncated`.
//! * **P-1** and **A-7** gain `expected_error: FloatNotAllowed`, which is A-8's other half
//!   (req/38 §18: 「旧 vector の宣言拡張(P-1/A-7)」). Neither kills a survivor of its own -- A-6
//!   already declares that arm -- and that is reported rather than implied: what they add is that a
//!   vector stating 「NaN is refused」 says *as what*, so a future arm that swallowed them would be
//!   visible here.
//!
//! # Hand 7: the three declarations F-1 and F-2 add (`req/38_ERRATA_2026-08-07.md` §17)
//!
//! * **F-1** 逐語: 「旧 12 vector のうち P-3 と A-6/P-1 に `expected_error` を 2 行宣言する（機構は既設・
//!   arm 削除の survivor 2 件が落ちる）」. `P-3` declares `IndefiniteLength` and `A-6` declares
//!   `FloatNotAllowed`. Both are arms that a catch-all sits behind: deleting `head`'s `31 =>` arm
//!   leaves `P-3` refused as `ReservedAdditionalInfo`, and deleting `simple_or_float`'s `25..=27 =>`
//!   arm leaves `A-6` refused as `SimpleValueNotAllowed`. Two files gained one line each; no bytes,
//!   no controls and no statements moved. `P-1` and `A-7` still declare nothing -- the ruling says
//!   two lines and this hand does not widen its own scope (req/58 §4 carries the observation).
//! * **I-5** (`req/38_ERRATA_2026-08-07.md` §26, the M3 fix hand): `TR-1` declared
//!   `normative_basis: "IPLD-MUST"`, which was the **mirror reading** of L66 rather than a verbatim
//!   one -- req/67 §2.6 measured that the IPLD spec names no MUST about truncation at all, and that
//!   L66 is a sentence about *extraneous* bytes. The sentence about *missing* ones is RFC 8949 §3's:
//!   「If the encoded sequence of bytes ends before the end of a data item, that item is not
//!   well-formed」 (L487), with 「A decoder MUST NOT return a decoded data item when it encounters
//!   input that is not a well-formed encoded CBOR data item」 (L445-446) supplying the strength. So
//!   the vocabulary below gains a fourth value, `RFC8949-MUST`, and `TR-1` is the one vector that
//!   carries it. The three req/33 ruled remain exactly as they were: the addition names a second
//!   normative document, not a fourth degree of strictness.
//! * **F-2** 逐語: 「64 段超の入れ子 vector 1 本を負例表へ追加（深さ制限 survivor 5 件が落ちる）。
//!   `normative_basis` は **gx-policy**（41 §6 panic=bug 系譜・42 §2.1 の条項でない事を vector 注記に
//!   明記）」. `D-65` is that vector, and it is the first one here whose input is **canonical IPLD
//!   DAG-CBOR**: nothing in 42 §2.1 refuses it, and gx does, because a recursive scanner walking a
//!   stranger's bytes needs a ceiling. Its control sits at exactly 64 levels and must be accepted --
//!   without it the suite could not tell `depth > MAX_DEPTH` from `depth >= MAX_DEPTH`, since both
//!   refuse the negative.

mod support;

use gx_canon::cbor;
use gx_canon::Error;
use ipld_core::ipld::Ipld;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use support::unhex;

/// req/33 ruled 8+1; P-1〜P-3 were hand 3's addition for T-30's per-rule coverage; S-AI24..S-AI31 are
/// the fix hand's six for H6-4's additional-info boundaries; `D-65` is hand 7's one for F-2; `D-65K`
/// and `TR-1` are M3 hand 4's for A-9 and F-3. A literal count, so adding a vector without
/// accounting for it here fails.
const EXPECTED_VECTOR_COUNT: usize = 21;

struct Vector {
    id: String,
    statement: String,
    bytes: Vec<u8>,
    normative_basis: String,
    /// The variant `scan_strict` must return, when the vector names one (H6-4). `None` for the ten
    /// vectors that still state a refusal without naming it.
    expected_error: Option<String>,
    /// The numeric fields that refusal must carry, when the vector names any (**F-3**). A subset:
    /// a field the vector does not name is not asserted.
    expected_error_payload: BTreeMap<String, u64>,
    control: Option<(Vec<u8>, bool)>, // (bytes, expected_accept)
}

/// The variant name a vector file may declare, for the one assertion that reads it.
///
/// Written out rather than taken from `Debug`, which prints the fields too, and rather than from
/// `Display`, which prints the byte offsets a vector cannot know in advance. `Error` is
/// `#[non_exhaustive]`, so the catch-all is required by the type and is not a shortcut: a variant
/// added later shows up as `other`, and a vector declaring a name that no longer exists fails.
fn error_name(e: &Error) -> &'static str {
    match e {
        Error::Encode(_) => "Encode",
        Error::Decode(_) => "Decode",
        Error::NotCanonicalizable(_) => "NotCanonicalizable",
        Error::Truncated { .. } => "Truncated",
        Error::Empty => "Empty",
        Error::NonMinimal { .. } => "NonMinimal",
        Error::IndefiniteLength { .. } => "IndefiniteLength",
        Error::ReservedAdditionalInfo { .. } => "ReservedAdditionalInfo",
        Error::TagNotAllowed { .. } => "TagNotAllowed",
        Error::FloatNotAllowed { .. } => "FloatNotAllowed",
        Error::SimpleValueNotAllowed { .. } => "SimpleValueNotAllowed",
        Error::NonTextMapKey { .. } => "NonTextMapKey",
        Error::UnsortedOrDuplicateMapKey { .. } => "UnsortedOrDuplicateMapKey",
        Error::InvalidUtf8 { .. } => "InvalidUtf8",
        Error::TrailingBytes { .. } => "TrailingBytes",
        Error::TooDeep { .. } => "TooDeep",
        Error::CidText { .. } => "CidText",
        Error::Jcs { .. } => "Jcs",
        _ => "other",
    }
}

fn vectors_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/negative")
}

/// A vector that cannot be read is a setup failure, not a test result. req/29 §4 forbids
/// reporting the two as the same thing and forbids letting an empty suite pass, so every step
/// here panics rather than skipping.
fn load() -> Vec<Vector> {
    let dir = vectors_dir();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("vector directory {} unreadable: {e}", dir.display()))
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();

    let mut out = Vec::new();
    for p in paths {
        let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let v: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let s = |k: &str| -> String {
            v.get(k)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("{}: missing string field {k}", p.display()))
                .to_string()
        };
        assert_eq!(
            s("expected"),
            "REJECT",
            "{}: negative vectors reject",
            p.display()
        );
        assert!(
            matches!(
                s("normative_basis").as_str(),
                "IPLD-MUST" | "IPLD-SHOULD+gx-policy" | "gx-policy" | "RFC8949-MUST"
            ),
            "{}: normative_basis must be one of the three values ruled in req/33 or RFC8949-MUST \
             (I-5, req/38 §26)",
            p.display()
        );
        assert!(
            !s("origin").is_empty() && !s("spec_anchor").is_empty(),
            "{}",
            p.display()
        );
        let control = v.get("control").map(|c| {
            let hexv = c["vector"].as_str().expect("control.vector").to_string();
            let expected_accept = c["expected"].as_str().expect("control.expected") == "ACCEPT";
            (unhex(&hexv), expected_accept)
        });
        assert!(
            control.is_some() || v.get("control_note").is_some(),
            "{}: a vector with no positive control must say why (req/29 §2)",
            p.display()
        );
        out.push(Vector {
            id: s("id"),
            statement: s("statement"),
            bytes: unhex(&s("vector")),
            normative_basis: s("normative_basis"),
            expected_error: v.get("expected_error").map(|e| {
                e.as_str()
                    .unwrap_or_else(|| panic!("{}: expected_error is a string", p.display()))
                    .to_string()
            }),
            expected_error_payload: v
                .get("expected_error_payload")
                .map(|payload| {
                    assert!(
                        v.get("expected_error").is_some(),
                        "{}: a payload without a variant names fields of nothing",
                        p.display()
                    );
                    payload
                        .as_object()
                        .unwrap_or_else(|| {
                            panic!("{}: expected_error_payload is an object", p.display())
                        })
                        .iter()
                        .map(|(field, value)| {
                            (
                                field.clone(),
                                value.as_u64().unwrap_or_else(|| {
                                    panic!(
                                        "{}: expected_error_payload.{field} is a number",
                                        p.display()
                                    )
                                }),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            control,
        });
    }
    assert_eq!(
        out.len(),
        EXPECTED_VECTOR_COUNT,
        "vector count changed; update EXPECTED_VECTOR_COUNT and the report"
    );
    out
}

#[test]
fn negative_every_vector_is_rejected_by_decode() {
    let vectors = load();
    let mut accepted = Vec::new();
    for v in &vectors {
        if cbor::decode::<Ipld>(&v.bytes).is_ok() {
            accepted.push(format!("{} ({})", v.id, v.statement));
        }
    }
    assert!(
        accepted.is_empty(),
        "decode accepted invalid vectors: {accepted:#?}"
    );
}

/// The gx layer on its own. `scan_strict` is written from the spec text (req/33, RFC 8949 §3)
/// and does not consult the codec, so this is the assertion that survives swapping the
/// dependency out.
#[test]
fn negative_every_vector_is_rejected_by_the_gx_scanner_alone() {
    let vectors = load();
    let mut missed = Vec::new();
    for v in &vectors {
        if cbor::scan_strict(&v.bytes).is_ok() {
            missed.push(format!("{} ({})", v.id, v.statement));
        }
    }
    assert!(
        missed.is_empty(),
        "scan_strict accepted invalid vectors: {missed:#?}"
    );
}

/// Which layer catches what. Printed, not asserted: it describes the dependency, and asserting
/// it would turn a `serde_ipld_dagcbor` release into a gx test failure. The numbers are what the
/// step-3 report quotes for 「reject 層の内訳」.
#[test]
fn negative_reject_layer_breakdown() {
    let vectors = load();
    let mut both = 0;
    let mut gx_only = 0;
    println!(
        "{:<5} {:<24} {:<10} gx scanner",
        "id", "normative_basis", "codec"
    );
    for v in &vectors {
        let codec_rejects = serde_ipld_dagcbor::from_slice::<Ipld>(&v.bytes).is_err();
        let gx = cbor::scan_strict(&v.bytes);
        assert!(gx.is_err(), "{}: gx scanner must reject", v.id);
        if codec_rejects {
            both += 1;
        } else {
            gx_only += 1;
        }
        println!(
            "{:<5} {:<24} {:<10} {}",
            v.id,
            v.normative_basis,
            if codec_rejects { "REJECT" } else { "accept" },
            gx.unwrap_err()
        );
    }
    println!("REJECTED_BY_BOTH_LAYERS={both} REJECTED_BY_GX_LAYER_ONLY={gx_only}");
    assert_eq!(both + gx_only, EXPECTED_VECTOR_COUNT);
}

/// The positive controls. A suite whose negatives all fail can be failing because the checker is
/// broken; the controls are what tells the two apart (req/29 §2, req/08 N-4).
#[test]
fn negative_positive_controls_behave_as_declared() {
    let vectors = load();
    let mut controls = 0;
    for v in &vectors {
        let Some((bytes, expect_accept)) = &v.control else {
            continue;
        };
        controls += 1;
        let got = cbor::decode::<Ipld>(bytes);
        assert_eq!(
            got.is_ok(),
            *expect_accept,
            "{}: control vector behaved unexpectedly: {got:?}",
            v.id
        );
    }
    // Eighteen of the nineteen carry a control; A-7 (NaN) has none because NaN has no canonical
    // form, and it says so in its `control_note`. D-65's control is the one that carries the most
    // weight: it is the only assertion in the workspace that fixes which side of 64 the depth
    // ceiling falls on (F-2).
    assert!(controls >= 18, "only {controls} controls found");
    println!("NEGATIVE_VECTOR_CONTROLS={controls} of {EXPECTED_VECTOR_COUNT}");
}

/// The refusal is the *declared* one, for every vector that declares one (**H6-4**).
///
/// `is_err()` is satisfied by any refusal, including the wrong one. `scan_strict`'s major-type-7 arm
/// ends in a catch-all, so removing the float arm, the reserved arm or the break arm leaves every
/// input still refused -- by `SimpleValueNotAllowed`, for a reason that is no longer true. This is
/// the assertion that tells those apart, and it is why the six S-AI vectors carry `expected_error`.
///
/// A vector without the field is skipped rather than defaulted: the twelve older ones state a
/// refusal and not a variant, and inventing one for them here would put an assertion in the code
/// that no vector file made.
#[test]
fn negative_the_declared_error_is_the_one_the_scanner_returns() {
    let vectors = load();
    let mut checked = 0;
    for v in &vectors {
        let Some(expected) = &v.expected_error else {
            continue;
        };
        checked += 1;
        let got = cbor::scan_strict(&v.bytes)
            .expect_err("a negative vector is refused; that is the other tests' subject");
        assert_eq!(
            error_name(&got),
            expected.as_str(),
            "{}: the scanner refused it as `{}` and the vector declares `{expected}`. The refusal \
             is not the claim -- the clause that made it is ({}).",
            v.id,
            error_name(&got),
            v.statement
        );
    }
    assert!(
        checked >= 13,
        "only {checked} vectors declare an expected error; H6-4 added six, hand 7 three (F-1: P-3 \
         and A-6; F-2: D-65) and M3 hand 4 four (F-3: A-8 and TR-1; A-8: P-1 and A-7; A-9: D-65K)"
    );
    println!("NEGATIVE_VECTORS_WITH_A_DECLARED_ERROR={checked} of {EXPECTED_VECTOR_COUNT}");
}

/// The numeric fields of a refusal, for the vectors that declare any (**F-3**).
///
/// `is_err()` says a byte string was refused and `expected_error` says under which clause; neither
/// reads the *numbers* the refusal carries, and two of gx-canon's mutation survivors lived exactly
/// there -- `missing: end - self.b.len()` and `extra: bytes.len() - scan.i`, whose `-` can become
/// `+` or `/` without any vector noticing. An error message an operator acts on ("input ends 8
/// bytes early") is a claim, and an unchecked claim in a message is the same defect class as an
/// unchecked claim in a name (req/08 N-1).
///
/// Written out per variant rather than through `Debug`: a `Debug` string would make the assertion
/// depend on formatting, and formatting is not what the vector is declaring.
fn error_payload(e: &Error) -> BTreeMap<&'static str, u64> {
    let mut out = BTreeMap::new();
    let mut put = |k: &'static str, v: usize| {
        out.insert(k, v as u64);
    };
    match e {
        Error::Truncated {
            at,
            wanted,
            missing,
        } => {
            put("at", *at);
            put("wanted", *wanted);
            put("missing", *missing);
        }
        Error::TrailingBytes { at, extra } => {
            put("at", *at);
            put("extra", *extra);
        }
        Error::TooDeep { at, max } => {
            put("at", *at);
            put("max", *max);
        }
        Error::NonMinimal {
            at,
            major,
            ai,
            value,
        } => {
            put("at", *at);
            put("major", usize::from(*major));
            put("ai", usize::from(*ai));
            out.insert("value", *value);
        }
        Error::ReservedAdditionalInfo { at, ai } => {
            put("at", *at);
            put("ai", usize::from(*ai));
        }
        Error::SimpleValueNotAllowed { at, ai } => {
            put("at", *at);
            put("ai", usize::from(*ai));
        }
        Error::NonTextMapKey { at, major } => {
            put("at", *at);
            put("major", usize::from(*major));
        }
        Error::TagNotAllowed { at, tag } => {
            put("at", *at);
            out.insert("tag", *tag);
        }
        Error::IndefiniteLength { at }
        | Error::FloatNotAllowed { at }
        | Error::UnsortedOrDuplicateMapKey { at }
        | Error::InvalidUtf8 { at, .. } => put("at", *at),
        _ => {}
    }
    out
}

/// The numbers a refusal reports are the numbers the vector declares (**F-3**).
///
/// A vector declares the fields it means and nothing else, so this asserts a subset. A field named
/// by a vector and absent from the refusal is a failure rather than a skip: it means the vector and
/// the variant disagree about what the refusal carries.
#[test]
fn negative_the_declared_payload_is_the_one_the_scanner_reports() {
    let vectors = load();
    let mut checked = 0;
    for v in &vectors {
        if v.expected_error_payload.is_empty() {
            continue;
        }
        checked += 1;
        let got = cbor::scan_strict(&v.bytes).expect_err("a negative vector is refused");
        let payload = error_payload(&got);
        for (field, declared) in &v.expected_error_payload {
            let actual = payload.get(field.as_str()).unwrap_or_else(|| {
                panic!(
                    "{}: the vector declares `{field}` and {} carries no such field",
                    v.id,
                    error_name(&got)
                )
            });
            assert_eq!(
                actual,
                declared,
                "{}: {} reported {field}={actual} and the vector declares {declared}. The \
                 refusal is not the whole claim -- the number is what an operator reads ({}).",
                v.id,
                error_name(&got),
                v.statement
            );
        }
    }
    assert!(
        checked >= 3,
        "only {checked} vectors declare a payload; hand 4 added three (A-8, TR-1, D-65K)"
    );
    println!("NEGATIVE_VECTORS_WITH_A_DECLARED_PAYLOAD={checked} of {EXPECTED_VECTOR_COUNT}");
}

/// 42 §2.1-2 as written says the sort key is 「キーのUTF-8バイト列」; IPLD's spec.md L58 says the
/// comparison includes 「their major type 3 and length」, which puts a short key before a long one
/// regardless of content. The two readings disagree -- under the first, `{"aa":2,"b":1}` is
/// canonical; under the second, `{"b":1,"aa":2}` is. `serde_ipld_dagcbor`, which 42 §2.1-6 names
/// as authoritative for the implementation, writes the second. This test pins the behaviour the
/// implementation actually has, so the discrepancy is recorded in the suite rather than only in
/// prose (erratum candidate, reported in req/41).
#[test]
fn negative_map_key_order_follows_the_encoded_bytes_not_the_key_text() {
    let length_first = unhex("a261620162616102"); // {"b":1,"aa":2}
    let text_first = unhex("a262616102616201"); // {"aa":2,"b":1}
    assert!(
        cbor::scan_strict(&length_first).is_ok(),
        "IPLD order (shorter key first) must be accepted"
    );
    assert!(
        cbor::scan_strict(&text_first).is_err(),
        "the 42 §2.1-2 literal reading must be rejected, since the encoder never writes it"
    );
}
