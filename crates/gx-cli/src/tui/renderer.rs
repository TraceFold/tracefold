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
//! [`Tier`] changes emphasis and nothing else. The six marks for nothing are drawn with no
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
    Measured {
        routes: readings.len(),
        read_at,
        worst_ms,
        statuses,
        link,
    }
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
            RegionRole::Apparatus => apparatus(frame, area, &screen.healthz, tier),
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
                let lines: Vec<Line> = layout::wrap(&text, area.width)
                    .into_iter()
                    .map(Line::raw)
                    .collect();
                frame.render_widget(Paragraph::new(lines).style(paint(Role::Quiet, tier)), area);
            }
        }
    }
}

/// `GET /v1/healthz`, in its own five keys.
fn apparatus(frame: &mut Frame, area: Rect, reading: &Reading, tier: Tier) {
    let body = reading.body.clone().unwrap_or(serde_json::Value::Null);
    let head = ["engine_version", "status", "ledger_agrees", "journal_rows"]
        .into_iter()
        .map(|key| {
            let cell = reading
                .nothing()
                .map_or_else(|| wire::cell(&body, key), wire::Cell::Nothing);
            format!("{key} {}", cell.text())
        })
        .collect::<Vec<_>>()
        .join("  ");
    let reason = reading.nothing().map_or_else(
        || wire::cell(&body, "status_reason").text(),
        |nothing| nothing.mark().to_string(),
    );
    // 🔴 The head is **wrapped**, not handed to the edge of the screen. It was clipped, silently,
    // at every width below sixty-six: measured on a real terminal at 46x12 the region drew
    // `engine_version 0.1.0  status ok  ledger_agrees` and the value of `ledger_agrees` and the
    // whole of `journal_rows 3` were gone — two of the engine's five facts about itself, dropped
    // with no mark and no line in the disclosure, by the region holding **two blank rows** at that
    // very moment. A face whose debt is disclosure cannot pay it and drop text off the right edge.
    //
    // The reason is measured first because it is the region's load-bearing fact when the engine is
    // not `ok`, and the head is what gives way. When the head does give way the cut is **marked**
    // with the same trailing `~` a table cell is cut with, through the same `pad`.
    let reason_lines = layout::wrap(&format!("status_reason {reason}"), area.width);
    let head_lines = layout::wrap(&head, area.width);
    let room = (area.height as usize)
        .saturating_sub(reason_lines.len())
        .max(1);
    let kept = head_lines.len().min(room);
    let mut lines: Vec<Line> = Vec::new();
    for (index, line) in head_lines.iter().take(kept).enumerate() {
        let cut = kept < head_lines.len() && index + 1 == kept;
        let text = if cut {
            pad(&format!("{line}~"), area.width)
        } else {
            line.clone()
        };
        lines.push(Line::styled(text, paint(Role::Head, tier)));
    }
    for line in reason_lines {
        lines.push(Line::raw(line));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// `GET /v1/transformations`, in the columns that fit.
///
/// 🔴 A row is a list of **spans** rather than one string since `req/38` SS965 convert row (b). The
/// difference is not decoration: a cell now carries the role of what it holds — the verdict's three
/// kinds and the fourth mark, and each of the six kinds of nothing — so the appearance of a cell is
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
    let open = view.open && !items.is_empty();
    // The note is composed and budgeted **before** the rows it sits under, for the same reason the
    // opened record's note is: it is the line that says where the reader is and what the screen let
    // go of, and a line written after the thing it describes is a line that gets clipped.
    if !open {
        // One space between columns and none after the last: the width the plan computed is
        // `sum(width) + (n - 1)`, and a trailing separator would put the row one cell over the
        // screen the plan was asked about.
        lines.push(Line::from(spans(
            plan.columns
                .iter()
                .map(|column| (pad(column.key, column.width), Role::Head)),
            tier,
        )));
    }

    let body_rows = area.height.saturating_sub(u16::from(!open)) as usize;
    if items.is_empty() {
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
        let note_rows = body_rows.saturating_sub(1).min(2);
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
        // Named ceiling: the disclosure line counts the **grid's** columns and does not subtract
        // what an opened record shows, so while a record is open the two disagree about one row.
        // Upgrade path is handing the view to `layout::resolve`, which is a signature three probes
        // already call.
        let index = view.selected.min(items.len() - 1);
        let members: Vec<(String, String)> = items[index]
            .as_object()
            .map(|map| {
                map.keys()
                    .map(|key| (key.clone(), wire::cell(items[index], key).text()))
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
        // 🔴 And it is paid for out of **spare** rows, never out of a record. The first build of
        // this note took a row whenever it wanted one, and at 46x12 that turned a list of three into
        // a list of two in order to print a legend that then had no room to name a single key — a
        // strictly worse screen than the one it replaced. When the rows were already overflowing the
        // note costs nothing new: the last row was being spent on the count before this existed, and
        // the count now travels with the keys.
        let overflowing = items.len() > body_rows;
        let note_rows = if overflowing {
            1
        } else {
            body_rows.saturating_sub(items.len()).min(2)
        };
        let shown = body_rows.saturating_sub(note_rows).min(items.len());
        for (index, item) in items.iter().enumerate().take(shown) {
            let cells = plan.columns.iter().map(|column| {
                if column.key == wire::VERDICT_KEY {
                    let verdict = wire::verdict(item);
                    (pad(&verdict.mark(), column.width), verdict.role())
                } else {
                    match wire::cell(item, column.key) {
                        wire::Cell::Nothing(nothing) => {
                            (pad(nothing.mark(), column.width), nothing.role())
                        }
                        wire::Cell::Value(text) => (pad(&text, column.width), Role::Body),
                    }
                }
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
        let position = format!("record {} of {}", index + 1, items.len());
        let heads = if shown < items.len() {
            // The rows that were let go of, named with the route that brings them back — the line
            // this build inherited — and the reader's position in front of it when there is room.
            let cut = format!(
                "+{} more rows | {}",
                items.len() - shown,
                layout::LEDGER_ADDRESS
            );
            vec![format!("{position} | {cut}"), cut]
        } else {
            // The empty rung is reachable only when nothing was let go of, so giving the position
            // up for the keys costs the reader nothing that is not drawn elsewhere.
            vec![position, String::new()]
        };
        if note_rows > 0 {
            for line in layout::wrap(
                &fold_note(&heads, offered(items.len()), area.width, note_rows),
                area.width,
            ) {
                lines.push(Line::styled(line, paint(Role::Quiet, tier)));
            }
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// The address that carries every key.
///
/// 🔴 Spellable because it is **gated**: `g12c` in `crates/gx-cli/tests/r942_tui.rs` requires the
/// help text to name every declared act. A note that folds can point here and be believed, which is
/// the difference between disclosing a cut and waving at one.
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
pub const NOTE_ORDER: [Act; 7] = [
    Act::Leave,
    Act::Open,
    Act::Next,
    Act::Prev,
    Act::Read,
    Act::First,
    Act::Last,
];

/// The same, for a list the engine answered with nothing in.
///
/// Everything that moves the attention has nothing to move, and `act.open` opens nothing.
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
pub const NOTE_ORDER_ONE: [Act; 3] = [Act::Leave, Act::Open, Act::Read];

/// Which acts the list state offers, at this many rows.
///
/// 🔴 **Not the reducer's answer, and the disagreement is worth writing down.** `acts::apply`
/// reports that `act.open` moves the view on an empty list — it flips the bool — and this face then
/// declines to open anything, because [`subject`] opens only when `view.open && !items.is_empty()`.
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
#[must_use]
pub fn spelled(act: Act) -> String {
    format!(
        "{}:{}",
        act.name().trim_start_matches("act."),
        act.keys()[0]
    )
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
    let keys = acts
        .iter()
        .take(spell)
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
    if spell < acts.len() {
        let count = acts.len() - spell;
        if spell == 0 {
            parts.push(format!("{count} keys: {HELP_ADDRESS}"));
        } else {
            parts.push(format!("{count} more keys: {HELP_ADDRESS}"));
        }
    }
    parts.join(" | ")
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
/// 🔴 The road every other buffer function ends up in, so a probe that sweeps the four states of the
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
    let plan = layout::resolve(width, height, &measured, wide);
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
            let plan = super::layout::resolve(size.width, size.height, &measured, options.wide);
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

/// The six marks, drawn through the very call a table cell draws them with.
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
