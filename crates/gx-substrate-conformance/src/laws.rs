//! What the rulings added to 51 §7, kept in a section of its own.
//!
//! # The laws the rulings added
//!
//! req/69 §3.4 wrote eight laws and §28 ruled 「全 8 本採用」; §30 M4H2-4 added one more property that is
//! not in the L-list, and `req/38` §58 R-4 added a second. Each row names where it comes from, because
//! a law with no ruling behind it would be a house rule wearing the harness's authority.
//!
//! | 法則 | 内容 | 出自 |
//! |---|---|---|
//! | L1 | `plan` は (intent, snapshot) の対に対して決定的——substrate が動いても同じ対には同じ答え | E-M4-4 / E-M4-29 / AC-047 |
//! | L2 | `apply` の再入(retry)は状態も観測値(`resulting_digest`/`postcondition`)も動かさない | E-M4-3 / 51 §7 契約 7 / 43 T-10c |
//! | L3 | 往復は bit-equal。量化は `invert` に渡した **pre の 1 点**で、その状態を Given に書く | E-M4-3 / AC-049 |
//! | L4 | `precondition` は状態変化を検知し、**変化が無ければ同じ値を返す**(対照つき) | 51 §7 契約 3 / AC-034 |
//! | L5 | `plan` が target を予言したなら `apply(δ).resulting_digest == target` | M4-06 採(b) |
//! | L6 | `commutation(a,b) == commutation(b,a)`・`commutation(a,a)` は `Conflicts` | M4-25 採(a) |
//! | L7 | locator 正規化は冪等で、同値な綴りを 1 つへ写す | E-M4-12 / 42 §2.3 |
//! | K1 | `plan`・`precondition`・`snapshot` の産物の `substrate` はすべて `kind()` | M4H2-4 採(a) |
//! | K2 | `apply` が報告する `postcondition` と、その後の `precondition` が**比較可能**で一致する | §58 R-4 / 51 §7 契約 3 / E-M4-15 |
//!
//! # 🔴 Why K2 exists: fifteen obligations could not see a real defect
//!
//! Every row above and every contract in [`crate::contracts`] compares a value with **another of its
//! own kind**: a `precondition` with a `precondition` (contract 3, L4), a `postcondition` with a
//! `postcondition` (L2), a digest with a digest (contract 1, contract 4). Until this row there was **no
//! obligation that compared one with the other**, and req/99 §4 is what that cost.
//!
//! M7 hand 1 mutated `gx-adapter-git` so that `precondition` named the **entry** as its scope while
//! `apply` named the **branch**, and the mutation survived the whole harness: `RC=0`, fifteen green.
//! It is not a subtle defect. [`gx_core::Fingerprint::cas_eq`] **refuses** to compare two fingerprints
//! of different scopes (**E-M4-15**, **E-M4-27**), so an engine holding that adapter would find 42
//! §3.5's CAS unusable -- an object failing to compare with itself -- and would read the refusal as a
//! wiring bug somewhere else entirely. Fifteen obligations, and the one question that would have
//! noticed was 「are these two of yours about the same thing?」.
//!
//! `req/38` §58 ruled it: 「共有 harness に precondition↔postcondition 突合 obligation を足し、監査手が
//! fs/git/mcp 3 adapter で再検」. This is that obligation, and M7 hand 3 is the hand that runs it against
//! the three.
//!
//! # Where L8 went
//!
//! **L8** (opacity: 「gx-core/canon/gate/witness/log の src に fs delta 文法を読む code が 0」) is not in
//! the table, and its absence is not an omission. It is not a property of an adapter -- the subject
//! of the sentence is the five crates below the boundary -- so running it through [`crate::Fixture`]
//! would ask an adapter to vouch for the code it is supposed to be opaque to. It is measured in
//! `tests/opacity.rs` of this crate, where the subject is the workspace, and a mutation in
//! `tools/verify_m4h3.sh` is what stops 「無い事」 from being vacuous.
//!
//! # Why a law can fail without the adapter being wrong
//!
//! Contracts come from 51 §7 and a failure means M4/M7 are not complete. Laws come from `req/38`,
//! and a failure means an implementation contradicts a decision -- which may equally mean the
//! decision needs revisiting. That is the reason [`crate::Origin`] is printed beside every line
//! rather than inferred from a name (§30 M4H2-1 (a)).

