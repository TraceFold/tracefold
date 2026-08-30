// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **`req/895` §3** — the whole `--help` surface, and the error path that shares its text.
//!
//! `r21_help_is_user_facing.rs` holds the **banner** to this rule and says, in as many words, that
//! it deliberately does not hold the rest: "It does **not** hold the per-subcommand summaries in
//! the `Commands:` block, and they are in the same condition or worse … `req/307` §3 files the rest
//! as a measured row with a count." `req/878` took that count on a fresh anonymous clone and found
//! **every one of the 18 public verbs leaking**, 6–16 hits each, with nine of them leaking on the
//! plain no-arguments error path as well. This file is that scope, widened deliberately rather than
//! silently, and closed.
//!
//! # What is being asserted, and why it is a boundary rather than a style rule
//!
//! `--help` is the first thing anybody runs after a build finishes. A citation like `44 §1.2`,
//! `sem: SEM-gx-cli-420` or `req/38 §92 ruling 1` names a document the reader does not have and
//! cannot get: this repository's `req/` tree is not shipped. So the sentence is not merely untidy,
//! it is **unresolvable** — it asks the reader to go somewhere that does not exist for them. The
//! same strings travel into `serve`'s and `cancel`'s JSON refusal bodies, which a caller's script
//! parses and logs.
//!
//! # Where the words went
//!
//! Nowhere. `req/306` §1 item 2's rule stands and this file keeps it: **a doc comment is
//! provenance and a help string is a user-facing string, and neither is the other.** The repair is
//! `#[arg(help = …, long_help = None)]` / `#[command(about = …, long_about = None)]` on the clap
//! items, which is the shape `Cli`'s own `about`/`long_about` has had since R21. Every doc comment
//! is left where it is, with every citation in it.
//!
//! # The two things this file does to keep itself honest
//!
//! * **A denominator.** It prints how many surfaces it walked and refuses to pass on a small
//!   number. A walker that found no subcommands would otherwise report zero hits and mean nothing.
//! * **A negative control on the predicate itself.** The vocabulary regex is asserted to fire on
//!   strings that are known to be private and to stay silent on ordinary English. A gate whose
//!   detector has quietly stopped matching is the failure mode this whole lane is about.

mod support;

use std::collections::BTreeSet;

use support::{gx, run, scratch};

/// Every invocation in this file runs against a throwaway home and a throwaway project.
///
/// 🔴 A bare `gx key gen` **succeeds** and writes a secret. Walking the no-argument path of every
/// subcommand without this would put one in the developer's own `~/.gx/keys/`, which is a test
/// with a side effect on the machine that ran it.
fn sandboxed(args: &[&str]) -> support::Run {
    let home = scratch("r895_help_home");
    let project = scratch("r895_help_project");
    let mut cmd = gx();
    cmd.env("HOME", &home)
        .env("USERPROFILE", &home)
        .arg("--project")
        .arg(&project);
    for a in args {
        cmd.arg(a);
    }
    run(&mut cmd)
}

/// 🔴 The vocabulary that names something the reader cannot open.
///
/// A **predicate**, not a copy of the old strings (`r21`'s reasoning, kept): a rewrite that swapped
/// `44 §1.2` for `47 §9.9` would satisfy a `!=` assertion and change nothing for a reader.
fn private_hits(text: &str) -> Vec<String> {
    let mut hits: Vec<String> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        for needle in NEEDLES {
            if line.contains(needle) {
                hits.push(format!("L{}: {needle:?} in {:?}", n + 1, line.trim()));
            }
        }
        // A section sign followed by a digit is a specification cross-reference in every spelling
        // this tree uses (`44 §1.2`, `§7-3b`, `42 §3.13`).
        if let Some(i) = line.find('§') {
            if line[i..].chars().nth(1).is_some_and(|c| c.is_ascii_digit()) {
                hits.push(format!("L{}: \"§<digit>\" in {:?}", n + 1, line.trim()));
            }
        }
    }
    hits
}

