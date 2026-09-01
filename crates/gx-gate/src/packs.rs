// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The pack a deployment starts from, and the one road it takes into a build (FR-028).
//!
//! Spec: 32 FR-028 verbatim -- "gx-gate MUST bundle a shipped policy pack (a ready-made invariant
//! set) under `policies/`. The fs-substrate pack is bundled in M3 ... it must be confirmed that each
//! policy pack carries at least one Admit case and one Deny case in a conformance test" -- with 34
//! AC-028 stating the conformance half and 41 §2 naming this module. Why a pack ships at all is 35
//! RSK-5: "policy-authorship cold start (people who can write invariants/policies are scarce)",
//! mitigated by a "pack shipped at launch" and -- 45 §87 is explicit -- "mitigates but does not
//! resolve". (sem: SEM-gx-gate-075)
//!
//! Rulings this file implements: **M3-16** (the directory is at the repository root), **M3-10** (what
//! a pack may reason over), **ASM-62-1** / C-4 (`@id` is a format requirement), C-8 and C-1 (two
//! things a pack cannot say), D-9 (whether a ready-made invariant ships beside it).
//!
//! # What ships
//!
//! Two directories, two files, two statements each ([`SHIPPED_PACKS`]):
//!
//! | | |
//! |---|---|
//! | [`FS_PACK_PATH`] | `policies/fs/deny-etc.cedar` -- the path 34 AC-025 writes, at the root (M3-16) |
//! | `fs-permit-default` | a permit scoped to the fs substrate. Cedar is default-deny ("no request is authorized ... unless there is a specific permit policy that grants it"), so without this statement an fs deployment admits nothing at all | (sem: SEM-gx-gate-076)
//! | `fs-deny-etc` | a forbid over `/etc` and everything under it. "forbid overrides permit", so it needs no exception carved into the permit |
//! | [`GIT_PACK_PATH`] | `policies/git/deny-nonbranch-refs.cedar` -- **M7 hand 2**, FR-028's git half. 34 AC-074 writes the directory and this hand writes the name |
//! | `git-permit-default` | the same argument on the git substrate: without a permit a git deployment admits nothing |
//! | `git-deny-nonbranch-refs` | a forbid over `refs/tags/*` and `refs/remotes/*` -- the two namespaces gx is not the writer of. The file itself carries why those two and not others, including why the obvious rule (CI configuration) **cannot** ship in v0.1: `MAX_PATH_DEPTH` is 1 in `gx-adapter-git`, so no locator with a nested path reaches a gate and a rule about one could never fire (req/99 §3 **D-4**) |
//! | [`MCP_PACK_PATH`] | `policies/mcp/deny-etc-resources.cedar` -- **M7 hand 4**, FR-028's mcp half |
//! | `mcp-permit-default` | the same argument again, on the mcp substrate |
//! | `mcp-deny-etc-resources` | a forbid over `file:` resources at or under `/etc` -- **the fs pack's rule, written on the substrate that is otherwise the road around it**. The file carries why the rule a reader expects ("forbid the `shell.exec` tool") **cannot** ship: the tool's name lives in the payload and P-6 keeps a payload opaque, so a statement about it could never fire -- D-4 again, and the third time this project has had to write it down | (sem: SEM-gx-gate-077)
//!
//! Two statements per pack is a decision rather than a stopping point: the rules worth shipping are
//! the ones a pack can state truthfully at the visibility M3 has, and
//! [the table below](#what-a-pack-can-reason-over-m3-10) is that visibility. `req/65` §3 records what
//! was considered and left out for fs; the git file records the same for git.
//!
//! # The one road
//!
//! req/60 §5.2 asks hand 5 for "there is exactly one road by which a pack's file gets embedded into
//! the build (i.e. do not create a second road; a mechanical check of the same shape as AC-014)"
//! (sem: SEM-gx-gate-078). [`FS_PACK_SOURCE`] is that road for the fs pack and
//! [`GIT_PACK_SOURCE`] is the git pack's: one `include_str!` each, in this file, naming the file on
//! disk. The claim is **per pack** -- one embedding of one file -- and [`SHIPPED_PACKS`] is what
//! makes it countable now that there is more than one.
//!
//! # The other one road (**G-4**)
//!
//! A second road is counted in this file, and it is not the same question. FR-028's is "how do these
//! bytes reach a build"; G-4's (`req/38_ERRATA_2026-08-07.md` §19, req/98 §3-4) is "when a third
//! party's pack is checked, does it go the same way ours does" -- "conformance checking runs down the
//! same single road (no branching between ours and a third party's)" (sem: SEM-gx-gate-079). [`check_pack`] is that road: it takes a [`Gate`] and a case
//! table and knows nothing about where the policies came from. `gx policy test` (44 §1.2) is the
//! operator's face of it and `crates/gx-gate/tests/ac_074.rs` is the shipped pack's.
//!
//! AC-014 is the shape of the check, not a metaphor for it -- there, one line takes every hash; here,
//! one line embeds the pack. `crates/gx-gate/tests/pack_embedding.rs` counts the roads by scanning
//! the workspace's sources, asserts the embedded bytes are the file on disk, and asserts that every
//! `.cedar` file under `policies/` is the one that is embedded. That last one is the fail-open case:
//! a policy file somebody adds and nobody embeds ships in the repository, is read by nothing, and
//! looks from the outside like a rule that is in force.
//!
//! Tests that predate this module read the same file from disk at run time (`ac_025`,
//! `policy_mapping`, `policy_determinism`). That is not a second road: a road is how the pack reaches
//! a **build**, and those suites reach the same bytes the build embeds -- which is itself asserted,
//! so a copy could not hide between them.
//!
//! # What a pack can reason over (M3-10)
//!
//! req/38 §19 rules the range and requires it be said out loud: "M3-10 (adopted = option b+a, filed):
//! the v0.1 pack's effective reach is **stated explicitly as the locator/actor/context/order class**
//! (no overclaiming)" (sem: SEM-gx-gate-080). What follows is that range,
//! taken from ASM-60-1's mapping (`crate::policy::RequestView`) rather than restated beside it:
//!
//! | a policy may read | it comes from | as |
//! |---|---|---|
//! | the actor's key | `t.actor.key()` | `principal`, `GxActor::"<key>"` |
//! | the kind of change | `t.context` | `action`, `GxAction::"Policy"` and the rest |
//! | the substrate and the locator | `pre.substrate`, `pre.locator()` | `resource`, plus its two attributes `substrate` and `locator` |
//! | the order (0, 1, 2) | `t.order()` | `context.order` |
//! | whether an inverse exists | `invert_available` | `context.invert_available` |
//! | how much evidence, and of what kinds | `evidence` | `context.evidence_count`, `context.evidence_kinds` |
//!
//! And three things a pack **cannot** say, each of which has bitten somebody who assumed otherwise:
//!
//! 1. **The change itself is invisible.** 42 §3.4 makes a delta's `payload` "an opaque change
//!    description; core/gate/witness treat it only as a byte string (P-6)", so a rule of the form
//!    "this change must not delete more than ten lines" cannot be written -- not "is not written
//!    yet". 45 TH-1's line-count invariant and the "ready-made invariant set" FR-028 names in passing
//!    both live behind that (sem: SEM-gx-gate-081)
//!    wall until an adapter hands the gate structured facts, which req/38 §19 keeps as an M4
//!    requirement. `crates/gx-gate/tests/ac_028.rs` measures the wall rather than describing it: two
//!    changes with different payloads and the same locator get the same verdict.
//! 2. **Reads never arrive.** A read does not pass through the gate (sem: SEM-gx-gate-082): every `Transformation` is a change to the
//!    thing it names (P-1), and 41 §3 has no read/write field, so a pack cannot write a rule about
//!    reading and does not need one -- a read was never going to reach this gate. (req/38 §21 C-8
//!    asks for exactly this line, because "it follows from P-1, but stop leaving it unsaid" (sem: SEM-gx-gate-083).)
//! 3. **The actor can be identified only by key.** ASM-60-1 maps `t.actor.key()` and nothing else, so
//!    41 §3's distinction between a `Human` and an `Agent` -- and the agent's `model` -- is not
//!    visible to a policy. "require additional evidence for an agent's changes" is therefore
//!    unwritable in v0.1 (sem: SEM-gx-gate-084), and req/38
//!    §21's C-1 ruling keeps it that way on purpose: "do not grow it in M3 ... if the need for
//!    expressiveness is real, do it once, in the same window as M4's facts path". FR-006/P-7's
//!    "accountability does not depend on the variant" is (sem: SEM-gx-gate-084)
//!    about who answers for a change, not about what a policy can branch on.
//!
//! And one thing the table above assumes rather than reads: **the locator is judged by the spelling
//! it was given (normalization is the adapter's responsibility)** (sem: SEM-gx-gate-085). A pack compares the string an adapter put in `pre.locator()`, so
//! `/tmp/../etc/passwd` and `/etc/passwd` are two different strings and the first is admitted by the
//! rules above (`crates/gx-gate/tests/false_admit.rs` pins that behaviour rather than hiding it).
//! Resolving a path is not this layer's to do -- 42 §3.1 makes the locator the adapter's value, and a
//! path algebra invented here would be semantics P-6 keeps outside the gate. req/38 §25's H-2 rules
//! the pair: this line, and an M4 adapter-contract ticket requiring locators to arrive normalised.
//!
//! # `@id` is a format requirement, not decoration (C-4 / **ASM-62-1**)
//!
//! Every statement in a pack must carry `@id("...")`, and one that does not is refused at load with
//! [`crate::Error::PolicySetUnreadable`] -- the whole set, not the statement. The reason is
//! arithmetic rather than taste: `PolicySet::from_str` names policies by position, that id lands in
//! `PolicyDecisionRecord`, 42 §1.3 puts the record inside `AdmitProof`'s IdentityView, and an
//! `AdmitProof` reaches a receipt's CID -- so swapping two statements in a file would change what a
//! receipt says without changing what was decided. req/38 §21 C-4 confirms it as ASM-62-1 (sem: SEM-gx-gate-086) and asks for
//! it to be written into the pack's specification, which is this paragraph: **a third-party pack
//! without `@id` on every statement is not loadable by gx**, and the failure is at load rather than
//! at the first request.
//!
//! Ids must also be distinct (two statements claiming one id is the same defect by another road) and
//! non-empty, and the set may hold no templates. [`crate::PolicyEngine::parse`] is where all four
//! refusals live.
//!
//! # No ready-made invariant ships with it (**D-9**)
//!
//! req/38 §22's D-9 leaves this hand the question "should a ready-made invariant that requires
//! additional approval for an order-2 transformation ship bundled with the pack" (sem: SEM-gx-gate-087), with no ruling either way. It does not ship, for three reasons, and `req/65`
//! §3 carries them in full:
//!
//! 1. **Whatever it could say, a policy already says.** An invariant sees a `GateInput`; the order,
//!    the locator and the evidence summary in it are all mapped into the Cedar request already, so a
//!    rule about order-2 is one `when { context.order == 2 }` clause. Shipping a Rust body for it
//!    would be a second road to one rule -- the thing the road count above exists to prevent.
//! 2. **"additional approval" is not an invariant's word.** (sem: SEM-gx-gate-088) An [`crate::InvariantCheck`] answers holds or does
//!    not hold, and a `false` becomes a `Deny` (**E-M3-6**'s `INVARIANT_VIOLATED`). Asking a human is
//!    the `Escalate` arm, which **E-M3-4** generates from `invert_available=false` and which 43 T-5
//!    resolves one layer up. A shipped invariant named "requires approval" that in fact refuses (sem: SEM-gx-gate-089)
//!    would teach the conflation the ruling asks the docs to avoid.
//! 3. **The invariant RSK-5 actually wants cannot be written.** "row-count preserved"-shaped checks need the (sem: SEM-gx-gate-090)
//!    payload, and P-6 says no until M4's facts path exists. Shipping a substitute that only looks at
//!    the locator would be the overclaim M3-10 forbids.
//!
//! What ships instead is the honest statement of the gap, here and in the pack file. When M4 opens
//! the facts path, the first ready-made invariant becomes writable and this decision is due for
//! re-reading -- `req/65` §4 raises it so the re-reading is scheduled rather than remembered.