use gx_core::Commutation;

use crate::{unmeasured_or_failed, Check, Fixture, Origin, Outcome};

/// The ids, in table order.
pub const LAW_IDS: [&str; 9] = ["L1", "L2", "L3", "L4", "L5", "L6", "L7", "K1", "K2"];

fn fail(message: impl Into<String>) -> Outcome {
    Outcome::Fail(message.into())
}

/// Run every law.
pub(crate) fn run(fixture: &dyn Fixture) -> Vec<Check> {
    let bodies: [fn(&dyn Fixture) -> Outcome; 9] = [
        law_1_plan_determinism,
        law_2_apply_re_entry,
        law_3_round_trip_at_one_point,
        law_4_precondition_sensitivity,
        law_5_the_prophecy_holds,
        law_6_commutation_is_symmetric,
        law_7_normalisation,
        law_k1_products_name_the_kind,
        law_k2_the_two_fingerprints_are_about_one_thing,
    ];
    LAW_IDS
        .iter()
        .zip(bodies)
        .map(|(id, body)| {
            let outcome = match fixture.reset() {
                Ok(()) => body(fixture),
                Err(e) => fail(format!("the fixture could not reset before this law: {e}")),
            };
            Check::new(id, Origin::Law, outcome)
        })
        .collect()
}

/// **L1** — `plan` answers about the pair it was handed, not about the world it can see.
///
/// **E-M4-29** (§30 M4H2-3 (b)) reads the trait's 「純関数」 as 「(intent, pre) の対に対する決定性+
/// substrate への書き込み 0」 and adds 「読み込みは禁じない——M7 の git adapter が object store を読む道を
/// 閉じない」. The two halves are consistent exactly when reading does not make the answer depend on
/// anything but the pair, and this is where that is measured: the same `(intent, pre)` is planned
/// twice with a disturbance in between, and the deltas have to agree.
///
/// 51 §7's contract 2 asks the weaker question -- two calls in one state -- so this is the law and
/// that is the contract, and neither subsumes the other.
fn law_1_plan_determinism(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let intent = fixture.intent();
    let pre = match adapter.snapshot(&fixture.locator()) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused: {e}")),
    };

    let before = match adapter.plan(&intent, &pre) {
        Ok(d) => d,
        Err(e) => return fail(format!("plan refused: {e}")),
    };
    if let Err(e) = fixture.disturb() {
        return fail(format!("the fixture could not move the substrate: {e}"));
    }
    let after = match adapter.plan(&intent, &pre) {
        Ok(d) => d,
        Err(e) => {
            return fail(format!(
                "plan refused against the same snapshot after the substrate moved: {e}"
            ))
        }
    };

    if before == after {
        Outcome::Pass
    } else {
        fail(
            "the same (intent, snapshot) planned two different deltas once the substrate moved, so \
             the answer depends on something that is not an argument (E-M4-4, E-M4-29)"
                .to_string(),
        )
    }
}

/// **L2** — the retry moves neither the state nor what the adapter reports about it.
///
/// 51 §7's contract 7 asks that the retry be safe; this asks that the *observation* be stable too.
/// The distinction matters for 43 T-10c's recovery: an engine that crashed between the write and
/// the journal record replays the delta and writes what the second `apply` returned, so a
/// `resulting_digest` that differed the second time would put a different digest in the journal
/// than the one the first run produced.
fn law_2_apply_re_entry(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let pre = match adapter.snapshot(&fixture.locator()) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused: {e}")),
    };
    let delta = match adapter.plan(&fixture.intent(), &pre) {
        Ok(d) => d,
        Err(e) => return fail(format!("plan refused: {e}")),
    };

    let first = match adapter.apply(&delta) {
        Ok(a) => a,
        Err(e) => return unmeasured_or_failed(&e, "L2's first apply"),
    };
    let second = match adapter.apply(&delta) {
        Ok(a) => a,
        Err(e) => return unmeasured_or_failed(&e, "L2's retry"),
    };

    if first.resulting_digest() != second.resulting_digest() {
        return fail(format!(
            "the retry reported a different object digest: {:?} then {:?}",
            first.resulting_digest(),
            second.resulting_digest()
        ));
    }
    match first.postcondition().cas_eq(second.postcondition()) {
        Ok(true) => Outcome::Pass,
        Ok(false) => fail(
            "the retry reported a different postcondition fingerprint, so a replay would record a \
             state the first run never saw"
                .to_string(),
        ),
        Err(e) => fail(format!(
            "the two postconditions are not comparable: {e} (E-M4-15 / E-M4-27)"
        )),
    }
}

