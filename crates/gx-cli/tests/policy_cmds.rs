//! 🔴 `gx policy lint` / `gx policy test` (44 §1.2) — and **M6-21's three accessors, called**.
//!
//! §41 M5H4-8 armed `Gate::policies`, `Gate::invariants` and `PolicyEngine::is_empty` with a kill
//! condition, req/88 §4 M6-21 named `gx policy lint` as the consumer, and req/38 §47 採(a) dissolved
//! the condition on one further condition: 「名指しただけでは消費者にならない——該当手の DoD に『3
//! accessor それぞれの呼び出し箇所を機械で数え、0 なら RED』を含める」.
//!
//! `probes/doubt/tests/m6_surface_doubt.rs` counts the call sites. This measures that the calls do
//! something an operator can read — a count of statements, a list of ids, the invariant half, and
//! the warning that a fail-closed deployment came up with **no rule at all**.

mod support;

use support::{deny_writable_pack, gx, run, scratch, write_json};

/// The shipped pack, at the path 34 AC-025 writes.
fn shipped_pack() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-cli")
        .join(gx_gate::packs::FS_PACK_PATH)
}

/// 🔴 `gx policy lint` on the shipped pack: two statements, named, and the invariant half reported.
#[test]
fn lint_reports_both_halves_of_a_verdict() {
    let linted = run(gx().arg("policy").arg("lint").arg(shipped_pack()));
    println!(
        "POLICY_LINT exit={} policies={:?} ids={:?} invariants={:?} warnings={}",
        linted.code,
        linted.json()["policies"]["count"],
        linted.json()["policies"]["ids"],
        linted.json()["invariants"]["count"],
        linted.json()["warnings"]
            .as_array()
            .map_or(0, std::vec::Vec::len)
    );
    assert_eq!(
        linted.code, 0,
        "44 §1.2 `gx policy lint`: 「0=問題なし」. stderr: {}",
        linted.stderr
    );
    // `Gate::policies()` — the first armed accessor, doing the job M6-21's reason gave it.
    assert_eq!(
        linted.json()["policies"]["count"],
        2,
        "the shipped pack is two statements (`packs::FS_PACK_POLICY_IDS`)"
    );
    let ids = linted.json()["policies"]["ids"].clone();
    assert!(
        ids.as_array()
            .is_some_and(|v| v.iter().any(|i| i == "fs-deny-etc")),
        "and it names them, because ASM-62-1 makes the id the thing a receipt records: {ids}"
    );
    // `Gate::invariants()` — the second. FR-027 composes policy **and** invariant, and D-9 ships no
    // ready-made invariant with the fs pack, so the honest report is 「zero, and here is why」.
    assert_eq!(
        linted.json()["invariants"]["count"],
        0,
        "D-9: no ready-made invariant ships with the fs pack"
    );
    let warnings = linted.json()["warnings"].clone();
    assert!(
        warnings
            .as_array()
            .is_some_and(|w| w.iter().any(|s| s.as_str().is_some_and(|s| s.contains("FR-027")))),
        "🔴 a linter that checked the policy half and said nothing would misreport what it checked \
         (M4H4-2 in a linter's clothes): {warnings}"
    );
}

/// 🔴 `PolicyEngine::is_empty` — the third accessor, and the warning it exists for.
///
/// 「policy 0 本で立った」 is the most dangerous configuration a fail-closed deployment can be in.
/// Cedar is default-deny, so an empty set is not a permissive gate — it is a gate that **admits
/// nothing at all**, and a deployment that came up that way would refuse every change with no rule
/// to point at. The status stays 0 because 44 §1.2's 1 is 「lint/testエラーあり」 and this is not an
/// error; it is the thing an operator has to be told.
#[test]
fn an_empty_pack_is_a_warning_and_not_a_silence() {
    let dir = scratch("m6h4_policy_empty");
    let empty = dir.join("empty.cedar");
    std::fs::write(&empty, "// no statements\n").expect("write");

    let linted = run(gx().arg("policy").arg("lint").arg(&empty));
    println!(
        "POLICY_LINT_EMPTY exit={} is_empty={:?} warnings={:?}",
        linted.code,
        linted.json()["policies"]["is_empty"],
        linted.json()["warnings"]
    );
    assert_eq!(
        linted.code, 0,
        "a warning is not an error: {}",
        linted.stderr
    );
    assert_eq!(linted.json()["policies"]["count"], 0);
    assert_eq!(linted.json()["policies"]["is_empty"], true);
    let warnings = linted.json()["warnings"].to_string();
    assert!(
        warnings.contains("default-deny"),
        "🔴 M6-21: 「policy 0 本で立った」の起動時警告. Without it, the most dangerous configuration \
         is the one that prints nothing: {warnings}"
    );
}

/// A pack that will not parse is a **refusal**, not a diagnostic line.
///
/// There is nothing to report about a set that does not exist, and 44 §1.2 gives this command a 1.
#[test]
fn a_pack_that_does_not_parse_exits_one() {
    let dir = scratch("m6h4_policy_broken");
    let broken = dir.join("broken.cedar");
    std::fs::write(&broken, "permit(principal, action, resource;\n").expect("write");

    let linted = run(gx().arg("policy").arg("lint").arg(&broken));
    println!("POLICY_LINT_BROKEN exit={}", linted.code);
    assert_eq!(linted.code, 1, "44 §1.2: 「1=lint/testエラーあり」");
    assert!(
        linted.stdout.trim().is_empty(),
        "44 §1.3: a refusal writes nothing to stdout"
    );
}