use gx_core::{
    Actor, ChangeContext, Cid, CompositionMetadata, DeltaRef, IntentId, ObjectId, ObjectSnapshot,
    PlannedDeltaBytes, ReprKind, Subject, SubstrateKind, Timestamp, Transformation,
    TransformationId, VerdictKind,
};
use gx_witness::Evidence;

use crate::policy::PolicyEngine;
use crate::{Error, Gate, GateInput, ReasonSource, Result, Verdict};

/// Where the shipped fs pack lives, relative to the repository root.
///
/// 34 AC-025 writes this path verbatim, and req/38 §19's M3-16 puts the directory at the root:
/// "`policies/` is **directly under root** (AC-025's verbatim path takes priority; shipped artifacts
/// are not placed under req/)" (sem: SEM-gx-gate-091). It is spelled here so that a
/// test, a report or a deployment names the file in one way, and so that moving the file is one
/// edit plus a failing test rather than a silent divergence between the two.
pub const FS_PACK_PATH: &str = "policies/fs/deny-etc.cedar";

/// The shipped fs pack, as the build embeds it -- **the one road** (FR-028).
///
/// The path is written relative to this file and names [`FS_PACK_PATH`] at the repository root. It
/// is the only embedding of a pack in the workspace, and `crates/gx-gate/tests/pack_embedding.rs`
/// is what keeps it the only one -- a `const` beside this one, or an `include_str!` in some other
/// crate, is a second copy that is free to be edited alone.
///
/// 🔴 **req/1032 §3 (2026-09-01), resolved**: this road used to reach outside the crate directory
/// (`../../../policies/...`), which is the cost of M3-16's ruling that the pack belongs at the
/// repository root -- `cargo package` does not follow such a path, and the empirical
/// `cargo package -p gx-gate --list` in req/1032 §3 measured **zero** `policies/` files in the
/// tarball, confirming the failure `req/65` §4 G-1 predicted rather than pre-solved. **Fix, chosen
/// over the two alternatives req/65 §4 G-1 and req/1032 §5 named**: `build.rs` copying at build time
/// was rejected first, in `req/65` §3.5, for making the "one road" claim ungameable-in-appearance
/// only (a copy-then-`include!` road is a second embedding *mechanism* the `pack_embedding.rs` scan
/// -- which greps for the `include_str!`/`include_bytes!` macro names -- would not see). Moving
/// `policies/` itself into the crate (and reversing every one of the 94 other repository files a
/// `policies/(fs|git|mcp|postgres)/` grep hits, measured 2026-09-01 -- `tools/pub_sync_dryrun.sh`'s
/// sync-set list, `req/spec/`, `docs/TUTORIAL.md`, `req/semantics.json` among them) was rejected as
/// disproportionate to this blocker: it would need an M3-16 erratum and would touch files this fix
/// does not need to touch.
/// **What this crate does instead**: `crates/gx-gate/policies/{fs,git,mcp,postgres}/*.cedar` are
/// byte-identical mirrors of the root files, placed inside the crate directory so `cargo package`
/// picks them up automatically (no `include`/`exclude` entry needed -- files under the crate root
/// ship by default). [`FS_PACK_PATH`] is deliberately left naming the **root** copy (AC-025's
/// verbatim path is about that file, not about where the build reaches for bytes), so every existing
/// reader of the root path is unaffected. The road (`include_str!` below) now reads the crate-local
/// mirror, and divergence between the two copies is not a new risk left unguarded: it is exactly
/// what `crates/gx-gate/tests/pack_embedding.rs::the_embedded_bytes_are_the_files_the_criteria_name`
/// already asserted (`pack.source == pack_text(pack.path)`, i.e. embedded bytes == root-file bytes)
/// before this change, unmodified -- the existing audited test becomes the sync gate for free.
pub const FS_PACK_SOURCE: &str = include_str!("../policies/fs/deny-etc.cedar");

/// The ids the shipped fs pack answers under, sorted (**ASM-62-1**).
///
/// Declared beside the road rather than derived from it, so that a statement added to the file
/// without a word here is a failing test rather than a policy nobody named. It is the same shape
/// `ERROR_KINDS` has against the `Error` enum, and `ac_014`'s declared-road snapshot before that:
/// two places that must agree, checked mechanically, instead of one place that quietly grows.
pub const FS_PACK_POLICY_IDS: [&str; 2] = ["fs-deny-etc", "fs-permit-default"];