/// **L3** — the round trip, quantified at the one state it was built for.
///
/// **E-M4-3** is the ruling and req/69 §3.2 is the three-line proof behind it: reading 41 §4's
/// 「適用は冪等」 and AC-049's round trip as laws about a state map at once forces `⟦δ⟧ = id`, so both
/// are narrowed. This law's Given is 「the state `invert` was handed」 and the message carries that
/// state's digest, because 「往復 property の Given に状態を書く事」 is what §28 made a condition -- a
/// property test whose generator moves the state between the two applications falsifies correct
/// adapters, which is M3-05 one milestone later.
///
/// It differs from contract 4 in what 「元に戻る」 is measured over: the contract compares the object's
/// digest, and this compares the **fingerprint**, which 42 §3.5 lets an adapter widen past the object
/// itself. An undo that restored the file and left the scope changed passes the first and fails this.
fn law_3_round_trip_at_one_point(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let locator = fixture.locator();

    // Given: this state.
    let pre = match adapter.snapshot(&locator) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused: {e}")),
    };
    let given = match adapter.precondition(&pre) {
        Ok(f) => f,
        Err(e) => return fail(format!("precondition refused: {e}")),
    };

    let delta = match adapter.plan(&fixture.intent(), &pre) {
        Ok(d) => d,
        Err(e) => return fail(format!("plan refused: {e}")),
    };
    let inverse = match adapter.invert(&delta, &pre) {
        Ok(Some(i)) => i,
        Ok(None) => {
            return Outcome::NotSupplied(format!(
                "invert answered Ok(None) at the Given state (scope {:?}), so the law has no \
                 instance here",
                given.scope()
            ))
        }
        Err(e) => return unmeasured_or_failed(&e, "L3's escrow step"),
    };

    if let Err(e) = adapter.apply(&delta) {
        return unmeasured_or_failed(&e, "L3's apply");
    }
    if let Err(e) = adapter.apply(&inverse) {
        return unmeasured_or_failed(&e, "L3's inverse apply");
    }

    let after = match adapter
        .snapshot(&locator)
        .and_then(|s| adapter.precondition(&s))
    {
        Ok(f) => f,
        Err(e) => return fail(format!("precondition refused after the round trip: {e}")),
    };
    match given.cas_eq(&after) {
        Ok(true) => Outcome::Pass,
        Ok(false) => fail(format!(
            "the round trip did not return to its Given (scope {:?}, digest {:?}); the quantifier \
             is the one state `invert` was handed and this is that state",
            given.scope(),
            given.digest()
        )),
        Err(e) => fail(format!(
            "the fingerprint after the round trip is not comparable with the Given: {e}"
        )),
    }
}

/// **L4** — the fingerprint moves when the state moves, and holds still when it does not.
///
/// 51 §7's contract 3 is the first half. The control is what stops an adapter from passing it with a
/// counter: a `precondition` that returned a fresh value on every call would 「detect」 every change
/// and every non-change alike, and CON-2's CAS would abort commits nobody interfered with.
fn law_4_precondition_sensitivity(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let locator = fixture.locator();
    let read = || {
        adapter
            .snapshot(&locator)
            .and_then(|s| adapter.precondition(&s))
    };

    let first = match read() {
        Ok(f) => f,
        Err(e) => return fail(format!("precondition refused: {e}")),
    };
    let again = match read() {
        Ok(f) => f,
        Err(e) => return fail(format!("the second precondition refused: {e}")),
    };
    match first.cas_eq(&again) {
        Ok(true) => {}
        Ok(false) => return fail(
            "two reads of an unchanged substrate disagree, so every CAS check would abort on a \
                 change nobody made"
                .to_string(),
        ),
        Err(e) => return fail(format!("the two reads are not comparable: {e}")),
    }

    if let Err(e) = fixture.disturb() {
        return fail(format!("the fixture could not move the substrate: {e}"));
    }
    let moved = match read() {
        Ok(f) => f,
        Err(e) => return fail(format!("precondition refused after a change: {e}")),
    };
    match first.cas_eq(&moved) {
        Ok(false) => Outcome::Pass,
        Ok(true) => fail("the substrate moved and the fingerprint did not".to_string()),
        Err(e) => fail(format!("the two reads are not comparable: {e}")),
    }
}

