// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Lexical locator normalisation for git: the `≈` of 42 §2.3, as a function (**E-M4-12**).
//!
//! The five clauses are in the crate root, where 42 §2.3 requires them and where an implementor
//! reads them; this module is their implementation and nothing else. It performs **no I/O**, which is
//! what keeps a scope computation from depending on a read taken at a different moment than the CAS
//! check it feeds.
//!
//! # Why a git locator has three parts and an fs locator has one
//!
//! A path names a position on a filesystem because a filesystem has one namespace. A git repository
//! has three that a change has to cross: which repository, which reference, which entry in the tree
//! that reference reaches. Collapsing any two of them loses a distinction gx needs -- the scope is the
//! first two (the crate root says why), and the object is all three.

use gx_substrate::{Error, Result};

/// The separator between the repository and the reference.
///
/// `#` because git's own revision syntax already spends `:` on the tree path (`HEAD:README.md`) and
/// because a `#` cannot appear in a POSIX path without quoting -- so a locator's parts can be found
/// by splitting rather than by parsing.
pub const REPO_SEPARATOR: char = '#';

/// The separator between the reference and the path inside its tree, which is git's own.
pub const PATH_SEPARATOR: char = ':';

/// The prefix a fully-spelled reference carries.
pub const REF_PREFIX: &str = "refs/";

/// What an unqualified reference name means to this adapter: a branch (crate root, clause 2).
pub const BRANCH_PREFIX: &str = "refs/heads/";

/// A locator, split into the three things it names.
///
/// Constructed only by [`parse`], so a value of this type is always **normalised**: there is no
/// spelling of a `Position` that [`normalize`] would change. That is what lets `apply`, `invert` and
/// `commutation` compare positions by comparing strings and lets the crate root's `≈` be a fact about
/// the type rather than a discipline about call sites.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    repository: String,
    reference: String,
    path: String,
}

impl Position {
    /// The repository, as a normalised absolute path.
    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }

    /// The reference, fully spelled (`refs/heads/main`).
    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }

    /// The entry inside the reference's tree.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// 42 §3.5's scope: the **branch**, which is what a change to any file on it moves.
    ///
    /// The crate root argues why this is not the object. It is a `String` rather than a borrow
    /// because it is a value neither field holds: the scope is the pair.
    #[must_use]
    pub fn scope(&self) -> String {
        format!("{}{REPO_SEPARATOR}{}", self.repository, self.reference)
    }

    /// The whole locator, in the one spelling this adapter writes.
    #[must_use]
    pub fn locator(&self) -> String {
        format!(
            "{}{REPO_SEPARATOR}{}{PATH_SEPARATOR}{}",
            self.repository, self.reference, self.path
        )
    }
}

/// The representative spelling of a locator (crate root, clauses 1-5).
///
/// Total: every string has a normal form, including one this adapter would refuse to act on. That is
/// deliberate and it is L7's shape -- the property is "normalising twice changes nothing" over *every* (sem: SEM-gx-adapter-git-053)
/// string, and the refusal of a locator that is not a position happens where the locator is **used**
/// ([`parse`]), not where it is spelled. A normaliser that refused would make L7 a property with
/// exceptions.
#[must_use]
pub fn normalize(locator: &str) -> String {
    let (repository, rest) = match locator.split_once(REPO_SEPARATOR) {
        Some(split) => split,
        // No `#` at all: fold the whole thing as a repository path, so that the normal form of a
        // spelling this adapter cannot act on is still stable under a second normalisation.
        None => return fold_repository(locator),
    };
    let (reference, path) = match rest.split_once(PATH_SEPARATOR) {
        Some((reference, path)) => (reference, path),
        None => (rest, ""),
    };
    format!(
        "{}{REPO_SEPARATOR}{}{PATH_SEPARATOR}{}",
        fold_repository(repository),
        full_reference(reference),
        fold_path(path, false)
    )
}

/// Read a locator as the three things it names, or say why it is not a position.
///
/// # Errors
/// [`Error::NotAPosition`] when the spelling names fewer than three parts, when the repository is not
/// absolute (ASM-69-3), when the reference or the path is empty, or when the path has more than
/// [`crate::delta::MAX_PATH_DEPTH`] components. All five are "the argument is not a position" and not "application failed
/// " (**M4H5-5, adopted (b)**): 43 T-11 turns `ApplyFailed` into `AbortReason::ApplyFailed`, which would (sem: SEM-gx-adapter-git-054)
/// record a change that failed where no change was ever describable.
pub fn parse(locator: &str) -> Result<Position> {
    let normalised = normalize(locator);
    let refuse = |why: &str| Error::NotAPosition {
        locator: format!("{locator} ({why})"),
        normalised: normalised.clone(),
    };

    let (repository, rest) = normalised.split_once(REPO_SEPARATOR).ok_or_else(|| {
        refuse("a git locator names a repository and a reference, separated by `#`")
    })?;
    let (reference, path) = rest.split_once(PATH_SEPARATOR).ok_or_else(|| {
        refuse("a git locator names a path inside the reference's tree, after a `:`")
    })?;

    if !repository.starts_with('/') {
        return Err(refuse(
            "the repository is named from the root; v0.1 names positions absolutely (ASM-69-3)",
        ));
    }
    if reference == REF_PREFIX || reference.is_empty() {
        return Err(refuse("the reference is empty"));
    }
    if path.is_empty() {
        return Err(refuse("the path inside the tree is empty"));
    }
    if path.split('/').count() > crate::delta::MAX_PATH_DEPTH {
        return Err(refuse(
            "v0.1 changes one entry of the top-level tree; a nested path rebuilds every tree \
             between the file and the root, which is n object writes for one change \
             (MAX_PATH_DEPTH)",
        ));
    }

    Ok(Position {
        repository: repository.to_string(),
        reference: reference.to_string(),
        path: path.to_string(),
    })
}