/// The shipped fs pack, parsed.
///
/// The entry point every consumer of the pack uses -- the conformance suite included -- so that
/// "which bytes did this gate decide with" has one answer (sem: SEM-gx-gate-092). Build a gate from it with
/// `Gate::with_policies(packs::fs_pack()?)`; the registry a deployment runs beside it is the
/// deployment's own (D-9 above).
///
/// # Errors
/// [`crate::Error::PolicySetUnreadable`] if the embedded source does not parse or a statement in it
/// carries no `@id`. Both are compile-time-fixed inputs here, so a failure is a broken shipped
/// artifact rather than a runtime condition -- which is why it is checked at build-adjacent test
/// time (`crates/gx-gate/tests/pack_embedding.rs`) and not swallowed here.
pub fn fs_pack() -> Result<PolicyEngine> {
    PolicyEngine::parse(FS_PACK_SOURCE)
}

// ---------------------------------------------------------------------------
// The git pack (**M7 hand 2**, FR-028's git half / AC-074)
// ---------------------------------------------------------------------------

/// Where the shipped git pack lives, relative to the repository root.
///
/// 34 AC-074 writes the **directory** (`policies/{git,mcp}/`) and not the file name, so the name is
/// this hand's: it says what the pack refuses, as `deny-etc.cedar` does. M3-16 puts the directory at
/// the root for the same reason it put the fs one there.
pub const GIT_PACK_PATH: &str = "policies/git/deny-nonbranch-refs.cedar";

/// The shipped git pack, as the build embeds it -- **the second road, for the second pack**.
///
/// FR-028's "exactly one road" is a claim per pack rather than per repository (sem: SEM-gx-gate-093): one embedding of *this*
/// file, so that the bytes in the build and the bytes on disk cannot diverge. `SHIPPED_PACKS` is what
/// makes the claim countable for a set of packs, and `crates/gx-gate/tests/pack_embedding.rs` counts.
///
/// 🔴 **req/1032 §3 (2026-09-01)**: reads the crate-local mirror `crates/gx-gate/policies/git/
/// deny-nonbranch-refs.cedar`, not the root file, for the same reason [`FS_PACK_SOURCE`]'s doc
/// comment gives in full -- `cargo package` cannot follow a path outside the crate directory.
/// [`GIT_PACK_PATH`] still names the root copy on purpose; the two are kept equal by
/// `pack_embedding.rs`'s existing, unmodified test.
pub const GIT_PACK_SOURCE: &str = include_str!("../policies/git/deny-nonbranch-refs.cedar");

/// The ids the shipped git pack answers under, sorted (**ASM-62-1**).
pub const GIT_PACK_POLICY_IDS: [&str; 2] = ["git-deny-nonbranch-refs", "git-permit-default"];

/// The shipped git pack, parsed.
///
/// # Errors
/// [`crate::Error::PolicySetUnreadable`] if the embedded source does not parse or a statement in it
/// carries no `@id`.
pub fn git_pack() -> Result<PolicyEngine> {
    PolicyEngine::parse(GIT_PACK_SOURCE)
}

// ---------------------------------------------------------------------------
// The mcp pack (**M7 hand 4**, FR-028's mcp half / AC-074)
// ---------------------------------------------------------------------------

/// Where the shipped mcp pack lives, relative to the repository root.
///
/// 34 AC-074 writes the **directory** (`policies/{git,mcp}/`) and not the file name, so the name is
/// this hand's: it says what the pack refuses, as the other two do.
pub const MCP_PACK_PATH: &str = "policies/mcp/deny-etc-resources.cedar";

/// The shipped mcp pack, as the build embeds it -- **the third road, for the third pack**.
///
/// 🔴 **req/1032 §3 (2026-09-01)**: reads the crate-local mirror `crates/gx-gate/policies/mcp/
/// deny-etc-resources.cedar`; see [`FS_PACK_SOURCE`]'s doc comment for the full reasoning.
pub const MCP_PACK_SOURCE: &str = include_str!("../policies/mcp/deny-etc-resources.cedar");

/// The ids the shipped mcp pack answers under, sorted (**ASM-62-1**).
pub const MCP_PACK_POLICY_IDS: [&str; 2] = ["mcp-deny-etc-resources", "mcp-permit-default"];

/// The shipped mcp pack, parsed.
///
/// # Errors
/// [`crate::Error::PolicySetUnreadable`] if the embedded source does not parse or a statement in it
/// carries no `@id`.
pub fn mcp_pack() -> Result<PolicyEngine> {
    PolicyEngine::parse(MCP_PACK_SOURCE)
}

// ---------------------------------------------------------------------------
// The postgres pack (policy pack v0, req/446 V0-C)
// ---------------------------------------------------------------------------

/// Where the shipped postgres pack lives, relative to the repository root.
///
/// 🔴 Behind `pg` with the source it names (`req/832` ATOM -1). The path and the embedding are one
/// claim -- "this build carries this file" -- so a distribution that does not carry the file must
/// not carry the path either; a path constant surviving the embedding would let a consumer form a
/// `PathBuf` to a file that is not there and read the resulting error as a missing *pack* rather
/// than as a pack this build never shipped.
#[cfg(feature = "pg")]
pub const POSTGRES_PACK_PATH: &str = "policies/postgres/deny-system-catalogs.cedar";

/// The shipped postgres pack, as the build embeds it -- **the fourth road, for the fourth pack**.
///
/// 🔴 **`req/832` ATOM -1 gates this, and it is the reason the feature exists.** `include_str!` is
/// resolved while the crate is being compiled, before `cfg` has decided anything about a
/// *dependent* crate -- so `gx-cli`'s `pg` feature could never have reached it (`req/817` §2.5),
/// and the published tree failed here with `couldn't read .../policies/postgres/...` for every
/// cargo build. The gate is on the embedding itself, which is the only place it can be.
///
/// 🔴 **req/1032 §3 (2026-09-01)**: reads the crate-local mirror `crates/gx-gate/policies/postgres/
/// deny-system-catalogs.cedar`; see [`FS_PACK_SOURCE`]'s doc comment for the full reasoning. `pg`
/// defaults on in the private manifest (`crates/gx-gate/Cargo.toml`), so a default-features
/// `cargo package -p gx-gate` compiles this road too -- the mirror has to exist for the same
/// dry-run req/1032 measured failing, not only for the three unconditional packs.
#[cfg(feature = "pg")]
pub const POSTGRES_PACK_SOURCE: &str =
    include_str!("../policies/postgres/deny-system-catalogs.cedar");

/// The ids the shipped postgres pack answers under, sorted (**ASM-62-1**).
///
/// 🔴 **One, not two.** The other three packs declare a `<substrate>-permit-default` beside their
/// forbid; this one deliberately does not, and `policies/PACK_FORMAT.md`'s F3 is written to admit
/// both shapes for that reason. The consequence is measured, not asserted: adding this pack to
/// [`SHIPPED_PACKS`] changes the admissible surface by zero, and
/// `crates/gx-gate/tests/ac_074.rs::adding_the_postgres_pack_admits_nothing_it_did_not_admit_before`
/// runs the three-pack set and the four-pack set against the same requests to show it.
#[cfg(feature = "pg")]
pub const POSTGRES_PACK_POLICY_IDS: [&str; 1] = ["postgres-deny-system-catalogs"];

/// 🔴 The exact string a policy in the postgres pack must compare `resource.substrate` against.
///
/// `substrate_tag` maps `SubstrateKind::Custom(name)` to `custom:{name}`, so this is
/// `custom:postgres` and **not** `postgres`. It is a `const` here so that the pack's own text can be
/// checked against a value the crate produces rather than against a second copy of the literal;
/// `ac_074.rs` closes the loop by deriving the same string from [`crate::policy::RequestView`]
/// itself, which is the only comparison that would survive `substrate_tag` changing.
#[cfg(feature = "pg")]
pub const POSTGRES_SUBSTRATE_TAG: &str = "custom:postgres";

/// The shipped postgres pack, parsed.
///
/// # Errors
/// [`crate::Error::PolicySetUnreadable`] if the embedded source does not parse or a statement in it
/// carries no `@id`.
#[cfg(feature = "pg")]
pub fn postgres_pack() -> Result<PolicyEngine> {
    PolicyEngine::parse(POSTGRES_PACK_SOURCE)
}

