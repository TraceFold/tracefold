//! The MCP adapter: a tool call that cannot reach a server except through an admitted `apply`.
//!
//! Spec: 41 §2 for the crate, 41 §4 for the seven methods, 42 §3.4 for the delta it carries, 42 §3.5
//! for the fingerprint it computes, 32 **FR-046** for the obligation (「gx-adapter-mcp は MCP tool-call
//! 境界を傍受し tool-call intent を `PlannedDelta` へ candidate 化するプロキシを実装しなければならない
//! （MUST）」) and 34 **AC-051** for what is measured. `22-roadmap.md:23` calls this crate 「**DR-1(a)
//! wedgeの実体**」. The M7 requirement definition is `req/98_M7_REQDEF_2026-08-11.md` §7-2 手 3,
//! ratified by `req/38` §59.
//!
//! # 🔴 Three sets, not two, and MCP is where they come apart
//!
//! `gx-adapter-fs` has one set: a file is its object, its CAS scope and its footprint (**ASM-69-1**).
//! `gx-adapter-git` has two: the object is an entry and the scope is the branch, and the branch is
//! **both** what a CAS reads and what a change disturbs. This adapter has **three**, and the reason is
//! that one of them is not observable:
//!
//! | | this adapter | why |
//! |---|---|---|
//! | **object** | the **resource** at one URI on one server. Its digest is the digest of that resource's bytes, through the same `gx-canon` road (41 §6 admits one hash) `gx-adapter-fs` and `gx-adapter-git` take. | MCP's `resources/read` is the one thing a proxy can observe about a server without asking it to do anything. |
//! | **CAS scope** (42 §3.5) | the **resource**, same as the object. | 51 §7 契約 3 requires the fingerprint to **move when the state moves**, so a scope has to be readable. The server as a whole is not. |
//! | **footprint** ([`commutation`]) | the **server**. | A tool's effects are the server's semantics, and a proxy has no map from a tool to the resources it touches. `Commutes` is the **fail-open** direction (M4-25), so two changes on one server are `Conflicts` and 43 §8 makes one of them wait. |
//!
//! `gx-adapter-git` can hold the last two together because a branch is readable *and* is what every
//! change to it moves. Here they are different sets and the honest thing is to say so rather than to
//! pick whichever one makes a table symmetric:
//!
//! * taking the **server** as the CAS scope would need a digest of the server, which does not exist;
//! * taking the **resource** as the footprint would answer `Commutes` for two tool calls on one
//!   server, and 43 §8 acts on that answer by letting both proceed.
//!
//! 🔴 **What that leaves open is written down rather than smoothed over.** A tool call whose effect
//! lands on a resource **other** than the one its transformation is about is invisible to the CAS: the
//! fingerprint reads the object, and the object is not where the effect went. What the membrane offers
//! against it is not detection but **serialisation** -- the footprint is the server, so nothing else on
//! that server runs beside it. Detection would need the server to tell a proxy what a tool touches,
//! which no part of MCP does.
//!
//! # The locator grammar (normative)
//!
//! ```text
//! locator := <server endpoint URI> "#" <resource URI>
//! ```
//!
//! e.g. `https://mcp.example/sse#file:///srv/notes.md`. Both parts are required and neither may be
//! empty; a spelling missing the separator is **not a position**
//! ([`gx_substrate::Error::NotAPosition`]) rather than a position that fails to apply -- the
//! distinction **M4H5-5 採(b)** made for the fs adapter, for the reason 43 T-11 gives.
//!
//! The **server endpoint carries a scheme** (`https://`, `stdio://`, `unix://`). ASM-69-3's argument,
//! one substrate over: a bare `mcp-1` is 「whichever server that name was pointing at when this ran」,
//! and 「where the process happens to be」 is not a fact a signed record may depend on.
//!
//! **A `#` in the resource URI is refused.** RFC 3986 spends `#` on the fragment, so a resource URI
//! may legally contain one and this grammar cannot then find its own separator. v0.1 refuses the
//! spelling instead of guessing which `#` was meant ([`gx_substrate::Error::NotAPosition`]); the
//! residue is raised in `req/101`.
//!
//! # The equivalence `≈` (normative)
//!
//! 42 §2.3 requires this section: 「この同値関係 `≈` はadapterごとに明示的に定義され…
//! `SubstrateAdapter`実装のドキュメントに記載を**必須とする**」, and **M4-22** made its presence a machine
//! check. Two locators are `≈` for this adapter -- **they name one position** -- exactly when
//! [`locator::normalize`] maps them to the same string. It is **purely lexical** (**E-M4-12 採(a)**):
//! a function of the text, performing no read of any server.
//!
//! 🔴 **Under-normalising a locator is a bypass**, which is why the clauses here are a standard's and
//! not this hand's taste. M3-10 fixes a policy pack's effective range at 「locator 級」, so two
//! spellings of one position are **two policy subjects**: a rule written on one is not in force on the
//! other. For an adapter whose acceptance criterion is 「バイパス手段が技術的に存在しない」, a normaliser
//! that leaves two spellings apart is a road around a policy. The four clauses are **RFC 3986 §6.2.2's
//! syntax-based normalisation**, applied to both parts:
//!
//! 1. **The scheme folds to lower case** (RFC 3986 §3.1: 「scheme names are case-insensitive」, §6.2.2.1).
//! 2. **The host folds to lower case** (§6.2.2.1). ASCII only: folding an internationalised authority
//!    lexically is a Unicode operation, and **ASM-69-2**'s 「the bytes are the bytes」 governs
//!    everything this clause does not name. Userinfo and port are left alone -- §6.2.2.1 names the
//!    scheme and the host and nothing else.
//! 3. **Percent-encoding is normalised** (§6.2.2.2): a triplet is spelled with upper-case hexadecimal,
//!    and a triplet that encodes an **unreserved** character (`A-Z a-z 0-9 - . _ ~`) is decoded.
//! 4. **Dot segments are removed from the path** (§6.2.2.3), by the algorithm of §5.2.4.
//!
//! What is **not** done is §6.2.3 (scheme-based: a default port, an empty path becoming `/`) and
//! §6.2.4 (protocol-based). Both need to know what a scheme means, and a normaliser that assumed
//! `https` semantics for a `stdio` endpoint would merge two positions a deployment distinguishes.
//! ∴ `https://x` and `https://x/` are **two** positions here, and a policy author writing a rule on
//! one is writing it on one. That residue is raised in `req/101` rather than left implicit.
//!
//! # 🔴 Why there is no way to call a tool except through `apply`
//!
//! **AC-051** 逐語: 「プロキシを経由しない直接呼び出し経路の有無を**構成レベルで検査**し、プロキシ経由呼び
//! 出しを実行する。Then: 全tool-callがsubmit→verify経路を必ず通過し、バイパス手段が技術的に存在しない」.
//! 裁定 #10 (req/38 §56) adds the form: **導出**, and 「宣言 list は不可」.
//!
//! The road count is not a list in a comment. It is what these two facts leave:
//!
//! 1. **[`transport::ToolCall`] and [`transport::Admitted`] cannot be constructed outside this
//!    crate.** Both have private fields and `pub(crate)` constructors, and
//!    [`transport::ToolTransport::call`] takes one of each. A deployment writes the transport -- that
//!    is the whole of what it writes -- and receives the two values; it cannot mint them. The
//!    population that fact closes is **every crate that is not this one**, and it is closed by the
//!    compiler rather than by a scan: `tests/ac_051.rs` carries the two `compile_fail` doctests.
//! 2. **Inside this crate, the two are minted in one place**, which is [`apply`] -- the module a gate
//!    has already admitted a delta before anything calls (41 §4: 「commit承認後にのみ呼ばれる」). That
//!    half is a text gate over this crate's own `src/`, enumerated by walking the directory rather
//!    than by naming the files, and its limit is printed beside it in the M6H8-1 form.
//!
//! Above the adapter the road is 則 2's, already measured: `gx-engine`'s `apply_once` is the only
//! call of `SubstrateAdapter::apply` in the workspace (req/78 §3.3, `gx-engine/tests/ac_035.rs`), and
//! `Engine::commit` reaches it only for a transformation the gate admitted. `tests/ac_051.rs` derives
//! that site again from this side and then **drives it**: a denying gate leaves the transport at zero
//! calls, an admitting one leaves it at exactly one.
//!
//! 🔴 **Two bounds, and neither is hidden.** A transport is the deployment's code: while it holds the
//! `&ToolCall` and `&Admitted` it was handed, nothing stops it from passing them to something else --
//! it can only replay *that* call, since it cannot build another, and a replay is what 51 §7 contract
//! 7 makes a no-op. And nothing here stops a **different process** from speaking to the same MCP
//! server directly: AC-051's subject is 「プロキシ配下の tool-call 境界」, and making the proxy the only
//! reachable endpoint is a deployment's job (45 §2.2's 「adapter経由の完全性のみ」, the same **N-05**
//! disclosure the fs and git adapters carry).
//!
//! # 🔴 Idempotence on an effect substrate is a record, not a comparison
//!
//! 51 §7 contract 7 and **E-M4-3** quantify idempotence over 「同一 delta の再入(retry)」, and 43 T-10c
//! is the situation: a crash between the write and the journal record, whose recovery is running the
//! same delta again. `gx-adapter-fs` and `gx-adapter-git` answer it by **comparing** -- the file
//! already holds these bytes, the branch already points at this commit -- because for both of them the
//! delta declares the state it wants.
//!
//! A tool call does not. What a call does is the server's, and a proxy that decided 「this call has
//! already been made」 from the resource's contents would be guessing. So this adapter keeps a
//! [`log::CallLog`] keyed by the delta's own CID and applies a delta at most once per record: the
//! retry re-observes and returns, and no second call goes out.
//!
//! **The consequence belongs in the same paragraph.** The idempotence of this adapter is a property of
//! the proxy's record, not of the substrate: a retry against a log that has forgotten the delta sends
//! the call again. [`log::MemoryCallLog`] is in-memory, so it forgets on exactly the event T-10c is
//! about, and a durable log is the deployment's ([`log::CallLog`] is the seam) until v0.2 ships one.
//! `req/101` raises it. Saying 「apply is idempotent」 without this sentence would be the more
//! comfortable claim and the false one.
//!
//! # What v0.1 does not close
//!
//! **No wire.** This crate frames no JSON-RPC and opens no connection: the transport is a trait and
//! the deployment implements it. The manifest says why (a linked client would be a second road to the
//! substrate), and what follows is that 「the proxy speaks MCP correctly」 is **not** among the things
//! this hand measured.
//!
//! **One operation per delta.** [`delta::MAX_OPS`] is 1, for **M4-13 採(a)**'s reason read for tool
//! calls: two calls in one delta are two effects that this version will not make silently non-atomic.
//!
//! **No confinement.** As with the fs and git adapters (**N-05**), any server this process can reach
//! is a server this adapter will call, and what stands between an intent and a tool is the **gate**.
//!
//! **An inverse is a compensating call, and many tools have none.** 42 §5 requires an escrowed inverse
//! to carry the body 「digest-onlyでは実際のundoが物理的に不可能なため」, and here it does: the inverse is
//! a call to the **restore tool** a [`catalogue::Catalogue`] declares, carrying the resource's prior
//! contents. A tool with no declared restore is one whose calls **cannot be undone**, and `invert`
//! answers `Ok(None)` so that **E-M3-4** asks a person. That is 「同一 object の正当な構成不能」 in
//! **E-M4-32**'s sense, and it is the honest shape of a wedge in front of effects.
//!
//! # 🔴 The escrow ceiling has an instance here, unlike git
//!
//! **M4-21 採(a)** bounds what an adapter carries back, because an unbounded body reaches a journal.
//! `gx-adapter-git` declares no such constant and argues why (a git inverse carries an object id: the
//! body is already escrowed in a content-addressed store), and req/99 §3 D-4 is the rule that stopped
//! it from declaring an unreachable one. **This adapter's inverse carries the whole prior content**,
//! exactly as the fs adapter's does, so [`delta::MAX_INVERSE_PAYLOAD_BYTES`] is declared, is
//! reachable, and answers `Ok(None)` over it.
//!
//! # How the seven were built
//!
//! All seven in one hand (M7 hand 3), so no method answers [`gx_substrate::Error::Unimplemented`] and
//! `is_complete` -- 「未測定 0」 -- is true rather than deferred.
//!
//! 51 §7's completion condition is bounded and the bound belongs beside it: the seven contracts and
//! the nine laws hold **against this fixture**, over an in-process server the test wrote,
//! single-threaded. It is not a claim about any MCP server, about concurrent sessions, or about a
//! transport that reorders.

#![forbid(unsafe_code)]

pub mod adapter;
pub mod apply;
pub mod catalogue;
pub mod commutation;
pub mod delta;
pub mod invert;
pub mod locator;
pub mod log;
pub mod plan;
pub mod transport;

pub use adapter::McpAdapter;
pub use catalogue::Catalogue;
pub use delta::{
    restore_arguments, McpDelta, McpOp, ToolIntent, MAX_FORWARD_PAYLOAD_BYTES,
    MAX_INVERSE_PAYLOAD_BYTES, MAX_OPS,
};
pub use locator::{normalize, Position};
pub use log::{CallLog, MemoryCallLog};
pub use transport::{Admitted, ToolCall, ToolTransport};
