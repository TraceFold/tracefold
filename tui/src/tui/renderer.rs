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
        engine.insert(0, (badge.to_string(), format!("{rest}, {} events", link.events)));
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
pub fn draw(frame: &mut Frame, screen: &Screen, plan: &Plan, tier: Tier, view: &View) {
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
            RegionRole::Subject => subject(frame, area, &screen.transformations, plan, tier, view),
            RegionRole::Provenance => {
                frame.render_widget(
                    Paragraph::new(Line::raw(plan.provenance.clone()))
                        .style(paint(Role::Quiet, tier)),
                    area,
                );
            }
            RegionRole::Disclosure => {
                // 🔴 The screen being too small to hold even the floor is itself something the
                // screen has to say. One character, inside the budget, in front of the line that
                // exists to say what is missing.
                let text = if plan.truncated {
                    format!("! {}", plan.disclosure)
                } else {
                    plan.disclosure.clone()
                };
                // 🔴 The enclosure closes here (`req/924` §TUI-22). The line was **composed**
                // against `area.width - layout::FRAME_MARGIN` when `plan.framed` is set, so the two
                // corners are drawn in cells the disclosure was never offered — the mark cannot
                // push a clause off the row it discloses.
                let inner = if plan.framed {
                    area.width.saturating_sub(layout::FRAME_MARGIN)
                } else {
                    area.width
                };
                let mut wrapped = layout::wrap(&text, inner);
                // 🔴 **The enclosure closes on a row the reader can see.** `wrap` can answer with
                // more rows than this region was budgeted: the disclosure's height is settled in
                // `layout::resolve_attended`'s loop and the clause that says how many keys the note
                // never spelled is added **after** it, against the rows the loop produced — a
                // ceiling that module names in full. A corner placed on the last *wrapped* row is
                // then a corner on a row the `Paragraph` clips, and the ledger is open at the
                // bottom while `plan.framed` says it is closed.
                //
                // The cut is already marked — `plan.truncated` puts `!` in front of the line — so
                // what is left to get right is which row carries the corner, and the answer is the
                // last one that is drawn. Measured at 80x32 over a ledger of twenty-eight, where
                // this lane's shorter top rail (`engine ok`) bought the disclosure one row fewer
                // and took `┘` off the screen with it: gate `g60` caught it.
                if wrapped.len() > area.height as usize {
                    wrapped.truncate((area.height as usize).max(1));
                }
                let last = wrapped.len().saturating_sub(1);
                let lines: Vec<Line> = wrapped
                    .into_iter()
                    .enumerate()
                    .map(|(index, line)| {
                        if !plan.framed {
                            return Line::raw(line);
                        }
                        let open = if index == 0 {
                            tokens::CORNERS[2]
                        } else {
                            " "
                        };
                        let head = format!("{open} {line}");
                        if index != last {
                            return Line::raw(head);
                        }
                        let used = head.chars().count();
                        let corner = tokens::CORNERS[3];
                        let room = area.width as usize;
                        if used + 1 + corner.chars().count() <= room {
                            let gap = " ".repeat(room - used - corner.chars().count());
                            Line::raw(format!("{head}{gap}{corner}"))
                        } else {
                            Line::raw(head)
                        }
                    })
                    .collect();
                frame.render_widget(Paragraph::new(lines).style(paint(Role::Quiet, tier)), area);
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
/// `GET /v1/transformations` on its own is gone — the address is the rail's **title**, read out of
/// [`layout::heading`], and this is the one row on the screen that spells it. The row that spelled
/// `status_reason ?` on a healthy engine is gone for the reason [`RAIL_KEYS`] gives.
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
#[must_use]
pub fn cell_mark(item: &serde_json::Value, key: &str) -> (String, Role) {
    if key == wire::VERDICT_KEY {
        let verdict = wire::verdict(item);
        (verdict.mark(), verdict.role())
    } else if key == wire::INVERSE_STATUS_KEY {
        let inverse = wire::inverse_status(item);
        (inverse.mark(), inverse.role())
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
pub fn hoist(
    items: &[&serde_json::Value],
    columns: &[layout::Column],
    window: layout::Window,
    capacity: usize,
    selected: usize,
) -> (
    Vec<layout::Column>,
    Vec<(&'static str, String)>,
    layout::Window,
) {
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
    let (kept, shared) = layout::resolve_shared(columns, &marks);
    if shared.is_empty() {
        return (columns.to_vec(), Vec::new(), window);
    }
    if items.len() < capacity {
        // Spare capacity: the region was never going to fill every row it was budgeted, so the
        // shared row spends a row nothing else wanted.
        return (kept, shared, window);
    }
    if items.len() == capacity {
        // 🔴 **The list fills the region exactly, so paying with a record row cuts a list that
        // nothing on the screen says was cut** (`[T-r42]`, 2026-09-01, gate `g29` at 120x32).
        // `layout::resolve_attended` budgets the note with `super::renderer::note_rows(items,
        // body_rows)`, and at equality that function answers **nought rows** — its reading of
        // equality is *the list fits, so there is nothing to be the address of*. That reading is
        // true of every path into this region except this one: here the shared row takes a record
        // out of a window the note budget had already called whole, so the reader lost a record
        // and `N of M` was never drawn to say which one they are standing on.
        //
        // Measured rather than reasoned: at 120x32 over a ledger of twenty-eight, the frame drew
        // twenty-seven records, no note, and a disclosure that named the missing **keys** and not
        // the missing **record**.
        //
        // The early-out below is the same judgement already made once — *when the payment is
        // dearer than the line, do not hoist* — and this payment is the dearest available to a
        // renderer: a cut it cannot name. Refusing to hoist costs repeated ink at exactly one
        // shape per ledger length and is honest at all of them.
        return (columns.to_vec(), Vec::new(), window);
    }
    let shrunk = layout::window(selected, items.len(), capacity.saturating_sub(1));
    if shrunk.rows < 2 {
        // Two records still have to be drawn under the line — not because the claim needs them
        // (it is measured over all of `items` now), but because a grid showing one record plus a
        // summary of the ledger is a worse trade than a grid showing two records.
        return (columns.to_vec(), Vec::new(), window);
    }
    (kept, shared, shrunk)
}

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
    let names: usize = kept.iter().map(|column| column.width as usize + 1).sum();
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

/// `GET /v1/transformations`, in the columns that fit.
///
/// 🔴 A row is a list of **spans** rather than one string since `req/38` SS965 convert row (b). The
/// difference is not decoration: a cell now carries the role of what it holds — the verdict's three
/// kinds and the fourth mark, and each of the seven kinds of nothing — so the appearance of a cell is
/// a value the token table resolves rather than the absence of a decision.
fn subject(frame: &mut Frame, area: Rect, reading: &Reading, plan: &Plan, tier: Tier, view: &View) {
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
    let (columns, shared, window) = if grid && !items.is_empty() {
        hoist(
            &items,
            &plan.columns,
            plan.window,
            plan.grid_capacity,
            view.selected,
        )
    } else {
        (plan.columns.clone(), Vec::new(), plan.window)
    };
    // 🔴 **What is constant *in the window* is compressed, never folded** (`req/924` §TUI-45).
    // [`compress_window`] carries the whole of the argument and the ceiling it costs.
    let (columns,window_shared) = if grid && !items.is_empty() {
        compress_window(&items, &columns, window, area.width)
    } else {
        (columns, Vec::new())
    };
    // The note is composed and budgeted **before** the rows it sits under, for the same reason the
    // opened record's note is: it is the line that says where the reader is and what the screen let
    // go of, and a line written after the thing it describes is a line that gets clipped.
    if grid {
        // One space between columns and none after the last: the width the plan computed is
        // `sum(width) + (n - 1)`, and a trailing separator would put the row one cell over the
        // screen the plan was asked about.
        // 🔴 The compressed columns ride the row that already names the columns, so they cost
        // nought rows — and the boundary glyph is what says the row has two halves, which is the
        // same glyph and the same argument as the top rail's.
        let heads = columns
            .iter()
            .map(|column| (pad(column.key, column.width), Role::Head));
        let mut compressed: Vec<(String, Role)> = Vec::new();
        if !window_shared.is_empty() {
            let sample = items[window.first];
            // 🔴 No trailing space in any cell: `spans` is what puts one beside the next, and a
            // cell that ends in one draws two. Measured on a real terminal at 120x29, where the
            // first cut of this row read `│  these 24  verdict Admit |  state Committed`. The
            // separator is the one the hoisted row above already uses, for the same reason.
            compressed.push((tokens::RULE.to_string(), Role::Quiet));
            compressed.push((format!("{WINDOW_SCOPE} {}", window.rows), Role::Quiet));
            for (index, (key, mark)) in window_shared.iter().enumerate() {
                let (_, role) = cell_mark(sample, key);
                let sep = if index + 1 < window_shared.len() { " |" } else { "" };
                compressed.push((format!("{key} {mark}{sep}"), role));
            }
        }
        lines.push(Line::from(spans(
            heads.chain(compressed.into_iter()),
            tier,
        )));
        if !shared.is_empty() {
            // 🔴 One row, standing for every column every row on screen already agreed on
            // (`req/38` SS1019). `hoist` already paid for it out of `window` when there was no
            // spare capacity to take it from instead, so this never costs a row nothing else
            // budgeted. The role beside each mark is read from the first drawn row — every record
            // in `items` agrees by construction since `req/924` §TUI-20 moved the domain there, so
            // any one of them names the same appearance.
            let sample = items[window.first];
            // 🔴 **The quantifier, and it is the membrane's second obligation rather than a
            // decoration** (`req/924` §TUI-20). A renderer cannot invert, so what it owes instead
            // is to say what it let go of — and what this line lets go of is not a row, it is the
            // **scope of a claim**. Without the count in front, `verdict Admit` is a sentence about
            // the ledger; with it, it is a sentence about a set of a stated size. The words it
            // replaces are the ones a reader would otherwise have to go and find: *is this true of
            // the rows I cannot see?*
            let quantifier =
                std::iter::once((format!("{FETCHED_SCOPE} {}", items.len()), Role::Quiet));
            let fields = shared.iter().enumerate().map(|(index, (key, mark))| {
                let (_, role) = cell_mark(sample, key);
                let sep = if index + 1 < shared.len() { " |" } else { "" };
                (format!("{key} {mark}{sep}"), role)
            });
            lines.push(Line::from(spans(quantifier.chain(fields), tier)));
        }
    }

    // One row for the grid's own header, and none for the heading: that row is the top rail's now
    // (`req/924` §TUI-22). Read the same way `super::layout::resolve_attended` computes it, so the
    // region and the plan spend the same rows.
    let body_rows = area.height.saturating_sub(u16::from(grid)) as usize;
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
        // One row is occupied by the kind-of-nothing above; the note is paid for out of what is left
        // over, by the same rule the list's note is (`note_rows`).
        //
        // 🔴 **Read from the plan rather than recomputed** (`req/988` §3-2). The budget is still
        // [`note_rows`] and it still lives here; what changed is that `layout::resolve_attended`
        // calls it once and this region reads the answer. The disclosure has to say when this
        // number is nought, so the number the disclosure spoke about and the number the region drew
        // against must be the same one — and until this line they were not, because the plan was
        // passing the record count where this call site passes one.
        let note_rows = plan.note_rows;
        if note_rows > 0 {
            for line in layout::wrap(
                &fold_note(&[String::new()], offered(0), area.width, note_rows),
                area.width,
            ) {
                lines.push(Line::styled(line, paint(Role::Quiet, tier)));
            }
        }
    } else if open {
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
        let members: Vec<(String, String)> = items[index]
            .as_object()
            .map(|map| {
                map.keys()
                    .map(|key| (key.clone(), cell_mark(items[index], key).0))
                    .collect()
            })
            .unwrap_or_default();
        // The note is composed first because it is the line that says how many members were let go
        // of, and how many rows **it** needs is what decides that count. Written the other way
        // round the note would be one row long, be clipped by the region, and the number it carries
        // would be the one number on the screen that nothing checks.
        let note = |shown: usize| {
            format!(
                "record {} of {} | {shown} of {} members | {} | close: {}",
                index + 1,
                items.len(),
                members.len(),
                layout::LEDGER_ADDRESS,
                Act::Close.keys()[0]
            )
        };
        let note_rows = layout::rows_needed(&note(members.len()), area.width) as usize;
        let room = body_rows.saturating_sub(note_rows);
        let shown = members.len().min(room);
        for (key, value) in members.iter().take(shown) {
            lines.push(Line::raw(format!("{key} {value}")));
        }
        for line in layout::wrap(&note(shown), area.width) {
            lines.push(Line::styled(line, paint(Role::Quiet, tier)));
        }
    } else {
        // 🔴 The list's note, which the first build did not have: it appeared **only** on overflow,
        // so the entry face — the first thing anybody sees — named not one of the eight declared
        // acts. Eight capabilities, advertised nowhere, on a screen a reader cannot leave without
        // guessing. It now stands in every list state and carries the way out first.
        //
        // Two rows at most. A legend that grows to fill a screen is furniture, and the rows below it
        // are the ledger honestly saying that this is all there is.
        //
        // 🔴 And it is paid for out of **spare** rows, never out of a record — the ruling, and the
        // named defect it leaves standing where the records fill the body exactly, are both in
        // [`note_rows`], where a gate can read them instead of a reader having to.
        // 🔴 The plan's, for the reason the window below is (`req/988` §3-2): the line that says the
        // legend went is composed in `layout::resolve_attended`, so the count it is composed from
        // has to be the count this region spends.
        let note_rows = plan.note_rows;
        // 🔴 **The window is `hoist`'s, not necessarily the plan's** (`req/38` SS999, T-r4-B; and
        // `req/984` §10-33 for the row a shared line above may have already spent). This was
        // `take(shown)` from the first record, so an attention moved past the bottom edge was drawn
        // nowhere while the note went on reporting where it was — measured at 80x24 against a
        // twenty-eight row ledger, where `G` produced a frame identical to the entry frame.
        // `layout::window` is where the decision lives now, which is what lets a gate read it
        // instead of inferring it from a picture; `hoist` only ever shrinks it by the one row its
        // own shared line spent, and never below two, so a smaller window here is always paid for
        // by a line already standing above these rows.
        let shown = window.rows;
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
        let compressed_role = window_shared.iter().find_map(|(key, _)| {
            let (_, role) = cell_mark(items[window.first], key);
            (role != Role::Body).then_some(role)
        });
        for (index, item) in items
            .iter()
            .enumerate()
            .skip(window.first)
            .take(window.rows)
        {
            let cells = columns.iter().map(|column| {
                let (text, role) = cell_mark(item, column.key);
                // Only a cell with nothing of its own to say takes the lifted column's role.
                let role = match (role, compressed_role) {
                    (Role::Body, Some(lifted)) => lifted,
                    _ => role,
                };
                (pad(&text, column.width), role)
            });
            let line = Line::from(spans(cells, tier));
            // The attention mark is the row itself rather than a gutter column: a gutter would take
            // two cells from a width the plan already spent, and the plan is what says which columns
            // fit.
            lines.push(if index == view.selected {
                line.style(paint(Role::Attend, tier))
            } else {
                line
            });
        }
        let index = view.selected.min(items.len() - 1);
        // 🔴 `N of M` and not `record N of M` (`req/924` §TUI-21's target form, `req/38` SS1048,
        // Owner `#265-T`). The word named the thing the whole screen is a list of, on a page whose
        // title is `GET /v1/transformations` and whose column header begins `transformation`. It
        // is six cells spent saying a third time what the reader already knows they are looking at.
        let position = format!("{} of {}", index + 1, items.len());
        let acts = offered(items.len());
        let ladder = note_ladder(
            &position,
            items.len().checked_sub(shown).filter(|d| *d > 0),
            acts,
        );
        if note_rows > 0 {
            for line in layout::wrap(
                &fold_note(
                    &afford(&ladder, acts, area.width, note_rows),
                    acts,
                    area.width,
                    note_rows,
                ),
                area.width,
            ) {
                lines.push(Line::styled(line, paint(Role::Quiet, tier)));
            }
        }
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
    let mut entries: Vec<String> = Vec::new();
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
    // 🔴 **Named ceiling, and it is what this ordering costs.** Three entries moving up means
    // two moving down, and measured on a real terminal the two that stop being reached at 80x24
    // are `provenance` and `link.open`. The first has a road — the provenance **region** draws that
    // line on the same screen. The second does not: `live::LIVE_MEANS` is Owner #227's ruling that
    // a face saying `LIVE` must say *whose* events it counts, and it is spelled nowhere else. It is
    // still reached at 120x29, which is the shape that ruling was made on and the shape this bed is
    // read at, and the help note still spells `17 of 19` so the cut is marked rather than silent.
    // Closing it properly means a hatch that can be scrolled, which this face has no act for.
    entries.push(format!(
        "{} {}",
        pad("routes", 11),
        layout::READ_NOT_DRAWN.join(", ")
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
    entries.push(format!(
        "{} {}",
        pad("not drawn", 11),
        if names.is_empty() {
            wire::Nothing::Zero.mark().to_string()
        } else {
            names.join(", ")
        }
    ));
    // 🔴 The engine's own keys **by name**, and the condition the last of them is drawn under.
    // [`RAIL_KEYS`] explains the condition: an engine that is `ok` has no reason, and the face will
    // not assert `--` on the engine's behalf, so it draws nothing and says here that it does.
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
    entries.push(format!(
        "{} {} | every record in this reading answered with that mark and no answer was obtained, \
         so the column is not drawn at any width",
        pad("no answer", 11),
        if plan.vacant_fields.is_empty() {
            wire::Nothing::Zero.mark().to_string()
        } else {
            plan.vacant_fields
                .iter()
                .map(|(key, mark)| format!("{key} {mark}"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));
    // 🔴 The engine's line **as it stands underneath the fold**, values and all. `req/924` §TUI-29
    // folds the rail to `engine ok` while both claims hold, and SS842 is why that is only half a
    // decision: `engine_version` is spelled on no other row of any screen, so folding it without
    // moving it is a deletion. `plan.engine_full` is where it moved to, and this is where a reader
    // reaches it. The condition sentence is kept word for word — `g59` reads it.
    entries.push(format!(
        "{} {} | status_reason is on the rail only when status is not ok | folded to \
         `{ENGINE_LABEL} {HEALTHY}` while status is {HEALTHY} and ledger_agrees is {AGREES}",
        pad(ENGINE_LABEL, 11),
        plan.engine_full
            .iter()
            .map(|(key, value)| format!("{key} {value}"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    // 🔴 The marks that stand in for an act's name, with the name they stand in for. Read out of
    // the two declarations rather than spelled again: `ACT_MARKS` pairs them and
    // `super::tokens::GLYPHS` holds the meaning and the deleted word, so a mark added without an
    // argument fails P6 before it ever reaches this line.
    entries.push(format!(
        "{} {}",
        pad("marks", 11),
        ACT_MARKS
            .iter()
            .map(|(act, mark)| format!("{mark} {}", act.name().trim_start_matches("act.")))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    entries.extend(acts::ACTS.into_iter().map(|act| {
        format!(
            "{} {}  {}",
            pad(act.name().trim_start_matches("act."), 6),
            pad(&act.keys().join(" "), 14),
            act.intent()
        )
    }));
    entries.extend(layout::REGIONS.into_iter().map(|region| {
        format!(
            "{} {}",
            pad(region.role.short(), 11),
            region.intent.sentence()
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
    let provenance = format!(
        "{} {}",
        pad(layout::RegionRole::Provenance.short(), 11),
        plan.provenance_full
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
    entries.push(format!(
        "{} {}",
        pad(live::Link::Open.name(), 11),
        live::LIVE_MEANS
    ));

    // Composed before the rows it sits under, like every other note in this face: a line that says
    // how much was let go of, written after the letting go, is a line that gets clipped.
    //
    // 🔴 `acts in full:` and not a bare address. `gx tui --help` spells every declared act — gate
    // g12c in the consumer's suite is what makes that worth saying — and it spells **neither** of
    // the two lines above, which are measurements of this run. An address offered against a count
    // of sixteen would be claiming to answer for all sixteen; naming what it carries is four words
    // and the difference between a road and a wave.
    let note = |shown: usize| {
        format!(
            "{shown} of {} | close: {} | acts in full: {HELP_ADDRESS}",
            entries.len(),
            spelled(Act::Help)
        )
    };
    let note_rows = layout::rows_needed(&note(entries.len()), width) as usize;
    let room = body_rows.saturating_sub(note_rows);
    let mut spent = 0usize;
    let mut shown = 0usize;
    for entry in &entries {
        let need = layout::rows_needed(entry, width) as usize;
        if spent + need > room {
            break;
        }
        for line in layout::wrap(entry, width) {
            lines.push(Line::raw(line));
        }
        spent += need;
        shown += 1;
    }
    // The obligation a renderer carries in place of `invert`: say what was let go of. The count is
    // of declarations, and `HELP_ADDRESS` is where the ones that did not fit are spelled in full.
    for line in layout::wrap(&note(shown), width) {
        lines.push(Line::styled(line, paint(Role::Quiet, tier)));
    }
}

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
    let last = heads.last().map_or("", String::as_str);
    let mut best = if layout::rows_needed(&note_line(last, acts, 0), width) as usize > rows {
        last.to_string()
    } else {
        note_line(last, acts, 0)
    };
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
    let mut out: Vec<Span> = Vec::new();
    for (text, role) in cells {
        if !out.is_empty() {
            out.push(Span::raw(" "));
        }
        out.push(Span::styled(text, paint(role, tier)));
    }
    out
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
    let measured = measured_with_link(screen, link);
    let items = screen.transformations.items().len();
    // 🔴 The question `acts::apply` asks after a key, asked here as well and by the same function.
    // A reading can carry fewer records than the last one did and nothing about that goes through
    // an act, so a view that was legal when the reader last touched it can arrive at the draw
    // pointing past the end of the list (`req/38` SS999, T-r9-B). Asked **before** the plan,
    // because the plan is resolved for this attention and a plan resolved for one view and drawn
    // with another is two answers again.
    let view = &acts::grounded(view, items);
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
        },
    );
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("the in-memory backend does not fail");
    terminal
        .draw(|frame| draw(frame, screen, &plan, tier, view))
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
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

    let mut terminal = ratatui::init();
    let mut screen = Screen::pending();
    let mut view = View::default();
    let mut frames: u64 = 0;
    let mut reads: u64 = 0;
    // Opened before the first frame, so that the first thing the screen says about the connection is
    // `opening` rather than a state invented to fill the gap.
    let subscription = Subscription::start(&options.base_url, options.token.as_deref());
    let mut link = subscription.report();
    let mut dirty = true;
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
            let plan = super::layout::resolve_attended(
                size.width,
                size.height,
                &measured,
                options.wide || view.wide,
                super::layout::subject_shape(&screen.transformations, &view),
                super::layout::Attention {
                    selected: view.selected,
                    items,
                },
            );
            if let Err(e) = terminal.draw(|frame| draw(frame, &screen, &plan, tier, &view)) {
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
    // a different failure.
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
