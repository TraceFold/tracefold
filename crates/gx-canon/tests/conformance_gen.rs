// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! M8-c conformance vector generator — `canon_idempotence` / `repr_independence` (T3).
//!
//! role: `req/142_M8_LEAN_CONFORMANCE_REQDEF_2026-08-14.md` §1 M8-c item 1, per the C-4 ruling (sem: SEM-gx-canon-079)
//! (`req/143_M8A_IMPL_REPORT_2026-08-14.md` §2 / `req/38` §84) that generation is a thin
//! output-side channel added to an existing crate's `tests/`, not a new crate. Vector envelope
//! schema (`vector_id`/`kind`/`input`/`expected`/`rust_git_sha`/`lean_toolchain`) is
//! `req/spec/40-architecture/46-verification-plan.md` §3.1 verbatim. Lean side:
//! `lean/GxSpec/Runner.lean` (`GxSpec.Runner.checkCanonIdempotence` /
//! `checkReprIndependence`), against `GxSpec.CanonModel` (`lean/GxSpec/Canon.lean`, frozen at
//! M8-b / `req/38` §86).
//!
//! # Why the vector content is an abstracted `{key,val}` list and not raw DAG-CBOR bytes
//!
//! `CanonModel` (46 §3.1's own executable-model column) does not model bytes: its own header
//! comment says the model abstracts a map key to its rank in bytewise order and a value to a
//! `Nat` (`lean/GxSpec/Canon.lean` L54-60). BLAKE3/DAG-CBOR are explicitly out of Lean's
//! formalisation scope (46 §1: "in-Lean verification of cryptography..." is a non-goal) (sem: SEM-gx-canon-080). So the part of each vector
//! Lean actually recomputes is the `fields` list; `canonical_bytes_hex` below is carried purely
//! as a real-Rust-artifact / provenance field (proof that `gx_canon::cbor::encode` really ran and
//! really produced canonical output for the same logical map) and is not re-derived by
//! `Runner.lean` — this split is reported in `req/145_M8C_IMPL_REPORT_2026-08-14.md`.
//!
//! Keys are fixed-length (3 lowercase ASCII letters) so that `BTreeMap<String,_>`'s `Ord`
//! (plain content-bytewise order) coincides with 42 §2.1-2's *encoded-header* bytewise order —
//! `crates/gx-canon/src/cbor.rs`'s own module doc already flags that the two orders disagree for
//! keys of differing byte length (filed as an erratum candidate in req/41); fixing the length
//! sidesteps that known edge case rather than fighting it inside a generator.
//!
//! **v0.2-c (`DR-46-2`, `req/150` §C, no-delete — the paragraph above described M8-c and is kept
//! as its record; it no longer describes this file)**: keys are now *variable-length* (1..=6
//! lowercase ASCII letters), precisely so the corpus exercises the case the fixed length used to
//! sidestep — the one `req/147` §2 A-1a showed the v0.1 rank-only abstraction was blind to. Two
//! changes close the A-1a blind spot end to end:
//!
//! 1. the canonical order each vector's `rank` refers to is no longer `BTreeMap` iteration order
//!    but is derived from the **real encoder**: keys are sorted by the byte string
//!    `gx_canon::cbor::encode(Ipld::String(key))` actually produces (encoded-header bytewise
//!    order, 42 §2.1-2's implemented reading), with the whole-map `encode` + `is_canonical`
//!    self-check kept as the backstop that this per-key derivation agrees with the map encoder;
//! 2. every field now carries `key_bytes` (the key's raw UTF-8 bytes) next to the v0.1 `key`
//!    rank, and `Runner.lean` re-derives the order from those bytes *independently* (abstract
//!    length-first-then-bytewise sequence comparison — not a CBOR header re-implementation, per
//!    46 §1's non-goal) and cross-checks it against the rank labelling. A rank labelling that
//!    inverts the true bytewise order — the exact `req/147` §2 A-1a attack — now diverges.
//!
//! `corpus_exercises_the_order_gap` (asserted below, deterministic under the fixed seeds): at
//! least one generated vector's canonical order differs from plain content-bytewise (`BTreeMap`)
//! order, so the corpus provably covers the inputs where the two orders disagree.
//!
//! **v0.3-c corpus widening (`req/159` §C-2, consuming `req/156` §6's declared denominator — the
//! paragraphs above are kept as the v0.2-c record)**: the key strategy is now a weighted union of
//! three arms — short ASCII (the v0.2-c corpus, unchanged), **UTF-8 multibyte keys** (Japanese
//! hiragana/katakana, 3 bytes per char), and **long keys over 23 bytes** (24..=32 ASCII letters,
//! whose CBOR text-string header takes the two-byte `0x78 <len>` form instead of the one-byte
//! `0x60+len` form). Both were `req/156` §6's explicitly-declared corpus gaps: `keyBytesLt`
//! (`lean/GxSpec/Runner.lean`) claims equivalence with 42 §2.1-2's *encoded-header* order for
//! text keys **at every length** (the length-first argument holds across the header-width
//! boundary: any 23-byte key's encoding starts `0x77`, any 24-byte key's starts `0x78`), and this
//! widening turns that argument's multibyte and header-2-byte cases from reasoning into measured
//! differential coverage. Two per-generation coverage assertions (deterministic under the fixed
//! seeds) join the order-gap gate: ≥1 vector carries a non-ASCII key, and ≥1 vector carries a
//! key longer than 23 bytes. Non-text keys remain out of scope **by the type system, not by
//! omission**: `ipld_core::ipld::Ipld::Map` is `BTreeMap<String, Ipld>` (ipld-core src/ipld.rs —
//! DAG-CBOR's own map-key restriction), so a non-text key is unrepresentable in the generator's
//! input model; declared here as the denominator rather than generated.

