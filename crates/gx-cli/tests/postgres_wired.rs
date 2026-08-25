// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **v0.2.7 Lane A** (`req/38` §81 ruling 1 (sem: SEM-gx-cli-2210), `req/136` §4-1 gotcha75, `req/137` §A1 item 2, AC-A2)
//! — escrow → gate → commit → journal → receipt, through `PostgresAdapter` reached by **this
//! binary's own `Engine` registry**, the one measurement `req/136`'s cross-phase audit found
//! missing: AC-P1-1..5 (`crates/gx-adapter-postgres/tests/ac_p1_*.rs`) call the adapter's seven
//! methods directly and never through a `gx_engine::Engine`, so nothing in this repository had
//! driven a postgres change through the loop `defaults.rs` already drives for git
//! (`an_engine_this_binary_opened_decides_a_git_change_with_the_git_pack`). This file is that
//! probe, one substrate over, plus the negative half AC-A2 asks for: an unconfigured DSN is a
//! *named* refusal and not a silent admission.
//!
//! # Why a fixture policy, and not the shipped set
//!
//! No shipped pack (`policies/{fs,git,mcp}/`) names `custom:postgres` — Cedar is default-deny, so a
//! `gx verify` of a postgres transformation against the shipped set alone always reaches
//! `Verdict::Deny` before the escrow/apply/journal path this probe measures ever runs. Closing that
//! (a shipped postgres permit) is neither this batch's Lane A nor Lane B scope (`req/137` §B1 does
//! not ask for one); `tests/fixtures/permit-postgres.cedar` is E-M6-12's road — the same one
//! `gx verify --policy`/`gx undo --policy` give an operator — read here through [`open_engine`]'s own
//! `--policy` parameter rather than the shipped default.
//!
//! # Environment
//!
//! `GX_ADAPTER_POSTGRES_DSN_DEFAULT` must name a reachable postgres server
//! (`tools/pg_local.sh start` then `eval "$(tools/pg_local.sh env)"`, §77's permanent requirement)
//! for [`the_postgres_adapter_is_reached_through_this_binarys_own_engine_registry`].
//! [`the_dsn_for_an_unconfigured_alias_is_a_named_error_not_a_fail_open`] needs no server at all —
//! an alias nothing ever gave a DSN refuses before a connection is attempted.

use std::path::{Path, PathBuf};

use gx_cli::session::open_engine;
use gx_core::{Actor, ChangeContext, GoalBytes, Intent, SubstrateKind, Timestamp};
use gx_engine::{reconstruct, InjectedEvidence, Lifecycle};
use gx_witness::KeyPair;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear the scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

/// `tests/fixtures/permit-postgres.cedar` — see the module header for why this file, and not the
/// shipped set, is what this suite decides with.
fn fixture_policy() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("permit-postgres.cedar")
}

/// An engine this binary opened exactly the way every verb does ([`open_engine`],
/// `crates/gx-cli/tests/defaults.rs::engine_in`'s construction), deciding with the fixture permit
/// above instead of the shipped set.
fn engine_in(name: &str) -> gx_engine::Engine<InjectedEvidence> {
    let project = scratch(name);
    let layout = gx_cli::layout::Layout::create(&project).expect("create .gx/");
    open_engine(
        &layout,
        InjectedEvidence::new(Vec::new()),
        None,
        gx_core::FailPosture::FailClosed,
        Some(&fixture_policy()),
    )
    .expect("the fixture pack parses and the journal opens")
}

