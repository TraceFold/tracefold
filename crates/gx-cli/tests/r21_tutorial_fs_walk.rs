// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **R21 / `req/304` D3+D4 (`req/306` §1 item 3)** — `docs/TUTORIAL.md` §2, run out of the file.
//!
//! # What `req/304` measured
//!
//! Its finding D4, severity **high** and named there as the root cause of the other two: neither
//! `README.md` nor this tutorial ever showed a plain `fs` `submit → plan → verify → commit` walk,
//! "even though `gx --help` lists `submit`/`plan`/`verify`/`commit` as the first four commands and
//! `fs` is listed first in `--substrate <fs|git|mcp>`. The only substrate that gets a full worked
//! example is `mcp`."
//!
//! Two things followed from that absence, both by a reader generalising from the one example they
//! were given:
//!
//! * **D2** — a relative `--locator`, accepted at `submit` and refused two verbs later.
//! * **D3**, severity **high**, and `req/304`'s own "single worst finding": an intent file shaped
//!   like the MCP example (`{"tool":…,"arguments":{…}}`) on the `fs` substrate, where `--intent`
//!   is the file's new bytes and nothing parses it. Exit 0 at submit, plan, verify **and** commit;
//!   the JSON string written into the target file, `Admit`-verdicted and signed; no warning at any
//!   of the four stages.
//!
//! # Why the test extracts the blocks rather than restating them
//!
//! `tools/verify_p5.sh` drives the MCP walk by **transcription** — the commands are copied into
//! the script — which makes the page's own claim ("every command below is real … so this page
//! cannot go stale without a battery turning red") true of the script's copy rather than of the
//! page. An edit to the markdown that the script does not receive is exactly the drift the claim
//! denies, and nothing would be red.
//!
//! So this suite reads `docs/TUTORIAL.md`, takes the ```` ```sh ```` blocks of §2 **out of the
//! file**, concatenates them in document order, and runs that as one script with `gx` on `PATH`.
//! The page is the source; the test is a reader of it. A command edited in the markdown is run in
//! its edited form on the next `cargo test`, and a command that stops working is red on the page
//! it lives on.
//!
//! # Denominator — what is *not* claimed
//!
//! * `$HOME` is a scratch directory on the system temporary filesystem, not the developer's. That
//!   is `support::secure_scratch`'s reason and it is also §2's own warning made mechanical: the
//!   repository sits on drvfs where every file reads `0777`, `KeyPair::load` refuses a key looser
//!   than `0600`, and a walk run with `HOME` under `/mnt/c` fails at `gx verify` with a mode
//!   complaint. Measured while writing this lane, and it is why the page says so.
//! * §2's blocks are shell **for a POSIX shell**, so the suite is `#![cfg(unix)]`. The MCP walk's
//!   own battery is Linux-only for the same reason.
//! * The blocks are run, and a handful of load-bearing outputs are asserted (`Admit`, the file's
//!   contents after `commit` and after `undo`, `"valid":true`, and D3's silent miswrite). The
//!   long JSON bodies printed under each block in the page are **not** compared: they carry ids,
//!   signatures and timestamps that differ every run, and a test asserting them would be a test
//!   about a fixture. The page says as much at its own top.

#![cfg(unix)]

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use support::{scratch, secure_scratch};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/gx-cli sits two levels under the root")
        .to_path_buf()
}

/// The heading §2 carries, character for character.
const SECTION: &str = "## 2. The same loop on one plain file — no server";

/// Every ```` ```sh ```` block of one `##` section of the tutorial, in document order.
fn shell_blocks(section: &str) -> Vec<String> {
    let doc = std::fs::read_to_string(repo_root().join("docs/TUTORIAL.md"))
        .expect("docs/TUTORIAL.md is readable");
    let start = doc
        .find(section)
        .unwrap_or_else(|| panic!("`docs/TUTORIAL.md` has no section titled {section:?}"));
    let body = &doc[start + section.len()..];
    let end = body.find("\n## ").unwrap_or(body.len());
    let body = &body[..end];

    let mut blocks = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find("```sh\n") {
        rest = &rest[at + "```sh\n".len()..];
        let close = rest
            .find("```")
            .expect("an unclosed ```sh block in TUTORIAL.md");
        blocks.push(rest[..close].to_string());
        rest = &rest[close + 3..];
    }
    blocks
}