mod support;

use gx_canon::cbor;
use ipld_core::ipld::Ipld;
use proptest::sample::SizeRange;
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;

/// Vectors per kind. 250 x 2 kinds from this file = half of AC-062's >=10^3 PR-scale floor;
/// gx-gate and gx-witness each contribute another 250 x 2 / 250 x 1 (`req/145` totals them).
const N_PER_KIND: usize = 250;

/// PR-scale default (`N_PER_KIND`, unchanged from M8-c) unless overridden by `GX_CONFORMANCE_N`
/// -- the nightly AC-063 run (`req/142` §1 M8-d item 2) sets this to reach >=10^5 vectors in
/// aggregate across all five kinds without touching the PR-scale gen path at all.
fn vectors_per_kind() -> usize {
    std::env::var("GX_CONFORMANCE_N")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(N_PER_KIND)
}

fn git_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// `<repo root>/conformance` — the bridge `req/142` §3 fixes as the only crossing between the
/// WSL2 Rust side and the Windows lake side (`/mnt/c` is the same bytes as `C:\`).
///
/// Same two-`ancestors` idiom `probes/doubt/tests/floor_doubt.rs::repo_root` uses: this crate
/// sits at `<root>/crates/gx-canon`, two segments below root, just as `probes/doubt` does.
fn conformance_dir() -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("gx-canon sits at <root>/crates/gx-canon")
        .join("conformance");
    std::fs::create_dir_all(&dir).expect("conformance/ is creatable");
    dir
}

fn write_jsonl(name: &str, lines: &[Value]) {
    let path = conformance_dir().join(name);
    let mut f = std::fs::File::create(&path)
        .unwrap_or_else(|e| panic!("cannot create {}: {e}", path.display()));
    for v in lines {
        writeln!(f, "{v}").expect("a line writes");
    }
}

/// Variable-length key, three weighted arms (module doc: v0.2-c `DR-46-2` + v0.3-c widening).
/// Variable length is the point: only length-mixed key sets make encoded-header order and plain
/// content-bytewise order disagree. The multibyte arm draws hiragana/katakana (3 UTF-8 bytes per
/// char — `key_bytes` carries the raw bytes either way); the long arm draws 24..=32 ASCII letters,
/// putting the key's CBOR text header into its two-byte form (`0x78 <len>`). Distinctness is
/// by-construction inside a `BTreeMap` draw, as before.
fn key_strategy() -> impl Strategy<Value = String> {
    proptest::prop_oneof![
        6 => "[a-z]{1,6}",
        2 => "[\u{3041}-\u{3093}\u{30a1}-\u{30f3}]{1,4}", // hiragana U+3041-U+3093 + katakana U+30A1-U+30F3, spelled by code point (sem: SEM-gx-canon-081)
        2 => "[a-z]{24,32}",
    ]
}

/// `N` distinct keys with small `u32` values, drawn as a map so the strategy itself guarantees
/// distinctness. `.into_iter()` yields plain content-bytewise (`String` `Ord`) order, which for
/// variable-length keys is **not** necessarily 42 §2.1-2's canonical order — the caller re-sorts
/// via `canonical_sort` (real-encoder-derived) before assigning ranks.
fn fields_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<(String, u32)>> {
    proptest::collection::btree_map(key_strategy(), 0u32..1000, SizeRange::from(min..=max))
        .prop_map(|m| m.into_iter().collect())
}

