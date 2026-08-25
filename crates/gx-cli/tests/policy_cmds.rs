// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 `gx policy lint` / `gx policy test` (44 §1.2) — and **M6-21's three accessors, called**.
//!
//! §41 M5H4-8 armed `Gate::policies`, `Gate::invariants` and `PolicyEngine::is_empty` with a kill
//! condition, req/88 §4 M6-21 named `gx policy lint` as the consumer, and req/38 §47 adopted (a) (sem: SEM-gx-cli-1623) dissolved
//! the condition on one further condition: "naming it alone does not make it a consumer—the relevant hand's DoD must include 'machine-count each of the 3
//! accessors' call sites; 0 is RED'" (sem: SEM-gx-cli-1623).
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
        "44 §1.2 `gx policy lint`: \"0=no problems\" (sem: SEM-gx-cli-1624). stderr: {}",
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
    // ready-made invariant with the fs pack, so the honest report is "zero, and here is why" (sem: SEM-gx-cli-1625).
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
/// "policy came up with 0 packs" (sem: SEM-gx-cli-1626) is the most dangerous configuration a fail-closed deployment can be in.
/// Cedar is default-deny, so an empty set is not a permissive gate — it is a gate that **admits
/// nothing at all**, and a deployment that came up that way would refuse every change with no rule
/// to point at. The status stays 0 because 44 §1.2's 1 is "a lint/test error exists" (sem: SEM-gx-cli-1627) and this is not an
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
        "🔴 M6-21: \"policy came up with 0 packs\" (sem: SEM-gx-cli-1628) startup warning. Without it, the most dangerous configuration \
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
    assert_eq!(
        linted.code, 1,
        "44 §1.2: \"1=a lint/test error exists\" (sem: SEM-gx-cli-1629)"
    );
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

/// 🔴 `gx policy test` — "runs a Gate evaluation against the specified scenario(s) and checks it against the expected value" (sem: SEM-gx-cli-1630).
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
        "44 §1.2: \"0=no problems\" (sem: SEM-gx-cli-1631). cases: {}",
        tested.stdout
    );
    assert_eq!(tested.json()["passed"], 3);
    assert_eq!(tested.json()["failed"], 0);

    // 🔴 The negative half. Without it this suite would pass against a runner that answered "pass"
    // to everything, which is the shape "do not place only positive controls" (sem: SEM-gx-cli-1632) keeps catching.
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
    assert_eq!(
        failed.code, 1,
        "44 §1.2: \"1=a lint/test error exists\" (sem: SEM-gx-cli-1633)"
    );
    assert_eq!(failed.json()["failed"], 1);
    assert_eq!(failed.json()["cases"][0]["actual"], "Deny");
    assert_eq!(failed.json()["cases"][0]["pass"], false);
}

/// 🔴 The fixture pack of **M6H3-9 adopted (a)** (sem: SEM-gx-cli-1634), linted and exercised.
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

// ---------------------------------------------------------------------------
// 🔴 AC-PP-10 (req/446) — a scenario file can say which statement decided it
// ---------------------------------------------------------------------------

/// The shipped postgres pack — the deny-default one, and therefore the pack whose scenarios
/// **cannot be written at all** without `deny_by_no_policy`.
fn postgres_pack() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-cli")
        .join(gx_gate::packs::POSTGRES_PACK_PATH)
}

fn pg_case(name: &str, locator: &str, extra: serde_json::Value) -> serde_json::Value {
    let mut case = serde_json::json!({
        "name": name,
        "substrate": "custom:postgres",
        "locator": locator,
        "context": "Policy",
        "actor_key": "key-1",
        "expect": "Deny",
    });
    let object = case.as_object_mut().expect("a case is an object");
    for (key, value) in extra.as_object().expect("extra is an object") {
        object.insert(key.clone(), value.clone());
    }
    case
}

const CATALOG: &str = "postgres://main/pg_catalog.pg_authid?oid=10";
const BUSINESS: &str = "postgres://main/public.orders?id=7";

