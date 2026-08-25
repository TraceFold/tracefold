// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-011 — the CID of a value is the same wherever it is computed.
//!
//! 34 AC-011 verbatim: "(a) as process A, call `gx_canon::cid::compute(&x)` directly as a
//! library function and obtain the CID; (b) in a completely independent child process B (a
//! separate binary, no shared cache, a separate working directory), call the same function on
//! the same input and obtain the CID. Then: `Cid_A == Cid_B` (the BLAKE3 32-byte values are
//! bit-equal)" (sem: SEM-gx-canon-028), and 42 §1.1 for what the digest is: BLAKE3-256 over the canonical
//! DAG-CBOR form of the value.
//!
//! # The two architectures
//!
//! The criterion also asks for "a CI matrix covering both x86_64/aarch64" (sem: SEM-gx-canon-029). There is no aarch64 execution
//! environment in this estate (req/10 §6 B-5), so A-5 conditions that half of the clause and this
//! file reports what it actually ran on. The report is computed from `std::env::consts::ARCH`
//! rather than written down, so it stays true if the same test is one day run on an arm runner:
//! the architecture that ran becomes the measured one and the other becomes the unmeasured one,
//! without anybody editing a string.
//!
//! # What determinism means here, and what it does not
//!
//! A hash is deterministic in a boring way -- BLAKE3 is a function. What this file is really
//! checking is the *input* to that function: that the bytes fed to BLAKE3 depend on nothing but
//! the value. Two processes with different working directories, no shared environment and no
//! shared cache is how that gets tested rather than assumed. `51 §3` calls the property
//! `transformationid_determinism`.

mod support;

use gx_canon::cid::{self, IdentityView};
use gx_canon::{cbor, Error};
use gx_core::{Cid, Timestamp};
use proptest::prelude::*;
use std::io::Write;
use std::process::{Command, Stdio};
use support::{any_transformation, any_transformation_id, sample_transformation};

/// 42 §1.1: BLAKE3-256, so thirty-two bytes and no self-describing prefix.
#[test]
fn ac_011_a_cid_is_thirty_two_bytes_of_blake3() {
    let value = sample_transformation();
    let computed = cid::compute(&value).expect("a valid value has a CID");

    let expected_input = cbor::encode(&value.identity_view()).expect("the projection encodes");
    let expected = blake3::hash(&expected_input);
    assert_eq!(
        computed,
        Cid(*expected.as_bytes()),
        "the CID must be BLAKE3 over the canonical form of the IdentityView (42 §1.1, §1.3)"
    );
    assert_eq!(computed.0.len(), 32);
}

/// AC-014 in miniature, stated where it can be stated: the digest is taken over bytes the wire
/// face produced. A CID that could be reached without the projection would be a second road to
/// an identity, and 42 §1.3's exclusions would hold only by convention.
#[test]
fn ac_011_the_hashed_bytes_are_the_canonical_form_of_the_projection() {
    let value = sample_transformation();
    let bytes = cbor::encode(&value.identity_view()).expect("the projection encodes");
    assert!(cbor::is_canonical(&bytes));
    assert_ne!(
        bytes,
        cbor::encode(&value).expect("the whole value encodes too"),
        "the projection must not be the whole value: `id` and `created_at` are dropped"
    );
}