/// The canonical key order, derived from the real encoder rather than restated: sort by the byte
/// string `gx_canon::cbor::encode` writes for the key alone (major-type-3 header included). The
/// whole-map `encode` + `is_canonical` assertions in each generator below are the backstop that
/// this per-key derivation and the map encoder's own key ordering agree.
fn canonical_sort(drawn: &[(String, u32)]) -> Vec<(String, u32)> {
    let mut v: Vec<(String, u32)> = drawn.to_vec();
    v.sort_by_key(|(k, _)| {
        cbor::encode(&Ipld::String(k.clone())).expect("a text-string key encodes")
    });
    v
}

fn build_ipld(sorted: &[(String, u32)]) -> Ipld {
    let map: BTreeMap<String, Ipld> = sorted
        .iter()
        .map(|(k, v)| (k.clone(), Ipld::Integer(i128::from(*v))))
        .collect();
    Ipld::Map(map)
}

/// `{key: rank, key_bytes, val}` objects, `rank` = index in the canonical (sorted) order — the
/// abstraction `CanonModel` (`lean/GxSpec/Canon.lean` L54-60) asks for — plus, since v0.2-c
/// (`DR-46-2`), `key_bytes` = the key's raw UTF-8 bytes, which `Runner.lean` orders
/// *independently* and cross-checks against the rank labelling (module doc above).
fn field_objs(order: &[usize], sorted: &[(String, u32)]) -> Vec<Value> {
    order
        .iter()
        .map(|&rank| {
            json!({
                "key": rank,
                "key_bytes": sorted[rank].0.as_bytes(),
                "val": sorted[rank].1,
            })
        })
        .collect()
}

fn new_runner(seed: u8) -> TestRunner {
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &[seed; 32]);
    TestRunner::new_with_rng(Config::default(), rng)
}

#[test]
fn conformance_gen_canon_idempotence() {
    let sha = git_sha();
    let mut runner = new_runner(0xC1);
    let strat = fields_strategy(1, 8);
    let count = vectors_per_kind();
    let mut lines = Vec::with_capacity(count);
    let mut order_gap_covered = false;
    let mut multibyte_covered = false;
    let mut long_key_covered = false;
    for i in 0..count {
        let drawn = strat
            .new_tree(&mut runner)
            .expect("strategy draws")
            .current();
        // v0.2-c (DR-46-2): ranks refer to the *encoded-order* canonical sort, derived from the
        // real encoder -- not `BTreeMap` iteration order, which disagrees for length-mixed keys.
        let sorted = canonical_sort(&drawn);
        order_gap_covered |= sorted != drawn;
        // v0.3-c coverage flags (module doc): the req/156 §6 denominator being consumed.
        multibyte_covered |= sorted.iter().any(|(k, _)| !k.is_ascii());
        long_key_covered |= sorted.iter().any(|(k, _)| k.len() > 23);
        let n = sorted.len();
        let canonical_order: Vec<usize> = (0..n).collect();
        // Deterministic scramble: reverse. Differs from sorted whenever n >= 2, which the
        // corpus is (min = 1 above only ever produces the degenerate n = 1 case rarely, since
        // btree_map's SizeRange still usually draws several entries).
        let scrambled_order: Vec<usize> = canonical_order.iter().rev().copied().collect();

        let ipld = build_ipld(&sorted);
        let bytes1 =
            cbor::encode(&ipld).expect("fixed-length keys stay inside gx-canon's canonical order");
        assert!(
            cbor::is_canonical(&bytes1),
            "gx-canon's own encoder output must self-certify"
        );
        // Real-Rust idempotence self-check (T3-a on the actual implementation, not the model):
        // re-parse leniently (candidate, possibly non-canonical in general) and re-encode; a
        // canonical byte string must be a fixed point of that round trip.
        let reparsed: Ipld =
            serde_ipld_dagcbor::from_slice(&bytes1).expect("gx-canon's own output decodes");
        let bytes2 = cbor::encode(&reparsed).expect("re-encoding a canonical value re-succeeds");
        assert_eq!(
            bytes1, bytes2,
            "gx-canon's real canonicalisation is not idempotent on {sorted:?}"
        );

        let vector = json!({
            "vector_id": format!("m8c-canon_idempotence-{i:05}"),
            "kind": "canon_idempotence",
            "input": {"fields": field_objs(&scrambled_order, &sorted)},
            "expected": {
                "fields": field_objs(&canonical_order, &sorted),
                "canonical_bytes_hex": support::hex(&bytes1),
            },
            "rust_git_sha": sha,
            "lean_toolchain": "leanprover/lean4:v4.33.0",
        });
        lines.push(vector);
    }
    // DR-46-2 corpus-coverage gate (module doc): the corpus must contain at least one vector
    // where encoded-order and plain content-bytewise order disagree, or the A-1a blind spot is
    // not being exercised at all. Deterministic under the fixed seed.
    assert!(
        order_gap_covered,
        "no generated vector had canonical order != plain bytewise order -- \
         the DR-46-2 corpus does not exercise the req/147 A-1a order gap"
    );
    // v0.3-c coverage gates (req/159 §C-2): the corpus must exercise a UTF-8 multibyte key and a
    // key whose text header takes the two-byte form, or the keyBytesLt equivalence claim's
    // multibyte / header-boundary cases are back to reasoning-only (the req/156 §6 denominator).
    assert!(
        multibyte_covered,
        "no generated vector carried a non-ASCII (UTF-8 multibyte) key -- \
         the v0.3-c corpus does not exercise the req/156 §6 multibyte gap"
    );
    assert!(
        long_key_covered,
        "no generated vector carried a key longer than 23 bytes (2-byte CBOR header form) -- \
         the v0.3-c corpus does not exercise the req/156 §6 long-key header boundary"
    );
    write_jsonl("canon_idempotence.jsonl", &lines);
    println!(
        "CONFORMANCE_GEN canon_idempotence={} multibyte_covered={multibyte_covered} \
         long_key_covered={long_key_covered}",
        lines.len()
    );
}

