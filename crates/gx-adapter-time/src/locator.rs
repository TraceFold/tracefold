// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Where an entry sits, and the one spelling of it this adapter names it by.
//!
//! A position is an **absolute path**, as in `gx-adapter-fs` and for the same reason (ASM-69-3):
//! v0.1 names positions from the root, so that the object a gate admitted and the object an apply
//! writes cannot be resolved against two different working directories.
//!
//! # Lexical only, and why `..` is left standing
//!
//! Normalisation collapses repeated separators, drops `.` segments, and drops a trailing separator.
//! It does **not** resolve `..`, and that is a decision rather than an omission: `/a/b/..` is `/a`
//! only when `b` is a directory and not a symbolic link, so resolving it lexically would let one
//! spelling name a position the filesystem would resolve elsewhere. A normaliser that is wrong in
//! that direction is worse than one that leaves a spelling alone, because the wrongness reaches a
//! fingerprint's scope.
//!
//! L7 asks for idempotence: every rule here is a fixed point after one pass, and the harness
//! measures it through `Fixture::normalise`.

/// The one spelling of a position.
#[must_use]
pub fn normalize(locator: &str) -> String {
    let absolute = locator.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for segment in locator.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        out.push(segment);
    }
    let body = out.join("/");
    if absolute {
        format!("/{body}")
    } else {
        body
    }
}

/// Whether a normalised locator names a position from the root.
#[must_use]
pub fn is_absolute(locator: &str) -> bool {
    locator.starts_with('/') && locator.len() > 1
}
