// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The time adapter: the substrate where an effect is placed now and lands later.
//!
//! Ticket: `req/1007` §4 item 3 (**WM-4a**), reqdef and rulings in `req/1038`. Spec: 41 §4 for the
//! seven methods, 42 §3.1 for `SubstrateKind::Custom`, 42 §3.4 for the delta, 42 §3.5 for the
//! fingerprint. It implements no new law and adds no contract row: `req/1038` §6a measured that the
//! shared harness reads `invert`'s `Option` projection and not its verdict, so a three-valued
//! adapter sits in the seats that are already there.
//!
//! # What the substrate is
//!
//! A **schedule**: a set of entries, each at its own position, each saying *what* is to be run and
//! *when*. The object this adapter acts on is one entry. The two changes it plans are the two an
//! agent actually makes to a schedule -- put an entry in it, and take one out.
//!
//! # 🔴 What runs the entries is not gx
//!
//! This crate starts no process, opens no socket, and speaks to no scheduling service. Something
//! else -- cron, a systemd timer, a service's own runner -- reads the schedule and runs what is due.
//! That division is the whole reason the substrate is interesting: gx holds the *record of a future
//! effect*, which it can revoke, while the *firing* belongs to a party gx does not control.
//!
//! ∴ two claims this crate does **not** make: that a scheduled action will run, and that an action
//! recorded as run actually ran. The second is a schedule's own assertion and this adapter reads it
//! rather than verifying it. A schedule that lies about firedness is outside what any code here can
//! detect, and `docs/LIMITS.md` is where that belongs if this adapter is ever shipped to a surface.
//!
//! # 🔴 No clock (INV-WM4a-2)
//!
//! 41 §6: "randomness and time are injected at the engine boundary (for deterministic replay)", and
//! `gx-substrate`'s own crate documentation adds that nothing in the seven signatures hands an
//! adapter a moment. **This crate names no clock**: not `SystemTime`, not `Instant`, not a `now`.
//! The consequence is exact and it is the reason rather than an accident -- an adapter that branched
//! on the current moment would answer differently on two runs of the same `(intent, snapshot)` pair,
//! which is L1, and a replay of the journal would not reach the state the journal records.
//!
//! So [`Entry::fire_at`](entry::Entry) is **carried and never branched on**. It is the substrate's
//! own datum, meaningful to the runner; this adapter treats it as opaque, exactly as the core treats
//! a payload. `tests/wm4a_time_substrate.rs` holds that as a source scan.
//!
//! # 🔴 The window closes by itself: firedness is inside the fingerprint
//!
//! The interesting question about a scheduled effect is when its undo stops meaning anything: once
//! the action has run, deleting the entry no longer un-runs it. A first design gave `invert` a
//! verdict that expired. That design was wrong for a structural reason, and the reason is worth
//! keeping here: **43 T-10b constructs the inverse *before* `apply`**, so at the moment `invert` is
//! asked, the effect being escrowed has not happened at all and cannot have fired. Nothing an
//! adapter can observe at escrow time is about the firing of the effect it is escrowing.
//!
//! What closes the window is a mechanism that was already in the system. A schedule records
//! firedness *in the entry*; this adapter digests **the whole entry**; so when the runner marks an
//! entry fired, the position's digest changes, and the compare-and-set the engine performs before
//! commit no longer matches the fingerprint the escrow was taken against. **An undo attempted after
//! the entry fired is refused by the CAS check, with no clock, no new law, and no new state.**
//! `tests/wm4a_time_substrate.rs` demonstrates it end to end: fingerprint, fire the entry the way a
//! runner would, fingerprint again, and the two do not compare equal.
//!
//! This is why the adapter's three-valued answer is about **whether the schedule records firedness
//! at all**, and not about the value of that record:
//!
//! | the entry at the position | verdict | why |
//! |---|---|---|
//! | records firedness (`fired` present) | [`Reversibility::True`](gx_core::Reversibility) | the inverse restores the bytes, *and* the record makes a later firing visible to the CAS |
//! | does not record firedness | `Unknown` | gx cannot tell whether restoring these bytes would restore the world, and never will |
//! | inverse over the escrow ceiling | `False` | no inverse is carried (the ceiling reason `gx-adapter-fs` already has) |
//!
//! The `Unknown` row is the one no other adapter in this workspace reaches from its own source:
//! `gx-adapter-fs` states that `Unknown` is unreachable for it, and `gx-adapter-mcp` reaches it only
//! through a deployment posture. Here it is a property of the substrate -- some schedules record
//! whether a job ran and some do not -- and collapsing it to `False` would be this crate reporting a
//! measurement nobody took.
//!
//! # 🔴 gx is never the author of firedness (INV-WM4a-1)
//!
//! [`plan`](plan::plan) refuses an intent whose entry says `fired: true`. gx may place an entry that
//! has not run (which is true by construction, since it did not exist a moment ago) and may cancel
//! one; it may not write the assertion *"this already ran"*, because that is a claim about the world
//! that gx did not observe. The refusal is one branch and one test, and it is what makes the
//! firedness in a digest a fact the runner owns rather than a field two parties write.
//!
//! # What is not here
//!
//! * **No surface.** No CLI verb, no HTTP member, no feature in `gx-cli`. `req/1038` §3-3 keeps the
//!   exposure for a use case that exists, on the precedent of `req/1010` §8-2.
//! * **No sandbox.** Like `gx-adapter-fs`, and with the same disclosure: a position is an absolute
//!   path, and any absolute path this process can write is one this adapter will write.
//! * **No multi-entry delta.** One delta is one entry. A schedule-wide change is several
//!   transformations, which is the shape that keeps each one's inverse the size of one entry.
#![forbid(unsafe_code)]

pub mod adapter;
pub mod apply;
pub mod commutation;
pub mod entry;
pub mod invert;
pub mod locator;
pub mod plan;

pub use adapter::TimeAdapter;
pub use entry::{Entry, TimeOp, MAX_FORWARD_PAYLOAD_BYTES};
pub use locator::normalize;