/// **L5** — an adapter that promises a post-state has to reach it.
///
/// **M4-06 採(b)**: 「M4 は「adapter の自己整合」を conformance property で強制——`plan` が `target` を
/// 予言したら `apply(δ).resulting_digest == target`(L5・harness 収載=手5)。engine 側の拒否と
/// `AbortReason` 拡張は **M5 起票**」. req/69 §4 M4-06 is why it exists at all: `resulting_digest` has
/// exactly one mention in the whole canon (42 §3.4's field table) and nothing compares it with the
/// `target` a plan promised, so 「post は返り値でなく観測値」 was an observation nobody looked at.
///
/// The prophecy is the fixture's because it is `Transformation.target` (41 §3), an engine-side value.
fn law_5_the_prophecy_holds(fixture: &dyn Fixture) -> Outcome {
    let Some(target) = fixture.promised_target() else {
        return Outcome::NotSupplied(
            "the fixture promises no target for its intent, so there is no prophecy to check \
             (M4-06 採(b); the engine-side refusal is M5's)"
                .to_string(),
        );
    };
    let adapter = fixture.adapter();
    let pre = match adapter.snapshot(&fixture.locator()) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused: {e}")),
    };
    let delta = match adapter.plan(&fixture.intent(), &pre) {
        Ok(d) => d,
        Err(e) => return fail(format!("plan refused: {e}")),
    };
    match adapter.apply(&delta) {
        Ok(applied) if applied.resulting_digest() == &target => Outcome::Pass,
        Ok(applied) => fail(format!(
            "the plan promised {target:?} and the apply observed {:?}",
            applied.resulting_digest()
        )),
        Err(e) => unmeasured_or_failed(&e, "L5's apply"),
    }
}

/// **L6** — independence is a relation between two deltas, not a view one has of the other.
///
/// **M4-25 採(a)+反射=Conflicts** (§28): 「`commutation` の**対称性を契約に**足し L6 property で測る。
/// `commutation(a,a)` は **`Conflicts`**(同一資源への二重干渉=保守側 fail-closed・DPO 並列独立性は同一
/// match で一般に不成立)」. 41 §4 and 51 §7 both leave the relation between `commutation(a,b)` and
/// `commutation(b,a)` unwritten, which req/69 §4 M4-25 measured; ASM-2's DPO parallel independence is
/// symmetric, so the asymmetric reading was never the intended one.
fn law_6_commutation_is_symmetric(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let mut pairs: Vec<(&str, _)> = Vec::new();
    if let Some(p) = fixture.commuting_pair() {
        pairs.push(("independent", p));
    }
    if let Some(p) = fixture.conflicting_pair() {
        pairs.push(("dependent", p));
    }
    if pairs.is_empty() {
        return Outcome::NotSupplied(
            "the fixture supplies neither pair, so symmetry has nothing to be symmetric about"
                .to_string(),
        );
    }

    for (name, (a, b)) in &pairs {
        let forward = match adapter.commutation(a, b) {
            Ok(c) => c,
            Err(e) => return unmeasured_or_failed(&e, &format!("L6 on the {name} pair")),
        };
        let backward = match adapter.commutation(b, a) {
            Ok(c) => c,
            Err(e) => return unmeasured_or_failed(&e, &format!("L6 on the reversed {name} pair")),
        };
        // The two answers agree as *answers*, not as values: `Conflicts` carries a residual, and
        // M4-25 says nothing about the two directions naming the same obstruction. Comparing the
        // residuals would be a stronger law than the ruling made.
        let agree = matches!(
            (&forward, &backward),
            (Commutation::Commutes, Commutation::Commutes)
                | (Commutation::Conflicts { .. }, Commutation::Conflicts { .. })
        );
        if !agree {
            return fail(format!(
                "the {name} pair answers {forward:?} one way and {backward:?} the other"
            ));
        }
    }

    // Reflexive: a delta and itself touch the same resource, and fail-closed is the conservative
    // side. Taken from the fixture's own delta rather than from a pair, so that it is measured even
    // when only one pair is supplied.
    let (a, _) = &pairs[0].1;
    match adapter.commutation(a, a) {
        Ok(Commutation::Conflicts { .. }) => Outcome::Pass,
        Ok(Commutation::Commutes) => fail(
            "a delta commutes with itself: M4-25 fixes the reflexive case at `Conflicts`, because \
             two applications of one change to one resource are not parallel-independent"
                .to_string(),
        ),
        Err(e) => unmeasured_or_failed(&e, "L6 on (a, a)"),
    }
}