/// 🔴 **The gap, measured**: under an arm-only expectation, a row decided by a statement and a row
/// decided by nothing at all both pass — and they are different facts.
///
/// This is G-4's "same road" holding at the engine and failing at the expressiveness, which is what
/// AC-PP-10 exists to close. The test asserts the weakness rather than working around it, so that
/// if `Deny` ever stopped being satisfied by Cedar's third rule this row would say so.
#[test]
fn an_arm_only_deny_cannot_tell_a_refusal_from_an_absence() {
    let dir = scratch("pp_v0_weak_expectation");
    let scenarios = write_json(
        &dir.join("weak.json"),
        &serde_json::json!([
            pg_case("a catalog row", CATALOG, serde_json::json!({})),
            pg_case("a business row", BUSINESS, serde_json::json!({})),
        ]),
    );
    let tested = run(gx()
        .arg("policy")
        .arg("test")
        .arg(postgres_pack())
        .arg("--scenario")
        .arg(&scenarios));
    let cases = tested.json();
    println!(
        "PP_V0_WEAK exit={} deciding={:?} / {:?}",
        tested.code, cases["cases"][0]["deciding"], cases["cases"][1]["deciding"]
    );
    assert_eq!(
        tested.code, 0,
        "both rows pass under an arm-only expectation"
    );
    assert_eq!(cases["cases"][0]["pass"], true);
    assert_eq!(cases["cases"][1]["pass"], true);
    // ...and yet the two were decided by different things entirely.
    assert_eq!(
        cases["cases"][0]["deciding"],
        serde_json::json!(["postgres-deny-system-catalogs"]),
        "the catalog row is refused by the pack's statement"
    );
    assert_eq!(
        cases["cases"][1]["deciding"],
        serde_json::json!([]),
        "the business row is refused by nothing having an opinion — the deny-default"
    );
}

/// 🔴 **AC-PP-10, the fix**: naming the deciding statement makes the two rows distinguishable.
#[test]
fn expect_policy_id_separates_a_named_refusal_from_an_unnamed_one() {
    let dir = scratch("pp_v0_named_expectation");
    let right = write_json(
        &dir.join("right.json"),
        &serde_json::json!([
            pg_case(
                "the catalog row, by name",
                CATALOG,
                serde_json::json!({ "expect_policy_id": "postgres-deny-system-catalogs" })
            ),
            pg_case(
                "the business row, by nothing",
                BUSINESS,
                serde_json::json!({ "deny_by_no_policy": true })
            ),
        ]),
    );
    let ok = run(gx()
        .arg("policy")
        .arg("test")
        .arg(postgres_pack())
        .arg("--scenario")
        .arg(&right));
    println!("PP_V0_NAMED exit={} json={}", ok.code, ok.json()["cases"]);
    assert_eq!(ok.code, 0, "both rows are true when stated precisely");

    // The same two rows with their expectations swapped: each must now fail, and the report has to
    // say why in terms an operator can read (`expected_by` beside `deciding`).
    let swapped = write_json(
        &dir.join("swapped.json"),
        &serde_json::json!([
            pg_case(
                "the catalog row, claimed to be decided by nothing",
                CATALOG,
                serde_json::json!({ "deny_by_no_policy": true })
            ),
            pg_case(
                "the business row, claimed to be decided by a statement",
                BUSINESS,
                serde_json::json!({ "expect_policy_id": "postgres-deny-system-catalogs" })
            ),
        ]),
    );
    let failed = run(gx()
        .arg("policy")
        .arg("test")
        .arg(postgres_pack())
        .arg("--scenario")
        .arg(&swapped));
    let json = failed.json();
    println!("PP_V0_SWAPPED exit={} json={}", failed.code, json["cases"]);
    assert_eq!(
        failed.code, 1,
        "a scenario naming the wrong decider must fail"
    );
    assert_eq!(
        json["failed"], 2,
        "both rows are wrong, and both must say so"
    );
    assert_eq!(json["cases"][0]["actual"], "Deny");
    assert_eq!(
        json["cases"][0]["expected_by"], "<no policy applied>",
        "🔴 the arm alone would print `expected Deny, actual Deny, pass false` — the report has to \
         carry what was expected of the decider, or a true refusal is unreadable"
    );
    assert_eq!(
        json["cases"][0]["deciding"],
        serde_json::json!(["postgres-deny-system-catalogs"])
    );
    assert_eq!(
        json["cases"][1]["expected_by"],
        "postgres-deny-system-catalogs"
    );
    assert_eq!(json["cases"][1]["deciding"], serde_json::json!([]));
}

