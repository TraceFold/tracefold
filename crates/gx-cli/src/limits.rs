// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `gx limits` (**P5**, `req/134` §1 item 7, ruling 2: filed as an addition to 44) — the eight lines
//! `req/spec/20-business/21-business-plan.md` §10-4 fixes ("say up front 'what we do not
//! guarantee'"), printed for (sem: SEM-gx-cli-054)
//! a terminal. `docs/LIMITS.md` prints the identical eight for a browser, and
//! `crates/gx-cli/tests/limits_sync.rs` is what keeps the two from drifting apart (AC-P5-5).
//!
//! # 🔴 This file writes no new limit
//!
//! 21 §10-4's own header: "this section creates no new limit" (the place to create one is 45) (sem: SEM-gx-cli-055). Every
//! [`Limit::statement`] below is a plain-language paraphrase of a sentence 45 (the threat model) or
//! req/38's P1/P2/P3 close reports already state, and [`Limit::citation`] is where. §10-4's original
//! eight are five here (TH-1/TH-2/TH-5/FR-M04/Lean+CI-scope); the other three fold in what closed
//! *after* §10-4 was drafted (2026-08-13) — P3's declared bypass roads (B-2/B-9/B-10) and P1/P2's
//! census measurement of how much of the write surface `gx undo` can actually reach — because a
//! `/limits` page that repeated only what was known before P3 closed would itself be the kind of
//! silent staleness 45 §0 calls a P-9 risk.

use serde_json::json;

use crate::exit::Outcome;

/// One line of `/limits`: the statement in plain language, and the clause it paraphrases.
pub struct Limit {
    /// What this build does not cover, in a sentence a first-time reader can act on without
    /// following the citation.
    pub statement: &'static str,
    /// Where the claim comes from. Never itself printed as the limit — a citation is for checking
    /// the paraphrase, not a substitute for [`Limit::statement`].
    pub citation: &'static str,
}

