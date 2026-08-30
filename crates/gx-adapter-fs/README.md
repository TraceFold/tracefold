# gx-adapter-fs

The filesystem SubstrateAdapter: single-file whole replacement, lexical locators (41 §4, M4-13(a)).

## What this crate guarantees

Quoted from `src/lib.rs`'s crate-level doc comment:

> All seven: `FsAdapter::kind`, `snapshot`, `plan`, `precondition` (hand 4), `apply` / `invert`
> (hand 5) and `commutation` (this hand, req/69 §6.2). No method of this adapter answers
> `gx_substrate::Error::Unimplemented` any more ... `is_conformant` and `is_complete` are both
> true, and `meets_51_7` with them.

> A locator is an absolute path and any absolute path this process can write is a path this
> adapter will write. `apply` creates, replaces and removes whole files, and it does not consult
> an allow-list, a root, or a chroot, because it has none of the three.

> The bound that does exist is the process's own credentials and the delta's grammar: one
> operation (`delta::MAX_OPS`), one whole file, no directory creation, no mode or owner change,
> no symbolic link followed by the adapter's own choice (the kernel still follows one when
> opening the path).

## What this crate does not guarantee

> 51 §7's completion condition is about **the seven contracts against a fixture**, and this
> adapter's fixture runs on a tmpfs, in one thread, over files it created ... It is not a claim
> about filesystems in general, about crash durability ..., about concurrency, or about the git
> and mcp adapters.

> **N-05** ... keeps Landlock out of v0.1 ... so this adapter has no confinement of its own.

> What stands between an intent and a file is therefore the **gate** and nothing in this crate.

## Position

`req/spec/40-architecture/41-architecture.md` §2 (crate), §4 (the seven `SubstrateAdapter`
methods this crate implements); `42-*.md` §3.4 (delta), §3.5 (fingerprint). Rulings M4-12,
M4-13, M4-16/ASM-69-1, M4-17, E-M4-31, E-M4-29 are recorded in
`req/38_ERRATA_2026-08-07.md` §28-§31.

## Not covered

TH-2 residue (a symlink followed by the kernel at open time, not by this adapter's own choice)
is not closed by this crate — see `gx-substrate`'s locator-normalisation contract for where that
residue is named.
