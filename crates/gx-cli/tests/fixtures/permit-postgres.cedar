// 🔴 **The test-only pack v0.2.7 Lane A needs** (`req/38` §81 ruling 1 (sem: SEM-gx-cli-2201), `req/137` §A1 item 2/3;
// E-M6-12's shape, the same one `deny-writable.cedar` already carries in this directory).
//
// # Why it has to exist
//
// None of the three shipped packs (`policies/{fs,git,mcp}/`) name the postgres substrate --
// `crate::policy::substrate_tag` maps `SubstrateKind::Custom("postgres".to_string())` to the
// resource attribute `"custom:postgres"`, and no `permit` anywhere under `policies/` matches it.
// Cedar is default-deny ("no request is authorized … unless there is a specific permit policy that
// grants it" (sem: SEM-gx-cli-2202)), so a `gx verify` of a postgres transformation against the **shipped** set always
// reaches `Verdict::Deny` -- correctly: v0.1 ships no policy opinion about a substrate P1 added after
// the fixed four (42 §3.1's `Custom` escape hatch, req/115 §A). That is not this batch's gap to
// close (Lane B's canon-editing scope, `req/137` §B1, does not ask for a shipped postgres pack
// either) -- it is the reason `gx verify --policy <PATH>` / `gx undo --policy <PATH>` exist at all
// (E-M6-12), and this file is what a suite or an operator names there to get past Cedar's default
// answer and actually exercise the postgres adapter's escrow/apply through a real gate decision.
//
// # Why it is **not** under `policies/`
//
// `crates/gx-gate/tests/pack_embedding.rs::nothing_ships_in_policies_that_no_build_loads` asserts
// that every `.cedar` under `policies/` is the one `packs.rs` embeds. A test fixture is not a rule
// in force; it lives beside the suites that name it and is read at run time by `gx verify --policy`/
// `gx undo --policy`, the same as any third-party pack an operator would write (E-M6-12's own words).
//
// The one statement: Cedar is default-deny, so a permit is what makes a postgres transformation
// decidable at all. `@id` because `PolicyEngine::parse` refuses a set without it (ASM-62-1).

@id("fixture-permit-postgres")
permit (
    principal,
    action,
    resource
)
when { resource.substrate == "custom:postgres" };
