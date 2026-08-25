// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The git adapter: a file on a branch, moved by a commit, undone by a reference reset.
//!
//! Spec: 41 §2 for the crate and its dependency line ("`gx-adapter-git/  # dep: gix (gitoxide)`") (sem: SEM-gx-adapter-git-062),
//! 41 §4 for the seven methods, 42 §3.4 for the delta it carries, 42 §3.5 for the fingerprint it
//! computes, 32 **FR-045** for the obligation ("`SubstrateAdapter`'s methods MUST all be implemented **via gix**")
//! and 34 **AC-050** for what is measured. The M7 requirement definition
//! is `req/98_M7_REQDEF_2026-08-11.md` §7-2 hand 1, ratified by `req/38` §57. (sem: SEM-gx-adapter-git-063)
//!
//! # What an object is here, and what a scope is (normative)
//!
//! 42 §3.5 draws a line this adapter is the first to need. An **object** is what a change is about;
//! a **scope** is "the surrounding state that could interfere with the target", which the specification explicitly lets an adapter widen (sem: SEM-gx-adapter-git-064)
//! past the object itself. `gx-adapter-fs` collapses them -- a file is its own scope (ASM-69-1) --
//! and git cannot:
//!
//! | | this adapter |
//! |---|---|
//! | **object** | the file at one path, on one branch, in one repository. Its digest is the digest of that file's bytes, which is the same value `gx-adapter-fs` computes for a file, through the same `gx-canon` road (41 §6 admits one hash). |
//! | **scope** | the **branch**. Its digest is the digest of the commit the branch points at. |
//!
//! The reason is that a git change is not local. Rewriting one file rewrites its tree and mints a
//! new commit and moves the branch, so **two changes to two different files on one branch are not
//! independent** -- both move the same reference, and the second would have to be rebuilt on the
//! first. An adapter that made the scope the file would answer `Commutes` for them, and 43 §8 acts on
//! that answer by letting both proceed. `Commutes` is the fail-open direction (M4-25), so the scope
//! is the branch and [`commutation`] compares scopes.
//!
//! What this costs is stated rather than hidden: **two changes to two different files on one branch
//! conflict**, and one of them waits. That is the true shape of a branch and not a limitation of this
//! version.
//!
//! # The locator grammar (normative)
//!
//! ```text
//! locator := <repository path> "#" <ref> ":" <path>
//! ```
//!
//! e.g. `/srv/repo#refs/heads/main:README.md`. All three parts are required; a spelling missing
//! either separator is not a position ([`gx_substrate::Error::NotAPosition`]) rather than a position
//! that fails to apply -- the distinction **M4H5-5, adopted (b)** made for the fs adapter, for the reason 43 (sem: SEM-gx-adapter-git-065)
//! T-11 gives (`ApplyFailed` becomes `AbortReason::ApplyFailed`, which would record a change that
//! failed where no change was ever describable).
//!
//! The **scope** of a locator is its first two parts, `<repository path> "#" <ref>`, and that string
//! is what reaches [`gx_core::Fingerprint`]. Over [`gx_core::MAX_SCOPE_BYTES`] it becomes a digest
//! line before the type is constructed ([`gx_substrate::elide_scope`], **M4H1-2**) -- the same single
//! road the fs adapter takes, which is what **req/98** §3-4's reserved item 6 asks of the M7 adapters
//! ("git/mcp's locator route refuses `ScopeTooLong` **at construction time**"). (sem: SEM-gx-adapter-git-066)
//!
//! # The equivalence `≈` (normative)
//!
//! 42 §2.3 requires this section: "this equivalence relation `≈` is explicitly defined per adapter...
//! and its documentation in a `SubstrateAdapter` implementation is **required** to record it", and **M4-22** made its presence a (sem: SEM-gx-adapter-git-067)
//! machine check. Two locators are `≈` for this adapter -- **they name one position** -- exactly when
//! [`locator::normalize`] maps them to the same string. The normalisation is the definition rather
//! than an implementation of one, and it is **purely lexical** (**E-M4-12, adopted (a)**): a function of the (sem: SEM-gx-adapter-git-068)
//! text, performing no repository read at all. Five clauses, each a rule about characters:
//!
//! 1. **The repository path folds like a path.** A `.` segment is dropped, a `..` cancels the segment
//!    before it, repeated separators collapse and a trailing separator is dropped except at the root
//!    -- the fs adapter's clauses 1-3, applied to the first part only. The path must be **absolute**
//!    (ASM-69-3's reason: "where the process happens to be" is not a fact a signed record may depend (sem: SEM-gx-adapter-git-069)
//!    on).
//! 2. **A reference is spelled in full.** `main` and `heads/main` and `refs/heads/main` are one
//!    reference: an unqualified name is a **branch**, and a name that already begins `refs/` is left
//!    alone. This is a rule about text and not a lookup -- git's own resolution order (which would
//!    also consider a tag of that name) needs the repository, and a normalisation that read the
//!    repository would put an I/O inside every scope computation.
//! 3. **The tree path is relative and folds.** A leading separator is dropped (nothing in a tree is
//!    absolute), `.` and `..` fold, repeated separators collapse, a trailing separator is dropped.
//! 4. **The bytes are the bytes** (**ASM-69-2**). No case folding and no Unicode normalisation: git
//!    stores a tree entry's name as bytes and compares them as bytes, so an adapter that folded case
//!    would disagree with the object store exactly where it mattered.
//! 5. **A `..` that would climb out** of the tree path, or of the repository root, is dropped: there
//!    is nothing above either to name.
//!
//! # What v0.1 does not close
//!
//! **The tree path is one component.** [`delta::MAX_PATH_DEPTH`] is 1, so `README.md` is a position
//! and `src/main.rs` is refused. A nested path means rebuilding every tree between the file and the
//! root, which is *n* object writes for one change and *n* more shapes for a failure to take; 42's
//! delta is "a single change" and this version keeps that literal. The refusal is explicit (sem: SEM-gx-adapter-git-070)
//! ([`gx_substrate::Error::NotAPosition`]) and never silent, and the residue is raised in `req/99`.
//!
//! **The commits this adapter writes are dated at the epoch.** 41 §6: "randomness and time are injected at the engine boundary
//! (for deterministic replay)", and a commit's timestamp is *part of its object id*. An adapter that read (sem: SEM-gx-adapter-git-071)
//! a clock would therefore mint a different commit for the same change on every attempt -- the same
//! delta would not be the same delta, and 51 §7 contract 7's idempotence would be unreachable by (sem: SEM-gx-adapter-git-072)
//! construction. So author and committer time are `0 +0000` and the real moment lives where it is
//! signed: in the gx receipt. See [`repo::gx_signature`]. A reader of the git history alone sees
//! epoch dates; a reader of the receipt sees when it happened, and only one of those two is
//! tamper-evident.
//!
//! **No confinement.** As with the fs adapter (**N-05**), there is no sandbox: any repository this
//! process can write is a repository this adapter will write, and what stands between an intent and a
//! branch is the **gate** and nothing in this crate (45 §2.2's "completeness only through the adapter"). (sem: SEM-gx-adapter-git-073)
//!
//! **A reference reset is a reset.** [`invert`] undoes a commit by putting the branch back where it
//! was, which is what **AC-050** asks for in as many words -- "the repo's HEAD and tree hash return to before the operation
//! bit-equal" -- and a revert commit could not satisfy it (a revert restores the tree and moves (sem: SEM-gx-adapter-git-074)
//! HEAD forward). The commit that was made is **not** removed from the object database; it becomes
//! unreachable. So an undo here is "the branch is where it was" and not "the change never existed", (sem: SEM-gx-adapter-git-075)
//! and `git reflog` still shows both. That is the honest sentence and it is not the softer one.
//!
//! # 🔴 The escrow ceiling has no instance here, and that is a finding rather than an omission
//!
//! **M4-21, adopted (a)** bounds what an adapter will carry back, because an fs inverse carries the whole old
//! file (42 §5: "because a digest-only inverse makes an actual undo physically impossible") and an unbounded body would reach a (sem: SEM-gx-adapter-git-076)
//! journal. A git inverse carries **an object id**: the old content is already in the object database,
//! addressed by content, and putting the branch back is a pointer move. So the ceiling that fires for
//! `gx-adapter-fs` cannot fire here at all, and declaring a constant that no input can reach would be
//! a refusal nobody asked for (52 contract 2). (sem: SEM-gx-adapter-git-077)
//!
//! The `Ok(None)` of 51 §7 contract 5 therefore has a **different** instance, and a real one: a branch
//! with no commits on it (git's "unborn" state, which is what `git init` leaves). A change to a file (sem: SEM-gx-adapter-git-078)
//! on an unborn branch creates the branch, and its inverse would have to **delete** it -- an operation
//! v0.1 does not spell. `invert` answers `Ok(None)`, **E-M3-4** escalates to a person, and a person is
//! the right one to ask before a branch disappears. That is "a legitimate construction of the same object is not possible" in (sem: SEM-gx-adapter-git-079)
//! **E-M4-32**'s sense, and `MAX_FORWARD_PAYLOAD_BYTES` -- the bound on what this adapter will
//! *accept* -- is still declared, because the goal bytes do travel through a gate and into a journal.
//!
//! # `plan` reads nothing, and the reason is a law rather than a preference
//!
//! §30 M4H2-3, adopted (b) reads 41 §4's "pure function" for the trait as "determinism over the (intent, pre)
//! pair + zero **writes** to the substrate" and adds, naming this crate: "reading is not forbidden -- it does not close the road for M7's git adapter to read an object
//! store". The road is open and this adapter does not take it, and the reason is (sem: SEM-gx-adapter-git-080)
//! **L1** (req/69 §3.4): the same `(intent, pre)` planned twice, with the substrate moving in between,
//! has to produce the same delta. A plan that read the branch tip to use as a parent would produce a
//! different commit each time the branch moved, so the tip cannot be in the payload.
//!
//! What follows is the shape of the grammar: the payload says **what state the file should be in**,
//! not which commit will carry it. `apply` reads the tip -- at the moment the engine's CAS has just
//! declared current (41 §5-5b) -- and builds the commit there. `tests/git_plan_purity.rs` measures
//! both halves: the module names no gix write, and the repository's `.git` directory is byte-identical
//! across a `plan`.
//!
//! # How the seven were built
//!
//! All seven in one hand (M7 hand 1), so no method of this adapter answers
//! [`gx_substrate::Error::Unimplemented`] and `is_complete` -- "0 unmeasured", the vocabulary **§31 (sem: SEM-gx-adapter-git-081)
//! M4H3-4 (b)** created for exactly this milestone -- is true rather than deferred. The word stays in
//! the vocabulary regardless: `gx-adapter-mcp` is hand 3 and may stand up one method at a time.
//!
//! 51 §7's completion condition is bounded and the bound belongs beside it: the seven contracts and
//! the nine laws hold **against this fixture**, on a tmpfs, single-threaded, over repositories the
//! test created. It is not a claim about every git repository, about concurrent pushes, about packed
//! references, or about a repository whose objects are in a pack rather than loose.

#![forbid(unsafe_code)]

pub mod adapter;
pub mod apply;
pub mod commutation;
pub mod delta;
pub mod invert;
pub mod locator;
pub mod plan;
pub mod repo;

pub use adapter::GitAdapter;
pub use delta::{GitDelta, GitOp, MAX_FORWARD_PAYLOAD_BYTES, MAX_OPS, MAX_PATH_DEPTH};
pub use locator::{normalize, Position};