/// The literal markers. Each one names a private artefact of this repository.
const NEEDLES: [&str; 14] = [
    "req/",     // requirement documents; not shipped
    "sem: ",    // semantic-anchor tags
    "SEM-gx",   // the same, spelled out
    "DR-",      // design rulings
    "E-M",      // errata
    "M4H",      // hand reports
    "M5H",      //
    "M6H",      //
    "AC-0",     // acceptance criteria by number
    "ASM-",     // assumptions by number
    "FR-M",     // functional requirements, internal numbering
    " ruling ", // "…, ruling 1"
    "🔴",       // the ledger's own emphasis marker
    "adopted (",
];

/// One command path, and the text a user sees for it.
struct Surface {
    path: Vec<String>,
    what: &'static str,
    text: String,
}

/// The subcommand names clap lists under `Commands:`.
fn subcommands_of(help: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut inside = false;
    for line in help.lines() {
        if line.starts_with("Commands:") {
            inside = true;
            continue;
        }
        if inside {
            if line.trim().is_empty() {
                continue;
            }
            // clap indents command rows by two spaces and starts options with a dash.
            if !line.starts_with("  ") || line.starts_with("   ") {
                break;
            }
            let Some(word) = line.split_whitespace().next() else {
                break;
            };
            if word == "help" || word.starts_with('-') {
                continue;
            }
            names.push(word.to_string());
        }
    }
    names
}

/// 🔴 Deliberately not invoked with no arguments: `gx demo` **runs** with none — it is a whole
/// throwaway lifecycle — so a bare call would be this file doing work rather than reading a
/// refusal. Its `--help` is still walked. Declared here rather than left as a silent gap.
const NOT_RUN_BARE: [&str; 1] = ["demo"];

/// Walk `--help` at every depth, plus the no-argument path at every depth that is safe to call.
fn walk() -> Vec<Surface> {
    let mut out = Vec::new();
    let root = run(gx().arg("--help"));
    assert_eq!(root.code, 0, "`gx --help` is a normal termination");
    out.push(Surface {
        path: vec![],
        what: "--help",
        text: root.stdout.clone(),
    });

    for verb in subcommands_of(&root.stdout) {
        let help = run(gx().args([verb.as_str(), "--help"]));
        out.push(Surface {
            path: vec![verb.clone()],
            what: "--help",
            text: format!("{}{}", help.stdout, help.stderr),
        });
        if !NOT_RUN_BARE.contains(&verb.as_str()) {
            let bare = sandboxed(&[verb.as_str()]);
            // 🔴 Only the **refusal**. `req/878` finding 2 is about the error path a bare call
            // takes ("nine verbs print the same leaky text on their plain no-args error path"), and
            // a verb that answers `0` to a bare call has printed its *output* rather than a
            // refusal. `gx limits` is the one such verb, and its eight lines are held byte-for-byte
            // against `docs/LIMITS.md` by a separate agreement check — editing one side of that
            // pair here would break the pair to tidy a sentence. It carries specification
            // citations of its own and that is a finding in its own right, recorded in `req/895`
            // §3-5 rather than folded in silently.
            if bare.code != 0 {
                out.push(Surface {
                    path: vec![verb.clone()],
                    what: "no arguments",
                    text: format!("{}{}", bare.stdout, bare.stderr),
                });
            }
        }
        for sub in subcommands_of(&help.stdout) {
            let sub_help = run(gx().args([verb.as_str(), sub.as_str(), "--help"]));
            out.push(Surface {
                path: vec![verb.clone(), sub.clone()],
                what: "--help",
                text: format!("{}{}", sub_help.stdout, sub_help.stderr),
            });
            // 🔴 The refusal a subcommand gives when it is called with nothing. This is where the
            // hand-validated requirements live — the ones `req/878` finding 3 named as invisible in
            // `--help` — so it is the surface on which a reader first meets them.
            let bare = sandboxed(&[verb.as_str(), sub.as_str()]);
            if bare.code != 0 {
                out.push(Surface {
                    path: vec![verb.clone(), sub.clone()],
                    what: "no arguments",
                    text: format!("{}{}", bare.stdout, bare.stderr),
                });
            }
        }
    }
    out
}

