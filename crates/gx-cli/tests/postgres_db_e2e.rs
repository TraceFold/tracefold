// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **v0.2.7 Lane A** (`req/38` §81 ruling 1; sem: SEM-gx-cli-1900, `req/137` §A1 item 3, AC-A3) — the db effect,
//! end to end, through the compiled `gx` binary: `gx submit --substrate custom:postgres` →
//! `gx plan` → `gx verify --policy` → `gx commit` → `gx undo --policy` →
//! `gx receipt verify --offline`, twice (the commit and the undo). This is the CLI process surface
//! `req/136`'s cross-phase audit measured as unreachable (`PG_PLAN_RC=1`, `req/136` §2's db section (sem: SEM-gx-cli-1901),
//! `no adapter named Custom("postgres")`) before Lane A's registration.
//! `crates/gx-cli/tests/postgres_wired.rs` measures the same loop **in-process**, through the
//! `Engine` API directly; this file is the surface an operator actually types at — four verbs, four
//! processes (Λ2, `crates/gx-cli/tests/pipeline_cmds.rs`'s module header, one substrate over), plus
//! a fifth and sixth for the two offline checks.
//!
//! # Why `--policy`, on `verify` and on `undo`
//!
//! See `postgres_wired.rs`'s module header and `tests/fixtures/permit-postgres.cedar`: no shipped
//! pack names `custom:postgres`, so Cedar's default-deny would refuse both the forward change and
//! 43 §5-2's own re-verify of the compensating undo. `gx verify --policy`/`gx undo --policy`
//! (E-M6-12) are the road an operator has today to decide a postgres change with an opinion at all.
//!
//! # Environment
//!
//! `GX_ADAPTER_POSTGRES_DSN_DEFAULT` must name a reachable postgres server
//! (`tools/pg_local.sh start` then `eval "$(bash tools/pg_local.sh env)"`, req/38 §77).

// 🔴 `req/817`: this suite exercises a verb whose mechanism is `gx-adapter-postgres`,
// one of the four crates `req/789` §3 holds private. The public distribution builds without
// it (`default = []` there), so the whole file compiles away rather than failing to resolve
// a crate that is not in the tree. The private build turns `pg` on by default and runs it.
#![cfg(feature = "pg")]

mod support;

use std::path::{Path, PathBuf};

use support::{run, scratch, secure_scratch, write_json};

/// `tests/fixtures/permit-postgres.cedar` — see the module header.
fn fixture_policy() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("permit-postgres.cedar")
}

/// One table on the live server, this file's own connection (not shared with
/// `postgres_wired.rs`'s or `gx-adapter-postgres`'s own suites).
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

