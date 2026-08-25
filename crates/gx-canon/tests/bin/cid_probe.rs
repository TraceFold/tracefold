// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The second process of AC-011.
//!
//! AC-011 asks for the CID to be computed in "a completely independent child process B (a
//! separate binary, no shared cache, a separate working directory)" (sem: SEM-gx-canon-074) and compared with the one the test process computed. A separate
//! binary is therefore part of the acceptance criterion rather than a convenience, and this is
//! it: read a `Transformation` as JSON on standard input, compute its CID with the same library
//! function, print the result in the `gx1:` form of 42 §1.2.
//!
//! It lives under `tests/` and is pointed at by an explicit `[[bin]]` path, so `src/` still holds
//! exactly the four modules 41 §2 lists (`lib`, `cbor`, `jcs`, `cid`). The I/O here is the
//! process boundary the criterion demands and no part of the library: `cargo tree -p gx-canon -e
//! normal` is unchanged by it, and 41 §6's ban on I/O is a statement about the crate's API.
//!
//! The architecture is printed as well, because the honest form of AC-011's x86_64/aarch64 clause
//! is a measurement that names what it ran on rather than a claim about both (A-5, and req/05 §4
//! R-5's rule against turning one measurement into a settled value).

// This is a crate root of its own, so gx-canon's `lib.rs` attribute does not reach it. Without
// this line the package ships one binary that may contain `unsafe` while its library may not --
// which `cargo geiger` reported as `?` on its first run in M2 hand 6, and which
// `tests/unsafe_forbidden.rs` now fails the build over.
#![forbid(unsafe_code)]

use gx_canon::cid;
use gx_core::Transformation;
use std::io::Read;

fn main() {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("cannot read stdin: {e}");
        std::process::exit(2);
    }

    let value: Transformation = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("stdin is not a Transformation in JSON: {e}");
            std::process::exit(3);
        }
    };

    match cid::compute(&value) {
        Ok(c) => {
            println!("CID={}", cid::to_text(&c));
            println!("ARCH={}", std::env::consts::ARCH);
        }
        Err(e) => {
            eprintln!("cid::compute refused the value: {e}");
            std::process::exit(4);
        }
    }
}