/// 🔴 The gate: nothing a reader cannot open appears on the surface a reader types.
#[test]
fn no_help_screen_or_bare_error_names_a_private_document() {
    let surfaces = walk();
    let verbs: BTreeSet<&str> = surfaces
        .iter()
        .filter_map(|s| s.path.first().map(String::as_str))
        .collect();

    let mut offenders: Vec<String> = Vec::new();
    let mut total_hits = 0usize;
    for surface in &surfaces {
        let hits = private_hits(&surface.text);
        if !hits.is_empty() {
            total_hits += hits.len();
            offenders.push(format!(
                "gx {} [{}] — {} hit(s):\n    {}",
                surface.path.join(" "),
                surface.what,
                hits.len(),
                hits.join("\n    ")
            ));
        }
    }

    println!(
        "R895_HELP_SURFACES={} VERBS={} OFFENDING_SURFACES={} TOTAL_HITS={}",
        surfaces.len(),
        verbs.len(),
        offenders.len(),
        total_hits
    );

    // 🔴 The denominator. A walker that parsed nothing would report zero hits and prove nothing —
    // which is the exact shape of the greens this lane exists to distrust.
    assert!(
        verbs.len() >= 15,
        "the walker found only {} verbs; it is measuring almost nothing",
        verbs.len()
    );
    assert!(
        surfaces.len() >= 40,
        "the walker found only {} surfaces; it is measuring almost nothing",
        surfaces.len()
    );

    assert!(
        offenders.is_empty(),
        "the public command line names documents its reader cannot open ({total_hits} hit(s) \
         across {} surface(s)). The words belong in doc comments, which keep them; the help \
         strings are `#[arg(help = …)]` / `#[command(about = …)]`:\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

/// 🔴 The gate's own negative control: the detector fires, and does not fire on ordinary English.
///
/// Without this, a regex that had quietly stopped matching would report a clean surface forever.
/// The strings below are real ones this binary printed before `req/895`.
#[test]
fn the_detector_fires_on_what_it_is_for_and_not_on_prose() {
    let must_fire = [
        "44 §1.3: \"the CLI offers additional human-readable formatting via `--pretty`\"",
        "The same declaration as a JSON file (A2, req/38 §92 ruling 1; sem: SEM-gx-cli-422)",
        "🔴 **R6 / DR-43-10** — take the project's signed head out of the box",
        "44 §1.2's flag. v0.1 signs with the original actor's key",
        "DR-2 record-only, for this call (M6-08 adopted (a))",
        "T-5b: a person rules on an escalated transformation (AC-072)",
        "the escrow is 43 T-10b and ASM-14 keeps it absent",
        "and E-M6-12 says so",
        "M6H4-6 writes it optional",
        "FR-M04's aggregate",
    ];
    let must_stay_quiet = [
        "The project whose .gx/ directory to use. Defaults to the working directory.",
        "Take a change back: gx undo replays the inverse gx escrowed before it applied anything.",
        "Print a machine-readable JSON object on stdout.",
        "Section 3 of the manual is a good place to start.",
    ];
    let mut fired = 0usize;
    for s in must_fire {
        let hits = private_hits(s);
        assert!(!hits.is_empty(), "the detector did not fire on {s:?}");
        fired += 1;
    }
    for s in must_stay_quiet {
        let hits = private_hits(s);
        assert!(
            hits.is_empty(),
            "the detector fired on ordinary prose {s:?}: {hits:?}"
        );
    }
    println!(
        "R895_DETECTOR_FIRED={fired}/{} QUIET={}",
        must_fire.len(),
        must_stay_quiet.len()
    );
    assert_eq!(fired, must_fire.len());
}
