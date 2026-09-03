// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The seam. This is the **only** module in the terminal face that spells a medium.
//!
//! `req/942` §11-4 gate g5 measures the claim: no file under `src/tui/` other than this one may
//! name `Constraint`, `Rect`, `Direction`, `Layout` or `ratatui::layout`. Everything above this
//! module names a [`super::layout::RegionRole`] and a [`super::layout::Priority`] and nothing else,
//! which is what makes the drop set a value the screen is handed rather than a branch it hides.
//!
//! # What this module owes, and what it cannot owe
//!
//! An adapter carries `invert` as a contract. A renderer cannot: drawing is a quotient and there is
//! no way back from the picture to what was projected. So the debt this module carries instead is
//! **disclosure** — and the disclosure is composed in `super::layout`, above the seam, precisely so
//! that it is written in role names rather than in cell counts.
//!
//! # Capability tiers, and why they never touch a mark
//!
//! [`Tier`] changes emphasis and nothing else. The seven marks for nothing are drawn with no
//! foreground and no background in every tier, so `mono` is a full-strength tier rather than a
//! degradation, and the pairwise-distinctness gate (P2) holds by construction rather than by luck.

use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};

use super::acts::{self, Act, Signal, View};
use super::layout::{self, Measured, Plan, RegionRole};
use super::live::{self, Subscription};
use super::tokens::{self, Role};
use super::wire::{self, Nothing, Reading, Screen};

/// How much colour the terminal can carry.
///
/// 🔴 Declared in `super::tokens` since `req/38` SS965 convert row (b) — a tier is the reader's
/// terminal, and the value a token takes in it belongs beside the token table rather than beside
/// the code that types escape sequences. Re-exported so that `renderer::Tier` is still the road.
pub use super::tokens::Tier;

/// How long the loop waits for a key before asking the subscription what has happened.
///
/// 🔴 Shorter than [`super::live::DEBOUNCE`] on purpose: the wake is what *notices* that the
/// debounce has expired, so a wake longer than it would make the debounce the wake's period instead
/// of its own. It is also the longest a keypress can sit unread, which is why it is a fifth of a
/// second rather than a comfortable one.
pub const WAKE: std::time::Duration = std::time::Duration::from_millis(200);

/// A role, in the medium's own type. **The one place in this face where a colour is spelled**, and
/// the numbers are not here either: they are in `super::tokens`, and this function only types them.
///
/// 🔴 Gate g13 measures the difference. A literal colour on a line in this file — `Color::Rgb(214,
/// 188, 106)`, which is what the first build wrote — is refused; a colour read out of an [`Ink`] is
/// not. That is the whole of what "the renderer binds the medium and does not decide the value"
/// means, made mechanical.
///
/// [`Ink`]: super::tokens::Ink
fn paint(role: Role, tier: Tier) -> Style {
    let ink = tokens::ink(role, tier);
    let mut style = Style::new();
    if let Some((red, green, blue)) = ink.rgb {
        style = style.fg(Color::Rgb(red, green, blue));
    }
    if let Some(index) = ink.c256.or(ink.c16) {
        style = style.fg(Color::Indexed(index));
    }
    if ink.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if ink.dim {
        style = style.add_modifier(Modifier::DIM);
    }
    if ink.reversed {
        style = style.add_modifier(Modifier::REVERSED);
    }
    style
}

/// Summarise what the four readings measured, for the provenance region.
///
/// The subscription is [`live::LinkReport::off`], which is the truth for every caller that draws one
/// frame and leaves: `gx tui --dump` opens no stream. A run that does subscribe uses
/// [`measured_with_link`].
#[must_use]
pub fn measured(screen: &Screen) -> Measured {
    measured_with_link(screen, live::LinkReport::off())
}

/// The same, for a run that is subscribed to the engine's events.
#[must_use]
pub fn measured_with_link(screen: &Screen, link: live::LinkReport) -> Measured {
    let readings = screen.readings();
    let worst_ms = readings.iter().map(|r| r.elapsed_ms).max().unwrap_or(0);
    let read_at = readings
        .iter()
        .map(|r| seconds(&r.read_at))
        .max()
        .unwrap_or_default();
    let codes: Vec<String> = readings
        .iter()
        .map(|r| r.status.map_or_else(|| "-".to_string(), |s| s.to_string()))
        .collect();
    let statuses = if codes.iter().all(|code| code == &codes[0]) && codes[0] != "-" {
        format!("all {}", codes[0])
    } else {
        format!("status {}", codes.join("/"))
    };
    // 🔴 The bool beside the sentence (`req/924` §TUI-29). `statuses` is composed for a reader and
    // may be reworded; the decision it drives one module up must not move when it is.
    let all_200 = !codes.is_empty() && codes.iter().all(|code| code == "200");
    let mut engine = engine_line(&screen.healthz, true);
    // 🔴 Measured where the fold is decided and not re-derived from the vector it produced
    // (`layout::Measured::healthy`). [`engine_line`] answers a single pair exactly when both of the
    // engine's claims hold, so the length **is** the decision — and reading a decision out of a
    // length at a call site one module away is how the two come to disagree.
    let healthy = engine.len() == 1;
    // 🔴 **The badge rides the rail exactly when the provenance region is about to stand down**
    // (`req/924` §TUI-29). It is inserted **first** because `layout::heading` keeps from the front
    // and gives up from the end: what no route can re-measure is what has to survive the narrowest
    // rail. `layout::resolve_attended` then reads the rail's own cells, so if the badge did not fit
    // the region stays and says it — the decision is never a second width test.
    //
    // 🔴 Split out of [`live::LIVE_BADGE`] rather than typed again. Owner #227's ruling is that a
    // face saying `LIVE` must say **whose** events it counts, and a second spelling here is exactly
    // how a screen comes to say `LIVE` alone on the day the constant is edited (gate g43).
    if all_200 && link.link == live::Link::Open {
        let (badge, rest) = live::LIVE_BADGE
            .split_once(' ')
            .unwrap_or((live::LIVE_BADGE, ""));
        engine.insert(
            0,
            (badge.to_string(), format!("{rest}, {} events", link.events)),
        );
    }
    Measured {
        routes: readings.len(),
        read_at,
        worst_ms,
        statuses,
        link,
        engine,
        engine_full: engine_line(&screen.healthz, false),
        all_200,
        healthy,
        vacant: vacant_columns(&screen.transformations),
    }
}

/// The engine's own line about itself, key by key, in the order the rail gives them up from.
///
/// 🔴 **The order is the ruling** (`req/924` §TUI-22). `super::layout::heading` drops from the end,
/// so the end is where the least load-bearing key goes. `status_reason` leads when the engine is
/// not `ok` because it is the one fact that explains the rest; `engine_version` is last because it
/// is identity rather than a caveat, and identity is what a reader can go and read again.
///
/// 🔴 **`the rail` in this comment means `the standing row`** (`req/924` §TUI-57;
/// independent audit F-04, 2026-09-02). There is no rail. The fold's *decision* is unchanged and is
/// what `g59` reads; where the unfolded line is drawn changed, and it is now the standing row's
/// caveat clause (`super::layout::Shape::engine_caveat`) plus the hatch.
///
/// 🔴 `status_reason` is **absent** when the engine is `ok`, and that is a decision rather than an
/// omission. This bed answers `ok` and `status_reason: null`, so the face drew `status_reason ?` on
/// every frame — `?` meaning *measured, and not knowable*. An engine that is `ok` has no reason, so
/// the true mark is `--` (Absent), and `req/924` §TUI-22 raised that as an engine-side collapse of
/// the seven words for nothing. A face is not free to assert `--` on the engine's behalf, so it
/// does the one honest thing left: it does not spell a value it cannot vouch for, and
/// [`help_lines`] names the key and the condition it is drawn under.
///
/// 🔴 **The paragraph above named the wrong culprit.** `req/924` §TUI-39 (SS1069) measured the live
/// engine: it never once sends the string `"?"`; it sends `null`, correctly, exactly as this
/// paragraph says it should. What could not be vouched for was drawn wrong regardless — `wire::cell`
/// read that `null` as [`Nothing::Unknown`] (the mark this paragraph is refusing), not
/// [`Nothing::Absent`] (the mark it wants). `wire::status_reason` (`req/924` §TUI-39's repair) is
/// the carve-out this key now reads through below, so the assertion this paragraph makes is one the
/// classifier now keeps. The row this function decides to hide when the engine is `ok` is a
/// **separate**, still-standing decision (`g59`) that this repair does not touch.
///
/// 🔴 **superseded → `req/924` §TUI-39 追記 (SS1075), and the two paragraphs above are kept whole**
/// (`req/38` SS1098 裁定1 #2; repaired by `[T-r55]`, 2026-09-02). Two sentences above have been
/// overtaken and neither is deleted, because a reader who cannot see what this face used to believe
/// cannot see how it came to believe this:
///
/// * *"the face does the one honest thing left: it does not spell a value it cannot vouch for"* —
///   **no longer true of this key.** `wire::status_reason` **does** spell `--` for it. What made
///   that honest is not that the face grew a right to assert on the engine's behalf; it is that
///   `crates/gx-api/src/handlers.rs`'s `healthz` writes `null` here for **exactly one fact**, so the
///   mark is read off the engine's contract rather than asserted over it.
/// * *"`Value::Null` → `Nothing::Absent`"* as SS1069 ruled it — **withdrawn as a general rule.**
///   SS1075: `null` means different things on different keys, and a face that spelled `--`
///   everywhere would assert that `created_at` does not exist on a row whose creation time the
///   engine plainly has. The general rule stays `Unknown` (`wire::cell`); the carve-outs are named
///   one key at a time, and there are now three (`wire::inverse_status`, `wire::status_reason`,
///   `wire::receipt_cell`).
///
/// The sentence SS1069 got right and SS1075 left standing: `?` means *this face measured and could
/// not know*, and a face that draws it for something it was **told** is asserting a failure that
/// never happened.
///
/// 🔴 **`engine ok`, and the four words it replaces are not deleted** (`req/924` §TUI-29, `req/38`
/// SS1058, Owner `#268-T`; repeated as §TUI-45 row 3). A caveat that holds on every normal frame is
/// furniture: `status ok`, `ledger_agrees yes` and `engine_version 0.1.0` change no reader's next
/// act while the engine is healthy, and they spend the cells the *unhealthy* frame will need. So the
/// standing row is one word, and the moment either claim stops holding the row expands to the full
/// spelling with `status_reason` in front of it.
///
/// 🔴 **This is not the defect `SS842` names.** Nothing is discarded — the words come back the
/// instant they carry information, and [`help_lines`] spells all three, and the condition the rail
/// draws them under, on every frame. `§TUI-29`'s own test is the one applied: *does this row change
/// the reader's next act when everything is normal?*
///
/// `fold` is the caller's, because the rail and the hatch want different answers to the same
/// question and neither of them should be re-deriving the other's: the rail asks for the line as it
/// is drawn, and [`help_lines`] asks for it as it stands underneath.
fn engine_line(healthz: &Reading, fold: bool) -> Vec<(String, String)> {
    let body = healthz.body.clone().unwrap_or(serde_json::Value::Null);
    let read = |key: &str| -> String {
        healthz
            .nothing()
            .map_or_else(
                || {
                    if key == wire::STATUS_REASON_KEY {
                        wire::status_reason(&body)
                    } else {
                        wire::cell(&body, key)
                    }
                },
                wire::Cell::Nothing,
            )
            .text()
    };
    if fold
        && healthz.nothing().is_none()
        && read("status") == HEALTHY
        && read("ledger_agrees") == AGREES
    {
        return vec![(ENGINE_LABEL.to_string(), HEALTHY.to_string())];
    }
    let mut line: Vec<(String, String)> = Vec::new();
    if healthz.nothing().is_some() || read("status") != HEALTHY {
        line.push(("status_reason".to_string(), read("status_reason")));
    }
    for key in RAIL_KEYS {
        line.push((key.to_string(), read(key)));
    }
    line
}

/// RFC 3339 trimmed to the second: the nanoseconds are precision the screen cannot spend.
fn seconds(at: &str) -> String {
    match at.split_once('.') {
        Some((head, _)) => format!("{head}Z"),
        None => at.to_string(),
    }
}

/// Draw one frame.
///
/// 🔴 `held` is a **sixth parameter and not a fifth member of [`Screen`]**, for the reason
/// [`wire::Held`] gives: `Screen` is what one frame of the ledger is, and this is what one row of it
/// says about itself. [`wire::Held::none`] is the frame where no record is open, which is most of
/// them.
pub fn draw(
    frame: &mut Frame,
    screen: &Screen,
    held: &wire::Held,
    plan: &Plan,
    tier: Tier,
    view: &View,
) {
    let constraints: Vec<Constraint> = plan
        .rows
        .iter()
        .map(|(_, rows)| Constraint::Length(*rows))
        .collect();
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(frame.area());
    for (index, (role, _)) in plan.rows.iter().enumerate() {
        let area = areas[index];
        match role {
            RegionRole::Apparatus => apparatus(frame, area, plan, tier),
            RegionRole::Subject => {
                subject(frame, area, &screen.transformations, held, plan, tier, view);
            }
            RegionRole::Provenance => {
                frame.render_widget(
                    Paragraph::new(Line::raw(plan.provenance.clone()))
                        .style(paint(Role::Quiet, tier)),
                    area,
                );
            }
            RegionRole::Disclosure => {
                // 🔴 **The one standing row** (`req/924` §TUI-57, `req/38` SS1088, Owner `#282-T`).
                // Where the reader is standing, the keys, the connection's dot and what is not on
                // the screen — four lines on one row, and the whole of this face's fixed chrome.
                //
                // Cells rather than one string because the dot carries a paint role of its own and
                // there are six of them ([`super::live::LinkReport::dot`]); a role cannot travel
                // inside a `String`. The plan composed all three parts against one budget, so this
                // region decides nothing, which is the rule the whole of `super::layout` keeps.
                //
                // 🔴 The caveat is pushed to the right edge and the note holds the left, which is
                // the Owner's sketch. It is done by padding rather than by two widgets so that a
                // row too narrow for both closes up instead of overlapping — the plan already
                // marked that row cut.
                let mut cells: Vec<(String, Role)> = Vec::new();
                if plan.truncated {
                    cells.push(("!".to_string(), Role::Quiet));
                }
                let spent: usize = plan
                    .status
                    .iter()
                    .map(|cell| cell.text.chars().count())
                    .sum::<usize>()
                    + plan.status.len().saturating_sub(1)
                    + usize::from(plan.truncated) * 2;
                let last = plan.status.len().saturating_sub(1);
                for (index, cell) in plan.status.iter().enumerate() {
                    // The gap cell brings a separator of its own, so it is one cell shorter than
                    // the room that is left — the arithmetic the enclosure's corner used, one row
                    // down and without the corner.
                    if index == last && plan.status.len() > 1 && spent + 1 < area.width as usize {
                        cells.push((" ".repeat(area.width as usize - spent - 1), Role::Quiet));
                    }
                    cells.push((cell.text.clone(), cell.role));
                }
                frame.render_widget(
                    Paragraph::new(vec![Line::from(spans(cells.into_iter(), tier))]),
                    area,
                );
            }
        }
    }
}

/// The keys the engine's own line still carries on the top rail.
///
/// 🔴 **Three, and it was five** (`req/924` §TUI-22, `req/38` SS1049, Owner `#266-T`).
///
/// * `journal_rows` is gone: it is the ledger's length, and the note already says
///   `N of M` against the very rows the reader is looking at. Two spellings of one number.
/// * `status_reason` is gone **from the standing row** and is drawn exactly when the engine is not
///   `ok`. On this bed it was drawn on every frame as `status_reason ?`, which
///   is the face repeating an engine-side collapse of the seven words for nothing: `?` is
///   *measured and not knowable*, and an engine that is `ok` has no reason, so the truth is `--`.
///   The face is not free to assert `--` on the engine's behalf, so it does the one honest thing
///   available to it — it does not draw a value it cannot vouch for, and [`help_lines`] names the
///   key and the condition it is drawn under. It is **not** in the disclosure's field count: that
///   count is `LEDGER_COLUMNS + LEDGER_PAGE_KEYS`, which is the transformations route, and
///   `status_reason` is a key of `GET /v1/healthz`. Adding it there would have made the count
///   describe two routes as though they were one.
///
/// 🔴 **superseded → `req/924` §TUI-39 追記 (SS1075), and this is the sibling site the gate could
/// not see** (`req/38` SS1098 裁定1 #4; repaired by `[T-r55]`, 2026-09-02). The bullet above states
/// the same overtaken claim as [`engine_line`]'s doc and, unlike it, carried no correction of its
/// own — `g70` pairs a block with a ruling only when the block **quotes the anchor**, so a paragraph
/// that asserts and cites nothing is structurally invisible to it. *The block that forgot to write
/// its correction is the block the gate cannot find*, which is the opposite of the selection anyone
/// wants.
///
/// What is overtaken: **the face does draw a value here.** `wire::status_reason` spells `--`, and
/// what makes that honest is not a right to assert on the engine's behalf but that
/// `crates/gx-api/src/handlers.rs`'s `healthz` writes `null` for exactly one fact. What still
/// stands: `status_reason` is off the standing row while the engine is `ok`, which is `g59`'s
/// separate decision and is what this bullet is really about.
///
/// What stays is what `req/924` §TUI-22 classified as a caveat rather than persuasion: the two
/// claims the engine makes about its own health, which may be folded and may not be discarded.
/// 🔴 The order is the order the rail gives them up in, last first: `engine_version` is identity
/// and identity is re-readable from `GET /v1/healthz`, so it is the first to go; the two claims
/// §TUI-22 called caveats are the last.
const RAIL_KEYS: [&str; 3] = ["status", "ledger_agrees", "engine_version"];

/// The one value of `status` that means the engine has no reason to give.
///
/// 🔴 Declared rather than typed into the condition, because it is the word the whole of the
/// `status_reason` ruling turns on and a gate has to be able to read it (`g59`).
const HEALTHY: &str = "ok";

/// The one value of `ledger_agrees` that means the engine's second claim about itself holds.
///
/// 🔴 Declared beside [`HEALTHY`] and not typed into the condition, for the same reason: it is the
/// second of the two words the fold turns on, and `wire::cell` is what decides that a JSON `true`
/// reads `yes` — so a gate has to be able to read the word this face compares against rather than
/// re-deriving it.
const AGREES: &str = "yes";

/// The label the folded engine line wears.
///
/// 🔴 A word of this face's own and not a wire key, which is why it is declared rather than typed:
/// `engine ok` is a **claim about two keys**, and spelling it with one of their names would say
/// the face is drawing `status` when it is drawing the conjunction.
const ENGINE_LABEL: &str = "engine";

/// The engine's health, and the page's address, on one row inside the ledger's enclosure.
///
/// 🔴 **One row, and it was three** (`req/924` §TUI-22). The row that spelled
/// `GET /v1/transformations` on its own is gone — the address was the rail's **title**, read out of
/// [`layout::heading`], and this was the one row on the screen that spelled it. The row that spelled
/// `status_reason ?` on a healthy engine is gone for the reason [`RAIL_KEYS`] gives.
///
/// 🔴 **Nought rows, and this function is now unreachable** (`req/924` §TUI-57, `req/38`
/// SS1088, Owner `#282-T`; the staleness of the paragraph above found by independent audit F-04,
/// 2026-09-02). The apparatus region is off the standing frame by ruling
/// ([`layout::STOOD_DOWN_REGIONS`]), `layout::resolve_attended` never gives it rows and
/// [`layout::Plan::heading`] is empty at every shape — so the loop below iterates nothing and the
/// paragraph above describes a screen that is not drawn. It is kept rather than deleted (no-delete,
/// and the ladder it draws is the answer the day a rail comes back), and this is where that is said
/// out loud. The address is behind `?`, which gates `g40`, `g75` and `g76` measure.
fn apparatus(frame: &mut Frame, area: Rect, plan: &Plan, tier: Tier) {
    // 🔴 The region draws what the plan composed and decides nothing, which is the rule the whole
    // of `super::layout` exists to keep. The ladder that chooses between the address, the page's
    // name and how many of the engine's keys fit is in `layout::heading`, beside the clause that
    // discloses what it dropped -- so the row that cuts and the row that says so are one decision.
    let mut cells: Vec<(String, Role)> = Vec::new();
    if plan.framed {
        cells.push((tokens::CORNERS[0].to_string(), Role::Quiet));
    }
    for cell in &plan.heading {
        cells.push((cell.text.clone(), cell.role));
    }
    let mut line = Line::from(spans(cells.into_iter(), tier));
    // The closing corner, in the cells the rail did not spend. `layout::heading` chose its rung
    // against `width - layout::FRAME_MARGIN`, so those cells exist by construction wherever
    // `plan.framed` is set; the check is kept because a corner drawn at one end of a rail and cut
    // off the other is worse than no corner at all.
    if plan.framed {
        let used: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        let corner = tokens::CORNERS[1];
        let room = area.width as usize;
        if used + 1 + corner.chars().count() <= room {
            line.spans
                .push(Span::raw(" ".repeat(room - used - corner.chars().count())));
            line.spans
                .push(Span::styled(corner, paint(Role::Quiet, tier)));
        }
    }
    frame.render_widget(Paragraph::new(vec![line]), area);
}

