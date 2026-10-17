// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The change overlay: the form being filled in, then the sentence saying
//! exactly what `y` will send.

use super::pane;
use crate::tui::edit::{CREATABLE, Edit, Form};
use bacnet_probe::{names, props};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};

pub(super) fn draw(frame: &mut Frame<'_>, edit: &Edit) {
    let (title, mut lines) = match &edit.form {
        Form::Write(w) => {
            let mut lines = vec![
                row("object", w.label.clone(), false),
                row("now", w.shown.clone(), false),
            ];
            if !w.slots.is_empty() {
                lines.push(row("slots", w.slots.clone(), false));
            }
            lines.push(row(
                "value",
                cursor(&w.value, w.field == 0 && !edit.confirming),
                w.field == 0,
            ));
            lines.push(row(
                "priority",
                cursor(&w.priority, w.field == 1 && !edit.confirming),
                w.field == 1,
            ));
            lines.push(
                Line::from(
                    "value is read as the type it holds; null relinquishes; priority 1 is highest",
                )
                .dark_gray(),
            );
            (format!(" Write {} ", props::name(w.property)), lines)
        }
        Form::Create(c) => {
            let kind = names::object_type(CREATABLE.get(c.kind).copied().unwrap_or(2));
            let lines = vec![
                row("type", format!("< {kind} >"), c.field == 0),
                row(
                    "name",
                    cursor(&c.name, c.field == 1 && !edit.confirming),
                    c.field == 1,
                ),
                row(
                    "value",
                    cursor(&c.value, c.field == 2 && !edit.confirming),
                    c.field == 2,
                ),
                Line::from("← → change the type; the device chooses the instance").dark_gray(),
            ];
            (" Create an object ".to_owned(), lines)
        }
        Form::Delete(_) => (" Delete an object ".to_owned(), Vec::new()),
    };
    lines.push(Line::default());
    if let Some(e) = &edit.error {
        lines.push(Line::from(format!("✗ {e}")).fg(Color::Red));
    }
    if edit.confirming {
        lines.push(Line::from(edit.summary()).bold());
        lines.push(
            Line::from("This changes the equipment.  y send · any other key back")
                .fg(Color::Yellow),
        );
    } else {
        lines.push(Line::from("Enter review · Tab next field · Esc cancel").dark_gray());
    }
    let area = frame.area();
    let width = 76.min(area.width);
    // Rows after wrapping, not lines: the summary sentence is often longer
    // than the popup is wide, and the warning under it must stay visible.
    let inner = usize::from(width.saturating_sub(2)).max(1);
    let rows: usize = lines.iter().map(|l| l.width().div_ceil(inner).max(1)).sum();
    let height = u16::try_from(rows + 2).unwrap_or(u16::MAX).min(area.height);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, popup);
    let block = pane(title, true).style(Style::new().bg(Color::Black));
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block),
        popup,
    );
}

fn row(label: &str, value: String, active: bool) -> Line<'static> {
    let style = if active {
        Style::new().fg(Color::Black).bg(Color::Yellow)
    } else {
        Style::new()
    };
    Line::from(vec![
        Span::raw(format!("{label:<9}")).fg(Color::Cyan),
        Span::styled(value, style),
    ])
}

fn cursor(text: &str, on: bool) -> String {
    if on {
        format!("{text}▏")
    } else {
        text.to_owned()
    }
}
