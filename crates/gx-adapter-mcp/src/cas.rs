// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! 🔴 **DR-46-16** (`req/38` §218 ruling 1) — `cas`: the one place the compare-and-set half reads
//! an object, and the second of this crate's **two** roads to the wire.
//!
//! # What this module is, in one sentence
//!
//! [`crate::McpAdapter::snapshot`], `SubstrateAdapter::precondition` and the post-apply
//! observation in [`crate::apply`] all ask one question — *what does this object hold right now?*
//! — and until this window all three answered it with an unconditional
//! `ToolTransport::read`, which is MCP's `resources/read`. A server that publishes no resource
//! face for an object therefore had no CAS at all, and `gx wrap` refused to plan on it however
//! complete its restore catalogue was (`tests/notion_page_catalogue.rs` measured that on the real
//! notion-mcp-server). This module is where the question is asked now, and it takes the road the
//! deployment declared: [`crate::catalogue::CAS_READ_KEY`] when a prefix matches the resource,
//! `resources/read` when none does.
//!
//! # 🔴 Why one function and not three call sites
//!
//! `read_prior_by_tool` frames a `tools/call`, so every site that reaches it is a road AC-051 has
//! to account for — `req/38` §196 made that accounting an implementation condition of DR-46-9 A-3
//! and `tests/ac_051.rs` D-6 derives it by walking `src/` rather than by being told. Three call
//! sites (`adapter.rs` twice, `apply.rs` once) would have been three roads to bound and three
//! places for the next lane to forget one. One funnel is **one** road, and D-6 counts two sites in
//! the whole crate: `invert.rs` (the escrow half) and this file (the CAS half).
//!
//! What goes out of here is bounded the same way the escrow road's is, and the bound is what
//! matters rather than the count: the tool name is a string in the **deployment's catalogue** and
//! every argument is built by [`crate::catalogue::CasTemplate`] out of the **locator** and
//! constants the declaration supplies. No member of this road's arguments can come from an agent's
//! payload, because at `snapshot` there is no payload — 41 §4 hands the method a locator and
//! nothing else. So the set of tools an agent can reach through this road is the set an operator
//! wrote down, and it is empty until an operator writes one.
//!
//! # 🔴 The critical section is not widened (`req/38` §195, clause ⑤)
//!
//! One read, either way, exactly as `crate::invert` says of the escrow road. A declaration that
//! names a CAS read face takes that road **instead of** `resources/read`, never in addition to it,
//! so the number of server round trips `snapshot`, `precondition` and `observe` each cost is the
//! number each cost before this lane: one. `tests/dr46_16_cas_read_by_tool.rs` counts the arrivals
//! on both roads and holds them equal rather than asserting the sentence.
//!
//! # 🔴 What this module does **not** do
//!
//! It does not bind the read's answer to the object. [`crate::catalogue::ObjectIdentity`] is that
//! predicate on the escrow road (DR-46-15, after `req/269` H-01 measured an undo overwriting a
//! third party's write because the compare-and-set was watching a file nobody had touched), and
//! the same two objects exist here: the CAS attests `position.resource()` and this read answers
//! about whatever the declared tool answers about. `req/38` §218 ruling 2 put that predicate in
//! its own lane — **DR-46-21**, `req/305` — under the one-lane-one-invariant rule, so it is
//! declared missing here and in `docs/LIMITS.md` rather than assumed present.

use gx_core::Cid;
use gx_substrate::{Error, Result};

use crate::catalogue::Catalogue;
use crate::locator::Position;
use crate::transport::ToolTransport;