/// **L7** — the normalisation is a choice of representative, and choosing twice changes nothing.
///
/// **E-M4-12** made the normalisation lexical and `crates/gx-substrate/src/lib.rs`'s
/// `# Locator normalisation (normative)` section is its five clauses. The property is req/69 §3.4's:
/// `normalize(normalize(l)) == normalize(l)` and `l ≈ l' → normalize(l) == normalize(l')`, which
/// req/69 §3.4 notes is T3's shape one layer over -- gx-canon does it for values and an adapter does
/// it for positions.
fn law_7_normalisation(fixture: &dyn Fixture) -> Outcome {
    let locator = fixture.locator();
    let Some(once) = fixture.normalise(&locator) else {
        return Outcome::NotSupplied(
            "the fixture exposes no normalisation, so E-M4-12's clauses are unmeasured for this \
             adapter (the fs one is hand 4's)"
                .to_string(),
        );
    };
    let Some(twice) = fixture.normalise(&once) else {
        return fail("normalising a normalised locator produced nothing".to_string());
    };
    if once != twice {
        return fail(format!(
            "normalisation is not idempotent: {locator:?} -> {once:?} -> {twice:?}"
        ));
    }

    let spellings = fixture.equivalent_spellings();
    if spellings.is_empty() {
        return Outcome::NotSupplied(
            "normalisation is idempotent, but the fixture names no equivalent spellings, so the \
             second half of L7 (「同値写像」, 42 §2.3's `≈`) is unmeasured"
                .to_string(),
        );
    }
    for (left, right) in spellings {
        let (Some(l), Some(r)) = (fixture.normalise(&left), fixture.normalise(&right)) else {
            return fail(format!(
                "normalising one of the equivalent spellings {left:?} / {right:?} produced nothing"
            ));
        };
        if l != r {
            return fail(format!(
                "{left:?} and {right:?} are called equivalent and normalise to {l:?} and {r:?}; \
                 M3-10 fixed the gate's effective range at locator 級, so two spellings of one \
                 subject are two policy subjects"
            ));
        }
    }
    Outcome::Pass
}

/// **K1** — everything an adapter produces names the substrate the adapter speaks for.
///
/// **§30 M4H2-4 採(a)**: 「「plan/precondition の産物の `substrate` == `kind()`」を harness の契約
/// property に(手3・adapter 非依存で全 adapter に効く)」. req/71 §2 M4H2-4 is the defect it closes:
/// `PlannedDelta::new` and `Fingerprint::new` both take a `SubstrateKind`, so an adapter can build a
/// delta naming somebody else's grammar, and **E-M4-27**'s refusal only fires later -- as 「this
/// adapter's own fingerprints are not comparable with each other」, which reads as a wiring bug
/// somewhere else entirely.
fn law_k1_products_name_the_kind(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let kind = adapter.kind();

    let snapshot = match adapter.snapshot(&fixture.locator()) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused: {e}")),
    };
    if snapshot.substrate() != &kind {
        return fail(format!(
            "the snapshot names {:?} and `kind()` is {kind:?}",
            snapshot.substrate()
        ));
    }

    let fingerprint = match adapter.precondition(&snapshot) {
        Ok(f) => f,
        Err(e) => return fail(format!("precondition refused: {e}")),
    };
    if fingerprint.substrate() != &kind {
        return fail(format!(
            "the fingerprint names {:?} and `kind()` is {kind:?}",
            fingerprint.substrate()
        ));
    }

    let delta = match adapter.plan(&fixture.intent(), &snapshot) {
        Ok(d) => d,
        Err(e) => return fail(format!("plan refused: {e}")),
    };
    if delta.substrate() != &kind {
        return fail(format!(
            "the delta names {:?} and `kind()` is {kind:?}",
            delta.substrate()
        ));
    }
    if delta.reference().substrate != kind {
        return fail(format!(
            "the delta's own reference names {:?} and `kind()` is {kind:?}",
            delta.reference().substrate
        ));
    }
    Outcome::Pass
}

