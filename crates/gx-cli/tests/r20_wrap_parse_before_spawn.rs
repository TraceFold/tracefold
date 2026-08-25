// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/288` item 5 / `req/38` §207 ruling 4** — "`gx wrap` does not start" is now literal.
//!
//! `req/285` §2 measured the eighteenth audit's repair from the outside and found a residue it
//! could not fix from its own write scope. With an H-01-shaped catalogue (`read_by` declared and
//! no restore template) `gx wrap` refuses, and the refusal is the right one — but
//! `crates/gx-cli/src/wrap.rs::run` called `client.initialize()` **before** it parsed the
//! catalogue, so by the time the refusal was printed the server's child process had already been
//! spawned and had already completed a handshake. The gate never opened and no `tools/call` was
//! ever relayed, so nothing was written; what was untrue was the **sentence**, and `docs/LIMITS.md`
//! prints that sentence to a buyer.
//!
//! This file holds the order from the outside, where a reader can see it: not "the parse function
//! is called on line N" but "**no child process ran**".
//!
//! # How the discriminator works
//!
//! The server handed to `gx wrap` is a one-line shell script that creates a marker file and exits.
//! It is a legal thing to spawn and an illegal thing to hand a handshake to, so it separates the
//! two questions cleanly:
//!
//! * **marker present** = the child was spawned (whatever happened afterwards).
//! * **marker absent** = the child was never spawned.
//!
//! The second probe is the control that keeps the first one from being vacuous: with a **sound**
//! catalogue the same script, the same flags and the same project produce the marker. A probe that
//! only ever asserted absence would pass on a `gx wrap` that had stopped spawning anything at all.
//!
//! # `cfg(unix)`
//!
//! For `serve_runtime_e2e.rs`'s reason, one notch smaller: this file needs an executable bit and a
//! `#!` line. Windows is measured zero times here as it is everywhere else in this crate, and
//! `docs/LIMITS.md` says so.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// `read_by` declared with no restore template — the audit's `n2`, the shape
/// `crates/gx-adapter-mcp/tests/r18_declaration_soundness.rs` refuses at parse time.
const H01_CATALOGUE: &str = r#"{
  "doc.write": {
    "restored_by": "doc.restore",
    "read_by": {
      "by_tool": "doc.get",
      "arguments": { "id": { "forward": "id" } },
      "identity": [ "doc:", { "answer": "/id" } ]
    }
  }
}"#;

/// The same declaration **with** the template: the shape that parses, unchanged by this lane.
const SOUND_CATALOGUE: &str = r#"{
  "doc.write": {
    "restored_by": "doc.restore",
    "arguments": { "id": { "forward": "id" }, "text": "prior_contents_utf8" },
    "read_by": {
      "by_tool": "doc.get",
      "arguments": { "id": { "forward": "id" } },
      "identity": [ "doc:", { "answer": "/id" } ]
    }
  }
}"#;

/// A temp directory that removes itself, so a failed probe does not leave a marker behind for the
/// next one to read.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "gx_r20_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock is after 1970")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("scratch directory");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Write the marker-dropping server script and return `(script, marker)`. The script is a legal
/// program and an illegal MCP server: it says it ran, then exits, so the handshake fails on EOF
/// rather than hanging.
fn marker_server(scratch: &Scratch) -> (PathBuf, PathBuf) {
    let marker = scratch.path().join("the_child_ran");
    let script = scratch.path().join("server.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\n# `req/288` item 5: this file exists to answer one question -- did anything\n# start this program?\n: > {}\nexit 0\n",
            marker.display()
        ),
    )
    .expect("write the server script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("the script is executable");
    (script, marker)
}

fn wrap_with(catalogue_json: &str, scratch: &Scratch, script: &Path) -> Output {
    let catalogue = scratch.path().join("catalogue.json");
    std::fs::write(&catalogue, catalogue_json).expect("write the catalogue");
    Command::new(env!("CARGO_BIN_EXE_gx"))
        .arg("--project")
        .arg(scratch.path())
        .arg("wrap")
        // 42 §3.2's two required facts about the agent. They are checked for **presence** before
        // `wrap::run` is entered and are never reached on either road below, so a literal pair is
        // honest here: nothing in this file signs anything.
        .args(["--actor-key", "r20-probe", "--actor-model", "r20-probe"])
        .arg("--restore-catalogue")
        .arg(&catalogue)
        .arg("--")
        .arg(script)
        .output()
        .expect("gx wrap runs")
}

/// 🔴 The residue of `req/285` §2, closed: an unsound catalogue is refused **before** anything is
/// started, so "`gx wrap` does not start" is a sentence about processes and not only about
/// sessions.
#[test]
fn an_unsound_catalogue_is_refused_before_the_server_child_is_spawned() {
    let scratch = Scratch::new("h01");
    let (script, marker) = marker_server(&scratch);
    let out = wrap_with(H01_CATALOGUE, &scratch, &script);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let said = format!("{stdout}{stderr}");
    println!("R20_H01_OUTPUT={said}");
    println!("R20_H01_MARKER_EXISTS={}", marker.exists());

    assert!(
        !out.status.success(),
        "an H-01 catalogue must not produce a session: {said}"
    );
    assert!(
        said.contains("--restore-catalogue") && said.contains("not sound"),
        "the refusal a reader gets must be the catalogue's, not a handshake's, because the \
         catalogue is what is wrong: {said}"
    );
    assert!(
        !marker.exists(),
        "🔴 `req/38` §207 ruling 4: the server's child process was spawned before the catalogue \
         was read. No gate opened and no call was relayed, but `docs/LIMITS.md` tells a buyer that \
         `gx wrap` does not start, and a process that ran is a process that ran"
    );
}

/// The control. Absence is only evidence if presence is reachable by the same road.
#[test]
fn a_sound_catalogue_still_spawns_the_server_child() {
    let scratch = Scratch::new("sound");
    let (script, marker) = marker_server(&scratch);
    let out = wrap_with(SOUND_CATALOGUE, &scratch, &script);
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    println!("R20_SOUND_OUTPUT={said}");
    println!("R20_SOUND_MARKER_EXISTS={}", marker.exists());

    assert!(
        marker.exists(),
        "a sound catalogue must still reach the spawn -- otherwise the probe above measures a \
         `gx wrap` that starts nothing at all, which is a different bug wearing the same green: \
         {said}"
    );
    // What happens after the spawn is this script's own fault (it is not an MCP server) and is not
    // what this file is about; the probe asserts only that the road was taken.
    assert!(
        !out.status.success(),
        "the script is not a server, so the handshake fails -- stated here so that a future reader \
         does not read the control as an end-to-end success: {said}"
    );
}
