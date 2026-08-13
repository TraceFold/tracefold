//! **M6-01 採(a)** and **M6-02 採(b)** — the draft store and the id-resolution cache, round-tripped.
//!
//! The draft store is the load-bearing half. req/88 §4 M6-01 declared it a **hand 1 blocker**: `gx
//! submit` and `gx plan` are two processes, `Engine::plan` takes an `&Intent`, and the journal keeps
//! an `IntentId` and no body. If a draft cannot survive being written and read back, hand 3 has no
//! `gx plan` to write. So the probes below are about the round trip and about the two things that
//! must not silently drift through it: the five fields of 42 §3.3, and the identity they determine.
//!
//! 🔴 **The identity check is the one that matters and this suite cannot do it alone.** `IntentId` is
//! the CID of the canonical form (ASM-11) and this crate may not compute one (則 1 (i)), so 「the
//! reloaded intent has the same id」 is asserted in gx-engine's suite, where an engine exists to mint
//! it. Here the claim is the weaker, checkable one: the reloaded `Intent` is `==` the original, field
//! for field. Stating which half lives where is the point — an equality that *looked* like an
//! identity check would be this suite claiming a guarantee it cannot give.

use std::path::{Path, PathBuf};

use gx_cli::draft::{DraftRecord, DraftStore};
use gx_cli::index::{both_readings, parse_id, Freshness, ResolutionIndex};
use gx_cli::layout::Layout;
use gx_core::{
    Actor, ChangeContext, Cid, GoalBytes, Intent, IntentId, SubstrateKind, TransformationId,
};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn intent(locator: &str, goal: &[u8]) -> Intent {
    Intent::new(
        SubstrateKind::Fs,
        locator.to_string(),
        GoalBytes(goal.to_vec()),
        ChangeContext::Policy,
        Actor::Agent {
            key: "key-agent-1".to_string(),
            model: "claude-fable-5".to_string(),
        },
    )
}

/// A draft written by one call and read by another is the same intent.
///
/// The id is handed in rather than computed — 則 1 (i) in the signature of `DraftStore::put` — so the
/// fixture uses an arbitrary one. That is honest about what this layer knows: the CLI files a body
/// under a name the engine gave it and never checks the name against the body, because checking
/// would mean computing.
#[test]
fn a_draft_survives_being_written_and_read_back() {
    let project = scratch("draft_roundtrip");
    let layout = Layout::create(&project).expect("create");
    let store = DraftStore::in_layout(&layout);
    let id = IntentId(Cid([3u8; 32]));
    let original = intent("/tmp/deep/path", b"a goal with \x00 bytes and \xff bytes");

    let path = store.put(&id, &original).expect("put");
    println!("DRAFT_PATH={}", path.display());
    let back = store.get(&id).expect("get").expect("the draft is there");
    assert_eq!(
        back, original,
        "the five fields of 42 §3.3 have to come back unchanged, non-UTF-8 goal included"
    );
    assert_eq!(store.len().expect("len"), 1);
}

/// An absent draft is `Ok(None)`; a corrupt one is `Err`. E-M4-35: 「読めない」を「無い」と読まない.
#[test]
fn an_absent_draft_and_a_corrupt_draft_are_different_answers() {
    let project = scratch("draft_absent");
    let layout = Layout::create(&project).expect("create");
    let store = DraftStore::in_layout(&layout);
    let id = IntentId(Cid([4u8; 32]));

    assert!(
        store.get(&id).expect("get").is_none(),
        "no draft is not an error"
    );
    assert!(store.len().expect("len") == 0);

    std::fs::write(store.path_of(&id), b"{ not json").expect("write");
    let err = store.get(&id).expect_err("a corrupt draft is an error");
    println!("CORRUPT_DRAFT={err}");
    assert!(matches!(err, gx_cli::Error::Malformed { .. }));
}

/// A goal that is not base64 is refused rather than repaired.
///
/// 42 §3.3 puts the goal inside the `IntentId`, so a lossy decode here would produce an intent whose
/// id is not the id it is filed under — a body and a name that disagree, which is the one failure a
/// content-addressed store cannot detect from the inside.
#[test]
fn a_goal_that_is_not_base64_is_refused() {
    let project = scratch("draft_badgoal");
    let layout = Layout::create(&project).expect("create");
    let store = DraftStore::in_layout(&layout);
    let id = IntentId(Cid([5u8; 32]));
    let record = DraftRecord {
        substrate: SubstrateKind::Fs,
        locator: "/tmp/x".to_string(),
        goal_b64: "!!! not base64 !!!".to_string(),
        context: ChangeContext::Policy,
        actor: Actor::Process {
            key: "key-1".to_string(),
        },
    };
    std::fs::write(
        store.path_of(&id),
        serde_json::to_vec(&record).expect("serialise"),
    )
    .expect("write");

    let err = store.get(&id).expect_err("a bad goal is refused");
    println!("BAD_GOAL={err}");
    assert!(matches!(err, gx_cli::Error::Malformed { .. }));
}

/// Removing a draft is idempotent and says whether there was one.
#[test]
fn removing_a_draft_reports_whether_there_was_one() {
    let project = scratch("draft_remove");
    let layout = Layout::create(&project).expect("create");
    let store = DraftStore::in_layout(&layout);
    let id = IntentId(Cid([6u8; 32]));
    store.put(&id, &intent("/tmp/x", b"g")).expect("put");
    assert!(store.remove(&id).expect("remove"));
    assert!(!store.remove(&id).expect("second remove"));
    assert!(store.is_empty().expect("is_empty"));
}

