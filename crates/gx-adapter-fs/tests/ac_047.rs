//! **AC-047**: two consecutive plans agree, and the substrate does not move.
//!
//! 34 逐語: 「Given: intent I。When: 同一adapterインスタンスに対し`adapter.plan(I)`を2回連続呼び出す。
//! Then: 同一`PlannedDelta`が返り、呼び出し前後でsubstrate状態（ファイルmtime/内容等）が変化しない。」
//! (判定方法: `unit + property`.) FR-042 is what it measures: 「`plan`は純関数（副作用なし）でなければ
//! ならない（MUST）」.
//!
//! # Three rulings decide how this is written
//!
//! * **E-M4-4** put the pre-state in the signature -- `plan(&self, intent, pre)` -- because 32
//!   FR-042 says 「同一intentから同一`PlannedDelta`」 without saying against what and 43 T-2 says
//!   「**同一snapshotに対し**」. So 「同一intent」 here means the pair.
//! * **M4-24** fixes what 「変化しない」 is measured with: 「内容 digest+mtime+size・atime は測らない
//!   (fs 依存で不安定=測れない物を測ったふりにしない)」. `support::state_of` is that measurement and
//!   its documentation carries the reason atime is absent.
//! * **E-M4-29** adds the fs-specific half: 「fs adapter v0.1 の `plan` は I/O 0 が成立する」, which is
//!   stronger than 「副作用なし」 and is measured separately in `tests/plan_purity.rs`.
//!
//! # The control
//!
//! req/69 §8.2: 「AC-047 の「変化しない」は、変化を注入したら RED になる対照を持って初めて主張になる」.
//! [`the_measurement_moves_when_the_substrate_moves`] is that control: the same three numbers, taken
//! either side of a write, disagree. Without it 「the state did not change」 could be said by a
//! measurement that cannot see change at all.
//!
//! Everything here writes to a tmpfs; `support` says why and proves the filesystem type.

mod support;

use gx_adapter_fs::FsAdapter;
use gx_substrate::SubstrateAdapter;
use support::{intent_for, state_of, FsFixture, Sandbox, GOAL, SUBJECT};

/// The acceptance criterion itself.
#[test]
fn ac_047_two_consecutive_plans_agree_and_the_substrate_does_not_move() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let path = sandbox.dir().join(SUBJECT);

    println!(
        "AC_047_ROOT={} FS={}",
        sandbox.dir().display(),
        support::filesystem_of(sandbox.dir())
    );

    let intent = intent_for(&locator, GOAL);
    let pre = adapter
        .snapshot(&locator)
        .expect("the sandbox holds the subject");

    let before = state_of(&path);
    let first = adapter.plan(&intent, &pre).expect("the adapter plans");
    let second = adapter
        .plan(&intent, &pre)
        .expect("the adapter plans again");
    let after = state_of(&path);

    assert_eq!(
        first, second,
        "two consecutive plans over one (intent, snapshot) returned different deltas (FR-042, \
         E-M4-4)"
    );
    assert_eq!(
        before, after,
        "planning moved the substrate: 34 AC-047 asks that 「呼び出し前後でsubstrate状態（ファイル\
         mtime/内容等）が変化しない」, measured as M4-24 fixed it (content digest + mtime + size)"
    );
    println!(
        "AC_047_DELTA_REFERENCE={:?} PAYLOAD_BYTES={}",
        first.reference().cid,
        first.payload().len()
    );
}

/// The control: the measurement can see a change, so its silence above means something.
#[test]
fn the_measurement_moves_when_the_substrate_moves() {
    let sandbox = Sandbox::new();
    let path = sandbox.dir().join(SUBJECT);

    let before = state_of(&path);
    sandbox.write(SUBJECT, b"a different length entirely");
    let after = state_of(&path);

    assert_ne!(
        before.digest, after.digest,
        "the content digest did not move when the content did"
    );
    assert_ne!(before.size, after.size, "the size did not move");
    // mtime is deliberately not asserted to differ: tmpfs timestamps have a resolution, and two
    // writes inside one tick share one mtime. A control that asserted it would be flaky about the
    // clock rather than strict about the state -- which is the same reason M4-24 leaves atime out.
}

/// The pair, not the intent alone: a different pre-state is allowed to plan a different delta.
///
/// This is the half of E-M4-4 that a determinism test can accidentally forbid. FR-042 read without
/// 43 T-2 would say two plans of one intent must agree **always**, and an adapter whose delta
/// depended on the pre-state would then be wrong for depending on its argument.
///
/// For this adapter, v0.1's whole-file replacement is a function of the intent alone, so the two
/// deltas do agree -- and that is recorded here as a *fact about this adapter* rather than as the
/// law, because L1 (the law) is quantified over the pair and lives in the harness.
#[test]
fn the_quantifier_is_the_pair_and_this_adapter_ignores_the_pre_state() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let intent = intent_for(&locator, GOAL);

    let early = adapter.snapshot(&locator).expect("a snapshot");
    sandbox.write(SUBJECT, b"and now something else");
    let late = adapter.snapshot(&locator).expect("a second snapshot");
    assert_ne!(early.digest(), late.digest(), "the snapshots differ");

    let from_early = adapter.plan(&intent, &early).expect("plan");
    let from_late = adapter.plan(&intent, &late).expect("plan");
    assert_eq!(
        from_early, from_late,
        "v0.1 replaces the whole file, so the delta is a function of the goal; if this ever stops \
         being true, L1 in the harness is the law that still holds and this probe is the one to \
         rewrite"
    );
}

/// Planning twice through the fixture the harness uses, so the two roads agree.
#[test]
fn the_fixture_the_harness_uses_plans_the_same_delta_twice() {
    use gx_substrate_conformance::Fixture;

    let fixture = FsFixture::new();
    let adapter = fixture.adapter();
    let locator = fixture.locator();
    let pre = adapter.snapshot(&locator).expect("a snapshot");
    let intent = fixture.intent();

    assert_eq!(
        adapter.plan(&intent, &pre).expect("plan"),
        adapter.plan(&intent, &pre).expect("plan"),
    );
}