/// 🔴 A statement with no `@id` is refused **at load** (ASM-62-1 / C-4).
///
/// `PolicySet::from_str` names policies by position, that id lands in `PolicyDecisionRecord`, and
/// 42 §1.3 puts the record inside `AdmitProof`'s identity view — so swapping two statements in a
/// file would change what a receipt says without changing what was decided. That is why the refusal
/// is at load and not at the first request, and `gx policy lint` is where an operator meets it.
#[test]
fn a_statement_without_an_id_is_refused() {
    let dir = scratch("m6h4_policy_no_id");
    let anonymous = dir.join("anonymous.cedar");
    std::fs::write(
        &anonymous,
        "permit (principal, action, resource) when { resource.substrate == \"fs\" };\n",
    )
    .expect("write");

    let linted = run(gx().arg("policy").arg("lint").arg(&anonymous));
    println!(
        "POLICY_LINT_NO_ID exit={} detail={:?}",
        linted.code,
        linted.stderr.trim()
    );
    assert_eq!(
        linted.code, 1,
        "ASM-62-1: a pack without `@id` is not loadable by gx"
    );
}

/// 🔴 `gx policy test` — 「指定シナリオ…に対しGate評価を実行し期待値と照合」.
///
/// Both of AC-025's cases against the shipped pack, plus the case the fixture pack exists for, plus
/// a **failing** expectation — because a runner that could only report agreement would pass on a
/// pack that admitted everything.
#[test]
fn test_runs_scenarios_and_reports_disagreement() {
    let dir = scratch("m6h4_policy_test");
    let scenarios = write_json(
        &dir.join("scenarios.json"),
        &serde_json::json!([
            {
                "name": "AC-025: /etc/passwd is denied",
                "substrate": "fs",
                "locator": "/etc/passwd",
                "context": "Policy",
                "actor_key": "key-1",
                "expect": "Deny"
            },
            {
                "name": "AC-025: /tmp/x is admitted",
                "substrate": "fs",
                "locator": "/tmp/x",
                "context": "Policy",
                "actor_key": "key-1",
                "expect": "Admit"
            },
            {
                "name": "E-M3-4: no inverse escalates",
                "substrate": "fs",
                "locator": "/tmp/x",
                "context": "Policy",
                "actor_key": "key-1",
                "invert_available": false,
                "expect": "Escalate"
            }
        ]),
    );

    let tested = run(gx()
        .arg("policy")
        .arg("test")
        .arg(shipped_pack())
        .arg("--scenario")
        .arg(&scenarios));
    println!(
        "POLICY_TEST exit={} passed={:?} failed={:?}",
        tested.code,
        tested.json()["passed"],
        tested.json()["failed"]
    );
    assert_eq!(
        tested.code, 0,
        "44 §1.2: 「0=問題なし」. cases: {}",
        tested.stdout
    );
    assert_eq!(tested.json()["passed"], 3);
    assert_eq!(tested.json()["failed"], 0);

    // 🔴 The negative half. Without it this suite would pass against a runner that answered 「pass」
    // to everything, which is the shape 「正の対照だけを置くな」 keeps catching.
    let wrong = write_json(
        &dir.join("wrong.json"),
        &serde_json::json!([{
            "name": "the shipped pack does not admit /etc",
            "substrate": "fs",
            "locator": "/etc/passwd",
            "context": "Policy",
            "actor_key": "key-1",
            "expect": "Admit"
        }]),
    );
    let failed = run(gx()
        .arg("policy")
        .arg("test")
        .arg(shipped_pack())
        .arg("--scenario")
        .arg(&wrong));
    println!(
        "POLICY_TEST_MISMATCH exit={} cases={}",
        failed.code, failed.stdout
    );
    assert_eq!(failed.code, 1, "44 §1.2: 「1=lint/testエラーあり」");
    assert_eq!(failed.json()["failed"], 1);
    assert_eq!(failed.json()["cases"][0]["actual"], "Deny");
    assert_eq!(failed.json()["cases"][0]["pass"], false);
}

/// 🔴 The fixture pack of **M6H3-9 採(a)**, linted and exercised.
///
/// It has to be a pack like any other — two statements with ids, a locator its forbid matches — or
/// the record-only E2E it exists for would be measuring a fixture rather than DR-2.
#[test]
fn the_test_fixture_pack_is_a_pack() {
    let pack = deny_writable_pack();
    let linted = run(gx().arg("policy").arg("lint").arg(&pack));
    println!(
        "FIXTURE_PACK_LINT exit={} ids={:?}",
        linted.code,
        linted.json()["policies"]["ids"]
    );
    assert_eq!(linted.code, 0, "stderr: {}", linted.stderr);
    assert_eq!(linted.json()["policies"]["count"], 2);

    let dir = scratch("m6h4_fixture_pack");
    let scenarios = write_json(
        &dir.join("scenarios.json"),
        &serde_json::json!([
            {
                "name": "the denied fragment",
                "substrate": "fs",
                "locator": "/tmp/whatever/gx-denied-target.txt",
                "context": "Policy",
                "actor_key": "key-1",
                "expect": "Deny"
            },
            {
                "name": "anything else",
                "substrate": "fs",
                "locator": "/tmp/whatever/ordinary.txt",
                "context": "Policy",
                "actor_key": "key-1",
                "expect": "Admit"
            }
        ]),
    );
    let tested = run(gx()
        .arg("policy")
        .arg("test")
        .arg(&pack)
        .arg("--scenario")
        .arg(&scenarios));
    println!(
        "FIXTURE_PACK_TEST exit={} cases={}",
        tested.code, tested.stdout
    );
    assert_eq!(
        tested.code, 0,
        "the fixture denies one name and admits the rest"
    );
}