/// The filename is the id's text form with the colon replaced, and two ids do not collide.
#[test]
fn the_draft_filename_is_derived_from_the_id_and_is_portable() {
    let project = scratch("draft_names");
    let layout = Layout::create(&project).expect("create");
    let store = DraftStore::in_layout(&layout);
    let one = store.path_of(&IntentId(Cid([1u8; 32])));
    let two = store.path_of(&IntentId(Cid([2u8; 32])));
    let name = one
        .file_name()
        .expect("named")
        .to_string_lossy()
        .to_string();
    println!("DRAFT_FILENAME={name}");
    assert_ne!(one, two);
    assert!(
        !name.contains(':'),
        "33 NFR-021 keeps development off Linux-only assumptions; a colon is not a filename \
         character on Windows"
    );
    assert!(name.ends_with(".json"));
}

// ---------------------------------------------------------------------------
// M6-02 採(b): the id-resolution cache
// ---------------------------------------------------------------------------

/// The cache round-trips and the later learning wins, which is the engine's rule (Λ3(ii)).
#[test]
fn the_resolution_cache_round_trips_and_the_later_learning_wins() {
    let project = scratch("index_roundtrip");
    let layout = Layout::create(&project).expect("create");
    let intent_id = IntentId(Cid([7u8; 32]));
    let first = TransformationId(Cid([8u8; 32]));
    let second = TransformationId(Cid([9u8; 32]));

    let mut index = ResolutionIndex::new();
    index.learn(intent_id, first);
    index.learn(intent_id, second);
    index.store(&layout).expect("store");

    let (back, freshness) = ResolutionIndex::load(&layout);
    println!("CACHE_LEN={} FRESHNESS={freshness:?}", back.len());
    assert_eq!(freshness, Freshness::Loaded);
    assert!(!freshness.needs_rebuild());
    assert_eq!(
        back.get(&intent_id),
        Some(second),
        "Λ3(ii): the later one wins"
    );
    assert_eq!(back.len(), 1, "one entry per intent, not a set");
}

/// 🔴 An absent or corrupt cache is an empty cache, **and the caller is told which**.
///
/// req/56 §2 declares this directory 「derived・消して良いと宣言」, so both cases have the same correct
/// repair and refusing on one of them would invent a failure mode on a path the specification
/// promises is disposable. What must not be lost is the distinction in the *report* — hence
/// [`Freshness`], and hence three separate assertions rather than one on emptiness.
#[test]
fn a_missing_or_corrupt_cache_is_empty_and_says_which() {
    let project = scratch("index_damage");
    let layout = Layout::create(&project).expect("create");

    let (empty, freshness) = ResolutionIndex::load(&layout);
    assert!(empty.is_empty() && freshness == Freshness::Absent);
    assert!(freshness.needs_rebuild());

    std::fs::write(ResolutionIndex::path_in(&layout), b"{ not json").expect("write");
    let (empty, freshness) = ResolutionIndex::load(&layout);
    println!("CORRUPT_CACHE_FRESHNESS={freshness:?}");
    assert!(empty.is_empty() && freshness == Freshness::Unreadable);

    // A well-formed file whose entries are not ids: the readable ones are kept and the count of the
    // dropped ones is reported, because 「some of it was junk」 and 「none of it was there」 are
    // different facts about a cache somebody may be debugging.
    let good = Cid([1u8; 32]).to_text();
    let body = format!("{{\"resolutions\":{{\"{good}\":\"{good}\",\"not-an-id\":\"{good}\"}}}}");
    std::fs::write(ResolutionIndex::path_in(&layout), body).expect("write");
    let (partial, freshness) = ResolutionIndex::load(&layout);
    println!(
        "PARTIAL_CACHE len={} freshness={freshness:?}",
        partial.len()
    );
    assert_eq!(partial.len(), 1);
    assert_eq!(freshness, Freshness::PartiallyUnreadable { skipped: 1 });
}

/// 🔴 則 1 (i) in the parser: a `gx1:` id is **parsed**, never computed.
///
/// 44 §0 asks the CLI to accept either kind of id, and 42 §1.1 says the bytes cannot tell you which
/// (「自己記述タグは付与しない」). So the two readings are produced as a pair and the store decides —
/// which is resolution against a store, exactly what 44 §0 describes, and not inspection.
#[test]
fn a_gx1_identifier_is_parsed_and_reads_two_ways() {
    let text = Cid([2u8; 32]).to_text();
    let parsed = parse_id(&text).expect("a well-formed id parses");
    let (as_intent, as_transformation) = both_readings(parsed);
    println!(
        "PARSED={text} AS_INTENT_EQ_AS_TID={}",
        u8::from(as_intent.0 == as_transformation.0)
    );
    assert_eq!(as_intent.0, as_transformation.0, "one Cid, two readings");

    for bad in ["", "gx1:", "not-an-id", "GX1:AAAA", &text[..text.len() - 1]] {
        assert!(
            parse_id(bad).is_err(),
            "{bad:?} is not a gx1 id and must be refused rather than coerced"
        );
    }
}
