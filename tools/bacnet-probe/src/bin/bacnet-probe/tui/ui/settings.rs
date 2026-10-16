// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The Settings tab: the network options on the left, and on the right what
//! the selected one does, whether the last apply took, how the socket is
//! open, and the command line that reproduces it.

use super::{pane, selected_style};
use crate::tui::app::App;
use crate::tui::settings::{FIELDS, Field, Form, Status, command_line, edit};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState, Wrap};

pub(super) fn settings(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Min(0)]).areas(area);
    let form = &app.settings;
    let rows: Vec<Row<'_>> = FIELDS
        .iter()
        .enumerate()
        .map(|(i, field)| row(form, i, *field))
        .collect();
    let title = if form.dirty() {
        " Settings (edited: a to apply, u to undo) ".to_owned()
    } else {
        " Settings ".to_owned()
    };
    let widths = [
        Constraint::Length(1),
        Constraint::Length(19),
        Constraint::Min(16),
        Constraint::Length(11),
    ];
    let table = Table::new(rows, widths)
        .header(Row::new(["", "SETTING", "VALUE", "FLAG"]).bold())
        .block(pane(title, true))
        .row_highlight_style(selected_style(form.editing.is_none()));
    let mut state = TableState::default().with_selected(Some(form.selected));
    frame.render_stateful_widget(table, left, &mut state);
    let about = Paragraph::new(details(form))
        .wrap(Wrap { trim: false })
        .block(pane(format!(" {} ", form.field().label().0), false));
    frame.render_widget(about, right);
}

fn row(form: &Form, i: usize, field: Field) -> Row<'static> {
    let error = form.errors.get(i).is_some_and(Option::is_some);
    let marker = if error {
        Cell::from("!").fg(Color::Red).bold()
    } else if form.changed(i) {
        Cell::from("*").fg(Color::Yellow).bold()
    } else {
        Cell::from(" ")
    };
    let text = form.text.get(i).cloned().unwrap_or_default();
    let value = if form.editing.is_some() && form.selected == i {
        Cell::from(format!("{text}▏"))
            .fg(Color::Black)
            .bg(Color::Yellow)
    } else if text.trim().is_empty() {
        Cell::from(placeholder(field)).dark_gray()
    } else if error {
        Cell::from(text).fg(Color::Red)
    } else {
        Cell::from(text)
    };
    let (label, flag) = field.label();
    Row::new([
        marker,
        Cell::from(label),
        value,
        Cell::from(flag).dark_gray(),
    ])
}

/// What an empty field means.
fn placeholder(field: Field) -> &'static str {
    match field {
        Field::Bind => "(auto)",
        Field::Timeout => "(1 s)",
        _ => "(none)",
    }
}

fn details(form: &Form) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(form.field().help())];
    if let Some(Some(error)) = form.errors.get(form.selected) {
        lines.push(Line::default());
        lines.push(Line::from(format!("✗ {error}")).fg(Color::Red));
    }
    lines.push(Line::default());
    lines.push(match &form.status {
        Status::Applying => Line::from("Applying: reopening the socket…").fg(Color::Yellow),
        Status::Failed(e) => Line::from(format!("Not applied: {e}")).fg(Color::Red),
        _ if form.dirty() => Line::from("Edited, not applied yet. a applies.").fg(Color::Yellow),
        Status::Applied => Line::from("Applied; rediscovering.").fg(Color::Green),
        Status::Idle => Line::from("Running with what is shown.").dark_gray(),
    });
    lines.push(Line::default());
    lines.push(Line::from("Connection").bold());
    lines.extend(form.notes.iter().map(|n| Line::from(n.clone()).dark_gray()));
    lines.push(Line::default());
    lines.push(Line::from("Running as").bold());
    lines.push(Line::from(command_line(&form.net, form.interval)).fg(Color::Cyan));
    lines.push(Line::default());
    lines.push(Line::from("Keys").bold());
    lines.extend(edit::KEYS.iter().map(|(k, what)| {
        Line::from(vec![
            Span::styled(format!("{k:<10}"), Style::new().fg(Color::Cyan)),
            Span::raw(*what),
        ])
    }));
    lines
}