/// One pack this build ships: where it lives, what it says, what it answers under, and **whose
/// substrate it speaks for**.
///
/// Declared rather than derived, for [`FS_PACK_POLICY_IDS`]'s reason one level up: a table walked by
/// a test can notice a pack that shipped without a row, and a `read_dir` cannot notice a tree that
/// is wrong.
#[derive(Clone, Copy, Debug)]
pub struct ShippedPack {
    /// The repository-relative path, as the `include_str!` names it.
    pub path: &'static str,
    /// The bytes the build embeds.
    pub source: &'static str,
    /// The ids the statements carry, sorted.
    pub policy_ids: &'static [&'static str],
    /// 🔴 The substrate every statement in this pack is scoped to, in the spelling
    /// [`crate::policy`] maps a [`gx_core::SubstrateKind`] into (`fs`, `git`, `mcp`,
    /// `custom:<NAME>`).
    ///
    /// **M7 hand 4**, and it is declared for two consumers that would otherwise each guess:
    ///
    /// 1. [`shipped_pack_set`]'s locality obligation -- "each pack decides only its own substrate" (sem: SEM-gx-gate-094)
    ///    is the property that makes composing the packs safe, and it cannot be stated without
    ///    knowing whose substrate each one is.
    /// 2. `crates/gx-gate/tests/false_admit.rs`'s **vector expiry** (**H-9**). A negative vector
    ///    whose expectation is "no shipped pack speaks for this substrate" stops being true the day (sem: SEM-gx-gate-095)
    ///    one does, silently, and this field is what a harness reads to notice.
    ///
    /// Declared and then **checked against the file**: `crates/gx-gate/tests/pack_embedding.rs::
    /// the_declared_substrate_is_the_one_every_statement_scopes_on` reads the `resource.substrate ==
    /// "…"` clauses out of the pack's own text and refuses a pack whose statements name any other.
    /// A declaration nobody compares with the artifact is the failure mode this whole module is
    /// built against.
    pub substrate: &'static str,
}

/// 🔴 **`req/832` ATOM -1: the table has two arities, and the three shared rows are written once.**
///
/// A distribution built without `pg` carries no postgres embedding (see [`POSTGRES_PACK_SOURCE`]),
/// so its table must be three rows -- a fourth row would name a pack the build does not carry, and
/// `crates/gx-cli/tests/defaults.rs` compares this table against what
/// `register_default_adapters` actually registers, so a row for an absent pack turns that probe
/// from a check into a lie (the same argument `req/817` §2.3 made for `DEFAULT_SUBSTRATES`).
///
/// The obvious way to write this -- two full array literals under `cfg` -- would copy the fs, git
/// and mcp rows into two places, and two copies of one declaration drift. So each row is a `const`
/// named once and the two arrays are assembled from them: the only difference between the private
/// table and the public one is whether [`PG_ROW`] is in it.
const FS_ROW: ShippedPack = ShippedPack {
    path: FS_PACK_PATH,
    source: FS_PACK_SOURCE,
    policy_ids: &FS_PACK_POLICY_IDS,
    substrate: "fs",
};

/// The git pack's row. See [`FS_ROW`] for why the rows are named.
const GIT_ROW: ShippedPack = ShippedPack {
    path: GIT_PACK_PATH,
    source: GIT_PACK_SOURCE,
    policy_ids: &GIT_PACK_POLICY_IDS,
    substrate: "git",
};

/// The mcp pack's row. See [`FS_ROW`] for why the rows are named.
const MCP_ROW: ShippedPack = ShippedPack {
    path: MCP_PACK_PATH,
    source: MCP_PACK_SOURCE,
    policy_ids: &MCP_PACK_POLICY_IDS,
    substrate: "mcp",
};

/// The postgres pack's row -- **the one row the `pg` feature adds**. See [`FS_ROW`].
#[cfg(feature = "pg")]
const PG_ROW: ShippedPack = ShippedPack {
    path: POSTGRES_PACK_PATH,
    source: POSTGRES_PACK_SOURCE,
    policy_ids: &POSTGRES_PACK_POLICY_IDS,
    substrate: POSTGRES_SUBSTRATE_TAG,
};

/// Every pack this build ships (FR-028).
///
/// Three as of **M7 hand 4**. A pack file that appears under `policies/` without a row here is caught
/// by `crates/gx-gate/tests/pack_embedding.rs::nothing_ships_in_policies_that_no_build_loads`.
///
/// 🔴 **What this table now does say, and did not until hand 4**: that a deployment loads these.
/// Until this hand `gx_cli::session::open_engine` built its gate from the fs pack alone, and the
/// note here read "the day a surface registers a second adapter, the default policy set has to
/// become the shipped **set**". req/38 §60 ruled that day to be this one (**the counter-ruling to
/// R-9**) (sem: SEM-gx-gate-096), and
/// [`shipped_pack_set`] is the value the CLI now starts from.
#[cfg(feature = "pg")]
pub const SHIPPED_PACKS: [ShippedPack; 4] = [FS_ROW, GIT_ROW, MCP_ROW, PG_ROW];

/// Every pack a build without `pg` ships -- three, and the postgres row is absent because its
/// source is (`req/832` ATOM -1).
#[cfg(not(feature = "pg"))]
pub const SHIPPED_PACKS: [ShippedPack; 3] = [FS_ROW, GIT_ROW, MCP_ROW];

/// 🔴 **PACK_FORMAT F3 shape 2's second conjunct, in one canonical form** (**R36**, `req/476` M-01).
///
/// F3 shape 2 is "no permit statement at all, **declared as such in the pack's header**". The
/// second clause is a *statement a human wrote*, not a property of the policy set, so a machine can
/// only check it against a form agreed in advance — and this is that form, quoted from the pack
/// that already shipped it:
///
/// ```text
/// // # 🔴 This pack declares a **deny-default**. It has no permit statement, and that is the ruling.
/// ```
///
/// Two things follow from choosing the phrase that was already there rather than inventing a
/// marker. First, **no shipped pack was edited to pass the new gate**: `policies/postgres/` carries
/// it verbatim today, so this check measures the artifacts rather than a change made to satisfy it.
/// Second, the phrase is prose an author would write anyway, so a pack that means it says it
/// naturally, and a pack that does not cannot acquire it by accident.
///
/// 🔴 **This is a spelling, and `req/476` §10 item 6 is the standing warning about those** — a
/// census that greps a name goes quiet when the name moves. It is accepted here for one reason: the
/// thing being checked *is* a sentence. Everything else this clause needs — how many permits, which
/// ids — is derived from Cedar's parse, precisely so that this is the only spelling in the check.
pub const DENY_DEFAULT_DECLARATION: &str = "declares a **deny-default**";

/// The words that turn a sentence carrying [`DENY_DEFAULT_DECLARATION`] into one that does **not**
/// make the declaration.
///
/// 🔴 **R37 / `req/496` M-03.** This list is short, it is here rather than implied, and it is not
/// claimed to be complete — see [`declares_deny_default`] for what is and is not promised.
const DENIALS: [&str; 7] = [
    " not ",
    " no ",
    "n't ",
    " never ",
    "cannot",
    " nor ",
    " without ",
];

