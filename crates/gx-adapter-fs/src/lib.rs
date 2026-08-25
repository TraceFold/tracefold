// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The filesystem adapter: the first thing in gx that actually touches a substrate.
//!
//! Spec: 41 §2 for the crate, 41 §4 for the seven methods it implements, 42 §3.4 for the delta it
//! carries, 42 §3.5 for the fingerprint it computes. The rulings it implements are **M4-12** (the
//! normalisation), **M4-13** (the delta shape), **M4-16 / ASM-69-1** (the scope), **M4-17** and
//! **E-M4-31** (the clock), and **E-M4-29** (the purity of `plan`), all in
//! `req/38_ERRATA_2026-08-07.md` §28-§31. (sem: SEM-gx-adapter-fs-062)
//!
//! # What M4 hand 6 finishes, and what "finished" is bounded by (sem: SEM-gx-adapter-fs-063)
//!
//! All seven: [`FsAdapter::kind`](adapter::FsAdapter), `snapshot`, `plan`, `precondition` (hand 4),
//! `apply` / `invert` (hand 5) and **`commutation`** (this hand, req/69 §6.2). No method of this
//! adapter answers [`gx_substrate::Error::Unimplemented`] any more, and that variant is the only one
//! the shared harness reads as "none" (**§31 M4H3-4 (b)**), so every one of 51 §7's obligations now has (sem: SEM-gx-adapter-fs-064)
//! a subject: **`is_conformant` and `is_complete` are both true**, and `meets_51_7` with them.
//!
//! That sentence is bounded and the bound belongs beside it. 51 §7's completion condition is about
//! **the seven contracts against a fixture**, and this adapter's fixture runs on a tmpfs, in one
//! thread, over files it created (`tests/support/mod.rs`). It is not a claim about filesystems in
//! general, about crash durability (`fsync` on a tmpfs is effectively free -- see below), about
//! concurrency, or about the git and mcp adapters, which inherit the same harness in M7 with subjects
//! of their own. What is closed is "the seven questions 51 §7 asks, asked and answered". (sem: SEM-gx-adapter-fs-065)
//!
//! The word "unimplemented" stays in the vocabulary all the same (§32 M4H4-2 confirmed): "unimplemented" and "failed" are (sem: SEM-gx-adapter-fs-066)
//! permanently different facts, and M7's adapters will stand up one hand at a time as this one did.
//!
//! # No sandbox, and what that means now that this adapter writes
//!
//! Hand 4's version of this paragraph described a reader. §32 M4H4-10 made the rewrite a condition of
//! this hand -- "when hand 5 writes `apply`, rewrite the crate root's disclosure from 'readable' to 'readable, writable'" -- (sem: SEM-gx-adapter-fs-067)
//! because a disclosure that lags the code understates the system by exactly the thing that was added,
//! and 45 §4 forbids that direction.
//!
//! **N-05** (req/69 §1) keeps Landlock out of v0.1 -- 41 §2 files it as "v0.2+" -- so this adapter has (sem: SEM-gx-adapter-fs-068)
//! no confinement of its own. Concretely, as of this hand:
//!
//! * A locator is an absolute path and **any absolute path this process can write is a path this
//!   adapter will write**. `apply` creates, replaces and removes whole files, and it does not consult
//!   an allow-list, a root, or a chroot, because it has none of the three.
//! * The parent directory is opened and `fsync`ed, and a temporary file is created beside the target.
//!   So the reach is the **directory** as well as the file.
//! * The bound that does exist is the process's own credentials and the delta's grammar: one
//!   operation ([`delta::MAX_OPS`]), one whole file, no directory creation, no mode or owner change,
//!   no symbolic link followed by the adapter's own choice (the kernel still follows one when opening
//!   the path -- see TH-2 below).
//!
//! What stands between an intent and a file is therefore the **gate** and nothing in this crate --
//! 45 §2.2's "completeness only through the adapter". v0.2's Landlock ruleset is the ticket that narrows it, and (sem: SEM-gx-adapter-fs-069)
//! until it lands the honest sentence is the one above rather than a softer one.
//!
//! # The equivalence `≈` (normative)
//!
//! 42 §2.3 requires this section: "this equivalence relation `≈` is explicitly defined per adapter...
//! and its documentation in a `SubstrateAdapter` implementation is **required** to record it". **M4-22** made its presence a machine (sem: SEM-gx-adapter-fs-070)
//! check (`tests/locator_normalisation.rs`), because a requirement nobody measures is a requirement
//! that decays.
//!
//! Two locators are `≈` for this adapter -- **they name one position** -- exactly when
//! [`locator::normalize`] maps them to the same string. The normalisation is the definition rather
//! than an implementation of one, and it is **purely lexical**: a function of the text and of
//! nothing else, performing no filesystem read at all. That is **E-M4-12, adopted (a)**, and it is what (sem: SEM-gx-adapter-fs-071)
//! keeps `plan` a pure function (M4-04) and keeps a `Fingerprint`'s scope from depending on a read
//! taken at a different moment than the CAS check.
//!
//! Five clauses, and every one of them is a rule about characters:
//!
//! 1. **`.` and `..` fold.** A `.` **dot-segment** is dropped and a `..` cancels the segment before
//!    it, as text: `/etc/../etc/passwd` and `/etc/passwd` are one position. A `..` that would climb
//!    past the root is dropped, because there is nothing above `/` to name. In a **relative**
//!    locator a leading `..` cannot cancel and is kept -- dropping it would invent a position the
//!    caller did not write -- and the refusal happens where the locator is used
//!    ([`locator::is_absolute`]), not where it is spelled.
//! 2. **Repeated separators collapse.** `/a//b` and `/a/b` are one spelling; a **duplicate
//!    separator** carries no information on any filesystem this adapter speaks to.
//! 3. **A trailing separator is dropped, except at the root.** E-M4-12 leaves the **trailing
//!    separator** convention to each adapter and forbids only deciding it per call site; this
//!    adapter drops it, and keeps `/` itself because the empty string is not a position.
//! 4. **The bytes are the bytes** (**ASM-69-2**). No case folding and no Unicode normalisation: the
//!    comparison is **byte-literal**. Both questions are answered by a filesystem's own mount
//!    options, so an adapter that folded case would disagree with the kernel exactly where it
//!    mattered, and `NFC` would make two files with different names into one policy subject.
//! 5. **Positions are absolute** (**ASM-69-3**). v0.1 names a position from the root; a relative
//!    locator normalises (clause 1 still applies to it) but `snapshot`, `precondition` and `plan`
//!    refuse it, because "where the process happens to be" is not a fact a signed record may depend (sem: SEM-gx-adapter-fs-072)
//!    on.
//!
//! A **symbolic link** is not resolved. That is clause 1 taken seriously -- `realpath` is a
//! filesystem read -- and E-M4-12 files the resolution as a v0.2+ ticket. The residue it leaves is
//! the next section.
//!
//! # What v0.1 does not close
//!
//! **TH-2 residue.** Lexical normalisation does not close the path a **symbolic link** opens: an
//! actor who can create a link can still choose which spelling the gate sees, and `/tmp/link` and
//! the file it points at are two positions to this adapter and one file to the kernel. E-M4-12
//! requires the residue to be stated "made explicit in the doc and the receipt-side disclosure", so a receipt reader is (sem: SEM-gx-adapter-fs-073)
//! entitled to read this paragraph: **the locators in a gx receipt are normalised spellings, not
//! canonical paths**, and resolution is a **v0.2** ticket. 45 §2.2 already limits the v0.1 claim to
//! "completeness only through the adapter" and 45 §4 forbids overclaiming past it. (sem: SEM-gx-adapter-fs-074)
//!
//! **The narrow scope's residue.** ASM-69-1 puts only the file's own content under the fingerprint,
//! so a change to its metadata -- mode, owner, mtime -- passes a CAS check unnoticed. The next
//! section says why that is the right trade for v0.1 and what it costs.
//!
//! **An absence and an empty file have one digest.** The same clause has a second consequence, which
//! hand 5 met when `apply` had to report what it observed after a removal: "digest = content only" leaves
//! this adapter nothing with which to distinguish "no file here" from "a file with no bytes". A (sem: SEM-gx-adapter-fs-075)
//! removal therefore reports the digest of no content, and a CAS check cannot tell the two states
//! apart. Inventing a marker would not help -- any byte string a marker used is also a file's
//! possible content -- so the fix belongs with a wider `Fingerprint` (v0.2), and `req/74` §2 raises
//! it rather than this paragraph hiding it.
//!
//! **A creation has no `pre` this adapter can produce.** `snapshot` reads, so a position that holds
//! nothing answers [`gx_substrate::Error::Unreadable`] and there is no `ObjectSnapshot` describing
//! "empty". `apply` and `invert` both handle the state; nothing in v0.1 can *name* it. AC-049's (sem: SEM-gx-adapter-fs-076)
//! creation case builds the snapshot in the test, and an engine planning a creation (M5) meets the
//! same wall -- also `req/74` §2.
//!
//! **No confinement.** N-05 above, in full.
//!
//! # Scope and digest (**ASM-69-1**, normative)
//!
//! req/38 §28 M4-16:
//!
//! > "fs's `scope` = one normalised absolute path (v0.1); `digest` = **content only** (metadata excluded). The reason for the shape:
//! > a rename-atomic apply always moves the mtime, so a digest that includes metadata is **structurally
//! > contradictory** with L2 (reentrancy safety). The narrow scope's remaining TOCTOU (on the metadata face) is stated explicitly in the doc" (sem: SEM-gx-adapter-fs-077)
//!
//! So a `Fingerprint` from this adapter is `{ Fs, the normalised absolute path, BLAKE3 of the
//! file's bytes }`. 42 §3.5 recommends widening a scope to "the surrounding state that could interfere with the target" and this is the (sem: SEM-gx-adapter-fs-078)
//! narrow end of that range, on purpose: hand 5's `apply` gets its atomicity from `rename`, a rename
//! always writes a new mtime, and a digest covering metadata would therefore make the second `apply`
//! of one delta observe a state the first never saw. That is L2, and no adapter can hold both.
//!
//! A scope past [`gx_core::MAX_SCOPE_BYTES`] becomes a digest line before it reaches the type
//! ([`gx_substrate::elide_scope`], **M4H1-2**), so a deep path costs a receipt a fixed number of
//! bytes rather than an unbounded one.
//!
//! # Durability: the three steps, and what a tmpfs cannot show
//!
//! Implemented in [`apply`] as of hand 5. The design of the delta (**M4-13, adopted (a)**: "v0.1's fs delta
//! is a single whole-file replacement, atomicity is from `rename`") only pays off if the write around that rename is done in one (sem: SEM-gx-adapter-fs-079)
//! order, and hand 5 fetched the three primaries **in full** before writing it
//! (`Desktop/GitRepo/REFERENCES.md`, 2026-08-09; the entry also records that hand 4's confirmation of
//! the same three was a search snippet).
//!
//! **What the primary says**, verbatim -- LWN 457667 lists five steps and this crate does all five:
//!
//! > "The following steps are required to perform this type of update: create a new temp file (on the
//! > same file system!) / write data to the temp file / fsync() the temp file / rename the temp file
//! > to the appropriate name / fsync() the containing directory" (sem: SEM-gx-adapter-fs-080)
//!
//! Same directory because POSIX `rename` is defined within one filesystem -- across one it fails with
//! `EXDEV` -- and because a copy across a mount point would not be a rename. The atomicity itself is
//! the Open Group's, and its normative sentence names **who** it protects: "a directory entry named
//! new shall remain visible to other threads throughout the renaming operation and refer either to
//! the file referred to by new or old before the operation began" (the word "atomic" appears in the
//! RATIONALE). Rust's `std::fs::rename` documents the replacement -- "replacing the original file if
//! `to` already exists" -- and documents **no atomicity at all**, because the guarantee belongs to the (sem: SEM-gx-adapter-fs-081)
//! filesystem underneath.
//!
//! 🔴 **What is derivation and not citation.** The reason usually given for the last step -- that
//! omitting the directory `fsync` can lose the rename itself -- is **not in LWN 457667**; the article
//! gives the ordered list and never discusses omitting a step. It follows from what a directory entry
//! is: data, which a crash can lose when it reached no stable storage. `research_bus/glovrex_m4/
//! SURVIVORS.md` §A-2 and `req/73` both wrote it as though sourced, and this paragraph is the
//! correction. The three steps stand; the reason for the third is ours.
//!
//! **What the tests of this crate can and cannot say.** They run on a **tmpfs**, where the rename is
//! atomic through the VFS and `fsync` is effectively free. So they are evidence about **concurrent**
//! observation and **not** about crash durability, and nothing here may be read as the second: AC-049
//! is an atomicity criterion that happens to exercise a durable shape. Measuring the durability would
//! need a filesystem with a device under it and a power failure to inject, which is not in M4. The
//! repository itself lives on `/mnt/c` (9p over NTFS), where `MoveFileEx` does not give POSIX rename
//! semantics at all -- which is why `tests/support/mod.rs` proves the filesystem type from
//! `/proc/self/mountinfo` before writing a byte.
//!
//! # The clock, and the moment an engine writes (**M4-17** / **E-M4-31**)
//!
//! 41 §6: "randomness and time are injected at the engine boundary (for deterministic replay)". Nothing in this crate reads a clock, (sem: SEM-gx-adapter-fs-082)
//! and `tests/plan_purity.rs` asserts the absence rather than trusting it.
//!
//! §31 **E-M4-31** settled what hand 5's `apply` does with `AppliedDelta.applied_at`, since 41 §4
//! gives the method no parameter an engine could inject through: "`applied_at` is a convention **overwritten by
//! the engine at commit time**, spelled out in 43's addendum form. The adapter [writes a] `Timestamp(0)` placeholder". So the adapter (sem: SEM-gx-adapter-fs-083)
//! writes `Timestamp(0)` and the engine (M5) overwrites it; a `Timestamp(0)` in a record that reached
//! a journal is an engine that skipped the step, not a filesystem that has no clock.
//!
//! **E-M4-30** fixes the other order, and [`invert`] is built around it: the escrowed inverse is
//! constructed **before** `apply` (43 T-10b), because the inverse of an overwrite carries the old
//! content and after the apply there is none. The conformance harness runs the round trip that way
//! and prints `ORDER=T-10b`; `tests/ac_049.rs` prints it per case.
//!
//! # Two ceilings, and the one relation between them (**M4-21**, **M4H5-4, adopted (b)**) (sem: SEM-gx-adapter-fs-084)
//!
//! [`MAX_INVERSE_PAYLOAD_BYTES`] (hand 5) bounds what this adapter will **carry back** and answers
//! with `Ok(None)`, which **E-M3-4** escalates to a human. [`MAX_FORWARD_PAYLOAD_BYTES`] (this hand)
//! bounds what it will **accept** and answers by refusing to plan. §33 M4H5-4, adopted (b) kept them apart
//! because "will it be accepted" and "can it be carried" are different judgements; v0.1 gives them the same number, and the (sem: SEM-gx-adapter-fs-085)
//! relation `MAX_FORWARD_PAYLOAD_BYTES <= MAX_INVERSE_PAYLOAD_BYTES` is what that number buys --
//! every change this adapter accepts is one whose result it could also have escrowed back.
//!
//! The forward bound is enforced in [`plan`] and **not** in the grammar, so a payload written by hand
//! still applies. That is the same split [`delta::MAX_OPS`] has between a legal value and a legal
//! v0.1 payload, except that `MAX_OPS` is enforced at `decode`; the difference is a residue and is
//! raised in `req/75` §2 rather than closed by a hand whose ruling said "on the plan side". (sem: SEM-gx-adapter-fs-086)
//!
//! # Independence, and what it costs to ask (**M4-25**, AC-052, AC-053)
//!
//! [`commutation`] answers from the two payloads and reads nothing: under **M4-13, adopted (a)** an
//! operation's footprint is its position, so "are these two independent" is "do they name two
//! positions", after normalisation. The verdict is symmetric by construction and `commutation(a,a)` is
//! `Conflicts` (**M4-25, adopted (a)**, the fail-closed side -- 43 §8 acts on the answer, so a false (sem: SEM-gx-adapter-fs-087)
//! `Commutes` is the direction that lets two changes proceed together).
//!
//! 🔴 The **residual** is the change that is held back, which is a fact about an order (43 §8 keeps
//! `T2` waiting for `T1`), so it is **not** symmetric: `commutation(a,b)` names `b` and
//! `commutation(b,a)` names `a`. That satisfies the harness's L6 and not the literal `==` in the
//! trait's contract row, and the two readings are raised in `req/75` §2 rather than reconciled here.

#![forbid(unsafe_code)]

pub mod adapter;
pub mod apply;
pub mod commutation;
pub mod delta;
pub mod invert;
pub mod locator;
pub mod plan;

pub use adapter::FsAdapter;
pub use delta::{
    Blob, FsDelta, FsOp, MAX_FORWARD_PAYLOAD_BYTES, MAX_INVERSE_PAYLOAD_BYTES, MAX_OPS,
};
pub use locator::normalize;
