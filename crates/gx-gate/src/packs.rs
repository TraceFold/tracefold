//! The pack a deployment starts from, and the one road it takes into a build (FR-028).
//!
//! Spec: 32 FR-028 逐語 -- 「gx-gateは出荷policy pack（既製invariant集）を`policies/`配下に同梱
//! しなければならない（MUST）。fs substrate向けpackはM3で同梱する…各policy packが少なくとも1つの
//! AdmitケースとDenyケースをconformanceテストで持つことを確認できる」 -- with 34 AC-028 stating the
//! conformance half and 41 §2 naming this module. Why a pack ships at all is 35 RSK-5: 「Policy
//! authorship cold start（invariant/policyを書ける人が希少）」, mitigated by 「出荷時policy pack」 and
//! -- 45 §87 is explicit -- 「緩和するが解消しない」.
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
//! | `fs-permit-default` | a permit scoped to the fs substrate. Cedar is default-deny (「no request is authorized … unless there is a specific permit policy that grants it」), so without this statement an fs deployment admits nothing at all |
//! | `fs-deny-etc` | a forbid over `/etc` and everything under it. 「forbid overrides permit」, so it needs no exception carved into the permit |
//! | [`GIT_PACK_PATH`] | `policies/git/deny-nonbranch-refs.cedar` -- **M7 hand 2**, FR-028's git half. 34 AC-074 writes the directory and this hand writes the name |
//! | `git-permit-default` | the same argument on the git substrate: without a permit a git deployment admits nothing |
//! | `git-deny-nonbranch-refs` | a forbid over `refs/tags/*` and `refs/remotes/*` -- the two namespaces gx is not the writer of. The file itself carries why those two and not others, including why the obvious rule (CI configuration) **cannot** ship in v0.1: `MAX_PATH_DEPTH` is 1 in `gx-adapter-git`, so no locator with a nested path reaches a gate and a rule about one could never fire (req/99 §3 **D-4**) |
//! | [`MCP_PACK_PATH`] | `policies/mcp/deny-etc-resources.cedar` -- **M7 hand 4**, FR-028's mcp half |
//! | `mcp-permit-default` | the same argument again, on the mcp substrate |
//! | `mcp-deny-etc-resources` | a forbid over `file:` resources at or under `/etc` -- **the fs pack's rule, written on the substrate that is otherwise the road around it**. The file carries why the rule a reader expects (「forbid the `shell.exec` tool」) **cannot** ship: the tool's name lives in the payload and P-6 keeps a payload opaque, so a statement about it could never fire -- D-4 again, and the third time this project has had to write it down |
//!
//! Two statements per pack is a decision rather than a stopping point: the rules worth shipping are
//! the ones a pack can state truthfully at the visibility M3 has, and
//! [the table below](#what-a-pack-can-reason-over-m3-10) is that visibility. `req/65` §3 records what
//! was considered and left out for fs; the git file records the same for git.
//!
//! # The one road
//!
//! req/60 §5.2 asks hand 5 for 「pack の file が build に埋め込まれる経路が 1 本(=第 2 の経路を作らない
//! ・AC-014 と同型の機械検査)」. [`FS_PACK_SOURCE`] is that road for the fs pack and
//! [`GIT_PACK_SOURCE`] is the git pack's: one `include_str!` each, in this file, naming the file on
//! disk. The claim is **per pack** -- one embedding of one file -- and [`SHIPPED_PACKS`] is what
//! makes it countable now that there is more than one.
//!
//! # The other one road (**G-4**)
//!
//! A second road is counted in this file, and it is not the same question. FR-028's is 「how do these
//! bytes reach a build」; G-4's (`req/38_ERRATA_2026-08-07.md` §19, req/98 §3-4) is 「when a third
//! party's pack is checked, does it go the same way ours does」 -- 「conformance 検査が同じ 1 本の経路
//! で走る(自社/第三者で分岐しない)」. [`check_pack`] is that road: it takes a [`Gate`] and a case
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
//! req/38 §19 rules the range and requires it be said out loud: 「M3-10(採用=案 b+a 起票): v0.1 pack の
//! 実効範囲=**locator/actor/context/order 級と明記**(overclaim 禁)」. What follows is that range,
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
//! 1. **The change itself is invisible.** 42 §3.4 makes a delta's `payload` 「opaque な変更記述。
//!    core/gate/witness は byte 列としてのみ扱う(P-6)」, so a rule of the form 「this change must not
//!    delete more than ten lines」 cannot be written -- not 「is not written yet」. 45 TH-1's
//!    line-count invariant and the 「既製invariant集」 FR-028 names in passing both live behind that
//!    wall until an adapter hands the gate structured facts, which req/38 §19 keeps as an M4
//!    requirement. `crates/gx-gate/tests/ac_028.rs` measures the wall rather than describing it: two
//!    changes with different payloads and the same locator get the same verdict.
//! 2. **Reads never arrive.** 読み取りは gate を通らない: every `Transformation` is a change to the
//!    thing it names (P-1), and 41 §3 has no read/write field, so a pack cannot write a rule about
//!    reading and does not need one -- a read was never going to reach this gate. (req/38 §21 C-8
//!    asks for exactly this line, because 「P-1 の帰結だが無言はやめる」.)
//! 3. **actor は key でのみ識別できる.** ASM-60-1 maps `t.actor.key()` and nothing else, so 41 §3's
//!    distinction between a `Human` and an `Agent` -- and the agent's `model` -- is not visible to a
//!    policy. 「agent の変更には追加 evidence を要求する」 is therefore unwritable in v0.1, and req/38
//!    §21's C-1 ruling keeps it that way on purpose: 「M3 では増やさない … 表現力の要求が実在するなら
//!    M4 の facts 経路と同窓で 1 度に」. FR-006/P-7's 「accountability は variant に依存しない」 is
//!    about who answers for a change, not about what a policy can branch on.
//!
//! And one thing the table above assumes rather than reads: **locator は与えられた綴りで判定される
//! (正規化は adapter の責務)**. A pack compares the string an adapter put in `pre.locator()`, so
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
//! receipt says without changing what was decided. req/38 §21 C-4 追認's it as ASM-62-1 and asks for
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
//! req/38 §22's D-9 leaves this hand the question 「order-2 の変換に追加承認を要求する既製 invariant を
//! pack に同梱するか」, with no ruling either way. It does not ship, for three reasons, and `req/65`
//! §3 carries them in full:
//!
//! 1. **Whatever it could say, a policy already says.** An invariant sees a `GateInput`; the order,
//!    the locator and the evidence summary in it are all mapped into the Cedar request already, so a
//!    rule about order-2 is one `when { context.order == 2 }` clause. Shipping a Rust body for it
//!    would be a second road to one rule -- the thing the road count above exists to prevent.
//! 2. **「追加承認」 is not an invariant's word.** An [`crate::InvariantCheck`] answers holds or does
//!    not hold, and a `false` becomes a `Deny` (**E-M3-6**'s `INVARIANT_VIOLATED`). Asking a human is
//!    the `Escalate` arm, which **E-M3-4** generates from `invert_available=false` and which 43 T-5
//!    resolves one layer up. A shipped invariant named 「requires approval」 that in fact refuses
//!    would teach the conflation the ruling asks the docs to avoid.
//! 3. **The invariant RSK-5 actually wants cannot be written.** 「行数保存」-shaped checks need the
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
/// 34 AC-025 writes this path 逐語, and req/38 §19's M3-16 puts the directory at the root: 「`policies/`
/// は **root 直下**(AC-025 の逐語 path 優先・出荷物は req/ 下に置かない)」. It is spelled here so that a
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
/// It reaches outside this crate's directory, which is the cost of M3-16's ruling that the pack
/// belongs at the repository root: `cargo package` does not follow such a path, so the day this
/// crate is published the road has to be reconsidered. Measured rather than assumed, and raised in
/// `req/65` §4 rather than pre-solved here.
pub const FS_PACK_SOURCE: &str = include_str!("../../../policies/fs/deny-etc.cedar");

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
/// 「which bytes did this gate decide with」 has one answer. Build a gate from it with
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
/// FR-028's 「経路が 1 本」 is a claim per pack rather than per repository: one embedding of *this*
/// file, so that the bytes in the build and the bytes on disk cannot diverge. `SHIPPED_PACKS` is what
/// makes the claim countable for a set of packs, and `crates/gx-gate/tests/pack_embedding.rs` counts.
pub const GIT_PACK_SOURCE: &str = include_str!("../../../policies/git/deny-nonbranch-refs.cedar");

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
pub const MCP_PACK_SOURCE: &str = include_str!("../../../policies/mcp/deny-etc-resources.cedar");

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
    /// 1. [`shipped_pack_set`]'s locality obligation -- 「each pack decides only its own substrate」
    ///    is the property that makes composing the packs safe, and it cannot be stated without
    ///    knowing whose substrate each one is.
    /// 2. `crates/gx-gate/tests/false_admit.rs`'s **vector expiry** (**H-9**). A negative vector
    ///    whose expectation is 「no shipped pack speaks for this substrate」 stops being true the day
    ///    one does, silently, and this field is what a harness reads to notice.
    ///
    /// Declared and then **checked against the file**: `crates/gx-gate/tests/pack_embedding.rs::
    /// the_declared_substrate_is_the_one_every_statement_scopes_on` reads the `resource.substrate ==
    /// "…"` clauses out of the pack's own text and refuses a pack whose statements name any other.
    /// A declaration nobody compares with the artifact is the failure mode this whole module is
    /// built against.
    pub substrate: &'static str,
}