/// 🔴 **R37 / `req/496` M-03** — does this pack's text declare a deny-default?
///
/// # What was wrong
///
/// R36 read the declaration as `source.lines().map(trim_start).filter(starts_with("//"))
/// .any(contains(NEEDLE))` — a scan of line beginnings, in the same function whose own note says
/// "a newline is not what should decide whether it has", one branch after the branch where that
/// note was written. Audit 36 (`req/496` §4-3) wrote one sentence five ways:
///
/// * on its own comment line — accepted (R36's shape);
/// * as a trailing `//` on a statement's line — **refused**;
/// * wrapped across two `//` lines the way a formatter wraps prose — **refused**;
/// * inside a Cedar `@doc(...)` annotation — **refused**;
/// * inside a comment that **denies** it, quoting the phrase — **accepted**.
///
/// The last one is the fail-open. F3 exists so that a pack picks its default out loud, and a header
/// reading "This pack does NOT say that it \"declares a **deny-default**\" — nobody decided its
/// default and a reader must not rely on one" picks nothing at all.
///
/// # What this does instead
///
/// Two changes, and they answer the two directions separately.
///
/// **Where it looks.** The declaration is looked for in the pack's text with `//` markers removed
/// and runs of whitespace collapsed to one space. A line beginning stops deciding anything: the
/// same words are the same declaration on their own line, at the end of a statement's line, split
/// across two lines, or inside an annotation a reader of the pack sees. This is looking *less*
/// carefully at typography, which is the direction the whole clause wants.
///
/// **Whether it is affirmed.** Looking less carefully is the direction that opens fail-opens, so
/// the sentence the phrase sits in is then read for a denial. The sentence runs between the nearest
/// `.`, `;`, `!` or `?` on each side, and if it carries any of [`DENIALS`] the occurrence does not
/// count. A pack may still declare its default elsewhere in the same file; every occurrence is
/// tried, and one affirmative occurrence is a declaration.
///
/// A quotation mark is deliberately **not** a sentence boundary. It is the tempting fifth
/// delimiter — an annotation value is wrapped in them — and taking it would re-open the exact
/// fail-open being closed: audit 36's denying header quotes the phrase, so a `"` boundary would cut
/// the sentence down to the quotation itself and read `declares a **deny-default**` with the word
/// `NOT` outside it. The annotation case does not need the boundary anyway; nothing between the
/// annotation's opening quote and the phrase is a denial.
///
/// # 🔴 What is **not** claimed
///
/// This is prose matching and it stays prose matching. It is not a natural-language understander,
/// [`DENIALS`] is a list of seven strings rather than a theory of negation, and a header written to
/// evade it can be — "this pack refrains from saying it declares a **deny-default**" carries no
/// word on that list and would be read as a declaration.
///
/// The reason that is acceptable **here** and would not be elsewhere: F3 is a gate on a pack a
/// deployment composes at start-up, and the thing it checks *is a sentence an author wrote on
/// purpose*. The failure mode this guards is a pack that says nothing being read as one that said
/// something, and the guard closes the shapes anybody has measured. What it does not do is what
/// R36's note claimed for the permit count and audit 36 falsified for this branch — it does not
/// promise that no spelling gets past. `req/502` records the residue rather than letting the
/// doc-comment imply it away.
#[must_use]
pub fn declares_deny_default(source: &str) -> bool {
    // 🔴 **R38 / `req/513` L-03** — the pack's comments and annotation values, not the whole file.
    // Runs of whitespace collapse to one space so that a wrapped sentence is one sentence.
    let flattened = declaration_surface(source);
    let normalized: String = flattened.split_whitespace().collect::<Vec<_>>().join(" ");

    let mut from = 0usize;
    while let Some(at) = normalized[from..].find(DENY_DEFAULT_DECLARATION) {
        let start = from + at;
        let end = start + DENY_DEFAULT_DECLARATION.len();
        let is_boundary = |c: char| matches!(c, '.' | ';' | '!' | '?');
        let sentence_start = normalized[..start]
            .rfind(is_boundary)
            .map_or(0, |cut| cut + 1);
        let sentence_end = normalized[end..]
            .find(is_boundary)
            .map_or(normalized.len(), |cut| end + cut);
        let sentence = normalized[sentence_start..sentence_end].to_ascii_lowercase();
        // A leading and trailing space so that ` no ` matches a word and not `nobody`.
        let padded = format!(" {sentence} ");
        if !DENIALS.iter().any(|denial| padded.contains(denial)) {
            return true;
        }
        from = end;
    }
    false
}

/// 🔴 **R38 / `req/513` L-03** — the text of a pack a **reader reads as the pack's own statement
/// about itself**: its comments, and the string values of its annotations.
///
/// # Why this is not the whole source
///
/// R37 replaced R36's line-beginning scan with a flatten of the entire file, which was the right
/// direction — the same words are the same declaration on their own line, at the end of a
/// statement's line, wrapped across two, or inside a `@doc(...)` a reader sees — and it went one
/// step too far, because the flatten does not know what a comment is. Audit 37 measured the
/// sentence being accepted out of ordinary program text (`req/513` §4-6).
///
/// R38 measured what that costs when the rest of F3's conjunction is satisfied too: a pack with
/// **no permit statement** whose only occurrence of the phrase sits inside a `forbid`'s string
/// literal satisfied the clause (`pack_v0::every_shipped_pack_declares_its_default`, the
/// `PACK_V0_F3_NON_COMMENT` line). F3 exists so that a pack picks its default *out loud*; a
/// condition's operand is not a place a reader looks for the pack's statement about itself, and a
/// pack whose author never wrote a header would have passed.
///
/// So the surface is narrowed to the two places that *are* the pack talking about itself, and every
/// shape R37 opened stays open: comment text is taken from the `//` marker to end of line (own-line
/// and trailing are the same rule), consecutive fragments are joined with a space (so a sentence a
/// formatter wrapped is still one sentence), and annotation values are appended.
///
/// # 🔴 What this does not do
///
/// It does not parse Cedar, and it is wrong in **three** directions rather than one. R38 declared
/// the first and `req/519` §10-7 recorded that no fixture drove it; audit 38 drove it and found the
/// other two (`req/533` L-01). All three are now driven, by
/// `crates/gx-gate/tests/r37_f3_and_reads.rs::r39_the_narrowing_is_wrong_in_three_directions_and_every_one_of_them_is_driven`:
///
/// 1. A `//` **inside a string literal** opens a comment as far as this is concerned. `s3://` and
///    `https://` are ordinary in a locator, so this is the shape an author reaches by accident.
/// 2. The annotation sweep below walks `source`, **not** the comment surface, so an `@name("…")`
///    written inside a **string literal** is appended as the pack's own statement about itself.
/// 3. The same sweep, for the same reason, reads an `@name("…")` out of a **comment body** — a
///    comment *about* a declaration is taken as a declaration.
///
/// All three are wrong *towards* the old behaviour rather than away from it: they widen the surface,
/// which can only accept a pack a stricter reading would have refused, and F3's job is to refuse a
/// pack that never picked its default out loud. What bounds the cost is the caller:
/// [`declares_its_default`] runs over [`SHIPPED_PACKS`], three packs embedded with `include_str!`,
/// so no third party's text reaches this function — a pack from outside takes `check_pack` and never
/// passes here.
///
/// 🔴 They are declared rather than closed, on purpose. Closing them means knowing where Cedar's
/// string literals and comments are, which is a second parser beside Cedar's own — the drift
/// E-M2-12 exists to prevent — and Cedar's parser is not reachable from here, because this runs
/// before a pack has been accepted. `req/540` R-5c rules the strengthening out of this lane and
/// leaves the residue measured. What is not allowed is what R38 left: a residue named in a
/// doc-comment with nothing driving it, and two more not named at all.
fn declaration_surface(source: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for line in source.lines() {
        if let Some(at) = line.find("//") {
            parts.push(&line[at + 2..]);
        }
    }
    // Annotation values: `@name("...")`. Cedar puts them above the statement they annotate and a
    // reader of the pack sees them, which is why `req/496` M-03 asked for them and why they stay.
    let mut rest = source;
    while let Some(at) = rest.find('@') {
        rest = &rest[at + 1..];
        let Some(open) = rest.find('(') else { break };
        // `@` inside prose is not an annotation. The name has to be an identifier sitting directly
        // on the parenthesis.
        if open == 0
            || !rest[..open]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        let after = &rest[open + 1..];
        let Some(first) = after.find('"') else {
            continue;
        };
        let Some(len) = after[first + 1..].find('"') else {
            continue;
        };
        parts.push(&after[first + 1..first + 1 + len]);
        rest = &after[first + 1 + len..];
    }
    parts.join(" ")
}