/// 🔴 **AC-A3**: the whole db-effect loop, real DB row read back directly (past the adapter) after
/// the commit and again after the undo -- the same discrimination `postgres_wired.rs` makes,
/// through the CLI process surface rather than the `Engine` API.
#[test]
fn the_db_effect_completes_submit_plan_verify_commit_undo_and_offline_verify_through_the_cli() {
    let sandbox = Sandbox::new("cli_e2e");
    let project = scratch("postgres_db_e2e_project");
    let home = secure_scratch("postgres_db_e2e_home");

    let gx = || {
        let mut cmd = support::gx();
        cmd.env("HOME", &home)
            .env("USERPROFILE", &home)
            .arg("--project")
            .arg(&project);
        cmd
    };

    // The actor key -- `gx key gen`'s own JSON is what `--key`/`--checkpoint-key` below read
    // (`crate::demo`'s own precedent for a one-key walk: the receipt is signed by the actor per
    // `session.rs`'s module header, M6H3-4, and this suite reuses the same key as the ledger's for
    // `gx log checkpoint`, exactly as `gx demo` does).
    let keygen = run(gx().args(["key", "gen", "--json"]));
    assert_eq!(keygen.code, 0, "key gen: {}", keygen.stderr);
    let key_id = keygen.json()["key_id"]
        .as_str()
        .expect("44 §1.2's `gx key gen` prints a key_id")
        .to_string();
    let pub_json = write_json(&project.join("pub.json"), &keygen.json());
    let secret_key = gx_cli::keys::KeyStore::at(home.join(".gx").join("keys")).path_of(&key_id);

    let goal_file = project.join("goal.sql");
    std::fs::write(
        &goal_file,
        format!(
            "UPDATE public.{} SET val = 'after' WHERE id = 1",
            sandbox.table
        ),
    )
    .expect("write the goal");

    // 1. submit
    let submitted = run(gx()
        .arg("submit")
        .args(["--substrate", "custom:postgres"])
        .arg("--locator")
        .arg(sandbox.locator())
        .arg("--intent")
        .arg(&goal_file)
        .args(["--context", "Evidence"])
        .args(["--actor-key", &key_id])
        .args(["--actor-kind", "agent"])
        .args(["--actor-model", "v027a-db-e2e/1 (Lane A db-effect E2E)"]));
    assert_eq!(submitted.code, 0, "submit: {}", submitted.stderr);
    let intent_id = submitted.json()["intent_id"]
        .as_str()
        .expect("an intent id")
        .to_string();

    // 2. plan -- the measurement req/136 §4-1/§2 found impossible before Lane A's registration
    // (`PG_PLAN_RC=1`, `no adapter named Custom("postgres")`).
    let planned = run(gx().args(["plan", &intent_id]));
    assert_eq!(planned.code, 0, "plan: {}", planned.stderr);
    let tid = planned.json()["transformation"]["id"]
        .as_str()
        .expect("a transformation id")
        .to_string();

    // 3. verify, against the fixture permit
    let verified = run(gx()
        .args(["verify", &tid])
        .arg("--policy")
        .arg(fixture_policy()));
    assert_eq!(verified.code, 0, "verify: {}", verified.stderr);
    assert_eq!(
        verified.json()["kind"],
        serde_json::json!("Admit"),
        "tests/fixtures/permit-postgres.cedar permits every custom:postgres change: {}",
        verified.json()
    );

    // 4. commit -- the real UPDATE
    let committed = run(gx().args(["commit", &tid]));
    assert_eq!(committed.code, 0, "commit: {}", committed.stderr);
    let commit_receipt = committed.json()["stored_at"]
        .as_str()
        .expect("a stored commit receipt path")
        .to_string();
    assert_eq!(
        sandbox.read_val().as_deref(),
        Some("after"),
        "the commit is a real UPDATE against the live server, read back on a connection this test \
         opened itself, past the CLI and the adapter both"
    );

    // 5. offline-verify the commit receipt, against a checkpoint taken while it still stands
    // (gotcha61/73's pairing).
    let head1 = project.join("head1.json");
    let cp1 = run(gx()
        .args(["log", "checkpoint", "--key"])
        .arg(&secret_key)
        .args(["--out"])
        .arg(&head1));
    assert_eq!(cp1.code, 0, "log checkpoint (post-commit): {}", cp1.stderr);
    let verify1 = run(gx()
        .args([
            "receipt",
            "verify",
            &commit_receipt,
            "--offline",
            "--checkpoint",
        ])
        .arg(&head1)
        .arg("--checkpoint-key")
        .arg(&pub_json)
        .arg("--key")
        .arg(&pub_json));
    assert_eq!(
        verify1.code, 0,
        "commit receipt offline verify: {}",
        verify1.stderr
    );
    assert_eq!(verify1.json()["valid"], serde_json::json!(true));

    // 6. undo -- 43 §5-2's own re-verify needs the same fixture permit.
    let undone = run(gx()
        .args(["undo", &tid])
        .arg("--policy")
        .arg(fixture_policy()));
    assert_eq!(undone.code, 0, "undo: {}", undone.stderr);
    let undo_receipt = undone.json()["stored_at"]
        .as_str()
        .expect("a stored undo receipt path")
        .to_string();
    assert_eq!(
        sandbox.read_val().as_deref(),
        Some("before"),
        "the undo restored the escrowed pre-image (T-10b's live read, taken before the forward apply)"
    );

    // 7. offline-verify the undo receipt, against a checkpoint taken while it stands.
    let head2 = project.join("head2.json");
    let cp2 = run(gx()
        .args(["log", "checkpoint", "--key"])
        .arg(&secret_key)
        .args(["--out"])
        .arg(&head2));
    assert_eq!(cp2.code, 0, "log checkpoint (post-undo): {}", cp2.stderr);
    let verify2 = run(gx()
        .args([
            "receipt",
            "verify",
            &undo_receipt,
            "--offline",
            "--checkpoint",
        ])
        .arg(&head2)
        .arg("--checkpoint-key")
        .arg(&pub_json)
        .arg("--key")
        .arg(&pub_json));
    assert_eq!(
        verify2.code, 0,
        "undo receipt offline verify: {}",
        verify2.stderr
    );
    assert_eq!(verify2.json()["valid"], serde_json::json!(true));

    println!(
        "PG_DB_E2E submit=0 plan=0 verify=Admit commit=0 undo=0 offline_verify_commit=valid \
         offline_verify_undo=valid table={}",
        sandbox.table
    );
}