/// **K2** — the fingerprint `apply` reports and the fingerprint `precondition` computes are about one
/// thing, and they agree.
///
/// **`req/38` §58 R-4**, from req/99 §4's survivor. The module documentation carries the argument; what
/// follows is the shape of the measurement, and both halves of it are load-bearing:
///
/// * **`Err` is a failure, not a skip.** [`gx_core::Fingerprint::cas_eq`] answers `Err` when the two
///   values name different substrates or different scopes -- 「その比較は意味を持たない」 (**E-M4-15**,
///   **E-M4-27**). For two fingerprints of **one object taken by one adapter**, that answer is not a
///   fact about the question: it is the adapter contradicting itself, and 42 §3.5's CAS is unusable
///   for it. This is the arm the git mutation lands in.
/// * **`Ok(false)` is a failure too.** An adapter whose `apply` reported a state the substrate is not
///   in afterwards would satisfy every same-kind comparison in the harness -- L2 compares two
///   `postcondition`s with each other and both would be equally wrong -- and would put a fingerprint
///   into a receipt (42 §3.4) that no reader could reproduce.
///
/// The observation is taken through `snapshot` + `precondition`, which is the road **an engine** takes
/// for the next transformation's CAS (41 §5-5b). So what this law asks is exactly the question the
/// next commit will ask, one moment earlier.
///
/// It is a **law** and not a contract because 51 §7 does not write it: the seven rows are copied cell
/// for cell in [`crate::contracts`] and adding an eighth would make 「上記7契約」 mean eight things
/// quietly (§30 M4H2-1 (a)). Its ruling is §58's.
fn law_k2_the_two_fingerprints_are_about_one_thing(fixture: &dyn Fixture) -> Outcome {
    let adapter = fixture.adapter();
    let locator = fixture.locator();

    let pre = match adapter.snapshot(&locator) {
        Ok(s) => s,
        Err(e) => return fail(format!("snapshot refused: {e}")),
    };
    let delta = match adapter.plan(&fixture.intent(), &pre) {
        Ok(d) => d,
        Err(e) => return fail(format!("plan refused: {e}")),
    };
    let applied = match adapter.apply(&delta) {
        Ok(a) => a,
        Err(e) => return unmeasured_or_failed(&e, "K2's apply"),
    };

    let observed = match adapter
        .snapshot(&locator)
        .and_then(|s| adapter.precondition(&s))
    {
        Ok(f) => f,
        Err(e) => return fail(format!("precondition refused after the apply: {e}")),
    };

    match applied.postcondition().cas_eq(&observed) {
        Ok(true) => Outcome::Pass,
        Ok(false) => fail(format!(
            "`apply` reported the fingerprint {:?} and a `precondition` taken straight after it \
             answers {:?}. The two are the adapter's own values about one object, so an engine \
             would CAS the next change against a state this one says it did not leave",
            applied.postcondition().digest(),
            observed.digest()
        )),
        Err(e) => fail(format!(
            "the fingerprint `apply` reported and the one `precondition` computes are **not \
             comparable**: {e}. Both are this adapter's, about this object, one moment apart -- so \
             42 §3.5's CAS (41 §5-5b) cannot be run for it at all, and E-M4-15's refusal would \
             reach an engine as a wiring bug somewhere else. `apply` scope {:?} / `precondition` \
             scope {:?}",
            applied.postcondition().scope(),
            observed.scope()
        )),
    }
}