/// 🔴 **PACK_FORMAT F3, mechanised** (**R35**, `req/470` M-02).
///
/// # What was wrong
///
/// `policies/PACK_FORMAT.md` counts F3 among "F2, F3, F5, F6 and F7 … they have machinery behind
/// them already" and then lists **four** machineries, one per clause except F3. The same omission
/// sits independently in `tools/gates/pack_format_gate.sh`, whose header lists the clauses it
/// checks (F8, F1, F4) and the clauses that "have machinery already" (F2, F5, F6, F7) — F3 appears
/// in neither list. A repository-wide search for a generic checker of the clause returned **one**
/// hit, and it was a test hard-wired to the postgres pack by name: it reads
/// `POSTGRES_PACK_POLICY_IDS` and `POSTGRES_PACK_SOURCE` as constants, takes no argument, and is
/// not parametrised over anything. So a **fifth** pack shipping with neither an explicit permit
/// default nor a declared deny default would have been stopped by nothing at all, and F3's own
/// row in the document would have gone on claiming it was.
///
/// # What the clause says, and which half of it is checkable here
///
/// F3 admits two shapes, and the point of the clause is that a pack must **pick one out loud**
/// rather than let Cedar's default-deny decide by omission:
///
/// 1. an explicit `<substrate>-permit-default` permit; or
/// 2. no permit statement at all, declared as such in the pack's header, and carrying at least one
///    conformance case whose expectation is `deny_by_no_policy`.
///
/// This function is the half that can be decided from the **pack itself**, which is the half that
/// belongs in library code: the shape of the statements, the shape of the ids, and — since R36 —
/// the header's own declaration, which is inside `pack.source` and was being left to nobody. The
/// remaining half — that a deny-default pack really does ship a `deny_by_no_policy` case — is a
/// fact about a scenario file on disk, and `gx-gate` does not read the filesystem; `crates/gx-gate/
/// tests/pack_v0.rs::every_shipped_pack_declares_its_default` drives both halves together and holds
/// the negative bed (a fifth pack that declares neither, refused by name).
///
/// # 🔴 What R36 changed, and why the two changes are one repair (`req/476` M-01)
///
/// Shape 2 is a **conjunction of three** and the shipped check tested the first conjunct with a
/// scan of line beginnings. Both halves of that were wrong in the same direction — towards
/// accepting:
///
/// * the count is now Cedar's (`Effect::Permit`), so a `permit` sharing a line with its `@id`
///   annotation is a permit;
/// * the header's declaration is now read, so "this pack has no permit statement" stops being
///   mistaken for "this pack declared a deny default".
///
/// The third conjunct stays where it has to: `pack_v0.rs` reads the scenarios off the disk.
///
/// Being on [`shipped_pack_set`] rather than in a test is deliberate: this is what a deployment
/// calls, so a pack that has not declared its default cannot compose, and therefore cannot start a
/// CLI. A gate only a test walks past is a gate a release can be cut around.
///
/// # Errors
/// [`crate::Error::PolicySetUnreadable`], the same ⊥ the clause's siblings raise, naming the pack
/// and which of the two shapes it failed to take.
pub fn declares_its_default(pack: &ShippedPack) -> Result<()> {
    // 🔴 **R36 / `req/476` M-01** — the permits are counted from **Cedar's parse of this source**,
    // not from the spelling at the start of a line.
    //
    // What was here until R36:
    //
    // ```text
    // let permits = statements.iter().filter(|line| line.starts_with("permit")).count();
    // ```
    //
    // A pack that writes `@id("fs-quiet") permit ( principal, action, resource ) …` on **one line**
    // therefore counted **zero** permits, took shape 2, declared no permit default, and was
    // accepted — while Cedar parsed the same text perfectly and shipped the permit. Audit 35 built
    // it and measured `f3_accepted=true cedar_parses=true`. F3's whole purpose is that a pack must
    // pick its default out loud, and a newline is not what should decide whether it has.
    //
    // Counting from `Effect::Permit` makes the check about the **statement** rather than about its
    // typography: an annotation on the same line, extra whitespace, a `permit` inside a string and
    // a comment that begins with the word are all answered correctly, and none of them needs a rule
    // of its own here.
    //
    // A source Cedar cannot read is refused rather than fallen back on. The old line-scan had an
    // answer for unparseable text and the answer was worthless — a pack that does not parse cannot
    // ship at all ([`shipped_pack_set`] is what a deployment composes at start-up), so the honest
    // reply to "what is this pack's default" is that it has no statements to have one.
    let parsed = crate::policy::PolicyEngine::parse(pack.source).map_err(|why| {
        crate::Error::PolicySetUnreadable {
            detail: format!(
                "{}: PACK_FORMAT F3 - this pack's default cannot be judged because its source does \
                 not parse: {why}",
                pack.path
            ),
        }
    })?;
    let permits = parsed.permit_count();
    let permit_default_ids = pack
        .policy_ids
        .iter()
        .filter(|id| id.ends_with("-permit-default"))
        .count();
    // 🔴 **R36 / `req/476` M-01, shape 2's second conjunct.** The clause reads "no permit statement
    // at all, **declared as such in the pack's header**", and until R36 the second half of that
    // conjunction was checked by nothing. The header is inside `pack.source`, so this is decidable
    // in library code — this function's own doc draws the line at "the half that can be decided
    // from the pack itself" and then dropped a half that is on its own side of the line.
    //
    // The declaration is a sentence rather than a property, so it has a canonical form and
    // [`DENY_DEFAULT_DECLARATION`] is it. `policies/postgres/deny-system-catalogs.cedar` already
    // carries it verbatim: no shipped pack was edited to satisfy this check, which is the point —
    // a gate that needed the artifacts changed to pass would be measuring the change.
    let declares_deny_default = declares_deny_default(pack.source);

    if permits == 0 {
        // Shape 2. The ids must agree with the statements: an id promising a permit over a pack
        // that has none is the disagreement F4 exists for, one clause along.
        if permit_default_ids > 0 {
            return Err(crate::Error::PolicySetUnreadable {
                detail: format!(
                    "{}: PACK_FORMAT F3 - this pack ships no permit statement, which is the \
                     deny-default shape, but its declared ids name a permit default. One of the \
                     two is wrong and a reader cannot tell which",
                    pack.path
                ),
            });
        }
        if !declares_deny_default {
            return Err(crate::Error::PolicySetUnreadable {
                detail: format!(
                    "{}: PACK_FORMAT F3 - this pack ships no permit statement, and shape 2 is not \
                     that fact alone: it is that fact **declared in the header**. Nothing in this \
                     pack's comments says {DENY_DEFAULT_DECLARATION:?}, so a reader cannot tell a \
                     deny-default that was decided from one that was arrived at by leaving the \
                     permit out. Write the declaration, or name a `<substrate>-permit-default` \
                     permit (req/476 M-01, req/470 M-02)",
                    pack.path
                ),
            });
        }
        return Ok(());
    }

    // 🔴 **R36 / `req/476` M-01** — and the contradiction in the other direction: a pack that
    // ships permits while its header declares a deny-default. Audit 35's seventh shape was this
    // one inverted, and neither direction was looked at.
    if declares_deny_default {
        return Err(crate::Error::PolicySetUnreadable {
            detail: format!(
                "{}: PACK_FORMAT F3 - this pack's header declares {DENY_DEFAULT_DECLARATION:?} and \
                 it carries {permits} permit statement(s). One of the two is wrong and a reader \
                 cannot tell which (req/476 M-01)",
                pack.path
            ),
        });
    }

    // Shape 1.
    if permit_default_ids == 0 {
        return Err(crate::Error::PolicySetUnreadable {
            detail: format!(
                "{}: PACK_FORMAT F3 - the default must be declared explicitly. This pack carries \
                 {permits} permit statement(s) but no id ending `-permit-default`, so what a \
                 request matching none of its rules gets is decided by Cedar's default rather \
                 than by the pack saying so. Either name the permit `<substrate>-permit-default`, \
                 or ship no permit at all and declare the deny default in the header with a \
                 `deny_by_no_policy` conformance case (req/470 M-02)",
                pack.path
            ),
        });
    }
    Ok(())
}

/// 🔴 **Every shipped pack, as one policy set** -- what a deployment that named no pack decides with.
///
/// **M7 hand 4**, implementing the pair `req/38` §60 ruled: "the default policy set and the default
/// registry are decided **as a pair** (adding a pack without the adapter being registered still
/// yields NotFound)". req/101 §9-1 is the material the ruling took, and its first change point is
/// this function: "add **`shipped_pack_set()`** to `gx_gate::packs` (parses all of `SHIPPED_PACKS`
/// into one `PolicyEngine`). `SHIPPED_PACKS` already exists, so this is not a new declaration but a
/// consumer of an existing one". (sem: SEM-gx-gate-097)
///
/// # Why composing is safe, stated as the property rather than as a hope
///
/// Every statement of every shipped pack carries `resource.substrate == "<its own>"`
/// ([`ShippedPack::substrate`], checked against each file's text). Cedar's evaluation is per
/// statement, so a pack whose statements cannot be satisfied by a request on another substrate
/// **cannot change that request's answer** -- and that is a claim with an obligation attached rather
/// than a paragraph: `crates/gx-gate/tests/shipped_set.rs` runs every row of a cross-substrate table
/// against this set and against the owning pack alone, and requires the same arm **and the same
/// deciding ids**.
///
/// 🔴 What composing **does** change, and the change is the point rather than a side effect: a
/// request on a substrate whose pack is now in the set stops falling to Cedar's third rule (nothing
/// satisfied → `Deny`, [`ReasonSource::NoPolicyApplied`]) and starts being **judged by that pack's
/// rules**. req/101 §9-1: "that is not mitigation but 'coming to be judged', and on the refusal face
/// `git-deny-nonbranch-refs` and (hand 4's) the mcp forbid **become newly reachable**" (sem: SEM-gx-gate-098). The two vectors
/// `crates/gx-gate/tests/false_admit.rs` gained in this hand are those two refusals, measured.
///
/// # Errors
/// [`crate::Error::PolicySetUnreadable`] if the concatenation does not parse, if a statement carries
/// no `@id`, or if two packs claim one id. All three are compile-time-fixed inputs, so a failure
/// here is a broken shipped artifact rather than a runtime condition -- and the duplicate-id refusal
/// is the one that earns its place: it is the only thing standing between two packs that both
/// shipped a statement called `permit-default` and a receipt whose `PolicyDecisionRecord` names a
/// rule nobody can find (42 §1.3).
pub fn shipped_pack_set() -> Result<PolicyEngine> {
    let mut composed = String::new();
    for pack in SHIPPED_PACKS {
        // 🔴 **R35 / `req/470` M-02** — F3 is a condition of every pack in the set, checked before
        // the set exists rather than trusted because the four that ship today happen to hold it.
        declares_its_default(&pack)?;
        composed.push_str(pack.source);
        // The packs' own texts end in a newline, but a set assembled by concatenation must not
        // depend on that: a file whose last line is a statement would otherwise be glued to the
        // next file's first comment line and take it with it.
        composed.push('\n');
    }
    PolicyEngine::parse(&composed)
}

