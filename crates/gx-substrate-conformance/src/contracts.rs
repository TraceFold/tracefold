//! 51 §7's seven, one to one.
//!
//! # The seven contracts of 51 §7
//!
//! Copied out of `req/spec/50-delivery/51-test-strategy.md` cell for cell rather than paraphrased,
//! because 51 §7's completion condition -- 「各adapterは上記7契約すべてに合格しない限りM4/M7完了条件を
//! 満たさない」 -- counts to seven and 「上記7契約」 has to stay resolvable.
//! `tests/harness_shape.rs` compares this table with the canon's on every run.
//!
//! | 契約 | 検証内容 | 対応AC |
//! |---|---|---|
//! | snapshot | `snapshot(locator)`が対象substrateの現在状態を正しく反映したObjectSnapshotを返す | AC-049〜051の前提 |
//! | plan | 純関数性（副作用なし・同一intentから同一PlannedDelta） | AC-047 |
//! | precondition | `precondition`が状態変化を検知できるFingerprintを返す（変更前後で異なる値） | AC-034の前提 |
//! | apply/invert往復 | `apply(delta)`→`invert`→`apply(inverse)`で対象状態が元に戻る | AC-049, AC-050 |
//! | invertのNone契約 | 逆構成不能時`Ok(None)`を返しgate側が扱いを変える | AC-048 |
//! | commutation | 可換/非可換2ケースがそれぞれ`Commutes`/`Conflicts{residual}` | AC-052 |
//! | apply冪等性 | `apply`の再試行が安全（41 §4「適用は冪等に設計する」、43 T-10c根拠） | crash-recovery整合 |
//!
//! # What a shared harness can and cannot decide
//!
//! Contract 1 asks whether a snapshot 「正しく反映した」 the substrate. No adapter-independent code can
//! answer that: reading the substrate to check is exactly what P-6 forbids the layers above an
//! adapter to do, and a harness that read it would need the adapter's grammar. What is decidable is
//! the part that is falsifiable without a second oracle -- the snapshot names this adapter's
//! substrate and the locator it was asked about, and it **moves when the substrate moves**. A
//! snapshot that never changed would satisfy every one-moment assertion, so the disturbance is what
//! makes contract 1 a contract. The residue is written down here rather than smoothed over.
//!
//! # 🔴 Contract 4 is run in 43 T-10b's order, and 51 §7 writes another
//!
//! 51 §7's cell is 「`apply(delta)`→`invert`→`apply(inverse)`で対象状態が元に戻る」 -- invert **after**
//! apply. 43 T-10b puts the call before it: the inverse is constructed and escrowed while the
//! pre-state is still live, and 42 §5 requires the escrowed body 「digest-onlyでは実際のundoが物理的に
//! 不可能なため」.
//!
//! The difference is not cosmetic. An inverse of an overwrite has to carry the **old** content
//! (M4-21), and after `apply` the old content is gone -- an adapter asked to invert then would have
//! to read a state that no longer exists. So 51 §7's literal order can only be satisfied by an
//! adapter that keeps its own history, which no ruling requires. This harness therefore runs
//! `snapshot → plan → invert(δ, pre) → apply(δ) → apply(δ⁻¹)` and prints `ORDER=T-10b`, and the
//! divergence is raised as a起票 in `req/72` §2 rather than decided here. req/69 §4 M4-21 saw the same
//! seam from the other side: 「逐語例は `invert` が apply の**前**(43 T-10b)に呼ばれる事と噛み合って
//! いない」.

use gx_core::Commutation;

use crate::{unmeasured_or_failed, Check, Fixture, Origin, Outcome};

/// The seven ids, which are 51 §7's first column.
///
/// Declared here and compared with the table above by `tests/harness_shape.rs`, so that a report
/// line and a canon row can be matched by a reader without a translation table.
pub const CONTRACT_IDS: [&str; 7] = [
    "snapshot",
    "plan",
    "precondition",
    "apply/invert往復",
    "invertのNone契約",
    "commutation",
    "apply冪等性",
];

fn fail(message: impl Into<String>) -> Outcome {
    Outcome::Fail(message.into())
}

/// Run all seven, in 51 §7's order.
pub(crate) fn run(fixture: &dyn Fixture) -> Vec<Check> {
    let bodies: [fn(&dyn Fixture) -> Outcome; 7] = [
        contract_snapshot,
        contract_plan,
        contract_precondition,
        contract_round_trip,
        contract_invert_none,
        contract_commutation,
        contract_apply_idempotence,
    ];
    CONTRACT_IDS
        .iter()
        .zip(bodies)
        .map(|(id, body)| {
            let outcome = match fixture.reset() {
                Ok(()) => body(fixture),
                Err(e) => fail(format!(
                    "the fixture could not reset before this check: {e}"
                )),
            };
            Check::new(id, Origin::Contract, outcome)
        })
        .collect()
}