/// What the object at `position` holds right now, by the road the deployment declared.
///
/// The **only** function in this crate's CAS half that reaches a transport, and one of the two in
/// the crate that reach [`ToolTransport::read_prior_by_tool`] (`tests/ac_051.rs` D-6 derives both
/// by walking `src/`).
///
/// # Errors
/// Whatever the transport says about an object it will not answer for — unchanged from the road
/// this replaced, which is what keeps a deployment that declares no `$cas_read` byte-for-byte where
/// it was. [`Error::Unreadable`] additionally for a CAS read declaration that is **not sound**, in
/// the words `crate::invert` already uses for the escrow road's version of that fault: the
/// catalogue is wrong and the server has done nothing, so the remedy named is the declaration.
pub(crate) fn read_subject(
    transport: &dyn ToolTransport,
    catalogue: &Catalogue,
    position: &Position,
) -> Result<Vec<u8>> {
    let Some((pattern, read)) = catalogue.cas_read_for(position.resource()) else {
        // No declaration for this locator: `resources/read`, exactly as every catalogue written
        // before this window meant without having a way to say it. Zero regression by
        // construction — this is the line the three call sites used to hold inline.
        return transport.read(position.server(), position.resource());
    };
    // 🔴 `req/269` M-05's argument, one road over: a catalogue built in code
    // (`Catalogue::with_cas_read`) never met a parser, so a check that lived only in `from_json`
    // would be a check a binary could ship past. The same predicate, over the same value, in the
    // words a declaration fault carries — and, like the escrow road's, it never consults
    // `$on_read_failure`, because that slot is a decision about what to do when a *server* will
    // not answer and offering it as the remedy for a typo is offering a way to make the typo
    // permanent.
    catalogue
        .cas_read_soundness(pattern)
        .and_then(|()| {
            read.arguments()
                .resolve(position.server(), position.resource(), pattern)
        })
        .map_err(|detail| Error::Unreadable {
            locator: position.locator(),
            detail: format!("{detail}: {}", crate::invert::DECLARATION_UNSOUND_REFUSAL),
        })
        .and_then(|arguments| {
            transport.read_prior_by_tool(position.server(), read.by_tool(), &arguments)
        })
        // 🔴 **DR-46-21** (`req/38` §218 ruling 2, §417/§421) — bind the answer to the object, for
        // the content-addressed half. The escrow road's `ObjectIdentity` cannot be reused here
        // (`tests/dr46_21_identity_binding_gap.rs` arm 2: a CAS answer is raw bytes, not a `{id,…}`
        // document), so the binding is the one content-addressing already carries.
        .and_then(|contents| verify_content_address(position, pattern, contents))
}

/// 🔴 **DR-46-21** — re-verify a **content-addressed** read's answer against the digest its locator
/// names, and let every other read past untouched.
///
/// Opt-in **by construction** (`req/38` §421 ruling (1)): the value keying this read is the suffix
/// the matched prefix left behind, and the binding fires only when that suffix is itself a `gx1:`
/// CID — i.e. the deployment content-addressed the object. A name-keyed read (`notion://page/abc-123`,
/// whose suffix [`Cid::from_text`] declines) is returned unchanged, byte-for-byte where it was, so
/// the name-keyed `$cas_read` declarations do not move and the neighbour a dishonest name-keyed
/// server answers stays a documented residual (`req/38` §421 ruling (2), `docs/LIMITS.md`) rather
/// than a gap this road can close.
///
/// Where it does fire, a mismatch is [`Error::Unreadable`] — the funnel's own word — carrying
/// [`crate::invert::CAS_OBJECT_DIGEST_REFUSAL`]: the read answered about a different object, and
/// going on would digest a stranger's bytes against this locator (`req/269` H-01, one road over).
fn verify_content_address(
    position: &Position,
    pattern: &str,
    contents: Vec<u8>,
) -> Result<Vec<u8>> {
    let resource = position.resource();
    let suffix = resource.strip_prefix(pattern).unwrap_or(resource);
    let Ok(expected) = Cid::from_text(suffix) else {
        return Ok(contents);
    };
    if crate::adapter::content_digest(&contents) == expected {
        Ok(contents)
    } else {
        Err(Error::Unreadable {
            locator: position.locator(),
            detail: crate::invert::CAS_OBJECT_DIGEST_REFUSAL.to_string(),
        })
    }
}