// ---8<--- the one conformance road (G-4) ---8<---
//
// 🔴 **G-4** (`req/38_ERRATA_2026-08-07.md` §19, req/98 §3-4): "when a third-party pack is dropped
// in, conformance checking runs down **the same single road** (no branching between ours and a
// third party's)". (sem: SEM-gx-gate-099)
//
// Everything between these two markers is the road. It is library code rather than test code,
// because a third party cannot call a test: `gx policy test <PATH> --scenario <FILE>` (44 §1.2) is
// the operator's face of this function, `crates/gx-gate/tests/ac_074.rs` is the shipped git pack's,
// and both reach the same lines. What the road knows is a gate and a list of cases; it does not know
// where the pack came from, and `ac_074.rs::the_pack_conformance_runner_names_no_pack` scans this
// region for every word an origin branch would have to be spelled with.
//
// The judgement is `Gate::verify`'s and is not repeated here (req/60 §7.2: "a gx test does not
// re-implement Cedar's decision"). What this adds is the comparison with an expectation and the
// arithmetic AC-028 and AC-074 both ask for -- "at least 1 Admit case and 1 Deny case" -- which is a
// statement about (sem: SEM-gx-gate-100)
// the case table and not about any one case.

/// What a conformance case expects a gate to answer.
///
/// Both the arm and, optionally, **which statement answered**. The id matters because 42 §1.3 puts
/// `PolicyDecisionRecord` inside `AdmitProof`'s IdentityView: two packs can reach `Admit` for
/// opposite reasons, and a table that asserted only the arm would pass on a pack whose permit was
/// deleted and whose deny was widened into an allow. `None` is the weaker form a `--scenario` file
/// can express (44 §1.2 gives it an "expected Verdict" and no id) (sem: SEM-gx-gate-101), and it is written as an option rather
/// than as a second enum so that the weaker expectation is visibly weaker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackExpectation {
    /// Admitted; `Some(id)` also asserts which statement permitted it.
    Admit(Option<String>),
    /// Refused by a statement; `Some(id)` also asserts which one forbade it.
    Deny(Option<String>),
    /// Refused because nothing in the pack was satisfied -- Cedar's third rule, reaching
    /// [`ReasonSource::NoPolicyApplied`] (**E-M3-11**). A different fact from a deny by a statement:
    /// one is a rule refusing, the other is a set with no opinion, and a deployment reading a
    /// conformance table has to be able to tell them apart.
    DenyByNoPolicy,
    /// A human is asked (43 T-5). **E-M3-4** generates it from `invert_available = false`, which is
    /// the only road to it in v0.1, so a case expecting it says so with
    /// [`PackCase::without_inverse`].
    Escalate,
}

impl PackExpectation {
    /// Admitted, by the statement with this id.
    #[must_use]
    pub fn admit_by(policy_id: &str) -> Self {
        Self::Admit(Some(policy_id.to_string()))
    }

    /// Refused, by the statement with this id.
    #[must_use]
    pub fn deny_by(policy_id: &str) -> Self {
        Self::Deny(Some(policy_id.to_string()))
    }

    /// The arm alone, as [`gx_core::VerdictKind`] names it.
    #[must_use]
    pub const fn kind(&self) -> VerdictKind {
        match self {
            Self::Admit(_) => VerdictKind::Admit,
            Self::Deny(_) | Self::DenyByNoPolicy => VerdictKind::Deny,
            Self::Escalate => VerdictKind::Escalate,
        }
    }
}

/// One conformance case: what to ask a gate, and what the pack's author expects back.
///
/// The six facts a policy can read (M3-10's range) are the ones a case can set, and no others: the
/// object id, the digest and the transformation id are outside that range, so they are placeholders
/// rather than parameters. That is the same honesty `gx_cli::policy`'s scenario file carries, said
/// once instead of twice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackCase {
    name: String,
    substrate: SubstrateKind,
    locator: String,
    context: ChangeContext,
    actor_key: String,
    order: u8,
    invert_available: bool,
    evidence: Vec<Evidence>,
    expect: PackExpectation,
    why: String,
}

impl PackCase {
    /// A case at order 0, on the substrate's own changes, by a nameless actor, with an inverse.
    ///
    /// The defaults are the ordinary change: `invert_available = true`, because **E-M3-4** makes
    /// `false` the one condition that produces an `Escalate`, and a table whose default let it fire
    /// would be measuring the fold rather than the pack (`crates/gx-gate/tests/verdict_meet.rs`
    /// measures the fold).
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        substrate: SubstrateKind,
        locator: impl Into<String>,
        expect: PackExpectation,
    ) -> Self {
        Self {
            name: name.into(),
            substrate,
            locator: locator.into(),
            context: ChangeContext::Substrate,
            actor_key: "conformance-actor".to_string(),
            order: 0,
            invert_available: true,
            evidence: Vec::new(),
            expect,
            why: String::new(),
        }
    }

    /// Why this case is in the table.
    ///
    /// A conformance table is documentation a deployment reads before it trusts a pack, so a row
    /// that cannot say what it is for does not belong in one. Carried into the report, where a
    /// failing row prints it.
    #[must_use]
    pub fn because(mut self, why: impl Into<String>) -> Self {
        self.why = why.into();
        self
    }

    /// 41 §3's order (v0.1 admits 0..=2, ASM-6). Order 2 is a change to the policy layer itself.
    #[must_use]
    pub fn at_order(mut self, order: u8) -> Self {
        self.order = order;
        self
    }

    /// 42 §3.2's `ChangeContext`, which the mapping puts in `action`.
    #[must_use]
    pub fn in_context(mut self, context: ChangeContext) -> Self {
        self.context = context;
        self
    }

    /// The key the acting principal is identified by (ASM-60-1 maps `actor.key()` and nothing else).
    #[must_use]
    pub fn by(mut self, actor_key: impl Into<String>) -> Self {
        self.actor_key = actor_key.into();
        self
    }

    /// 42 §3.7's values, passed to the gate "as-is" (AC-016). (sem: SEM-gx-gate-102)
    #[must_use]
    pub fn with_evidence(mut self, evidence: Vec<Evidence>) -> Self {
        self.evidence = evidence;
        self
    }

    /// The adapter could not build an inverse (FR-043) -- **E-M3-4**'s road to `Escalate`.
    #[must_use]
    pub fn without_inverse(mut self) -> Self {
        self.invert_available = false;
        self
    }

    /// What this case is called.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The substrate the change is on.
    #[must_use]
    pub const fn substrate(&self) -> &SubstrateKind {
        &self.substrate
    }

    /// The locator, in the adapter's own spelling.
    #[must_use]
    pub fn locator(&self) -> &str {
        &self.locator
    }

    /// What the case expects.
    #[must_use]
    pub const fn expect(&self) -> &PackExpectation {
        &self.expect
    }
}