/// The two-process comparison of AC-011 (T-32).
///
/// Independence is arranged three ways, one per clause of the criterion: a different binary
/// (`gx_cid_probe`, a separate compilation unit with its own `main`), a different working
/// directory (a fresh directory under the system temp), and no inherited environment at all
/// (`env_clear`, which is the strongest available reading of "no shared cache" (sem: SEM-gx-canon-030) -- there is no
/// cache in `compute` to share, so what is removed is every variable that could make one process
/// behave differently from the other).
///
/// The value crosses the boundary as JSON because a process boundary only carries bytes. That
/// makes the child's route the JSON one of AC-013 as well, which is the stronger test: the two
/// processes agree even though they did not reach the value the same way.
#[test]
fn ac_011_two_independent_processes_agree_on_the_cid() {
    let value = sample_transformation();
    let parent = cid::compute(&value).expect("parent computes");
    let parent_text = cid::to_text(&parent);

    let json = serde_json::to_string(&value).expect("the value has a JSON form");

    let workdir = std::env::temp_dir().join(format!(
        "gx-canon-ac011-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&workdir).expect("a working directory of its own");

    let mut child = Command::new(env!("CARGO_BIN_EXE_gx_cid_probe"))
        .current_dir(&workdir)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the probe binary is built before the integration tests");

    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(json.as_bytes())
        .expect("the child accepts its input");

    let out = child.wait_with_output().expect("the child terminates");
    let stdout = String::from_utf8(out.stdout).expect("the child prints UTF-8");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "the child failed ({:?}): {stderr}",
        out.status.code()
    );

    let field = |name: &str| -> String {
        stdout
            .lines()
            .find_map(|l| l.strip_prefix(name))
            .unwrap_or_else(|| panic!("the child printed no {name} line: {stdout}"))
            .trim()
            .to_string()
    };
    let child_text = field("CID=");
    let child_arch = field("ARCH=");

    let child_cid = cid::from_text(&child_text).expect("the child printed a gx1 CID");
    assert_eq!(
        child_cid, parent,
        "two processes disagreed about the identity of one value"
    );
    assert_eq!(child_text, parent_text);

    let parent_arch = std::env::consts::ARCH;
    let unmeasured: Vec<&str> = ["x86_64", "aarch64"]
        .into_iter()
        .filter(|a| *a != parent_arch && *a != child_arch)
        .collect();

    // Machine output, not prose. req/31 §9's DoD for this step asks for "aarch64=not measured" (sem: SEM-gx-canon-031) to come
    // out of the machine, and the honest way to produce it is to name the architecture that ran.
    println!("CID_A={parent_text}");
    println!("CID_B={child_text}");
    println!("CID_MATCH=true");
    println!("ARCH_PARENT={parent_arch} ARCH_CHILD={child_arch}");
    println!("ARCH_MEASURED={parent_arch}");
    println!("ARCH_UNMEASURED={}", unmeasured.join(","));

    assert_eq!(
        parent_arch, child_arch,
        "the two processes ran on different architectures, which is a different experiment"
    );

    std::fs::remove_dir_all(&workdir).expect("the working directory is removed");
}

/// A refusal rather than a panic. 41 §6 counts a panic as a bug, and `compute` is on the path
/// every identity is minted through, so its failure mode is a returned error.
#[test]
fn ac_011_a_value_with_no_canonical_form_is_refused_not_hashed() {
    // Every M1 structure has a canonical form, so the refusal is shown on the type that does
    // not: a float, which 42 §2.1-4 keeps out of canonical values.
    let refused = cbor::encode(&1.5f64);
    assert!(
        matches!(refused, Err(Error::NotCanonicalizable(_))),
        "the encoder gates what `compute` may hash: {refused:?}"
    );
}

proptest! {
    /// `transformationid_determinism` (51 §3): the same value has the same identity, every time.
    #[test]
    fn ac_011_transformationid_determinism(value in any_transformation()) {
        let first = cid::compute(&value).expect("valid value");
        let second = cid::compute(&value.clone()).expect("valid value");
        prop_assert_eq!(first, second);
    }

    /// 42 §1.3-1 and §1.3-2, stated at the level the acceptance criterion cares about: the two
    /// excluded fields cannot move a CID. Recording the same change at another time, or filing
    /// it under another id, must not mint a second identity.
    #[test]
    fn ac_011_id_and_created_at_do_not_move_the_cid(
        base in any_transformation(),
        other_id in any_transformation_id(),
        other_time in any::<i64>(),
    ) {
        let mut moved = base.clone();
        moved.id = other_id;
        moved.created_at = Timestamp(other_time);
        prop_assert_eq!(cid::compute(&moved).unwrap(), cid::compute(&base).unwrap());
    }

    /// Non-vacuity. Without this, a `compute` that returned a constant would satisfy everything
    /// above.
    #[test]
    fn ac_011_a_different_projected_field_gives_a_different_cid(
        base in any_transformation(),
        other_intent in support::any_intent_id(),
    ) {
        prop_assume!(other_intent != base.intent_id);
        let mut other = base.clone();
        other.intent_id = other_intent;
        prop_assert_ne!(cid::compute(&other).unwrap(), cid::compute(&base).unwrap());
    }
}
