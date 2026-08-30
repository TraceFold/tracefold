// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **RESIDUAL-895-B1** (`req/38` SS861 §⑥, `req/910` B6, `req/919` W7, 2026-08-30) — `.gx/index/`
//! holds the branch the engine holds.
//!
//! `DEFECT-891-1` (`req/895` §2) repaired the **engine**'s side: `Engine::resolved`'s backing map
//! became a journal-ordered `Vec<TransformationId>` per intent, because `undo` mints a `T_u` whose
//! `parents` are inside a `Transformation`'s identity and outside an `Intent`'s, so two undos
//! restoring the same bytes at the same locator under the same context and actor are one intent and
//! two transformations. SS861 §⑥ then recorded, in the "what I did not look at" section, that the
//! CLI's disposable copy of that map was **still** one-intent-one-transformation. This suite is that
//! residual closed, and it asserts three separate things, because they can fail separately:
//!
//! 1. **the branch survives a round trip** — the shape `undo`, `undo` produces;
//! 2. **a pre-W7 file still loads whole** — the compatibility req/56 §2's "regenerable" cell does
//!    *not* require (a rebuild would be legal) and which is provided anyway, because throwing away
//!    an undamaged cache to honour a version number nobody wrote down is not a repair;
//! 3. **damage is still counted, per id** — the [`Freshness`] report is the layer's §5 obligation
//!    ("what was lost and what was regenerated is always declared") and a widened value must not
//!    blur it.
//!
//! # What this suite is **not**
//!
//! It is not evidence that any `gx` command was answering wrongly. The cache is consulted as an
//! ordering hint and the engine is asked before `Session::intent_of` returns (`resolves_to`), so the
//! flattening cost a hint, not an answer — stated here rather than left for a reader to assume the
//! stronger claim. The engine-side defect that *did* cost an answer is `DEFECT-891-1`, and it was
//! measured in `req/895` §2 with an exit code.

use std::path::{Path, PathBuf};

use gx_cli::index::{Freshness, ResolutionIndex};
use gx_cli::layout::Layout;
use gx_core::{Cid, IntentId, TransformationId};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

/// One intent, two transformations, through the file and back.
///
/// The fixture is the sequence the defect was found on: an intent is planned into `t_plan`, and then
/// the same `Intent` is reached a second time by `undo`'s road into `t_undo`. `learn` is called the
/// way `Session::remember` calls it — once per road, on a cache re-loaded in between, because that
/// is the real path (each `gx` verb is its own process and re-reads the file).
#[test]
fn w7_the_cache_keeps_the_branch_across_a_round_trip() {
    let project = scratch("r919_index_branch");
    let layout = Layout::create(&project).expect("create");
    let intent_id = IntentId(Cid([11u8; 32]));
    let t_plan = TransformationId(Cid([12u8; 32]));
    let t_undo = TransformationId(Cid([13u8; 32]));

    // Process 1: `gx plan`.
    let (mut index, freshness) = ResolutionIndex::load(&layout);
    assert_eq!(freshness, Freshness::Absent);
    index.learn(intent_id, t_plan);
    index.store(&layout).expect("store");

    // Process 2: `gx undo`, reaching the same intent by the other road.
    let (mut index, freshness) = ResolutionIndex::load(&layout);
    assert_eq!(freshness, Freshness::Loaded);
    index.learn(intent_id, t_undo);
    index.store(&layout).expect("store");

    // Process 3: anyone asking.
    let (back, freshness) = ResolutionIndex::load(&layout);
    println!(
        "W7_BRANCH intents={} chain={:?} freshness={freshness:?}",
        back.len(),
        back.get_all(&intent_id)
            .iter()
            .map(|t| t.0.to_text())
            .collect::<Vec<_>>()
    );
    assert_eq!(freshness, Freshness::Loaded);
    assert_eq!(
        back.get_all(&intent_id),
        &[t_plan, t_undo],
        "🔴 the branch, in journal order. If this is `[t_undo]` the cache has flattened the tree \
         again -- which is RESIDUAL-895-B1 exactly, and the engine's `resolved_all` disagreeing \
         with its own cache"
    );
    assert_eq!(
        back.get(&intent_id),
        Some(t_undo),
        "Λ3(ii) is unchanged: the single-valued road answers the last one, as `Engine::resolved` \
         does. W7 widened what is kept, not what 44 §0's id-resolution reads"
    );
    assert!(
        back.resolves_to(&intent_id, &t_plan),
        "🔴 the predicate `Session::intent_of` orders candidates by. `get() == Some(t_plan)` is \
         false here and always was -- that is DEFECT-891-1's spelling, one layer down"
    );
    assert!(back.resolves_to(&intent_id, &t_undo));
    assert_eq!(back.len(), 1, "one intent, still one key");
}

/// Idempotent, because the engine's fold is: 43 T-2 re-runs against the same snapshot and yields the
/// same `TransformationId`, and a cache that grew a duplicate each time would be a second opinion
/// about how long a branch is. Mirrors `Engine::remember_resolution`.
#[test]
fn w7_relearning_the_same_transformation_does_not_grow_the_chain() {
    let intent_id = IntentId(Cid([21u8; 32]));
    let t = TransformationId(Cid([22u8; 32]));
    let mut index = ResolutionIndex::new();
    index.learn(intent_id, t);
    index.learn(intent_id, t);
    index.learn(intent_id, t);
    println!("W7_IDEMPOTENT chain_len={}", index.get_all(&intent_id).len());
    assert_eq!(index.get_all(&intent_id), &[t]);
}