/// What one case answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackRow {
    /// The case's name.
    pub name: String,
    /// The arm the gate reached, or `None` if it could not evaluate the case (**E-M3-3**: "the
    /// policy could not be evaluated" and "the policy said something else" are different facts). (sem: SEM-gx-gate-103)
    pub actual: Option<VerdictKind>,
    /// The ids of the statements the gate recorded as deciding, in the order it recorded them.
    /// Empty for a `Deny` nothing decided (`NoPolicyApplied`) and for an `Escalate`.
    pub deciding: Vec<String>,
    /// Whether the answer matched the expectation.
    pub pass: bool,
    /// The refusal, when the gate could not evaluate the case.
    pub detail: Option<String>,
}

/// What a whole case table answered, with the arithmetic AC-028 and AC-074 ask about the table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackConformance {
    rows: Vec<PackRow>,
    admits: usize,
    denies: usize,
    escalates: usize,
    failures: Vec<String>,
}

impl PackConformance {
    /// One row per case, in the table's order.
    #[must_use]
    pub fn rows(&self) -> &[PackRow] {
        &self.rows
    }

    /// How many cases **expect** an admission. "at least 1 Admit case" is a statement about the table (sem: SEM-gx-gate-104),
    /// so it is counted off the expectations rather than off the answers: a table whose Admit case
    /// silently started being denied would otherwise still report one.
    #[must_use]
    pub const fn admits(&self) -> usize {
        self.admits
    }

    /// How many cases expect a refusal, by a statement or by no policy at all.
    #[must_use]
    pub const fn denies(&self) -> usize {
        self.denies
    }

    /// How many cases expect an escalation.
    #[must_use]
    pub const fn escalates(&self) -> usize {
        self.escalates
    }

    /// One line per failing row, naming the case, both answers and the reason the case exists.
    #[must_use]
    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    /// Whether every row passed.
    #[must_use]
    pub fn holds(&self) -> bool {
        self.failures.is_empty()
    }
}

impl core::fmt::Display for PackConformance {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "cases={} passed={} failed={} admits={} denies={} escalates={}",
            self.rows.len(),
            self.rows.len() - self.failures.len(),
            self.failures.len(),
            self.admits,
            self.denies,
            self.escalates
        )
    }
}

/// 🔴 Run a case table against a gate -- **the one road** every pack's conformance takes (G-4).
///
/// The gate is the caller's: `Gate::with_policies(packs::git_pack()?)` for a shipped pack,
/// `Gate::with_policies(PolicyEngine::parse(text)?)` for one a stranger sent, and either of those
/// with `.with_invariants(...)` for a deployment that registered its own. Nothing here can tell the
/// three apart, which is the property G-4 asks for.
///
/// # Errors
/// [`Error::Unevaluable`] if a case does not describe a transformation this build can represent --
/// an order above ASM-6's 2, or a negative timestamp. The id it names is the all-zero provisional
/// one, because the transformation the caller described was never constructed: 42 §1.3 keeps `id`
/// out of the identity view, so the placeholder names no other value.
///
/// A case the **gate** refused is not an error: it is a row with `actual: None` and the refusal in
/// `detail`, counted as a failure. E-M3-3 is why -- ⊥ is neither `Deny` nor `Escalate`, and folding
/// it into either would be the fail-open req/29 §4 forbids.
pub fn check_pack(gate: &Gate, cases: &[PackCase]) -> Result<PackConformance> {
    let mut rows = Vec::with_capacity(cases.len());
    let mut failures = Vec::new();
    let (mut admits, mut denies, mut escalates) = (0usize, 0usize, 0usize);

    for case in cases {
        match case.expect.kind() {
            VerdictKind::Admit => admits += 1,
            VerdictKind::Deny => denies += 1,
            VerdictKind::Escalate => escalates += 1,
        }

        let (t, pre, planned) = hypothetical(case)?;
        let answered = gate.verify(GateInput {
            t: &t,
            pre: &pre,
            planned: &planned,
            evidence: &case.evidence,
            invert_available: case.invert_available,
            // E-DR4627-1. A pack's conformance run is a **hypothetical** and G-4 asks that nothing
            // here tell a shipped pack from a stranger's; reading a real clock would make the same
            // case table answer differently on two machines, which is the opposite of what a
            // conformance runner is for. `hypothetical` declares `created_at: Timestamp(0)`, so the
            // case's own declared moment is the only moment in scope, and that is the one passed.
            decided_at: t.created_at,
        });

        let (actual, deciding, detail) = match &answered {
            Ok(verdict) => {
                let deciding = deciding_ids(verdict);
                (Some(verdict.kind()), deciding, None)
            }
            Err(e) => (None, Vec::new(), Some(e.to_string())),
        };
        let pass = matches(&case.expect, actual, &deciding, answered.as_ref().ok());
        if !pass {
            failures.push(format!(
                "{}: expected {:?}, answered {:?} by {:?}{} ({})",
                case.name,
                case.expect,
                actual,
                deciding,
                detail.as_ref().map_or(String::new(), |d| format!(" [{d}]")),
                case.why
            ));
        }
        rows.push(PackRow {
            name: case.name.clone(),
            actual,
            deciding,
            pass,
            detail,
        });
    }

    Ok(PackConformance {
        rows,
        admits,
        denies,
        escalates,
        failures,
    })
}

/// Which statements the gate recorded as having decided.
///
/// Read off the verdict rather than off the policy set: what a conformance table is about is which
/// rule answered *this* request, and a set's ids say only which rules exist.
fn deciding_ids(verdict: &Verdict) -> Vec<String> {
    match verdict {
        Verdict::Admit(proof) => proof
            .policy_decisions()
            .iter()
            .map(|record| record.policy_id.clone())
            .collect(),
        Verdict::Deny(reasons) => reasons
            .iter()
            .filter_map(|reason| match reason.source() {
                ReasonSource::Policy { policy_id } => Some(policy_id.clone()),
                _ => None,
            })
            .collect(),
        Verdict::Escalate(_) => Vec::new(),
    }
}

/// Whether an answer met an expectation.
///
/// `DenyByNoPolicy` is checked against the **source** rather than against an empty id list, because
/// an invariant's refusal also records no policy id: `ReasonSource::NoPolicyApplied` is the value
/// E-M3-11 introduced for "nothing in the set applied" and it is the only thing that means it. (sem: SEM-gx-gate-105)
fn matches(
    expect: &PackExpectation,
    actual: Option<VerdictKind>,
    deciding: &[String],
    verdict: Option<&Verdict>,
) -> bool {
    if actual != Some(expect.kind()) {
        return false;
    }
    match expect {
        PackExpectation::Admit(by) | PackExpectation::Deny(by) => by
            .as_ref()
            .is_none_or(|id| deciding.len() == 1 && deciding[0] == *id),
        PackExpectation::DenyByNoPolicy => match verdict {
            Some(Verdict::Deny(reasons)) => {
                reasons.len() == 1 && *reasons[0].source() == ReasonSource::NoPolicyApplied
            }
            _ => false,
        },
        PackExpectation::Escalate => true,
    }
}

/// The `GateInput` a case describes.
///
/// The ids are placeholders and the two kinds of placeholder are different on purpose, as
/// `gx_cli::policy::hypothetical` found: `TransformationId` is the all-zero provisional value
/// because 42 §1.3 keeps `id` out of the identity view, and everything else is non-zero because
/// `Transformation::new` refuses an all-zero `intent_id` by name -- a zero there would be a
/// transformation claiming to come from a draft that cannot exist.
fn hypothetical(case: &PackCase) -> Result<(Transformation, ObjectSnapshot, PlannedDeltaBytes)> {
    let provisional = Cid([0u8; 32]);
    let placeholder = Cid([1u8; 32]);
    let pre = ObjectSnapshot::new(
        ObjectId(placeholder),
        case.substrate.clone(),
        case.locator.clone(),
        placeholder,
        ReprKind::Bytes,
    );
    let t = Transformation::new(
        TransformationId(provisional),
        case.order,
        Subject::Object(ObjectId(placeholder)),
        None,
        Vec::new(),
        CompositionMetadata {
            intent_id: IntentId(placeholder),
            delta: DeltaRef {
                substrate: case.substrate.clone(),
                cid: placeholder,
            },
            context: case.context.clone(),
            actor: Actor::Human {
                key: case.actor_key.clone(),
            },
            created_at: Timestamp(0),
        },
    )
    .map_err(|e| Error::Unevaluable {
        transformation: TransformationId(provisional),
        detail: format!(
            "the case {:?} does not describe a transformation this build can represent: {e} (v0.1 \
             admits order <= 2, ASM-6)",
            case.name
        ),
    })?;
    Ok((t, pre, PlannedDeltaBytes(Vec::new())))
}

// ---8<--- end of the one conformance road ---8<---