/// 1. `snapshot(locator)` reports this adapter's view of the state it was asked about.
fn contract_snapshot(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let locator = fixture.locator();

    let before = match adapter.snapshot(&locator) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot({locator:?}) refused: {e}")),
    };
    if before.substrate() != &adapter.kind() {
        return fail(format!(
            "the snapshot names {:?} and the adapter is {:?}",
            before.substrate(),
            adapter.kind()
        ));
    }
    if before.locator() != locator {
        return fail(format!(
            "asked about {locator:?} and the snapshot names {:?}",
            before.locator()
        ));
    }

    if let Err(e) = fixture.disturb() {
        return fail(format!("the fixture could not move the substrate: {e}"));
    }
    let after = match adapter.snapshot(&locator) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot({locator:?}) refused after a change: {e}")),
    };
    if after.digest() == before.digest() {
        return fail(
            "the substrate moved and the snapshot did not: 「現在状態を正しく反映した」 is the one \
             half of contract 1 a shared harness can decide, and it is this one"
                .to_string(),
        );
    }
    Outcome::Pass
}

/// 2. `plan` is a function of its arguments and leaves the substrate alone.
fn contract_plan(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let locator = fixture.locator();
    let intent = fixture.intent();

    let pre = match adapter.snapshot(&locator) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused: {e}")),
    };
    let before = match adapter.precondition(&pre) {
        Ok(f) => f,
        Err(e) => return fail(format!("precondition refused: {e}")),
    };

    let first = match adapter.plan(&intent, &pre) {
        Ok(d) => d,
        Err(e) => return fail(format!("plan refused: {e}")),
    };
    let second = match adapter.plan(&intent, &pre) {
        Ok(d) => d,
        Err(e) => return fail(format!("the second plan refused: {e}")),
    };
    if first != second {
        return fail(
            "two plans over the same (intent, snapshot) disagree: 51 §7 契約 2 「同一intentから同一\
             PlannedDelta」, quantified over the pair by E-M4-4"
                .to_string(),
        );
    }

    let after_snapshot = match adapter.snapshot(&locator) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused after planning: {e}")),
    };
    let after = match adapter.precondition(&after_snapshot) {
        Ok(f) => f,
        Err(e) => return fail(format!("precondition refused after planning: {e}")),
    };
    match before.cas_eq(&after) {
        Ok(true) => Outcome::Pass,
        Ok(false) => fail(
            "planning moved the substrate: 51 §7 契約 2 「副作用なし」, and E-M4-29 reads it as \
             「substrate への書き込み 0」"
                .to_string(),
        ),
        Err(e) => fail(format!(
            "the two fingerprints around `plan` are not comparable: {e}"
        )),
    }
}

/// 3. `precondition` answers differently when the state is different.
fn contract_precondition(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let locator = fixture.locator();

    let before = match adapter
        .snapshot(&locator)
        .and_then(|s| adapter.precondition(&s))
    {
        Ok(f) => f,
        Err(e) => return fail(format!("precondition refused: {e}")),
    };
    if let Err(e) = fixture.disturb() {
        return fail(format!("the fixture could not move the substrate: {e}"));
    }
    let after = match adapter
        .snapshot(&locator)
        .and_then(|s| adapter.precondition(&s))
    {
        Ok(f) => f,
        Err(e) => return fail(format!("precondition refused after a change: {e}")),
    };

    match before.cas_eq(&after) {
        Ok(false) => Outcome::Pass,
        Ok(true) => fail(
            "the substrate moved and the fingerprint did not: a `precondition` that never changes \
             makes the CAS check of 41 §5-5b unfalsifiable (51 §7 契約 3, AC-034 の前提)"
                .to_string(),
        ),
        Err(e) => fail(format!(
            "the two fingerprints of one scope are not comparable: {e}"
        )),
    }
}