/// The eight lines. `crates/gx-cli/tests/limits_sync.rs` fixes the count at 8 (AC-P5-5) and checks
/// that every [`Limit::statement`] appears verbatim in `docs/LIMITS.md`.
pub const LIMITS: [Limit; 8] = [
    Limit {
        statement: "if a policy does not say what you actually meant, gx enforces the policy exactly \
                     as written -- it checks that a change satisfies the rule, not that the rule was \
                     the right one to write (the Oracle problem).",
        citation: "45 §2.1 TH-1 / §4.1",
    },
    Limit {
        statement: "an attacker with root or kernel privilege can write around gx entirely, and this \
                     build does not detect it.",
        citation: "45 §2.2 TH-2",
    },
    Limit {
        statement: "revoking a key does not undo what was signed while it was still trusted -- how \
                     far back a revocation reaches is a setting an operator chooses, and gx only \
                     checks that everything after that point is consistent with it.",
        citation: "45 §3.1 candidate 13 / TH-5",
    },
    Limit {
        statement: "gx cannot see where an MCP tool's effect actually lands once a call reaches the \
                     server -- an agent that starts its own server, or a call that only reads \
                     (`resources/read`), is outside this build's proxy by declared design, not by an \
                     omission nobody noticed.",
        citation: "45 §3.1 candidate 14; req/119 §2.6 roads B-2 and B-9",
    },
    Limit {
        // 🔴 The URL stays on this opening line rather than a continuation one: gx-canon's
        // `authority_boundary.rs` scans each source line independently for an un-quoted `//`, and
        // only the line carrying the literal's opening `"` starts that scan already `in_string`.
        statement: "gx normalises a resource address the way RFC 3986 does, but it cannot see a server that quietly maps one path onto another underneath it -- if a server resolves `file:///srv/x` to \
                     `/etc/passwd` internally, gx never learns.",
        citation: "45 §3.1 candidate 12; req/119 §2.6 road B-10",
    },
    Limit {
        statement: "a verdict count only proves nothing was hidden from it -- it does not prove the \
                     policy behind it was strict. weaken the policy to admit everything and the count \
                     still reads clean, and two verifiers can legitimately be shown two different \
                     chains.",
        citation: "32 FR-M04 limits ①②",
    },
    Limit {
        statement: "`gx undo` only works where the server it asks exposes a real inverse for that \
                     tool -- measured across a first census of public MCP tools, that held for about \
                     13.8% of the ones that write anything at all; the rest have no restoring call to \
                     send, and gx cannot invent one.",
        citation: "req/38 §78 (census v2 stage1, first measurement)",
    },
    // v0.4-l (`req/189` H-13, `req/182` §1-1): the eighth line said "only 2 of this project's
    // crates" are checked on every push and "an independent Lean proof ... does not exist yet" --
    // both stale by three milestones (ci.yml gates 16 crates via tools/ci.sh; lean/ holds eight
    // kernel-checked theorems since M8). An honest-limits page that under-reports is a page that
    // is not true, which is P-9's whole objection to over-reporting, one row over. The old sentence
    // is kept in `docs/LIMITS.md` under a struck note (no-delete); the live line below is written
    // in the three faces req/38 §120 asks for: what is proven (Lean), what is only compared (the
    // differential test -- a difference check, not a proof), and what a machine enforces (CI scope).
    // 🔴 **`req/38` §207 ruling 2 / `req/288` item 1** — the first clause used to say "eight
    // theorems and five counterexamples", `README.md` said "92 theorems, 2 axioms", and both were
    // read by the same buyer. Neither was a primary count: the eight/five pair was written when
    // `lean/` held eight theorems and was never moved, and the 92/2 pair came from a pattern that
    // tolerates leading whitespace and so counted two lines of English prose inside a block comment
    // in `MinimalityF0.lean` that begin with the words `theorem` and `axiom`. The numbers below are
    // a **line-start** count over `lean/`, and `crates/gx-cli/tests/limits_sync.rs` now recounts the
    // tree on every run and holds this line, `docs/LIMITS.md` and `README.md` to what it finds --
    // so the next lane that adds a theorem is red here rather than silently stale for a release.
    Limit {
        statement: "the Lean model under `lean/` proves 117 theorems about the F0 specification, 12 \
                     of them named counterexamples, over 13 files, with 0 `sorry` and 1 `axiom` \
                     carried rather than proved (a line-start count of the tree itself, which a test \
                     re-takes on every run); the Rust implementation is \
                     compared against that model by a differential test -- 1,500 conformance vectors, \
                     six kinds, on every push -- and a comparison is a difference check, not a proof: \
                     no refinement theorem connects the two. of the checks that run automatically on \
                     every push today, 16 of this project's 17 workspace crates are covered \
                     (`probes/doubt` runs by hand because its subject lives outside the repository) and \
                     the TypeScript SDK's own tests are not run by CI at all -- their green is a \
                     person's run, not a machine's.",
        citation: "45 §4.2 (v0.2.3 note; v0.4-l present-tense note); 51 §11.1 (v0.2.3 note; v0.4-l present-tense note)",
    },
];

/// `gx limits` (ruling 2: filed as an addition to 44; sem: SEM-gx-cli-056). Plain text by default, one entry per [`LIMITS`] row;
/// `--json` for the structured form a script reads.
///
/// # Errors
/// Never — `LIMITS` is a compile-time constant and nothing here touches the filesystem or a
/// subprocess.
pub fn run(as_json: bool) -> crate::Result<Outcome> {
    let items: Vec<_> = LIMITS
        .iter()
        .map(|l| json!({ "statement": l.statement, "citation": l.citation }))
        .collect();
    if as_json {
        return Ok(Outcome::ok(
            json!({ "gx": "limits", "count": LIMITS.len(), "limits": items }),
        ));
    }
    // 🔴 **R13 / `req/244` H-01** — the plain-text face goes through `emit` for the reason the JSON
    // one does: `println!` panics on a write error, and `gx limits | head -1` is a command an
    // operator types. Each `?` is the line saying it did not land.
    crate::say!("gx does not cover everything. this build's known gaps, plainly:")?;
    crate::say!()?;
    for (i, limit) in LIMITS.iter().enumerate() {
        crate::say!("{}. {}", i + 1, limit.statement)?;
        crate::say!("   ({})", limit.citation)?;
        crate::say!()?;
    }
    Ok(Outcome::ok(
        json!({ "gx": "limits", "count": LIMITS.len() }),
    ))
}
