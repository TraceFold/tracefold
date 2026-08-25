// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Lexical locator normalisation: the `≈` of 42 §2.3, as a function (**E-M4-12**).
//!
//! The five clauses are in the crate root, where 42 §2.3 requires them and where an implementor
//! reads them; this module is their implementation and nothing else. It performs **no I/O**, which
//! is clause 1 and is also what keeps [`crate::plan`] a pure function -- a normaliser that resolved
//! symbolic links would put a filesystem read inside `plan` and inside every scope computation.
//!
//! # Why the boundary crate has no normaliser
//!
//! `gx-substrate` states the contract (its `# Locator normalisation (normative)` section) and
//! implements none of it, on purpose: a shared path grammar living above the adapters would be the
//! road M3-10 refused for the gate, and `git` refs and MCP tool names are not paths at all.

/// The one separator this adapter knows.
///
/// A byte, and deliberately not a platform question: gx's fs adapter speaks POSIX paths, and a
/// Windows port would be a different `SubstrateKind` rather than a second meaning for this one.
pub const SEPARATOR: char = '/';

/// The representative spelling of a position (**E-M4-12**, crate root clauses 1-3).
///
/// `l ≈ l'` exactly when `normalize(l) == normalize(l')`, which makes this function the definition
/// of the equivalence rather than a convenience over it. It is idempotent -- L7's first half -- for
/// the structural reason that the output has no `.`, no `..` it could cancel, no repeated separator
/// and no trailing separator, so a second pass has nothing to do.
///
/// Absolute and relative inputs differ in one clause: a leading `..` that cannot cancel is **kept**
/// in a relative locator and **dropped** at the root. Both are in the crate root, and the second is
/// the one that closes a fail-open path -- M3-10 fixed a v0.1 policy pack's effective range at
/// the "locator level", so `/etc/../../etc/passwd` has to arrive at the gate as `/etc/passwd` or a (sem: SEM-gx-adapter-fs-088)
/// `/etc/**` forbid is a contest of spellings.
#[must_use]
pub fn normalize(locator: &str) -> String {
    let absolute = is_absolute(locator);
    let mut segments: Vec<&str> = Vec::new();

    for segment in locator.split(SEPARATOR) {
        match segment {
            // Clause 2 (`.`) and clause 3 (`//` leaves an empty segment): both carry nothing.
            "" | "." => {}
            ".." => match segments.last() {
                // Cancel the segment before it.
                Some(&previous) if previous != ".." => {
                    segments.pop();
                }
                // Nothing to cancel. At the root there is nowhere above to name, so it is dropped;
                // in a relative locator it is a position the caller wrote, so it is kept.
                _ => {
                    if !absolute {
                        segments.push("..");
                    }
                }
            },
            other => segments.push(other),
        }
    }

    let body = segments.join("/");
    if absolute {
        format!("/{body}")
    } else if body.is_empty() {
        // A relative locator that cancelled itself out is "here", and `.` is its one spelling. (sem: SEM-gx-adapter-fs-089)
        ".".to_string()
    } else {
        body
    }
}

/// Whether a locator names a position from the root (**ASM-69-3**).
///
/// The methods that need a position consult this; [`normalize`] does not, because normalising a
/// relative locator is well defined and refusing it is a decision about *use*. Keeping the two apart
/// is what lets L7 be a property over every string rather than over the ones this adapter accepts.
#[must_use]
pub fn is_absolute(locator: &str) -> bool {
    locator.starts_with(SEPARATOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The root is a position and the empty string is not.
    #[test]
    fn the_root_normalises_to_itself() {
        assert_eq!(normalize("/"), "/");
        assert_eq!(normalize("//"), "/");
        assert_eq!(normalize("/././"), "/");
    }

    /// A relative locator that cancels itself out is "here". (sem: SEM-gx-adapter-fs-090)
    #[test]
    fn a_relative_locator_that_cancels_out_is_here() {
        assert_eq!(normalize("a/.."), ".");
        assert_eq!(normalize(""), ".");
        assert!(!is_absolute(&normalize("a/..")));
    }
}
