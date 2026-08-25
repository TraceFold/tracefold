// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! **M4-21**'s ceiling: the one declaration, the boundary either side of it, and the row that names
//! it.
//!
//! req/38 §28 M4-21, adopted (a): "the ceiling on the inverse delta payload is declared **in exactly one constant**; exceeding it is `invert`=`Ok(None)`
//! (**the 1st reason AC-048's `None` actually occurs**). The value is decided by hand 5, with the reasoning printed (this ruling fixes only the shape)", and
//! §30 M4H2-8 adoption added the probe that keeps the two ends tied together: "hand 5's DoD adds a '1:1 probe between the contract table's row and the constant
//! declaration's site' (once M4-21's constant lands, the contract becomes measurable)". (sem: SEM-gx-adapter-fs-225)
//!
//! # Why a boundary needs two cases and not one
//!
//! A ceiling asserted only from above is satisfied by an adapter that answers `Ok(None)` to
//! everything, and one asserted only from below by an adapter with no ceiling at all. The pair below
//! differs by **one byte** of the file whose content the inverse has to carry, which is the smallest
//! statement that the number in the source is the number in effect.

mod support;

use gx_adapter_fs::{FsAdapter, MAX_INVERSE_PAYLOAD_BYTES};
use gx_substrate::SubstrateAdapter;
use support::{planned, snapshot_of, Sandbox, GOAL};

/// The payload of an inverse over `size` bytes, in the encoding that is actually escrowed.
///
/// The bound is on the **payload** rather than on the content, because 42 §5 escrows the payload: a
/// bound on the content would leave the encoding's own overhead outside the number that was declared.
fn invert_over(size: usize) -> (Option<usize>, usize) {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let name = "sized";
    sandbox.write(name, &vec![b'z'; size]);
    let locator = sandbox.locator(name);

    let pre = snapshot_of(&adapter, &locator);
    let delta = planned(&adapter, &locator, GOAL);
    let answer = adapter
        .invert(&delta, &pre)
        .expect("invert answers")
        .into_inverse();
    (answer.map(|d| d.payload().len()), size)
}

/// Under the ceiling there is an inverse; over it there is `Ok(None)`.
#[test]
fn the_ceiling_is_the_number_the_source_declares() {
    // The payload carries the locator too, so the largest content whose inverse fits is a little
    // under the ceiling. What the two cases below fix is that a value near it is answered and a value
    // clearly over it is not -- the exact crossing point is a function of the path length, which is
    // the sandbox's and not this adapter's.
    let (small, small_size) = invert_over(1024);
    let (large, large_size) = invert_over(MAX_INVERSE_PAYLOAD_BYTES + 1);

    println!(
        "INVERT_CEILING={MAX_INVERSE_PAYLOAD_BYTES} UNDER={small_size}->{small:?} \
         OVER={large_size}->{large:?}"
    );
    let small = small.expect("a kilobyte fits in the escrow");
    assert!(
        small > small_size,
        "the inverse carries the old content and its locator, so its payload is larger than the \
         content it restores"
    );
    assert!(
        large.is_none(),
        "a content over the ceiling still produced an inverse, so the constant is decorative"
    );
}

