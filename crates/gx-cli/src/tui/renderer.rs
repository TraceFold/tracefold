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
use super::tokens::{self, Role};
use super::wire::{self, Nothing, Reading, Screen};

/// How much colour the terminal can carry.
///
/// 🔴 Declared in `super::tokens` since `req/38` SS965 convert row (b) — a tier is the reader's
/// terminal, and the value a token takes in it belongs beside the token table rather than beside
/// the code that types escape sequences. Re-exported so that `renderer::Tier` is still the road.
pub use super::tokens::Tier;

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
#[must_use]
pub fn measured(screen: &Screen) -> Measured {
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
    let mut lines = vec![Line::styled(head, paint(Role::Head, tier))];
    for line in layout::wrap(&format!("status_reason {reason}"), area.width) {
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
    // One space between columns and none after the last: the width the plan computed is
    // `sum(width) + (n - 1)`, and a trailing separator would put the row one cell over the screen
    // the plan was asked about.
    lines.push(Line::from(spans(
        plan.columns
            .iter()
            .map(|column| (pad(column.key, column.width), Role::Head)),
        tier,
    )));

    let items = reading.items();
    let body_rows = area.height.saturating_sub(1) as usize;
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
    } else if view.open {
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
        // The last body row is spent on the count when there is more than the screen holds, so the
        // rows that were let go of are named with the route that brings them back.
        let overflow = items.len() > body_rows;
        let shown = if overflow {
            body_rows.saturating_sub(1)
        } else {
            items.len()
        };
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
        if overflow {
            lines.push(Line::styled(
                format!(
                    "+{} more rows | {}",
                    items.len() - shown,
                    layout::LEDGER_ADDRESS
                ),
                paint(Role::Quiet, tier),
            ));
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
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
    let measured = measured(screen);
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
/// # Errors
/// [`crate::Error::OutputFailed`] when the terminal cannot be measured, drawn on, or read from.
pub fn interactive(options: &super::Options, tier: Tier) -> crate::Result<(u64, u64)> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

    let mut terminal = ratatui::init();
    let mut screen = Screen::pending();
    let mut view = View::default();
    let mut frames: u64 = 0;
    let mut reads: u64 = 0;
    let outcome = loop {
        let size = match terminal.size() {
            Ok(size) => size,
            Err(e) => break Err(e),
        };
        let measured = measured(&screen);
        let plan = super::layout::resolve(size.width, size.height, &measured, options.wide);
        if let Err(e) = terminal.draw(|frame| draw(frame, &screen, &plan, tier, &view)) {
            break Err(e);
        }
        frames += 1;
        if screen.healthz.is_pending() {
            screen = Screen::read(&options.base_url, options.token.as_deref());
            reads += 1;
            continue;
        }
        match event::read() {
            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                // 🔴 The interrupt is the medium's, not an act. `Ctrl-C` stops a process in every
                // terminal program a reader has ever used; declaring the convention as one of this
                // face's capabilities would be claiming credit for the terminal's own manners.
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break Ok(());
                }
                // Key -> name -> act -> effect. The binding table is `super::acts`; the only thing
                // the medium contributes is how the bytes of a key are spelled.
                let Some(act) = key_name(key.code).and_then(|name| acts::for_key(&name)) else {
                    continue;
                };
                let rows = screen.transformations.items().len();
                let (next, signal) = acts::apply(&view, act, rows);
                view = next;
                match signal {
                    Signal::Read => {
                        screen = Screen::read(&options.base_url, options.token.as_deref());
                        reads += 1;
                    }
                    Signal::Leave => break Ok(()),
                    Signal::None => {}
                }
            }
            Ok(_) => {}
            Err(e) => break Err(e),
        }
    };
    // 🔴 Unconditionally, on the error road as well: a process that leaves the terminal in raw mode
    // and in the alternate screen has broken the operator's session, and it did so while reporting
    // a different failure.
    ratatui::restore();
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