/// Every pack this build ships (FR-028).
///
/// Three as of **M7 hand 4**. A pack file that appears under `policies/` without a row here is caught
/// by `crates/gx-gate/tests/pack_embedding.rs::nothing_ships_in_policies_that_no_build_loads`.
///
/// 🔴 **What this table now does say, and did not until hand 4**: that a deployment loads these.
/// Until this hand `gx_cli::session::open_engine` built its gate from the fs pack alone, and the
/// note here read 「the day a surface registers a second adapter, the default policy set has to
/// become the shipped **set**」. req/38 §60 ruled that day to be this one (**R-9 の対裁定**), and
/// [`shipped_pack_set`] is the value the CLI now starts from.
pub const SHIPPED_PACKS: [ShippedPack; 3] = [
    ShippedPack {
        path: FS_PACK_PATH,
        source: FS_PACK_SOURCE,
        policy_ids: &FS_PACK_POLICY_IDS,
        substrate: "fs",
    },
    ShippedPack {
        path: GIT_PACK_PATH,
        source: GIT_PACK_SOURCE,
        policy_ids: &GIT_PACK_POLICY_IDS,
        substrate: "git",
    },
    ShippedPack {
        path: MCP_PACK_PATH,
        source: MCP_PACK_SOURCE,
        policy_ids: &MCP_PACK_POLICY_IDS,
        substrate: "mcp",
    },
];

