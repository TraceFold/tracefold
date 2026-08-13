//! The substrate boundary: what an adapter is asked to promise, and the delta values it moves.
//!
//! Spec: 41 §2 places this crate in the workspace and calls it 「P-6の担保点」, 41 §4 gives the
//! `SubstrateAdapter` trait, 42 §3.4 gives the field tables of `PlannedDelta` and `AppliedDelta`.
//!
//! # What this hand builds, and what it does not
//!
//! M4 hand 1 created the crate and the data skeleton; hand 2 added [`adapter::SubstrateAdapter`] --
//! 41 §4's seven methods with their contracts and quantifiers. The shared contract harness
//! `gx-substrate-conformance` is hand 3 and `gx-adapter-fs` is hand 4
//! (`req/69_M4_FORMAL_REQDEF_2026-08-09.md` §6.2). Nothing here reads a clock, opens a file or
//! computes a digest -- the trait says what an adapter promises and implements none of it; a crate
//! that does none of those cannot be an adapter, and it is not meant to be one.
//! `crates/gx-substrate/tests/substrate_contract.rs` asserts that absence rather than leaving it to
//! be noticed later.
//!
//! Two types 42 §0 files here live in gx-core instead, and the reason is one ruling applied twice:
//! **E-M4-1** puts `Fingerprint` there and re-reads 42 §0's `fingerprint.rs` row as 「計算の所在」,
//! and ASM-16 already put `DeltaRef` there. The rule both follow is E-M2-1's 「型は下層・計算は上層」
//! -- a receipt (gx-witness) must be able to name a fingerprint without depending on an adapter.
//!
//! # Locator normalisation (normative)
//!
//! This section is a **contract on every implementation of `SubstrateAdapter`**, not a description
//! of one. It exists because the gate decides on locator strings: M3-10 fixed the effective range
//! of a v0.1 policy pack at 「locator/actor/context/order 級」, so `/etc/../etc/passwd` and
//! `/etc/passwd` are the same file and different policy subjects. Left unnormalised that is a
//! fail-open path, which is 45 TH-2 arriving through a spelling instead of through a syscall.
//!
//! The obligation is **H-2** (`req/38_ERRATA_2026-08-07.md` §25) 逐語: 「M4 adapter 契約の必須
//! ticket『**locator は正規化済みで gate に渡す**』を予約」. What the normalisation *is* was then
//! fixed by **E-M4-12** (§28), and the five clauses below are that ruling:
//!
//! 1. **字句(lexical)のみ.** The normalisation is a function of the string and of nothing else. It
//!    performs no I/O, which is what keeps `plan` a pure function (M4-04) and keeps a `Fingerprint`
//!    scope from depending on a filesystem read taken at a different moment than the CAS check.
//! 2. **dot-segment 畳込み.** `.` is removed and `..` cancels the segment before it, as a matter of
//!    text. A leading `..` that cannot cancel is the adapter's to define and to write down.
//! 3. **重複区切り除去.** Repeated separators collapse to one, so `/a//b` and `/a/b` are one
//!    spelling.
//! 4. **末尾区切り規約.** Whether a trailing separator is kept or dropped is decided once per
//!    adapter and stated in its documentation; what is forbidden is deciding it per call site.
//! 5. **symlink/realpath 解決は v0.2+.** Resolving a symbolic link is a filesystem read and would
//!    put I/O back inside clause 1, so v0.1 does not do it. E-M4-12 files the resolution as a v0.2+
//!    ticket.
//!
//! Clause 5 leaves a hole and the hole is named rather than papered over. **TH-2 残存**: a locator
//! reaching a policy-protected path through a symbolic link is not closed by lexical normalisation,
//! so an actor who can create links can still choose which spelling the gate sees. 45 §2.2 already
//! limits the v0.1 claim to 「adapter経由の完全性のみ」 and 45 §4 forbids overclaiming beyond it;
//! E-M4-12 requires this residue to appear 「doc と受領書側 disclosure に明示」, so an adapter that
//! normalises lexically must say so where a receipt reader can see it, and must not describe its
//! locators as canonical paths.
//!
//! What an adapter still owns: the separator, the case and byte-vs-character question (E-M4-12
//! makes the fs adapter byte-literal, as an ASM in hand 4), and the equivalence relation `≈` that
//! 42 §2.3 requires 「`SubstrateAdapter`実装のドキュメントに記載を必須とする」. The property that
//! measures the whole clause list is L7 (req/69 §3.4) -- `normalize(normalize(l)) == normalize(l)`
//! and `l ≈ l' → normalize(l) == normalize(l')` -- and it is hand 4's, over a real implementation.
//! There is no normalisation function in this crate, deliberately: it is per adapter, and a shared
//! one here would be a path grammar in the boundary crate, which is the road M3-10 refused for the
//! gate.
//!
//! # Composite deltas (normative)
//!
//! This section is a **contract on every implementation of `SubstrateAdapter`** and, more than that,
//! a statement of what gx does **not** claim. It is the ruling **M4-07 採(c)** (`req/38` §28) and it
//! closes the hole D-8 / E-M3-14 left open:
//!
//! > 「合成 delta は **free monoid**——fs payload を「単一 file 操作の列」とし、列の連結が合成の witness。
//! > **trait は 1 行も変えず**(P-6 無傷・F1 不発)。**一般法則(合成 arrow の delta=部分の合成)は主張しない**
//! > 事を doc に明記。**T2 の一次候補は生存**」
//!
//! Four clauses, in the order they bite.
//!
//! 1. **A composite lives in the payload, not in the trait.** `SubstrateAdapter` has no
//!    `compose_delta`, and req/69 §4 M4-07 measured that no line of 41 or 42 names one. A composite
//!    delta is therefore whatever an adapter writes into a `payload`, which 42 §3.4 already calls
//!    「adapter定義スキーマ」. That is what 「trait は 1 行も変えず」 buys: P-6 is untouched, and F1's
//!    change-action algebra (12 F1, v0.2+) stays unfired.
//! 2. **The shape is a free monoid.** For the fs adapter of v0.1 the payload is a 「単一 file 操作の列」
//!    and **列の連結が合成の witness** -- concatenation, associative, with the empty sequence as the
//!    unit. That is the smallest algebra that composes at all: no commutativity (ASM-2 gave up the
//!    commutator, and M4-25 replaced it with a symmetric *independence predicate*), no inverses
//!    (41 §4's `invert` is partial and source-relative), no subtraction (research/03 A3: 「minus sign
//!    is unfunded」).
//! 3. **No general law is claimed.** gx does **not** state that the delta of a composite arrow is the
//!    composition of its parts' deltas. It cannot: `gx_core::compose` takes the composite's delta as
//!    an argument supplied by the caller (E-A7-2), so nothing in the type system relates the two, and
//!    a category-theoretic reading of req/69 §3.2 says why -- there is a category of arrows and there
//!    is a naming map from `PlannedDelta` into it, and nobody has shown the naming map closed under
//!    composition. **一般法則(合成 arrow の delta=部分の合成)は主張しない** is written here as a
//!    refusal rather than left as a silence, because an unstated law and a refused law read the same
//!    in prose and differently in a review.
//! 4. **T2 stays a candidate rather than a result.** E-M3-14 (D-8) put 「不変条件合成」 first in line
//!    「M4 で合成 delta 意味論が決まった時」. Clause 2 gives the sequence an associative composition to
//!    reason about, so the Lean side of T2 has a subject; clause 3 says gx claims nothing yet. If T2
//!    does not go up in this milestone the honest sentence is 「M4 でも上がらなかった」 and the fallback
//!    is M8's differential testing.
//!
//! ## Where this sits among the patch theories, and what gx declines to take
//!
//! req/69 §9 lists three systems to read before writing 「採らない理由」. The primary sources were
//! re-opened by hand 3 and recorded in `Desktop/GitRepo/REFERENCES.md`; what follows is the shape of
//! the difference, and no line of anybody's code.
//!
//! * **darcs** makes patches first class and requires that **全域** -- every patch has an inverse --
//!   then uses commutation to slide a patch to the end of a sequence before inverting it. gx's
//!   `invert` returns `Result<Option<PlannedDelta>>`: **gx の逆=部分**, and source-relative besides
//!   (it takes `pre`). The consequence is not a weaker library but a different claim -- DR-1(a)'s
//!   verified undo is the *escrowed* inverse of a particular delta at a particular state, and
//!   M4-21's ceiling makes `Ok(None)` a real answer rather than a theoretical one.
//! * **pijul** defines merge as a **pushout** and, where none exists, extends the category so that
//!   one does -- conflicts become first-class objects and are themselves resolvable by patches. gx
//!   does the opposite on purpose: 43 §8 has the engine ask `commutation(a, b)` and, on
//!   `Conflicts{residual}`, stop. **合流を解かず Conflicts で止める** is the whole design, because a
//!   system whose output is a signed decision record may not invent a state neither party asked for.
//! * **git** stores each commit as a **snapshot** and reconstructs a merge from three points: the two
//!   tips and their merge base. gx's `invert(δ, pre)` carries the same idea in its signature -- `pre`
//!   is the explicit base point -- which is why req/69 §3.2 reads an arrow as having endpoints.
//! * **CRDTs** (Automerge and its family) make operations commute so replicas converge, and pay for
//!   it with tombstones: an undo is not an inverse element. That the inverse is not obvious is a
//!   property of the problem rather than a weakness of gx.
//!
//! ## What is deliberately absent from this crate
//!
//! No `enum DeltaOp`, no sequence type, no decoder. Giving the list a shape here would put the fs
//! delta grammar in the boundary crate, which is exactly what clause 1 avoids and what L8 (measured
//! by `gx-substrate-conformance`'s `tests/opacity.rs`) asserts of the five crates below.
//! `crates/gx-substrate/tests/delta_semantics.rs` holds this section to its clauses.

#![forbid(unsafe_code)]

pub mod adapter;
pub mod delta;
pub mod error;
pub mod fingerprint;

pub use adapter::SubstrateAdapter;
pub use delta::{AppliedDelta, PlannedDelta};
pub use error::{Error, Result, ERROR_KINDS};
pub use fingerprint::elide_scope;