/// One table on the live server `GX_ADAPTER_POSTGRES_DSN_DEFAULT` names — a connection and a name
/// of this file's own, not shared with `gx-adapter-postgres`'s own suites (a different crate's
/// `tests/support/mod.rs::Sandbox`, over its own connection).
struct Sandbox {
    table: String,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let table = format!("gx_v027a_{name}_{}", std::process::id());
        let mut client = gx_adapter_postgres::db::connect("default").expect(
            "GX_ADAPTER_POSTGRES_DSN_DEFAULT must name a reachable postgres server \
             (tools/pg_local.sh start; eval \"$(bash tools/pg_local.sh env)\", req/38 §77)",
        );
        client
            .batch_execute(&format!(
                "DROP TABLE IF EXISTS public.{table}; \
                 CREATE TABLE public.{table} (id integer PRIMARY KEY, val text); \
                 INSERT INTO public.{table} (id, val) VALUES (1, 'before')"
            ))
            .expect("the sandbox table is created");
        Self { table }
    }

    fn locator(&self) -> String {
        format!("postgres://default/public.{}?id=1", self.table)
    }

    /// Read the row **directly**, past the adapter entirely — the discrimination that tells a real
    /// `UPDATE` apart from a simulated one, the same one AC-P1-1's own offline re-verification makes
    /// for its narrower (adapter-only) claim.
    fn read_val(&self) -> Option<String> {
        let mut client =
            gx_adapter_postgres::db::connect("default").expect("a connection for the read-back");
        let rows = client
            .query(
                &format!("SELECT val FROM public.{} WHERE id = 1", self.table),
                &[],
            )
            .expect("the direct read runs");
        rows.first().and_then(|row| row.get::<_, Option<String>>(0))
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if let Ok(mut client) = gx_adapter_postgres::db::connect("default") {
            let _ = client.batch_execute(&format!("DROP TABLE IF EXISTS public.{}", self.table));
        }
    }
}

fn update_intent(locator: &str, table: &str) -> Intent {
    Intent::new(
        SubstrateKind::Custom("postgres".to_string()),
        locator.to_string(),
        GoalBytes(format!("UPDATE public.{table} SET val = 'after' WHERE id = 1").into_bytes()),
        ChangeContext::Evidence,
        Actor::Agent {
            key: "key-agent-1".to_string(),
            model: "claude-fable-5".to_string(),
        },
    )
}

/// 🔴 **AC-A2, positive half**: escrow (T-2's `snapshot`/`plan`) → gate (T-3/T-4 `verify`) → commit
/// (T-8..T-11, `InverseEscrowed`+`Committed` in the journal) → receipt, all through `PostgresAdapter`
/// reached via this binary's own registry rather than called directly (the gap `req/136` §4-1
/// found).
#[test]
fn the_postgres_adapter_is_reached_through_this_binarys_own_engine_registry() {
    let sandbox = Sandbox::new("wired");
    let mut engine = engine_in("postgres_wired_positive");
    let intent = update_intent(&sandbox.locator(), &sandbox.table);
    let at = Timestamp(1_755_000_000_000_000_000);

    engine
        .submit(&intent, 0, at)
        .expect("T-1: a draft is recorded before any substrate is touched");
    let id = engine.plan(&intent, Timestamp(at.0 + 1)).expect(
        "T-2: the fourth adapter is registered, so `plan` reaches PostgresAdapter::snapshot rather \
         than `NotFound { what: \"adapter\" }` (gotcha75's own reproduction, closed)",
    );

    let key = KeyPair::from_seed("key-agent-1", &[7u8; 32]);
    let verified = engine
        .verify(&id, Timestamp(at.0 + 2), &key, None)
        .expect("T-3/T-4: the gate reaches a decision");
    assert_eq!(
        verified,
        Lifecycle::Admitted,
        "the fixture pack permits every custom:postgres change; a Denied here means the gate this \
         binary opened is not deciding with the named --policy"
    );

    engine
        .canonicalize(&id, Timestamp(at.0 + 3), None)
        .expect("T-8: canonicalize");
    let state = engine
        .commit(&id, Timestamp(at.0 + 4), &key)
        .expect("T-9..T-11: precondition re-check (CAS), apply, ledger.append, receipt issue");
    assert_eq!(state, Lifecycle::Committed);

    // The effect really landed.
    assert_eq!(
        sandbox.read_val().as_deref(),
        Some("after"),
        "the commit is a real UPDATE against the live server, read back on a connection this test \
         opened itself"
    );

    // The journal: `InverseEscrowed` is T-10b's escrow, the record this probe's title promises and
    // AC-P1-1/AC-P1-2 (adapter-only) never put behind an `Engine`.
    let kinds: Vec<&str> = engine
        .journal()
        .records()
        .iter()
        .map(|r| r.kind())
        .collect();
    println!("PG_WIRED_JOURNAL_KINDS={kinds:?}");
    for want in [
        "Planned",
        "VerifyStarted",
        "Verdict",
        "InverseEscrowed",
        "Committed",
    ] {
        assert!(
            kinds.contains(&want),
            "43's road from submit to Committed writes {want:?}: {kinds:?}"
        );
    }

    // Sigma agrees from a read-only replay of the same journal (E-M5-2), independent of the live
    // table above.
    let sigma = reconstruct(engine.journal().records());
    let row = sigma
        .state_of(&id)
        .expect("the replay rebuilds a row for a transformation the journal recorded");
    assert_eq!(row.state, Some(Lifecycle::Committed));

    // The receipt: what `gx commit` would file to `.gx/receipts/` (M6H2-1) — read off the engine's
    // own table, the same accessor `crate::pipeline::commit` reads before writing it to disk.
    assert!(
        engine.receipt(&id).is_some(),
        "T-11 issues a CommitReceipt; `Engine::receipt` is the CLI's own read of it"
    );
    assert_eq!(
        engine.enforced(&id),
        Some(true),
        "an Admitted, non-record-only commit is enforced (DR-2's ordinary case)"
    );
}