/// 🔴 **Every shipped pack, as one policy set** -- what a deployment that named no pack decides with.
///
/// **M7 hand 4**, implementing the pair `req/38` §60 ruled: 「既定 policy set と既定 registry は**対**
/// で決まる(pack を足しても adapter が register されなければ NotFound)」. req/101 §9-1 is the material
/// the ruling took, and its first change点 is this function: 「`gx_gate::packs` に
/// **`shipped_pack_set()`**(`SHIPPED_PACKS` 全部を 1 つの `PolicyEngine` に parse する)を足す。
/// `SHIPPED_PACKS` は既に在るので、これは新しい宣言ではなく既存宣言の消費者である」.
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
/// rules**. req/101 §9-1: 「それは緩和ではなく『判定されるようになる』であり、拒否面としては
/// `git-deny-nonbranch-refs` と(手 4 の)mcp forbid が**新たに到達可能になる**」. The two vectors
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
// 🔴 **G-4** (`req/38_ERRATA_2026-08-07.md` §19, req/98 §3-4): 「第三者 pack を投入した時、conformance
// 検査が**同じ 1 本の経路**で走る(自社/第三者で分岐しない)」.
//
// Everything between these two markers is the road. It is library code rather than test code,
// because a third party cannot call a test: `gx policy test <PATH> --scenario <FILE>` (44 §1.2) is
// the operator's face of this function, `crates/gx-gate/tests/ac_074.rs` is the shipped git pack's,
// and both reach the same lines. What the road knows is a gate and a list of cases; it does not know
// where the pack came from, and `ac_074.rs::the_pack_conformance_runner_names_no_pack` scans this
// region for every word an origin branch would have to be spelled with.
//
// The judgement is `Gate::verify`'s and is not repeated here (req/60 §7.2: 「Cedar の決定を gx の
// test が再実装しない」). What this adds is the comparison with an expectation and the arithmetic
// AC-028 and AC-074 both ask for -- 「最低1 Admitケース・1 Denyケース」 -- which is a statement about
// the case table and not about any one case.

/// What a conformance case expects a gate to answer.
///
/// Both the arm and, optionally, **which statement answered**. The id matters because 42 §1.3 puts
/// `PolicyDecisionRecord` inside `AdmitProof`'s IdentityView: two packs can reach `Admit` for
/// opposite reasons, and a table that asserted only the arm would pass on a pack whose permit was
/// deleted and whose deny was widened into an allow. `None` is the weaker form a `--scenario` file
/// can express (44 §1.2 gives it 「期待Verdict」 and no id), and it is written as an option rather
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

    /// 42 §3.7's values, passed to the gate 「そのまま」 (AC-016).
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
    /// The arm the gate reached, or `None` if it could not evaluate the case (**E-M3-3**: 「the
    /// policy could not be evaluated」 and 「the policy said something else」 are different facts).
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

    /// How many cases **expect** an admission. 「最低1 Admitケース」 is a statement about the table,
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
/// E-M3-11 introduced for 「nothing in the set applied」 and it is the only thing that means it.
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