/// Clause 2: an unqualified name is a branch, and a `refs/` name is already spelled.
///
/// Purely textual. git resolves an unqualified name against several namespaces in order and would
/// consult the repository to do it; a normalisation that did the same would be a filesystem read
/// inside every scope computation, and two reads a moment apart could disagree about one locator.
fn full_reference(reference: &str) -> String {
    let folded = fold_path(reference, false);
    if folded.is_empty() {
        return String::new();
    }
    if folded.starts_with(REF_PREFIX) {
        return folded;
    }
    // `heads/main` and `tags/v1` name the namespace already; anything else is a branch.
    if folded.starts_with("heads/") || folded.starts_with("tags/") || folded.starts_with("remotes/")
    {
        return format!("{REF_PREFIX}{folded}");
    }
    format!("{BRANCH_PREFIX}{folded}")
}

/// Clauses 1, 3, 4 and 5: fold `.` and `..`, collapse separators, drop a trailing one.
///
/// `rooted` says whether a leading separator survives. 🔴 It is a fact about the **input** for the
/// repository part and a constant `false` for a reference and a tree path, neither of which has a
/// root — and the distinction is load-bearing rather than tidy. The first spelling of this function
/// took `absolute: bool` as a statement about the *role* and prefixed `/` unconditionally for the
/// repository, which quietly turned `relative/repo` into `/relative/repo`: ASM-69-3's refusal of a
/// relative position could then never fire, because there was no relative position left to refuse by
/// the time [`parse`] looked. `tests/git_delta.rs` caught it on the first run and it is the reason
/// this parameter is named for the input rather than for the caller.
///
/// A `..` that cannot cancel is dropped when `rooted` (there is nothing above `/`) and kept when not,
/// where dropping it would invent a position the caller did not write.
fn fold_path(text: &str, rooted: bool) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for segment in text.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if matches!(segments.last(), Some(&"..")) || segments.is_empty() {
                    if !rooted {
                        segments.push("..");
                    }
                } else {
                    segments.pop();
                }
            }
            other => segments.push(other),
        }
    }
    let joined = segments.join("/");
    if rooted {
        format!("/{joined}")
    } else {
        joined
    }
}

/// The repository part: folded, and absolute exactly when it was written absolute.
fn fold_repository(text: &str) -> String {
    fold_path(text, text.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L7's first half, over the clauses this module implements.
    #[test]
    fn normalisation_is_idempotent_over_every_clause() {
        for spelling in [
            "/srv/repo#refs/heads/main:README.md",
            "/srv/./repo#main:README.md",
            "/srv/other/../repo#heads/main:/README.md",
            "/srv//repo#refs//heads/main:README.md/",
            "/srv/repo/#main:./README.md",
            "/../srv/repo#main:README.md",
            "not a locator at all",
            "",
        ] {
            let once = normalize(spelling);
            assert_eq!(once, normalize(&once), "{spelling:?}");
        }
    }

    /// Clause 2, which is the one no other adapter has.
    #[test]
    fn an_unqualified_reference_is_a_branch() {
        assert_eq!(full_reference("main"), "refs/heads/main");
        assert_eq!(full_reference("heads/main"), "refs/heads/main");
        assert_eq!(full_reference("refs/heads/main"), "refs/heads/main");
        assert_eq!(full_reference("tags/v1"), "refs/tags/v1");
        assert_eq!(full_reference("refs/tags/v1"), "refs/tags/v1");
    }

    /// The scope is the pair, and it drops the path (the crate root says why).
    #[test]
    fn the_scope_is_the_branch_and_not_the_file() {
        let one = parse("/srv/repo#main:a.txt").expect("a position");
        let two = parse("/srv/repo#main:b.txt").expect("a position");
        assert_eq!(one.scope(), "/srv/repo#refs/heads/main");
        assert_eq!(one.scope(), two.scope());
        assert_ne!(one.locator(), two.locator());
    }
}