/// A pair of fields that describe no verdict is refused before the gate is asked.
///
/// `Error::Usage`, not a failing case: "the policy could not be evaluated" and "the policy said
/// something else" are different facts, and E-M3-3 is the standing rule against giving them one
/// face. A contradictory scenario is a third thing again — the question itself is malformed.
#[test]
fn a_scenario_that_describes_no_verdict_is_refused_as_usage() {
    let dir = scratch("pp_v0_contradiction");
    let both = write_json(
        &dir.join("both.json"),
        &serde_json::json!([pg_case(
            "both at once",
            CATALOG,
            serde_json::json!({
                "expect_policy_id": "postgres-deny-system-catalogs",
                "deny_by_no_policy": true
            })
        )]),
    );
    let refused = run(gx()
        .arg("policy")
        .arg("test")
        .arg(postgres_pack())
        .arg("--scenario")
        .arg(&both));
    println!(
        "PP_V0_CONTRADICTION exit={} stderr={}",
        refused.code,
        refused.stderr.trim()
    );
    assert_eq!(refused.code, 1);
    assert!(
        refused.stdout.trim().is_empty(),
        "44 §1.3: a refusal writes nothing to stdout"
    );
    assert!(
        refused.stderr.contains("VALIDATION_ERROR"),
        "the refusal must name itself a usage error rather than a failing case: {}",
        refused.stderr
    );

    let escalate = write_json(
        &dir.join("escalate.json"),
        &serde_json::json!([{
            "name": "escalate names a statement",
            "substrate": "custom:postgres",
            "locator": BUSINESS,
            "context": "Policy",
            "actor_key": "key-1",
            "invert_available": false,
            "expect": "Escalate",
            "expect_policy_id": "postgres-deny-system-catalogs"
        }]),
    );
    let refused = run(gx()
        .arg("policy")
        .arg("test")
        .arg(postgres_pack())
        .arg("--scenario")
        .arg(&escalate));
    println!(
        "PP_V0_ESCALATE_ID exit={} stderr={}",
        refused.code,
        refused.stderr.trim()
    );
    assert_eq!(refused.code, 1);
    assert!(
        refused.stderr.contains("VALIDATION_ERROR"),
        "E-M3-4's escalation is the gate's rule, so there is no statement for a row to name: {}",
        refused.stderr
    );
}

/// 🔴 **AC-PP-03**: every shipped pack passes its own co-shipped scenario file.
///
/// The condition PACK_FORMAT F1 gates, exercised end to end rather than by `find`: before v0 the
/// `gx policy test` road existed and **no shipped pack had ever travelled it** (0/3 co-shipped a
/// scenario file). This is the test that would notice if that regressed.
#[test]
fn every_shipped_pack_passes_its_own_scenario_file() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-cli")
        .to_path_buf();
    let mut checked = 0usize;
    for pack in gx_gate::packs::SHIPPED_PACKS {
        let cedar = root.join(pack.path);
        let scenarios = cedar
            .parent()
            .expect("a pack lives in a directory")
            .join("scenarios.json");
        assert!(
            scenarios.is_file(),
            "PACK_FORMAT F1: {} ships no scenario file, so the pack has never travelled the road \
             it asks a third party to travel",
            pack.path
        );
        let tested = run(gx()
            .arg("policy")
            .arg("test")
            .arg(&cedar)
            .arg("--scenario")
            .arg(&scenarios));
        println!(
            "PP_V0_SHIPPED_SCENARIOS pack={} exit={} passed={} failed={}",
            pack.path,
            tested.code,
            tested.json()["passed"],
            tested.json()["failed"]
        );
        assert_eq!(
            tested.code, 0,
            "{}: its own scenario file must pass",
            pack.path
        );
        assert_eq!(tested.json()["failed"], 0);
        checked += 1;
    }
    assert_eq!(checked, 4, "four packs ship as of policy pack v0");
}