/// 🔴 **AC-A2, negative half**: an alias nothing gave a DSN is a *named* `Error::Unreadable`
/// (`gx_adapter_postgres::db::dsn_for`'s own message, `crate::db`'s crate documentation: "v0.1 is environment-
/// variable-only, plaintext forbidden" (sem: SEM-gx-cli-2211)), surfaced through the engine as `Error::Adapter { action: "snapshot", .. }` —
/// **fail-open is zero**: `plan` never returns `Ok` for a substrate this process cannot reach, and
/// it never falls back to any other secret source (there is not one).
///
/// No live server needed: [`gx_adapter_postgres::db::dsn_for`] refuses before a connection is even
/// attempted, which is the whole of this test's point.
#[test]
fn the_dsn_for_an_unconfigured_alias_is_a_named_error_not_a_fail_open() {
    const ALIAS: &str = "v027a_unconfigured";
    let mut engine = engine_in("postgres_wired_negative");
    let locator = format!("postgres://{ALIAS}/public.gx_v027a_never_created?id=1");
    let intent = update_intent(&locator, "gx_v027a_never_created");
    let at = Timestamp(1_755_100_000_000_000_000);

    let submitted = engine.submit(&intent, 0, at);
    assert!(
        submitted.is_ok(),
        "T-1 touches no substrate; a draft is recorded regardless of whether the alias below \
         resolves: {submitted:?}"
    );

    let planned = engine.plan(&intent, Timestamp(at.0 + 1));
    println!("PG_UNCONFIGURED_PLAN={planned:?}");
    match planned {
        Err(gx_engine::Error::Adapter { action, detail }) => {
            assert_eq!(
                action, "snapshot",
                "the refusal is PostgresAdapter's own (via `db::connect`), not the registry's \
                 (`NotFound {{ what: \"adapter\" }}` would mean gotcha75 regressed)"
            );
            let expected_var = gx_adapter_postgres::db::env_var_for(ALIAS);
            assert!(
                detail.contains(&expected_var),
                "the refusal names the exact environment variable an operator would set \
                 ({expected_var:?}), not a generic failure: {detail}"
            );
        }
        other => panic!(
            "an alias with no DSN must refuse by name and never Ok — fail-open here would mean an \
             unreadable substrate was silently treated as reachable: {other:?}"
        ),
    }
}