/// An escrow of **exactly** the ceiling is carried, which is the byte `>` and `>=` disagree about.
///
/// 🔴 req/76 §2.2 listed `invert.rs:111` (`>` → `>=`) as a `cargo mutants` survivor, with the same
/// one-line reason as the forward bound: "nobody has built the case where payload is exactly 1,048,576". The (sem: SEM-gx-adapter-fs-226)
/// pair above differs by one byte of *content* and lands on neither side of the bound exactly -- the
/// note in it says so in as many words ("the exact crossing point is a function of the path length").
/// This probe closes that by solving for the content whose **payload** is the number, so "at most N"
/// and "fewer than N" stop being the same claim. (sem: SEM-gx-adapter-fs-227)
///
/// M4-21's `Ok(None)` is an escalation to a human (**E-M3-4**), so the byte in question is the one
/// where a change stops being undoable without anybody being asked. Off by one in the safe direction
/// is still off by one: an operator interrupted about a file that fits is a gate that cried wolf.
#[test]
fn an_escrow_of_exactly_the_ceiling_is_carried() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let name = "onthebound";
    let locator = sandbox.locator(name);

    // The escrow carries the OLD content, so the size that has to land on the bound is the file's,
    // and the overhead is the locator plus the CBOR framing. Measured once in the same length class
    // (a byte string of 2^16 or more takes a five-byte header) and then solved, as `forward_ceiling`
    // does for the other bound.
    let escrow_of = |size: usize| -> Option<usize> {
        sandbox.write(name, &vec![b'z'; size]);
        let pre = snapshot_of(&adapter, &locator);
        let delta = planned(&adapter, &locator, GOAL);
        adapter
            .invert(&delta, &pre)
            .expect("invert answers")
            .into_inverse()
            .map(|d| d.payload().len())
    };

    let sample = MAX_INVERSE_PAYLOAD_BYTES - 4096;
    let overhead = escrow_of(sample).expect("a file well under the ceiling is invertible") - sample;
    let content = MAX_INVERSE_PAYLOAD_BYTES - overhead;

    let exact = escrow_of(content).expect(
        "an escrow of exactly the ceiling answered Ok(None): `invert` reads its bound as 'fewer
         than' where M4-21, adopted (a) declares 'at most', so a change that fits is escalated to a human (sem: SEM-gx-adapter-fs-228)",
    );
    println!(
        "INVERT_CEILING={MAX_INVERSE_PAYLOAD_BYTES} OVERHEAD={overhead} CONTENT={content} \
         PAYLOAD={exact} ON_THE_BOUND={}",
        exact == MAX_INVERSE_PAYLOAD_BYTES
    );
    assert_eq!(
        exact, MAX_INVERSE_PAYLOAD_BYTES,
        "the fixture did not land on the bound, so this probe is not about the boundary byte"
    );
    assert!(
        escrow_of(content + 1).is_none(),
        "the control: one byte over the ceiling is Ok(None), so the answer above is the bound \
         holding rather than the bound being absent"
    );
}

/// **M4H2-8**: the contract row in `gx-substrate` and the declaration in this crate are one thing.
///
/// The row is a promise a reader of the trait acts on; the constant is what runs. Neither one on its
/// own is a contract, and a row that named a constant nobody declared -- or two declarations that
/// could drift -- is the shape M4-21 wrote "one constant" against. (sem: SEM-gx-adapter-fs-229)
///
/// 🔴 **M7 hand 3 narrowed the scan, and the narrowing is req/99 §9 R-3 answered.** Until there was an
/// adapter whose inverse carries a body this walked every crate under `crates/` and asserted **one**
/// declaration workspace-wide. M7 hand 1 did the same narrowing for `MAX_FORWARD_PAYLOAD_BYTES` when
/// `gx-adapter-git` declared its own (req/99 §7), and left this one alone **because git declares none**
/// -- an inverse there carries an object id, so a ceiling could not be reached by any input and
/// declaring it would be "a refusal nobody asked for" (52 contract 2, req/99 §3 D-4). R-3 is the residue that
/// raised the question for hand 3: "if mcp declares an escrow ceiling, it needs the same re-reading". It does. (sem: SEM-gx-adapter-fs-230)
///
/// So the reading is "**at most one per adapter, none anywhere else**" rather than "one per adapter": (sem: SEM-gx-adapter-fs-231)
/// unlike the forward bound, which every adapter needs because every adapter accepts payloads, the
/// escrow bound is needed only by an adapter whose inverse **carries a body**, and that is not a fact
/// in the source. The two adapters that made the choice each say so where a reader will find it --
/// `gx-adapter-git`'s `this_adapter_declares_no_escrow_ceiling` and `gx-adapter-mcp`'s
/// `the_escrow_ceiling_is_reachable_here_and_answers_ok_none`.
///
/// The gate is not weakened. A crate below the boundary declaring one is red; an adapter declaring two
/// is red; and "nobody declares one" is red, which is the case that would make M4-21 a constant with no (sem: SEM-gx-adapter-fs-232)
/// reader.
#[test]
fn the_contract_row_and_the_one_declaration_name_each_other() {
    let root = repo_root();
    let mut per_crate: Vec<(String, Vec<String>)> = Vec::new();
    for crate_dir in std::fs::read_dir(root.join("crates")).expect("crates/ is readable") {
        let dir = crate_dir.expect("an entry").path();
        let src = dir.join("src");
        if !src.is_dir() {
            continue;
        }
        let name = dir
            .file_name()
            .expect("a named directory")
            .to_string_lossy()
            .into_owned();
        let mut found: Vec<String> = Vec::new();
        for file in walk(&src) {
            let text = std::fs::read_to_string(&file).expect("a source file is readable");
            for line in text.lines() {
                if line
                    .trim_start()
                    .starts_with("pub const MAX_INVERSE_PAYLOAD_BYTES")
                {
                    found.push(format!("{}: {}", file.display(), line.trim()));
                }
            }
        }
        per_crate.push((name, found));
    }
    per_crate.sort();

    let adapters: Vec<&(String, Vec<String>)> = per_crate
        .iter()
        .filter(|(name, _)| name.starts_with("gx-adapter-"))
        .collect();
    let declaring: usize = adapters
        .iter()
        .filter(|(_, found)| !found.is_empty())
        .count();
    println!(
        "MAX_INVERSE_PAYLOAD_BYTES_ADAPTERS={} DECLARING={declaring} PER_CRATE={:?}",
        adapters.len(),
        per_crate
            .iter()
            .map(|(name, found)| (name, found.len()))
            .collect::<Vec<_>>()
    );
    assert!(
        !adapters.is_empty(),
        "the scan found no adapter crate, so it is measuring nothing (§30's disease)"
    );
    assert!(
        declaring >= 1,
        "no adapter declares an escrow ceiling, so M4-21 is a constant with no reader"
    );
    for (name, found) in &per_crate {
        let limit = usize::from(name.starts_with("gx-adapter-"));
        assert!(
            found.len() <= limit,
            "M4-21 fixes the ceiling at 'one constant' per adapter and no crate below the boundary declares one; `{name}` has {found:?} (sem: SEM-gx-adapter-fs-233)"
        );
    }

    // The row that promises it. The cell is `invert`'s, not the table's -- §30 M4H2-6's rule about
    // "somewhere in the file" -- and `adapter_contract.rs` is where the clause "over the ceiling is `Ok(None)`" (sem: SEM-gx-adapter-fs-234)
    // itself is held to that cell.
    let trait_doc = std::fs::read_to_string(root.join("crates/gx-substrate/src/adapter.rs"))
        .expect("the trait is readable");
    let row = trait_doc
        .lines()
        .find(|l| l.contains("| `invert` |"))
        .expect("the contract table has an `invert` row");
    assert!(
        row.contains("MAX_INVERSE_PAYLOAD_BYTES"),
        "the `invert` contract row does not name the constant that decides its `Ok(None)`: {row}"
    );
}

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits at <root>/crates/gx-adapter-fs")
        .to_path_buf()
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).expect("a directory is readable") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

