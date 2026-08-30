# gx-adapter-git

The git SubstrateAdapter: a file on a branch, moved by a commit, undone by a reference reset (41 §4, FR-045).

## What this crate guarantees

Quoted from `src/lib.rs`'s crate-level doc comment:

> An **object** is what a change is about; a **scope** is "the surrounding state that could
> interfere with the target" ... | **object** | the file at one path, on one branch, in one
> repository ... | **scope** | the **branch**. Its digest is the digest of the commit the branch
> points at. |

> What this costs is stated rather than hidden: **two changes to two different files on one
> branch conflict**, and one of them waits. That is the true shape of a branch and not a
> limitation of this version.

> locator := `<repository path> "#" <ref> ":" <path>` ... All three parts are required; a
> spelling missing either separator is not a position (`gx_substrate::Error::NotAPosition`)
> rather than a position that fails to apply.

> Two locators are `≈` for this adapter — **they name one position** — exactly when
> `locator::normalize` maps them to the same string ... it is **purely lexical**: a function of
> the text, performing no repository read at all.

## What this crate does not guarantee

> **Observed, never copied.** gitoxide's implementation is not read and not reproduced here:
> what this crate uses is the public API ... and the delta grammar, the locator normalisation
> and the quantifiers of the seven contracts are gx's own.

## Position

`req/spec/40-architecture/41-architecture.md` §2 (crate, dependency line), §4 (the seven
methods); `42-*.md` §3.4 (delta), §3.5 (fingerprint). Obligation FR-045 (`32-functional.md`),
measured by AC-050 (`34-*.md`). M7 requirement definition `req/98_M7_REQDEF_2026-08-11.md`
§7-2 hand 1, ratified `req/38` §57.

## Not covered

Two changes to two different files on one branch are treated as conflicting, not as
independent — this is documented as the branch's actual shape, not a gap to close.
