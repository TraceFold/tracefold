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
    // 🔴 **The breadcrumb, in the row this region was already holding and not using**
    // (`req/988` §3-1). `REGIONS[Apparatus].min_rows` is three; at eighty cells the head takes one
    // row and the reason takes one, so the third has been blank on every frame this face has ever
    // drawn. A screen that never says which page it is on, holding an empty row to do it in.
    //
    // So the cost is **nought rows**: not one record leaves the ledger for it, and if the region
    // has no row to spare the address is simply not drawn. The word is `layout::LEDGER_ADDRESS`,
    // the same declared const the disclosure and the note already spell, so there is no second
    // spelling of the page's address for the two to disagree about.
    //
    // 🔴 **The ceiling this paragraph used to name is repaired** (`req/984` R13-1, T-r22). It read:
    // the spare row is a function of width, the head is sixty-seven characters, so at sixty-six
    // cells and below it wraps to two rows, the third row is spent, and the breadcrumb is gone — at
    // exactly the widths where a reader most needs to be told where they are. Below forty-six cells
    // the disclosure also falls to its short form, which does not spell the address either, so the
    // page's address was on **no row of the whole screen**. Gate g35 counted those shapes and
    // printed them; g40 is the same measurement turned into an assertion.
    //
    // A spare **row** was the wrong unit. At forty by ten this region is not out of room, it is out
    // of rows: the last row it draws is `status_reason ?` and twenty-five of its forty cells are
    // empty. So the address is offered a whole row when one is spare and the last drawn row's spare
    // cells when one is not — taken, in either case, only when it fits whole. The cost stays
    // **nought rows** at every width, which is the ruling the breadcrumb was admitted under and not
    // something this lane is free to spend.
    //
    // The row it lands on is always a reason row: `reason_lines` is `wrap` of a non-empty string so
    // it is never empty, and it is pushed after the head. That is why the appended cell can be
    // painted `Role::Quiet` and mean it — an unstyled `Line::raw` is what it joins.
    //
    // 🔴 **Named ceiling, and it is what this lane did not close.** A row whose spare cells are
    // fewer than the address is long has nowhere to put it, and the address is then still on no
    // row. Measured rather than assumed: g40 prints those shapes, and when it was written they were
    // the widths of thirty and below — where `status_reason`'s own row cannot hold twenty-three
    // cells — together with the shapes short enough for the fit loop to let go of this region
    // altogether, which is `Priority::Three` doing what it is declared to do. Reaching them means
    // widening the disclosure's short form, and that costs a whole region at forty rather than a
    // row: a worse screen than the one it repairs, so it is not taken here.
    let breadcrumb = Span::styled(layout::LEDGER_ADDRESS, paint(Role::Quiet, tier));
    if lines.len() < area.height as usize {
        lines.push(Line::from(breadcrumb));
    } else if let Some(last) = lines.last_mut() {
        if last.width() + 1 + layout::LEDGER_ADDRESS.chars().count() <= area.width as usize {
            last.spans.push(Span::raw(" "));
            last.spans.push(breadcrumb);
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
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
fn cell_mark(item: &serde_json::Value, key: &str) -> (String, Role) {
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
/// record, and only when a second record is still left to justify calling anything constant — a
/// window of one row proves nothing repeats. When the window has no spare capacity to give up for
/// free, this recomputes it one row smaller and asks the question again on the rows that actually
/// survive the smaller window, because a column that agreed across six rows is not guaranteed to
/// still agree across whichever five [`layout::window`] keeps once `selected` moves the slice.
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
    let marks = |window: layout::Window| -> Vec<Vec<String>> {
        items[window.first..window.first + window.rows]
            .iter()
            .map(|item| {
                columns
                    .iter()
                    .map(|column| cell_mark(item, column.key).0)
                    .collect()
            })
            .collect()
    };
    let (kept, shared) = layout::resolve_shared(columns, &marks(window));
    if shared.is_empty() {
        return (columns.to_vec(), Vec::new(), window);
    }
    if items.len() < capacity {
        // Spare capacity: the region was never going to fill every row it was budgeted, so the
        // shared row spends a row nothing else wanted.
        return (kept, shared, window);
    }
    let shrunk = layout::window(selected, items.len(), capacity.saturating_sub(1));
    if shrunk.rows < 2 {
        // No shape claims a constant from fewer than two rows.
        return (columns.to_vec(), Vec::new(), window);
    }
    let (kept2, shared2) = layout::resolve_shared(columns, &marks(shrunk));
    if shared2.is_empty() {
        (columns.to_vec(), Vec::new(), window)
    } else {
        (kept2, shared2, shrunk)
    }
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
    // 🔴 **The heading, and it stands over all three shapes** (Owner #227, 2026-09-01). It is the
    // row that answers *which screen is this, and which of the three am I on* — the question this
    // face could not answer below eighty cells, measured against twelve reference faces of which
    // six keep a named structure at forty by ten. Composed in `super::layout::heading` and only
    // bound here, so what a reader sees is a value a gate can read rather than a branch in a
    // drawing loop.
    lines.push(Line::from(spans(
        plan.heading
            .iter()
            .map(|cell| (cell.text.clone(), cell.role)),
        tier,
    )));
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
    // The note is composed and budgeted **before** the rows it sits under, for the same reason the
    // opened record's note is: it is the line that says where the reader is and what the screen let
    // go of, and a line written after the thing it describes is a line that gets clipped.
    if grid {
        // One space between columns and none after the last: the width the plan computed is
        // `sum(width) + (n - 1)`, and a trailing separator would put the row one cell over the
        // screen the plan was asked about.
        lines.push(Line::from(spans(
            columns
                .iter()
                .map(|column| (pad(column.key, column.width), Role::Head)),
            tier,
        )));
        if !shared.is_empty() {
            // 🔴 One row, standing for every column every row on screen already agreed on
            // (`req/38` SS1019). `hoist` already paid for it out of `window` when there was no
            // spare capacity to take it from instead, so this never costs a row nothing else
            // budgeted. The role beside each mark is read from the first drawn row — every row in
            // `window` agrees by construction, so any one of them names the same appearance.
            let sample = items[window.first];
            let fields = shared.iter().enumerate().map(|(index, (key, mark))| {
                let (_, role) = cell_mark(sample, key);
                let sep = if index + 1 < shared.len() { " |" } else { "" };
                (format!("{key} {mark}{sep}"), role)
            });
            lines.push(Line::from(spans(fields, tier)));
        }
    }

    // One row for the heading, which every shape draws, and one more for the grid's own header.
    let body_rows = area
        .height
        .saturating_sub(1)
        .saturating_sub(u16::from(grid)) as usize;
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
        for (index, item) in items
            .iter()
            .enumerate()
            .skip(window.first)
            .take(window.rows)
        {
            let cells = columns.iter().map(|column| {
                let (text, role) = cell_mark(item, column.key);
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
        let position = format!("record {} of {}", index + 1, items.len());
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
    entries.push(format!(
        "{} {}",
        pad(layout::RegionRole::Provenance.short(), 11),
        plan.provenance_full
    ));
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
pub fn note_ladder(position: &str, dropped: Option<usize>, acts: &[Act]) -> Vec<Rung> {
    match dropped {
        Some(more) => vec![
            // 🔴 **The one rung that may be asked to pay** (`req/984` §10-8). All this rung adds
            // over the next one down is `LEDGER_ADDRESS`, and the disclosure region on the same
            // screen spells that address too, so it is the only line in the ladder whose loss
            // costs the reader nothing they cannot read a few rows lower. Its price is
            // [`legend_floor`], and [`afford`] is where it is charged.
            Rung {
                head: format!(
                    "{position} | +{more} more rows | {}",
                    layout::LEDGER_ADDRESS
                ),
                keeps: legend_floor(acts),
            },
            // Free. `+K more rows` is the drop disclosure and is spelled nowhere else.
            Rung {
                head: format!("{position} | +{more} more rows"),
                keeps: 0,
            },
            // Free. `record N of M` is where the reader stands and is spelled nowhere else — the
            // floor T-r4-A2 reordered this ladder to protect.
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