/// 4. Apply and undo, and the object is where it started. Run in 43 T-10b's order (see the module
///    documentation).
fn contract_round_trip(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let locator = fixture.locator();
    let intent = fixture.intent();

    let pre = match adapter.snapshot(&locator) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused: {e}")),
    };
    let delta = match adapter.plan(&intent, &pre) {
        Ok(d) => d,
        Err(e) => return fail(format!("plan refused: {e}")),
    };

    // ORDER=T-10b: the inverse is constructed while the pre-state is still live.
    let inverse =
        match adapter.invert(&delta, &pre) {
            Ok(Some(i)) => i,
            Ok(None) => return Outcome::NotSupplied(
                "invert returned Ok(None) for the fixture's own delta, so there is no round trip \
                 to run (ORDER=T-10b)"
                    .to_string(),
            ),
            Err(e) => return unmeasured_or_failed(&e, "the round trip's escrow step"),
        };

    if let Err(e) = adapter.apply(&delta) {
        return unmeasured_or_failed(&e, "the round trip's apply");
    }
    if let Err(e) = adapter.apply(&inverse) {
        return unmeasured_or_failed(&e, "applying the inverse");
    }

    let after = match adapter.snapshot(&locator) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused after the round trip: {e}")),
    };
    if after.digest() == pre.digest() {
        Outcome::Pass
    } else {
        fail(format!(
            "the object did not come back (ORDER=T-10b): started at {:?} and ended at {:?}",
            pre.digest(),
            after.digest()
        ))
    }
}

/// 5. When an inverse cannot be built, the answer is `Ok(None)` rather than an error.
fn contract_invert_none(fixture: &dyn Fixture) -> Outcome {
    let Some((delta, pre)) = fixture.uninvertible() else {
        return Outcome::NotSupplied(
            "the fixture supplies no delta whose inverse cannot be constructed, so AC-048's \
             `Ok(None)` path is unmeasured for this adapter"
                .to_string(),
        );
    };
    match fixture.adapter().invert(&delta, &pre) {
        Ok(None) => Outcome::Pass,
        Ok(Some(_)) => fail(
            "the adapter built an inverse for the delta the fixture calls uninvertible; one of the \
             two is wrong and the harness cannot tell which"
                .to_string(),
        ),
        Err(e) => unmeasured_or_failed(
            &e,
            "invert answered with an error rather than `Ok(None)` (41 §4 separates 「the question              cannot be answered」 from 「the answer is no inverse」, and E-M3-4 escalates on the              second)",
        ),
    }
}

/// 6. Independent deltas commute and dependent ones name the obstruction.
fn contract_commutation(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let (Some((a, b)), Some((c, d))) = (fixture.commuting_pair(), fixture.conflicting_pair())
    else {
        return Outcome::NotSupplied(
            "the fixture supplies fewer than both of 51 §7's two cases (可換/非可換)".to_string(),
        );
    };

    match adapter.commutation(&a, &b) {
        Ok(Commutation::Commutes) => {}
        Ok(Commutation::Conflicts { residual }) => {
            return fail(format!(
                "the pair the fixture calls independent conflicts, residual {residual:?}"
            ))
        }
        Err(e) => return unmeasured_or_failed(&e, "commutation on the independent pair"),
    }
    match adapter.commutation(&c, &d) {
        Ok(Commutation::Conflicts { .. }) => Outcome::Pass,
        Ok(Commutation::Commutes) => fail(
            "the pair the fixture calls dependent commutes: 43 §8 stops on `Conflicts`, so a false \
             `Commutes` is the fail-open direction"
                .to_string(),
        ),
        Err(e) => unmeasured_or_failed(&e, "commutation on the dependent pair"),
    }
}

/// 7. Running the same delta again is safe.
fn contract_apply_idempotence(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let locator = fixture.locator();
    let intent = fixture.intent();

    let pre = match adapter.snapshot(&locator) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused: {e}")),
    };
    let delta = match adapter.plan(&intent, &pre) {
        Ok(d) => d,
        Err(e) => return fail(format!("plan refused: {e}")),
    };

    if let Err(e) = adapter.apply(&delta) {
        return unmeasured_or_failed(&e, "the first apply");
    }
    let once = match adapter.snapshot(&locator) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused after the first apply: {e}")),
    };
    if let Err(e) = adapter.apply(&delta) {
        return unmeasured_or_failed(
            &e,
            "the retry (43 T-10c is a crash between the write and the journal record, and recovery              means running the same delta again)",
        );
    }
    let twice = match adapter.snapshot(&locator) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused after the retry: {e}")),
    };

    if once.digest() == twice.digest() {
        Outcome::Pass
    } else {
        fail(format!(
            "the retry moved the object: {:?} became {:?}. The quantifier is 「同一 delta の再入」 \
             (E-M4-3), which is the one reading under which 41 §4's 「適用は冪等」 and AC-049's round \
             trip can both hold (req/69 §3.2)",
            once.digest(),
            twice.digest()
        ))
    }
}