/// The text and paint role one cell carries, before it is padded to its column's width.
///
/// 🔴 Factored out of the row loop rather than duplicated: this is the exact classification that
/// loop used to inline, unchanged, so pulling it out changes nothing about what a row draws and is
/// what lets [`hoist`] ask the same question of a column without a second copy of the verdict's
/// special case drifting from the first.
/// 🔴 Takes `&str` rather than `&'static str` since `req/924` §TUI-13 追記, so that the opened
/// record — which walks the keys the wire actually sent, and owns them — asks this same question
/// instead of reaching past it into [`wire::cell`]. Two classifiers for one column is how the grid
/// and the record came to disagree about `inverse_status`.
/// 🔴 `pub` since gate g58, and for the reason SS855 gives: a gate that has to ask "would this cell
/// have said the same thing on every record?" must ask **this** classifier. A second copy written
/// inside the test would measure the test's understanding of the rule instead of the rule.
/// The mark a judged field takes when the wire carried no answer for it, read from the row's own
/// `state` where the lifecycle entails one and left as it arrived where it does not.
///
/// 🔴 **Two rows, one wire value, two facts** (`req/924` §TUI-101, SS1145, 2026-09-02). A
/// `Candidate` and a terminal `{"Aborted":"Expired"}` both send `verdict: null` and
/// `inverse_status: null`, and before this the face drew the **same symbol pair** on both — while
/// §TUI-97 had already ruled that those two nothings must be visibly different. The separating
/// fact was on the row the whole time: `state` is non-null on every record of the measured bed.
///
/// 🔴 **The failure this repairs is the one `INHERITED_PRINCIPLES` names first** in the *nothing
/// vertical*: a `Candidate`'s missing verdict is *not yet* and was drawn as *measured and not
/// knowable*, and its missing inverse is *not yet* and was drawn as *never written*. A "not yet"
/// that comes from time must not be drawn as a semantic "never".
///
/// The declaration and the whole of the reasoning — including why only two states and only two
/// fields are read this way, and why no eighth mark is minted — live beside the table in
/// [`wire::NULL_MEANING_BY_STATE`]. Gate `g98` is what holds the table to the engine's own
/// `LIFECYCLE_STATES` and to what this function actually draws.
fn state_nothing(item: &serde_json::Value, fallback: wire::Nothing) -> (String, Role) {
    let nothing = wire::null_meaning(item).unwrap_or(fallback);
    (nothing.mark().to_string(), nothing.role())
}

#[must_use]
pub fn cell_mark(item: &serde_json::Value, key: &str) -> (String, Role) {
    if key == wire::VERDICT_KEY {
        let verdict = wire::verdict(item);
        match verdict {
            // 🔴 **The kind of nothing is read from the row's `state`, not from the key's own
            // `null`** (`[T-r82]`, 2026-09-02; `req/924` §TUI-101). `wire::NULL_MEANING_BY_STATE`
            // carries the whole ruling and the reason no eighth mark is minted for it.
            wire::VerdictMark::None(fallback) => state_nothing(item, fallback),
            _ => (verdict.mark(), verdict.role()),
        }
    } else if key == wire::INVERSE_STATUS_KEY {
        let inverse = wire::inverse_status(item);
        match inverse {
            wire::InverseMark::Nothing(fallback) => state_nothing(item, fallback),
            _ => (inverse.mark(), inverse.role()),
        }
    } else if key == wire::STATE_KEY {
        // 🔴 **The third classifier, and it is here for the reason the second is** (`[T-r76]`,
        // 2026-09-02). `state` carries two wire types in one column — thirty-one strings and one
        // single-key object on the Owner's engine — and `wire::cell`'s general rule turns the
        // object into its own JSON, so a thirteen-cell column read `{"Aborted":"~`: the whole
        // column spent on punctuation and `Expired` lost. `wire::state` spells the tag and its
        // member as two words, the same repair `InverseMark::Consumed` already carries.
        let state = wire::state(item);
        (state.mark(), state.role())
    } else {
        match wire::cell(item, key) {
            wire::Cell::Nothing(nothing) => (nothing.mark().to_string(), nothing.role()),
            wire::Cell::Value(text) => (text, Role::Body),
        }
    }
}

