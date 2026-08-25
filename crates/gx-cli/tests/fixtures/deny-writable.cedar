// 🔴 **The test-only pack of M6H3-9 adopted (a); sem: SEM-gx-cli-1902** — a pack that denies a path a suite may actually write.
//
// req/38 §50: "deny a writable path — **a test-only policy pack fixture**, plus `gx verify --policy <PATH>`
// (44's extension = E-M6-12, settled by hand 4 at implementation time). Record-only E2E's completion goes to hand 4's DoD" (sem: SEM-gx-cli-1903).
//
// # Why it has to exist
//
// The shipped pack (`policies/fs/deny-etc.cedar`) has one forbid and it names `/etc`. That made it
// the CLI's only route to a `Verdict::Deny` in v0.1, and hand 3 could therefore measure DR-2's
// enforcing half through the binary (`gx verify` exits 2, T-8 refuses before `apply`, the file is
// untouched) and **not** its record-only half: under `EnforcementMode::RecordOnly` T-8r opens, the
// commit walks on to `apply`, and on that locator `apply` is a write to `/etc` — a permission error
// for an ordinary user and a broken machine for a suite run as root. So hand 3 measured T-8r inside
// the engine and raised the gap (M6H3-9). This file closes it: the denied locator is a file in a
// temporary directory, so the record-only commit can be allowed to do exactly what DR-2 says it
// does — "the apply goes through, but `Receipt.enforced=false` is recorded, and the exit code is 0" (sem: SEM-gx-cli-1904) — and the suite can then
// read the file and see that it moved.
//
// # Why it is **not** under `policies/`
//
// `crates/gx-gate/tests/pack_embedding.rs::nothing_ships_in_policies_that_no_build_loads` asserts
// that every `.cedar` under `policies/` is the one `packs.rs` embeds, and the fail-open it closes is
// precisely a second pack file that looks, to anybody reading the repository, like a rule in force.
// A test fixture is not a rule in force. It lives beside the suite that names it, reaches a build
// through no `include_str!`, and is read at run time by `gx verify --policy` like any pack an
// operator would write.
//
// # The two statements
//
// Cedar is default-deny ("no request is authorized … unless there is a specific permit policy that
// grants it") (sem: SEM-gx-cli-1905), so the permit is what makes an fs transformation decidable at all, and "forbid
// overrides permit" (sem: SEM-gx-cli-1906) is what makes the second statement win where both apply. `@id` on both, because
// `PolicyEngine::parse` refuses a set without it (ASM-62-1) and because the deciding policy's id
// reaches `PolicyDecisionRecord` and from there a receipt's CID.
//
// The forbid matches on a **name fragment** rather than on a path, because the locator a suite
// denies is a temporary directory that does not exist until the test runs. `like` is Cedar's only
// string matcher and it works on the resource attribute ASM-60-1 maps from `pre.locator()`.

@id("fixture-permit-fs")
permit (
    principal,
    action,
    resource
)
when { resource.substrate == "fs" };

@id("fixture-deny-writable")
forbid (
    principal,
    action,
    resource
)
when {
    resource.substrate == "fs"
    && resource.locator like "*gx-denied-target*"
};