/// 🔴 A `.gx/index/` written by any build before this one loads **whole**, and says so.
///
/// The bytes below are the pre-W7 spelling verbatim (`{"resolutions":{"<intent>":"<transformation>"}}`),
/// written by hand rather than produced by an old binary, and that is the honest limit of this
/// probe: it pins the *shape* the old writer emitted (`serde_json::to_vec_pretty` of a
/// `BTreeMap<String, String>`), not a byte-for-byte artefact of a build that no longer exists.
///
/// What must not happen is `Freshness::Unreadable`. That would be legal under req/56 §2 — the
/// directory is declared safe to delete — and it would still be a regression: every existing
/// project would silently lose a cache that was not damaged, on upgrade, for a shape change that
/// loses no information.
#[test]
fn w7_a_pre_w7_cache_file_still_loads_whole() {
    let project = scratch("r919_index_legacy");
    let layout = Layout::create(&project).expect("create");
    let intent_id = IntentId(Cid([31u8; 32]));
    let t = TransformationId(Cid([32u8; 32]));

    let legacy = format!(
        "{{\n  \"resolutions\": {{\n    \"{}\": \"{}\"\n  }}\n}}",
        intent_id.0.to_text(),
        t.0.to_text()
    );
    std::fs::create_dir_all(layout.join("index")).expect("mkdir");
    std::fs::write(ResolutionIndex::path_in(&layout), legacy.as_bytes()).expect("write");

    let (back, freshness) = ResolutionIndex::load(&layout);
    println!("W7_LEGACY freshness={freshness:?} len={}", back.len());
    assert_eq!(
        freshness,
        Freshness::Loaded,
        "🔴 a pre-W7 file is not damage. If this is `Unreadable`, the untagged `Chain` has lost \
         its `One` arm and every existing project's cache is discarded on upgrade"
    );
    assert!(!freshness.needs_rebuild());
    assert_eq!(back.get_all(&intent_id), &[t], "read as a chain of one");
    assert_eq!(back.get(&intent_id), Some(t));

    // And the rewrite is the new spelling, so the migration happens by being used.
    back.store(&layout).expect("store");
    let raw = std::fs::read_to_string(ResolutionIndex::path_in(&layout)).expect("read");
    println!("W7_REWRITTEN={raw}");
    assert!(
        raw.contains('['),
        "the writer emits the list form even for a chain of one"
    );
    let (again, freshness) = ResolutionIndex::load(&layout);
    assert_eq!(freshness, Freshness::Loaded);
    assert_eq!(again.get_all(&intent_id), &[t], "and reads back the same");
}

/// 🔴 Adversarial: a tampered cache. Damage is counted **per id**, and what parses is kept.
///
/// Three entries: one clean chain of two, one chain whose second element is not an id, and one whose
/// key is not an id. The expected report is three survivors across two intents and two losses — and
/// the reason it is asserted rather than left to `Loaded`/`Unreadable` is req/56 §5's own rule for
/// this directory: what was lost and what was regenerated is always declared.
#[test]
fn w7_a_tampered_chain_loses_only_the_bad_ids_and_reports_them() {
    let project = scratch("r919_index_tampered");
    let layout = Layout::create(&project).expect("create");
    let good_intent = IntentId(Cid([41u8; 32]));
    let hurt_intent = IntentId(Cid([42u8; 32]));
    let a = TransformationId(Cid([43u8; 32]));
    let b = TransformationId(Cid([44u8; 32]));
    let c = TransformationId(Cid([45u8; 32]));

    let body = format!(
        "{{\"resolutions\":{{\
           \"{gi}\":[\"{a}\",\"{b}\"],\
           \"{hi}\":[\"{c}\",\"not-an-id\"],\
           \"also-not-an-id\":[\"{a}\"]\
         }}}}",
        gi = good_intent.0.to_text(),
        hi = hurt_intent.0.to_text(),
        a = a.0.to_text(),
        b = b.0.to_text(),
        c = c.0.to_text()
    );
    std::fs::create_dir_all(layout.join("index")).expect("mkdir");
    std::fs::write(ResolutionIndex::path_in(&layout), body.as_bytes()).expect("write");

    let (back, freshness) = ResolutionIndex::load(&layout);
    println!(
        "W7_TAMPERED freshness={freshness:?} intents={} good={:?} hurt={:?}",
        back.len(),
        back.get_all(&good_intent).len(),
        back.get_all(&hurt_intent).len()
    );
    assert_eq!(
        freshness,
        Freshness::PartiallyUnreadable { skipped: 2 },
        "🔴 two losses: one bad id inside a chain, one unreadable key. Counting the damaged chain \
         as a single dropped entry would flatten the report the same way the cache used to flatten \
         the branch"
    );
    assert_eq!(back.len(), 2, "the third entry had no readable intent");
    assert_eq!(back.get_all(&good_intent), &[a, b], "untouched");
    assert_eq!(
        back.get_all(&hurt_intent),
        &[c],
        "the readable half of a damaged chain is kept, as the pre-W7 loader kept the readable \
         half of a damaged file"
    );
}