#[test]
fn conformance_gen_repr_independence() {
    let sha = git_sha();
    let mut runner = new_runner(0xC2);
    let strat = fields_strategy(2, 8);
    let count = vectors_per_kind();
    let mut lines = Vec::with_capacity(count);
    let mut order_gap_covered = false;
    let mut multibyte_covered = false;
    let mut long_key_covered = false;
    for i in 0..count {
        let drawn = strat
            .new_tree(&mut runner)
            .expect("strategy draws")
            .current();
        // v0.2-c (DR-46-2): same real-encoder-derived canonical sort as canon_idempotence above.
        let sorted = canonical_sort(&drawn);
        order_gap_covered |= sorted != drawn;
        // v0.3-c coverage flags, same pair as conformance_gen_canon_idempotence (module doc).
        multibyte_covered |= sorted.iter().any(|(k, _)| !k.is_ascii());
        long_key_covered |= sorted.iter().any(|(k, _)| k.len() > 23);
        let n = sorted.len();
        let canonical_order: Vec<usize> = (0..n).collect();
        let repr_a: Vec<usize> = canonical_order.iter().rev().copied().collect();
        // rotate-by-one: a second, independent scramble so repr_a and repr_b usually differ from
        // each other too (not just from the canonical order), for n >= 3.
        let repr_b: Vec<usize> = (0..n).map(|k| (k + 1) % n).collect();

        let ipld = build_ipld(&sorted);
        let bytes = cbor::encode(&ipld).expect("fixed-length keys stay canonical");
        assert!(cbor::is_canonical(&bytes));

        let vector = json!({
            "vector_id": format!("m8c-repr_independence-{i:05}"),
            "kind": "repr_independence",
            "input": {
                "repr_a": {"fields": field_objs(&repr_a, &sorted)},
                "repr_b": {"fields": field_objs(&repr_b, &sorted)},
            },
            "expected": {
                "fields": field_objs(&canonical_order, &sorted),
                "canonical_bytes_hex": support::hex(&bytes),
            },
            "rust_git_sha": sha,
            "lean_toolchain": "leanprover/lean4:v4.33.0",
        });
        lines.push(vector);
    }
    // Same DR-46-2 corpus-coverage gate as conformance_gen_canon_idempotence (module doc).
    assert!(
        order_gap_covered,
        "no generated vector had canonical order != plain bytewise order -- \
         the DR-46-2 corpus does not exercise the req/147 A-1a order gap"
    );
    // Same v0.3-c coverage gates as conformance_gen_canon_idempotence (req/159 §C-2).
    assert!(
        multibyte_covered,
        "no generated vector carried a non-ASCII (UTF-8 multibyte) key -- \
         the v0.3-c corpus does not exercise the req/156 §6 multibyte gap"
    );
    assert!(
        long_key_covered,
        "no generated vector carried a key longer than 23 bytes (2-byte CBOR header form) -- \
         the v0.3-c corpus does not exercise the req/156 §6 long-key header boundary"
    );
    write_jsonl("repr_independence.jsonl", &lines);
    println!(
        "CONFORMANCE_GEN repr_independence={} multibyte_covered={multibyte_covered} \
         long_key_covered={long_key_covered}",
        lines.len()
    );
}