/// 🔴 **`req/304` D4** — the section exists at all, and it is the `fs` walk it says it is.
#[test]
fn the_tutorial_has_an_fs_walk_and_it_names_the_two_things_a_first_run_gets_wrong() {
    let doc = std::fs::read_to_string(repo_root().join("docs/TUTORIAL.md")).expect("readable");
    assert!(
        doc.contains(SECTION),
        "🔴 `req/304` D4 (high, and the root cause of D2 and D3): `fs` is the first substrate \
         `--substrate` lists and the only one with no worked example on this page"
    );
    let blocks = shell_blocks(SECTION);
    let script = blocks.join("\n");
    println!(
        "FS_SECTION_SH_BLOCKS={} BYTES={}",
        blocks.len(),
        script.len()
    );

    // The seven verbs `req/306` §1 item 3 asks the worked example to carry.
    for verb in [
        "key gen",
        "submit",
        "plan",
        "verify",
        "commit",
        "receipt show",
        "log checkpoint",
        "receipt verify",
        "undo",
    ] {
        assert!(
            script.contains(verb),
            "the worked example has to drive `gx {verb}`; the section's shell blocks do not"
        );
    }
    // D2 and D3, said in the prose rather than left for the reader to discover.
    let section_prose = {
        let start = doc.find(SECTION).expect("found above");
        let body = &doc[start..];
        let end = body[SECTION.len()..]
            .find("\n## ")
            .map_or(body.len(), |e| e + SECTION.len());
        body[..end].to_string()
    };
    assert!(
        section_prose.contains("must be an absolute path"),
        "🔴 D2: a relative `--locator` is the first thing a newcomer types and the page has to \
         say so before they type it"
    );
    assert!(
        section_prose.contains("It is not JSON"),
        "🔴 D3 (the worst finding in `req/304`): `--intent` for `fs` is raw bytes, and a reader \
         generalising from the MCP example silently writes a JSON string into their file"
    );
}

/// 🔴 The page's own claim, made true: §2's shell blocks are run, out of the file.
#[test]
fn the_fs_walk_in_the_tutorial_runs_green_as_written() {
    let blocks = shell_blocks(SECTION);
    assert!(
        blocks.len() >= 8,
        "§2 parsed short: {} blocks",
        blocks.len()
    );

    let work = scratch("r21_tutorial_fs");
    let home = secure_scratch("r21_tutorial_fs_home");
    let bin = support::gx().get_program().to_string_lossy().to_string();
    let bin_dir = Path::new(&bin)
        .parent()
        .expect("the test binary has a directory")
        .to_path_buf();

    // 🔴 The blocks are run **verbatim**, with nothing substituted.
    //
    // That is a property of the page rather than of this file, and it was worth a rewrite of §2 to
    // get: its blocks capture each verb's stdout to a file and derive the next id from it with
    // `python3 -c`, which is the shape §4 already used for the MCP walk. A page whose ids were
    // literals would need this test to patch them, and a test that patches a page is not running
    // the page — it is running its own edit of one.
    //
    // `set -eu` so the first non-zero exit is the failure rather than a later, confusing one.
    let mut script = String::from("set -eu\n");
    for block in &blocks {
        script.push_str(block);
        script.push('\n');
    }
    println!("--- SCRIPT ---\n{script}\n--- END ---");

    let out = Command::new("bash")
        .arg("-c")
        .arg(&script)
        .current_dir(&work)
        .env("HOME", &home)
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin_dir.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .expect("bash runs");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    println!("TUTORIAL_FS_RC={:?}", out.status.code());
    println!("--- STDOUT ---\n{stdout}");
    println!("--- STDERR ---\n{stderr}");
    assert!(
        out.status.success(),
        "🔴 a command on the page does not work as written (rc={:?}).\nSTDERR:\n{stderr}",
        out.status.code()
    );

    // The load-bearing outputs, and only those.
    assert!(
        stdout.contains("\"kind\":\"Admit\""),
        "the gate admitted the change: {stdout}"
    );
    assert!(
        stdout.contains("\"inclusion\":\"verified\"") && stdout.contains("\"valid\":true"),
        "the offline verify passed with three files and no project: {stdout}"
    );
    assert!(
        stdout.contains("after an agent wrote through gx"),
        "`commit` really wrote the file: {stdout}"
    );
    // 🔴 D3, kept as a live measurement rather than as a paragraph: the page's warning is only
    // worth reading while the thing it warns about is still true of the binary.
    assert!(
        stdout.contains("{\"tool\":\"notes.write\",\"arguments\":{\"contents\":\"hello\"}}"),
        "🔴 `req/304` D3: the page warns that a JSON intent is written into the file verbatim, \
         with four exit-0 verbs and no refusal. If that is no longer true the warning is wrong \
         and has to be rewritten rather than left standing: {stdout}"
    );
    // …and the way back, which is the half that makes the warning bearable.
    let restored = stdout.matches("before any agent touched it").count();
    println!("RESTORED_LINES={restored}");
    assert!(
        restored >= 2,
        "both undos restore the file byte for byte (the walk's, and D3's): {stdout}"
    );
}