/// Which of `columns` the rows on screen agree on, and the window left once a shared row is paid
/// for out of it.
///
/// 🔴 **Crosses the line `req/942` §11 draws between layout and wire.** [`layout::resolve_shared`]
/// only ever sees text; this is the one place that turns a record into the text a cell would show
/// before asking it the question, which is why it lives here and not in `super::layout` —
/// [`layout::Plan`] is built from a width and an item **count** alone and never reaches into a
/// record, so it cannot know two rows agree on a field (`req/984` §10-33).
///
/// A shared row is only ever bought with a row [`layout::window`] would otherwise have spent on a
/// record, and only when a second record is still left to justify spending a row on the line at
/// all. When the window has no spare capacity to give up for free, this recomputes it one row
/// smaller — the *budget* shrinks, and the question is not asked again, because the answer does
/// not depend on which rows survive.
///
/// 🔴 **The viewport is not the domain** (`req/924` §TUI-20, `req/38` SS1047, Owner #264-T). This
/// asked its question of `items[window.first..][..window.rows]` — the records this terminal
/// happened to be tall enough to draw — and then drew the answer as an unquantified line. Measured
/// against a live bed of thirty-one records of which one carries `verdict ?`: with the attention on
/// record 1 the screen hoisted `verdict Admit` and stated it flatly, and with the attention on the
/// last record the same screen drew a verdict column. **The face was asserting over a set it had
/// not measured, and the truth of the assertion was a function of the terminal's height and of
/// where the cursor was standing** — which is the error this product exists to refuse, committed by
/// the product's own face.
///
/// So the domain is every record the read carried. The consequence is real and is the point: on a
/// bed with one record out of thirty-one that differs, **nothing hoists**, the forty-three percent
/// of repeated ink `req/38` SS1019 measured comes back, and the rows are honest. Verbose and true
/// beats readable and false.
///
/// The rejected alternative is recorded rather than left implicit: keep measuring the viewport and
/// qualify the line with `19 drawn of 31`. It is refused because a qualifier does not stop the
/// *column set* from being a function of the cursor position, and a table whose shape changes as
/// the reader moves through it is a defect on its own terms.
///
/// **Named ceiling**: "every record the read carried" is not "every record the ledger holds". The
/// route is paged and `next_cursor` is in [`layout::LEDGER_PAGE_KEYS`], so a second page could
/// disagree with a line this function calls constant. That is why the caller quantifies the line
/// with the count it was measured over instead of stating it flatly; closing it properly needs the
/// page count, which no route on this face's four returns.
///
/// 🔴 **`capacity` is the row budget, and `window.rows` is not a substitute for it** (independent
/// audit, 2026-09-01: the first cut of this function compared `items.len()` against
/// `window.rows`, which [`layout::window`]'s own body caps at `items.min(capacity)` -- so
/// `window.rows <= items.len()` always, the comparison could never find spare capacity, and every
/// hoist paid for its shared row by dropping a real record even when the region had blank rows
/// going unused below the list). `capacity` is [`layout::Plan::grid_capacity`], the number
/// [`layout::window`] was asked for before the item count capped it, which is the only place that
/// fact still exists.
#[must_use]
/// 🔴 **The window left this function's answer with `req/924` §TUI-57** (`req/38` SS1088, Owner
/// `#282-T`). It used to pay for its shared row by shrinking the window — and the two early-outs
/// that guarded that payment (*the list fills the region exactly*, and *fewer than two records
/// would be left*) existed because a record cut here was a record nothing on the screen named.
/// Neither is the case any more: the shared row is a **content row at the head of the scrolling
/// stream** ([`layout::scrolled`]), and `N of M` is on the standing row at every shape, so the
/// number of records not drawn is always spelled. What is left of the guard is the second half —
/// a grid showing one record over a summary of the ledger is a worse trade than a grid showing two
/// — and that is what `capacity` still buys.
pub fn hoist(
    items: &[&serde_json::Value],
    columns: &[layout::Column],
    capacity: usize,
) -> (Vec<layout::Column>, Vec<(&'static str, String)>) {
    // 🔴 One voice, a quorum of two, and the tally flattened back to the single mark this
    // signature has always carried (`[T-r87]`, 2026-09-02). Kept as its own name because
    // twenty-odd call sites and three gates ask for it, and because *every record agrees* is a
    // question worth having a name for even now that a scheme may ask a looser one.
    let (kept, folded) = hoist_at(items, columns, capacity, 1, 2);
    (
        kept,
        folded
            .into_iter()
            .filter_map(|(key, tally)| tally.into_iter().next().map(|(mark, _)| (key, mark)))
            .collect(),
    )
}

/// The same question, with the two numbers that decide it taken from the placement ladder.
///
/// 🔴 **`[T-r87]`, 2026-09-02.** [`hoist`] asks *does every record agree*, and the Owner's finding
/// is what that rule costs when two records out of thirty-two do not: thirty identical rows stay on
/// the screen and twenty-six of them are the same three words. `super::layout::fold_budget` is the
/// declaration — `fold.voices` and `fold.quorum` — and this is where the grid asks it.
///
/// The domain is unchanged and is the whole of what `req/924` §TUI-20 ruled: **every record the read
/// carried**, never the slice on screen.
#[must_use]
pub fn hoist_folded(
    items: &[&serde_json::Value],
    columns: &[layout::Column],
    capacity: usize,
    width: u16,
) -> (Vec<layout::Column>, Vec<(&'static str, Vec<(String, usize)>)>) {
    let (voices, quorum) = layout::fold_budget(width);
    hoist_at(items, columns, capacity, voices, quorum)
}

/// One implementation, so the two roads above cannot answer differently about the same bed.
#[must_use]
pub fn hoist_at(
    items: &[&serde_json::Value],
    columns: &[layout::Column],
    capacity: usize,
    voices: usize,
    quorum: usize,
) -> (Vec<layout::Column>, Vec<(&'static str, Vec<(String, usize)>)>) {
    // Every record the read carried, and not the slice on screen: the answer is a property of the
    // ledger this face was handed, so it does not move when the terminal is resized or when the
    // reader presses `j`.
    let marks: Vec<Vec<String>> = items
        .iter()
        .map(|item| {
            columns
                .iter()
                .map(|column| cell_mark(item, column.key).0)
                .collect()
        })
        .collect();
    let (kept, shared) = layout::resolve_folded(columns, &marks, voices, quorum);
    if shared.is_empty() {
        return (columns.to_vec(), Vec::new());
    }
    // Two records still have to be drawn under the line — not because the claim needs them (it is
    // measured over all of `items`), but because a grid showing one record plus a summary of the
    // ledger is a worse trade than a grid showing two. The preamble is two rows when this line is
    // taken (the column header and this one), so four is the floor.
    if capacity < 4 {
        return (columns.to_vec(), Vec::new());
    }
    (kept, shared)
}

// 🔴 **The two early-outs `hoist` used to carry, recorded rather than deleted** (`no-delete`).
// They priced a payment that no longer happens, and each is written down so the reason it went is
// legible to whoever wonders where it went:
//
// * `items.len() < capacity` -- *spare capacity*: the region was never going to fill every row it
//   was budgeted, so the shared row spent a row nothing else wanted. There is no such spare now:
//   the shared row is a content row and the stream scrolls.
// * `items.len() == capacity` -- *the list fills the region exactly, so paying with a record row
//   cuts a list that nothing on the screen says was cut* (`[T-r42]`, 2026-09-01, gate `g29` at
//   120x32). Measured then: the frame drew twenty-seven records of twenty-eight, no note, and a
//   disclosure that named the missing **keys** and not the missing **record**. That defect is
//   structurally closed by `req/924` §TUI-57 -- the note is on the standing row, so `N of M` is
//   drawn at every shape and no cut of this kind can be silent again.

/// The quantifier a line hoisted over **every record the read carried** wears.
///
/// 🔴 Declared, because `req/38` SS1047 is a ruling about exactly this word: an unquantified
/// `verdict Admit` was a sentence whose truth was a function of the terminal's height. The two
/// scopes are two constants so that a gate can read which one a line claimed rather than parsing it
/// back out of a sentence.
pub const FETCHED_SCOPE: &str = "all";

/// The quantifier a line compressed over **the rows on this screen** wears.
///
/// 🔴 `req/924` §TUI-45 (`req/38` SS1076, Owner `#275-T`). The rejected alternative is named in the
/// ruling and is refused here by construction: *fold on the window and say `all 31`* is the lie
/// SS1047 killed. Saying `these 23` about twenty-three rows is a sentence about a set whose size the
/// reader can count on the screen in front of them.
pub const WINDOW_SCOPE: &str = "these";

/// The columns of this reading in which every record says one of the seven words for nothing.
///
/// 🔴 **`req/924` §TUI-45's new rule** (`req/38` SS1076, Owner `#275-T`): *a column whose every
/// value is a mark for nothing is a column used to say nothing.* `created_at ?` down twenty-three
/// rows says *measured, and not knowable* twenty-three times, and once is the whole of it — so the
/// column is not drawn and is **counted in the disclosure instead**, which is the half that makes it
/// a disclosure rather than a deletion.
///
/// 🔴 **The boundary is the ruling's and it is narrower than it looks, twice over.**
///
/// First, [`layout::resolve_shared`] answers only for a column every row agrees on, so a column that
/// is `?` on some rows and carries a value on others is untouched (*that is information*), and so is
/// one that is `?` on some rows and `--` on others — two different words for nothing, in one column,
/// is this face telling them apart, which is the distinction the vocabulary exists for.
///
/// 🔴 Second, and this is the half the first cut of this function got wrong: the mark must be one of
/// [`wire::VACANT_MARKS`], **not** one of the seven. `no`, `0`, `-x` and `''` are answers that were
/// obtained, and a ledger whose every record says `enforced no` is making the most actionable
/// statement a face like this can carry. Dropping it and reporting that the column "answered with a
/// mark for nothing" folds *measured false* into *not measured* — the first-principle breach this
/// product exists to refuse, committed by a rule written to save ink. An independent audit of this
/// lane found the first cut doing it; `g64d` and `g64e` are the controls that now hold it.
///
/// 🔴 The **mark travels out with the key**, so the hatch can spell `created_at ?` rather than "a
/// mark for nothing" — otherwise this rule would collapse `?` and `--` and `...` for every column it
/// drops, inside the lane whose other half exists to keep them apart (audit, finding 4).
///
/// **Named ceiling**: fewer than two records answers empty, because [`layout::resolve_shared`] is
/// built on `uniform`, which proves nothing about repetition from a single row. One record whose
/// `created_at` is `?` is a fact about that record; this function is about columns.
///
/// # 🔴 The rule was asked to grow a second case, and the canon refuses it (`[T-r76]`, 2026-09-02)
///
/// A brief reached this lane asking for the all-or-nothing quantifier to be loosened: on a bed
/// measured at 118x29, `created_at`, `actor` and `scope` each carried a value on **one** of
/// thirty-two records, so the columns stood and about forty-four cells of the screen were spent on
/// `?`. The cost is real and it was measured. **The fold is still refused, and by two rulings that
/// name this exact shape**, so it is written down here rather than argued again next quarter:
///
/// * `req/924` §TUI-45 states its own boundary in the same breath as the rule this function
///   implements: 🔴 *一部の行だけが「無」なら落とすな(それは情報)。全値が「無」の時だけ。* One record
///   answering is information, and it is the information a reader came to the column for. `g64b` is
///   the standing control for it and carries the same sentence.
/// * `req/924` §TUI-48 (SS1079) ruled **this state specifically**. The `null` behind those marks is
///   the engine saying *this process does not hold the body*, which is neither `?` nor `--`; the
///   ruling lists *列を落として開示へ合流(§TUI-45 の新規則)* among the **却下した2案**, because a
///   reader then cannot learn the value is fetchable — 🔴 *§TUI-45 の規則自体は生きるが、「全値が無
///   の列」の判定に、この状態を「無」と数えてはならない。* The accepted direction is a **mark of its
///   own** beside the seven, carrying *the body is elsewhere and the address is here*, with `open`
///   as its road — and §TUI-48 puts the spelling of that mark through the Owner (見本 → Owner 裁定
///   → 量産), not through a lane.
///
/// **What was implemented and then reverted**, so the next lane does not re-derive it: *fold when
/// fewer than two records answered, more were silent than answered, and the silent ones all used
/// one word for nothing.* It is a clean quantifier with no number in it, it folds the three columns
/// on the measured bed, and **it is still wrong here** — it cannot tell the state §TUI-48 protects
/// from a column that is genuinely empty, because the vocabulary that would tell them apart is the
/// thing §TUI-48 says is missing. `g96` is the standing measurement of what the columns cost, so
/// the Owner's adjudication has a number in front of it rather than a lane's memory of one.
#[must_use]
pub fn vacant_columns(reading: &Reading) -> Vec<(&'static str, String)> {
    let items = reading.items();
    if items.len() < 2 {
        return Vec::new();
    }
    // Every declared column and not the ones a width chose: vacancy is a property of the reading,
    // and a column dropped for saying nothing is dropped at every terminal size.
    let columns = layout::LEDGER_COLUMNS.to_vec();
    let marks: Vec<Vec<String>> = items
        .iter()
        .map(|item| {
            columns
                .iter()
                .map(|column| cell_mark(item, column.key).0)
                .collect()
        })
        .collect();
    let (_, shared) = layout::resolve_shared(&columns, &marks);
    shared
        .into_iter()
        .filter(|(_, mark)| wire::is_vacant_mark(mark))
        .collect()
}

/// The characters past the common prefix this face keeps beyond the number that separated the rows
/// it was handed.
///
/// 🔴 A **budget for the rows this reading did not carry**, and not a proof about them. The route is
/// paged and `next_cursor` is in [`layout::LEDGER_PAGE_KEYS`], so a second page can hold a row whose
/// id agrees with a drawn one for longer than any row in this reading did. Two base32 symbols is a
/// factor of `32^2 = 1024` on the space the measured separation actually needed. It is a number
/// chosen for what it means; what it buys is measured afterwards and reported, never the other way
/// round.
pub const ID_PREFIX_MARGIN: usize = 2;

/// The fewest characters past the common prefix this face will draw, whatever the measurement says.
///
/// 🔴 **The bound is stated because a reader is owed it.** `gx1:` ids are base32 after the scheme, so
/// six symbols is a space of `32^6 = 1_073_741_824`, and the birthday bound puts an even chance of
/// one collision at roughly **thirty-eight thousand six hundred rows**. Below this the prefix stops
/// being a discriminator a reader can rely on for a ledger of any size worth reading on a terminal.
///
/// 🔴 It is a floor and **not** a target: [`id_cells`] answers the measurement when the measurement
/// is larger, and answers [`None`] — leaving the declared width of
/// [`layout::LEDGER_COLUMNS`]`[0]` alone — when it cannot measure at all.
pub const ID_PREFIX_FLOOR: usize = 6;

/// How many characters **past the prefix every row shares** it takes to tell the rows of this
/// reading apart, or [`None`] for a reading this cannot be measured on.
///
/// # What the characters of that column are for
///
/// 🔴 **They are a discriminator and they are not a name** (`[T-r71]`, 2026-09-02, measured). The
/// column draws a cut prefix of a `gx1:` id and has done since the face existed. Measured against
/// the live engine on both roads this face and its reader have — `gx receipt show` and
/// `GET /v1/transformations/{id}` — **no prefix resolves at any length**: three, six, eleven and
/// **fifty-one** of fifty-two characters all come back `VALIDATION_ERROR` / `422`, and only the
/// whole fifty-two answers `200`. So the characters on that column cannot be carried anywhere by a
/// reader; the only work they do is tell one row on the screen from another, and `↵` is the road to
/// the whole id.
///
/// That is the whole argument for measuring the number rather than typing it: eleven characters were
/// buying separation that three characters bought on a twenty-nine row bed, and the other eight were
/// not buying a name because there is no name to buy.
///
/// # This measures. It does not choose a width.
///
/// 🔴 **The declaration stays a declaration** (`[T-r71]`, and the reason is measured rather than
/// preferred). A first cut of this lane fed this number into [`layout::columns_for_less`] and let it
/// narrow the column per reading. It **cut the column's own header**: `transformation` is fourteen
/// characters, the header row draws that key unchanged (`req/942` §9, gate P5), and three gates went
/// red naming a header they could no longer find. So the binding constraint on that column is the
/// **label**, not the hash — and a width derived from the rows would answer the label's number on
/// every ledger anybody will ever read, which is a constant with machinery around it.
///
/// What is left is the honest arrangement: the width is declared at the label's length, and **this
/// is what a gate holds it to** (`g92`). `INHERITED_PRINCIPLES`: *do not bake a value; and where it
/// must be baked, pair it with the thing that detects that the baking has gone stale.*
///
/// # What is measured
///
/// The **common prefix** every row's cell text shares carries no separation — for a `gx1:` ledger it
/// is the scheme — so it is measured rather than assumed and subtracted. This module names no id
/// scheme: a face that spelled `gx1:` here would be a face that stops working, silently and in the
/// identity column, on the day the engine spells an id another way.
///
/// # When this answers [`None`], and why each case is not a defect
///
/// * **Fewer than two rows.** One row is not a set to tell apart — the same ruling
///   [`layout::resolve_shared`] is built on and [`vacant_columns`] restates.
/// * **A row whose identity cell is a mark for nothing.** A mark carries no separation, and a number
///   measured against one would be measuring the face's own vocabulary.
/// * **No prefix separates the rows.** Two rows with one id is a ledger this face will not paper over
///   with a number.
///
/// # Named ceiling
///
/// The domain is *every record the read carried*, which is [`hoist`]'s domain and carries [`hoist`]'s
/// ceiling word for word: it is not every record the ledger holds. [`ID_PREFIX_MARGIN`] is the budget
/// taken against that ceiling and is declared as a budget.
#[must_use]
pub fn id_separation(reading: &Reading) -> Option<usize> {
    let (common, separating) = id_prefix_facts(reading)?;
    Some(separating - common)
}

/// The two lengths every question about the identity column is answered from: how far the rows
/// **agree**, and the shortest prefix at which no two of them do.
///
/// 🔴 One measurement and two readers. [`id_separation`] and [`id_cells_needed`] each need both
/// numbers, and a second walk of the same rows is how the two come to disagree about a ledger — the
/// shape `layout::row_width` was written to close one module over.
fn id_prefix_facts(reading: &Reading) -> Option<(usize, usize)> {
    let items = reading.items();
    if items.len() < 2 {
        return None;
    }
    let key = layout::LEDGER_COLUMNS[0].key;
    let mut texts: Vec<Vec<char>> = Vec::with_capacity(items.len());
    for item in &items {
        match wire::cell(item, key) {
            wire::Cell::Value(text) => texts.push(text.chars().collect()),
            wire::Cell::Nothing(_) => return None,
        }
    }
    let shortest = texts.iter().map(Vec::len).min()?;
    // The characters every row agrees on carry no separation. Measured, because assuming a scheme is
    // how a face comes to depend on a spelling the engine never promised.
    let common = (0..shortest)
        .take_while(|index| texts.iter().all(|text| text[*index] == texts[0][*index]))
        .count();
    // The shortest prefix at which no two rows agree. `+ 1` on the last character so a ledger whose
    // ids differ only in their final character is still separable rather than silently unmeasurable.
    let longest = texts.iter().map(Vec::len).max()?;
    let separating = (common + 1..=longest).find(|length| {
        let mut seen: Vec<&[char]> = texts
            .iter()
            .map(|text| &text[..(*length).min(text.len())])
            .collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        seen.len() == before
    })?;
    Some((common, separating))
}

/// The cells the identity column would have to spend to carry [`id_separation`]'s answer with its
/// budget on it, or [`None`] where that cannot be measured.
///
/// 🔴 The cut mark is a **cell of the column and not a character of the id**: [`pad`] spends the last
/// cell on `~` whenever it cuts, and here it always cuts — a `gx1:` id is fifty-six characters and
/// this answer is never that wide. A number that forgot the mark would say the column fits a value it
/// draws one character short of.
///
/// Read by `g92`, and by nothing that draws: see [`id_separation`] for why the width is a declaration
/// and this is the thing that keeps the declaration honest.
#[must_use]
pub fn id_cells_needed(reading: &Reading) -> Option<u16> {
    let (common, separating) = id_prefix_facts(reading)?;
    let separation = separating - common;
    u16::try_from(common + (separation + ID_PREFIX_MARGIN).max(ID_PREFIX_FLOOR) + 1).ok()
}

/// Which of `columns` every row **on this screen** already agrees on, so the header can say it once.
///
/// 🔴 **`req/924` §TUI-45 (`req/38` SS1076, Owner `#275-T`), and it does not move `SS1047`.** The
/// two questions are different and the ruling separates them: *may this face **claim** a column is
/// constant?* is answered over the fetched set and always will be ([`hoist`]); *may a column that
/// says one word on every row of this screen say it once?* is answered here, over the window, and
/// carries [`WINDOW_SCOPE`] rather than [`FETCHED_SCOPE`] so the sentence is true of exactly the
/// rows it was measured on. `all 31 verdict Admit` measured on twenty-three rows stays refused.
///
/// 🔴 **Named ceiling, and it is the one SS1047 named.** The column *set* is still a function of
/// where the cursor stands: a window that reaches the one record whose verdict differs is a window
/// whose verdicts disagree, and the header clause disappears. §TUI-45 took that trade knowingly
/// (the ruling names it as a price it is paying); this paragraph is the record that it was taken rather than overlooked.
///
/// A ladder and not a width test, like every other line here: the compression is offered and taken
/// only if the header row can hold it whole. It never returns an empty `kept` — the ledger's id is
/// unique per record, so at least one column always varies — and the early-out is kept anyway,
/// because a grid drawing no columns at all is worse than a grid repeating one word.
#[must_use]
pub fn compress_window(
    items: &[&serde_json::Value],
    columns: &[layout::Column],
    window: layout::Window,
    width: u16,
) -> (Vec<layout::Column>, Vec<(&'static str, String)>) {
    let untouched = (columns.to_vec(), Vec::new());
    let last = (window.first + window.rows).min(items.len());
    if window.first >= last || last - window.first < 2 {
        return untouched;
    }
    let slice = &items[window.first..last];
    let marks: Vec<Vec<String>> = slice
        .iter()
        .map(|item| {
            columns
                .iter()
                .map(|column| cell_mark(item, column.key).0)
                .collect()
        })
        .collect();
    let (kept, shared) = layout::resolve_shared(columns, &marks);
    if shared.is_empty() || kept.is_empty() {
        return untouched;
    }
    if header_width(&kept, &shared, slice.len()) > width as usize {
        return untouched;
    }
    (kept, shared)
}

/// How many cells a grid header carrying `shared` beside `kept` needs.
///
/// 🔴 One function, read by the ladder that decides and by the row that draws, so the row cannot be
/// composed against one width and drawn at another — the defect `layout::FRAME_MARGIN` was declared
/// once to prevent, in the one other place two call sites had to agree about a width.
#[must_use]
pub fn header_width(
    kept: &[layout::Column],
    shared: &[(&'static str, String)],
    rows: usize,
) -> usize {
    let names: usize = kept
        .iter()
        .map(|column| column.width as usize + layout::COLUMN_GAP as usize)
        .sum();
    let scope = tokens::RULE.chars().count()
        + 1
        + WINDOW_SCOPE.chars().count()
        + 1
        + rows.to_string().chars().count()
        + 1;
    let clause: usize = shared
        .iter()
        .map(|(key, mark)| key.chars().count() + mark.chars().count() + 4)
        .sum();
    names + scope + clause
}

/// The command that decides whether a receipt holds, run where the reader is.
///
/// 🔴 **The face draws the proof and does not grade it**, which is the engine's own sentence about
/// its own endpoint: `crates/gx-api/src/handlers.rs`'s `receipt_view` records that it "carries no
/// judgement — no `verified`, no `refuted`, no `inclusion`", because "a server that graded its own
/// receipts would be marking its own paper". A **face** grading them would be worse: it has neither
/// the key nor the tree. So the row beside the receipt says `not here` in words and carries this
/// address, and a `✓` is refused for the reason `req/924` §TUI-30 refuses two-valued marks —
/// *verified*, *not checked* and *failed* are three answers and a tick has room for two.
pub const RECEIPT_VERIFY_ADDRESS: &str = "gx receipt verify";

/// The command that runs the escrowed inverse.
///
/// 🔴 **A road and not an act** (`req/924` §TUI-50). Undo is a write; the write surface is ruled to
/// come after the consent screen, and this face has no consent screen. Saying *there is a road and
/// it is not here* is a **read** — it is the fact `inverse_status Available` already carries, plus
/// the address that fact implies — and refusing to say it would be this face withholding what it
/// knows in order to look disciplined. What is not here is the **key**: `super::acts::ACTS` gains
/// nothing, `act-table.json` does not move, and no legend spells `u`.
pub const UNDO_ADDRESS: &str = "gx undo";

/// The words the receipt rows of an opened record are spelled with.
///
/// 🔴 Declared beside the rows that draw them (`[T-r58]`, 2026-09-02) so that `help_lines` can name
/// them without writing a second spelling. `key_id` is [`wire::RECEIPT_KEY_ID`]; the other two are
/// the row's own labels — `leaf_index` and `tree_size` are two members drawn on **one** row that
/// reads `leaf N of M`, so the word a reader sees is `leaf` and the wire key is not.
pub const RECEIPT_ROW_WORDS: [&str; 3] = [wire::RECEIPT_KEY_ID, "leaf", "root"];


/// The two words the record's own rows are judged with, as opposed to read off the wire.
///
/// 🔴 Declared for the reason [`RECEIPT_ROW_WORDS`] is: `help_lines` names them and
/// [`record_rows`] draws them, and a face where the two are separate string literals is a face whose
/// hatch stops explaining a word the day the row changes it. `agrees` is [`agreement`]'s verdict on
/// two readings this face made; `checked` is the row that says this face did **not** grade the
/// signature.
pub const RECORD_ROW_WORDS: [&str; 2] = ["agrees", "checked"];

/// The word the receipt row uses for *this face has not checked the signature*.
///
/// Declared rather than typed, so a gate can read the word the ruling turns on.
pub const NOT_CHECKED: &str = "not here";

/// The keys the row's own read is compared against the page's row on.
///
/// 🔴 **All four are on both routes.** `crates/gx-api/src/list.rs`'s `row_json` and
/// `handlers.rs`'s `get_transformation` each serialise `state` from `Engine::state`, so the two
/// spellings are one spelling; `verdict` is deliberately **not** here, because the singular route
/// does not carry it and comparing a key one side does not have would report a disagreement that is
/// really an absence.
pub const AGREEMENT_KEYS: [&str; 4] = ["state", "superseded_by", "rollback", "inverse_status"];

/// The order an opened record draws its members in.
///
/// # 🔴 Why this is a declaration and not `map.keys()`
///
/// It **was** `map.keys()`, and that is a road out of this face and into a feature flag it does not
/// declare. `serde_json::Map` is a `BTreeMap` — alphabetical — unless `preserve_order` is on, in
/// which case it is an `IndexMap` and the order is the order the **wire** happened to serialise.
/// Cargo unifies features across a build, and measured on this workspace (`[T-r55]`, 2026-09-02,
/// `cargo tree -e features`):
///
/// * `-p tracefold-tui` — `preserve_order` **absent**. Every gate in `tui/tests/` links this graph.
/// * `-p gx-cli` — `preserve_order` **present**, pulled in transitively. This is the `gx` binary.
///
/// So the record face drew `actor, created_at, enforced, …` for every instrument and
/// `transformation, state, verdict, …` for every reader, and **no gate could see it**, because a
/// gate cannot photograph a screen drawn by a binary it is not linked into. Captured both ways: the
/// alphabetical order is in this lane's own red-first output for `g82`, the wire order is in
/// `req/942_artifacts/tui_r55_2026-09-02/pty/record_120x32.txt`.
///
/// The repair is not to pick one of the two. It is to stop asking a question whose answer is a
/// property of the link: the order is [`layout::LEDGER_COLUMNS`] — **the order the grid gives its
/// columns up in, so the record and the table agree about what matters most** — and then whatever
/// else the row carried, sorted, so a key the engine adds tomorrow lands somewhere stable rather
/// than somewhere the linker decided. Gate `g83` fires it against a row built in a scrambled order.
fn member_order(item: &serde_json::Value) -> Vec<String> {
    let Some(map) = item.as_object() else {
        return Vec::new();
    };
    let mut ordered: Vec<String> = layout::LEDGER_COLUMNS
        .iter()
        .filter(|column| map.contains_key(column.key))
        .map(|column| column.key.to_string())
        .collect();
    let mut rest: Vec<String> = map
        .keys()
        .filter(|key| !ordered.iter().any(|known| known == *key))
        .cloned()
        .collect();
    rest.sort_unstable();
    ordered.append(&mut rest);
    ordered
}

/// Cells cut to a width, with the cut **marked**.
///
/// 🔴 **A terminal clips from the right in silence** (`[T-r58]`, 2026-09-02, defect 5). A `Line`
/// wider than its region is not an error and draws no mark: the row simply stops, mid-word, and the
/// reader has no way to tell a value that ends from a value that ran out. Measured on the real
/// capture at 46x12 — `undo not from this face -- gx undo gx1:2hc4zanmdgh0001` is fifty-two cells
/// and the screen is forty-six, so the address a reader is being told to retype ended six characters
/// early with nothing saying so.
///
/// The mark is [`pad`]'s, which is this face's declared idiom for a value cut to its column, so
/// there is no second vocabulary for *this stopped early*. A cell with no room left at all is
/// dropped and the mark is moved onto the last cell that survived, because a cut nothing marks is
/// the thing this function exists to refuse — a dropped cell is still a cut.
///
/// # 🔴 The ruling when the width cannot pay for the mark (`[T-r66]`, 2026-09-02)
///
/// Two answers were open and they are **both** taken, at different layers, which is why they do not
/// contradict each other:
///
/// * **Inside a declared column, the mark wins and the value gives up a cell.** That is [`pad`]:
///   `2026-08-30T09:00:00Z` in a nineteen-cell column is `2026-08-30T09:00:0~`. The column's width
///   is a budget the value was told about, so spending one of its cells on saying *there is more*
///   costs the reader a character and buys them the knowledge that characters are missing.
///
/// * **At the edge of the row, the cell is given up and the giving-up is named.** There is no
///   declared budget out here to spend a cell of, so shaving a digit off the last value to make room
///   for a `~` would produce a loss that is **anonymous**: the reader sees that something stopped
///   but not which column. Dropping the column instead routes the loss into
///   [`super::layout::Plan::dropped_fields`], which the disclosure line counts and names, and the
///   mark moves onto the last cell that did survive — so the row still says it was cut *and* the
///   screen still says which field went. A named loss beats an unnamed one.
///
/// **What is forbidden in both cases is dropping the mark itself.** A cell with `room <= 1` is not
/// drawn at half width in silence: it is dropped, and `~` lands on the previous cell — or, when
/// there is no previous cell, `~` is the whole row. A row one cell wide can still say that it is not
/// the whole story. That is the only rule here with no exception: the face may lose a column, it may
/// lose a character, it may not lose the fact that it lost something.
fn fit(cells: &[(String, Role)], width: u16, gap: u16) -> Vec<(String, Role)> {
    let width = width as usize;
    let gap = gap as usize;
    let mut out: Vec<(String, Role)> = Vec::new();
    let mut used = 0usize;
    let mut marked = false;
    let mut dropped = 0usize;
    for (text, role) in cells {
        let lead = if out.is_empty() { 0 } else { gap };
        let room = width.saturating_sub(used + lead);
        let len = text.chars().count();
        // 🔴 An empty cell has nothing to lose, so it is not a cut (independent audit, 2026-09-02,
        // M2). Without this, a zero-length cell arriving with no room left wrote a `~` onto the
        // previous cell — a mark for a cut of nothing, which is the same class of lie as `--` for a
        // fact nobody measured.
        if len == 0 {
            continue;
        }
        if room > 0 && len <= room {
            used += lead + len;
            out.push((text.clone(), *role));
            continue;
        }
        if room > 1 {
            out.push((pad(text, u16::try_from(room).unwrap_or(u16::MAX)), *role));
            marked = true;
        } else {
            dropped += 1;
        }
        break;
    }
    if dropped > 0 && !marked {
        // Every cell that fit, fit — and one did not get on the row at all. The mark goes on the
        // last character there is room for, so the row still says it stopped early.
        //
        // 🔴 **And when there is no last cell, the mark is the whole row** (independent audit,
        // 2026-09-02, S3). The recovery block used to be `if let Some(..) = out.last_mut()`, so the
        // one case where the **first** cell did not fit and there was no room to cut it into — a
        // row with `room <= 1` at the front — dropped the row entirely with **no mark at all**:
        // exactly the failure this function exists to refuse, inside the branch written to refuse
        // it. A row that is one cell wide can still say `~`.
        if let Some((text, _)) = out.last_mut() {
            let keep: String = text.chars().take(text.chars().count().saturating_sub(1)).collect();
            *text = format!("{keep}~");
        } else if width > 0 {
            out.push(("~".to_string(), Role::Quiet));
        }
    }
    out
}

/// The keys an opened record folds into one head row.
///
/// 🔴 **Read off [`layout::LEDGER_COLUMNS`]'s own priorities, not written down here** (`[T-r58]`,
/// 2026-09-02, defect 1). `Priority::One` is the declaration *never dropped while anything is drawn
/// at all* — the three the grid keeps at forty cells — so the three facts that identify a row are
/// already named, in one place, by the table that draws them. A second list here would be a second
/// answer to *which facts name a record*, and the day a column changes priority the two would
/// disagree.
fn head_keys() -> Vec<&'static str> {
    layout::LEDGER_COLUMNS
        .iter()
        .filter(|column| column.priority == layout::Priority::One)
        .map(|column| column.key)
        .collect()
}

/// An opened record's own rows: the head, the members that carry a value, and the marks, gathered.
///
/// # 🔴 Why this is not seventeen rows of `key value`
///
/// Measured on the real capture (`req/942_artifacts/tui_r55_2026-09-02/pty/record_120x32.txt`): the
/// record drew **seventeen flat rows**, of which **five were the same mark** — `created_at ?`,
/// `scope ?`, `rollback ?`, `superseded_by ?`, `actor ?`. The seat's ruling (`[T-r58]`, 2026-09-02,
/// defects 1 and 2) is that a reader cannot see the shape of a record through seventeen equal rows,
/// and that five rows spelling *this process does not have it* are one fact about five keys rather
/// than five facts.
///
/// So three things happen, and each of them deletes rows rather than adding marks:
///
/// * **the head.** The three [`head_keys`] become one row. Their labels are drawn in `Role::Head`
///   and their values keep the role the cell earned, so `verdict` still arrives in the verdict's own
///   appearance — which the flat rows did not do at all, because they were `Line::raw` and carried
///   **no role**.
/// * **the members that carry a value** keep a row each. They are what the wire said about this row
///   and nothing about them is redundant.
/// * **the marks gather, one row per kind.** `? created_at scope rollback superseded_by actor`.
///
/// # 🔴 What the gathering may not do
///
/// **It may not put two kinds of nothing on one row.** `?` is *measured and not known* and `--` is
/// *never written*; `0` is a count and `''` is a value the wire opened and closed. Collapsing them
/// would be the one simplification `INHERITED_PRINCIPLES` puts outside the reach of every reduction
/// pass — the seven words are the product. The gathering is keyed on [`wire::Nothing`] itself, so a
/// row can only ever join a group that draws the identical mark, and the mark keeps its own paint
/// role. Gate `g86` fires it on a row carrying four different kinds.
///
/// `verdict` and `inverse_status` are never gathered: they have mark vocabularies of their own
/// ([`wire::VerdictMark`], [`wire::InverseMark`]) which [`cell_mark`] resolves and [`wire::cell`]
/// does not, so gathering them would draw one word and mean another.
fn record_own(item: &serde_json::Value, width: u16) -> Vec<(Vec<(String, Role)>, usize)> {
    let heads = head_keys();
    let gap = layout::COLUMN_GAP as usize;
    let mut head: Vec<(String, Role)> = Vec::new();
    let mut head_keys_spelled: usize = 0;
    let mut head_cells: usize = 0;
    // 🔴 **The fold closes on the first key that does not fit, and does not reopen.** Without this
    // the walk kept trying: at sixty-six cells the id is sixty-eight and went to a row of its own,
    // and `verdict` and `state` — which do fit — folded into a head row **above it**. The screen
    // then read `verdict Admit  state Superseded` and named the record on the second line. Measured
    // on the real capture at 66x20, 46x12 and 40x10 before this line existed. The keys are walked in
    // the declared order, so closing the fold is what makes the rows keep it.
    let mut folding = true;
    let mut valued: Vec<(Vec<(String, Role)>, usize)> = Vec::new();
    let mut gathered: Vec<(wire::Nothing, Vec<String>)> = Vec::new();
    for key in member_order(item) {
        let (text, role) = cell_mark(item, &key);
        if heads.contains(&key.as_str()) {
            // 🔴 **The fold is priced, and what does not fit stays a row of its own.** A head row
            // composed without asking the width is a row the terminal clips from the right — and
            // what it clips first is `state`, which is the fact a reader opened the record for. At
            // forty cells the address alone is thirty-four of them, so the fold buys nothing there
            // and is not attempted; at a hundred and twenty it buys two rows. The keys are walked in
            // the declared order and a key that does not fit becomes the first of the rows below, so
            // the order `member_order` settles survives the fold.
            let want = key.chars().count() + gap + text.chars().count();
            let cost = if head.is_empty() { want } else { gap + want };
            if folding && head_cells + cost <= width as usize {
                head_cells += cost;
                head_keys_spelled += 1;
                head.push((key, Role::Head));
                head.push((text, role));
                continue;
            }
            folding = false;
            valued.push((vec![(key, Role::Head), (text, role)], 1));
            continue;
        }
        // 🔴 **One address, two keys, one drawing of it** (`[T-r76]`, 2026-09-02). Measured on the
        // real capture: `inverse_status Consumed gx1:647…rszja` and `superseded_by gx1:647…rszja`
        // stood on adjacent rows, and at 40x10 those two were **two of the nine rows the record
        // had**. `wire::supersede_agrees` is where it is established that the repetition is a
        // repetition — the engine writes both from one binding at 43 T-12 and rebuilds both from
        // one journal field on replay, so the second address cannot say anything the first did not.
        //
        // 🔴 **Neither word is deleted, and the fold is refused when the fact is not established.**
        // `superseded_by` keeps its label; `inverse_status` keeps `Consumed` and keeps the address.
        // A row where the two merely *look* alike — a `superseded_by` that is not a string, or an
        // `inverse_status` that is not `Consumed` — takes neither branch, because folding two facts
        // that are equal by accident is the product's own lie told on its own screen.
        //
        // The sentence is `wire::SUPERSEDE_AGREEMENT` and it is painted `Role::Quiet`, the role
        // this face reserves for what it says about itself rather than what the wire said. A reader
        // who wants the address reads it one row up; a reader who wants to know whether the two can
        // ever differ is told, on the screen, that they do not.
        if key == wire::SUPERSEDED_BY_KEY && wire::supersede_agrees(item) {
            valued.push((
                vec![
                    (key, Role::Head),
                    (wire::SUPERSEDE_AGREEMENT.to_string(), Role::Quiet),
                ],
                1,
            ));
            continue;
        }
        // 🔴 **`state` joins the two keys this branch is not allowed to classify** (`[T-r82]`,
        // 2026-09-02, closing an audit finding of `[T-r76]`). This function's own doc comment gives
        // the rule: a key whose kind of nothing `cell_mark` resolves and `wire::cell` does not may
        // not be gathered, because gathering it draws one word and means another. `[T-r76]` gave
        // `state` exactly such a classifier (`wire::state`) one file over and did not add it here,
        // leaving **one key with two classifiers**.
        //
        // 🔴 **And it changes nothing today, which is measured rather than hoped.** `state` is
        // `Priority::One` in `layout::LEDGER_COLUMNS`, so `head_keys` contains it and the branch
        // above takes every `state` before this line is reached: the second classifier was
        // unreachable, not merely in agreement. That is *why* the disagreement had never shown —
        // and it is also why the guard is worth one line, because the thing standing between the
        // two classifiers is a **priority in a table another lane may change**, not an argument.
        let own_vocabulary = key == wire::VERDICT_KEY
            || key == wire::INVERSE_STATUS_KEY
            || key == wire::STATE_KEY;
        if !own_vocabulary {
            if let wire::Cell::Nothing(nothing) = wire::cell(item, &key) {
                match gathered.iter_mut().find(|(mark, _)| *mark == nothing) {
                    Some((_, keys)) => keys.push(key),
                    None => gathered.push((nothing, vec![key])),
                }
                continue;
            }
        }
        valued.push((vec![(key, Role::Head), (text, role)], 1));
    }
    // 🔴 **Each row carries how many of the wire's keys it spells, and the count is the row's own
    // rather than derived from its shape.** A head row holding three pairs and a gathered row
    // holding three keys are the same length and are not the same number of members; a reader of
    // this vector cannot tell them apart, and the disclosure of a cut counts **keys** — dropping one
    // gathered row can drop four members. Derived at the call site, the two would disagree the day a
    // fourth kind of row exists.
    let mut out: Vec<(Vec<(String, Role)>, usize)> = Vec::new();
    if !head.is_empty() {
        out.push((head, head_keys_spelled));
    }
    out.append(&mut valued);
    for (nothing, keys) in gathered {
        let spelled = keys.len();
        let mut row = vec![(nothing.mark().to_string(), nothing.role())];
        row.extend(keys.into_iter().map(|key| (key, Role::Head)));
        out.push((row, spelled));
    }
    out
}

/// How many rows an opened record's own part and the routes beyond it need.
///
/// 🔴 **The one place they are counted.** `super::layout::resolve_attended` decides the cut and the
/// ledger's capacity from these two numbers and `super::renderer::subject` draws against the plan's
/// answer, so the count the screen was budgeted from and the count it draws are one measurement. The
/// two call sites that resolve a plan with a record held ask this rather than counting again.
///
/// Nought and nought when the reading carries no record, which is when
/// [`layout::subject_shape`] does not answer [`layout::Subject::Record`] at all.
#[must_use]
pub fn record_extent(
    screen: &Screen,
    held: &wire::Held,
    view: &View,
    width: u16,
) -> (usize, usize) {
    let items = screen.transformations.items();
    if items.is_empty() {
        return (0, 0);
    }
    let index = view.selected.min(items.len() - 1);
    (
        record_own(items[index], width).len(),
        record_rows(items[index], held).len(),
    )
}

/// The rows an opened record adds to its own members, each read on a route of its own.
///
/// # What is here, and where each row came from
///
/// * **the refusal**, when [`wire::Held::refusal`] is set — this face declined to put the id in a
///   request line. First, because it is spelled nowhere else.
/// * **`receipt`** — [`wire::Held::receipt_mark`] over `GET /v1/receipts/{tid}`.
/// * **`read again`** — whether `GET /v1/transformations/{id}` agrees with the row the page
///   carried, on [`AGREEMENT_KEYS`]. A **renderer-local fact**, in the sense
///   `super::wire::Reading`'s last four members are: no route answers it, and a second read makes a
///   new comparison rather than recovering this one.
/// * **`key_id` / `leaf` / `root`** — `receipt_view`, the decoded half the engine mounts beside the
///   document so that a reader does not have to carry a DAG-CBOR decoder.
/// * **`checked`** and **`undo`** — two roads, [`RECEIPT_VERIFY_ADDRESS`] and [`UNDO_ADDRESS`].
///
/// # 🔴 What is **not** here, by name
///
/// **The lifecycle chain.** `req/924` §TUI-30's sketch draws
/// `submitted ──▶ planned ──▶ verified ──▶ committed`, and **no route this face may read carries
/// it**: `state` is one word for where the row is *now*, and neither list nor singular route
/// answers *which states it passed through, and when*. Drawing the chain from the vocabulary and
/// marking the current position would put three arrows on the screen for three transitions nobody
/// measured — the face asserting a history the engine never sent, which is the same act as drawing
/// `--` on the engine's behalf and is refused for the same reason. It is dropped, and it is named
/// as dropped in [`help_lines`], because the membrane's second obligation is to say what was let go
/// of rather than to let go of it quietly.
///
/// **`actor`, `scope` and `created_at`.** They are drawn — as the member rows, with whatever mark
/// the wire earned — and nothing here improves them. `crates/gx-api/src/list.rs`'s `row_json` says
/// in its own words that they are `null` "when the row is not on the table, which after a restart is
/// every row", and that inventing a value from Σ "would be the skip/pass conflation `req/29` §4
/// forbids". A face that reached into the receipt to fill them in would be committing the engine's
/// forbidden move on the engine's behalf: `receipt_view`'s `issued_at` is **when the receipt was
/// issued**, which is a different fact from when the transformation was created, and spelling one as
/// the other is how two facts become one.
fn record_rows(item: &serde_json::Value, held: &wire::Held) -> Vec<(String, Role)> {
    let mut rows: Vec<(String, Role)> = Vec::new();
    if let Some(refusal) = held.refusal.as_deref() {
        rows.push((refusal.to_string(), wire::Nothing::Absent.role()));
    }
    let mark = held.receipt_mark();
    // 🔴 **The three members of `receipt_view` this face reads through and does not draw**
    // (`[T-r76]`, 2026-09-02; `[T-r58]` named them and left them open). The membrane's second
    // obligation admits two closures — draw it, or drop it **and say the name** — and this is the
    // second. `wire::RECEIPT_VIEW_NOT_DRAWN` says of each member why it went.
    //
    // 🔴 **It rides the row that introduces the object, and it rides it only when the object is
    // there.** Two other homes were tried and measured on this lane's own engine-backed captures,
    // and both were paid for out of somebody else's disclosure: on the hatch's `record` entry the
    // extra row turned `g36` red at 120x32 (it took the last `Intent::sentence` off the page — the
    // consequence that entry's own comment predicts), and on the hatch's `routes` entry it cost the
    // `beyond` entry at 66x20 and an act's intent at 100x30. `[T-r55]` refused exactly that trade
    // in this file — *displacing an act's intent to explain `key_id` trades a road for a road* —
    // and the refusal is honoured. A third home, the `key_id` row, cost no row but **is not drawn
    // at 40x10**, so the names vanished at the width where the least is on the screen. This row is
    // drawn at every width an opened record is, so the clause is never absent.
    //
    // 🔴 **What it still costs, stated rather than hidden.** The clause is spelled in full at
    // 120x32, 100x30 and 80x24 and is **cut with `~` at the four narrower shapes** — a reader there
    // is told that something was let go of and cannot read which. `w` / `gx tui --wide` does **not**
    // widen it (measured in `g95`, not assumed), so this is a gap and it is written down as one
    // rather than papered over with a road that does not arrive (`req/924` §TUI-21). And three
    // words now stand on the screen that the hatch does not gloss, so
    // `tools/gates/legibility_gate.mjs` — already red on both faces before this lane — counts three
    // more.
    let dropped = if held.view().is_some() {
        format!(
            " | {} {}",
            wire::RECEIPT_VIEW_DROP_PHRASE,
            wire::RECEIPT_VIEW_NOT_DRAWN.join(", ")
        )
    } else {
        // 🔴 Nothing, and deliberately: with no decoded view there is no object whose members could
        // have been let go of, and naming three members of a thing this reading does not have would
        // be a disclosure about a fiction.
        String::new()
    };
    rows.push((format!("receipt {}{dropped}", mark.mark()), mark.role()));
    rows.push(agreement(item, held));
    if let Some(view) = held.view() {
        let read = |key: &str| wire::receipt_cell(view, key).text();
        rows.push((
            format!("{} {}", wire::RECEIPT_KEY_ID, read(wire::RECEIPT_KEY_ID)),
            Role::Body,
        ));
        // 🔴 Two cells on one row and each keeps its own mark: a receipt with no inclusion proof
        // reads `leaf -- of --`, which says *these coordinates were never written* twice rather
        // than saying it once and leaving the reader to decide the second number was the same
        // kind of nothing as the first.
        rows.push((
            format!(
                "{} {} of {}",
                RECEIPT_ROW_WORDS[1],
                read("leaf_index"),
                read("tree_size")
            ),
            Role::Body,
        ));
        rows.push((
            format!("{} {}", RECEIPT_ROW_WORDS[2], read("root")),
            Role::Body,
        ));
    }
    if mark == wire::ReceiptMark::Held {
        rows.push((
            format!(
                "{} {NOT_CHECKED} -- {RECEIPT_VERIFY_ADDRESS}",
                RECORD_ROW_WORDS[1]
            ),
            Role::Quiet,
        ));
    }
    // 🔴 The road is drawn exactly when the engine says there is one. `Available` is 42 §3.12's
    // word for *the escrowed inverse is still there*; on `Consumed`, `Unavailable` or any kind of
    // nothing the member row already says so and an address here would point at a command that
    // refuses.
    if wire::inverse_status(item) == wire::InverseMark::Kind("Available") {
        rows.push((
            format!("undo not from this face -- {UNDO_ADDRESS} {}", held.id),
            Role::Quiet,
        ));
    }
    rows
}

/// Whether the row's own read agrees with the row the page carried.
///
/// 🔴 **A comparison of two measurements this face made, and it is labelled as one.** The page is
/// read at one instant and the row at another, so a disagreement is not an error — it is the row
/// having moved, which is the one question a record face can answer that a list cannot. The keys
/// are [`AGREEMENT_KEYS`] and the disagreeing ones are **named**: a bare `differs` would tell a
/// reader that something changed and refuse to say what.
fn agreement(item: &serde_json::Value, held: &wire::Held) -> (String, Role) {
    // 🔴 **Is this reading about this row?** Asked first, and it was not asked at all until an
    // independent audit found it (`[T-r55]`, 2026-09-02, finding S2). Without it the four keys were
    // compared between whatever the page carried and whatever the singular route answered, with
    // **nothing anywhere checking they name the same transformation** — and on the common row
    // (`state Committed`, `superseded_by null`, `rollback null`, `inverse_status Available`) all
    // four would match, so the face would draw `agrees`: *this record has not moved*, asserted from
    // a comparison that was never made about it. That is `absent is not false` one layer up, in the
    // module whose heading refuses exactly that.
    //
    // The two things compared are the id this face **asked with** and the id the page **drew**.
    // Both are always present and neither depends on the singular route's body shape, which carries
    // `transformation` as a whole object rather than as a string.
    let asked_about = item
        .get(wire::LEDGER_ID_KEY)
        .and_then(serde_json::Value::as_str);
    if held.is_open() && asked_about != Some(held.id.as_str()) {
        return (
            "read again about another row".to_string(),
            wire::Nothing::Unknown.role(),
        );
    }
    if held.refusal.is_some() {
        return (
            format!("read again {}", wire::ReceiptMark::Refused.mark()),
            wire::Nothing::Absent.role(),
        );
    }
    if held.transformation.is_pending() {
        return (
            format!("read again {}", wire::Nothing::Loading.mark()),
            wire::Nothing::Loading.role(),
        );
    }
    let Some(body) = held
        .transformation
        .body
        .as_ref()
        .filter(|_| held.transformation.is_ok())
    else {
        return (
            format!("read again {}", wire::Nothing::Unknown.mark()),
            wire::Nothing::Unknown.role(),
        );
    };
    let moved: Vec<&str> = AGREEMENT_KEYS
        .into_iter()
        .filter(|key| item.get(*key) != body.get(*key))
        .collect();
    if moved.is_empty() {
        (
            format!("read again {}", RECORD_ROW_WORDS[0]),
            Role::Body,
        )
    } else {
        (
            format!("read again moved on {}", moved.join(", ")),
            Role::Body,
        )
    }
}

/// `GET /v1/transformations`, in the columns that fit.
///
/// 🔴 A row is a list of **spans** rather than one string since `req/38` SS965 convert row (b). The
/// difference is not decoration: a cell now carries the role of what it holds — the verdict's three
/// kinds and the fourth mark, and each of the seven kinds of nothing — so the appearance of a cell is
/// a value the token table resolves rather than the absence of a decision.
fn subject(
    frame: &mut Frame,
    area: Rect,
    reading: &Reading,
    held: &wire::Held,
    plan: &Plan,
    tier: Tier,
    view: &View,
) {
    let mut lines: Vec<Line> = Vec::new();
    let items = reading.items();
    // 🔴 The grid's header belongs to the grid. An opened record is a list of members, not a table
    // of columns, and a header standing over it names columns that are not drawn — a signpost
    // pointing down a road that is not there. Measured on a real terminal at 46x12: the row the
    // header took was half of what the record had, `1 of 10 members` where two fit.
    //
    // The empty list keeps its header, and that is not an inconsistency: an empty **grid** is still
    // a grid, and the header is what says which columns found nothing.
    // 🔴 One classifier. `layout::resolve` reads this same function to compose the disclosure, so
    // the line that says what is not on the screen cannot disagree with the region about which
    // shape was drawn (`req/964` §16). A second `if` here would be a second answer.
    let shape = layout::subject_shape(reading, view);
    // 🔴 **The heading is drawn one region up** (`req/924` §TUI-22, `req/38` SS1049,
    // Owner `#266-T`). It still stands over all three shapes and still answers *which screen is
    // this, and which of the three am I on* — the question Owner #227 admitted it for. What
    // changed is where it is charged: it was a whole row taken off the ledger, and the apparatus
    // region was already holding a row it spent on a breadcrumb that repeated the page's address.
    // Putting the two together costs nought rows and returns one record.
    let open = shape == layout::Subject::Record;
    // The header belongs to the **grid** and to nothing else, which is why the test is for the
    // grid rather than against the record. It was `!open`, which reads as "everything that is
    // not an opened record is a table" -- true while there were two shapes and false the moment
    // there were three.
    let grid = shape == layout::Subject::Grid;
    // 🔴 **The columns and window this grid actually draws, not necessarily the plan's**
    // (`req/984` §10-33). `hoist` asks whether the rows on screen already agree on a column and,
    // if they do, spends one row of the window saying it once instead of on every row —
    // 43 percent of the ink at 120x32 was five columns repeating the same five words down every
    // one of twenty-three rows. Asked only where there is a record to ask the question of: an
    // empty grid has no cell to compare, and an opened record is not this branch at all.
    // 🔴 **`hoist_folded`, and it is the one line that lets a declaration answer the Owner's
    // finding** (`[T-r87]`, 2026-09-02). `hoist` asks *does every record agree*; this asks the
    // question `super::layout::fold_budget` declares, and under the shipped default
    // (`super::tokens::SCHEME_DEFAULT`) the two are the same question with the same answer.
    let (columns, shared) = if grid && !items.is_empty() {
        hoist_folded(&items, &plan.columns, plan.grid_capacity, area.width)
    } else {
        (plan.columns.clone(), Vec::new())
    };
    // 🔴 **The preamble, and the scroll over it** (`req/924` §TUI-57, `req/38` SS1088, Owner
    // `#282-T`). The column header and the `all N` clause are derived from the records, so they are
    // **content**: they sit at the head of one scrolling stream rather than being pinned above it.
    // The plan asked [`layout::scrolled`] the same question with `preamble` of one; the shared line
    // is only known here, so this asks it again with the larger number — one function, two callers,
    // no second arithmetic to disagree with the first.
    let preamble = if grid {
        1 + usize::from(!shared.is_empty())
    } else {
        0
    };
    let (preamble_shown, window) = if grid {
        layout::scrolled(
            view.selected,
            items.len(),
            preamble,
            area.height as usize,
            view.glide,
        )
    } else {
        (0, plan.window)
    };
    // 🔴 **`§TUI-45` item 2 is withdrawn, and this is where it stood** (`req/924` §TUI-62 (`req/38` SS1093, Owner `#284-T`, 2026-09-01)).
    // `compress_window` lifted the columns every row **of the window** agreed on into the header.
    // It was measured on the drawn slice, so with the attention anywhere in 1..29 the ledger came
    // back as rows of bare id, and at 31 the columns reappeared -- **the shape of the table was a
    // function of the cursor**, which is the symptom `SS1047` was opened for, rebuilt by the
    // repair that answered `SS1047`'s other half.
    //
    // The ruling: **the viewport is not the domain of the claim and it is not the domain of the
    // *form* either.** Constancy is measured over the fetched set ([`hoist`]) and nowhere else; a
    // column that is not constant there is drawn. `Admit Committed Available` comes back down
    // every row and that price is paid knowingly -- **repetition is readable; a bare id and a
    // table that changes shape are not.**
    //
    // [`compress_window`], [`WINDOW_SCOPE`] and [`header_width`] are kept (no-delete): `SS1047`'s
    // ruling that a claim measured on a window must be quantified `these N` is untouched, and the
    // day a line is measured on a window again those are its vocabulary. They are called by
    // nothing that draws, and this paragraph is where that is said out loud rather than left for
    // an audit to find.
    // 🔴 The preamble is built whole and then **the rows the scroll has moved past are dropped from
    // the front** (`req/924` §TUI-57). Built whole because the shared line's role is read off the
    // first drawn record and that record is not known until the window is; dropped from the front
    // because a stream scrolls upward, and the column header is what leaves first.
    let mut preamble_lines: Vec<Line> = Vec::new();
    if grid {
        // 🔴 **The margin is a cell, so it is charged a gap like every other cell** (`[T-r66]`,
        // 2026-09-02). The sentence that stood here — *the width `columns_for_less` priced them
        // against is `LEFT_MARGIN + sum(width) + (n - 1) * COLUMN_GAP`* — was true of the price and
        // false of the row: `spans_with` puts a gap after the margin too, so the row is
        // `LEFT_MARGIN + n * COLUMN_GAP + sum(width)`, two cells more. The price is
        // [`layout::row_width`] now and it is that second number, so this row and the plan that
        // chose its columns are one arithmetic rather than two that agreed by luck.
        let heads: Vec<(String, Role)> = std::iter::once((margin(), Role::Head))
            .chain(
                columns
                    .iter()
                    .map(|column| (pad(column.key, column.width), Role::Head)),
            )
            .collect();
        preamble_lines.push(Line::from(spans_with(
            // 🔴 `fit`, for the reason the record's ledger slice has it: the arithmetic above is
            // what *should* make this row fit, and this is what makes the row **say so** if it ever
            // stops fitting again. A guarantee held in two files by two numbers agreeing is the
            // shape of the defect that was just repaired.
            fit(&heads, area.width, layout::COLUMN_GAP).into_iter(),
            tier,
            layout::COLUMN_GAP,
        )));
        if !shared.is_empty() {
            // 🔴 One row, standing for every column every row on screen already agreed on
            // (`req/38` SS1019). `hoist` already paid for it out of `window` when there was no
            // spare capacity to take it from instead, so this never costs a row nothing else
            // budgeted. The role beside each mark is read from the first drawn row — every record
            // in `items` agrees by construction since `req/924` §TUI-20 moved the domain there, so
            // any one of them names the same appearance.
            // 🔴 **The role is read off the record that carries the very mark being drawn**
            // (`[T-r87]`, 2026-09-02). This read it off one sample row, which was exact while a
            // fold meant *every record agrees* — any row named the same appearance — and becomes
            // wrong the moment a fold carries two voices, because the sample's mark is only one of
            // them. Asking [`cell_mark`] which row actually says this mark is the same classifier
            // the row would have been drawn by, so a `?` in the tally is painted as a `?` and an
            // `Admit` as an `Admit`, with no second table to disagree with the first.
            let voice_role = |key: &str, mark: &str| -> Role {
                items
                    .iter()
                    .find_map(|item| {
                        let (text, role) = cell_mark(item, key);
                        (text == mark).then_some(role)
                    })
                    // Unreachable while the tally is built from these same records; `Role::Body`
                    // rather than a panic because a face that dies over an appearance is worse
                    // than one that draws the wire's own value plainly.
                    .unwrap_or(Role::Body)
            };
            // 🔴 Whether any folded column carries more than one voice, which is the one thing that
            // decides whether this face owes a road. One voice loses nothing: every record said the
            // same word and the word is on the screen. Two or more keep the **distribution** and
            // give up the binding of a value to a particular row — so the clause below spells where
            // the binding is (`req/942` §1-1: a renderer cannot invert, so it discloses).
            let multivoice = shared.iter().any(|(_, tally)| tally.len() > 1);
            // 🔴 **The quantifier, and it is the membrane's second obligation rather than a
            // decoration** (`req/924` §TUI-20). A renderer cannot invert, so what it owes instead
            // is to say what it let go of — and what this line lets go of is not a row, it is the
            // **scope of a claim**. Without the count in front, `verdict Admit` is a sentence about
            // the ledger; with it, it is a sentence about a set of a stated size. The words it
            // replaces are the ones a reader would otherwise have to go and find: *is this true of
            // the rows I cannot see?*
            let quantifier = [
                (margin(), Role::Quiet),
                (format!("{FETCHED_SCOPE} {}", items.len()), Role::Quiet),
            ]
            .into_iter();
            let fields = shared
                .iter()
                .enumerate()
                .flat_map(|(index, (key, tally))| {
                    let last = index + 1 == shared.len() && !multivoice;
                    let sep = if last { "" } else { " |" };
                    match tally.as_slice() {
                        // 🔴 **One voice keeps the spelling it has always had**, down to the byte:
                        // `verdict Admit |`. The count is not drawn because the quantifier in front
                        // of the row already said it — `all 32` — and a second `32` on the same row
                        // would be the face saying one number twice. This arm is what makes
                        // `super::tokens::Scheme::Ledger` reproduce the old screen exactly.
                        [(mark, _)] => {
                            vec![(format!("{key} {mark}{sep}"), voice_role(key, mark))]
                        }
                        // 🔴 Two or more: the **key once, then each value with the count of records
                        // that said it**, each painted in its own role. The counts are new
                        // information — `all 32` says how many records there are and says nothing
                        // about how they divide — and they are what keeps this from being the
                        // dishonest fold: nothing is asserted of a record that did not say it.
                        voices => {
                            let mut cells = vec![(key.to_string(), Role::Quiet)];
                            for (at, (mark, count)) in voices.iter().enumerate() {
                                let tail = if at + 1 < voices.len() { "" } else { sep };
                                cells.push((
                                    format!("{mark} {count}{tail}"),
                                    voice_role(key, mark),
                                ));
                            }
                            cells
                        }
                    }
                })
                .collect::<Vec<_>>()
                .into_iter()
                // 🔴 **The road, and it is drawn exactly when there is something to recover**
                // (`req/924` §TUI-21: a road has to arrive). `super::acts::Act::Open` is already
                // this face's word for *show me this record whole*, and the record face draws every
                // member — including the ones the grid has no column for — so the binding this line
                // gave up is one keystroke away. Spelled through [`spelled`] rather than typed, so
                // there is no second name for the act or for its key.
                .chain(multivoice.then(|| {
                    (
                        format!("by row {}", spelled(super::acts::Act::Open)),
                        Role::Quiet,
                    )
                }));
            // The same gap the header above it uses, so the two rows of the preamble start on the
            // same cell. Composed with `spans_with` for that reason and not because this row has
            // columns — it has clauses.
            //
            // 🔴 **And it is the row that was actually being cut** (`[T-r66]`, 2026-09-02). Unlike
            // the header and the records, this row is priced against **nothing**: its length is the
            // sum of however many columns came back constant and however long their values are, and
            // no ladder chose it against a width. Measured on a forty-record bed at 120x32 it came
            // to a hundred and twenty-two cells and the terminal took the last two, so the screen
            // read `inverse_status Availab` — a face telling a reader a word the engine never said.
            // `fit` is the same mark the record face uses, so the clause that runs out now ends in
            // `~` and the reader knows there is more.
            let clauses: Vec<(String, Role)> = quantifier.chain(fields).collect();
            preamble_lines.push(Line::from(spans_with(
                fit(&clauses, area.width, layout::COLUMN_GAP).into_iter(),
                tier,
                layout::COLUMN_GAP,
            )));
        }
    }
    // The rows the stream has moved past. `preamble_shown` is [`layout::scrolled`]'s answer, so the
    // rows this drops and the rows the window was computed against are one decision.
    let gone = preamble_lines.len().saturating_sub(preamble_shown);
    lines.extend(preamble_lines.into_iter().skip(gone));

    // 🔴 **The whole region is body now** (`req/924` §TUI-57). It used to give one row to the
    // grid's column header before anything else was measured; the header is a row of the scrolling
    // stream, so the rows the stream has are the rows the region has.
    let body_rows = area.height as usize;
    if shape == layout::Subject::Help {
        help_lines(&mut lines, body_rows, area.width, tier, plan);
    } else if items.is_empty() {
        // 🔴 `zero` only when the engine answered with the list and the list was empty. A refusal
        // has a body too, and drawing `0` for it would tell the reader there are no records when
        // the truth is that this process was not allowed to see them.
        let mark = reading.nothing().unwrap_or(Nothing::Zero);
        lines.push(Line::from(spans(
            plan.columns
                .iter()
                .map(|column| (pad(mark.mark(), column.width), mark.role())),
            tier,
        )));
        // 🔴 **No note here** (`req/924` §TUI-57). The line that says where the reader is and what
        // the screen let go of is the standing row now, so an empty list draws its one
        // kind-of-nothing row and nothing else. `plan.note_rows` is nought at every shape for the
        // same reason, and it is nought as a computed fact rather than as a branch here.
    } else if open {
        // 🔴 **The rows this record adds to its own members** (`req/924` §TUI-64, `req/38` SS1095,
        // Owner `#285-T`). See [`record_rows`] for what each one is and where it came from.
        // The attended record, every member of it, including the ones the grid has no column for.
        //
        // 🔴 Drawn **inside** the subject region rather than as a fifth region: the four regions are
        // declared, gated (g3, g4, g10) and laid out by row budget, and a region that exists only
        // while a key is held would be a fifth declaration whose priority nothing has ruled on.
        //
        // 🔴 The ceiling that used to be named here is **closed** (`req/964` §16): the disclosure
        // is composed from `layout::subject_shape`, the same classifier the `open` above is read
        // from, so while a record is open the disclosure says so instead of quoting a field count
        // that belongs to a grid nobody is looking at.
        let index = view.selected.min(items.len() - 1);
        // 🔴 [`cell_mark`], not [`wire::cell`] (`req/924` §TUI-13 追記). The record is the road the
        // disclosure names for every column the grid let go of, so a member drawn here by a
        // different classifier than the grid's is a road that arrives somewhere else: at forty
        // cells the grid has no `inverse_status` column at all, and this line was the only place a
        // reader could see it — spelled as the serialisation of an object rather than as a value.
        // 🔴 **The head, the values, and the marks gathered** (`[T-r58]`, 2026-09-02, defects 1
        // and 2). See [`record_own`] for what each row is and why seventeen flat rows were not it.
        let members = record_own(items[index], area.width);
        let beyond = record_rows(items[index], held);
        // 🔴 **The cut was decided by the plan and is read here rather than made again**
        // (`req/942` §11-3, and `super::layout::record_split`). The region used to compose its own
        // note, measure it, and cut against what was left — three decisions inside the region, and
        // the number the note carried was the one number on the screen nothing checked. The plan
        // settles all of it now: how many members, how many rows beyond them, whether there was a
        // cut, and how many rows the ledger underneath gets.
        let shown = plan.record_members_shown.min(members.len());
        let past = plan.record_beyond_shown.min(beyond.len());
        for (row, _) in members.iter().take(shown) {
            lines.push(Line::from(spans_with(
                fit(row, area.width, layout::COLUMN_GAP).into_iter(),
                tier,
                layout::COLUMN_GAP,
            )));
        }
        // 🔴 **The rule between the two groups is an underline on the last member row**
        // (`req/924` §TUI-62 裁定3). A blank row or a rule row would each cost one of the rows the
        // record is made of; an underline is the one horizontal line a terminal draws without
        // spending a row or a cell of ink, and it is the idiom the grid already uses for its
        // groups ([`layout::GROUP_ROWS`]). Drawn only when there is something below it to
        // separate: a rule under the last row of the last group is a line to nowhere.
        //
        // It separates **what the wire carried about this row** from **what other routes said about
        // it** — the one boundary on this screen a reader cannot infer from the words, because both
        // groups read `key value`.
        if past > 0 && shown > 0 {
            if let Some(last) = lines.last_mut() {
                *last = std::mem::take(last).style(Style::new().add_modifier(Modifier::UNDERLINED));
            }
        }
        for (text, role) in beyond.iter().take(past) {
            lines.push(Line::from(spans_with(
                fit(
                    std::slice::from_ref(&(text.clone(), *role)),
                    area.width,
                    layout::COLUMN_GAP,
                )
                .into_iter(),
                tier,
                layout::COLUMN_GAP,
            )));
        }
        // 🔴 **The cut is named by the region that made it, and only when there is one**
        // (`[T-r58]`, 2026-09-02, defect 4). The line it replaces was drawn at **every** shape and
        // spelled five counts, an address and a key at 120x32 where **nothing had been cut** — a
        // permanent row reporting an event that had not happened. The row the plan reserved for
        // this is inside the budget, so the sentence saying a member was dropped can never be the
        // thing that was dropped.
        // 🔴 `body_rows > 0` is not redundant (independent audit, 2026-09-02, M1): a region with
        // nought rows cut everything, so `record_split` sets the flag — and a line saying so on a
        // region that draws nothing would be a disclosure addressed to a screen that is not there.
        // The backend clips it either way; the guard is here so the *plan* and the *drawing* agree
        // rather than agreeing by accident of the backend.
        if plan.record_cut && body_rows > 0 {
            let mut clauses: Vec<String> = Vec::new();
            // 🔴 **Members are counted in the wire's keys, not in the face's rows.** One gathered
            // row can hold five of them, so a screen that dropped it and reported *1 of 9* would be
            // telling the reader that one fact went when five did. The rows are how the cut is
            // **made**; the keys are what the reader lost.
            let keys_total: usize = members.iter().map(|(_, keys)| keys).sum();
            let keys_shown: usize = members.iter().take(shown).map(|(_, keys)| keys).sum();
            if keys_shown < keys_total {
                clauses.push(format!("{} of {keys_total} members", keys_total - keys_shown));
            }
            if past < beyond.len() {
                clauses.push(format!("{} of {} more rows", beyond.len() - past, beyond.len()));
            }
            // `not drawn` leads, for the reason `Measured::bare` leads with the connection's mark:
            // a terminal cuts from the right, so what cannot be recovered from what is left goes at
            // the front. A row clipped to `not drawn: 3 of 10 mem` still says a cut happened.
            lines.push(Line::from(spans_with(
                fit(
                    &[(format!("not drawn: {}", clauses.join(", ")), Role::Quiet)],
                    area.width,
                    layout::COLUMN_GAP,
                )
                .into_iter(),
                tier,
                layout::COLUMN_GAP,
            )));
        }
        // 🔴 **The ledger the record was opened from keeps the rows the record does not need**
        // (`[T-r58]`, 2026-09-02, defect 3). Thirteen rows of the 120x32 capture — a third of the
        // terminal — were blank, and a blank third is furniture. What goes there is not padding: it
        // is the one fact a detail face otherwise destroys, **where this record sits among the
        // others**, and it is what lets the sentence `record 1 of 31` be deleted rather than
        // reworded. The attended row is inside the window by construction
        // (`super::layout::window`, gate g28) and is drawn in `Role::Attend`, so the row whose
        // detail stands above is the row marked below.
        //
        // No column header stands over them: the header belongs to the grid, and this is a slice of
        // the ledger under a record rather than a table (the same argument `shape` is read for
        // above, and what probe `p14` measures).
        for (index, item) in items
            .iter()
            .enumerate()
            .skip(plan.window.first)
            .take(plan.window.rows)
        {
            let cells: Vec<(String, Role)> = std::iter::once((margin(), Role::Body))
                .chain(columns.iter().map(|column| {
                    let (text, role) = cell_mark(item, column.key);
                    (pad(&text, column.width), role)
                }))
                .collect();
            // 🔴 **`fit`, and the grid's own loop does not have it** (`[T-r58]`, 2026-09-02,
            // defect 5). `columns_for_less` prices the columns against
            // `LEFT_MARGIN + sum(width) + (n-1) * COLUMN_GAP` and on a reading where every column
            // carries a value the row still arrives one cell or more over the screen — measured on a
            // forty-record bed at 66x20, where `created_at` lost its `Z` and nothing said so. This
            // is **not** a defect this lane introduced and it is **not** repaired here: the grid's
            // own loop is another lane's write-target, and the arithmetic is in
            // `layout::columns_for_less`. What is repaired is this face: a row it draws is cut with
            // the cut marked, so the record face does not inherit a silence it can see.
            //
            // 🔴 **Superseded, and both halves are closed** (`[T-r66]`, 2026-09-02): the arithmetic
            // is [`super::layout::row_width`] — the price is now the row, so the `(n-1)` above is no
            // longer what anything computes — and the grid's own loop draws through `fit` like this
            // one. Kept rather than deleted (no-delete) because the paragraph is the record of what
            // was known when, and gates `g89` and `g90` are what say it is closed.
            let line = Line::from(spans_with(
                fit(&cells, area.width, layout::COLUMN_GAP).into_iter(),
                tier,
                layout::COLUMN_GAP,
            ));
            lines.push(if index == view.selected {
                line.style(paint(Role::Attend, tier))
            } else {
                line
            });
        }
    } else {
        // 🔴 **The note and the window it was measured against both moved** (`req/924` §TUI-57).
        // The list's note — the way out, the keys, and `N of M` — is composed in
        // `layout::resolve_attended` and drawn on the standing row, so it is on the screen at every
        // shape instead of out of whatever rows the records left over; the bounded defect
        // `note_rows` names (the spare is nought, the legend vanishes, nothing says so) is closed
        // by that rather than bounded. The window is [`layout::scrolled`]'s, asked above with the
        // preamble this region is actually drawing, so the rows the stream moved past and the rows
        // this loop draws are one arithmetic.
        // 🔴 **A compressed column keeps painting the rows it was lifted off** (`req/924` §TUI-53,
        // `req/38` SS1084 ①). `compress_window` takes the columns every drawn row agrees on, and on
        // this bed those are exactly the columns that carry a role — so the ledger came back as
        // twenty-four rows of bare id in one colour, and the measured emphasis fell from twenty-five
        // rows to two. That is the defect `req/924` §TUI-19 named (*no handhold for telling one row
        // from the next*) made **worse** by a cut meant to help: the compression was taking a fact
        // off the row and giving nothing back.
        //
        // The repair is a fact rather than a decoration, and that is why it is allowed: the column
        // is constant across exactly these rows — that is what `compress_window` measured — so
        // every one of them **is** `Admit`, and painting them in the verdict's own role says so.
        // The header still spells the word once; the rows carry what it means.
        //
        // The first roled column wins and the rest are already spoken for by the header: stacking
        // two would be two meanings on one cell, which the token table has no way to resolve.
        // 🔴 **The lifted role is gone with the lift** (`req/924` §TUI-62 (`req/38` SS1093, Owner `#284-T`, 2026-09-01)).
        // `§TUI-53` had the compressed columns paint the rows they were taken off, because taking a
        // fact off a row and giving nothing back is what turned the ledger into one colour. The
        // columns are on the rows again, so each cell carries its own role and there is nothing to
        // hand down -- the emphasis comes back by the same route the columns did.
        for (index, item) in items
            .iter()
            .enumerate()
            .skip(window.first)
            .take(window.rows)
        {
            let cells: Vec<(String, Role)> = std::iter::once((margin(), Role::Body))
                .chain(columns.iter().map(|column| {
                    let (text, role) = cell_mark(item, column.key);
                    (pad(&text, column.width), role)
                }))
                .collect();
            // 🔴 **`fit`, and the grid's own loop is where it was missing** (`[T-r66]`, 2026-09-02,
            // closing `[T-r58]` defect 5). `[T-r58]` wrote the mark into the record face's ledger
            // slice forty rows above this one and left this loop alone — the grid was another
            // lane's write-target that day — so the ledger a reader spends every session in was the
            // one face that could stop mid-value in silence. One idiom for *this row was cut* on
            // every face that draws a ledger row, not two.
            let line = Line::from(spans_with(
                fit(&cells, area.width, layout::COLUMN_GAP).into_iter(),
                tier,
                layout::COLUMN_GAP,
            ));
            // The attention mark is the row itself rather than a gutter column: a gutter would take
            // two cells from a width the plan already spent, and the plan is what says which columns
            // fit.
            //
            // 🔴 **The group rule** (`req/924` §TUI-62 (`req/38` SS1093, Owner `#284-T`, 2026-09-01) 区切り). A run of rows
            // with nothing between them is hard to keep a place in. The ruling offered a rule row or
            // a blank row between groups; both cost a record, and the underline on the last row of
            // each group is the one horizontal line a terminal draws **without spending a row or a
            // cell of ink**. [`layout::GROUP_ROWS`] is how many share a group.
            let grouped = (index + 1) % layout::GROUP_ROWS == 0;
            let line = if index == view.selected {
                line.style(paint(Role::Attend, tier))
            } else if grouped {
                line.style(Style::new().add_modifier(Modifier::UNDERLINED))
            } else {
                line
            };
            lines.push(line);
        }
        // 🔴 **The note left this region** (`req/924` §TUI-57, `req/38` SS1088, Owner `#282-T`).
        // `N of M`, the key legend and the count of rows let go of are composed in
        // `layout::resolve_attended` and drawn on the standing row, so they are on the screen at
        // every shape rather than out of whatever rows the records left over. The ladder itself is
        // unchanged and is walked there — `note_ladder`, `afford` and `fold_note` are the same
        // three functions the gates have always measured; only the caller moved.
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// What this face can do, drawn from the declarations that already say so.
///
/// 🔴 **This face's two dead declarations, brought back to the screen.** `super::acts::Act::intent`
/// and `super::layout::Intent::sentence` were both written, both gated for internal consistency,
/// and called by **nothing that draws** — a sentence per act and a sentence per region, declared for
/// a reader who had no way to reach them. This is where they are read.
///
/// Nothing here is a new word: every string comes from `Act::name`, `Act::keys`, `Act::intent`,
/// `RegionRole::short` and `Intent::sentence`, so there is no second vocabulary to drift from the
/// first. There is no border, no colour and no mark of its own — **the visual design of this face
/// is deliberately nothing at all**, because the Owner's visual ruling on the sample has not been
/// made and a new screen invented before it would be inventing an answer.
fn help_lines(
    lines: &mut Vec<Line<'static>>,
    body_rows: usize,
    width: u16,
    tier: Tier,
    plan: &Plan,
) {
    // 🔴 **The word each line is filed under, carried beside the line** (`[T-r86]`, 2026-09-02,
    // `req/924` §TUI-103). The note below names the entries a shape had no room for, and reading
    // those names back out of the drawn text would be this face measuring its own padding instead
    // of its own declarations — `pad` is not a vocabulary. Every label here is already a declared
    // `&'static str` (`Act::name`, `RegionRole::short`, `Link::name`, `ENGINE_LABEL`), so the note
    // introduces no word of its own, which is the property that lets this face be believed.
    fn entry(label: &'static str, rest: &str) -> (&'static str, String) {
        (label, format!("{} {rest}", pad(label, 11)))
    }
    let mut entries: Vec<(&'static str, String)> = Vec::new();
    // 🔴 **The escape hatch, and this is the half that makes the shorter disclosure honest**
    // (`req/924` §TUI-21: *the numbers may stay on the screen and the names may move behind `?` —
    // but do not say they were moved until a gate has confirmed the hatch lists them*). The
    // disclosure now says `2 routes read and not drawn -> help:?` and `N of M fields not drawn`
    // without their names; these two lines are where the names are, spelled from the same two
    // declarations the counts are counted from, so a route added to `READ_NOT_DRAWN` or a column
    // let go of by width grows this face rather than escaping it. Gate `g61` fires on exactly that
    // pairing: **a hatch that lists nothing turns a disclosure into a deletion.**
    //
    // 🔴 **First, and they were written last** (`[T-r42]`, 2026-09-01, `req/924` §TUI-21's own
    // gate clause). Written last they were never reached: measured at the seven ruled shapes the
    // hatch listed the names at 120x32 and 100x30 and cut before them at the other **five**, and
    // at **four** of those five the grid's disclosure was still spelling `-> help:?` (the fifth,
    // 40x10, takes the short form and makes no claim). A road that leads to a page which does not
    // carry the thing is the deletion this clause was admitted to prevent.
    //
    // The ordering rule is the one [`Rung::keeps`] already states — *what a line gives up may be
    // given up only if the same screen spells it somewhere else*. The acts below are spelled on
    // the note line of every face and again by `gx tui --help`, which this face's own note names
    // as their road; these three are spelled **nowhere else** and `gx tui --help` does not answer
    // for them. So they outrank the acts inside the hatch, and what gives way is what keeps a
    // road. This is the same argument the retreat below was decided by, read in the other
    // direction, and the retreat's own subject — the provenance — is untouched by it.
    //
    // 🔴 **Named ceiling, and the counts inside it were false** (independent audit F-01,
    // 2026-09-02). It read: *the two that stop being reached at 80x24 are `provenance` and
    // `link.open` -- the second is spelled nowhere else ... it is still reached at 120x29 ... and
    // the note still spells `17 of 19`.* Measured on this lane's own capture: `link.open` was
    // reached at **no** shape, `provenance` **was** reached at 120x29, and the note spelled
    // `22 of 23`. A paragraph that names counts has to be re-measured whenever the list it counts
    // changes, and this one was carried through two lanes without that.
    //
    // The ceiling itself is real and stands: this face has no act that scrolls the hatch, so
    // entries past the rows a shape has are cut, and the note's `N of M` is what marks the cut.
    // What changed is the order: the three sentences spelled **nowhere else** -- the address, the
    // dots and `live::LIVE_MEANS` -- are the first three now, so what a cut takes is an entry that
    // has a road somewhere else.
    // 🔴 **The page's address, and it is first** (`req/924` §TUI-57, `req/38` SS1088, Owner
    // `#282-T`). `GET /v1/transformations` was the top rail's title; the rail is gone and the
    // ruling is that *a signpost is enough once and the complete address is a detail*. This is the
    // detail, and it is the **only** spelling of the address on any screen this face draws — which
    // is what gate `g75` measures from the other end.
    //
    // First, for the reason the three below it are first: it is spelled nowhere else, so a shape
    // that cuts the hatch before reaching it would turn §TUI-57's *moved* into §TUI-21's *deleted*.
    // 🔴 **And what the identity column draws is *not* one** (`[T-r71]`, 2026-09-02,
    // `req/942_artifacts/tui_r71_2026-09-02/RULING.md`). It rides this entry rather than one of its own, and that is a measured
    // decision rather than tidiness: the hatch is already cut at 120x32 -- it says so, `N of M` --
    // so an entry of its own takes an act's line off the screen, which gate `g36` refuses. This line
    // had sixty spare cells and the fact belongs beside the address anyway, because it is a fact
    // about what an address is.
    //
    // The fact: measured on both roads a reader has -- `gx receipt show <id>` and
    // `GET /v1/transformations/{id}` -- **no prefix of an id resolves at any length**, not three,
    // not thirteen, not fifty-one of fifty-two. So a reader who copies what that column shows gets a
    // refusal on either road, and until this clause no screen said so.
    let address = format!(
        "{} | id column: {} of a `{}`; no prefix resolves, {} opens the whole",
        layout::LEDGER_ADDRESS,
        layout::LEDGER_COLUMNS[0].width.saturating_sub(1),
        layout::LEDGER_COLUMNS[0].key,
        // 🔴 The key the road is actually bound to, read off the declaration rather than typed.
        // `req/924` §TUI-21: a road named here has to arrive, and a key spelled twice arrives
        // nowhere the day the binding moves.
        acts::Act::Open.keys().first().copied().unwrap_or_default(),
    );
    entries.push(entry("address", &address));
    // 🔴 **The six dots, and what each one says** (`req/924` §TUI-57, `req/38` SS1088, Owner
    // `#282-T`). The dot is the only thing on the standing row that stands for the connection, and
    // a mark a reader cannot look up is a mark that says nothing to them. Read out of
    // [`tokens::GLYPHS`] through [`tokens::DOTS`], so the meanings here and the marks drawn are one
    // declaration — a hand-written copy would be this face's second vocabulary for the one thing
    // §TUI-30 admitted the mark under.
    //
    // Second, for the reason the address above it is first: it is spelled nowhere else, and gate
    // `g74` is what confirms the six reach this page rather than this comment being believed.
    let dots = tokens::DOTS
        .iter()
        .filter_map(|mark| tokens::glyph(mark).map(|g| format!("{mark} {}", g.means)))
        .collect::<Vec<_>>()
        .join(" | ");
    entries.push(entry("dots", &dots));
    // 🔴 **Third, and it was last** (independent audit F-01, 2026-09-02). `live::LIVE_MEANS` is
    // Owner #227's ruling — *a face that prints `LIVE` has to say **whose** events it counts* — and
    // it is the one sentence in this face about a **measured engine limitation** (`req/38` SS987:
    // the event bus is in-process, so a journal another process writes is never observed). It is
    // spelled nowhere else on any screen.
    //
    // It stood last, and the entries this lane added above it pushed it off the drawn set at
    // 120x29 — the very shape Owner #227's ruling was made on. **The reduction pass cut the
    // honesty**, which is the defect `INHERITED_PRINCIPLES` §削る前に3分類 names, committed by a lane
    // whose report claimed it had not. It goes beside the dot because the dot is now the thing the
    // sentence qualifies: `●` is a claim about *this process's* events and about nothing else.
    entries.push(entry(live::Link::Open.name(), live::LIVE_MEANS));
    // 🔴 **This entry was almost made to carry `wire::RECEIPT_VIEW_NOT_DRAWN` and must not be**
    // (`[T-r76]`, 2026-09-02). The predicate reads as if it fits — *read and not drawn* is true of
    // the two routes and of those three members alike — but measured on this lane's own
    // engine-backed captures the extra text cost the `beyond` entry at 66x20 and an act's intent at
    // 100x30, and both are 道標 rather than 説得. The names went onto the record's own `key_id` row
    // instead, where they cost no row at any width. Written down so the next lane does not rediscover
    // the fit and pay the price for it.
    entries.push(entry("routes", &layout::READ_NOT_DRAWN.join(", ")));
    // 🔴 **The two roads an opened record carries, and the one thing it cannot draw** (`req/924`
    // §TUI-64, `req/38` SS1095, Owner `#285-T`). Fourth, immediately after the three sentences
    // spelled nowhere else, and for the same reason they are first: the addresses below are the
    // only spelling of `GET /v1/transformations/{id}` and `GET /v1/receipts/{tid}` on any screen
    // this face draws, and the sentence after them is the **membrane's second obligation** — a
    // renderer cannot invert, so what it owes instead is to name what it let go of.
    //
    // The chain is dropped rather than approximated. `req/924` §TUI-30's sketch draws
    // `submitted -> planned -> verified -> committed`; `state` is one word for where the row is
    // **now**, and no route this face reads answers *which states it passed through*. Three arrows
    // for three transitions nobody measured is the face asserting a history the engine never sent.
    // See [`record_rows`] for the full argument; this is where a reader is told.
    let record = format!(
        "{} | {}",
        wire::RECORD_ROUTES
            .iter()
            .map(|route| format!("GET {route}"))
            .collect::<Vec<_>>()
            .join(", "),
        // 🔴 The three words the receipt's rows are spelled with, on the entry that names the route
        // they come off ([`RECEIPT_ROW_WORDS`], `[T-r58]` 2026-09-02). They stand here rather than
        // on an entry of their own because this hatch has **no spare row**: measured at 120x32, one
        // more entry of this lane's takes the last `Intent::sentence` off the page and gates `g36`
        // and `g65` go red.
        RECEIPT_ROW_WORDS.join(", ")
    );
    entries.push(entry("record", &record));
    // 🔴 **The vocabulary the two ledger faces stand on, and it is derived** (`[T-r58]`,
    // 2026-09-02, defect 6). `req/924` §TUI-21 is that a road has to arrive: a word on the screen
    // that `?` cannot explain is *deleted* wearing *moved*'s clothes. Measured on this lane's own
    // engine-backed captures, the words the gate could not find at 120x32 were mostly **column
    // labels** and **the engine's closed enumerations** — and the **list** face fails on the same
    // ones, so this entry is not the record's alone.
    //
    // 🔴 **Both halves are derived.** The columns are `layout::LEDGER_COLUMNS`; the verdicts and the
    // inverse states are `wire::VERDICT_KINDS`, `wire::INVERSE_KINDS` and `wire::CONSUMED_KIND`. The
    // gate's own ruling is the reason these two are required and an address is not: a value from an
    // **open** set cannot be listed by any page, and a value from a **closed declared** set can. A
    // word the face learns tomorrow arrives on this page the same day.
    //
    // 🔴 **What is missing, by name: the lifecycle states** (`Committed`, `Superseded`, `Denied`,
    // `Draft`). This face has no declared enumeration of them — `state` arrives as a bare string,
    // and `wire.rs` says in its own words why the engine's enum is not named in this directory — so
    // a hand-made list here would go stale in silence, which is worse than a gap that is measured.
    // **Four words, open, and counted.**
    //
    // 🔴 **It is composed here and pushed further down, and that is a measured retreat.** Written in
    // place — fourth, beside the other entries this lane owns — it is two rows wide at 120 cells and
    // **eleven** at 46, and those eleven come out of the entries below it. Measured on the
    // engine-backed sweep: 120x32 went 26 → 10 unexplained words, and 66x20, 60x20 and 46x12 went
    // 31 → 33, 31 → 34 and 23 → 33, because the hatch stopped reaching its own later entries and
    // the **vocabulary** the gate compares against shrank. An entry that explains words by taking
    // other explanations off the page is `SS842`'s reduction pass wearing a disclosure's face.
    //
    // So it is placed after the entries that are spelled nowhere else and before the acts, where the
    // shapes that cannot afford it cut **it** rather than them. The consequence is stated rather
    // than hidden: at 66x20 and below this entry is not on the page, and the words it explains are
    // unexplained there. The count is in this lane's report.
    let vocabulary = format!(
        "{} | verdict {} | inverse {}",
        layout::LEDGER_COLUMNS
            .iter()
            .map(|column| column.key)
            .collect::<Vec<_>>()
            .join(", "),
        wire::VERDICT_KINDS.join(", "),
        wire::INVERSE_KINDS
            .iter()
            .copied()
            .chain(std::iter::once(wire::CONSUMED_KIND))
            .collect::<Vec<_>>()
            .join(", ")
    );
    // 🔴 **The record face's own vocabulary belongs here and does not fit, and that is a measured
    // fact rather than a decision** (`[T-r55]`, 2026-09-02). `req/924` §TUI-21 is that a road has
    // to arrive: a word on the screen that `?` cannot explain is *deleted* wearing *moved*'s
    // clothes. `tools/gates/legibility_gate.mjs` fired against three captures in
    // `req/942_artifacts/tui_r55_2026-09-02/` -- this lane's engine-backed record face, the same
    // engine's **list** face as one control, and the **base** binary's record face as the other --
    // at 120x32:
    //
    //   record face, base 2971840a   15 words on the screen `?` does not carry
    //   record face, this lane       27
    //   list face, this lane         15   (the gate is red on BOTH faces, before and after)
    //
    // 🔴 **The first reading of this said "six", and six was wrong.** It was counted off the
    // gate's printed list, which is **capped at twelve per region and says so on the line above**;
    // the full `--json` set gives **thirteen** tokens added and one removed. **Eight are words** --
    // `held`, `agrees`, `key_id`, `leaf`, `root`, `checked`, `more`, `rows` -- and the rest are
    // fragments of a single `ed25519-…` key id, which is the gate's tokeniser meeting a **value**
    // rather than a word. Reading a capped list as a complete one is a defect this repository has a
    // name for, and it is recorded here rather than corrected away.
    //
    // 🔴 The fragments are known to be fragments **because they moved**: two sweeps against two
    // `gx demo` runs gave `ed f c e de d` and `ed f efc c eaf`, while the eight words were identical
    // both times. A count that changes when the bed mints a new key is counting the bed.
    //
    // An entry spelling the eight was written, and adding it turned `g36` and `g65` red --
    // the hatch at 120x32 has a fixed number of rows, and a fourth entry of this lane's pushed four
    // acts' intents and the vacant-field names off the page. Room was then bought back by cutting
    // this lane's own two entries below to one row each, **and it was still not enough**.
    //
    // So the entry is **not** here, and buying the room out of somebody else's disclosure was
    // refused: that is the reduction pass that cuts the honesty (`INHERITED_PRINCIPLES`
    // 削る前に3分類), and displacing an act's intent to explain `key_id` trades a road for a road.
    // The vocabulary itself is declared and reachable in code as `wire::ReceiptMark::means`, which
    // says of itself that no screen reads it yet. **The gap is open, it is measured, and the number
    // is eight words.** Closing it needs a hatch that scrolls, or a second page -- either is a
    // ruling, and a lane that invented one to fit its own eight words would be deciding it alone.
    // 🔴 **These two were each a row longer, and the row was spent on the one above them**
    // (`[T-r55]`, 2026-09-02). Adding `record row` put gates `g36` and `g65` red: the hatch at
    // 120x32 has a fixed number of rows, and a fourth entry of this lane's pushed four acts'
    // intents and the vacant-field names off the page. The room was found **in this lane's own
    // three entries** rather than in anybody else's — trading another disclosure for mine would be
    // the reduction pass that cuts the honesty (`INHERITED_PRINCIPLES` 削る前に3分類), and the three
    // categories that survive a cut are exactly what is kept here: both roads, the disclaimer on
    // each, and the fact that the chain is not carried. What went is only ①説得.
    let beyond = format!(
        "{RECEIPT_VERIFY_ADDRESS} grades a receipt; this face does not | {UNDO_ADDRESS} runs the \
         inverse; no key here does"
    );
    entries.push(entry("beyond", &beyond));
    entries.push(entry(
        "not carried",
        "the lifecycle chain: no route says which states a row passed through, so it is not drawn",
    ));
    // 🔴 Asked of `columns_for` rather than read off `plan.dropped_fields`, and the difference is
    // a real one: while the help face is the subject, the plan's dropped set is **empty** by
    // construction (`layout::resolve_attended` — the help face draws no wire value, so a set of
    // wire keys it "did not draw" would be counting a grid nobody is looking at). The reader
    // pressing `?` wants the names the *grid* let go of at this width, which is this question.
    let (_, unseen) = layout::columns_for(width);
    // 🔴 **`columns_for` already carries the page keys**, and extending them again is why the hatch
    // has been spelling `next_cursor, next_cursor` on every frame — on `main`, before this lane
    // touched anything. Found in this lane's own base capture; repaired here because the line it is
    // in is one this lane is editing anyway, and reported as **found rather than introduced**.
    let names: Vec<&str> = unseen;
    let not_drawn = if names.is_empty() {
        wire::Nothing::Zero.mark().to_string()
    } else {
        names.join(", ")
    };
    entries.push(entry("not drawn", &not_drawn));
    // 🔴 The engine's own keys **by name**, and the condition the last of them is drawn under.
    // [`RAIL_KEYS`] explains the condition: an engine that is `ok` has no reason, and the face will
    // not assert `--` on the engine's behalf, so it draws nothing and says here that it does.
    //
    // 🔴 **superseded → `req/924` §TUI-39 追記 (SS1075) — and this is a THIRD site** (`[T-r55]`,
    // 2026-09-02). `req/38` SS1098 裁定1 ruled the divergence to be two sites and named them; the
    // sibling sweep this lane ran over the **claim** rather than over the two line numbers found
    // this one. `wire::status_reason` does spell `--` for that key, so "it draws nothing" is
    // overtaken here exactly as it is at the other two. The count in SS1098 was measured with an
    // instrument that pairs on anchor quotation, and **none of the three blocks quoted one at the
    // time it was measured** — which is the finding SS1098 wrote about `g70` arriving one site
    // short of its own conclusion.
    //
    // 🔴 **And that sentence stopped being true the moment this lane wrote it** (`[T-r55]`,
    // 2026-09-02, independent audit finding M2). The corrections added above cite `§TUI-39`, so
    // `g70` now pairs `engine_line` with it — the gate can see the site the paragraph says it
    // cannot. The tense is corrected rather than the claim deleted: *why* `g70` could not see them
    // is still the finding, and the repair for it is that the blocks now quote the anchor.
    //
    // 🔴 **The names are here since `[T-r42]`** (2026-09-01). The rail gives its keys up from the
    // end at every width below a hundred cells and the disclosure counts them — `2 engine keys not
    // drawn | GET /v1/healthz` — so the count had a road and the **names** had none on any screen.
    // `req/924` §TUI-22 classified `status ok` and `ledger_agrees yes` as things that may be folded
    // and may **not** be discarded; a count whose members cannot be named anywhere is discarding
    // them with a number left behind.
    // 🔴 **The names behind the count `req/924` §TUI-45 grew** (`req/38` SS1076), and it stands
    // **third**, beside the other clause about what is not on the screen. The disclosure says
    // `N of 11 fields not drawn` and the rule that raised N is a different rule from width, so a
    // reader who cannot tell the two apart cannot act on either. `g61`'s argument, one rule up: a
    // count whose members can be named nowhere is a deletion with a number left behind.
    let no_answer = format!(
        "{} | every record in this reading answered with that mark and no answer was obtained, so \
         the column is not drawn at any width",
        if plan.vacant_fields.is_empty() {
            wire::Nothing::Zero.mark().to_string()
        } else {
            plan.vacant_fields
                .iter()
                .map(|(key, mark)| format!("{key} {mark}"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    entries.push(entry("no answer", &no_answer));
    // 🔴 The engine's line **as it stands underneath the fold**, values and all. `req/924` §TUI-29
    // folds the rail to `engine ok` while both claims hold, and SS842 is why that is only half a
    // decision: `engine_version` is spelled on no other row of any screen, so folding it without
    // moving it is a deletion. `plan.engine_full` is where it moved to, and this is where a reader
    // reaches it. The condition sentence is kept word for word — `g59` reads it.
    // 🔴 **`on the rail` became `here`** (`req/924` §TUI-57). There is no rail; the standing row
    // spells none of the engine's keys and the dot is what stands for it. The condition
    // `status_reason` is carried under has not changed and neither has the fold's own sentence,
    // which gate `g59` reads — what changed is the place the sentence names, and a sentence that
    // goes on naming a row this face no longer draws is the defect this whole session is about.
    let engine = format!(
        "{} | status_reason is carried here only when status is not ok | folded to \
         `{ENGINE_LABEL} {HEALTHY}` while status is {HEALTHY} and ledger_agrees is {AGREES}",
        plan.engine_full
            .iter()
            .map(|(key, value)| format!("{key} {value}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    entries.push(entry(ENGINE_LABEL, &engine));
    // 🔴 The marks that stand in for an act's name, with the name they stand in for. Read out of
    // the two declarations rather than spelled again: `ACT_MARKS` pairs them and
    // `super::tokens::GLYPHS` holds the meaning and the deleted word, so a mark added without an
    // argument fails P6 before it ever reaches this line.
    // 🔴 **And the receipt's own marks, on the same row** (`[T-r58]`, 2026-09-02, defect 6).
    // `wire::ReceiptMark` is a mark vocabulary in the way `ACT_MARKS` is, and the record face draws
    // one of its five on every open record — `held` stood on the screen with no road to it. Read out
    // of `ReceiptMark::ALL`, so the five are the five. It is **here** and not on an entry of its own
    // for the reason given above the `record` entry: the hatch has no spare row.
    //
    // The ledger's vocabulary, composed above, stands here — after everything spelled nowhere else
    // and before the acts.
    entries.push(entry("vocabulary", &vocabulary));
    let marks = format!(
        "{} | receipt {} | record rows {}",
        ACT_MARKS
            .iter()
            .map(|(act, mark)| format!("{mark} {}", act.name().trim_start_matches("act.")))
            .collect::<Vec<_>>()
            .join(", "),
        wire::ReceiptMark::ALL
            .iter()
            .map(|mark| mark.mark())
            .collect::<Vec<_>>()
            .join(", "),
        RECORD_ROW_WORDS.join(", ")
    );
    entries.push(entry("marks", &marks));
    entries.extend(acts::ACTS.into_iter().map(|act| {
        // 🔴 Not `entry`: an act's line is padded to six and fourteen rather than eleven, and the
        // note below names it by the same word this line opens with either way.
        let name = act.name().trim_start_matches("act.");
        (
            name,
            format!(
                "{} {}  {}",
                pad(name, 6),
                pad(&act.keys().join(" "), 14),
                act.intent()
            ),
        )
    }));
    // 🔴 **And whether the region is on the standing frame at all** (`req/924` §TUI-57). Two of the
    // four are ruled off ([`layout::STOOD_DOWN_REGIONS`]) and a reader who counts regions on the
    // screen finds two. Read from the declaration rather than written out, so the day one comes
    // back this line says so without being edited.
    entries.extend(layout::REGIONS.into_iter().map(|region| {
        let standing = if layout::STOOD_DOWN_REGIONS.contains(&region.role) {
            " | not on the standing frame"
        } else {
            ""
        };
        entry(
            region.role.short(),
            &format!("{}{standing}", region.intent.sentence()),
        )
    }));
    // 🔴 **The disclosed half of the provenance, and it stands last** (Owner #227: keep the four
    // measured facts, and separate what stands on every frame from what is reached on demand). The
    // provenance region has three rungs and the bottom one gives the connection's counts up; this
    // is the line in full, and the badge's meaning beside it, because neither is spelled anywhere
    // else on any screen.
    //
    // Last, and that is a **measured retreat**. They were first — a rung-price argument: what is
    // spelled nowhere else outranks what the address below carries. At forty by ten the provenance
    // line wraps to three of the four body rows, so first meant the help face read `1 of 16` and a
    // reader who pressed `?` for the keys got a clock. The rule the ordering was serving is real
    // and the screen it produced was worse than the one it replaced, so the ordering yields and
    // the **note** carries the honesty instead: it names what the shell address answers for rather
    // than implying it answers for all sixteen.
    //
    // Both labels are declared words. `RegionRole::short` is the region this belongs to and
    // `Link::name` is the state whose badge it explains, so the help face still introduces no
    // vocabulary of its own — the property that lets it be believed.
    let provenance = entry(
        layout::RegionRole::Provenance.short(),
        &plan.provenance_full,
    );
    // 🔴 **It moves to the front exactly when something else on the screen promises it.**
    //
    // The retreat written above is real and is kept: placed first unconditionally, this line wraps
    // to three of the four body rows at forty by ten and a reader who pressed `?` for the keys gets
    // a clock. What changed is that `req/924` §TUI-29 lets the provenance **region** stand down when
    // every route answered `200`, and the disclosure then spells `provenance -> ?`. A road that
    // arrives at a page which was cut before this line is the defect `req/924` §TUI-21 names by its
    // own words — *逃がし先が空なら、それは逃がしたのでなく消したのと同じ* — and `g61` is the gate
    // written for it one clause over.
    //
    // 🔴 **Measured, not reasoned, and it was this lane's own defect**: with the entry last, the
    // region stood down at 80x24, 66x20 and 60x20 while the hatch at those three shapes carried no
    // `worst … ms` at all. Three of eight shapes shipped a signpost pointing at nothing.
    //
    // The retreat's own shape is untouched at forty by ten, and that is a fact rather than a hope:
    // there the rail cannot carry the live badge, so `layout::resolve_attended` keeps the region —
    // the branch below is not taken, and the ordering the retreat chose still stands where the
    // retreat was measured.
    let stood_down = plan.rows_for(layout::RegionRole::Provenance) == 0 && !plan.provenance_folded;
    if stood_down {
        entries.insert(1, provenance);
    } else {
        entries.push(provenance);
    }
    // Composed before the rows it sits under, like every other note in this face: a line that says
    // how much was let go of, written after the letting go, is a line that gets clipped.
    //
    // 🔴 **RETRACTED (`[T-r86]`, 2026-09-02, `req/924` §TUI-103, `req/38` SS1155). The paragraph
    // below is kept because the argument in it is still the right argument — it is the conclusion
    // that was wrong.** `acts in full: gx tui --help` was measured against eight other terminal
    // products photographed at the same shapes on the same instrument: **this face was the only
    // one of nine whose standing help sends the reader out of the process**, at 5/5 shapes, while
    // the other thirty captures point at 0 addresses outside their own window. It is also this
    // face's own §TUI-9 A3 — *do not put a way out of the process on the standing face* — broken by
    // the face whose whole job is to be the way in. The clause is gone; what replaces it is below.
    //
    // 🔴 `acts in full:` and not a bare address. `gx tui --help` spells every declared act — gate
    // g12c in the consumer's suite is what makes that worth saying — and it spells **neither** of
    // the two lines above, which are measurements of this run. An address offered against a count
    // of sixteen would be claiming to answer for all sixteen; naming what it carries is four words
    // and the difference between a road and a wave.
    //
    // 🔴 **What replaces it: the names, not a road.** The count was never the missing half —
    // `24 of 27` is on the screen and always was. What a reader could not get was **which three**,
    // and that answer is *inside* this process, in the words the entries are already filed under.
    // Photographed across nine products at three narrow shapes, the ways of speaking about what was
    // cut were: a count (1/9, this face), a percentage (1/9), **names (0/9)**, and nothing at all
    // (7/9). The 0/9 is a fact about the other eight and **not** an argument for this — this is the
    // membrane's second obligation (a renderer cannot `invert`, so what it owes instead is to say
    // what it let go of), and it would be owed on an empty shelf.
    //
    // 🔴 **The names are bounded by the note's own rows, and the bound is spoken.** The rows are
    // measured on the count alone, so a name can never take a row away from the entry it is naming;
    // whatever will not fit is left as `+N`, a number the reader can act on rather than a silence.
    // At forty cells nothing fits and the note is the count alone — the same disclosure it made
    // before, minus the road out of the process, and one row cheaper, because the address it used
    // to carry wrapped to a second row there.
    let head = |shown: usize| {
        format!(
            "{shown} of {} | close: {}",
            entries.len(),
            spelled(Act::Help)
        )
    };
    let note_rows = layout::rows_needed(&head(entries.len()), width) as usize;
    let room = body_rows.saturating_sub(note_rows);
    let mut spent = 0usize;
    let mut shown = 0usize;
    for (_, text) in &entries {
        let need = layout::rows_needed(text, width) as usize;
        if spent + need > room {
            break;
        }
        for line in layout::wrap(text, width) {
            lines.push(Line::raw(line));
        }
        spent += need;
        shown += 1;
    }
    // The obligation a renderer carries in place of `invert`: say what was let go of. The count is
    // of declarations, and the words after it are the declarations the count is short by, named in
    // the order they would have been drawn in — so a reader who widens the terminal sees the first
    // of them arrive where the note said it would.
    let mut note = head(shown);
    let dropped = entries.len() - shown;
    let mut named = 0usize;
    while named < dropped {
        let lead = if named == 0 { LET_GO_LEAD } else { ", " };
        let label = entries[shown + named].0;
        // What would still be unnamed after taking this one. It is added to the candidate before
        // the fit is asked, so a row can never be spent on a name and then have no cells left to
        // admit the count of the ones it displaced — the silent bound this clause exists to refuse.
        let unnamed = dropped - named - 1;
        let mut candidate = format!("{note}{lead}{label}");
        if unnamed > 0 {
            candidate.push_str(&format!(" +{unnamed}"));
        }
        if layout::rows_needed(&candidate, width) as usize > note_rows {
            break;
        }
        note = format!("{note}{lead}{label}");
        named += 1;
    }
    if named > 0 && named < dropped {
        note.push_str(&format!(" +{}", dropped - named));
    }
    for line in layout::wrap(&note, width) {
        lines.push(Line::styled(line, paint(Role::Quiet, tier)));
    }
}

/// The words the note opens the names of the cut entries with.
///
/// 🔴 Declared rather than spelled inline for the reason every other word on this face is: it is
/// read by `g65` to tell the note apart from an entry, and by the gate that refuses a road out of
/// the process on this page (`tools/gates/help_process_exit_gate.mjs`); a phrase spelled twice
/// diverges the day one of the two is edited. The phrase is this face's own — `help_lines`'s
/// obligation is written in the same words — rather than a third synonym beside the `not drawn` and
/// `not carried` entries, which name **wire fields** rather than entries of this hatch.
pub const LET_GO_LEAD: &str = " | let go: ";

/// The address that carries every key.
///
/// 🔴 Spellable because it is **gated**: `g12c` in `crates/gx-cli/tests/r942_tui_binding.rs`
/// requires the help text to name every declared act. A note that folds can point here and be
/// believed, which is the difference between disclosing a cut and waving at one.
///
/// The gate is in the *consumer's* suite since #188/#189 and that is the honest place for it: this
/// crate declares the acts, `gx-cli` prints the help, and the promise is checked where it is made.
pub const HELP_ADDRESS: &str = "gx tui --help";

/// The order the list's note spells acts in.
///
/// 🔴 [`Act::Leave`] first, and not as taste: the first thing a reader of a full-screen program
/// needs is the way out, and it is the one act offered in every state — including the one where the
/// engine answered with nothing at all.
///
/// [`Act::Close`] is in neither list. Closing a record that is not open moves nothing, and a legend
/// that names an inert key is a promise the face does not keep; the opened record's own note is what
/// names it.
/// ?? [`Act::Help`] sits **second**, straight after the way out: the two things a reader of a
/// full-screen program needs first are how to leave and how to find out what it does, and this
/// note is folded from the right, so an act placed late is an act a narrow terminal never names.
pub const NOTE_ORDER: [Act; 9] = [
    Act::Leave,
    Act::Help,
    Act::Open,
    Act::Next,
    Act::Prev,
    Act::Read,
    Act::First,
    Act::Last,
    Act::Wide,
];

/// The same, for a list the engine answered with nothing in.
///
/// Everything that moves the attention has nothing to move, and `act.open` opens nothing.
///
/// ?? **`act.help` and `act.wide` are deliberately absent** (`req/984` ?9-7). `super::acts::grounded`
/// clamps both on a list with nothing in it, so naming them here would advertise two keys that do
/// nothing -- exactly what gate g21 refuses, and exactly the defect this third rung was created
/// for one row count up. The address `gx tui --help` still reaches the same text from a shell.
pub const NOTE_ORDER_EMPTY: [Act; 2] = [Act::Leave, Act::Read];

/// The same, for a list of exactly one record.
///
/// 🔴 **The third rung, and gate g21 is what asked for it.** The repair this build made to
/// `super::acts::apply` closed the disagreement at nought records; running the same question over
/// every row count found it again one size up. On a list of one there is nowhere for the attention
/// to go, so `act.next`, `act.prev`, `act.first` and `act.last` are all inert — and this note was
/// naming all four of them. A legend that offers a key which does nothing teaches the reader that
/// the program is broken, which is the exact defect `super::acts` exists to prevent, arrived at
/// from the other side.
///
/// There is no fourth rung: from two records upward every declared act moves something.
pub const NOTE_ORDER_ONE: [Act; 5] = [Act::Leave, Act::Help, Act::Open, Act::Read, Act::Wide];

/// Which acts the list state offers, at this many rows.
///
/// 🔴 **Not the reducer's answer, and the disagreement is worth writing down.** `acts::apply`
/// reports that `act.open` moves the view on an empty list — it flips the bool — and this face then
/// declines to open anything, because [`subject`] opens only when `layout::subject_shape` says the
/// shape is a record.
/// A note derived from the reducer would therefore promise a key that does nothing on that screen.
/// The declaration lives in `super::acts` and reconciling the two is not a drawing decision, so the
/// disagreement is recorded here rather than papered over.
///
/// 🔴 **Closed, `req/38` SS974.** The reconciliation happened where the paragraph above said it
/// belonged: `acts::apply` now reads the row count for `act.open` as it already did for the four
/// acts that move the attention, so the reducer and this function agree without either of them
/// knowing about the other. The paragraph stays because it is the record of a defect that was named
/// before it was repaired — and because the *shape* of it is still the rule: a note derived from a
/// declaration that disagrees with the screen is a promise the face does not keep. Gate g21 is what
/// stops the two drifting apart again.
#[must_use]
pub fn offered(rows: usize) -> &'static [Act] {
    match rows {
        0 => &NOTE_ORDER_EMPTY,
        1 => &NOTE_ORDER_ONE,
        _ => &NOTE_ORDER,
    }
}

/// One act, as the note spells it: the key that produces it, and the act's own declared name with
/// the prefix off.
///
/// 🔴 Both halves come out of `super::acts`, so there is no second binding table and no second
/// vocabulary. A hand-spelled `Enter` here would be a key this face does not bind.
/// 🔴 And **no space inside it**, which is a wrapping fact rather than a style: `super::layout`'s
/// `wrap` breaks at spaces, and the first build of this note spelled `G last` and had the screen
/// break between the `G` and the `last`. A key severed from the act it produces is worse than no
/// legend, because it reads as a typo. The name comes first to match the opened record's own note,
/// which has spelled `close: escape` since before this line existed.
/// 🔴 **And three of them are a mark now** (`req/924` §TUI-45 row 4, `INHERITED_PRINCIPLES`
/// §3c-③''). The admission test is *a mark may replace a word only if the word then goes*; adding a
/// mark beside a word that stays is weight. [`ACT_MARKS`] is the declaration and
/// `super::tokens::GLYPHS` is where each one's meaning and the word it deleted are written down, so
/// the three-tuple the ruling asks for is a thing that is read rather than a claim that is made.
#[must_use]
pub fn spelled(act: Act) -> String {
    match act_mark(act) {
        // 🔴 The glyph **is** the key here, so nothing is spelled beside it. `↵` names the return
        // key, and the act it produces is the only thing that key does on this face — so `open` and
        // `return` both go, and eleven cells become one.
        Some(mark) if act == Act::Open => mark.to_string(),
        // The mark carries the act and the key stays, because `j` and `k` are not the arrows.
        Some(mark) => format!("{mark}:{}", act.keys()[0]),
        None => format!(
            "{}:{}",
            act.name().trim_start_matches("act."),
            act.keys()[0]
        ),
    }
}

/// The acts whose name a mark carries, and the mark.
///
/// 🔴 **Three, and the rest keep their words on purpose.** `first`/`last`/`read`/`wide`/`leave`/
/// `help` have no mark that carries them without a reader having to be taught it, and a mark that
/// has to be taught is a word plus a lookup. The ruling admits direction indicators and the return
/// key; it does not admit inventing a pictogram for *ask the engine again*.
///
/// Each mark's meaning and the words it deleted are declared in `super::tokens::GLYPHS`, which gate
/// P6 refuses to let stand empty — so the pairing here cannot become a mark nobody argued for.
/// 🔴 **The index is not the argument** (independent audit, finding 11). Reading `GLYPHS[5]` pairs
/// an act with a *position*, so swapping two entries of that array, or inserting one above them,
/// silently re-pairs all three — and the help face's `marks` line is generated from this same array,
/// so it would lie in the same direction. What ties the two is the declaration's own
/// `instead_of`, which names the word the mark deleted; gate `g17` requires it to name **this** act.
pub const ACT_MARKS: [(Act, &str); 3] = [
    (Act::Open, tokens::GLYPHS[5].text),
    (Act::Prev, tokens::GLYPHS[6].text),
    (Act::Next, tokens::GLYPHS[7].text),
];

/// The mark that carries this act's name, if one does.
#[must_use]
pub fn act_mark(act: Act) -> Option<&'static str> {
    ACT_MARKS
        .iter()
        .find(|(declared, _)| *declared == act)
        .map(|(_, mark)| *mark)
}

/// The note at one length: where the reader is, then the keys, then what was folded away.
#[must_use]
pub fn note_line(head: &str, acts: &[Act], spell: usize) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !head.is_empty() {
        parts.push(head.to_string());
    }
    // 🔴 The keys are one part, joined by the two spaces the apparatus head already joins its own
    // key/value pairs with, rather than by the pipe that separates the parts. With a pipe between
    // them the screen wrapped after a separator and left a bare `|` hanging at the end of a row,
    // which reads as a defect. Two spaces group without punctuating.
    // 🔴 **`help` is spelled in the road slot or in the legend, never in both.**
    //
    // At a hundred cells this line read `... | leave:q  help:? | 7 more keys: help:?` -- `help:?`
    // twice on one row, four cells apart. A signpost printed twice is not two signposts.
    //
    // The first repair of this deleted the road and kept the legend, and two gates refused it and
    // were right to: g34's partition (`declared = spelled + disclosed`) fell to `disclosed 0` in
    // 135 of 479 shapes, and g18's control stopped moving the line. `{n} more keys` says how many
    // went and not where they went, and *the line* disclosing them is not *the clause* disclosing
    // them. So the road stays and the **duplicate** goes: wherever there is a fold clause to carry
    // it, `Act::Help` comes out of the legend, because the clause is about to spell it anyway.
    //
    // The clause's text is unchanged at every shape -- the count below already discounted the act
    // the address spells. What changes is that the cells the duplicate held are spent on a key the
    // reader could not otherwise see.
    let folds = spell < acts.len();
    let in_place = acts.contains(&Act::Help);
    let hoisted = folds && in_place;
    let shown: Vec<Act> = acts
        .iter()
        .take(spell)
        .copied()
        .filter(|act| !(hoisted && *act == Act::Help))
        .collect();
    let keys = shown
        .iter()
        .copied()
        .map(spelled)
        .collect::<Vec<_>>()
        .join("  ");
    if !keys.is_empty() {
        parts.push(keys);
    }
    // 🔴 The fold names its own count and the address that has the rest. A legend that quietly
    // spells four of seven keys is a legend that has taught the reader there are four.
    //
    // `more` only once there is something for it to be more *than*. Dropping the word at the floor
    // is not tidying: it is five cells, and five cells is the difference between `7 keys: gx tui
    // --help` fitting a forty-cell row and the row being cut in the middle of `gx tui --he`, which
    // reads as a command rather than as a cut.
    if folds {
        let count = acts.len() - shown.len();
        // 🔴 **The address is the key when the key works here** (`req/942_artifacts/
        // sidebyside_round3_2026-09-01.md` §8-2). `?` has opened the help face in place since
        // `Act::Help` landed, and this line went on sending the reader to a command that ends the
        // process — the mechanism moved and the words did not. `HELP_ADDRESS` is still the road
        // when the list has nothing in it, because `super::acts::grounded` clamps `?` there and an
        // address that does nothing is worse than a long one.
        //
        // It is decided from `acts` rather than passed in: this function is handed the very list
        // the reducer offers, so the line cannot name a road the state does not have.
        let address = if in_place {
            spelled(Act::Help)
        } else {
            HELP_ADDRESS.to_string()
        };
        // 🔴 **An address that is itself a key is a key on the screen, and the count says so.**
        // `help:?` is `spelled(Act::Help)` character for character, so a line reading
        // `9 keys: help:?` was spelling one of the nine and counting all nine as missing — the
        // reader is told the way in and told, in the same breath, that they cannot see it. The
        // partition g34 measures (`declared = spelled + disclosed`) would also have come to ten
        // out of nine, which is how a count like this gets caught rather than argued about.
        // 🔴 It is `hoisted` rather than a search of the legend, which is the same fact read
        // from the other end: the legend no longer holds `Act::Help` wherever this clause exists,
        // because this clause is where it is drawn.
        let spoken = usize::from(hoisted);
        let count = count - spoken;
        if count == 0 {
            // The one act left over is the one the address spells, so the clause is the address.
            parts.push(address);
        } else if shown.is_empty() {
            parts.push(format!("{count} keys: {address}"));
        } else {
            parts.push(format!("{count} more keys: {address}"));
        }
    }
    parts.join(" | ")
}

/// How many rows the note is given, out of the rows the body was not going to spend on content.
///
/// `occupied` is what the region draws before the note: the records of a list, or the single
/// kind-of-nothing row an empty read draws.
///
/// 🔴 **The ruling, in a form a gate can read.** It is paid for out of **spare** rows and never out
/// of a record. The first build of this note took a row whenever it wanted one, and at 46x12 that
/// turned a list of three into a list of two in order to print a legend that then had no room to
/// name a single key — a strictly worse screen than the one it replaced. When the rows were already
/// overflowing the note costs nothing new: the last row was being spent on the count before the
/// legend existed, and the count now travels with the keys. Two rows at most, because a legend that
/// grows to fill a screen is furniture. Gate g26 is that sentence, fired over every shape.
///
/// 🔴 **A named defect, and it is left standing on purpose** (`req/964` §16, `req/38` SS999). At
/// `occupied == body_rows` the spare is nought, so the note is not drawn and **nothing on the screen
/// says it was there to draw**. Closing it needs one of two things, and this lane took neither:
/// letting the note take a record's row would reinstate exactly the 46x12 screen the ruling above
/// was written from, and saying so in the disclosure needs the drawn row count to reach
/// `super::layout::resolve`, which composes the disclosure that decides how many rows this region
/// gets — the order inversion §16 named. So the defect is **bounded** instead: g26 holds the set of
/// shapes where the note vanishes to exactly `occupied == body_rows`, and a reversal of the ruling
/// fires the same gate rather than passing quietly.
///
/// The budget never exceeds the rows that exist. It could before: an overflowing list one row tall
/// was handed a note row it did not have, and the terminal cut it without saying so.
#[must_use]
pub const fn note_rows(occupied: usize, body_rows: usize) -> usize {
    let want = if occupied > body_rows {
        1
    } else if body_rows - occupied > 2 {
        2
    } else {
        body_rows - occupied
    };
    if want > body_rows {
        body_rows
    } else {
        want
    }
}

/// The list note's head ladder: what it would say at every length, longest first, and what each
/// length is allowed to charge for itself.
///
/// 🔴 **The position is the floor of this ladder and the count of dropped rows is what gives way**
/// (`req/38` SS999, T-r4-A2). It was the other way round, and a single character was enough to
/// lose the position outright: at eighty cells
/// `record 1 of 28 | +12 more rows | GET /v1/transformations | 7 keys: gx tui --help` is eighty
/// characters and `record 28 of 28 | ...` is eighty-one, so the reader's position left the screen
/// at the exact moment the attention left the window.
///
/// The order is a mechanism and not a preference. `record N of M` says where the reader stands
/// *and*, against the rows a reader can count, that there are records not drawn — so it carries the
/// cut. `+K more rows` says nothing at all about where the reader stands. One implies the other and
/// not the reverse, so the one that survives is the position. The route the dropped rows come back
/// from is spelled by the disclosure region, which is `Priority::One` and always names
/// `LEDGER_ADDRESS` over a grid, because `LEDGER_PAGE_KEYS` is dropped at every width and that
/// clause is never empty.
///
/// This reverses the rung order argued for in [`fold_note`]'s own comment. The reversal is named
/// and dated rather than quiet: raised as T-r4-A2 in `req/38` SS999, repaired here, and gate g29 is
/// what stops it drifting back.
///
/// 🔴 **Composed here rather than in [`subject`] so that a gate can read the same ladder the screen
/// draws** (`req/984` §10-8). A gate that rebuilt these strings would be measuring its own copy,
/// and this repository has already shipped one gate that checked a declaration nothing read.
#[must_use]
/// 🔴 `_acts` since `req/924` §TUI-22: the list was read only by [`legend_floor`], to price the
/// rung that carried `LEDGER_ADDRESS`, and that rung no longer carries it. Every rung is free now
/// because every rung's own contribution is spelled nowhere else — which is what
/// [`Rung::keeps`] means. The parameter is kept rather than removed so the ladder's shape is still
/// a function of what the reducer offers if a priced rung is ever added back, and so the gates
/// that call this keep calling the same function.
pub fn note_ladder(position: &str, dropped: Option<usize>, _acts: &[Act]) -> Vec<Rung> {
    match dropped {
        // 🔴 `_more` since `req/924` §TUI-21: the count of rows let go of is no longer spelled on
        // its own rung. The `Option` is kept because *whether* rows were let go of still decides
        // which floor the ladder ends on — with nothing dropped the floor is the empty head and
        // the keys outrank the position (g18), and with rows dropped the position is the floor.
        Some(_more) => vec![
            // 🔴 **The one rung that may be asked to pay** (`req/984` §10-8). All this rung adds
            // over the next one down is `LEDGER_ADDRESS`, and the disclosure region on the same
            // screen spells that address too, so it is the only line in the ladder whose loss
            // costs the reader nothing they cannot read a few rows lower. Its price is
            // [`legend_floor`], and [`afford`] is where it is charged.
            // 🔴 **The address is off this rung** (`req/924` §TUI-21/§TUI-22, `req/38` SS1048 and
            // SS1049, Owner `#265-T`/`#266-T`). It was the third of five spellings of
            // `GET /v1/transformations` on one screen. The rung's own price paid for it —
            // `legend_floor` — on the argument that the disclosure spells the address too; the
            // disclosure no longer does either, because the **top rail** does, once. So the rung
            // is free now: all it adds over the next one down is the count of rows that were let
            // go of, and that count is spelled nowhere else.
            //
            // 🔴 **And `+K more rows` goes with it** (`req/924` §TUI-21's ① list, `req/38`
            // SS1048, Owner `#265-T`: *it duplicates `N of M`*). This is the ladder's own argument
            // read to its conclusion — the paragraph above already says `N of M` carries the cut,
            // *against the rows a reader can count*, and the count of rows that were let go of is
            // `M` minus those rows. The rung that spelled it separately was the same sentence
            // twice. What is left is one rung, which is the floor T-r4-A2 reordered this ladder to
            // protect.
            //
            // The other side, since the ruling is not free: a reader now subtracts rather than
            // reads. On a terminal one row tall the subtraction is `31 - 1`, which is a worse
            // reading than `+30 more rows` would have been. It is taken because five spellings of
            // one fact on one screen is the defect this lane was opened for, and this was the
            // fifth.
            Rung {
                head: position.to_string(),
                keeps: 0,
            },
        ],
        // The empty rung is reachable only when nothing was let go of, so giving the position up
        // for the keys costs the reader nothing that is not drawn elsewhere.
        None => vec![
            Rung {
                head: position.to_string(),
                keeps: 0,
            },
            Rung {
                head: String::new(),
                keeps: 0,
            },
        ],
    }
}

/// One rung of the note's head ladder, and what it must still leave room for to earn its cells.
///
/// 🔴 **The price is what makes this a declaration rather than a branch** (`req/984` §10-8/§10-9).
/// The ruling it carries is not about width: *what a line gives up may be given up only if the
/// same screen spells it somewhere else.* A rung whose own contribution is spelled elsewhere may
/// therefore be asked to pay for itself, and one whose contribution is spelled nowhere else may
/// not. That is a property of the **rung**, known where the ladder is written, and it is why this
/// face still contains no `if width < 80` — `super::layout`'s module documentation says why a
/// screen that writes one cannot name what it dropped.
///
/// `keeps` is a floor on the acts the rung must still be able to spell, and `0` means free: the
/// rung is taken whenever it fits, exactly as every rung was before this type existed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rung {
    /// The head this rung draws.
    pub head: String,
    /// Acts this rung must still spell to be worth taking, or it yields to the next rung down.
    pub keeps: usize,
}

/// The floor a redundant rung pays: enough keys to reach the one that says what the program does.
///
/// 🔴 **Derived from the declared order rather than written as a number.** `NOTE_ORDER` puts the
/// way out first and the way to find out what the program does second, and says why in its own
/// documentation; this reads that order back rather than restating it, so moving [`Act::Help`]
/// moves the floor with it and a hand-written `2` cannot drift from the list it describes.
///
/// A list of acts that does not offer help has no floor at all — `NOTE_ORDER_EMPTY` is that list,
/// and a rung priced against it is free, which is the right answer: there is no help key to buy.
#[must_use]
pub fn legend_floor(acts: &[Act]) -> usize {
    acts.iter()
        .position(|act| *act == Act::Help)
        .map_or(0, |at| at + 1)
}

/// How many acts a head can spell at this shape: `0` when the head does not fit at all.
#[must_use]
pub fn spellable(head: &str, acts: &[Act], width: u16, rows: usize) -> usize {
    if layout::rows_needed(&note_line(head, acts, 0), width) as usize > rows {
        return 0;
    }
    let mut most = 0;
    for spell in 1..=acts.len() {
        if layout::rows_needed(&note_line(head, acts, spell), width) as usize > rows {
            break;
        }
        most = spell;
    }
    most
}

/// The ladder with the rungs that cannot pay their price removed.
///
/// 🔴 **Pruning rather than a second walk.** [`fold_note`] is unchanged and still takes the first
/// rung that fits, longest first — g18 and g29 measure the same function they always did. What
/// changes is the ladder it is handed: a rung that fits but cannot spell what it declared it would
/// keep is not offered, so the walk arrives at the next rung down on its own. There is one
/// selection rule in this face, not two.
///
/// The last rung is never removed. It is the floor of the ladder, and a floor that can be priced
/// out is not a floor — at the narrowest shapes it is the only line left saying where the reader
/// stands.
#[must_use]
pub fn afford(ladder: &[Rung], acts: &[Act], width: u16, rows: usize) -> Vec<String> {
    let last = ladder.len().saturating_sub(1);
    ladder
        .iter()
        .enumerate()
        .filter(|(at, rung)| *at == last || spellable(&rung.head, acts, width, rows) >= rung.keeps)
        .map(|(_, rung)| rung.head.clone())
        .collect()
}

/// The longest note that fits the rows it was given: the first `head` that fits at all, then the
/// most acts that fit under it.
///
/// 🔴 `heads` is a ladder, longest first, for the same reason the disclosure has a long and a short
/// form. The first build of this note had one head, and at 46x12 against the live engine the head
/// alone needed three rows in the one row it had — so the line that says a record was let go of was
/// itself cut, mid-address. The defect `p12` guards against for the opened record, reintroduced next
/// to it.
///
/// **Named ceiling.** When no head fits, the shortest is drawn and the screen clips it, and nothing
/// says so: the region that would say it is composed in `super::layout::resolve`, which is not told
/// how many records there are. Same cause as the disclosure being wrong while a record is open.
#[must_use]
pub fn fold_note(heads: &[String], acts: &[Act], width: u16, rows: usize) -> String {
    // 🔴 The last rung is the caller's, not this function's, and that is the whole of the ordering
    // question. Where there is nothing to disclose the last rung is the **empty** head — the keys
    // outrank the position, because the attention mark also says where the reader stands and
    // nothing else says what the keys are (g18 caught the opposite arrangement dropping seven
    // declared acts in silence at thirty cells). Where records **were** let go of the last rung is
    // the line that says so, and it is never given up for a legend: a drop disclosure outranks a
    // convenience.
    //
    // 🔴 **Superseded in the caller, `req/38` SS999 T-r4-A2 -> [`subject`].** The paragraph above
    // is right that the last rung is the caller's and right that a drop disclosure outranks a
    // legend; it is wrong about which line is the drop disclosure. `record N of M` names the total
    // against the rows a reader can count, so it *is* a drop disclosure and it is also the only
    // line that says where the reader stands. The ladder `subject` now passes therefore ends with
    // the position rather than with `+K more rows`. This function is unchanged — it still walks
    // whatever ladder it is given, longest first — which is why g18 measures the same behaviour it
    // always did.
    //
    // **Named ceiling**: when even the last rung cannot carry the keys, it is drawn alone and the
    // keys go unmentioned. Nothing on the screen says so, for the same reason the disclosure is
    // wrong while a record is open — the region that would say it is composed in
    // `super::layout::resolve`, which is not told how many records there are.
    // 🔴 **The named ceiling above is closed** (`req/924` §TUI-57, `req/38` SS1088). It said: *when
    // even the last rung cannot carry the keys, it is drawn alone and the keys go unmentioned, and
    // nothing on the screen says so.* Measured on this lane's own first cut, at forty by ten: the
    // row read `1 of 31` and eight declared acts left the screen in silence — the very partition
    // gate g34 exists to hold (`declared = spelled + disclosed`).
    //
    // The floor is now the head **with its disclosure clause**, whether or not it fits. It is
    // allowed not to fit because the row that draws it marks a cut twice — `!` in front and `~`
    // where the cut fell (`layout::resolve_attended`) — and a marked cut of a clause that names
    // where the keys went is strictly better than an unmarked absence of one. The clause is
    // `{n} keys: {address}`, which is `note_line(last, acts, 0)`.
    // 🔴 **The floor spells the clause and not a key, and that was measured rather than
    // preferred** (independent audit, 2026-09-02, and the measurement that answered it). The audit
    // found `leave:q` unnamed at five of eight captured shapes and was right that the way out is
    // the act `NOTE_ORDER` puts first. The repair — make the floor spell the first act — was built
    // and **measured**, and at forty cells it cost the **road**: the row read
    // `1 of 31 | leave:q | 7 more keys: h~`, with `help:?` cut mid-word. Gates `g38`, `g40` and
    // `g65` all fired on it.
    //
    // So the floor stays `{n} keys: {address}`. The way out is **disclosed rather than spelled** at
    // the shapes that cannot hold both, and the disclosure names the key that reaches it — which is
    // `req/924` §TUI-21's own test, and `g76`/`g61` are what confirm the hatch really lists the acts.
    // A road that arrives outranks a key that is spelled while the road is cut.
    let last = heads.last().map_or("", String::as_str);
    let mut best = note_line(last, acts, 0);
    for head in heads.iter().map(String::as_str) {
        if layout::rows_needed(&note_line(head, acts, 0), width) as usize > rows {
            continue;
        }
        best = note_line(head, acts, 0);
        for spell in 1..=acts.len() {
            let candidate = note_line(head, acts, spell);
            if layout::rows_needed(&candidate, width) as usize > rows {
                break;
            }
            best = candidate;
        }
        break;
    }
    best
}

/// One row's cells, each in its own role, separated by the single space the plan budgeted for.
fn spans<'a>(cells: impl Iterator<Item = (String, Role)>, tier: Tier) -> Vec<Span<'a>> {
    spans_with(cells, tier, 1)
}

/// The same, with the gap the caller was budgeted.
///
/// 🔴 **The gap is a declared quantity now** (`req/924` §TUI-62 (`req/38` SS1093, Owner `#284-T`, 2026-09-01)).
/// A terminal has no line height, so the only room this face can make is between the columns and
/// before the first one; `super::layout::COLUMN_GAP` is what `columns_for_less` prices a column
/// against, so it has to be what draws one too. One is kept as the default because the standing
/// row's three parts are one sentence rather than a table.
fn spans_with<'a>(
    cells: impl Iterator<Item = (String, Role)>,
    tier: Tier,
    gap: u16,
) -> Vec<Span<'a>> {
    let mut out: Vec<Span> = Vec::new();
    for (text, role) in cells {
        if !out.is_empty() {
            out.push(Span::raw(" ".repeat(gap as usize)));
        }
        out.push(Span::styled(text, paint(role, tier)));
    }
    out
}

/// The cells before the first column of a ledger row.
///
/// 🔴 A cell of its own rather than a prefix glued to the first column's text (`req/924` §TUI-62,
/// `req/38` SS1093, Owner `#284-T`). `spans_with` is what puts the gap between cells, so a margin
/// pasted onto a value would be a value that is wider than its column — and `pad` would then cut a
/// character off the end of every id to make room for a space at the front.
fn margin() -> String {
    " ".repeat(layout::LEFT_MARGIN as usize)
}

/// Fit a value to its column.
///
/// A value wider than its column is cut, and the cut is **marked** with a trailing `~` rather than
/// performed silently.
///
/// Two ceilings, named rather than hidden:
/// * the number of cut values is not yet counted into the disclosure line, because the count is
///   only known after the rows are laid out and the disclosure's height is decided before that.
///   Upgrade path is a second layout pass.
/// * padding counts **codepoints**, which equals display width for everything inside this face's
///   declared budget but not for a wire value carrying a wide character — such a row would sit one
///   cell off. Upgrade path is `unicode-width`, already resolved in this workspace's `Cargo.lock`
///   at 0.2.2 and therefore a zero-package declaration behind the `tui` feature. It is not declared
///   today because nothing measured has produced such a value, and a dependency taken against a
///   defect nobody has seen is a dependency taken on a guess.
fn pad(text: &str, width: u16) -> String {
    let width = width as usize;
    let count = text.chars().count();
    if count > width {
        let head: String = text.chars().take(width.saturating_sub(1)).collect();
        format!("{head}~")
    } else {
        format!("{text:<width$}")
    }
}

/// Draw into an off-screen buffer of exactly this size.
///
/// 🔴 The same [`draw`] every live frame goes through. A capture produced by a second code path
/// would be a picture of a program that does not exist.
///
/// # Panics
/// Only if the in-memory backend fails to allocate, which it does not.
#[must_use]
pub fn render_to_buffer(
    screen: &Screen,
    width: u16,
    height: u16,
    tier: Tier,
    wide: bool,
) -> Buffer {
    render_view_to_buffer(screen, width, height, tier, wide, &View::default())
}

/// The same, for a reader who has moved: the frame a [`View`] produces.
///
/// # Panics
/// Only if the in-memory backend fails to allocate, which it does not.
#[must_use]
pub fn render_view_to_buffer(
    screen: &Screen,
    width: u16,
    height: u16,
    tier: Tier,
    wide: bool,
    view: &View,
) -> Buffer {
    render_live_to_buffer(
        screen,
        width,
        height,
        tier,
        wide,
        view,
        live::LinkReport::off(),
    )
}

/// The same, for a run whose subscription is in a given state.
///
/// 🔴 The road every other buffer function ends up in, so a probe that sweeps the five states of the
/// connection is drawing the frame the live face draws rather than a second rendering of it. That is
/// the same reason `--dump` goes through [`draw`].
///
/// # Panics
/// Only if the in-memory backend fails to allocate, which it does not.
#[must_use]
pub fn render_live_to_buffer(
    screen: &Screen,
    width: u16,
    height: u16,
    tier: Tier,
    wide: bool,
    view: &View,
    link: live::LinkReport,
) -> Buffer {
    // 🔴 The question `acts::apply` asks after a key is asked by [`render_held_to_buffer`] and by
    // the same function. A reading can carry fewer records than the last one did and nothing about
    // that goes through an act, so a view that was legal when the reader last touched it can arrive
    // at the draw pointing past the end of the list (`req/38` SS999, T-r9-B). Asked **before** the
    // plan there, because the plan is resolved for this attention and a plan resolved for one view
    // and drawn with another is two answers again.
    render_held_to_buffer(
        screen,
        &wire::Held::none(),
        width,
        height,
        tier,
        wide,
        view,
        link,
    )
}

/// The same, for a run in which a record has been opened and its own two routes have answered.
///
/// 🔴 **The road every other buffer function ends up in.** [`render_live_to_buffer`] passes
/// [`wire::Held::none`], which is what every frame with no record open is, so there is one drawing
/// road rather than two — the same reason `--dump` goes through [`draw`].
///
/// # Panics
/// Only if the in-memory backend fails to allocate, which it does not.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn render_held_to_buffer(
    screen: &Screen,
    held: &wire::Held,
    width: u16,
    height: u16,
    tier: Tier,
    wide: bool,
    view: &View,
    link: live::LinkReport,
) -> Buffer {
    let measured = measured_with_link(screen, link);
    let items = screen.transformations.items().len();
    let view = &acts::grounded(view, items);
    let (record_members, record_beyond) = record_extent(screen, held, view, width);
    let plan = layout::resolve_attended(
        width,
        height,
        &measured,
        // The invocation's flag **or** the act the reader pressed since; either is a request for
        // the long form and neither overrules the other. `resolve_attended` keeps its signature.
        wide || view.wide,
        layout::subject_shape(&screen.transformations, view),
        layout::Attention {
            selected: view.selected,
            items,
            glide: view.glide,
            record_members,
            record_beyond,
        },
    );
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("the in-memory backend does not fail");
    terminal
        .draw(|frame| draw(frame, screen, held, &plan, tier, view))
        .expect("the in-memory backend does not fail");
    terminal.backend().buffer().clone()
}

/// Take the terminal, draw until the reader leaves, and give it back.
///
/// Returns the number of frames drawn and the number of times the four routes were read, which is
/// what the invocation reports on the way out.
///
/// 🔴 The first frame goes up **before** the first read returns, and it is the only producer of
/// [`Nothing::Loading`] in this build. A face that showed an empty table while it waited would be
/// saying "nothing happened" about a question it had not yet asked.
///
/// # The loop waits on two things, and redraws for a third reason
///
/// 🔴 It used to block in `event::read()`, which is correct for a face whose only source of change
/// is a keypress and wrong for one the engine can talk to. It now waits [`WAKE`] for a key and,
/// when none arrives, asks the subscription whether anything happened. Three things make a frame:
/// a key, a re-read, and the connection's own state moving — the last of those matters because
/// `opening -> open -> closed` has to reach the screen even on a run where nobody touches the
/// keyboard and no event ever arrives.
///
/// 🔴 And the frame is drawn only when one of them happened. A loop that redrew on every wake would
/// make `frames` a count of elapsed time rather than a count of changes, which is the number this
/// verb reports on the way out.
///
/// # Errors
/// [`crate::Error::OutputFailed`] when the terminal cannot be measured, drawn on, or read from.
pub fn interactive(options: &super::Options, tier: Tier) -> crate::Result<(u64, u64)> {
    use ratatui::crossterm::event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseButton, MouseEventKind,
    };

    let mut terminal = ratatui::init();
    // 🔴 **The pointer, wired** (`req/924` §TUI-62 裁定2, `req/38` SS1093, Owner `#284-T`). The seat
    // had ruled it behind the input surface; the ruling withdraws that on the ground that **clicking
    // to select is a read** — it moves the attention and has no effect, so `req/924` §TUI-50's order
    // (an act with an effect comes after the consent screen) is untouched.
    //
    // Failure to enable it is **not** fatal and does not stop the face: a terminal that will not
    // report the pointer is a terminal where the keys still work, and refusing to draw at all would
    // be this face deciding that a reader without a mouse may not read.
    let mouse = ratatui::crossterm::execute!(std::io::stdout(), EnableMouseCapture).is_ok();
    let _ = mouse;
    let mut screen = Screen::pending();
    let mut view = View::default();
    let mut frames: u64 = 0;
    let mut reads: u64 = 0;
    // 🔴 **What the opened record's own two routes said, and it is cached against the id rather
    // than fetched per frame.** The four list routes are re-read on a subscription event; these two
    // are read when the record a reader is looking at *changes*, which is the only thing that makes
    // the answer stale in a way a frame can see. A face that asked again on every draw would put
    // two sockets between a keypress and the screen.
    let mut held = wire::Held::none();
    // Opened before the first frame, so that the first thing the screen says about the connection is
    // `opening` rather than a state invented to fill the gap.
    let subscription = Subscription::start(&options.base_url, options.token.as_deref());
    let mut link = subscription.report();
    let mut dirty = true;
    // The preamble rows still on the screen, and the slice of records under them, as of the last
    // frame that was actually drawn. `None` until there has been one.
    let mut hit: Option<(usize, super::layout::Window)> = None;
    let outcome = loop {
        if dirty {
            let size = match terminal.size() {
                Ok(size) => size,
                Err(e) => break Err(e),
            };
            let measured = measured_with_link(&screen, link);
            let items = screen.transformations.items().len();
            // 🔴 The subscription's road, closed. `Subscription::due` below re-reads and marks the
            // frame dirty without any act being applied, so this is where a list that shrank
            // between reads meets a view that was standing somewhere legal against the longer one.
            // The same function every key goes through, and the view itself is moved rather than a
            // grounded copy being drawn: the next key has to start from where the reader actually
            // is (`req/38` SS999, T-r9-B).
            view = acts::grounded(&view, items);
            let (record_members, record_beyond) = record_extent(&screen, &held, &view, size.width);
            let plan = super::layout::resolve_attended(
                size.width,
                size.height,
                &measured,
                options.wide || view.wide,
                super::layout::subject_shape(&screen.transformations, &view),
                super::layout::Attention {
                    selected: view.selected,
                    items,
                    glide: view.glide,
                    record_members,
                    record_beyond,
                },
            );
            // 🔴 **Where the records are on the screen, recorded from the frame that was drawn**
            // (`req/924` §TUI-62 裁定2). A click carries a row and nothing else, so something has to
            // say which record that row held — and it has to be the frame the reader clicked on
            // rather than a second computation of where the records *would* be. The subject region
            // is the first entry of `plan.rows` and therefore starts at row nought; the preamble the
            // scroll left on the screen sits above the records.
            hit = Some((plan.preamble_shown, plan.window));
            if let Err(e) = terminal.draw(|frame| draw(frame, &screen, &held, &plan, tier, &view)) {
                break Err(e);
            }
            frames += 1;
            dirty = false;
        }
        if screen.healthz.is_pending() {
            screen = Screen::read(&options.base_url, options.token.as_deref());
            reads += 1;
            dirty = true;
            continue;
        }
        // 🔴 **The record's own reads, decided from the state rather than from the act that
        // changed it.** `act.open`, `act.close`, `act.next` while open, a click while open and a
        // reading that emptied the list all move which record is being looked at, and asking each
        // of them to remember to fetch is five places that can disagree — the two-binding-tables
        // defect this face has already walked into three times. So the question is asked here, of
        // the view: *is the id on the screen the id these two readings are about?*
        let wanted = view
            .open
            .then(|| screen.transformations.items().get(view.selected).copied())
            .flatten()
            .and_then(|item| item.get(wire::LEDGER_ID_KEY))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        if wanted != held.id {
            held = if wanted.is_empty() {
                wire::Held::none()
            } else {
                reads += 1;
                wire::Held::read(&options.base_url, options.token.as_deref(), &wanted)
            };
            dirty = true;
            continue;
        }
        match event::poll(WAKE) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    // 🔴 The interrupt is the medium's, not an act. `Ctrl-C` stops a process in
                    // every terminal program a reader has ever used; declaring the convention as one
                    // of this face's capabilities would be claiming credit for the terminal's own
                    // manners.
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        break Ok(());
                    }
                    // Key -> name -> act -> effect. The binding table is `super::acts`; the only
                    // thing the medium contributes is how the bytes of a key are spelled.
                    let Some(act) = key_name(key.code).and_then(|name| acts::for_key(&name)) else {
                        continue;
                    };
                    let rows = screen.transformations.items().len();
                    let (next, signal) = acts::apply(&view, act, rows);
                    dirty |= next != view;
                    view = next;
                    match signal {
                        Signal::Read => {
                            screen = Screen::read(&options.base_url, options.token.as_deref());
                            reads += 1;
                            dirty = true;
                        }
                        Signal::Leave => break Ok(()),
                        Signal::None => {}
                    }
                }
                // 🔴 **A resize is not a key, and it is not nothing.** The loop reads
                // `terminal.size()` only on a dirty frame, and this arm was `Ok(_) => {}` -- so a
                // window made wider went on being drawn against the plan resolved for the old one,
                // for as long as nothing else happened to mark the frame dirty. On a quiet engine
                // that is indefinitely.
                //
                // Measured, and the first statement of this was too strong and is corrected here.
                // `req/942_artifacts/tui_r28_2026-09-01/RESIZE_PROOF.txt`, resizing a running
                // process from 80x24 to 120x32:
                //
                //   engine live, this arm absent : did not follow within 10s (2 of 2 runs)
                //   engine live, this arm present:  5ms, 7ms
                //   no engine,   this arm absent : 3181ms
                //   no engine,   this arm present:    6ms
                //
                // A third run of the absent case *did* follow within 3s, and that is the shape of
                // the defect rather than a contradiction: without this arm the frame is redrawn
                // only when something **else** dirties it, so the face follows a resize by luck --
                // whenever a subscription event or a change of link state happens to arrive. On an
                // idle engine nothing arrives and nothing is redrawn.
                //
                // Every ruled shape in this repository is measured by starting a *new* process at
                // that shape. No measurement anywhere in it resizes anything, which is why a suite
                // of 493 shapes could not see this.
                // 🔴 **The pointer** (`req/924` §TUI-62 裁定2/裁定3, `req/38` SS1093, Owner
                // `#284-T`). Two gestures, and both are reads:
                //
                // * **click** — attend to the record under it. `super::acts::attend` is the road,
                //   and it is a declared function rather than an [`Act`] because `ACTS` is the
                //   **key** binding table and a pointer carries a position no key can.
                // * **wheel** — move the face and leave the attention where it is.
                //   `super::acts::glide`. Scrolling down moves the content up, which is the
                //   direction the ruling names and the one `Claude Code` uses.
                //
                // A click on a row that holds no record is not an error and does nothing: the rows
                // above the records are the grid's own preamble, and attending to a column header
                // would be inventing a record.
                Ok(Event::Mouse(mouse)) => {
                    let rows = screen.transformations.items().len();
                    let next = match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            hit.and_then(|(preamble_shown, window)| {
                                let row = mouse.row as usize;
                                let index = row.checked_sub(preamble_shown)?;
                                (index < window.rows)
                                    .then(|| acts::attend(&view, window.first + index, rows))
                            })
                        }
                        MouseEventKind::ScrollDown => Some(acts::glide(&view, 1, rows)),
                        MouseEventKind::ScrollUp => Some(acts::glide(&view, -1, rows)),
                        _ => None,
                    };
                    if let Some(next) = next {
                        dirty |= next != view;
                        view = next;
                    }
                }
                Ok(Event::Resize(..)) => dirty = true,
                Ok(_) => {}
                Err(e) => break Err(e),
            },
            Ok(false) => {}
            Err(e) => break Err(e),
        }
        // 🔴 The event says "look again"; it does not say what to look at. The rows on the screen
        // are still whatever the four routes answered, which is what keeps the engine the only thing
        // that says what is true (`super::live::ON_EVENT`).
        if subscription.due() {
            screen = Screen::read(&options.base_url, options.token.as_deref());
            reads += 1;
            dirty = true;
        }
        let now = subscription.report();
        if now != link {
            link = now;
            dirty = true;
        }
    };
    // 🔴 Unconditionally, on the error road as well: a process that leaves the terminal in raw mode
    // and in the alternate screen has broken the operator's session, and it did so while reporting
    // a different failure. The pointer is released the same way and for the same reason: a terminal
    // left reporting mouse events writes escape bytes into whatever shell the reader gets back.
    let _ = ratatui::crossterm::execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    // 🔴 After the restore, and that order is the decision: dropping the subscription asks the
    // worker to stop and waits for it, and the longest that can take is one read window. The
    // operator gets the terminal back first and the wait happens on a shell they can already use.
    drop(subscription);
    outcome.map_err(|e| crate::Error::OutputFailed {
        detail: e.to_string(),
    })?;
    Ok((frames, reads))
}

/// A key, in the name `super::acts` declares its bindings with.
///
/// 🔴 This is the value layer of the act ladder and the reason it is **here**: `Enter` arriving as
/// `\r` and the up arrow arriving as three bytes are facts about terminals, and a declaration that
/// spelled them could not be read by a face drawn in any other medium. `None` for a key this face
/// binds nothing to — an unbound key is not an error, and beeping at one would be inventing a
/// refusal the reader did not earn.
fn key_name(code: ratatui::crossterm::event::KeyCode) -> Option<String> {
    use ratatui::crossterm::event::KeyCode;
    Some(match code {
        KeyCode::Char(character) => character.to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::Enter => "return".to_string(),
        KeyCode::Esc => "escape".to_string(),
        _ => return None,
    })
}

/// The seven marks, drawn through the very call a table cell draws them with.
///
/// P2 sweeps this over the four tiers. It is a probe of the **vocabulary** rather than of a whole
/// frame, because one of the six (`deleted`) is out of reach from these four routes — declared, and
/// gated as declared, by g4. Drawing it from wire data would mean inventing wire data that says a
/// row was removed, and no route says that.
#[must_use]
pub fn marks_buffer(tier: Tier) -> Buffer {
    let width: u16 = 8;
    let height = Nothing::ALL.len() as u16;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("the in-memory backend does not fail");
    terminal
        .draw(|frame| {
            // 🔴 Each mark in **its own declared role**, so that the sweep measures the marks as
            // they are drawn in a table rather than a colourless copy of them. P2 still compares
            // symbols alone on `mono`, which is the assertion that keeps a hue from becoming the
            // thing that tells two marks apart.
            let lines: Vec<Line> = Nothing::ALL
                .into_iter()
                .map(|nothing| {
                    Line::from(vec![Span::styled(
                        pad(nothing.mark(), width),
                        paint(nothing.role(), tier),
                    )])
                })
                .collect();
            frame.render_widget(Paragraph::new(lines), frame.area());
        })
        .expect("the in-memory backend does not fail");
    terminal.backend().buffer().clone()
}

/// The buffer as text, one line per row, trailing blanks kept so the grid is visible.
#[must_use]
pub fn buffer_text(buffer: &Buffer) -> String {
    let area = buffer.area;
    let mut out = String::new();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}
