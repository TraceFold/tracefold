// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//
// The shipped **git** pack (FR-028, AC-074), beside `policies/fs/deny-etc.cedar`.
//
// 32 FR-028 verbatim: "gx-gate MUST ship a policy pack (a collection of ready-made invariants) under
// `policies/`. The fs-substrate pack ships in M3", and 34 AC-074 is the git/MCP half: "Given: the
// shipped policy pack for the git/MCP substrates (`policies/{git,mcp}/`). When: each pack's
// conformance tests run. Then: each pack has at least 1 Admit case and 1 Deny case, and both pass." The conformance cases are (sem: SEM-policies-014)
// `crates/gx-gate/tests/ac_074.rs`; the road into the build is `crates/gx-gate/src/packs.rs`.
//
// Where this file lives: root, not `req/` -- req/38 §19's M3-16, the same ruling that put the fs
// pack at `policies/fs/`.
//
// # 🔴 What a rule here may say is bounded by what the git adapter can produce
//
// `gx-adapter-git` writes a locator as `<repository>#<reference>:<entry>`
// (`crates/gx-adapter-git/src/locator.rs`, `Position::locator`), the repository is absolute
// (ASM-69-3), the reference is spelled in full (`refs/heads/main`), and **the entry is a single
// component**: `MAX_PATH_DEPTH` is 1, because a nested path rebuilds every tree between the file and
// the root, which is n object writes for one change (req/99 §9 R-2).
//
// So a rule about `.github/workflows/**` -- the obvious git analogue of `/etc`, the file that decides
// what runs with the repository's credentials -- **cannot fire in v0.1**: no locator the adapter
// accepts has two path components, and the gate compares the string the adapter put in
// `pre.locator()`. Shipping it would be req/99 §3's **D-4** by another road: "declaring an unreachable
// constant is inventing a refusal nobody asked for, and worse, it becomes a constant the reader
// believes is reachable". `crates/gx-gate/tests/ac_074.rs::every_case_names_a_locator_the_git_adapter_could_produce` (sem: SEM-policies-015)
// keeps that boundary measured rather than remembered, and the rule ships the day `MAX_PATH_DEPTH`
// moves.
//
// What is left is the **reference namespace**, which is git's own semantics rather than a hosting
// convention, and which is reachable today: `full_reference` maps `tags/v1` to `refs/tags/v1` and
// `remotes/origin/main` to `refs/remotes/origin/main`, so both spellings parse and both can reach a
// gate.
//
// # What ships: two statements
//
// Two, as the fs pack ships two, and for the same reason -- the rules worth shipping are the ones a
// pack can state truthfully at the visibility M3-10 fixed ("v0.1 pack's effective scope = the
// locator/actor/context/order class"). The payload is opaque (P-6), the actor is a key and nothing more (C-1), and a (sem: SEM-policies-016)
// read never arrives at a gate at all (C-8); `crates/gx-gate/src/packs.rs` carries the whole table.
//
//   1. `git-permit-default` -- Cedar is default-deny ("no request is authorized (decision Allow)
//      unless there is a specific permit policy that grants it", (sem: SEM-policies-017)
//      https://docs.cedarpolicy.com/auth/authorization.html, read 2026-08-08 and recorded in M3), so
//      without a permit a git deployment admits nothing at all. It is scoped to the git substrate,
//      which is what makes an fs change evaluated against this set alone reach Cedar's third rule
//      rather than being quietly admitted by a git permit.
//
//   2. `git-deny-nonbranch-refs` -- a forbid over the two reference namespaces gx is not the writer
//      of. "forbid overrides permit", so it needs no exception carved into the permit. (sem: SEM-policies-018)
//
// # Why those two namespaces, and not others
//
// **`refs/tags/*`**: a tag is a name a release, a signature or a deployment already refers to. Moving
// it does not add a fact; it changes what an existing reference meant, after the fact, for everybody
// who resolved it earlier. That is the same shape as the fs pack's `/etc`: the thing a change to it
// alters is what other things mean.
//
// **`refs/remotes/*`**: a remote-tracking reference is this repository's record of what another
// repository said. Writing it locally does not change the remote; it fabricates a claim about the
// remote, and the next fetch either overwrites the fabrication or is confused by it. gx records
// provenance for changes it mediates, and a change whose whole content is a false statement about a
// system gx cannot see is one a shipped pack should refuse by default.
//
// What is **not** here: a rule about `refs/heads/main` itself. Refusing changes to the default branch
// would refuse the wedge case -- an agent committing to a branch is the thing gx exists to mediate --
// and a deployment that wants a protected branch writes one statement of its own. A shipped pack is a
// starting point, not a policy (35 RSK-5: "Policy authorship cold start", "mitigates but does not resolve"). (sem: SEM-policies-019)
//
// `@id` on every statement is a format requirement rather than decoration (**ASM-62-1** / C-4): a set
// without it is refused at load, because `PolicySet::from_str` names policies by position, that id
// lands in `PolicyDecisionRecord`, and 42 §1.3 puts the record inside `AdmitProof`'s IdentityView --
// so swapping two statements in this file would change what a receipt says without changing what was
// decided.

@id("git-permit-default")
permit (
    principal,
    action,
    resource
)
when { resource.substrate == "git" };

// `like` is the only string matcher Cedar has and it works on strings rather than on entity
// references (https://docs.cedarpolicy.com/policies/syntax-operators.html: "Binary operator that (sem: SEM-policies-020)
// evaluates to true if the string in the left operand matches the pattern string in the right
// operand … one or more asterisks (*) as wildcard characters"). The locator arrives as an attribute (sem: SEM-policies-021)
// of the resource entity for exactly that reason -- see `RequestView` in `crates/gx-gate/src/policy.rs`
// for the whole mapping.
//
// The pattern anchors on `#`, which is the separator between the repository and the reference, so a
// repository directory that happens to be called `refs` cannot make an ordinary branch change match:
// `/srv/refs/tags/x#refs/heads/main:f` does not contain `#refs/tags/`. `crates/gx-gate/tests/ac_074.rs`
// carries that case rather than leaving the anchoring to be believed.
@id("git-deny-nonbranch-refs")
forbid (
    principal,
    action,
    resource
)
when {
    resource.substrate == "git"
    && (resource.locator like "*#refs/tags/*" || resource.locator like "*#refs/remotes/*")
};