/// A `pre` that names another position is a **wiring mistake**, and is refused as one (**E-M4-32**).
///
/// 🔴 This probe is hand 5's, reversed. Hand 5 answered `Ok(None)` here and raised the reading against
/// itself (req/74 §2 M4H5-1); §33 took case (b) and made it **E-M4-32**:
///
/// > "**`invert` for a `pre` that names another position is `Err`**. Reason: passing a pre unrelated to the delta is a wiring
/// > bug in the engine, and `Ok(None)`->Escalate would disguise the bug as a legitimate business condition of 'the inverse cannot be
/// > constructed' (the same fallacy as E-M4-27 refusing `Ok(false)`). **`Ok(None)` is limited to 'a legitimate construction of the same object
/// > is not possible' (over the ceiling, or the old content already discarded)** (sem: SEM-gx-adapter-fs-235)
///
/// So the two answers now mean two things that cannot be confused: `Ok(None)` is a real business
/// condition an operator is asked about (E-M3-4's escalation), and `Err(LocatorMismatch)` is a defect
/// in whoever assembled the call. The pin is rewritten rather than deleted -- the same operation §29
/// M4H1-9 and §21 C-9 acknowledged for the door pin and the `cas_eq` pin, with the fixture unchanged and the (sem: SEM-gx-adapter-fs-236)
/// name and documentation moved to the new intent.
#[test]
fn a_pre_that_names_another_position_is_a_locator_mismatch() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let subject = sandbox.locator(support::SUBJECT);
    let beside = sandbox.locator(support::OTHER);

    let delta = planned(&adapter, &subject, GOAL);
    let elsewhere = snapshot_of(&adapter, &beside);

    let refusal = adapter
        .invert(&delta, &elsewhere)
        .expect_err("E-M4-32: a `pre` naming another object is a mis-wired call, not a state");
    println!(
        "INVERT_FOREIGN_PRE KIND={} MESSAGE={refusal}",
        refusal.kind()
    );
    assert_eq!(refusal.kind(), "LocatorMismatch");
    assert!(
        refusal.to_string().contains(&subject) && refusal.to_string().contains(&beside),
        "the refusal names neither the position the delta is about nor the one it was handed: \
         {refusal}"
    );
    assert!(
        adapter
            .invert(&delta, &snapshot_of(&adapter, &subject))
            .expect("the question is answerable")
            .inverse()
            .is_some(),
        "the control: the same delta with the matching pre has an inverse"
    );
}
